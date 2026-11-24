//
//  TranslationSession.swift
//  WhiskyTranslation
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

public struct TranslationCacheKey: Hashable, Sendable {
    public let configurationID: UUID
    public let providerKind: ProviderKind
    public let endpoint: String
    public let modelID: String?
    public let gameID: String
    public let sourceLanguage: String
    public let targetLanguage: String
    public let sourceText: String
    public let speaker: String?
    public let customPrompt: String
    public let contextFingerprint: String
    public let outputTokenLimit: Int
    public let openAIChatTokenLimitField: OpenAIChatTokenLimitField
    public let thinkingLevel: ThinkingLevel
    public let contextWindowTokens: Int
    public let promptVersion: String
    public let normalizerVersion: String

    public init(configuration: ProviderConfiguration, request: TranslationRequest) {
        configurationID = configuration.id
        providerKind = configuration.kind
        endpoint = configuration.baseURL.absoluteString
        modelID = configuration.modelID
        gameID = request.gameID
        sourceLanguage = request.sourceLanguage
        targetLanguage = request.targetLanguage
        sourceText = request.sourceText
        speaker = request.speaker
        customPrompt = request.customPrompt
        contextFingerprint = (try? JSONEncoder.sorted.encode(request.context).base64EncodedString()) ?? ""
        outputTokenLimit = request.outputTokenLimit
        openAIChatTokenLimitField = configuration.openAIChatTokenLimitField
        thinkingLevel = configuration.thinkingLevel
        contextWindowTokens = request.contextWindowTokens
        promptVersion = TranslationPrompt.promptVersion(for: configuration.kind, modelID: configuration.modelID)
        normalizerVersion = "raw-utf8-v1"
    }
}

public actor TranslationCache {
    private let maximumEntries: Int
    private var values: [TranslationCacheKey: String] = [:]
    private var recency: [TranslationCacheKey] = []

    public init(maximumEntries: Int = 256) { self.maximumEntries = max(1, maximumEntries) }

    public func value(for key: TranslationCacheKey) -> String? {
        guard let value = values[key] else { return nil }
        recency.removeAll { $0 == key }
        recency.append(key)
        return value
    }

    public func insert(_ value: String, for key: TranslationCacheKey) {
        values[key] = value
        recency.removeAll { $0 == key }
        recency.append(key)
        while recency.count > maximumEntries {
            values.removeValue(forKey: recency.removeFirst())
        }
    }

    public func removeAll() {
        values.removeAll()
        recency.removeAll()
    }

    public func removeAll(gameID: String) {
        let keys = values.keys.filter { $0.gameID == gameID }
        for key in keys {
            values.removeValue(forKey: key)
        }
        recency.removeAll { $0.gameID == gameID }
    }
}

public actor TranslationSession {
    public let gameID: String
    public let sessionID: UUID
    private var configuration: ProviderConfiguration
    private var settings: TranslationSettings
    private let service: TranslationService
    private let repository: any TranslationRepository
    private let cache: TranslationCache
    private let sentenceCache: (any SentenceTranslationCaching)?
    private let providerDisplayName: String
    private var entries: [TranslationHistoryEntry]
    private var activeEntryID: UUID?
    private var activeTasks: [UUID: Task<Void, Never>] = [:]
    private var activeTaskOrder: [UUID] = []
    private var requestEpoch: UInt64 = 0
    private var requestEpochByEntry: [UUID: UInt64] = [:]
    private var latestSourceVersions: [UUID: SourceVersion] = [:]
    private var sourceVersionOrder: [UUID] = []
    private var persistenceTail: Task<Void, any Error>?
    // swiftlint:disable:next large_tuple
    private var lastDeduplicated: (
        streamID: String,
        generation: Int,
        revision: Int,
        text: String,
        speaker: String?,
        time: Date,
        entryID: UUID
    )?

    public init(
        gameID: String,
        sessionID: UUID = UUID(),
        configuration: ProviderConfiguration,
        settings: TranslationSettings? = nil,
        service: TranslationService,
        repository: any TranslationRepository,
        cache: TranslationCache = TranslationCache(),
        sentenceCache: (any SentenceTranslationCaching)? = nil,
        providerDisplayName: String = "",
        initialEntries: [TranslationHistoryEntry] = []
    ) {
        self.gameID = gameID
        self.sessionID = sessionID
        self.configuration = configuration
        let resolvedSettings = settings ?? TranslationSettings.defaults(for: configuration)
        self.settings = resolvedSettings
        self.service = service
        self.repository = repository
        self.cache = cache
        self.sentenceCache = sentenceCache
        self.providerDisplayName = providerDisplayName
        let retentionLimit = min(max(resolvedSettings.historyRetentionTurns, 0), 1_000)
        entries = retentionLimit == 0 ? [] : Array(initialEntries.suffix(retentionLimit))
        for entry in initialEntries {
            let version = SourceVersion(generation: entry.source.generation, revision: entry.source.revision)
            if let current = latestSourceVersions[entry.source.eventID] {
                if current < version { latestSourceVersions[entry.source.eventID] = version }
            } else {
                latestSourceVersions[entry.source.eventID] = version
                sourceVersionOrder.append(entry.source.eventID)
            }
        }
    }

    public static func restore(
        gameID: String,
        sessionID: UUID,
        configuration: ProviderConfiguration,
        settings: TranslationSettings,
        service: TranslationService,
        repository: any TranslationRepository,
        cache: TranslationCache = TranslationCache(),
        sentenceCache: (any SentenceTranslationCaching)? = nil,
        providerDisplayName: String = ""
    ) async throws -> TranslationSession {
        let snapshot = try await repository.loadHistory(gameID: gameID, sessionID: sessionID)
        let session = TranslationSession(
            gameID: gameID,
            sessionID: sessionID,
            configuration: configuration,
            settings: settings,
            service: service,
            repository: repository,
            cache: cache,
            sentenceCache: sentenceCache,
            providerDisplayName: providerDisplayName,
            initialEntries: snapshot?.entries ?? []
        )
        let restoredEntries = await session.history()
        if restoredEntries != (snapshot?.entries ?? []) { try await session.persistHistory() }
        return session
    }

    public func submit(_ source: FinalizedTranslationSource, retainActiveCount: Int = 1) async throws -> UUID {
        try settings.validate()
        guard !gameID.isEmpty, gameID.utf8.count <= 256,
              !source.text.isEmpty, source.text.utf8.count <= 1_048_576,
              source.revision >= 0, source.generation >= 0,
              !source.streamID.isEmpty, source.streamID.utf8.count <= 256,
              (source.speaker?.utf8.count ?? 0) <= 1_024
        else {
            throw TranslationError.invalidConfiguration("Source text is empty or exceeds 1 MiB")
        }
        let version = SourceVersion(generation: source.generation, revision: source.revision)
        if let previousVersion = latestSourceVersions[source.eventID], version < previousVersion {
            throw TranslationError.staleSource
        }
        if let previous = lastDeduplicated,
           previous.streamID == source.streamID,
           previous.generation == source.generation,
           previous.revision == source.revision,
           previous.text == source.text,
           previous.speaker == source.speaker,
           source.capturedAt.timeIntervalSince(previous.time) >= 0,
           source.capturedAt.timeIntervalSince(previous.time) <= 0.35 {
            recordSourceVersion(version, eventID: source.eventID)
            return previous.entryID
        }

        let maxConcurrent = max(1, min(10, retainActiveCount))
        advanceRequestEpoch()
        let epoch = requestEpoch
        evictActiveResponses(keepingAtMost: maxConcurrent - 1)
        let entry = TranslationHistoryEntry(source: source)
        entries.append(entry)
        activeEntryID = entry.id
        requestEpochByEntry[entry.id] = epoch
        activeTaskOrder.append(entry.id)
        recordSourceVersion(version, eventID: source.eventID)
        lastDeduplicated = (
            source.streamID,
            source.generation,
            source.revision,
            source.text,
            source.speaker,
            source.capturedAt,
            entry.id
        )
        trimHistory()
        let request = try makeRequest(for: entry)
        let key = TranslationCacheKey(configuration: configuration, request: request)
        let service = self.service
        let configuration = self.configuration
        try await persistHistory()
        guard isEntryRequestValid(epoch, entryID: entry.id) else { throw CancellationError() }

        if let cached = await cache.value(for: key) {
            guard isEntryRequestValid(epoch, entryID: entry.id) else { throw CancellationError() }
            updateEntry(entry.id) { $0.translatedText = cached
                $0.status = .completed
            }
            finishActiveTracking(entryID: entry.id)
            try await persistHistory()
            guard isEntryRetained(entryID: entry.id) else { throw CancellationError() }
            return entry.id
        }
        guard isEntryRequestValid(epoch, entryID: entry.id) else { throw CancellationError() }

        if settings.sentenceCacheEnabled,
           let sentenceCache,
           let persistentHit = try? await sentenceCache.lookup(
               gameID: gameID,
               sourceText: source.text,
               speaker: source.speaker,
               sourceLanguage: request.sourceLanguage,
               targetLanguage: request.targetLanguage,
               providerKind: configuration.kind,
               modelID: configuration.modelID ?? "",
               customPrompt: request.customPrompt
           ) {
            guard isEntryRequestValid(epoch, entryID: entry.id) else { throw CancellationError() }
            updateEntry(entry.id) {
                $0.translatedText = persistentHit.translatedText
                $0.status = .completed
                $0.failureDescription = nil
            }
            await cache.insert(persistentHit.translatedText, for: key)
            finishActiveTracking(entryID: entry.id)
            try await persistHistory()
            guard isEntryRetained(entryID: entry.id) else { throw CancellationError() }
            return entry.id
        }
        guard isEntryRequestValid(epoch, entryID: entry.id) else { throw CancellationError() }

        activeTasks[entry.id] = Task { [weak self] in
            do {
                for try await event in service.translate(request, using: configuration) {
                    await self?.accept(event, entryID: entry.id, requestEpoch: epoch, cacheKey: key)
                }
            } catch is CancellationError {
                await self?.markCancelled(entryID: entry.id, requestEpoch: epoch)
            } catch {
                await self?.markFailed(entryID: entry.id, requestEpoch: epoch, error: error)
            }
        }
        return entry.id
    }

    public func history() -> [TranslationHistoryEntry] { entries }

    public func entries(for ids: Set<UUID>) -> [TranslationHistoryEntry] {
        guard !ids.isEmpty else { return [] }
        return entries.filter { ids.contains($0.id) }
    }

    public func activeEntry() -> TranslationHistoryEntry? {
        guard let activeEntryID else { return nil }
        return entries.first { $0.id == activeEntryID }
    }

    public func setIncludedInContext(_ include: Bool, entryID: UUID) async throws {
        updateEntry(entryID) { $0.includeInContext = include }
        try await persistHistory()
    }

    public func updateSettings(_ settings: TranslationSettings) async throws {
        try settings.validate()
        guard settings != self.settings else { return }
        lastDeduplicated = nil
        advanceRequestEpoch()
        let epoch = requestEpoch
        cancelCurrentResponse(markCancelled: true, invalidateActiveRequest: true)
        self.settings = settings
        trimHistory()
        let repository = self.repository
        try await enqueuePersistence { try await repository.saveSettings(settings) }
        guard requestEpoch == epoch else { return }
        try await persistHistory()
    }

    public func updateConfiguration(_ configuration: ProviderConfiguration) async throws {
        try configuration.validate()
        guard configuration != self.configuration else { return }
        lastDeduplicated = nil
        advanceRequestEpoch()
        let epoch = requestEpoch
        cancelCurrentResponse(markCancelled: true, invalidateActiveRequest: true)
        self.configuration = configuration
        let repository = self.repository
        try await enqueuePersistence { try await repository.saveConfiguration(configuration) }
        guard requestEpoch == epoch else { return }
    }

    public func applyDiscoveredModels(_ models: [ModelDescriptor]) async throws {
        try await updateConfiguration(configuration.withDiscoveredModels(models))
    }

    public func currentConfiguration() -> ProviderConfiguration { configuration }
    public func currentSettings() -> TranslationSettings { settings }

    public func cancelActive() async throws {
        lastDeduplicated = nil
        advanceRequestEpoch()
        cancelCurrentResponse(markCancelled: true, invalidateActiveRequest: true)
        try await persistHistory()
    }

    public func clearHistory(clearCache: Bool = true) async throws {
        advanceRequestEpoch()
        cancelCurrentResponse(markCancelled: false, invalidateActiveRequest: true)
        entries.removeAll()
        activeEntryID = nil
        lastDeduplicated = nil
        requestEpochByEntry.removeAll()
        latestSourceVersions.removeAll()
        sourceVersionOrder.removeAll()
        let cache = self.cache
        let gameID = self.gameID
        let cacheClearTask = clearCache ? Task { await cache.removeAll(gameID: gameID) } : nil
        let repository = self.repository
        let sessionID = self.sessionID
        do {
            try await enqueuePersistence { try await repository.clearHistory(gameID: gameID, sessionID: sessionID) }
        } catch {
            await cacheClearTask?.value
            throw error
        }
        await cacheClearTask?.value
    }

    public func clearCache() async {
        await cache.removeAll(gameID: gameID)
    }

    private func makeRequest(for entry: TranslationHistoryEntry) throws -> TranslationRequest {
        let previous = entries.filter { $0.id != entry.id && $0.includeInContext && $0.status == .completed }
        let selected = Array(previous.suffix(settings.contextTurnLimit)).map { item in
            TranslationContextTurn(
                sourceText: settings.includeSourceHistory ? item.source.text : nil,
                translatedText: settings.includeTranslatedHistory ? item.translatedText : nil,
                speaker: item.source.speaker
            )
        }.filter { $0.sourceText != nil || $0.translatedText != nil }
        let languageSource = settings.sourceLanguage
        let languageTarget = settings.targetLanguage
        let model = configuration.effectiveModelDescriptor()
        let contextLimit = settings.effectiveContextWindow(for: model)
        return TranslationRequest(
            gameID: gameID,
            sessionID: sessionID,
            eventID: entry.source.eventID,
            generation: entry.source.generation,
            sourceText: entry.source.text,
            speaker: entry.source.speaker,
            sourceLanguage: languageSource,
            targetLanguage: languageTarget,
            context: selected,
            customPrompt: settings.customPrompt,
            outputTokenLimit: settings.outputTokenLimit,
            contextWindowTokens: contextLimit.tokens,
            contextWindowProvenance: contextLimit.provenance
        )
    }

    private func accept(
        _ event: TranslationEvent,
        entryID: UUID,
        requestEpoch eventEpoch: UInt64,
        cacheKey: TranslationCacheKey
    ) async {
        guard requestEpochByEntry[entryID] == eventEpoch, entries.contains(where: { $0.id == entryID }) else { return }
        switch event.kind {
        case .delta:
            if let text = event.text { updateEntry(entryID) { $0.translatedText = ($0.translatedText ?? "") + text } }
            return // Persist the pending source and terminal result, not every token.
        case .completed:
            if let text = event.text {
                updateEntry(entryID) { $0.translatedText = text
                    $0.status = .completed
                    $0.failureDescription = nil
                }
                await cache.insert(text, for: cacheKey)
                if settings.sentenceCacheEnabled,
                   !text.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty,
                   let sentenceCache,
                   let completedEntry = entries.first(where: { $0.id == entryID }) {
                    let effectiveProviderName = providerDisplayName.isEmpty ? configuration.name : providerDisplayName
                    _ = try? await sentenceCache.store(
                        gameID: gameID,
                        sourceText: completedEntry.source.text,
                        translatedText: text,
                        speaker: completedEntry.source.speaker,
                        sourceLanguage: cacheKey.sourceLanguage,
                        targetLanguage: cacheKey.targetLanguage,
                        providerName: effectiveProviderName,
                        providerKind: cacheKey.providerKind,
                        modelID: cacheKey.modelID ?? "",
                        customPrompt: cacheKey.customPrompt
                    )
                }
                guard requestEpochByEntry[entryID] == eventEpoch,
                      entries.contains(where: { $0.id == entryID })
                else { return }
                finishActiveTracking(entryID: entryID)
            }
        case .usage: return
        }
        try? await persistHistory()
    }

    private func markFailed(entryID: UUID, requestEpoch eventEpoch: UInt64, error: any Error) async {
        guard requestEpochByEntry[entryID] == eventEpoch, entries.contains(where: { $0.id == entryID }) else { return }
        updateEntry(entryID) { $0.status = .failed
            $0.failureDescription = Self.safeFailureDescription(error)
        }
        finishActiveTracking(entryID: entryID)
        try? await persistHistory()
    }

    private func markCancelled(entryID: UUID, requestEpoch eventEpoch: UInt64) async {
        guard requestEpochByEntry[entryID] == eventEpoch, entries.contains(where: { $0.id == entryID }) else { return }
        updateEntry(entryID) { if $0.status == .pending { $0.status = .cancelled } }
        finishActiveTracking(entryID: entryID)
        try? await persistHistory()
    }

    private func evictActiveResponses(keepingAtMost limit: Int) {
        while activeTaskOrder.count > limit {
            let oldID = activeTaskOrder.removeFirst()
            activeTasks.removeValue(forKey: oldID)?.cancel()
            updateEntry(oldID) { if $0.status == .pending { $0.status = .cancelled } }
            requestEpochByEntry.removeValue(forKey: oldID)
        }
    }

    private func cancelCurrentResponse(markCancelled: Bool, invalidateActiveRequest: Bool) {
        let trackedIDs = Set(activeTaskOrder).union(activeTasks.keys)
        for task in activeTasks.values {
            task.cancel()
        }
        activeTasks.removeAll()
        activeTaskOrder.removeAll()
        for id in trackedIDs {
            if markCancelled { updateEntry(id) { if $0.status == .pending { $0.status = .cancelled } } }
            if invalidateActiveRequest { requestEpochByEntry.removeValue(forKey: id) }
        }
        if let activeEntryID {
            if markCancelled { updateEntry(activeEntryID) { if $0.status == .pending { $0.status = .cancelled } } }
            if invalidateActiveRequest { requestEpochByEntry.removeValue(forKey: activeEntryID) }
        }
    }

    private func finishActiveTracking(entryID: UUID) {
        requestEpochByEntry.removeValue(forKey: entryID)
        activeTasks.removeValue(forKey: entryID)
        activeTaskOrder.removeAll { $0 == entryID }
    }

    private func advanceRequestEpoch() { requestEpoch &+= 1 }

    private func isEntryRequestValid(_ epoch: UInt64, entryID: UUID) -> Bool {
        requestEpochByEntry[entryID] == epoch && entries.contains(where: { $0.id == entryID })
    }

    private func isEntryRetained(entryID: UUID) -> Bool {
        entries.contains(where: { $0.id == entryID })
    }

    private func recordSourceVersion(_ version: SourceVersion, eventID: UUID) {
        if latestSourceVersions[eventID] == nil { sourceVersionOrder.append(eventID) }
        latestSourceVersions[eventID] = version
        while sourceVersionOrder.count > 4_096 {
            latestSourceVersions.removeValue(forKey: sourceVersionOrder.removeFirst())
        }
    }

    private func updateEntry(_ id: UUID, update: (inout TranslationHistoryEntry) -> Void) {
        guard let index = entries.firstIndex(where: { $0.id == id }) else { return }
        update(&entries[index])
    }

    private func trimHistory() {
        let count = settings.historyRetentionTurns
        if count == 0 {
            entries = entries.filter { $0.id == activeEntryID }
        } else if entries.count > count {
            entries.removeFirst(entries.count - count)
        }
        let retained = Set(entries.map(\.id))
        requestEpochByEntry = requestEpochByEntry.filter { retained.contains($0.key) }
        for (id, task) in activeTasks where !retained.contains(id) {
            task.cancel()
            activeTasks.removeValue(forKey: id)
        }
        activeTaskOrder.removeAll { !retained.contains($0) }
    }

    private func persistHistory() async throws {
        let repository = self.repository
        guard settings.historyRetentionTurns > 0 else {
            let gameID = self.gameID
            let sessionID = self.sessionID
            try await enqueuePersistence { try await repository.clearHistory(gameID: gameID, sessionID: sessionID) }
            return
        }
        let snapshot = TranslationSessionSnapshot(gameID: gameID, sessionID: sessionID, entries: entries)
        try await enqueuePersistence { try await repository.saveHistory(snapshot) }
    }

    private func enqueuePersistence(_ operation: @escaping @Sendable () async throws -> Void) async throws {
        let previous = persistenceTail
        let next = Task<Void, any Error> {
            _ = try? await previous?.value
            try await operation()
        }
        persistenceTail = next
        try await next.value
    }

    private static func safeFailureDescription(_ error: any Error) -> String {
        if let translationError = error as? TranslationError { return translationError.localizedDescription }
        if error is CancellationError { return "Translation cancelled" }
        return "Translation request failed"
    }
}

private struct SourceVersion: Comparable {
    let generation: Int
    let revision: Int

    static func < (lhs: SourceVersion, rhs: SourceVersion) -> Bool {
        if lhs.generation != rhs.generation { return lhs.generation < rhs.generation }
        return lhs.revision < rhs.revision
    }
}
