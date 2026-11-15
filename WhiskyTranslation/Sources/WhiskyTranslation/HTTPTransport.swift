//
//  HTTPTransport.swift
//
//  This file is part of Maltoria, licensed under GPL-3.0-or-later.
//

import Foundation
#if canImport(FoundationNetworking)
import FoundationNetworking
#endif

public struct HTTPResponse: Sendable {
    public let statusCode: Int
    public let headers: [String: String]
    public let body: AsyncThrowingStream<Data, any Error>
    private let cancelBody: @Sendable () -> Void

    public init(
        statusCode: Int,
        headers: [String: String] = [:],
        body: AsyncThrowingStream<Data, any Error>,
        cancel: @escaping @Sendable () -> Void = {}
    ) {
        self.statusCode = statusCode
        self.headers = headers
        self.body = body
        cancelBody = cancel
    }

    public func header(_ name: String) -> String? {
        headers.first { $0.key.caseInsensitiveCompare(name) == .orderedSame }?.value
    }

    public func cancel() { cancelBody() }
}

public protocol HTTPTransport: Sendable {
    func execute(_ request: URLRequest) async throws -> HTTPResponse
}

public final class URLSessionStreamTransport: HTTPTransport, @unchecked Sendable {
    private let configuration: URLSessionConfiguration
    private let maximumBufferedChunks: Int
    private let maximumResponseBytes: Int

    public init(
        sessionConfiguration: URLSessionConfiguration? = nil,
        timeout: TimeInterval = 30,
        maximumBufferedChunks: Int = 64,
        maximumResponseBytes: Int = 4 * 1_024 * 1_024
    ) {
        let configuration = sessionConfiguration ?? URLSessionConfiguration.ephemeral
        let boundedTimeout = min(600, max(1, timeout))
        configuration.timeoutIntervalForRequest = boundedTimeout
        configuration.timeoutIntervalForResource = min(900, max(boundedTimeout, 60))
        configuration.httpCookieStorage = nil
        configuration.urlCache = nil
        self.configuration = configuration
        self.maximumBufferedChunks = min(1_024, max(1, maximumBufferedChunks))
        self.maximumResponseBytes = min(64 * 1_024 * 1_024, max(1_024, maximumResponseBytes))
    }

    public func execute(_ request: URLRequest) async throws -> HTTPResponse {
        let pipe = URLSessionResponsePipe(
            configuration: configuration,
            maximumBufferedChunks: maximumBufferedChunks,
            maximumResponseBytes: maximumResponseBytes
        )
        return try await withTaskCancellationHandler {
            try await withCheckedThrowingContinuation { continuation in
                pipe.start(request, responseContinuation: continuation)
            }
        } onCancel: {
            pipe.cancel()
        }
    }
}

private final class URLSessionResponsePipe: NSObject, URLSessionDataDelegate, @unchecked Sendable {
    private let lock = NSLock()
    private let configuration: URLSessionConfiguration
    private let maximumBufferedChunks: Int
    private let maximumResponseBytes: Int
    private var responseContinuation: CheckedContinuation<HTTPResponse, any Error>?
    private var bodyContinuation: AsyncThrowingStream<Data, any Error>.Continuation?
    private var bodyStream: AsyncThrowingStream<Data, any Error>?
    private var session: URLSession?
    private var task: URLSessionDataTask?
    private var cancelled = false
    private var receivedBytes = 0

    init(configuration: URLSessionConfiguration, maximumBufferedChunks: Int, maximumResponseBytes: Int) {
        self.configuration = configuration
        self.maximumBufferedChunks = maximumBufferedChunks
        self.maximumResponseBytes = maximumResponseBytes
    }

    func start(
        _ request: URLRequest,
        responseContinuation: CheckedContinuation<HTTPResponse, any Error>
    ) {
        let (stream, bodyContinuation) = AsyncThrowingStream<Data, any Error>.makeStream(
            bufferingPolicy: .bufferingOldest(maximumBufferedChunks)
        )
        bodyContinuation.onTermination = { [weak self] termination in
            if case .cancelled = termination { self?.cancel() }
        }

        lock.lock()
        if cancelled {
            lock.unlock()
            bodyContinuation.finish(throwing: CancellationError())
            responseContinuation.resume(throwing: CancellationError())
            return
        }
        self.responseContinuation = responseContinuation
        self.bodyContinuation = bodyContinuation
        bodyStream = stream
        let session = URLSession(configuration: configuration, delegate: self, delegateQueue: nil)
        let task = session.dataTask(with: request)
        self.session = session
        self.task = task
        lock.unlock()
        task.resume()
    }

    func cancel() {
        lock.lock()
        cancelled = true
        let task = self.task
        let responseContinuation = self.responseContinuation
        self.responseContinuation = nil
        let bodyContinuation = self.bodyContinuation
        self.bodyContinuation = nil
        let session = self.session
        lock.unlock()
        task?.cancel()
        responseContinuation?.resume(throwing: CancellationError())
        bodyContinuation?.finish(throwing: CancellationError())
        session?.invalidateAndCancel()
    }

    func urlSession(
        _: URLSession,
        dataTask _: URLSessionDataTask,
        didReceive response: URLResponse,
        completionHandler: @escaping (URLSession.ResponseDisposition) -> Void
    ) {
        guard let response = response as? HTTPURLResponse else {
            completionHandler(.cancel)
            finish(TranslationError.malformedResponse)
            return
        }
        var headers: [String: String] = [:]
        for (key, value) in response.allHeaderFields {
            if let key = key as? String { headers[key] = String(describing: value) }
        }
        lock.lock()
        let continuation = responseContinuation
        responseContinuation = nil
        let stream = bodyStream
        lock.unlock()
        guard let continuation, let stream else {
            completionHandler(.cancel)
            return
        }
        continuation.resume(returning: HTTPResponse(
            statusCode: response.statusCode,
            headers: headers,
            body: stream,
            cancel: { [weak self] in self?.cancel() }
        ))
        completionHandler(.allow)
    }

    func urlSession(
        _: URLSession,
        task _: URLSessionTask,
        willPerformHTTPRedirection _: HTTPURLResponse,
        newRequest _: URLRequest,
        completionHandler: @escaping (URLRequest?) -> Void
    ) {
        completionHandler(nil)
    }

    func urlSession(_: URLSession, dataTask _: URLSessionDataTask, didReceive data: Data) {
        lock.lock()
        receivedBytes += data.count
        let overLimit = receivedBytes > maximumResponseBytes
        let continuation = bodyContinuation
        lock.unlock()
        guard let continuation else { return }
        if overLimit {
            failBody(TranslationError.responseTooLarge)
            return
        }
        switch continuation.yield(data) {
        case .enqueued: break
        case .dropped: failBody(TranslationError.streamConsumerTooSlow)
        case .terminated: cancel()
        @unknown default: failBody(TranslationError.malformedResponse)
        }
    }

    func urlSession(_: URLSession, task _: URLSessionTask, didCompleteWithError error: (any Error)?) {
        finish(error)
    }

    private func failBody(_ error: any Error) {
        lock.lock()
        let task = self.task
        let continuation = bodyContinuation
        bodyContinuation = nil
        lock.unlock()
        continuation?.finish(throwing: error)
        task?.cancel()
    }

    private func finish(_ error: (any Error)?) {
        lock.lock()
        let responseContinuation = self.responseContinuation
        self.responseContinuation = nil
        let bodyContinuation = self.bodyContinuation
        self.bodyContinuation = nil
        let session = self.session
        lock.unlock()
        if let responseContinuation {
            responseContinuation.resume(throwing: error ?? TranslationError.malformedResponse)
        } else if let bodyContinuation {
            if let error { bodyContinuation.finish(throwing: error) } else { bodyContinuation.finish() }
        }
        session?.finishTasksAndInvalidate()
    }
}

public protocol RetryClock: Sendable {
    var now: Date { get async }
    func sleep(for duration: Duration) async throws
}

public struct SystemRetryClock: RetryClock {
    public init() {}
    public var now: Date { .now }
    public func sleep(for duration: Duration) async throws { try await Task.sleep(for: duration) }
}

struct RetryPolicy: Sendable {
    var maximumAttempts = 3
    var maximumDelaySeconds = 20.0

    func delay(attempt: Int, retryAfter: String?, now: Date = .now) -> Duration {
        if let retryAfter, let explicit = Self.retryAfter(retryAfter, now: now) {
            return .milliseconds(Int64(min(maximumDelaySeconds, max(0, explicit)) * 1_000))
        }
        let base = min(maximumDelaySeconds, 0.5 * pow(2, Double(max(0, attempt - 1))))
        let jitter = Double.random(in: 0 ... base * 0.2)
        return .milliseconds(Int64((base + jitter) * 1_000))
    }

    private static func retryAfter(_ value: String, now: Date) -> Double? {
        if let seconds = Double(value.trimmingCharacters(in: .whitespacesAndNewlines)), seconds >= 0 {
            return seconds
        }
        let formatter = DateFormatter()
        formatter.locale = Locale(identifier: "en_US_POSIX")
        formatter.timeZone = TimeZone(secondsFromGMT: 0)
        formatter.dateFormat = "EEE',' dd MMM yyyy HH':'mm':'ss 'GMT'"
        return formatter.date(from: value).map { max(0, $0.timeIntervalSince(now)) }
    }
}

func executeWithRetry(
    _ request: URLRequest,
    transport: any HTTPTransport,
    clock: any RetryClock,
    policy: RetryPolicy = RetryPolicy()
) async throws -> HTTPResponse {
    var attempt = 0
    while true {
        try Task.checkCancellation()
        attempt += 1
        do {
            let response = try await transport.execute(request)
            guard !(200 ..< 300).contains(response.statusCode) else { return response }
            let status = response.statusCode
            if status == 429, await responseIndicatesQuotaOrBilling(response.body) {
                response.cancel()
                throw TranslationError.quotaOrBillingLimit
            }
            let retryable = [408, 429, 500, 502, 503, 504].contains(status)
            guard retryable, attempt < policy.maximumAttempts else {
                response.cancel()
                throw TranslationError.httpStatus(status)
            }
            let delay = policy.delay(attempt: attempt, retryAfter: response.header("Retry-After"), now: await clock.now)
            response.cancel()
            try await clock.sleep(for: delay)
        } catch is CancellationError {
            throw CancellationError()
        } catch let error as TranslationError {
            throw error
        } catch let error as URLError {
            let retryable: Set<URLError.Code> = [.timedOut, .cannotConnectToHost, .networkConnectionLost, .notConnectedToInternet, .dnsLookupFailed]
            guard retryable.contains(error.code), attempt < policy.maximumAttempts else {
                throw TranslationError.transportFailure
            }
            try await clock.sleep(for: policy.delay(attempt: attempt, retryAfter: nil, now: await clock.now))
        } catch {
            throw TranslationError.transportFailure
        }
    }
}

private func responseIndicatesQuotaOrBilling(_ stream: AsyncThrowingStream<Data, any Error>) async -> Bool {
    let maximumBytes = 8 * 1_024
    var data = Data()
    do {
        for try await chunk in stream {
            let remaining = maximumBytes - data.count
            if remaining > 0 { data.append(contentsOf: chunk.prefix(remaining)) }
            if data.count >= maximumBytes { break }
        }
    } catch { return false }
    guard let root = try? JSONSerialization.jsonObject(with: data) as? [String: Any] else { return false }
    let error = root["error"] as? [String: Any] ?? root
    let identifiers = [error["code"], error["type"]].compactMap { $0 as? String }.map { $0.lowercased() }
    if identifiers.contains(where: { ["insufficient_quota", "billing_hard_limit_reached", "billing_not_active", "quota_exceeded", "billing_error"].contains($0) }) {
        return true
    }
    let message = (error["message"] as? String ?? "").lowercased()
    return message.contains("billing") || message.contains("insufficient_quota")
        || message.contains("quota exceeded") || message.contains("quota limit")
}
