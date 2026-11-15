import Foundation
import Testing
@testable import WhiskyTranslation

@Suite("Translation models, settings, and persistence")
struct ModelAndPersistenceTests {
    @Test func providerConfigurationRedactsNoSecretBecauseItStoresOnlyReference() throws {
        let config = ProviderConfiguration(kind: .openAIChat, modelID: "fixture-model", credentialReference: "credential-ref", openAIChatTokenLimitField: .maxTokens)
        let data = try JSONEncoder.sorted.encode(config)
        let json = String(decoding: data, as: UTF8.self)
        #expect(json.contains("credential-ref"))
        #expect(!json.contains("fixture-secret-never-persist"))
        #expect(!json.contains("apiKey"))
        #expect(config.capabilities.modelDiscovery)
        #expect(try JSONDecoder().decode(ProviderConfiguration.self, from: data).openAIChatTokenLimitField == .maxTokens)
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
        let configuration = ProviderConfiguration(kind: .openAIChat, defaultSourceLanguage: "ja", defaultTargetLanguage: "fr")
        let settings = TranslationSettings.defaults(for: configuration)
        #expect(settings.sourceLanguage == "ja" && settings.targetLanguage == "fr")
    }

    @Test func validationRejectsRemoteHTTPAndURLCredentialsButAllowsLoopbackHTTP() throws {
        let remote = ProviderConfiguration(kind: .ollama, baseURL: URL(string: "http://example.com:11434")!)
        do { try remote.validate(); Issue.record("remote cleartext endpoint accepted") }
        catch let error as TranslationError { #expect(error == .insecureEndpoint) }
        let withCredentials = ProviderConfiguration(kind: .ollama, baseURL: URL(string: "https://user:pass@example.com")!)
        do { try withCredentials.validate(); Issue.record("URL credentials accepted") }
        catch let error as TranslationError { #expect(error == .invalidConfiguration("Provider URL must not contain credentials, query, or fragment")) }
        try ProviderConfiguration(kind: .ollama, baseURL: URL(string: "http://127.0.0.1:11434/")!).validate()
        try ProviderConfiguration(kind: .ollama, baseURL: URL(string: "http://localhost:11434")!).validate()
    }

    @Test func settingsRejectInvalidAndBoundValues() throws {
        for settings in [
            TranslationSettings(contextWindowOverride: 0),
            TranslationSettings(outputTokenLimit: 0),
            TranslationSettings(historyRetentionTurns: -1),
            TranslationSettings(contextTurnLimit: 1_001),
            TranslationSettings(targetLanguage: "auto")
        ] {
            do { try settings.validate(); Issue.record("invalid settings accepted") }
            catch is TranslationError { }
        }
        try TranslationSettings(contextWindowOverride: 1_000_000, outputTokenLimit: 32_768).validate()
    }

    @Test func discoveryDoesNotClearSelectedManualModel() {
        let config = ProviderConfiguration(kind: .openAIChat, modelID: "manual-model")
        let unchanged = config.withDiscoveredModels([])
        #expect(unchanged.modelID == "manual-model")
        #expect(unchanged.effectiveModelDescriptor()?.contextWindowProvenance == .unknown)
        let updated = config.withDiscoveredModels([ModelDescriptor(id: "other"), ModelDescriptor(id: "manual-model", contextWindow: 16_000)])
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
            repository: FileTranslationRepository(directory: FileManager.default.temporaryDirectory.appending(path: UUID().uuidString, directoryHint: .isDirectory))
        )
        do { _ = try await service.discoverModels(for: configuration); Issue.record("malformed model list accepted") }
        catch let error as TranslationError { #expect(error == .malformedResponse) }
        #expect(await session.currentConfiguration().modelID == "manual-selected")
    }

    @Test func fileRepositoryPersistsPrivatelyWithoutCredentials() async throws {
        let directory = FileManager.default.temporaryDirectory.appending(path: UUID().uuidString, directoryHint: .isDirectory)
        defer { try? FileManager.default.removeItem(at: directory) }
        let repository = FileTranslationRepository(directory: directory)
        let configuration = ProviderConfiguration(kind: .openAIChat, modelID: "fixture-model", credentialReference: "credential-ref")
        try await repository.saveConfiguration(configuration)
        let settings = TranslationSettings(customPrompt: "Keep names consistent")
        try await repository.saveSettings(settings)
        let loaded = try await repository.loadConfiguration(id: configuration.id)
        #expect(loaded == configuration)
        #expect(try await repository.loadSettings() == settings)
        let persisted = try String(contentsOf: directory.appending(path: "provider-\(configuration.id.uuidString).json"), encoding: .utf8)
        #expect(persisted.contains("credential-ref"))
        #expect(!persisted.contains("fixture-secret-never-persist"))
        let attributes = try FileManager.default.attributesOfItem(atPath: directory.path)
        #expect((attributes[.posixPermissions] as? Int) == 0o700)
    }

    @Test func sessionOnlyCredentialStoreReportsItsLimits() async throws {
        let store = SessionOnlyCredentialStore()
        #expect(await store.availability() == .sessionOnly)
        try await store.store("only-in-memory", for: "ref")
        #expect(try await store.secret(for: "ref") == "only-in-memory")
        try await store.remove(reference: "ref")
        #expect(try await store.secret(for: "ref") == nil)
    }
}
