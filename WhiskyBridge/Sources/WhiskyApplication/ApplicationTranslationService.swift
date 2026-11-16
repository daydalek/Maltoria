//
//  ApplicationTranslationService.swift
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

import Foundation
import WhiskyBridgeProtocol
import WhiskyTranslation

@MainActor
public final class ApplicationTranslationService {
    public static let methods = [
        "translation.providers/list", "translation.providers/save", "translation.providers/delete",
        "translation.credential.set", "translation.credential.delete",
        "translation.models", "translation.modelInfo", "translation.testConnection",
        "translation.settings/get", "translation.settings/save",
        "translation.cache/list", "translation.cache/delete"
    ]
    public static let capabilities = ["translation.providers", "translation.models", "translation.settings"]

    private let catalog: TranslationApplicationCatalog
    private let credentials: any CredentialStore
    private let switchableCredentials: SwitchableCredentialStore?
    private let translator: TranslationService
    private let sentenceCache: PersistentSentenceCache
    private let directory: URL
    var storageDirectory: URL { directory }
    public var networkAccess: () async throws -> Void = {}

    public init(
        directory: URL = FileTranslationRepository.defaultDirectory(),
        transport: any HTTPTransport = URLSessionStreamTransport(),
        credentials: (any CredentialStore)? = nil
    ) {
        let catalog = TranslationApplicationCatalog(directory: directory)
        let switchable: SwitchableCredentialStore?
        let resolvedCredentials: any CredentialStore
        if let credentials {
            switchable = nil
            resolvedCredentials = credentials
        } else {
            let initialMode = TranslationApplicationCatalog.storedCredentialStorageMode(in: directory)
            let store = SwitchableCredentialStore(
                mode: initialMode,
                keychain: TranslationService.platformCredentialStore(),
                localFile: LocalFileCredentialStore(
                    fileURL: directory.appendingPathComponent("bridge-credentials.json")
                ),
                sessionOnly: SessionOnlyCredentialStore()
            )
            switchable = store
            resolvedCredentials = store
        }
        self.catalog = catalog
        self.directory = directory
        self.switchableCredentials = switchable
        self.credentials = resolvedCredentials
        self.sentenceCache = PersistentSentenceCache(
            fileURL: directory.appendingPathComponent("bridge-translation-cache.json")
        )
        translator = TranslationService(transport: transport, credentialStore: resolvedCredentials)
    }

    init(
        directory: URL,
        transport: any HTTPTransport = URLSessionStreamTransport(),
        keychainStore: any CredentialStore
    ) {
        let catalog = TranslationApplicationCatalog(directory: directory)
        let initialMode = TranslationApplicationCatalog.storedCredentialStorageMode(in: directory)
        let store = SwitchableCredentialStore(
            mode: initialMode,
            keychain: keychainStore,
            localFile: LocalFileCredentialStore(
                fileURL: directory.appendingPathComponent("bridge-credentials.json")
            ),
            sessionOnly: SessionOnlyCredentialStore()
        )
        self.catalog = catalog
        self.directory = directory
        self.switchableCredentials = store
        self.credentials = store
        self.sentenceCache = PersistentSentenceCache(
            fileURL: directory.appendingPathComponent("bridge-translation-cache.json")
        )
        translator = TranslationService(transport: transport, credentialStore: store)
    }

    public func call(_ method: String, params: [String: JSONValue]) async throws -> JSONValue {
        do {
            switch method {
            case "translation.providers/list":
                return try await listProviders()
            case "translation.providers/save":
                return try await saveProvider(params)
            case "translation.providers/delete":
                return try await deleteProvider(params)
            case "translation.credential.set":
                return try await setCredential(params)
            case "translation.credential.delete":
                return try await deleteCredential(params)
            case "translation.models":
                return try await discoverModels(params)
            case "translation.modelInfo":
                return try await modelInfo(params)
            case "translation.testConnection":
                return try await testConnection(params)
            case "translation.settings/get":
                return try await getSettings()
            case "translation.settings/save":
                return try await saveSettings(params)
            case "translation.cache/list":
                return try await listSentenceCache(params)
            case "translation.cache/delete":
                return try await deleteSentenceCache(params)
            default:
                throw BridgeFailure("method_unavailable", "Translation method is unavailable")
            }
        } catch let failure as BridgeFailure {
            throw failure
        } catch let error as TranslationError {
            throw BridgeFailure("translation_error", error.localizedDescription)
        } catch {
            throw BridgeFailure("translation_operation_failed", "Translation operation failed")
        }
    }

    private func listProviders() async throws -> JSONValue {
        let profiles = try await catalog.providers()
        var values: [JSONValue] = []
        for profile in profiles {
            try await values.append(providerDTO(profile))
        }
        return .array(values)
    }

    private func saveProvider(_ params: [String: JSONValue]) async throws -> JSONValue {
        guard let value = params["provider"], let object = value.object else {
            throw BridgeFailure("invalid_params", "Expected provider object")
        }
        guard object["key"] == nil, object["apiKey"] == nil, object["secret"] == nil else {
            throw BridgeFailure("invalid_params", "Credentials must be set separately")
        }
        var profile = try value.decoded(ProviderConfiguration.self)
        try profile.validate()
        var profiles = try await catalog.providers()
        if let index = profiles.firstIndex(where: { $0.id == profile.id }) {
            profile.credentialReference = profiles[index].credentialReference
            profiles[index] = profile
        } else {
            guard profiles.count < 64
            else { throw BridgeFailure("provider_limit", "At most 64 providers can be saved") }
            profile.credentialReference = nil
            profiles.append(profile)
        }
        try await catalog.saveProviders(profiles)
        return try await providerDTO(profile)
    }

    private func deleteProvider(_ params: [String: JSONValue]) async throws -> JSONValue {
        let id = try providerID(params)
        var profiles = try await catalog.providers()
        guard let profile = profiles.first(where: { $0.id == id }) else {
            throw BridgeFailure("provider_not_found", "Provider profile was not found")
        }
        profiles.removeAll { $0.id == id }
        try await catalog.saveProviders(profiles)
        if let reference = profile.credentialReference {
            try await credentials.remove(reference: reference)
        }
        return .object(["deleted": .bool(true)])
    }

    private func setCredential(_ params: [String: JSONValue]) async throws -> JSONValue {
        let id = try providerID(params)
        let key = try params.text("key")
        var profiles = try await catalog.providers()
        guard let index = profiles.firstIndex(where: { $0.id == id }) else {
            throw BridgeFailure("provider_not_found", "Provider profile was not found")
        }
        let profile = profiles[index]
        let reference = profile.credentialReference ?? UUID().uuidString.lowercased()
        try await credentials.store(key, for: reference)
        profiles[index].credentialReference = reference
        do {
            try await catalog.saveProviders(profiles)
        } catch {
            if profile.credentialReference == nil { try? await credentials.remove(reference: reference) }
            throw error
        }
        return await .object([
            "credentialReference": .string(reference),
            "storage": .string(credentials.availability().rawValue)
        ])
    }

    private func deleteCredential(_ params: [String: JSONValue]) async throws -> JSONValue {
        let id = try providerID(params)
        var profiles = try await catalog.providers()
        guard let index = profiles.firstIndex(where: { $0.id == id }) else {
            throw BridgeFailure("provider_not_found", "Provider profile was not found")
        }
        guard let reference = profiles[index].credentialReference else {
            return .object(["deleted": .bool(false)])
        }
        try await credentials.remove(reference: reference)
        profiles[index].credentialReference = nil
        try await catalog.saveProviders(profiles)
        return .object(["deleted": .bool(true)])
    }

    private func discoverModels(_ params: [String: JSONValue]) async throws -> JSONValue {
        let profile = try await provider(providerID(params))
        try await requireRemoteAccess(for: profile)
        let models = try await translator.discoverModels(for: profile)
        return try .encoded(models)
    }

    private func modelInfo(_ params: [String: JSONValue]) async throws -> JSONValue {
        let profile = try await provider(providerID(params))
        let model = try params["modelID"].map { try $0.decoded(String.self) } ?? profile.modelID
        guard let model, !model.isEmpty else { throw TranslationError.missingModel }
        try await requireRemoteAccess(for: profile)
        return try await .encoded(translator.modelInfo(id: model, for: profile))
    }

    private func testConnection(_ params: [String: JSONValue]) async throws -> JSONValue {
        let profile = try await provider(providerID(params))
        try await requireRemoteAccess(for: profile)
        let translation = try await translator.testConnection(configuration: profile)
        return .object(["translation": .string(translation)])
    }

    private func getSettings() async throws -> JSONValue {
        let snapshot = try await catalog.snapshot()
        return try await .object([
            "settings": .encoded(snapshot.settings),
            "remoteProvidersEnabled": .bool(snapshot.remoteProvidersEnabled),
            "credentialStorage": .string(credentials.availability().rawValue)
        ])
    }

    private func saveSettings(_ params: [String: JSONValue]) async throws -> JSONValue {
        guard let value = params["settings"] else { throw BridgeFailure("invalid_params", "Expected settings object") }
        let settings = try value.decoded(TranslationSettings.self)
        try settings.validate()
        let remoteEnabled = try params["remoteProvidersEnabled"].map { value -> Bool in
            guard let flag = value.bool else { throw BridgeFailure(
                "invalid_params",
                "Expected boolean: remoteProvidersEnabled"
            ) }
            return flag
        }
        try await catalog.saveSettings(settings, remoteProvidersEnabled: remoteEnabled)
        if let switchableCredentials {
            await switchableCredentials.setMode(settings.credentialStorageMode)
        }
        return try await getSettings()
    }

    private func listSentenceCache(_ params: [String: JSONValue]) async throws -> JSONValue {
        let gameID: String? = switch params["gameID"] {
        case nil, .null:
            nil
        case let .string(value):
            value.isEmpty ? nil : value
        default:
            throw BridgeFailure("invalid_params", "Expected string: gameID")
        }
        let query: String? = switch params["query"] {
        case nil, .null:
            nil
        case let .string(value):
            value
        default:
            throw BridgeFailure("invalid_params", "Expected string: query")
        }
        let limit: Int? = switch params["limit"] {
        case nil, .null:
            nil
        case let .integer(value):
            if let resolved = Int(exactly: value), (1 ... 5_000).contains(resolved) {
                resolved
            } else {
                throw BridgeFailure("invalid_params", "Expected positive integer: limit")
            }
        default:
            throw BridgeFailure("invalid_params", "Expected integer: limit")
        }
        let stats = try await sentenceCache.stats(gameID: gameID)
        let records = try await sentenceCache.list(gameID: gameID, query: query, limit: limit)
        let entries: [JSONValue] = records.map { record in
            .object([
                "id": .string(record.id),
                "gameID": .string(record.gameID),
                "speaker": record.speaker.map(JSONValue.string) ?? .null,
                "sourceText": .string(record.sourceText),
                "translatedText": .string(record.translatedText),
                "providerName": .string(record.providerName),
                "providerKind": .string(record.providerKind.rawValue),
                "modelID": .string(record.modelID),
                "sourceLanguage": .string(record.sourceLanguage),
                "targetLanguage": .string(record.targetLanguage),
                "hitCount": .integer(Int64(record.hitCount)),
                "updatedAt": .number(record.updatedAt.timeIntervalSince1970)
            ])
        }
        return .object([
            "totalCount": .integer(Int64(stats.totalCount)),
            "gameCount": .integer(Int64(stats.gameCount)),
            "totalBytes": .integer(Int64(stats.totalBytes)),
            "resolvedGameID": gameID.map(JSONValue.string) ?? .null,
            "entries": .array(entries)
        ])
    }

    private func deleteSentenceCache(_ params: [String: JSONValue]) async throws -> JSONValue {
        let id = try params.text("id")
        guard !id.isEmpty else { throw BridgeFailure("invalid_params", "Expected non-empty string: id") }
        let removed = try await sentenceCache.delete(id: id)
        let stats = try await sentenceCache.stats(gameID: nil)
        return .object([
            "removed": .bool(removed),
            "totalCount": .integer(Int64(stats.totalCount)),
            "totalBytes": .integer(Int64(stats.totalBytes))
        ])
    }

    private func provider(_ id: UUID) async throws -> ProviderConfiguration {
        guard let profile = try await catalog.providers().first(where: { $0.id == id }) else {
            throw BridgeFailure("provider_not_found", "Provider profile was not found")
        }
        return profile
    }

    private func providerDTO(_ profile: ProviderConfiguration) async throws -> JSONValue {
        let hasCredential: Bool = if let reference = profile.credentialReference {
            try await credentials.hasSecret(for: reference)
        } else {
            false
        }
        return try .object([
            "id": .string(profile.id.uuidString.lowercased()),
            "name": .string(profile.name),
            "kind": .string(profile.kind.rawValue),
            "baseURL": .string(profile.baseURL.absoluteString),
            "modelID": profile.modelID.map(JSONValue.string) ?? .null,
            "defaultSourceLanguage": .string(profile.defaultSourceLanguage),
            "defaultTargetLanguage": .string(profile.defaultTargetLanguage),
            "modelMetadata": profile.modelMetadata.map(JSONValue.encoded) ?? .null,
            "openAIChatTokenLimitField": .string(profile.openAIChatTokenLimitField.rawValue),
            "thinkingLevel": .string(profile.thinkingLevel.rawValue),
            "capabilities": .encoded(profile.capabilities),
            "hasCredential": .bool(hasCredential)
        ])
    }

    private func providerID(_ params: [String: JSONValue]) throws -> UUID {
        guard let id = try UUID(uuidString: params.text("providerID")) else {
            throw BridgeFailure("invalid_params", "Invalid providerID")
        }
        return id
    }

    private func requireRemoteAccess(for profile: ProviderConfiguration) async throws {
        guard !Self.isLoopback(profile.baseURL) else { return }
        try await networkAccess()
        guard try await catalog.snapshot().remoteProvidersEnabled else {
            throw BridgeFailure("remote_providers_disabled", "Enable remote providers in translation settings first")
        }
    }

    func captureConfiguration(providerID: UUID) async throws -> ProviderConfiguration {
        try await provider(providerID)
    }

    func captureSettings() async throws -> TranslationSettings { try await catalog.snapshot().settings }

    func currentDebounceMs() async -> Int {
        await (try? catalog.snapshot().settings.translationDebounceMs) ?? 150
    }

    func clearSentenceCache(gameID: String?) async throws -> (totalCount: Int, totalBytes: Int) {
        try await sentenceCache.clear(gameID: gameID)
        let stats = try await sentenceCache.stats(gameID: nil)
        return (totalCount: stats.totalCount, totalBytes: stats.totalBytes)
    }

    func allowTranslation(providerID: UUID) async throws {
        try await requireRemoteAccess(for: provider(providerID))
    }

    func makeCaptureSession(gameID: String, providerID: UUID, restoreHistory: Bool) async throws -> TranslationSession {
        let configuration = try await provider(providerID)
        let settings = try await catalog.snapshot().settings
        let repository = CaptureHistoryRepository(directory: directory, gameID: gameID)
        let id = UUID()
        var entries: [TranslationHistoryEntry] = []
        if restoreHistory, settings.historyRetentionTurns > 0 {
            entries = try await repository.loadHistory(gameID: gameID, sessionID: id)?.entries ?? []
            entries = entries.map { value in
                var value = value
                if value.status == .pending { value.status = .cancelled }
                return value
            }
        } else {
            try await repository.clearHistory(gameID: gameID, sessionID: id)
        }
        return TranslationSession(
            gameID: gameID,
            sessionID: id,
            configuration: configuration,
            settings: settings,
            service: translator,
            repository: repository,
            sentenceCache: sentenceCache,
            providerDisplayName: configuration.name,
            initialEntries: entries
        )
    }

    private static func isLoopback(_ url: URL) -> Bool {
        guard let host = URLComponents(url: url, resolvingAgainstBaseURL: false)?.host?.lowercased()
        else { return false }
        if host == "localhost" || host.hasSuffix(".localhost") || host == "::1" { return true }
        let octets = host.split(separator: ".")
        return octets.count == 4 && octets.first == "127"
            && octets.allSatisfy { Int($0).map { (0 ... 255).contains($0) } == true }
    }
}

private actor SwitchableCredentialStore: CredentialStore {
    private var mode: CredentialStorageMode
    private let keychain: any CredentialStore
    private let localFile: LocalFileCredentialStore
    private let sessionOnly: SessionOnlyCredentialStore

    init(
        mode: CredentialStorageMode,
        keychain: any CredentialStore,
        localFile: LocalFileCredentialStore,
        sessionOnly: SessionOnlyCredentialStore
    ) {
        self.mode = mode
        self.keychain = keychain
        self.localFile = localFile
        self.sessionOnly = sessionOnly
    }

    func setMode(_ mode: CredentialStorageMode) {
        self.mode = mode
    }

    private func activeStore() -> any CredentialStore {
        switch mode {
        case .keychain: keychain
        case .localFile: localFile
        case .sessionOnly: sessionOnly
        }
    }

    func availability() async -> CredentialStoreAvailability {
        await activeStore().availability()
    }

    func store(_ secret: String, for reference: String) async throws {
        try await activeStore().store(secret, for: reference)
    }

    func secret(for reference: String) async throws -> String? {
        try await activeStore().secret(for: reference)
    }

    func hasSecret(for reference: String) async throws -> Bool {
        try await activeStore().hasSecret(for: reference)
    }

    func remove(reference: String) async throws {
        try await activeStore().remove(reference: reference)
    }
}

private struct TranslationCatalogSnapshot: Codable, Sendable {
    var version = 1
    var profiles: [ProviderConfiguration] = []
    var settings = TranslationSettings()
    var remoteProvidersEnabled = true
}

private actor TranslationApplicationCatalog {
    private let directory: URL
    private let fileManager = FileManager.default
    private var current: TranslationCatalogSnapshot?

    init(directory: URL) { self.directory = directory }

    static func storedCredentialStorageMode(in directory: URL) -> CredentialStorageMode {
        let file = directory.appendingPathComponent("bridge-profiles.json")
        guard let data = try? Data(contentsOf: file),
              let state = try? JSONDecoder().decode(TranslationCatalogSnapshot.self, from: data)
        else { return .keychain }
        return state.settings.credentialStorageMode
    }

    func snapshot() throws -> TranslationCatalogSnapshot { try load() }
    func providers() throws -> [ProviderConfiguration] { try load().profiles }

    func saveProviders(_ profiles: [ProviderConfiguration]) throws {
        var state = try load()
        state.profiles = profiles
        try save(state)
    }

    func saveSettings(_ settings: TranslationSettings, remoteProvidersEnabled: Bool?) throws {
        var state = try load()
        state.settings = settings
        if let remoteProvidersEnabled { state.remoteProvidersEnabled = remoteProvidersEnabled }
        try save(state)
    }

    private func load() throws -> TranslationCatalogSnapshot {
        if let current { return current }
        let file = directory.appendingPathComponent("bridge-profiles.json")
        guard fileManager.fileExists(atPath: file.path) else {
            let fresh = TranslationCatalogSnapshot()
            current = fresh
            return fresh
        }
        do {
            let data = try Data(contentsOf: file)
            let state = try JSONDecoder().decode(TranslationCatalogSnapshot.self, from: data)
            guard state.version == 1, state.profiles.count <= 64,
                  Set(state.profiles.map(\.id)).count == state.profiles.count
            else { throw BridgeFailure("translation_storage_unavailable", "Saved translation profiles are invalid") }
            try state.profiles.forEach { try $0.validate() }
            try state.settings.validate()
            current = state
            return state
        } catch let failure as BridgeFailure {
            throw failure
        } catch {
            throw BridgeFailure("translation_storage_unavailable", "Saved translation settings could not be read")
        }
    }

    private func save(_ state: TranslationCatalogSnapshot) throws {
        do {
            try fileManager.createDirectory(
                at: directory,
                withIntermediateDirectories: true,
                attributes: [.posixPermissions: 0o700]
            )
            let data = try JSONEncoder.sorted.encode(state)
            let file = directory.appendingPathComponent("bridge-profiles.json")
            try data.write(to: file, options: .atomic)
            try fileManager.setAttributes([.posixPermissions: 0o600], ofItemAtPath: file.path)
            current = state
        } catch {
            throw BridgeFailure("translation_storage_unavailable", "Translation profiles could not be saved")
        }
    }
}

private extension JSONEncoder {
    static var sorted: JSONEncoder {
        let encoder = JSONEncoder()
        encoder.outputFormatting = [.sortedKeys]
        return encoder
    }
}
