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
import WhiskyCore
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
    var listedProcesses: [JSONValue]
    var isClosed = false
    init(processes: [JSONValue] = [captureTarget]) { listedProcesses = processes }
    func setProcesses(_ processes: [JSONValue]) { listedProcesses = processes }
    func ready() {}
    func events() -> AsyncThrowingStream<JSONValue, any Error> { pair.stream }
    func close() { isClosed = true
        pair.continuation.finish()
    }

    func request(_ command: String, params: [String: JSONValue]) throws -> JSONValue {
        guard !isClosed else { throw hookFailure("closed") }
        commands.append((command, params))
        if command == "listProcesses" { return .object(["processes": .array(listedProcesses)]) }
        return .object([:])
    }

    func emit(_ event: JSONValue) { pair.continuation.yield(event) }
    func text(_ value: String, sequence: Int64, thread: JSONValue = captureThread) {
        emit(.object([
            "type": .string("text"),
            "thread": thread,
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
    // swiftlint:disable:next large_tuple
    var launches: [(URL, URL, [String: String])] = []
    var stopped = 0
    var listedProcesses: [JSONValue] = [captureTarget]
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
                let client = FixtureHookClient(processes: self?.listedProcesses ?? [captureTarget])
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

    func attach(thread: JSONValue = captureThread) async throws {
        _ = try await call("translation.capture.start", params)
        _ = try await call("translation.capture.attach", ["target": captureTarget])
        await clients.last!.emit(.object([
            "type": .string("thread"),
            "action": .string("created"),
            "thread": thread
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
    #expect(settings.textColor == "#FFFFFF")
    #expect(settings.sourceTextColor == "#D4D4D8")
    #expect(try settings.merging(["fontSize": .integer(36)]).fontSize == 36)
    #expect(try settings.merging(["opacity": .number(0)]).opacity == 0)
    #expect(try settings.merging(["textColor": .string("#12abEF"), "sourceTextColor": .string("#000000")])
        .textColor == "#12abEF")
    #expect(throws: BridgeFailure.self) { try settings.merging(["opacity": .number(2)]) }
    #expect(throws: BridgeFailure.self) { try settings.merging(["opacity": .number(-0.01)]) }
    #expect(throws: BridgeFailure.self) { try settings.merging(["textColor": .string("FFFFFF")]) }
    #expect(throws: BridgeFailure.self) { try settings.merging(["textColor": .string("#FFF")]) }
    #expect(throws: BridgeFailure.self) { try settings.merging(["sourceTextColor": .string("#12345G")]) }
    #expect(throws: BridgeFailure.self) { try settings.merging([
        "showSource": .bool(false),
        "showTranslation": .bool(false)
    ]) }

    let legacyJSON = #"{"visible":true,"clickThrough":false,"fontSize":24,"opacity":0.5}"#
    let decodedLegacy = try JSONDecoder().decode(SubtitleOverlaySettings.self, from: Data(legacyJSON.utf8))
    #expect(decodedLegacy.textColor == "#FFFFFF")
    #expect(decodedLegacy.sourceTextColor == "#D4D4D8")
    #expect(decodedLegacy.fontSize == 24)
}

@Test @MainActor func capturePresetSaveGetClearAreIsolatedPerGame() async throws {
    let harness = try await CaptureHarness()
    let otherGame = try MetadataFixture()
    defer {
        harness.fixture.remove()
        otherGame.remove()
    }
    let otherWorkspace = ApplicationWorkspace(containers: [harness.fixture.bottle, otherGame.bottle])
    let providerID = try #require(harness.params["providerID"]?.string)

    let empty = try await harness.service.call(
        "translation.capture.preset/get",
        params: harness.fixture.params,
        workspace: otherWorkspace
    )
    #expect(empty.object?["preset"] == .null)

    let savedA = try await harness.service.call(
        "translation.capture.preset/save",
        params: [
            "container": .string(harness.fixture.bottle.path),
            "entryID": .string(harness.fixture.entryID),
            "autoAttach": .bool(true),
            "restoreHistory": .bool(true),
            "architecture": .string("x86"),
            "providerID": .string(providerID)
        ],
        workspace: otherWorkspace
    )
    #expect(savedA.object?["preset"]?.object?["autoAttach"] == .bool(true))
    #expect(savedA.object?["preset"]?.object?["restoreHistory"] == .bool(true))
    #expect(savedA.object?["preset"]?.object?["architecture"] == .string("x86"))

    let fetchedB = try await harness.service.call(
        "translation.capture.preset/get",
        params: otherGame.params,
        workspace: otherWorkspace
    )
    #expect(fetchedB.object?["preset"] == .null)

    _ = try await harness.service.call(
        "translation.capture.preset/save",
        params: [
            "container": .string(otherGame.bottle.path),
            "entryID": .string(otherGame.entryID),
            "autoAttach": .bool(false),
            "architecture": .string("x64")
        ],
        workspace: otherWorkspace
    )
    let fetchedA = try await harness.service.call(
        "translation.capture.preset/get",
        params: harness.fixture.params,
        workspace: otherWorkspace
    )
    #expect(fetchedA.object?["preset"]?.object?["architecture"] == .string("x86"))
    #expect(fetchedA.object?["preset"]?.object?["autoAttach"] == .bool(true))

    let clearedA = try await harness.service.call(
        "translation.capture.preset/clear",
        params: harness.fixture.params,
        workspace: otherWorkspace
    )
    #expect(clearedA.object?["cleared"] == .bool(true))
    #expect(try await harness.service.call(
        "translation.capture.preset/get",
        params: harness.fixture.params,
        workspace: otherWorkspace
    ).object?["preset"] == .null)
    #expect(try await harness.service.call(
        "translation.capture.preset/get",
        params: otherGame.params,
        workspace: otherWorkspace
    ).object?["preset"]?.object?["architecture"] == .string("x64"))
}

@Test @MainActor func captureAutoAttachReinjectsHooksAndAutoBindsAcrossASLRRestarts() async throws {
    let harness = try await CaptureHarness()
    defer { harness.fixture.remove() }

    _ = try await harness.call("translation.capture.start", harness.params)
    _ = try await harness.call("translation.capture.attach", ["target": captureTarget])
    _ = try await harness.call("translation.capture.insertHook", ["hookCode": .string("HS8@-4:game.exe")])

    let customThread: JSONValue = .object([
        "guestPID": .integer(42),
        "addressHex": .string("0x0000000000001234"),
        "contextHex": .string("0x0000000000000010"),
        "context2Hex": .string("0x0000000000000020"),
        "hookCode": .string("HS8@-4:game.exe"),
        "hookName": .string("UserHook0")
    ])
    let speakerThread: JSONValue = .object([
        "guestPID": .integer(42),
        "addressHex": .string("0x0000000000005678"),
        "contextHex": .string("0x0000000000000030"),
        "context2Hex": .string("0x0000000000000040"),
        "hookName": .string("TextOutW")
    ])
    let client1 = harness.clients.last!
    await client1.emit(.object(["type": .string("thread"), "action": .string("created"), "thread": customThread]))
    await client1.emit(.object(["type": .string("thread"), "action": .string("created"), "thread": speakerThread]))
    try await harness.wait { await (harness.service.snapshot()).object?["threadsTotal"] == .integer(2) }

    _ = try await harness.call("translation.threads.select", ["selections": .array([
        .object([
            "threadID": .string("0x0000000000001234:0x0000000000000010:0x0000000000000020"),
            "role": .string("body")
        ]),
        .object([
            "threadID": .string("0x0000000000005678:0x0000000000000030:0x0000000000000040"),
            "role": .string("speaker")
        ])
    ])])

    let savedPreset = try await harness.call("translation.capture.preset/get", harness.fixture.params)
    #expect(savedPreset.object?["preset"]?.object?["customHookCodes"] == .array([.string("HS8@-4:game.exe")]))
    #expect(savedPreset.object?["preset"]?.object?["targetExePath"] == .string("C:\\Games\\日本語\\game.exe"))
    await harness.service.stop()

    // Restart with new PID, moved exe directory (matching filename), and ASLR-shifted addressHex & context2Hex
    let restartedTarget: JSONValue = .object([
        "guestPID": .integer(99),
        "exePath": .string("D:\\MovedFolder\\GAME.EXE"),
        "arch": .string("x86"),
        "creationTime": .string("FEDCBA9876543210")
    ])
    harness.listedProcesses = [restartedTarget]
    var restartParams = harness.params
    restartParams["autoAttach"] = .bool(true)
    let started = try await harness.call("translation.capture.start", restartParams)
    #expect(started.object?["captureState"] == .string("attached"))
    #expect(started.object?["target"] == restartedTarget)

    let client2 = harness.clients.last!
    let commands = await client2.commands
    #expect(commands.contains { $0.0 == "insertHook" && $0.1["hookCode"] == .string("HS8@-4:game.exe") })

    let shiftedCustomThread: JSONValue = .object([
        "guestPID": .integer(99),
        "addressHex": .string("0x000000000000ABCD"),
        "contextHex": .string("0x0000000000000010"),
        "context2Hex": .string("0x0000000000009999"),
        "hookCode": .string("HS8@-4:game.exe"),
        "hookName": .string("UserHook0")
    ])
    let shiftedSpeakerThread: JSONValue = .object([
        "guestPID": .integer(99),
        "addressHex": .string("0x000000000000EF01"),
        "contextHex": .string("0x0000000000000030"),
        "context2Hex": .string("0x0000000000000040"),
        "hookName": .string("TextOutW")
    ])
    await client2.emit(.object([
        "type": .string("thread"),
        "action": .string("created"),
        "thread": shiftedCustomThread
    ]))
    await client2.emit(.object([
        "type": .string("thread"),
        "action": .string("created"),
        "thread": shiftedSpeakerThread
    ]))
    try await harness.wait { await (harness.service.snapshot()).object?["autoBoundCount"] == .integer(2) }

    let snapAfterBind = await harness.service.snapshot()
    let threads = try #require(snapAfterBind.object?["threads"]?.array)
    #expect(threads.first {
        $0.object?["id"] == .string("0x000000000000ABCD:0x0000000000000010:0x0000000000009999")
    }?.object?["role"] == .string("body"))
    #expect(threads.first {
        $0.object?["id"] == .string("0x000000000000EF01:0x0000000000000030:0x0000000000000040")
    }?.object?["role"] == .string("speaker"))

    await client2.emit(.object([
        "type": .string("text"),
        "thread": shiftedSpeakerThread,
        "text": .string("アリス"),
        "sequence": .integer(1)
    ]))
    await client2.emit(.object([
        "type": .string("text"),
        "thread": shiftedCustomThread,
        "text": .string("こんにちは"),
        "sequence": .integer(1)
    ]))
    try await harness.wait {
        await (harness.service.snapshot()).object?["subtitle"]?.object?["status"] == .string("completed")
    }
    let translated = await harness.service.snapshot()
    #expect(translated.object?["subtitle"]?.object?["speaker"] == .string("アリス"))
    #expect(translated.object?["subtitle"]?.object?["source"] == .string("こんにちは"))
    await harness.service.stop()

    // Test manual translation.capture.autoAttach RPC when process appears after start
    harness.listedProcesses = []
    let selecting = try await harness.call("translation.capture.start", harness.params)
    #expect(selecting.object?["captureState"] == .string("selecting"))
    let notReady = try await harness.call("translation.capture.autoAttach")
    #expect(notReady.object?["attached"] == .bool(false))
    harness.listedProcesses = [restartedTarget]
    await harness.clients.last!.setProcesses([restartedTarget])
    let attached = try await harness.call("translation.capture.autoAttach")
    #expect(attached.object?["attached"] == .bool(true))
    #expect(attached.object?["snapshot"]?.object?["captureState"] == .string("attached"))
    await harness.service.stop()
}

@Test @MainActor func captureThreadsAreSortedByRolePreviewAndRecentActivity() async throws {
    let harness = try await CaptureHarness()
    defer { harness.fixture.remove() }

    _ = try await harness.call("translation.capture.start", harness.params)
    _ = try await harness.call("translation.capture.attach", ["target": captureTarget])
    let client = harness.clients.last!

    func makeThread(_ suffix: String) -> JSONValue {
        .object([
            "guestPID": .integer(42),
            "addressHex": .string("0x000000000000000\(suffix)"),
            "contextHex": .string("0x0000000000000000"),
            "context2Hex": .string("0x0000000000000000"),
            "hookName": .string("TextOutW")
        ])
    }

    let ids = (1 ... 7).map { "0x000000000000000\($0):0x0000000000000000:0x0000000000000000" }
    for index in 1 ... 7 {
        await client.emit(.object([
            "type": .string("thread"),
            "action": .string("created"),
            "thread": makeThread("\(index)")
        ]))
    }
    try await harness.wait { await (harness.service.snapshot()).object?["threadsTotal"] == .integer(7) }

    // Give t2 an older preview, t3 a newer preview, and t7 newer empty activity than t1
    await client.emit(.object([
        "type": .string("text"),
        "thread": makeThread("2"),
        "text": .string("older"),
        "sequence": .integer(1)
    ]))
    await client.emit(.object([
        "type": .string("text"),
        "thread": makeThread("3"),
        "text": .string("newer"),
        "sequence": .integer(1)
    ]))
    await client.emit(.object(["type": .string("thread"), "action": .string("created"), "thread": makeThread("7")]))

    try await harness.wait {
        let list = await (try? harness.call("translation.threads.list"))?.object?["threads"]?.array ?? []
        return list.first { $0.object?["id"] == .string(ids[2]) }?.object?["preview"] == .string("newer")
    }

    // Assign body=t5, speaker=t4, choice=t6
    _ = try await harness.call("translation.threads.select", ["selections": .array([
        .object(["threadID": .string(ids[4]), "role": .string("body")]),
        .object(["threadID": .string(ids[3]), "role": .string("speaker")]),
        .object(["threadID": .string(ids[5]), "role": .string("choice")])
    ])])

    let page = try await harness.call("translation.threads.list")
    let orderedIDs = try #require(page.object?["threads"]?.array?.compactMap { $0.object?["id"]?.string })
    #expect(orderedIDs == [
        ids[4], // body
        ids[3], // speaker
        ids[5], // choice
        ids[2], // preview + newer activity
        ids[1], // preview + older activity
        ids[6], // no preview + newer activity
        ids[0] // no preview + older activity
    ])
    await harness.service.stop()
}

@Test func capturePresetBackwardCompatibleDecoding() throws {
    let emptyLegacy = Data("{\"architecture\":\"x64\",\"restoreHistory\":false,\"autoAttach\":true}".utf8)
    let decodedEmpty = try JSONDecoder().decode(CapturePreset.self, from: emptyLegacy)
    #expect(decodedEmpty.translationEnabled == false)
    #expect(decodedEmpty.promptAttachOnLaunch == true)

    let providerLegacy = Data(
        "{\"providerID\":\"11111111-1111-1111-1111-111111111111\",\"architecture\":\"x86\"}".utf8
    )
    let decodedProvider = try JSONDecoder().decode(CapturePreset.self, from: providerLegacy)
    #expect(decodedProvider.translationEnabled == true)
    #expect(decodedProvider.promptAttachOnLaunch == true)

    let targetLegacy = Data("{\"targetExePath\":\"C:\\\\Game\\\\game.exe\"}".utf8)
    let decodedTarget = try JSONDecoder().decode(CapturePreset.self, from: targetLegacy)
    #expect(decodedTarget.translationEnabled == true)
    #expect(decodedTarget.promptAttachOnLaunch == true)

    let explicitDisabled = Data(
        "{\"providerID\":\"11111111-1111-1111-1111-111111111111\",\"translationEnabled\":false,\"promptAttachOnLaunch\":false}"
            .utf8
    )
    let decodedExplicit = try JSONDecoder().decode(CapturePreset.self, from: explicitDisabled)
    #expect(decodedExplicit.translationEnabled == false)
    #expect(decodedExplicit.promptAttachOnLaunch == false)
}

@Test @MainActor func capturePresetSavePersistsTranslationEnablementAndStateCallbackFires() async throws {
    let harness = try await CaptureHarness()
    defer { harness.fixture.remove() }

    let saved = try await harness.call("translation.capture.preset/save", [
        "container": .string(harness.fixture.bottle.path),
        "entryID": .string(harness.fixture.entryID),
        "translationEnabled": .bool(true),
        "promptAttachOnLaunch": .bool(false)
    ])
    #expect(saved.object?["preset"]?.object?["translationEnabled"] == .bool(true))
    #expect(saved.object?["preset"]?.object?["promptAttachOnLaunch"] == .bool(false))

    let loaded = await harness.service.preset(container: harness.fixture.bottle, entryID: harness.fixture.entryID)
    #expect(loaded?.translationEnabled == true)
    #expect(loaded?.promptAttachOnLaunch == false)

    var updates: [String] = []
    harness.service.onStateUpdate = { snapshot in
        if let state = snapshot.object?["captureState"]?.string {
            updates.append(state)
        }
    }
    try await harness.attach()
    try await harness.wait { updates.contains("attached") }
    let afterAttach = await harness.service.preset(container: harness.fixture.bottle, entryID: harness.fixture.entryID)
    #expect(afterAttach?.translationEnabled == true)
    await harness.service.stop()
    try await harness.wait { updates.last == "stopped" }
}

@MainActor
private final class LaunchHUDDriver: LibraryRuntimeDriver {
    var snapshot = LibraryRuntimeDriverSnapshot()
    func events() -> AsyncStream<LibraryRuntimeDriverEvent> {
        AsyncStream { $0.yield(.changed(self.snapshot)) }
    }

    func capabilities(for entry: LibraryEntry) -> LibraryRuntimeCapabilities { [.launch] }
    func retainContainers(_ urls: Set<URL>) {}
    func configure(entries: [LibraryEntry], containerURL: URL) async {}
    func launchProgram(
        _ request: LibraryLaunchRequest,
        onStarted: @escaping @MainActor () -> Void
    ) async -> LaunchResult {
        onStarted()
        return .launchedSuccessfully(programName: request.entry.name)
    }

    func programDidStart(_ request: LibraryLaunchRequest) {}
    func launchSteam(_ entry: LibraryEntry) {}
    func cancelSteamLaunch(_ entry: LibraryEntry) {}
    func stop(_ entry: LibraryEntry) async {}
    func stopTracking() {}
}

@MainActor
private final class LaunchHUDNativeStub: NativeApplicationServices {
    var methods: [String] { ["translation.capture.hud/show", "translation.capture.hud/hide"] }
    var capabilities: [String] { [] }
    var registeredCapture: ApplicationCaptureService?
    // swiftlint:disable:next large_tuple
    var presentedLaunches: [(container: URL, entryID: String, preset: CapturePreset)] = []
    var hudCalls: [(String, [String: JSONValue])] = []

    func registerCaptureContext(
        capture: ApplicationCaptureService,
        translation: ApplicationTranslationService,
        workspace: ApplicationWorkspace
    ) {
        registeredCapture = capture
    }

    func presentCaptureHUDOnLaunch(container: URL, entry: EntryRecord, preset: CapturePreset) {
        presentedLaunches.append((container, entry.id, preset))
    }

    func call(
        _ method: String,
        params: [String: JSONValue],
        workspace: ApplicationWorkspace
    ) async throws -> JSONValue {
        hudCalls.append((method, params))
        return .object(["visible": .bool(method == "translation.capture.hud/show")])
    }

    func configurationChanged(
        _ field: ConfigurationField,
        value: ConfigurationValue,
        values: [String: ConfigurationValue],
        container: URL
    ) async throws {}

    func shutdown(applyExitPolicy: Bool) async {}
}

@Test @MainActor func runtimeLaunchTriggersCaptureHUDOnlyWhenPresetEnabledAndPromptOnLaunch() async throws {
    let fixture = try MetadataFixture()
    defer { fixture.remove() }
    let native = LaunchHUDNativeStub()
    let runtime = RuntimeService(driver: LaunchHUDDriver())
    let translation = ApplicationTranslationService(
        directory: fixture.root.appendingPathComponent("Translation"),
        transport: CaptureHTTP()
    )
    let service = ApplicationService(
        workspace: fixture.workspace,
        native: native,
        runtime: runtime,
        translationService: translation
    )
    #expect(native.registeredCapture != nil)

    // No preset yet -> launch does not trigger HUD.
    _ = try await service.call("runtime.launch", params: fixture.params)
    #expect(native.presentedLaunches.isEmpty)

    // Save preset with translationEnabled = true, promptAttachOnLaunch = true -> triggers HUD.
    _ = try await service.call("translation.capture.preset/save", params: [
        "container": .string(fixture.bottle.path),
        "entryID": .string(fixture.entryID),
        "translationEnabled": .bool(true),
        "promptAttachOnLaunch": .bool(true)
    ])
    _ = try await service.call("runtime.launch", params: fixture.params)
    #expect(native.presentedLaunches.count == 1)
    #expect(native.presentedLaunches[0].entryID == fixture.entryID)

    // Turn off promptAttachOnLaunch -> launch does not trigger HUD.
    _ = try await service.call("translation.capture.preset/save", params: [
        "container": .string(fixture.bottle.path),
        "entryID": .string(fixture.entryID),
        "promptAttachOnLaunch": .bool(false)
    ])
    _ = try await service.call("runtime.launch", params: fixture.params)
    #expect(native.presentedLaunches.count == 1)

    // HUD show/hide methods route to nativeCall.
    let showRes = try await service.call("translation.capture.hud/show", params: fixture.params)
    let hideRes = try await service.call("translation.capture.hud/hide", params: [:])
    #expect(showRes.object?["visible"] == .bool(true))
    #expect(hideRes.object?["visible"] == .bool(false))
    #expect(native.hudCalls.map(\.0) == ["translation.capture.hud/show", "translation.capture.hud/hide"])
    await service.shutdown()
}

@Test @MainActor func captureDebouncesRapidTextFramesAndPersistsSentenceCache() async throws {
    let harness = try await CaptureHarness()
    defer { harness.fixture.remove() }

    _ = try await harness.profiles.call("translation.settings/save", params: [
        "settings": .encoded(TranslationSettings(translationDebounceMs: 80, sentenceCacheEnabled: true))
    ])

    try await harness.attach()
    try await harness.select()

    // Rapid partial frames before debounce expires: only final stabilized line should hit HTTP
    await harness.clients.last!.text("こん", sequence: 1)
    try await Task.sleep(for: .milliseconds(20))
    await harness.clients.last!.text("こんにち", sequence: 2)
    try await Task.sleep(for: .milliseconds(20))
    await harness.clients.last!.text("こんにちは", sequence: 3)

    let immediate = await harness.service.snapshot()
    #expect(immediate.object?["subtitle"]?.object?["source"] == .string("こんにちは"))
    #expect(immediate.object?["subtitle"]?.object?["status"] == .string("pending"))
    #expect(await harness.http.calls == 0)

    try await harness.wait {
        await (harness.service.snapshot()).object?["subtitle"]?.object?["status"] == .string("completed")
    }
    #expect(await harness.http.calls == 1)

    // Settings change should NOT wipe persistent sentence cache
    await harness.service.settingsChanged()

    // Stop and restart capture session; replaying the same text should hit persistent sentence cache
    await harness.service.stop()
    try await harness.attach()
    try await harness.select()

    await harness.clients.last!.text("こんにちは", sequence: 1)
    try await harness.wait {
        await (harness.service.snapshot()).object?["subtitle"]?.object?["status"] == .string("completed")
    }
    #expect(await harness.http.calls == 1)

    let listed = try await harness.profiles.call("translation.cache/list", params: [:])
    #expect(listed.object?["totalCount"] == .integer(1))
    #expect(listed.object?["entries"]?.array?.first?.object?["hitCount"] == .integer(2))

    // Scoped clear for a different gameID keeps this game's cache intact
    let otherClear = try await harness.call("translation.cache.clear", ["gameID": .string("other-game")])
    #expect(otherClear.object?["cleared"] == .bool(true))
    #expect(otherClear.object?["totalCount"] == .integer(1))

    // Global clear removes all entries
    let fullClear = try await harness.call("translation.cache.clear")
    #expect(fullClear.object?["cleared"] == .bool(true))
    #expect(fullClear.object?["totalCount"] == .integer(0))
    #expect(fullClear.object?["totalBytes"] == .integer(0))

    await harness.service.stop()
}

@Test func hookCodeClassificationDistinguishesCharHooksFromStringAndUnknownHooks() {
    // Explicit char-by-char hooks without F flag
    #expect(ApplicationCaptureService.isCharByCharHook("HW-4@1234:game.exe"))
    #expect(ApplicationCaptureService.isCharByCharHook("HA-4@1234:game.exe"))
    #expect(ApplicationCaptureService.isCharByCharHook("HB-4@1234"))
    #expect(ApplicationCaptureService.isCharByCharHook("HC-4@1234"))
    #expect(ApplicationCaptureService.isCharByCharHook("HI-4@1234"))
    #expect(ApplicationCaptureService.isCharByCharHook("HWX-4@1234"))
    #expect(ApplicationCaptureService.isCharByCharHook("HWN-4@1234"))
    #expect(ApplicationCaptureService.isCharByCharHook("EBW-4@1234"))
    #expect(ApplicationCaptureService.isCharByCharHook("EXHB-4@1234"))
    #expect(ApplicationCaptureService.isCharByCharHook("EDNBW#932@1234"))
    #expect(ApplicationCaptureService.isCharByCharHook("EOBW-4@1234"))
    #expect(ApplicationCaptureService.isCharByCharHook("BW-4@1234"))
    #expect(ApplicationCaptureService.isCharByCharHook("HB4F@1234"))

    // String, FULL_STRING (F), direct-read (R), empty, and unknown hooks replace cleanly
    #expect(!ApplicationCaptureService.isCharByCharHook(""))
    #expect(!ApplicationCaptureService.isCharByCharHook("R-4@1234"))
    #expect(!ApplicationCaptureService.isCharByCharHook("HS-4@1234:game.exe"))
    #expect(!ApplicationCaptureService.isCharByCharHook("HQXF-4@1234:game.exe"))
    #expect(!ApplicationCaptureService.isCharByCharHook("HM-4@1234"))
    #expect(!ApplicationCaptureService.isCharByCharHook("HV-4@1234"))
    #expect(!ApplicationCaptureService.isCharByCharHook("HU-4@1234"))
    #expect(!ApplicationCaptureService.isCharByCharHook("HWF-4@1234"))
    #expect(!ApplicationCaptureService.isCharByCharHook("HWXF-4@1234"))
    #expect(!ApplicationCaptureService.isCharByCharHook("HWNF-4@1234"))
    #expect(!ApplicationCaptureService.isCharByCharHook("EXHBF-4@1234"))
    #expect(!ApplicationCaptureService.isCharByCharHook("USER_HOOK"))
}

private func threadWithHookCode(_ hookCode: String) -> JSONValue {
    var record = captureThread.object!
    record["hookCode"] = .string(hookCode)
    return .object(record)
}

@Test @MainActor func captureFastForwardFullStringHooksReplaceInsteadOfAccumulating() async throws {
    let harness = try await CaptureHarness()
    defer { harness.fixture.remove() }

    _ = try await harness.profiles.call("translation.settings/save", params: [
        "settings": .encoded(TranslationSettings(translationDebounceMs: 80, sentenceCacheEnabled: false))
    ])

    let stringThread = threadWithHookCode("HS-4@1234:game.exe")
    try await harness.attach(thread: stringThread)
    try await harness.select()

    // Rapid FULL_STRING / USING_STRING lines during fast-forward replace cleanly and drop skipped lines
    await harness.clients.last!.text("一行目です。", sequence: 1, thread: stringThread)
    try await Task.sleep(for: .milliseconds(15))
    await harness.clients.last!.text("二行目です。", sequence: 2, thread: stringThread)
    try await Task.sleep(for: .milliseconds(15))
    await harness.clients.last!.text("三行目です。", sequence: 3, thread: stringThread)

    let immediate = await harness.service.snapshot()
    #expect(immediate.object?["subtitle"]?.object?["source"] == .string("三行目です。"))
    #expect(immediate.object?["subtitle"]?.object?["status"] == .string("pending"))
    #expect(await harness.http.calls == 0)

    try await harness.wait {
        await (harness.service.snapshot()).object?["subtitle"]?.object?["status"] == .string("completed")
    }
    #expect(await harness.http.calls == 1)

    let history = try await harness.call("translation.history")
    let entries = history.object?["entries"]?.array ?? []
    #expect(entries.count == 1)
    #expect(entries.first?.object?["source"]?.object?["text"] == .string("三行目です。"))

    // F-flagged hook codes (e.g. HQXF-4@...) also replace cleanly without accumulating
    let flaggedThread = threadWithHookCode("HQXF-4@1234:game.exe")
    await harness.clients.last!.text("スキップされる行", sequence: 4, thread: flaggedThread)
    try await Task.sleep(for: .milliseconds(15))
    await harness.clients.last!.text("最終行のみ翻訳", sequence: 5, thread: flaggedThread)

    try await harness.wait {
        let calls = await harness.http.calls
        let status = await (harness.service.snapshot()).object?["subtitle"]?.object?["status"]
        return calls == 2 && status == .string("completed")
    }
    let updatedHistory = try await harness.call("translation.history")
    let updatedEntries = updatedHistory.object?["entries"]?.array ?? []
    #expect(updatedEntries.count == 2)
    #expect(updatedEntries.last?.object?["source"]?.object?["text"] == .string("最終行のみ翻訳"))

    await harness.service.stop()
}

@Test @MainActor func captureCharByCharHookCoalescesFragmentsAndStartsFreshPerSentence() async throws {
    let harness = try await CaptureHarness()
    defer { harness.fixture.remove() }

    _ = try await harness.profiles.call("translation.settings/save", params: [
        "settings": .encoded(TranslationSettings(translationDebounceMs: 80, sentenceCacheEnabled: false))
    ])

    let charThread = threadWithHookCode("HW-4@1234:game.exe")
    try await harness.attach(thread: charThread)
    try await harness.select()

    // Non-cumulative char-by-char fragments within delayMs coalesce into one sentence
    await harness.clients.last!.text("こ", sequence: 1, thread: charThread)
    try await Task.sleep(for: .milliseconds(10))
    await harness.clients.last!.text("ん", sequence: 2, thread: charThread)
    try await Task.sleep(for: .milliseconds(10))
    await harness.clients.last!.text("に", sequence: 3, thread: charThread)
    try await Task.sleep(for: .milliseconds(10))
    await harness.clients.last!.text("ちは。", sequence: 4, thread: charThread)

    let immediate = await harness.service.snapshot()
    #expect(immediate.object?["subtitle"]?.object?["source"] == .string("こんにちは。"))
    #expect(immediate.object?["subtitle"]?.object?["status"] == .string("pending"))
    #expect(await harness.http.calls == 0)

    try await harness.wait {
        let calls = await harness.http.calls
        let status = await (harness.service.snapshot()).object?["subtitle"]?.object?["status"]
        return calls == 1 && status == .string("completed")
    }

    // After the previous debounce fires, the next sentence starts fresh
    await harness.clients.last!.text("次の台詞", sequence: 5, thread: charThread)
    let nextImmediate = await harness.service.snapshot()
    #expect(nextImmediate.object?["subtitle"]?.object?["source"] == .string("次の台詞"))

    try await harness.wait {
        let calls = await harness.http.calls
        let status = await (harness.service.snapshot()).object?["subtitle"]?.object?["status"]
        return calls == 2 && status == .string("completed")
    }

    // A pending fragment ending with newline does not coalesce with the next line
    await harness.clients.last!.text("一行目\n", sequence: 6, thread: charThread)
    try await Task.sleep(for: .milliseconds(10))
    await harness.clients.last!.text("二行目", sequence: 7, thread: charThread)

    let afterNewline = await harness.service.snapshot()
    #expect(afterNewline.object?["subtitle"]?.object?["source"] == .string("二行目"))

    try await harness.wait {
        let calls = await harness.http.calls
        let status = await (harness.service.snapshot()).object?["subtitle"]?.object?["status"]
        return calls == 3 && status == .string("completed")
    }

    // Cumulative char-by-char fragments do not duplicate prefix
    await harness.clients.last!.text("あ", sequence: 8, thread: charThread)
    try await Task.sleep(for: .milliseconds(10))
    await harness.clients.last!.text("あい", sequence: 9, thread: charThread)
    try await Task.sleep(for: .milliseconds(10))
    await harness.clients.last!.text("あいう", sequence: 10, thread: charThread)

    let cumulativeImmediate = await harness.service.snapshot()
    #expect(cumulativeImmediate.object?["subtitle"]?.object?["source"] == .string("あいう"))

    try await harness.wait {
        let calls = await harness.http.calls
        let status = await (harness.service.snapshot()).object?["subtitle"]?.object?["status"]
        return calls == 4 && status == .string("completed")
    }

    let history = try await harness.call("translation.history")
    let sources = (history.object?["entries"]?.array ?? []).compactMap {
        $0.object?["source"]?.object?["text"]?.string
    }
    #expect(sources == ["こんにちは。", "次の台詞", "二行目", "あいう"])

    await harness.service.stop()
}

@Test @MainActor func captureStopIfMatchingStopsOnlyActiveMatchingGameID() async throws {
    let harness = try await CaptureHarness()
    defer { harness.fixture.remove() }

    try await harness.attach()
    let snapshot = await harness.service.snapshot()
    let gameID = try #require(snapshot.object?["gameID"]?.string)
    #expect(snapshot.object?["captureState"] == .string("attached"))

    // Non-matching game ID leaves capture active
    let ignored = await harness.service.stopIfMatching(gameIDs: ["unrelated-game-id"])
    #expect(!ignored)
    #expect(await (harness.service.snapshot()).object?["captureState"] == .string("attached"))

    // Matching game ID stops active capture
    let stopped = await harness.service.stopIfMatching(gameIDs: [gameID])
    #expect(stopped)
    #expect(await (harness.service.snapshot()).object?["captureState"] == .string("stopped"))

    // Already stopped capture returns false
    let alreadyStopped = await harness.service.stopIfMatching(gameIDs: [gameID])
    #expect(!alreadyStopped)
}
