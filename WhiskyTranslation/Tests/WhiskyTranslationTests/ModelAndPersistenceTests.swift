//
//  ModelAndPersistenceTests.swift
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
import Testing
@testable import WhiskyTranslation

@Suite("Translation models, settings, and persistence")
struct ModelAndPersistenceTests {
    @Test func providerConfigurationRedactsNoSecretBecauseItStoresOnlyReference() throws {
        let config = ProviderConfiguration(
            kind: .openAIChat,
            modelID: "fixture-model",
            credentialReference: "credential-ref",
            openAIChatTokenLimitField: .maxTokens
        )
        let data = try JSONEncoder.sorted.encode(config)
        let json = String(decoding: data, as: UTF8.self)
        #expect(json.contains("credential-ref"))
        #expect(!json.contains("fixture-secret-never-persist"))
        #expect(!json.contains("apiKey"))
        #expect(config.capabilities.modelDiscovery)
        #expect(try JSONDecoder().decode(ProviderConfiguration.self, from: data)
            .openAIChatTokenLimitField == .maxTokens)
    }

    @Test func detectedManualAndUnknownContextRemainDistinct() throws {
        let detected = ModelDescriptor(id: "fixture-model", contextWindow: 32_000)
        let manual = ModelDescriptor(id: "fixture-manual", contextWindow: 12_000, contextWindowProvenance: .manual)
        let unknown = ModelDescriptor(id: "fixture-unknown")
        #expect(detected.contextWindowProvenance == .detected)
        #expect(manual.contextWindowProvenance == .manual)
        #expect(unknown.contextWindow == nil && unknown.contextWindowProvenance == .unknown)
        var settings = TranslationSettings()
        #expect(settings.effectiveContextWindow(for: unknown).tokens == 4_096)
        #expect(settings.effectiveContextWindow(for: unknown).provenance == .unknown)
        #expect(settings.contextWindowLabel(for: unknown).contains("Estimated"))
        settings.contextWindowOverride = 8_192
        #expect(settings.effectiveContextWindow(for: detected).tokens == 8_192)
        #expect(settings.effectiveContextWindow(for: detected).provenance == .manual)
    }

    @Test func providerLanguageDefaultsCanSeedTranslationSettings() {
        let configuration = ProviderConfiguration(
            kind: .openAIChat,
            defaultSourceLanguage: "ja",
            defaultTargetLanguage: "fr"
        )
        let settings = TranslationSettings.defaults(for: configuration)
        #expect(settings.sourceLanguage == "ja" && settings.targetLanguage == "fr")
    }

    @Test func validationRejectsRemoteHTTPAndURLCredentialsButAllowsLoopbackHTTP() throws {
        let remote = ProviderConfiguration(kind: .ollama, baseURL: URL(string: "http://example.com:11434")!)
        do { try remote.validate()
            Issue.record("remote cleartext endpoint accepted")
        } catch let error as TranslationError { #expect(error == .insecureEndpoint) }
        let withCredentials = ProviderConfiguration(
            kind: .ollama,
            baseURL: URL(string: "https://user:pass@example.com")!
        )
        do { try withCredentials.validate()
            Issue.record("URL credentials accepted")
        } catch let error as TranslationError {
            #expect(error == .invalidConfiguration("Provider URL must not contain credentials, query, or fragment"))
        }
        try ProviderConfiguration(kind: .ollama, baseURL: URL(string: "http://127.0.0.1:11434/")!).validate()
        try ProviderConfiguration(kind: .ollama, baseURL: URL(string: "http://localhost:11434")!).validate()
    }

    @Test func settingsRejectInvalidAndBoundValues() throws {
        for settings in [
            TranslationSettings(contextWindowOverride: 0),
            TranslationSettings(outputTokenLimit: 0),
            TranslationSettings(historyRetentionTurns: -1),
            TranslationSettings(contextTurnLimit: 1_001),
            TranslationSettings(targetLanguage: "auto"),
            TranslationSettings(translationDebounceMs: -1),
            TranslationSettings(translationDebounceMs: 5_001)
        ] {
            do { try settings.validate()
                Issue.record("invalid settings accepted")
            } catch is TranslationError {}
        }
        try TranslationSettings(
            contextWindowOverride: 1_000_000,
            outputTokenLimit: 32_768,
            translationDebounceMs: 0,
            sentenceCacheEnabled: false
        ).validate()
        try TranslationSettings(translationDebounceMs: 5_000).validate()
    }

    @Test func discoveryDoesNotClearSelectedManualModel() {
        let config = ProviderConfiguration(kind: .openAIChat, modelID: "manual-model")
        let unchanged = config.withDiscoveredModels([])
        #expect(unchanged.modelID == "manual-model")
        #expect(unchanged.effectiveModelDescriptor()?.contextWindowProvenance == .unknown)
        let updated = config.withDiscoveredModels([
            ModelDescriptor(id: "other"),
            ModelDescriptor(id: "manual-model", contextWindow: 16_000)
        ])
        #expect(updated.modelID == "manual-model")
        #expect(updated.modelMetadata?.contextWindow == 16_000)
    }

    @Test func failedDiscoveryLeavesSelectedManualModelIntact() async throws {
        let transport = QueueHTTPTransport([makeHTTPResponse("{\"data\":null}")])
        let service = TranslationService(transport: transport, credentialStore: SessionOnlyCredentialStore())
        let configuration = ProviderConfiguration(
            kind: .openAIChat,
            baseURL: URL(string: "http://localhost:8765/v1")!,
            modelID: "manual-selected"
        )
        let session = TranslationSession(
            gameID: "manual-model",
            configuration: configuration,
            service: service,
            repository: FileTranslationRepository(directory: FileManager.default.temporaryDirectory.appending(
                path: UUID().uuidString,
                directoryHint: .isDirectory
            ))
        )
        do { _ = try await service.discoverModels(for: configuration)
            Issue.record("malformed model list accepted")
        } catch let error as TranslationError { #expect(error == .malformedResponse) }
        #expect(await session.currentConfiguration().modelID == "manual-selected")
    }

    @Test func fileRepositoryPersistsPrivatelyWithoutCredentials() async throws {
        let directory = FileManager.default.temporaryDirectory.appending(
            path: UUID().uuidString,
            directoryHint: .isDirectory
        )
        defer { try? FileManager.default.removeItem(at: directory) }
        let repository = FileTranslationRepository(directory: directory)
        let configuration = ProviderConfiguration(
            kind: .openAIChat,
            modelID: "fixture-model",
            credentialReference: "credential-ref"
        )
        try await repository.saveConfiguration(configuration)
        let settings = TranslationSettings(customPrompt: "Keep names consistent")
        try await repository.saveSettings(settings)
        let loaded = try await repository.loadConfiguration(id: configuration.id)
        #expect(loaded == configuration)
        #expect(try await repository.loadSettings() == settings)
        let persisted = try String(
            contentsOf: directory.appending(path: "provider-\(configuration.id.uuidString).json"),
            encoding: .utf8
        )
        #expect(persisted.contains("credential-ref"))
        #expect(!persisted.contains("fixture-secret-never-persist"))
        let attributes = try FileManager.default.attributesOfItem(atPath: directory.path)
        #expect((attributes[.posixPermissions] as? Int) == 0o700)
    }

    @Test func sessionOnlyCredentialStoreReportsItsLimits() async throws {
        let store = SessionOnlyCredentialStore()
        #expect(await store.availability() == .sessionOnly)
        #expect(try await !store.hasSecret(for: "ref"))
        try await store.store("only-in-memory", for: "ref")
        #expect(try await store.hasSecret(for: "ref"))
        #expect(try await store.secret(for: "ref") == "only-in-memory")
        try await store.remove(reference: "ref")
        #expect(try await !store.hasSecret(for: "ref"))
        #expect(try await store.secret(for: "ref") == nil)
    }

    @Test func localFileCredentialStoreRoundTripsWithPrivatePermissions() async throws {
        let directory = FileManager.default.temporaryDirectory.appending(
            path: UUID().uuidString,
            directoryHint: .isDirectory
        )
        defer { try? FileManager.default.removeItem(at: directory) }
        let fileURL = directory.appending(path: "nested/bridge-credentials.json", directoryHint: .notDirectory)

        let store = LocalFileCredentialStore(fileURL: fileURL)
        #expect(await store.availability() == .localPlaintextFile)
        #expect(try await !store.hasSecret(for: "api-key"))
        #expect(try await store.secret(for: "api-key") == nil)

        try await store.store("super-secret-value", for: "api-key")
        #expect(try await store.hasSecret(for: "api-key"))
        #expect(try await store.secret(for: "api-key") == "super-secret-value")

        let attributes = try FileManager.default.attributesOfItem(atPath: fileURL.path)
        #expect((attributes[.posixPermissions] as? Int) == 0o600)

        let reloaded = LocalFileCredentialStore(fileURL: fileURL)
        #expect(try await reloaded.hasSecret(for: "api-key"))
        #expect(try await reloaded.secret(for: "api-key") == "super-secret-value")

        try await reloaded.remove(reference: "api-key")
        #expect(try await !reloaded.hasSecret(for: "api-key"))
        #expect(try await reloaded.secret(for: "api-key") == nil)

        let afterRemoval = LocalFileCredentialStore(fileURL: fileURL)
        #expect(try await !afterRemoval.hasSecret(for: "api-key"))

        do {
            try await store.store("", for: "api-key")
            Issue.record("empty secret accepted")
        } catch let error as TranslationError {
            #expect(error == .credentialUnavailable)
        }
    }

    @Test func credentialStorageModeDecodesWithBackwardCompatibility() throws {
        let legacyJSON = """
        {
          "outputTokenLimit": 256,
          "historyRetentionTurns": 10,
          "contextTurnLimit": 4,
          "includeSourceHistory": true,
          "includeTranslatedHistory": false,
          "customPrompt": "Keep honorifics",
          "sourceLanguage": "ja",
          "targetLanguage": "en"
        }
        """
        let decodedLegacy = try JSONDecoder().decode(TranslationSettings.self, from: Data(legacyJSON.utf8))
        #expect(decodedLegacy.credentialStorageMode == .keychain)
        #expect(decodedLegacy.outputTokenLimit == 256)
        #expect(decodedLegacy.customPrompt == "Keep honorifics")
        #expect(decodedLegacy.translationDebounceMs == 150)
        #expect(decodedLegacy.sentenceCacheEnabled == true)

        for mode in CredentialStorageMode.allCases {
            let settings = TranslationSettings(
                credentialStorageMode: mode,
                translationDebounceMs: 300,
                sentenceCacheEnabled: false
            )
            let encoded = try JSONEncoder.sorted.encode(settings)
            let decoded = try JSONDecoder().decode(TranslationSettings.self, from: encoded)
            #expect(decoded.credentialStorageMode == mode)
            #expect(decoded.translationDebounceMs == 300)
            #expect(decoded.sentenceCacheEnabled == false)
        }
    }

    @Test func persistentSentenceCachePersistsHitsSearchesDeletesAndEvictsLRU() async throws {
        let directory = FileManager.default.temporaryDirectory.appending(
            path: UUID().uuidString,
            directoryHint: .isDirectory
        )
        defer { try? FileManager.default.removeItem(at: directory) }
        let fileURL = directory.appending(path: "nested/sentence-cache.json", directoryHint: .notDirectory)

        let cache = PersistentSentenceCache(fileURL: fileURL, maximumEntries: 3)
        let first = try #require(await cache.store(
            gameID: "game-a",
            sourceText: "  おはよう  ",
            translatedText: "Good morning",
            speaker: "Alice",
            sourceLanguage: "ja",
            targetLanguage: "en",
            providerName: "Local Ollama",
            providerKind: .ollama,
            modelID: "qwen2.5",
            customPrompt: ""
        ))
        let second = try #require(await cache.store(
            gameID: "game-a",
            sourceText: "おやすみ",
            translatedText: "Good night",
            speaker: "Bob",
            sourceLanguage: "ja",
            targetLanguage: "en",
            providerName: "OpenAI",
            providerKind: .openAIChat,
            modelID: "gpt-4o",
            customPrompt: ""
        ))
        let third = try #require(await cache.store(
            gameID: "game-b",
            sourceText: "こんにちは",
            translatedText: "Hello",
            speaker: nil,
            sourceLanguage: "ja",
            targetLanguage: "en",
            providerName: "OpenAI",
            providerKind: .openAIChat,
            modelID: "gpt-4o",
            customPrompt: ""
        ))

        let dirAttrs = try FileManager.default.attributesOfItem(atPath: fileURL.deletingLastPathComponent().path)
        #expect((dirAttrs[.posixPermissions] as? Int) == 0o700)
        let fileAttrs = try FileManager.default.attributesOfItem(atPath: fileURL.path)
        #expect((fileAttrs[.posixPermissions] as? Int) == 0o600)

        // Lookup trims whitespace, increments hitCount, and updates recency (making `second` the LRU entry)
        let hit = try #require(await cache.lookup(
            gameID: "game-a",
            sourceText: "おはよう",
            speaker: "Alice",
            sourceLanguage: "ja",
            targetLanguage: "en",
            providerKind: .ollama,
            modelID: "qwen2.5",
            customPrompt: ""
        ))
        #expect(hit.id == first.id)
        #expect(hit.hitCount == 2)

        // Adding a 4th entry evicts `second` (least recently used), keeping `first` and `third`
        _ = try await cache.store(
            gameID: "game-b",
            sourceText: "ありがとう",
            translatedText: "Thank you",
            speaker: nil,
            sourceLanguage: "ja",
            targetLanguage: "en",
            providerName: "DeepL",
            providerKind: .deepL,
            modelID: "",
            customPrompt: ""
        )

        // Reload from disk in a fresh instance
        let reloaded = PersistentSentenceCache(fileURL: fileURL, maximumEntries: 3)
        let stats = try await reloaded.stats(gameID: "game-a")
        #expect(stats.totalCount == 3)
        #expect(stats.gameCount == 1)
        #expect(stats.totalBytes > 0)

        let allEntries = try await reloaded.list(gameID: nil, query: nil, limit: nil)
        #expect(allEntries.count == 3)
        #expect(!allEntries.contains(where: { $0.id == second.id }))
        #expect(allEntries.first(where: { $0.id == first.id })?.hitCount == 2)

        // Case-insensitive search across fields (speaker, modelID, sourceText, translatedText, providerName)
        #expect(try await reloaded.list(gameID: nil, query: "ALICE", limit: nil).map(\.id) == [first.id])
        #expect(try await reloaded.list(gameID: nil, query: "qwen2.5", limit: nil).map(\.id) == [first.id])
        #expect(try await reloaded.list(gameID: nil, query: "thank YOU", limit: nil).count == 1)
        #expect(try await reloaded.list(gameID: "game-b", query: nil, limit: 1).count == 1)

        // Delete single record
        #expect(try await reloaded.delete(id: third.id))
        #expect(try await !reloaded.delete(id: third.id))
        #expect(try await reloaded.stats(gameID: nil).totalCount == 2)

        // Clear per game
        try await reloaded.clear(gameID: "game-a")
        #expect(try await reloaded.stats(gameID: nil).totalCount == 1)

        // Clear all
        try await reloaded.clear(gameID: nil)
        let clearedStats = try await reloaded.stats(gameID: nil)
        #expect(clearedStats.totalCount == 0)
        #expect(clearedStats.totalBytes == 0)
    }
}
