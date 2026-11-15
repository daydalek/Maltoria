//
//  HTTPTransportTests.swift
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

#if os(macOS)
import Foundation
import Testing
@testable import WhiskyTranslation

private final class BlockingURLProtocol: URLProtocol, @unchecked Sendable {
    private static let lock = NSLock()
    private nonisolated(unsafe) static var started = false
    private nonisolated(unsafe) static var stopped = false

    static func reset() {
        lock.lock()
        started = false
        stopped = false
        lock.unlock()
    }

    static var didStart: Bool {
        lock.lock()
        defer { lock.unlock() }
        return started
    }

    static var didStop: Bool {
        lock.lock()
        defer { lock.unlock() }
        return stopped
    }

    override static func canInit(with request: URLRequest) -> Bool { true }
    override static func canonicalRequest(for request: URLRequest) -> URLRequest { request }

    override func startLoading() {
        Self.lock.lock()
        Self.started = true
        Self.lock.unlock()
    }

    override func stopLoading() {
        Self.lock.lock()
        Self.stopped = true
        Self.lock.unlock()
    }
}

private final class RedirectURLProtocol: URLProtocol, @unchecked Sendable {
    private static let lock = NSLock()
    private nonisolated(unsafe) static var location: URL?
    private nonisolated(unsafe) static var requestedHosts: [String] = []
    private nonisolated(unsafe) static var requestedURLs: [String] = []
    private nonisolated(unsafe) static var redirectAttempts = 0

    static func configure(location: URL) {
        lock.lock()
        self.location = location
        requestedHosts = []
        requestedURLs = []
        redirectAttempts = 0
        lock.unlock()
    }

    static func hosts() -> [String] {
        lock.lock()
        defer { lock.unlock() }
        return requestedHosts
    }

    static func urls() -> [String] {
        lock.lock()
        defer { lock.unlock() }
        return requestedURLs
    }

    static func attempts() -> Int {
        lock.lock()
        defer { lock.unlock() }
        return redirectAttempts
    }

    override static func canInit(with request: URLRequest) -> Bool { true }
    override static func canonicalRequest(for request: URLRequest) -> URLRequest { request }

    override func startLoading() {
        Self.lock.lock()
        Self.requestedHosts.append(request.url?.host ?? "")
        Self.requestedURLs.append(request.url?.absoluteString ?? "")
        Self.redirectAttempts += 1
        let location = Self.location
        Self.lock.unlock()
        guard let location, let url = request.url,
              let response = HTTPURLResponse(
                  url: url,
                  statusCode: 302,
                  httpVersion: "HTTP/1.1",
                  headerFields: ["Location": location.absoluteString]
              )
        else { return }
        client?.urlProtocol(self, wasRedirectedTo: URLRequest(url: location), redirectResponse: response)
        client?.urlProtocolDidFinishLoading(self)
    }

    override func stopLoading() {}
}

@Suite("URLSession streaming transport")
struct HTTPTransportTests {
    @Test func cancellingRequestCancelsUnderlyingURLSessionTask() async throws {
        BlockingURLProtocol.reset()
        let configuration = URLSessionConfiguration.ephemeral
        configuration.protocolClasses = [BlockingURLProtocol.self]
        let transport = URLSessionStreamTransport(sessionConfiguration: configuration, timeout: 20)
        let request = URLRequest(url: URL(string: "https://transport-fixture.invalid/stream")!)
        let task = Task { try await transport.execute(request) }
        for _ in 0 ..< 100 where !BlockingURLProtocol.didStart {
            try await Task.sleep(for: .milliseconds(5))
        }
        #expect(BlockingURLProtocol.didStart)
        task.cancel()
        do { _ = try await task.value
            Issue.record("cancelled request returned a response")
        } catch is CancellationError {}
        for _ in 0 ..< 100 where !BlockingURLProtocol.didStop {
            try await Task.sleep(for: .milliseconds(5))
        }
        #expect(BlockingURLProtocol.didStop)
    }

    @Test func crossHostAndInsecureRedirectsAreRefused() async throws {
        for target in [
            URL(string: "https://redirect-target.example.invalid/secret")!,
            URL(string: "http://configured.example.invalid/secret")!
        ] {
            RedirectURLProtocol.configure(location: target)
            let configuration = URLSessionConfiguration.ephemeral
            configuration.protocolClasses = [RedirectURLProtocol.self]
            let transport = URLSessionStreamTransport(sessionConfiguration: configuration, timeout: 2)
            var request = URLRequest(url: URL(string: "https://configured.example.invalid/start")!)
            request.setValue("synthetic-key", forHTTPHeaderField: "x-api-key")
            do {
                let response = try await transport.execute(request)
                #expect(response.statusCode == 302)
                response.cancel()
            } catch let error as TranslationError {
                #expect(error == .malformedResponse)
            }
            #expect(RedirectURLProtocol.attempts() == 1)
            #expect(RedirectURLProtocol.hosts() == ["configured.example.invalid"])
            #expect(RedirectURLProtocol.urls() == ["https://configured.example.invalid/start"])
        }
    }
}
#endif
