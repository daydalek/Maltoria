import Foundation
import Testing
@testable import WhiskyTranslation

@Suite("Translation session history and lifecycle")
struct TranslationSessionTests {
    @Test func omittedSettingsUseProviderLanguageDefaults() async throws {
        let provider = RecordingProvider()
        let service = TranslationService(providers: [.openAIChat: provider], credentialStore: try await testCredentialStore())
        let configuration = ProviderConfiguration(
            kind: .openAIChat,
            modelID: "fixture-model",
            credentialReference: "credential-ref",
            defaultSourceLanguage: "ja",
            defaultTargetLanguage: "fr"
        )
        let session = TranslationSession(
            gameID: "game-defaults",
            configuration: configuration,
            service: service,
            repository: FileTranslationRepository(
                directory: FileManager.default.temporaryDirectory.appending(path: UUID().uuidString, directoryHint: .isDirectory)
            )
        )
        let settings = await session.currentSettings()
        #expect(settings.sourceLanguage == "ja")
        #expect(settings.targetLanguage == "fr")
    }

    @Test func selectedContextCustomPromptAndGameIsolationReachProvider() async throws {
        let provider = RecordingProvider()
        let credentials = try await testCredentialStore()
        let service = TranslationService(providers: [.openAIChat: provider], credentialStore: credentials)
        let root = FileManager.default.temporaryDirectory.appending(path: UUID().uuidString, directoryHint: .isDirectory)
        defer { try? FileManager.default.removeItem(at: root) }
        let repository = FileTranslationRepository(directory: root)
        let config = ProviderConfiguration(kind: .openAIChat, modelID: "fixture-model", credentialReference: "credential-ref")
        let settings = TranslationSettings(contextTurnLimit: 4, customPrompt: "Preserve honorifics")
        let session = TranslationSession(gameID: "game-one", configuration: config, settings: settings, service: service, repository: repository)
        let first = try await session.submit(FinalizedTranslationSource(revision: 1, streamID: "stream", generation: 1, text: "first line", speaker: "Mina"))
        _ = try await waitForEntry(session, id: first)
        try await session.setIncludedInContext(false, entryID: first)
        let second = try await session.submit(FinalizedTranslationSource(revision: 2, streamID: "stream", generation: 1, text: "second line"))
        _ = try await waitForEntry(session, id: second)
        let requests = provider.recordedRequests()
        #expect(requests.count == 2)
        #expect(requests[1].context.isEmpty)
        #expect(requests[1].customPrompt == "Preserve honorifics")
        #expect(requests.allSatisfy { $0.gameID == "game-one" })

        let isolated = TranslationSession(gameID: "game-two", configuration: config, settings: settings, service: service, repository: repository)
        let other = try await isolated.submit(FinalizedTranslationSource(revision: 1, streamID: "stream", generation: 1, text: "other game"))
        _ = try await waitForEntry(isolated, id: other)
        #expect(provider.recordedRequests().last?.context.isEmpty == true)
        #expect(provider.recordedRequests().last?.gameID == "game-two")
    }

    @Test func deduplicationIsNarrowAndRepeatedDialogueAfterWindowIsKept() async throws {
        let provider = RecordingProvider()
        let service = TranslationService(providers: [.openAIChat: provider], credentialStore: try await testCredentialStore())
        let repository = FileTranslationRepository(directory: FileManager.default.temporaryDirectory.appending(path: UUID().uuidString, directoryHint: .isDirectory))
        let config = ProviderConfiguration(kind: .openAIChat, modelID: "fixture-model", credentialReference: "credential-ref")
        let session = TranslationSession(gameID: "game", configuration: config, settings: TranslationSettings(), service: service, repository: repository)
        let base = Date(timeIntervalSince1970: 1_000)
        let first = try await session.submit(FinalizedTranslationSource(revision: 4, streamID: "same-stream", generation: 1, text: "again", capturedAt: base))
        let duplicate = try await session.submit(FinalizedTranslationSource(revision: 4, streamID: "same-stream", generation: 1, text: "again", capturedAt: base.addingTimeInterval(0.2)))
        #expect(first == duplicate)
        let repeated = try await session.submit(FinalizedTranslationSource(revision: 4, streamID: "same-stream", generation: 1, text: "again", capturedAt: base.addingTimeInterval(1)))
        #expect(repeated != first)
        #expect(await session.history().count == 2)
    }

    @Test func zeroRetentionClearsDiskButKeepsOnlyActiveMemoryEntry() async throws {
        let provider = RecordingProvider()
        let service = TranslationService(providers: [.openAIChat: provider], credentialStore: try await testCredentialStore())
        let root = FileManager.default.temporaryDirectory.appending(path: UUID().uuidString, directoryHint: .isDirectory)
        defer { try? FileManager.default.removeItem(at: root) }
        let repository = FileTranslationRepository(directory: root)
        let config = ProviderConfiguration(kind: .openAIChat, modelID: "fixture-model", credentialReference: "credential-ref")
        var settings = TranslationSettings()
        settings.historyRetentionTurns = 0
        let id = UUID()
        let session = TranslationSession(gameID: "game", sessionID: id, configuration: config, settings: settings, service: service, repository: repository)
        let entryID = try await session.submit(FinalizedTranslationSource(revision: 1, streamID: "s", generation: 1, text: "temporary"))
        _ = try await waitForEntry(session, id: entryID)
        #expect(await session.history().count == 1)
        #expect(try await repository.loadHistory(gameID: "game", sessionID: id) == nil)
        try await session.clearHistory()
        #expect(await session.history().isEmpty)
    }

    @Test func cacheKeyInvalidatesOnPromptContextEndpointAndModel() {
        let config = ProviderConfiguration(kind: .openAIChat, modelID: "model-a")
        let original = TranslationCacheKey(configuration: config, request: testTranslationRequest())
        let changedPrompt = TranslationCacheKey(configuration: config, request: testTranslationRequest(customPrompt: "different"))
        let changedContext = TranslationCacheKey(configuration: config, request: testTranslationRequest(context: [TranslationContextTurn(sourceText: "past", translatedText: "prior")]))
        let changedEndpoint = TranslationCacheKey(configuration: ProviderConfiguration(kind: .openAIChat, baseURL: URL(string: "https://example.invalid/v1")!, modelID: "model-a"), request: testTranslationRequest())
        let changedModel = TranslationCacheKey(configuration: ProviderConfiguration(kind: .openAIChat, modelID: "model-b"), request: testTranslationRequest())
        #expect(original != changedPrompt)
        #expect(original != changedContext)
        #expect(original != changedEndpoint)
        #expect(original != changedModel)
    }

    @Test func lateOldResponseCannotReplaceActiveSubtitle() async throws {
        let provider = ControlledProvider()
        let service = TranslationService(providers: [.openAIChat: provider], credentialStore: try await testCredentialStore())
        let repository = FileTranslationRepository(directory: FileManager.default.temporaryDirectory.appending(path: UUID().uuidString, directoryHint: .isDirectory))
        let config = ProviderConfiguration(kind: .openAIChat, modelID: "fixture-model", credentialReference: "credential-ref")
        let session = TranslationSession(gameID: "game", configuration: config, settings: TranslationSettings(), service: service, repository: repository)
        let oldID = try await session.submit(FinalizedTranslationSource(revision: 1, streamID: "a", generation: 1, text: "old"))
        for _ in 0 ..< 100 where provider.recordedRequests().isEmpty { try await Task.sleep(for: .milliseconds(5)) }
        let oldEventID = try #require(provider.recordedRequests().first?.eventID)
        let newID = try await session.submit(FinalizedTranslationSource(revision: 2, streamID: "a", generation: 1, text: "new"))
        for _ in 0 ..< 100 where provider.recordedRequests().count < 2 { try await Task.sleep(for: .milliseconds(5)) }
        let newEventID = try #require(provider.recordedRequests().last?.eventID)
        provider.complete(eventID: oldEventID, text: "late old")
        provider.complete(eventID: newEventID, text: "current")
        let current = try await waitForEntry(session, id: newID)
        #expect(await session.activeEntry()?.id == newID)
        #expect(current.translatedText == "current")
        #expect(oldID != newID)
        let old = try #require(await session.history().first(where: { $0.id == oldID }))
        #expect(old.status == .cancelled)
        #expect(old.translatedText == nil)
    }

    @Test func restoredHistoryImmediatelyHonorsRetentionLimitAndZero() async throws {
        let provider = RecordingProvider()
        let service = TranslationService(providers: [.openAIChat: provider], credentialStore: try await testCredentialStore())
        let root = FileManager.default.temporaryDirectory.appending(path: UUID().uuidString, directoryHint: .isDirectory)
        defer { try? FileManager.default.removeItem(at: root) }
        let repository = FileTranslationRepository(directory: root)
        let configuration = ProviderConfiguration(kind: .openAIChat, modelID: "fixture-model", credentialReference: "credential-ref")
        let gameID = "retention-restore"
        let limitedSessionID = UUID()
        let entries = (1 ... 3).map { revision in
            TranslationHistoryEntry(
                source: FinalizedTranslationSource(revision: revision, streamID: "stream", generation: 1, text: "source-\(revision)"),
                translatedText: "translated-\(revision)",
                status: .completed
            )
        }
        try await repository.saveHistory(TranslationSessionSnapshot(gameID: gameID, sessionID: limitedSessionID, entries: entries))
        let limited = try await TranslationSession.restore(
            gameID: gameID,
            sessionID: limitedSessionID,
            configuration: configuration,
            settings: TranslationSettings(historyRetentionTurns: 1),
            service: service,
            repository: repository
        )
        #expect(await limited.history().map(\.id) == [entries[2].id])
        #expect(try await repository.loadHistory(gameID: gameID, sessionID: limitedSessionID)?.entries.map(\.id) == [entries[2].id])

        let zeroSessionID = UUID()
        try await repository.saveHistory(TranslationSessionSnapshot(gameID: gameID, sessionID: zeroSessionID, entries: entries))
        let zero = try await TranslationSession.restore(
            gameID: gameID,
            sessionID: zeroSessionID,
            configuration: configuration,
            settings: TranslationSettings(historyRetentionTurns: 0),
            service: service,
            repository: repository
        )
        #expect(await zero.history().isEmpty)
        #expect(try await repository.loadHistory(gameID: gameID, sessionID: zeroSessionID) == nil)
    }

    @Test func configurationChangeInvalidatesLateResponseForActiveEntry() async throws {
        let provider = ControlledProvider()
        let service = TranslationService(providers: [.openAIChat: provider], credentialStore: try await testCredentialStore())
        let repository = FileTranslationRepository(directory: FileManager.default.temporaryDirectory.appending(path: UUID().uuidString, directoryHint: .isDirectory))
        let config = ProviderConfiguration(kind: .openAIChat, modelID: "before", credentialReference: "credential-ref")
        let session = TranslationSession(gameID: "config-race", configuration: config, service: service, repository: repository)
        let entryID = try await session.submit(FinalizedTranslationSource(revision: 1, streamID: "s", generation: 1, text: "pending"))
        for _ in 0 ..< 100 where provider.recordedRequests().isEmpty { try await Task.sleep(for: .milliseconds(5)) }
        let eventID = try #require(provider.recordedRequests().first?.eventID)
        try await session.updateConfiguration(ProviderConfiguration(kind: .openAIChat, modelID: "after", credentialReference: "credential-ref"))
        provider.complete(eventID: eventID, text: "late result")
        try await Task.sleep(for: .milliseconds(20))
        let entry = try #require(await session.history().first(where: { $0.id == entryID }))
        #expect(entry.status == .cancelled)
        #expect(entry.translatedText == nil)
    }

    @Test func cacheReusesOnlyMatchingContextAwareRequest() async throws {
        let provider = RecordingProvider()
        let credentials = try await testCredentialStore()
        let service = TranslationService(providers: [.openAIChat: provider], credentialStore: credentials)
        let cache = TranslationCache()
        let repository = FileTranslationRepository(directory: FileManager.default.temporaryDirectory.appending(path: UUID().uuidString, directoryHint: .isDirectory))
        let config = ProviderConfiguration(kind: .openAIChat, modelID: "fixture-model", credentialReference: "credential-ref")
        let settings = TranslationSettings(historyRetentionTurns: 10, contextTurnLimit: 0)
        let firstSession = TranslationSession(gameID: "game", configuration: config, settings: settings, service: service, repository: repository, cache: cache)
        let oldID = try await firstSession.submit(FinalizedTranslationSource(revision: 1, streamID: "a", generation: 1, text: "old"))
        let old = try await waitForEntry(firstSession, id: oldID)
        #expect(old.translatedText == "translated: old")
        let cachedSession = TranslationSession(gameID: "game", configuration: config, settings: settings, service: service, repository: repository, cache: cache)
        let cachedID = try await cachedSession.submit(FinalizedTranslationSource(revision: 3, streamID: "b", generation: 1, text: "old"))
        let cached = try await waitForEntry(cachedSession, id: cachedID)
        #expect(cached.translatedText == "translated: old")
        #expect(provider.recordedRequests().count == 1)
    }

    @Test func sourceExceedingBudgetIsRecordedAsFailureNotSilentlyTruncated() async throws {
        let provider = RecordingProvider()
        let service = TranslationService(providers: [.openAIChat: provider], credentialStore: try await testCredentialStore())
        let repository = FileTranslationRepository(directory: FileManager.default.temporaryDirectory.appending(path: UUID().uuidString, directoryHint: .isDirectory))
        let config = ProviderConfiguration(kind: .openAIChat, modelID: "fixture-model", credentialReference: "credential-ref")
        let settings = TranslationSettings(outputTokenLimit: 512)
        let session = TranslationSession(gameID: "game", configuration: config, settings: settings, service: service, repository: repository)
        let id = try await session.submit(FinalizedTranslationSource(revision: 1, streamID: "s", generation: 1, text: String(repeating: "x", count: 5_000)))
        let entry = try await waitForEntry(session, id: id)
        #expect(entry.status == .failed)
        #expect(entry.failureDescription?.contains("exceeds") == true)
        #expect(provider.recordedRequests().isEmpty)
    }

    @Test func olderRevisionOrGenerationForSameEventIsRejected() async throws {
        let provider = RecordingProvider()
        let service = TranslationService(providers: [.openAIChat: provider], credentialStore: try await testCredentialStore())
        let repository = FileTranslationRepository(directory: FileManager.default.temporaryDirectory.appending(path: UUID().uuidString, directoryHint: .isDirectory))
        let config = ProviderConfiguration(kind: .openAIChat, modelID: "fixture-model", credentialReference: "credential-ref")
        let session = TranslationSession(gameID: "game", configuration: config, service: service, repository: repository)
        let eventID = UUID()
        let first = try await session.submit(FinalizedTranslationSource(eventID: eventID, revision: 2, streamID: "s", generation: 3, text: "latest"))
        _ = try await waitForEntry(session, id: first)
        for stale in [
            FinalizedTranslationSource(eventID: eventID, revision: 1, streamID: "s", generation: 3, text: "old revision"),
            FinalizedTranslationSource(eventID: eventID, revision: 99, streamID: "s", generation: 2, text: "old generation")
        ] {
            do { _ = try await session.submit(stale); Issue.record("stale source accepted") }
            catch let error as TranslationError { #expect(error == .staleSource) }
        }
        let newerGeneration = try await session.submit(FinalizedTranslationSource(eventID: eventID, revision: 1, streamID: "s", generation: 4, text: "new generation"))
        _ = try await waitForEntry(session, id: newerGeneration)
        #expect(provider.recordedRequests().count == 2)
    }

    @Test func selectedContextCanBeSourceOnlyOrTranslationOnly() async throws {
        let provider = RecordingProvider()
        let service = TranslationService(providers: [.openAIChat: provider], credentialStore: try await testCredentialStore())
        let repository = FileTranslationRepository(directory: FileManager.default.temporaryDirectory.appending(path: UUID().uuidString, directoryHint: .isDirectory))
        let config = ProviderConfiguration(kind: .openAIChat, modelID: "fixture-model", credentialReference: "credential-ref")
        let sourceOnly = TranslationSession(
            gameID: "source-only", configuration: config,
            settings: TranslationSettings(includeSourceHistory: true, includeTranslatedHistory: false),
            service: service, repository: repository
        )
        let firstSource = try await sourceOnly.submit(FinalizedTranslationSource(revision: 1, streamID: "s", generation: 1, text: "source line"))
        _ = try await waitForEntry(sourceOnly, id: firstSource)
        let secondSource = try await sourceOnly.submit(FinalizedTranslationSource(revision: 2, streamID: "s", generation: 1, text: "next source"))
        _ = try await waitForEntry(sourceOnly, id: secondSource)
        let sourceContext = try #require(provider.recordedRequests().last?.context.first)
        #expect(sourceContext.sourceText == "source line")
        #expect(sourceContext.translatedText == nil)

        let translationOnly = TranslationSession(
            gameID: "translation-only", configuration: config,
            settings: TranslationSettings(includeSourceHistory: false, includeTranslatedHistory: true),
            service: service, repository: repository
        )
        let firstTranslation = try await translationOnly.submit(FinalizedTranslationSource(revision: 1, streamID: "t", generation: 1, text: "spoken line"))
        _ = try await waitForEntry(translationOnly, id: firstTranslation)
        let secondTranslation = try await translationOnly.submit(FinalizedTranslationSource(revision: 2, streamID: "t", generation: 1, text: "another line"))
        _ = try await waitForEntry(translationOnly, id: secondTranslation)
        let translationContext = try #require(provider.recordedRequests().last?.context.first)
        #expect(translationContext.sourceText == nil)
        #expect(translationContext.translatedText == "translated: spoken line")
    }

    @Test func submitReentryCannotStartOlderRequestAfterNewerSubmit() async throws {
        let repository = FirstSaveBlockingRepository()
        let provider = RecordingProvider()
        let service = TranslationService(providers: [.openAIChat: provider], credentialStore: try await testCredentialStore())
        let config = ProviderConfiguration(kind: .openAIChat, modelID: "fixture-model", credentialReference: "credential-ref")
        let session = TranslationSession(gameID: "race", configuration: config, service: service, repository: repository)
        let first = Task { try await session.submit(FinalizedTranslationSource(revision: 1, streamID: "s", generation: 1, text: "older")) }
        await repository.waitUntilFirstSaveStarts()
        let second = Task { try await session.submit(FinalizedTranslationSource(revision: 2, streamID: "s", generation: 1, text: "newer")) }
        try await Task.sleep(for: .milliseconds(20))
        await repository.releaseFirstSave()
        do { _ = try await first.value; Issue.record("superseded submit started successfully") }
        catch is CancellationError { }
        let activeID = try await second.value
        _ = try await waitForEntry(session, id: activeID)
        #expect(provider.recordedRequests().map(\.sourceText) == ["newer"])
        #expect(await session.activeEntry()?.id == activeID)
    }

    @Test func clearDuringPersistenceInvalidatesSubmitAndClearsSessionCache() async throws {
        let repository = FirstSaveBlockingRepository()
        let cache = TranslationCache()
        let config = ProviderConfiguration(kind: .openAIChat, modelID: "fixture-model", credentialReference: "credential-ref")
        let request = TranslationRequest(gameID: "clear-race", sessionID: UUID(), eventID: UUID(), generation: 1, sourceText: "cached", speaker: nil, sourceLanguage: "en", targetLanguage: "ja", context: [], customPrompt: "", outputTokenLimit: 10, contextWindowTokens: 4_096, contextWindowProvenance: .unknown)
        let key = TranslationCacheKey(configuration: config, request: request)
        await cache.insert("cached translation", for: key)
        let provider = RecordingProvider()
        let service = TranslationService(providers: [.openAIChat: provider], credentialStore: try await testCredentialStore())
        let session = TranslationSession(gameID: "clear-race", configuration: config, service: service, repository: repository, cache: cache)
        let submit = Task { try await session.submit(FinalizedTranslationSource(revision: 1, streamID: "s", generation: 1, text: "new source")) }
        await repository.waitUntilFirstSaveStarts()
        let clearing = Task { try await session.clearHistory() }
        try await Task.sleep(for: .milliseconds(20))
        await repository.releaseFirstSave()
        try await clearing.value
        do { _ = try await submit.value; Issue.record("submit survived clear") }
        catch is CancellationError { }
        #expect(await session.history().isEmpty)
        let sessionID = await session.sessionID
        #expect(try await repository.loadHistory(gameID: "clear-race", sessionID: sessionID) == nil)
        #expect(await cache.value(for: key) == nil)
        #expect(provider.recordedRequests().isEmpty)
    }
}
