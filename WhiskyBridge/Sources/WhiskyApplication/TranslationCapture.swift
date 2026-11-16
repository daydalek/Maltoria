//
//  TranslationCapture.swift
//  WhiskyApplication
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
import WhiskyBridgeProtocol
import WhiskyCore
import WhiskyTranslation

@MainActor
public final class TranslationHelperHandle {
    private let stopOperation: () -> Void
    private var stopped = false
    public init(stop: @escaping () -> Void) { stopOperation = stop }
    public func stop() {
        guard !stopped else { return }
        stopped = true
        stopOperation()
    }
}

/// One current history file per game, with explicit opt-in restoration across
/// launches. Provider profiles and credentials belong to their separate stores.
actor CaptureHistoryRepository: TranslationRepository {
    let file: URL
    let gameID: String
    init(directory: URL, gameID: String) {
        self.gameID = gameID
        file = directory.appendingPathComponent("history").appendingPathComponent(gameID + ".json")
    }

    func saveConfiguration(_: ProviderConfiguration) {}
    func loadConfiguration(id _: UUID) -> ProviderConfiguration? { nil }
    func saveSettings(_: TranslationSettings) {}
    func loadSettings() -> TranslationSettings? { nil }
    func saveHistory(_ snapshot: TranslationSessionSnapshot) throws {
        guard snapshot.gameID == gameID else { throw hookFailure("history_mismatch") }
        let manager = FileManager.default
        try manager.createDirectory(
            at: file.deletingLastPathComponent(),
            withIntermediateDirectories: true,
            attributes: [.posixPermissions: 0o700]
        )
        try JSONEncoder().encode(snapshot).write(to: file, options: .atomic)
        try manager.setAttributes([.posixPermissions: 0o600], ofItemAtPath: file.path)
    }

    func loadHistory(gameID: String, sessionID _: UUID) throws -> TranslationSessionSnapshot? {
        guard gameID == self.gameID, FileManager.default.fileExists(atPath: file.path) else { return nil }
        let data = try Data(contentsOf: file)
        guard data.count <= 64 * 1_024 * 1_024 else { throw hookFailure("history_too_large") }
        let saved = try JSONDecoder().decode(TranslationSessionSnapshot.self, from: data)
        guard saved.gameID == gameID, saved.entries.count <= 1_000 else { throw hookFailure("history_mismatch") }
        return saved
    }

    func clearHistory(gameID: String, sessionID _: UUID) throws {
        guard gameID == self.gameID else { throw hookFailure("history_mismatch") }
        if FileManager.default.fileExists(atPath: file.path) { try FileManager.default.removeItem(at: file) }
    }
}

@MainActor
public final class ApplicationCaptureService {
    public static let methods = [
        "translation.capture.start", "translation.capture.processes", "translation.capture.attach",
        "translation.capture.autoAttach",
        "translation.capture.detach", "translation.capture.stop", "translation.capture.insertHook",
        "translation.capture.preset/get", "translation.capture.preset/save", "translation.capture.preset/clear",
        "translation.threads.select", "translation.threads.list", "translation.snapshot", "translation.events",
        "translation.history",
        "translation.history.text",
        "translation.history.include", "translation.history.clear", "translation.cache.clear"
    ]
    public typealias Launcher = @MainActor (URL, URL, [String: String]) throws -> TranslationHelperHandle
    public typealias ClientFactory = @MainActor (String, String) throws -> HookClientBootstrap
    public var onSubtitle: ((JSONValue) -> Void)?
    public var onStateUpdate: ((JSONValue) -> Void)?
    private let profiles: ApplicationTranslationService
    private let launcher: Launcher
    private let clientFactory: ClientFactory
    private let payloadRoot: URL
    private let presetDirectory: URL
    private let requirePayload: Bool
    private var client: (any HookClient)?
    private var helper: TranslationHelperHandle?
    private var incoming: Task<Void, Never>?
    private var heartbeat: Task<Void, Never>?
    private var updates: Task<Void, Never>?
    private var stateUpdateTask: Task<Void, Never>?
    private var debounceTask: Task<Void, Never>?
    private var debounceGeneration: UInt64 = 0
    private var pendingDebounceKey: String?
    private var pendingDebounceText: String?
    private var session: TranslationSession?
    private var captureID: String?
    private var channelID: String?
    private var architecture = "x64"
    private var container: URL?
    private var gameID: String?
    private var providerID: UUID?
    private var target: JSONValue?
    private var processes: [JSONValue] = []
    private var threads: [String: [String: JSONValue]] = [:]
    private var roles: [String: String] = [:]
    private var sequences: [String: Int64] = [:]
    private var pendingBindings: [CaptureThreadBinding] = []
    private var autoBoundCount = 0
    private var lastActivity: [String: UInt64] = [:]
    private var nextActivityCounter: UInt64 = 0
    private var speaker: String?
    private var activeSourceID: UUID?
    private var activeEntryID: UUID?
    private var selectionEpoch = 0
    private var captureState = "stopped"
    private var lastError: String?
    private var subtitle: JSONValue = .object([
        "source": .string(""),
        "translation": .string(""),
        "status": .string("idle")
    ])
    private var cursor: Int64 = 0
    private var eventBuffer: [JSONValue] = []

    public init(
        profiles: ApplicationTranslationService,
        payloadRoot: URL? = nil,
        requirePayload: Bool = true,
        presetDirectory: URL? = nil,
        clientFactory: @escaping ClientFactory = { try HookIPCClient.make(session: $0, architecture: $1) },
        launcher: @escaping Launcher
    ) {
        self.profiles = profiles
        self.launcher = launcher
        self.clientFactory = clientFactory
        self.requirePayload = requirePayload
        self.payloadRoot = payloadRoot ?? Self.defaultPayloadRoot()
        self.presetDirectory = presetDirectory ?? profiles.storageDirectory
    }

    public static func defaultPayloadRoot() -> URL {
        if let override = ProcessInfo.processInfo.environment["MALTORIA_LUNAHOOK_ROOT"], override.hasPrefix("/") {
            return URL(fileURLWithPath: override)
        }
        let executable = URL(fileURLWithPath: CommandLine.arguments[0]).standardizedFileURL
        return executable.deletingLastPathComponent().appendingPathComponent("LunaHook")
    }

    // swiftlint:disable:next cyclomatic_complexity
    public func call(
        _ method: String,
        params: [String: JSONValue],
        workspace: ApplicationWorkspace
    ) async throws -> JSONValue {
        switch method {
        case "translation.capture.start": return try await start(params, workspace: workspace)
        case "translation.capture.processes": return try await listProcesses()
        case "translation.capture.attach": return try await attach(params)
        case "translation.capture.autoAttach": return try await autoAttach()
        case "translation.capture.detach", "translation.capture.stop":
            await stop()
            return await snapshot()
        case "translation.capture.insertHook":
            guard target != nil, let client else { throw hookFailure("not_attached") }
            let code = try params.text("hookCode")
            guard !code.isEmpty, code.utf8.count <= 2_048 else { throw hookFailure("invalid_hook_code") }
            let result = try await client.request("insertHook", params: ["hookCode": .string(code)])
            if let currentGameID = gameID {
                _ = try? await updatePreset(for: currentGameID) { current in
                    if !current.customHookCodes.contains(code) {
                        current.customHookCodes.append(code)
                        if current.customHookCodes.count > 16 {
                            current.customHookCodes.removeFirst(current.customHookCodes.count - 16)
                        }
                    }
                }
            }
            return result
        case "translation.capture.preset/get": return try await getPreset(params, workspace: workspace)
        case "translation.capture.preset/save": return try await savePreset(params, workspace: workspace)
        case "translation.capture.preset/clear": return try await clearPreset(params, workspace: workspace)
        case "translation.threads.select": return try await selectThreads(params)
        case "translation.threads.list": return threadPage(params)
        case "translation.snapshot": return await snapshot()
        case "translation.events":
            await refreshSubtitle()
            let after = max(0, params["afterCursor"]?.integer ?? 0)
            let maximum = min(32, max(1, Int(params["maxCount"]?.integer ?? 16)))
            let available = eventBuffer.filter { ($0.object?["cursor"]?.integer ?? 0) > after }
            var page: [JSONValue] = []
            var bytes = 0
            for event in available.prefix(maximum) {
                let size = try JSONEncoder().encode(event).count
                if bytes + size > 512 * 1_024 { break }
                bytes += size
                page.append(event)
            }
            return .object([
                "events": .array(page), "cursor": page.last?.object?["cursor"] ?? .integer(cursor),
                "resetRequired": .bool(after > cursor || after < (eventBuffer.first?.object?["cursor"]?.integer ?? 1) -
                    1)
            ])
        case "translation.history": return try await history(params)
        case "translation.history.text":
            guard let session, let id = try UUID(uuidString: params.text("entryID")),
                  let entry = await session.history().first(where: { $0.id == id })
            else { throw hookFailure("unknown_entry") }
            let field = try params.text("field")
            guard ["source", "translation"].contains(field) else { throw hookFailure("invalid_params") }
            let text = field == "source" ? entry.source.text : entry.translatedText ?? ""
            let offset = min(text.count, max(0, Int(params["offset"]?.integer ?? 0)))
            let limit = min(16_384, max(1, Int(params["maxCharacters"]?.integer ?? 16_384)))
            let chunk = String(text.dropFirst(offset).prefix(limit))
            return .object([
                "text": .string(chunk),
                "nextOffset": .integer(Int64(offset + chunk.count)),
                "total": .integer(Int64(text.count))
            ])
        case "translation.history.include":
            guard let session, let id = try UUID(uuidString: params.text("entryID")),
                  let included = params["include"]?.bool
            else { throw hookFailure("invalid_params") }
            try await session.setIncludedInContext(included, entryID: id)
            return .object(["updated": .bool(true)])
        case "translation.history.clear":
            cancelDebounce()
            selectionEpoch += 1
            activeSourceID = nil
            activeEntryID = nil
            subtitle = .object(["source": .string(""), "translation": .string(""), "status": .string("idle")])
            emit("subtitle", subtitle)
            onSubtitle?(subtitle)
            try await session?.clearHistory()
            return .object(["cleared": .bool(true)])
        case "translation.cache.clear":
            let requestedGameID: String?
            switch params["gameID"] {
            case nil, .null:
                requestedGameID = nil
            case let .string(value):
                requestedGameID = value.isEmpty ? nil : value
            default:
                throw BridgeFailure("invalid_params", "Expected string: gameID")
            }
            if let requestedGameID {
                if gameID == requestedGameID {
                    await session?.clearCache()
                }
            } else {
                await session?.clearCache()
            }
            let remaining = try await profiles.clearSentenceCache(gameID: requestedGameID)
            return .object([
                "cleared": .bool(true),
                "totalCount": .integer(Int64(remaining.totalCount)),
                "totalBytes": .integer(Int64(remaining.totalBytes))
            ])
        default: throw BridgeFailure("method_unavailable", "Translation capture method is unavailable")
        }
    }

    private func start(_ params: [String: JSONValue], workspace: ApplicationWorkspace) async throws -> JSONValue {
        let entry = try await workspace.entry(params)
        let prefix = try workspace.resolve(params)
        guard let selectedProvider = try UUID(uuidString: params.text("providerID"))
        else { throw hookFailure("invalid_provider") }
        let resolvedGameID = LibraryStorageLayout.key(prefix.standardizedFileURL.path + "\n" + entry.id)
        let existingPreset = try? await CapturePresetRepository(directory: presetDirectory, gameID: resolvedGameID)
            .load()
        let arch = params["architecture"]?.string ?? existingPreset?.architecture ?? "x64"
        guard ["x86", "x64"].contains(arch) else { throw hookFailure("invalid_architecture") }
        let restoreHistory = params["restoreHistory"]?.bool ?? existingPreset?.restoreHistory ?? false
        let shouldAutoAttach = params["autoAttach"]?.bool ?? false
        await stop()
        session = nil
        let id = UUID().uuidString
        captureID = id
        container = prefix
        gameID = resolvedGameID
        providerID = selectedProvider
        captureState = "connecting"
        lastError = nil
        subtitle = .object(["source": .string(""), "translation": .string(""), "status": .string("idle")])
        activeSourceID = nil
        activeEntryID = nil
        onSubtitle?(subtitle)
        do {
            _ = try await profiles.captureConfiguration(providerID: selectedProvider)
            guard captureID == id else { throw CancellationError() }
            try await openChannel(architecture: arch)
            guard captureID == id else { throw CancellationError() }
            _ = try await listProcesses()
            guard captureID == id else { throw CancellationError() }
            // A missing payload or failed helper launch must not clear the
            // previous game's persisted history.
            session = try await profiles.makeCaptureSession(
                gameID: resolvedGameID,
                providerID: selectedProvider,
                restoreHistory: restoreHistory
            )
            guard captureID == id else { throw CancellationError() }
            captureState = "selecting"
            updates = Task { [weak self] in
                while !Task.isCancelled {
                    await self?.refreshSubtitle()
                    do { try await Task.sleep(for: .milliseconds(150)) } catch { break }
                }
            }
            emit("state", .string(captureState))
            let preset = try await updatePreset(for: resolvedGameID) { current in
                current.providerID = selectedProvider
                current.architecture = arch
                current.restoreHistory = restoreHistory
                current.translationEnabled = true
            }
            scheduleStateUpdate(debounced: false)
            if shouldAutoAttach, let savedPath = preset.targetExePath, !savedPath.isEmpty {
                _ = try await matchAndAttach(using: preset)
            }
            return await snapshot()
        } catch {
            if captureID == id { await stop()
                setError(error)
            }
            throw error
        }
    }

    private func openChannel(architecture arch: String) async throws {
        guard captureID != nil, let container else { throw hookFailure("not_started") }
        await closeChannel()
        let wireID = UUID().uuidString
        channelID = wireID
        architecture = arch
        let bits = arch == "x86" ? "32" : "64"
        let root = payloadRoot.appendingPathComponent(arch)
        let executable = root.appendingPathComponent("MaltoriaLunaHost\(bits).exe")
        if requirePayload {
            for filename in [
                "MaltoriaLunaHost\(bits).exe",
                "LunaHost\(bits).dll",
                "LunaHook\(bits).dll",
                "LunaTmpFontLoader.dll"
            ] {
                guard FileManager.default.fileExists(atPath: root.appendingPathComponent(filename).path) else {
                    throw hookFailure("payload_unavailable")
                }
            }
        }
        let bootstrap = try clientFactory(wireID, arch)
        client = bootstrap.client
        do {
            helper = try launcher(executable, container, bootstrap.environment)
            try await bootstrap.client.ready()
            guard channelID == wireID else { throw CancellationError() }
            incoming = Task { [weak self, client = bootstrap.client] in
                do {
                    for try await event in await client.events() {
                        await self?.consume(event, channel: wireID)
                    }
                    await self?.disconnected(channel: wireID, error: hookFailure("disconnected"))
                } catch { await self?.disconnected(channel: wireID, error: error) }
            }
            heartbeat = Task { [weak self, client = bootstrap.client] in
                do {
                    while !Task.isCancelled {
                        try await Task.sleep(for: .seconds(10))
                        _ = try await client.request("ping", params: [:])
                    }
                } catch { await self?.disconnected(channel: wireID, error: error) }
            }
        } catch {
            if channelID == wireID { await closeChannel() }
            throw error
        }
    }

    private func listProcesses() async throws -> JSONValue {
        guard let client, let wireID = channelID else { throw hookFailure("not_started") }
        let result = try await client.request("listProcesses", params: [:])
        guard channelID == wireID else { throw CancellationError() }
        guard let list = result.object?["processes"]?.array,
              list.count <= 512
        else { throw hookFailure("invalid_process_list") }
        processes = list.filter { Self.validTarget($0) }
        scheduleStateUpdate(debounced: false)
        return .object(["processes": .array(processes), "truncated": result.object?["truncated"] ?? .bool(false)])
    }

    private func attach(_ params: [String: JSONValue]) async throws -> JSONValue {
        guard let selected = params["target"], Self.validTarget(selected), processes.contains(selected) else {
            throw hookFailure("invalid_target")
        }
        try await attachTarget(selected)
        return await snapshot()
    }

    private func autoAttach() async throws -> JSONValue {
        guard captureState == "selecting", let currentGameID = gameID, target == nil else {
            return await .object(["attached": .bool(false), "snapshot": snapshot()])
        }
        guard let preset = try await CapturePresetRepository(directory: presetDirectory, gameID: currentGameID).load(),
              let savedPath = preset.targetExePath, !savedPath.isEmpty
        else {
            return await .object(["attached": .bool(false), "snapshot": snapshot()])
        }
        _ = try await listProcesses()
        let attached = try await matchAndAttach(using: preset)
        return await .object(["attached": .bool(attached), "snapshot": snapshot()])
    }

    private func matchAndAttach(using preset: CapturePreset) async throws -> Bool {
        guard captureState == "selecting", target == nil,
              let savedPath = preset.targetExePath, !savedPath.isEmpty,
              let matched = CapturePresetMatcher.matchProcess(
                  targetExePath: savedPath,
                  preferredArchitecture: preset.architecture,
                  in: processes
              )
        else { return false }
        try await attachTarget(matched)
        return true
    }

    private func attachTarget(_ selected: JSONValue) async throws {
        guard target == nil, let id = captureID, let currentGameID = gameID else {
            throw hookFailure("already_attached")
        }
        let arch = selected.object!["arch"]!.string!
        let exePath = selected.object!["exePath"]!.string!
        do {
            if arch != architecture { try await openChannel(architecture: arch) }
            guard let client, captureID == id else { throw CancellationError() }
            let existingPreset = try? await CapturePresetRepository(
                directory: presetDirectory,
                gameID: currentGameID
            ).load()
            autoBoundCount = 0
            pendingBindings = existingPreset?.threadBindings ?? []
            target = selected
            captureState = "attaching"
            _ = try await client.request("attach", params: ["target": selected])
            guard captureID == id else { throw CancellationError() }
            let preset = try await updatePreset(for: currentGameID) { current in
                current.targetExePath = exePath
                current.architecture = arch
                current.translationEnabled = true
            }
            for code in preset.customHookCodes {
                guard captureID == id else { break }
                _ = try? await client.request("insertHook", params: ["hookCode": .string(code)])
            }
            guard captureID == id else { throw CancellationError() }
            captureState = "attached"
            emit("state", .string(captureState))
            scheduleStateUpdate(debounced: false)
        } catch {
            if captureID == id {
                await stop()
                setError(error)
            }
            throw error
        }
    }

    nonisolated static func validTarget(_ value: JSONValue) -> Bool {
        guard let record = value.object, let pid = record["guestPID"]?.integer, pid > 0, pid <= Int64(UInt32.max),
              let path = record["exePath"]?.string, !path.isEmpty, path.utf8.count <= 8_192,
              let arch = record["arch"]?.string, ["x86", "x64"].contains(arch),
              let creation = record["creationTime"]?.string, creation.count == 16,
              creation.allSatisfy(\.isHexDigit)
        else { return false }
        return path.hasPrefix("\\\\") || (path.count > 3 && path.dropFirst().hasPrefix(":\\"))
    }

    private func selectThreads(_ params: [String: JSONValue]) async throws -> JSONValue {
        guard let selections = params["selections"]?.array,
              selections.count <= 8
        else { throw hookFailure("invalid_threads") }
        var next: [String: String] = [:]
        var bindings: [CaptureThreadBinding] = []
        for selection in selections {
            guard let object = selection.object, let id = object["threadID"]?.string,
                  let thread = threads[id],
                  let role = object["role"]?.string, ["body", "speaker", "choice"].contains(role), next[id] == nil
            else { throw hookFailure("invalid_threads") }
            next[id] = role
            bindings.append(CaptureThreadBinding(
                role: role,
                hookCode: thread["hookCode"]?.string ?? "",
                hookName: thread["hookName"]?.string ?? "",
                contextHex: thread["contextHex"]?.string ?? "",
                context2Hex: thread["context2Hex"]?.string ?? ""
            ))
        }
        guard next.values.filter({ $0 == "body" }).count <= 1,
              next.values.filter({ $0 == "speaker" }).count <= 1
        else { throw hookFailure("invalid_threads") }
        roles = next
        pendingBindings = []
        cancelDebounce()
        selectionEpoch += 1
        activeSourceID = nil
        activeEntryID = nil
        speaker = nil
        markSubtitleCancelled()
        if let currentGameID = gameID {
            _ = try? await updatePreset(for: currentGameID) { current in
                current.threadBindings = bindings
            }
        }
        emit("threadsChanged", .null)
        scheduleStateUpdate(debounced: false)
        try await session?.cancelActive()
        return .object(["selected": .integer(Int64(roles.count))])
    }

    // swiftlint:disable:next function_body_length
    private func consume(_ value: JSONValue, channel: String) async {
        guard channel == channelID, let frame = value.object, let target,
              let pid = target.object?["guestPID"]
        else { return }
        switch frame["type"]?.string {
        case "process":
            if frame["guestPID"] == pid, frame["action"] == .string("disconnected") {
                await stop()
                captureState = "targetExited"
                emit("state", .string(captureState))
                scheduleStateUpdate(debounced: false)
            }
        case "error":
            lastError = String((frame["code"]?.string ?? "capture_error").prefix(80))
            emit("error", .string(lastError!))
            scheduleStateUpdate(debounced: false)
        case "thread", "text":
            guard let thread = frame["thread"]?.object, thread["guestPID"] == pid,
                  let address = thread["addressHex"]?.string, let context = thread["contextHex"]?.string,
                  let context2 = thread["context2Hex"]?.string,
                  [address, context, context2].allSatisfy({ $0.hasPrefix("0x") && $0.count <= 18 })
            else { return }
            let key = [address, context, context2].joined(separator: ":")
            if frame["action"] == .string("removed") {
                threads[key] = nil
                roles[key] = nil
                sequences[key] = nil
                lastActivity[key] = nil
                if pendingDebounceKey == key {
                    cancelDebounce()
                }
                emit("threadsChanged", .null)
                scheduleStateUpdate(debounced: true)
                return
            }
            guard threads[key] != nil || threads.count < 256 else { lastError = "thread_limit"
                return
            }
            var record = thread
            record["id"] = .string(key)
            record["preview"] = threads[key]?["preview"] ?? .string("")
            if record["hookCode"] == nil, let existingHookCode = threads[key]?["hookCode"] {
                record["hookCode"] = existingHookCode
            }
            threads[key] = record
            if frame["type"] == .string("thread") {
                nextActivityCounter += 1
                lastActivity[key] = nextActivityCounter
                if tryAutoBindThread(key: key, thread: record) {
                    emit("threadsChanged", .null)
                    scheduleStateUpdate(debounced: true)
                }
                return
            }
            guard let text = frame["text"]?.string,
                  !text.isEmpty, text.utf8.count <= 64 * 1_024,
                  let sequence = frame["sequence"]?.integer, sequence > (sequences[key] ?? 0)
            else { return }
            sequences[key] = sequence
            nextActivityCounter += 1
            lastActivity[key] = nextActivityCounter
            threads[key]?["preview"] = .string(String(text.prefix(256)))
            _ = tryAutoBindThread(key: key, thread: threads[key]!)
            emit("threadsChanged", .null)
            scheduleStateUpdate(debounced: true)
            guard let role = roles[key] else { return }
            if role == "speaker" { speaker = String(text.prefix(256))
                return
            }
            guard let session, let providerID else { return }
            let hookCode = record["hookCode"]?.string ?? ""
            let nextText = coalescedText(for: text, key: key, hookCode: hookCode)
            cancelDebounce()
            let generation = debounceGeneration
            let sourceID = UUID()
            let selectedAt = selectionEpoch
            let currentSpeaker = speaker
            activeSourceID = sourceID
            activeEntryID = nil
            subtitle = .object([
                "source": .string(nextText), "translation": .string(""),
                "speaker": currentSpeaker.map(JSONValue.string) ?? .null,
                "kind": .string(role), "status": .string("pending"), "error": .null
            ])
            emit("subtitle", subtitle)
            onSubtitle?(subtitle)
            let delayMs = await profiles.currentDebounceMs()
            guard channel == channelID,
                  selectedAt == selectionEpoch,
                  activeSourceID == sourceID,
                  debounceGeneration == generation
            else { return }
            if delayMs <= 0 {
                await submitCapturedText(
                    text: nextText,
                    speaker: currentSpeaker,
                    key: key,
                    sourceID: sourceID,
                    providerID: providerID,
                    session: session,
                    channel: channel,
                    selectedAt: selectedAt,
                    generation: generation
                )
            } else {
                pendingDebounceKey = key
                pendingDebounceText = nextText
                debounceTask = Task { @MainActor [weak self] in
                    do {
                        try await Task.sleep(for: .milliseconds(delayMs))
                    } catch {
                        return
                    }
                    guard let self,
                          !Task.isCancelled,
                          self.debounceGeneration == generation,
                          self.channelID == channel,
                          self.selectionEpoch == selectedAt,
                          self.activeSourceID == sourceID
                    else { return }
                    self.debounceTask = nil
                    self.pendingDebounceKey = nil
                    self.pendingDebounceText = nil
                    await self.submitCapturedText(
                        text: nextText,
                        speaker: currentSpeaker,
                        key: key,
                        sourceID: sourceID,
                        providerID: providerID,
                        session: session,
                        channel: channel,
                        selectedAt: selectedAt,
                        generation: generation
                    )
                }
            }
        default: break
        }
    }

    private static let maxCoalescedDebounceChars = 2_000

    private func coalescedText(for text: String, key: String, hookCode: String) -> String {
        guard Self.isCharByCharHook(hookCode),
              debounceTask != nil,
              pendingDebounceKey == key,
              let pending = pendingDebounceText,
              !pending.hasSuffix("\n"),
              !pending.hasSuffix("\r")
        else {
            return text
        }
        if text.hasPrefix(pending) {
            return text
        }
        guard pending.count + text.count <= Self.maxCoalescedDebounceChars else {
            return text
        }
        return pending + text
    }

    /// Returns `true` only when `hookCode` matches a LunaHook character-by-character hook
    /// without the `FULL_STRING` (`F`) flag:
    /// `[E[X|[D][N|O]]]{H|B}{W|C|I|A|B}[X][F][N]...[@address]`.
    /// Direct-read (`R...`), string (`M|Q|V|U|S`), `F`-flagged, empty, and unknown hook codes
    /// return `false` so every incoming text event replaces pending debounce state cleanly.
    nonisolated static func isCharByCharHook(_ hookCode: String) -> Bool {
        let header = hookCode.split(separator: "@", maxSplits: 1, omittingEmptySubsequences: false).first ?? ""
        guard !header.isEmpty, header.first != "R" else { return false }

        var index = header.startIndex
        if header[index] == "E" {
            header.formIndex(after: &index)
            if index < header.endIndex, header[index] == "X" {
                header.formIndex(after: &index)
            } else {
                if index < header.endIndex, header[index] == "D" {
                    header.formIndex(after: &index)
                }
                if index < header.endIndex, header[index] == "N" || header[index] == "O" {
                    header.formIndex(after: &index)
                }
            }
        }

        guard index < header.endIndex, header[index] == "H" || header[index] == "B" else {
            return false
        }
        header.formIndex(after: &index)

        guard index < header.endIndex, ["W", "C", "I", "A", "B"].contains(header[index]) else {
            return false
        }
        header.formIndex(after: &index)

        while index < header.endIndex, ["X", "F", "N"].contains(header[index]) {
            if header[index] == "F" { return false }
            header.formIndex(after: &index)
        }
        return true
    }

    private func cancelDebounce() {
        debounceGeneration &+= 1
        debounceTask?.cancel()
        debounceTask = nil
        pendingDebounceKey = nil
        pendingDebounceText = nil
    }

    // swiftlint:disable:next function_parameter_count
    private func submitCapturedText(
        text: String,
        speaker: String?,
        key: String,
        sourceID: UUID,
        providerID: UUID,
        session: TranslationSession,
        channel: String,
        selectedAt: Int,
        generation: UInt64
    ) async {
        do {
            try await profiles.allowTranslation(providerID: providerID)
            guard channel == channelID,
                  selectedAt == selectionEpoch,
                  activeSourceID == sourceID,
                  debounceGeneration == generation
            else { return }
            let entryID = try await session.submit(FinalizedTranslationSource(
                eventID: sourceID,
                revision: 0,
                streamID: key,
                generation: 1,
                text: text,
                speaker: speaker
            ))
            guard channel == channelID,
                  selectedAt == selectionEpoch,
                  activeSourceID == sourceID,
                  debounceGeneration == generation
            else { return }
            activeEntryID = entryID
            await refreshSubtitle()
        } catch is CancellationError {
            return
        } catch {
            guard channel == channelID,
                  selectedAt == selectionEpoch,
                  activeSourceID == sourceID,
                  debounceGeneration == generation
            else { return }
            setError(error)
            if var display = subtitle.object {
                display["status"] = .string("failed")
                display["error"] = .string(lastError!)
                subtitle = .object(display)
                emit("subtitle", subtitle)
                onSubtitle?(subtitle)
            }
        }
    }

    private func refreshSubtitle() async {
        guard let session, let id = captureID, let active = await session.activeEntry(), captureID == id,
              active.id == activeEntryID
        else { return }
        var display = subtitle.object ?? [:]
        display["translation"] = .string(String((active.translatedText ?? "").prefix(16_384)))
        display["translationTruncated"] = .bool((active.translatedText?.count ?? 0) > 16_384)
        display["status"] = .string(active.status.rawValue)
        display["error"] = active.failureDescription.map(JSONValue.string) ?? .null
        let next = JSONValue.object(display)
        if next != subtitle { subtitle = next
            emit("subtitle", next)
            onSubtitle?(next)
        }
    }

    public func preset(container: URL, entryID: String) async -> CapturePreset? {
        let id = LibraryStorageLayout.key(container.standardizedFileURL.path + "\n" + entryID)
        return try? await CapturePresetRepository(directory: presetDirectory, gameID: id).load()
    }

    public func currentSnapshot() async -> JSONValue {
        await snapshot()
    }

    public func snapshot() async -> JSONValue {
        await refreshSubtitle()
        let count = await session?.history().count ?? 0
        let presetValue: JSONValue = if let gameID,
                                        let saved = try? await CapturePresetRepository(
                                            directory: presetDirectory,
                                            gameID: gameID
                                        ).load(),
                                        let encoded = try? JSONValue.encoded(saved) {
            encoded
        } else {
            .null
        }
        return .object([
            "sessionID": captureID.map(JSONValue.string) ?? .null, "gameID": gameID.map(JSONValue.string) ?? .null,
            "captureState": .string(captureState), "lastError": lastError.map(JSONValue.string) ?? .null,
            "target": target ?? .null, "subtitle": subtitle, "cursor": .integer(cursor),
            "processes": .array(processes), "historyCount": .integer(Int64(count)),
            "threads": threadPage([:]).object?["threads"] ?? .array([]),
            "threadsTotal": .integer(Int64(threads.count)),
            "autoBoundCount": .integer(Int64(autoBoundCount)),
            "preset": presetValue
        ])
    }

    private func history(_ params: [String: JSONValue]) async throws -> JSONValue {
        let all = await session?.history() ?? []
        let offset = min(all.count, max(0, Int(params["offset"]?.integer ?? 0)))
        let maximum = min(20, max(1, Int(params["maxCount"]?.integer ?? 20)))
        var page: [JSONValue] = []
        var bytes = 0
        for entry in all.dropFirst(offset).prefix(maximum) {
            var record = try JSONValue.encoded(entry).object!
            record["translatedText"] = entry.translatedText.map { .string(String($0.prefix(16_384))) } ?? .null
            record["translationTruncated"] = .bool((entry.translatedText?.count ?? 0) > 16_384)
            let value = JSONValue.object(record)
            let size = try JSONEncoder().encode(value).count
            if bytes + size > 512 * 1_024 { break }
            bytes += size
            page.append(value)
        }
        return .object([
            "entries": .array(page),
            "total": .integer(Int64(all.count)),
            "nextOffset": .integer(Int64(offset + page.count))
        ])
    }

    private func threadPage(_ params: [String: JSONValue]) -> JSONValue {
        let offset = min(threads.count, max(0, Int(params["offset"]?.integer ?? 0)))
        let count = min(64, max(1, Int(params["maxCount"]?.integer ?? 64)))
        let keys = threads.keys.sorted { lhs, rhs in
            let lhsRole = Self.roleSortPriority(roles[lhs])
            let rhsRole = Self.roleSortPriority(roles[rhs])
            if lhsRole != rhsRole { return lhsRole < rhsRole }
            let lhsHasPreview = !(threads[lhs]?["preview"]?.string ?? "").isEmpty
            let rhsHasPreview = !(threads[rhs]?["preview"]?.string ?? "").isEmpty
            if lhsHasPreview != rhsHasPreview { return lhsHasPreview && !rhsHasPreview }
            let lhsActivity = lastActivity[lhs] ?? 0
            let rhsActivity = lastActivity[rhs] ?? 0
            if lhsActivity != rhsActivity { return lhsActivity > rhsActivity }
            return lhs < rhs
        }
        let records: [JSONValue] = keys.dropFirst(offset).prefix(count).compactMap { key in
            guard var thread = threads[key] else { return nil }
            thread["role"] = roles[key].map(JSONValue.string) ?? .null
            if let name = thread["hookName"]?.string { thread["hookName"] = .string(String(name.prefix(128))) }
            return .object(thread)
        }
        return .object([
            "threads": .array(records),
            "total": .integer(Int64(threads.count)),
            "nextOffset": .integer(Int64(offset + records.count))
        ])
    }

    private static func roleSortPriority(_ role: String?) -> Int {
        switch role {
        case "body": 0
        case "speaker": 1
        case "choice": 2
        default: 3
        }
    }

    private func emit(_ type: String, _ value: JSONValue) {
        cursor += 1
        eventBuffer.append(.object(["cursor": .integer(cursor), "type": .string(type), "value": value]))
        if eventBuffer.count > 64 { eventBuffer.removeFirst(eventBuffer.count - 64) }
    }

    private func scheduleStateUpdate(debounced: Bool) {
        guard onStateUpdate != nil else { return }
        stateUpdateTask?.cancel()
        stateUpdateTask = Task { @MainActor [weak self] in
            if debounced {
                try? await Task.sleep(for: .milliseconds(60))
                guard !Task.isCancelled else { return }
            }
            guard let self, let onStateUpdate = self.onStateUpdate else { return }
            let value = await self.currentSnapshot()
            guard !Task.isCancelled else { return }
            onStateUpdate(value)
        }
    }

    private func setError(_ error: any Error) {
        lastError = (error as? BridgeFailure)?.code ?? "translation_failed"
        if let error = error as? TranslationError { lastError = error.localizedDescription }
        emit("error", .string(lastError!))
        scheduleStateUpdate(debounced: false)
    }

    private func disconnected(channel: String, error: any Error) async {
        guard channelID == channel else { return }
        await stop()
        captureState = "error"
        setError(error)
    }

    private func closeChannel() async {
        channelID = nil
        incoming?.cancel()
        incoming = nil
        heartbeat?.cancel()
        heartbeat = nil
        let oldClient = client
        client = nil
        let oldHelper = helper
        helper = nil
        await oldClient?.close() // EOF asks the helper to detach only its own target.
        if let oldHelper {
            Task { try? await Task.sleep(for: .seconds(2))
                oldHelper.stop()
            }
        }
    }

    public func stop() async {
        let oldSession = session
        cancelDebounce()
        captureID = nil
        selectionEpoch += 1
        activeSourceID = nil
        activeEntryID = nil
        updates?.cancel()
        updates = nil
        target = nil
        threads = [:]
        roles = [:]
        sequences = [:]
        pendingBindings = []
        autoBoundCount = 0
        lastActivity = [:]
        nextActivityCounter = 0
        processes = []
        speaker = nil
        captureState = "stopped"
        markSubtitleCancelled()
        emit("state", .string(captureState))
        scheduleStateUpdate(debounced: false)
        await closeChannel()
        try? await oldSession?.cancelActive()
    }

    private func markSubtitleCancelled() {
        if var value = subtitle.object, value["status"] == .string("pending") {
            value["status"] = .string("cancelled")
            subtitle = .object(value)
            emit("subtitle", subtitle)
            onSubtitle?(subtitle)
        }
    }

    public func settingsChanged() async {
        guard let session, let providerID else { return }
        cancelDebounce()
        do {
            try await session.cancelActive()
            try await profiles.allowTranslation(providerID: providerID)
            let configuration = try await profiles.captureConfiguration(providerID: providerID)
            try await session.updateConfiguration(configuration)
            try await session.updateSettings(profiles.captureSettings())
            await session.clearCache()
        } catch { await stop()
            setError(error)
        }
    }

    public func prepareContainerRemoval(_ url: URL) async {
        if container?.standardizedFileURL == url.standardizedFileURL { await stop() }
    }

    @discardableResult
    public func stopIfMatching(gameIDs: Set<String>) async -> Bool {
        guard let gameID, gameIDs.contains(gameID), captureState != "stopped" else {
            return false
        }
        await stop()
        return true
    }

    private func tryAutoBindThread(key: String, thread: [String: JSONValue]) -> Bool {
        guard roles[key] == nil, !pendingBindings.isEmpty else { return false }
        guard let index = CapturePresetMatcher.matchPendingBinding(
            for: thread,
            in: pendingBindings,
            activeRoles: roles
        )
        else { return false }
        let binding = pendingBindings.remove(at: index)
        roles[key] = binding.role
        autoBoundCount += 1
        return true
    }

    private func resolveGameID(
        _ params: [String: JSONValue],
        workspace: ApplicationWorkspace
    ) async throws -> String {
        let entry = try await workspace.entry(params)
        let prefix = try workspace.resolve(params)
        return LibraryStorageLayout.key(prefix.standardizedFileURL.path + "\n" + entry.id)
    }

    private func getPreset(
        _ params: [String: JSONValue],
        workspace: ApplicationWorkspace
    ) async throws -> JSONValue {
        let id = try await resolveGameID(params, workspace: workspace)
        let preset = try await CapturePresetRepository(directory: presetDirectory, gameID: id).load()
        return try .object(["preset": preset.map(JSONValue.encoded) ?? .null])
    }

    private func savePreset(
        _ params: [String: JSONValue],
        workspace: ApplicationWorkspace
    ) async throws -> JSONValue {
        let id = try await resolveGameID(params, workspace: workspace)
        let repo = CapturePresetRepository(directory: presetDirectory, gameID: id)
        var preset = try await repo.load() ?? CapturePreset(autoAttach: false)
        if let value = params["autoAttach"] {
            guard let flag = value.bool else { throw hookFailure("invalid_params") }
            preset.autoAttach = flag
        }
        if let value = params["restoreHistory"] {
            guard let flag = value.bool else { throw hookFailure("invalid_params") }
            preset.restoreHistory = flag
        }
        if let value = params["architecture"] {
            guard let arch = value.string, ["x86", "x64"].contains(arch) else {
                throw hookFailure("invalid_architecture")
            }
            preset.architecture = arch
        }
        if let value = params["providerID"] {
            if value == .null {
                preset.providerID = nil
            } else {
                guard let raw = value.string, let uuid = UUID(uuidString: raw) else {
                    throw hookFailure("invalid_provider")
                }
                preset.providerID = uuid
            }
        }
        if let value = params["translationEnabled"] {
            guard let flag = value.bool else { throw hookFailure("invalid_params") }
            preset.translationEnabled = flag
        }
        if let value = params["promptAttachOnLaunch"] {
            guard let flag = value.bool else { throw hookFailure("invalid_params") }
            preset.promptAttachOnLaunch = flag
        }
        try await repo.save(preset)
        return try .object(["preset": .encoded(preset)])
    }

    private func clearPreset(
        _ params: [String: JSONValue],
        workspace: ApplicationWorkspace
    ) async throws -> JSONValue {
        let id = try await resolveGameID(params, workspace: workspace)
        try await CapturePresetRepository(directory: presetDirectory, gameID: id).clear()
        if gameID == id {
            pendingBindings = []
        }
        return .object(["cleared": .bool(true)])
    }

    @discardableResult
    private func updatePreset(
        for gameID: String,
        mutate: (inout CapturePreset) -> Void
    ) async throws -> CapturePreset {
        let repo = CapturePresetRepository(directory: presetDirectory, gameID: gameID)
        var preset = await (try? repo.load()) ?? CapturePreset(autoAttach: true)
        mutate(&preset)
        try await repo.save(preset)
        return preset
    }
}
