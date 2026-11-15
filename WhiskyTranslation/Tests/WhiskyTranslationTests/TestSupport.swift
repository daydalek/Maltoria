//
//  TestSupport.swift
//  WhiskyTranslationTests
//
//  This file is part of Whisky.
//
//  Whisky is free software: you can redistribute it and/or modify it under the terms
//  of the GNU General Public License as published by the Free Software Foundation,
//  either version 3 of the License, or (at your option) any later version.
//
//  Whisky is distributed in the hope that it will be useful, but WITHOUT ANY WARRANTY;
//  without even the implied warranty of MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.
//  See the GNU General Public License for more details.
//
//  You should have received a copy of the GNU General Public License along with Whisky.
//  If not, see https://www.gnu.org/licenses/.
//

import Foundation
#if canImport(FoundationNetworking)
import FoundationNetworking
#endif
import Testing
@testable import WhiskyTranslation

struct CapturedRequest: Sendable {
    let url: URL?
    let method: String?
    let headers: [String: String]
    let body: Data?

    init(_ request: URLRequest) {
        url = request.url
        method = request.httpMethod
        headers = (request.allHTTPHeaderFields ?? [:])
        body = request.httpBody
    }
}

actor QueueHTTPTransport: HTTPTransport {
    private var responses: [HTTPResponse]
    private var requests: [CapturedRequest] = []

    init(_ responses: [HTTPResponse]) { self.responses = responses }

    func execute(_ request: URLRequest) async throws -> HTTPResponse {
        requests.append(CapturedRequest(request))
        guard !responses.isEmpty else { throw URLError(.badServerResponse) }
        return responses.removeFirst()
    }

    func capturedRequests() -> [CapturedRequest] { requests }
}

actor FailingHTTPTransport: HTTPTransport {
    private let failure: URLError
    private(set) var requestCount = 0

    init(_ code: URLError.Code) { failure = URLError(code) }

    func execute(_: URLRequest) async throws -> HTTPResponse {
        requestCount += 1
        throw failure
    }
}

final class CancellationFlag: @unchecked Sendable {
    private let lock = NSLock()
    private var cancelled = false

    var value: Bool {
        lock.lock()
        defer { lock.unlock() }
        return cancelled
    }

    func cancel() {
        lock.lock()
        cancelled = true
        lock.unlock()
    }
}

actor PersistentTerminalHTTPTransport: HTTPTransport {
    private let cancellation = CancellationFlag()
    func wasCancelled() -> Bool { cancellation.value }

    func execute(_: URLRequest) async throws -> HTTPResponse {
        let (stream, continuation) = AsyncThrowingStream<Data, any Error>.makeStream()
        continuation.yield(Data("data: {\"choices\":[{\"delta\":{\"content\":\"done\"}}]}\n\ndata: [DONE]\n\n".utf8))
        return HTTPResponse(statusCode: 200, headers: ["Content-Type": "text/event-stream"], body: stream) {
            self.cancellation.cancel()
            continuation.finish()
        }
    }
}

actor FirstSaveBlockingRepository: TranslationRepository {
    private var shouldBlock = true
    private var firstSaveStarted = false
    private var startWaiter: CheckedContinuation<Void, Never>?
    private var releaseSave: CheckedContinuation<Void, Never>?
    private var histories: [UUID: TranslationSessionSnapshot] = [:]

    func saveConfiguration(_: ProviderConfiguration) async throws {}
    func loadConfiguration(id _: UUID) async throws -> ProviderConfiguration? { nil }
    func saveSettings(_: TranslationSettings) async throws {}
    func loadSettings() async throws -> TranslationSettings? { nil }

    func saveHistory(_ snapshot: TranslationSessionSnapshot) async throws {
        if shouldBlock {
            shouldBlock = false
            firstSaveStarted = true
            startWaiter?.resume()
            startWaiter = nil
            await withCheckedContinuation { releaseSave = $0 }
        }
        histories[snapshot.sessionID] = snapshot
    }

    func loadHistory(gameID: String, sessionID: UUID) async throws -> TranslationSessionSnapshot? {
        guard let snapshot = histories[sessionID], snapshot.gameID == gameID else { return nil }
        return snapshot
    }

    func clearHistory(gameID: String, sessionID: UUID) async throws {
        if histories[sessionID]?.gameID == gameID { histories.removeValue(forKey: sessionID) }
    }

    func waitUntilFirstSaveStarts() async {
        if firstSaveStarted { return }
        await withCheckedContinuation { startWaiter = $0 }
    }

    func releaseFirstSave() {
        releaseSave?.resume()
        releaseSave = nil
    }
}

actor ImmediateRetryClock: RetryClock {
    private(set) var delays: [Duration] = []
    var now: Date { Date(timeIntervalSince1970: 0) }
    func sleep(for duration: Duration) async throws { delays.append(duration) }
}

final class RecordingProvider: TranslationProvider, @unchecked Sendable {
    let kind: ProviderKind
    private let lock = NSLock()
    private var requests: [TranslationRequest] = []
    private let translatedText: @Sendable (TranslationRequest) -> String

    init(
        kind: ProviderKind = .openAIChat,
        translatedText: @escaping @Sendable (TranslationRequest) -> String = { "translated: \($0.sourceText)" }
    ) {
        self.kind = kind
        self.translatedText = translatedText
    }

    func recordedRequests() -> [TranslationRequest] {
        lock.lock()
        defer { lock.unlock() }
        return requests
    }

    func discoverModels(
        configuration _: ProviderConfiguration,
        credential _: String?
    ) async throws -> [ModelDescriptor] { [] }
    func modelInfo(
        id: String,
        configuration _: ProviderConfiguration,
        credential _: String?
    ) async throws -> ModelDescriptor {
        ModelDescriptor(id: id)
    }

    func translate(
        _ request: TranslationRequest,
        configuration _: ProviderConfiguration,
        credential _: String?
    ) -> AsyncThrowingStream<TranslationEvent, any Error> {
        lock.lock()
        requests.append(request)
        lock.unlock()
        return AsyncThrowingStream { continuation in
            let task = Task {
                let translation = translatedText(request)
                continuation.yield(.delta(translation))
                continuation.yield(.completed(translation))
                continuation.finish()
            }
            continuation.onTermination = { termination in
                if case .cancelled = termination { task.cancel() }
            }
        }
    }
}

final class ControlledProvider: TranslationProvider, @unchecked Sendable {
    let kind: ProviderKind = .openAIChat
    private let lock = NSLock()
    private var requests: [TranslationRequest] = []
    private var continuations: [UUID: AsyncThrowingStream<TranslationEvent, any Error>.Continuation] = [:]

    func recordedRequests() -> [TranslationRequest] {
        lock.lock()
        defer { lock.unlock() }
        return requests
    }

    func complete(eventID: UUID, text: String) {
        lock.lock()
        let continuation = continuations.removeValue(forKey: eventID)
        lock.unlock()
        continuation?.yield(.completed(text))
        continuation?.finish()
    }

    func discoverModels(
        configuration _: ProviderConfiguration,
        credential _: String?
    ) async throws -> [ModelDescriptor] { [] }
    func modelInfo(
        id: String,
        configuration _: ProviderConfiguration,
        credential _: String?
    ) async throws -> ModelDescriptor { ModelDescriptor(id: id) }

    func translate(
        _ request: TranslationRequest,
        configuration _: ProviderConfiguration,
        credential _: String?
    ) -> AsyncThrowingStream<TranslationEvent, any Error> {
        let (stream, continuation) = AsyncThrowingStream<TranslationEvent, any Error>.makeStream()
        lock.lock()
        requests.append(request)
        continuations[request.eventID] = continuation
        lock.unlock()
        return stream
    }
}

func makeHTTPResponse(
    _ body: String,
    status: Int = 200,
    headers: [String: String] = [:],
    chunkSize: Int = .max
) -> HTTPResponse {
    let data = Data(body.utf8)
    let size = max(1, min(chunkSize, max(1, data.count)))
    let chunks = stride(from: 0, to: data.count, by: size).map { start in
        Data(data[start ..< min(data.count, start + size)])
    }
    let (stream, continuation) = AsyncThrowingStream<Data, any Error>.makeStream()
    Task {
        for chunk in chunks {
            continuation.yield(chunk)
        }
        continuation.finish()
    }
    return HTTPResponse(statusCode: status, headers: headers, body: stream)
}

func testCredentialStore() async throws -> SessionOnlyCredentialStore {
    let store = SessionOnlyCredentialStore()
    try await store.store("fixture-secret-never-persist", for: "credential-ref")
    return store
}

func testTranslationRequest(
    sourceText: String = "hello",
    speaker: String? = "speaker",
    context: [TranslationContextTurn] = [],
    customPrompt: String = "",
    contextWindowTokens: Int = 4_096,
    outputTokenLimit: Int = 256
) -> TranslationRequest {
    TranslationRequest(
        gameID: "fixture-game",
        sessionID: UUID(),
        eventID: UUID(),
        generation: 1,
        sourceText: sourceText,
        speaker: speaker,
        sourceLanguage: "en",
        targetLanguage: "ja",
        context: context,
        customPrompt: customPrompt,
        outputTokenLimit: outputTokenLimit,
        contextWindowTokens: contextWindowTokens,
        contextWindowProvenance: .unknown
    )
}

func collectEvents(_ stream: AsyncThrowingStream<TranslationEvent, any Error>) async throws -> [TranslationEvent] {
    var result: [TranslationEvent] = []
    for try await event in stream {
        result.append(event)
    }
    return result
}

func waitForEntry(_ session: TranslationSession, id: UUID) async throws -> TranslationHistoryEntry {
    for _ in 0 ..< 100 {
        let entries = await session.history()
        if let entry = entries.first(where: { $0.id == id }), entry.status != .pending { return entry }
        try await Task.sleep(for: .milliseconds(5))
    }
    throw TranslationError.incompleteStream
}
