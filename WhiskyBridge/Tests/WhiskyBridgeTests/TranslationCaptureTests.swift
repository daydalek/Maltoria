//
//  TranslationCaptureTests.swift
//  WhiskyBridgeTests
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

// SPDX-License-Identifier: GPL-3.0-or-later
import Foundation
import Testing
@testable import WhiskyApplication
import WhiskyBridgeProtocol
import WhiskyTranslation
#if canImport(FoundationNetworking)
import FoundationNetworking
#endif

private let captureTarget: JSONValue = .object([
    "guestPID": .integer(42), "exePath": .string("C:\\Games\\日本語\\game.exe"),
    "arch": .string("x86"), "creationTime": .string("0123456789ABCDEF")
])
private let captureThread: JSONValue = .object([
    "guestPID": .integer(42), "addressHex": .string("0x0000000000001234"),
    "contextHex": .string("0x0000000000000000"), "context2Hex": .string("0x0000000000000000"),
    "hookName": .string("TextOutW")
])
private let captureThreadID = "0x0000000000001234:0x0000000000000000:0x0000000000000000"

private actor FixtureHookClient: HookClient {
    let pair = AsyncThrowingStream<JSONValue, any Error>.makeStream()
    var commands: [(String, [String: JSONValue])] = []
    var isClosed = false
    func ready() {}
    func events() -> AsyncThrowingStream<JSONValue, any Error> { pair.stream }
    func close() { isClosed = true
        pair.continuation.finish()
    }

    func request(_ command: String, params: [String: JSONValue]) throws -> JSONValue {
        guard !isClosed else { throw hookFailure("closed") }
        commands.append((command, params))
        if command == "listProcesses" { return .object(["processes": .array([captureTarget])]) }
        return .object([:])
    }

    func emit(_ event: JSONValue) { pair.continuation.yield(event) }
    func text(_ value: String, sequence: Int64) {
        emit(.object([
            "type": .string("text"),
            "thread": captureThread,
            "text": .string(value),
            "sequence": .integer(sequence)
        ]))
    }
}

private actor CaptureHTTP: HTTPTransport {
    var calls = 0
    var status = 200
    func fail() { status = 401 }
    func execute(_ request: URLRequest) -> HTTPResponse {
        calls += 1
        let body = "data: {\"choices\":[{\"delta\":{\"content\":\"你好，世界。\"}}]}\n\ndata: [DONE]\n\n"
        return HTTPResponse(
            statusCode: status,
            headers: ["Content-Type": "text/event-stream"],
            body: AsyncThrowingStream {
                $0.yield(Data(body.utf8))
                $0.finish()
            }
        )
    }
}

@MainActor
private final class CaptureHarness {
    let fixture: MetadataFixture
    let profiles: ApplicationTranslationService
    let http = CaptureHTTP()
    var clients: [FixtureHookClient] = []
    var launches: [(URL, URL, [String: String])] = []
    var stopped = 0
    var service: ApplicationCaptureService!
    var params: [String: JSONValue]
    init() async throws {
        fixture = try MetadataFixture()
        profiles = ApplicationTranslationService(
            directory: fixture.root.appendingPathComponent("Translation"),
            transport: http
        )
        let provider = ProviderConfiguration(
            kind: .openAIChat,
            baseURL: URL(string: "http://127.0.0.1:1234/v1")!,
            modelID: "fixture"
        )
        _ = try await profiles.call("translation.providers/save", params: ["provider": .encoded(provider)])
        params = fixture.params
        params["providerID"] = .string(provider.id.uuidString)
        service = ApplicationCaptureService(
            profiles: profiles,
            requirePayload: false,
            clientFactory: { [weak self] _, _ in
                let client = FixtureHookClient()
                self?.clients.append(client)
                return HookClientBootstrap(client: client, environment: ["MALTORIA_HOOK_TOKEN": "fixture-token"])
            },
            launcher: { [weak self] exe, prefix, environment in
                self?.launches.append((exe, prefix, environment))
                return TranslationHelperHandle { [weak self] in self?.stopped += 1 }
            }
        )
    }

    func call(_ method: String, _ params: [String: JSONValue] = [:]) async throws -> JSONValue {
        try await service.call(method, params: params, workspace: fixture.workspace)
    }

    func attach() async throws {
        _ = try await call("translation.capture.start", params)
        _ = try await call("translation.capture.attach", ["target": captureTarget])
        await clients.last!.emit(.object([
            "type": .string("thread"),
            "action": .string("created"),
            "thread": captureThread
        ]))
        try await wait { await (self.service.snapshot()).object?["threads"]?.array?.count == 1 }
    }

    func select() async throws {
        _ = try await call("translation.threads.select", ["selections": .array([
            .object(["threadID": .string(captureThreadID), "role": .string("body")])
        ])])
    }

    func wait(_ predicate: @MainActor () async -> Bool) async throws {
        for _ in 0 ..< 100 {
            if await predicate() { return }
            try await Task.sleep(for: .milliseconds(20))
        }
        throw hookFailure("test_timeout")
    }
}

@Test @MainActor func captureSelectedThreadTranslatesAndSwitchesHelperBitness() async throws {
    let harness = try await CaptureHarness()
    defer { harness.fixture.remove() }
    try await harness.attach()
    #expect(harness.launches.count == 2)
    #expect(harness.launches[0].0.lastPathComponent == "MaltoriaLunaHost64.exe")
    #expect(harness.launches[1].0.lastPathComponent == "MaltoriaLunaHost32.exe")
    #expect(harness.launches.allSatisfy {
        $0.1.resolvingSymlinksInPath().path == harness.fixture.bottle.resolvingSymlinksInPath().path
    })
    #expect(await harness.clients[0].isClosed)
    await harness.clients.last!.text("preview only", sequence: 1)
    try await harness
        .wait {
            await (harness.service.snapshot()).object?["threads"]?.array?.first?
                .object?["preview"] == .string("preview only")
        }
    #expect(await harness.http.calls == 0)
    try await harness.select()
    await harness.clients.last!.text("こんにちは、世界。", sequence: 2)
    try await harness
        .wait { await (harness.service.snapshot()).object?["subtitle"]?.object?["status"] == .string("completed") }
    let display = await harness.service.snapshot()
    #expect(display.object?["subtitle"]?.object?["source"] == .string("こんにちは、世界。"))
    #expect(display.object?["subtitle"]?.object?["translation"] == .string("你好，世界。"))
    await harness.clients.last!.text("こんにちは、世界。", sequence: 3)
    try await harness
        .wait { await (harness.service.snapshot()).object?["subtitle"]?.object?["status"] == .string("completed") }
    #expect(await harness.http.calls == 1)
    let history = try await harness.call("translation.history")
    let entry = try #require(history.object?["entries"]?.array?.first?.object?["id"]?.string)
    _ = try await harness.call("translation.history.include", ["entryID": .string(entry), "include": .bool(false)])
    let changed = try await harness.call("translation.history")
    #expect(changed.object?["entries"]?.array?.first?.object?["includeInContext"] == .bool(false))
    let events = try await harness.call("translation.events", ["afterCursor": .integer(0)])
    #expect(events.object?["events"]?.array?.isEmpty == false)
    await harness.service.stop()
    #expect(await harness.clients.last!.isClosed)
}

@Test @MainActor func captureFailurePreservesSourceAndTargetExitCancelsSession() async throws {
    let harness = try await CaptureHarness()
    defer { harness.fixture.remove() }
    await harness.http.fail()
    try await harness.attach()
    try await harness.select()
    await harness.clients.last!.text("原文は残る", sequence: 1)
    try await harness
        .wait { await (harness.service.snapshot()).object?["subtitle"]?.object?["status"] == .string("failed") }
    let display = await harness.service.snapshot()
    #expect(display.object?["subtitle"]?.object?["source"] == .string("原文は残る"))
    await harness.clients.last!.emit(.object([
        "type": .string("process"),
        "action": .string("disconnected"),
        "guestPID": .integer(42)
    ]))
    try await harness.wait { await (harness.service.snapshot()).object?["captureState"] == .string("targetExited") }
    #expect(await harness.clients.last!.isClosed)
    _ = try await harness.call("translation.history.clear")
    #expect(try await harness.call("translation.history").object?["total"] == .integer(0))
}

@Test @MainActor func captureRejectsUnlistedTargetAndOldSessionEvents() async throws {
    let harness = try await CaptureHarness()
    defer { harness.fixture.remove() }
    _ = try await harness.call("translation.capture.start", harness.params)
    var invalid = captureTarget.object!
    invalid["creationTime"] = .string("0000000000000000")
    await #expect(throws: BridgeFailure.self) {
        try await harness.call("translation.capture.attach", ["target": .object(invalid)])
    }
    let old = harness.clients[0]
    try await harness.attach()
    try await harness.select()
    await old.text("stale source", sequence: 99)
    await harness.clients.last!.text("new session", sequence: 1)
    try await harness
        .wait { await (harness.service.snapshot()).object?["subtitle"]?.object?["status"] == .string("completed") }
    #expect(await (harness.service.snapshot()).object?["subtitle"]?.object?["source"] == .string("new session"))
    #expect(await harness.http.calls == 1)
    await harness.service.stop()
}

@Test func captureFramingLimitsUnicodeAndTruncation() throws {
    var decoder = HookFrameDecoder()
    var frames: [JSONValue] = []
    for byte in "{\"text\":\"日本語\"}\r\n{}\n".utf8 {
        frames += try decoder.append(Data([byte]))
    }
    #expect(frames.count == 2)
    #expect(frames.first?.object?["text"] == .string("日本語"))
    try decoder.finish()
    var oversized = HookFrameDecoder()
    #expect(throws: BridgeFailure.self) { try oversized.append(Data(repeating: 65, count: 65_536)) }
    var truncated = HookFrameDecoder()
    _ = try truncated.append(Data("{".utf8))
    #expect(throws: BridgeFailure.self) { try truncated.finish() }
    var malformed = HookFrameDecoder()
    #expect(throws: BridgeFailure.self) { try malformed.append(Data([123, 255, 125, 10])) }
}

@Test func captureTargetIdentityAndOverlaySettingsValidation() throws {
    #expect(ApplicationCaptureService.validTarget(captureTarget))
    var invalid = captureTarget.object!
    invalid["guestPID"] = .integer(-1)
    #expect(!ApplicationCaptureService.validTarget(.object(invalid)))
    let settings = SubtitleOverlaySettings()
    #expect(try settings.merging(["fontSize": .integer(36)]).fontSize == 36)
    #expect(throws: BridgeFailure.self) { try settings.merging(["opacity": .number(2)]) }
    #expect(throws: BridgeFailure.self) { try settings.merging([
        "showSource": .bool(false),
        "showTranslation": .bool(false)
    ]) }
}
