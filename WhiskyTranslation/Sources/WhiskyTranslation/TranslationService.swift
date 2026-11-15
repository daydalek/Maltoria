//
//  TranslationService.swift
//
//  This file is part of Maltoria, licensed under GPL-3.0-or-later.
//

import Foundation

public struct TranslationService: Sendable {
    private let providers: [ProviderKind: any TranslationProvider]
    private let credentialStore: any CredentialStore

    public init(
        transport: any HTTPTransport = URLSessionStreamTransport(),
        credentialStore: any CredentialStore = TranslationService.platformCredentialStore(),
        retryClock: any RetryClock = SystemRetryClock()
    ) {
        providers = Dictionary(uniqueKeysWithValues: ProviderKind.allCases.map { kind in
            (kind, HTTPTranslationProvider(kind: kind, transport: transport, retryClock: retryClock))
        })
        self.credentialStore = credentialStore
    }

    public init(providers: [ProviderKind: any TranslationProvider], credentialStore: any CredentialStore) {
        self.providers = providers
        self.credentialStore = credentialStore
    }

    public static func platformCredentialStore() -> any CredentialStore {
        #if canImport(Security)
        KeychainCredentialStore()
        #else
        SessionOnlyCredentialStore()
        #endif
    }

    public func credentialAvailability() async -> CredentialStoreAvailability {
        await credentialStore.availability()
    }

    public func discoverModels(for configuration: ProviderConfiguration) async throws -> [ModelDescriptor] {
        let provider = try provider(for: configuration)
        let credential = try await credential(for: configuration)
        return try await provider.discoverModels(configuration: configuration, credential: credential)
    }

    public func modelInfo(id: String, for configuration: ProviderConfiguration) async throws -> ModelDescriptor {
        let provider = try provider(for: configuration)
        let credential = try await credential(for: configuration)
        return try await provider.modelInfo(id: id, configuration: configuration, credential: credential)
    }

    public func translate(
        _ request: TranslationRequest,
        using configuration: ProviderConfiguration
    ) -> AsyncThrowingStream<TranslationEvent, any Error> {
        serviceEventStream { continuation in
            try request.validate()
            if !configuration.capabilities.customPrompt, !request.customPrompt.isEmpty {
                throw TranslationError.unsupportedCapability("custom prompt")
            }
            let provider = try self.provider(for: configuration)
            let credential = try await self.credential(for: configuration)
            let fitted = try TranslationPrompt.fitting(request, kind: configuration.kind)
            let stream = provider.translate(fitted, configuration: configuration, credential: credential)
            for try await event in stream { try yieldServiceEvent(event, to: continuation) }
        }
    }

    public func testConnection(
        configuration: ProviderConfiguration,
        sourceText: String = "Hello",
        sourceLanguage: String = "en",
        targetLanguage: String = "ja"
    ) async throws -> String {
        let request = TranslationRequest(
            gameID: "provider-test",
            sessionID: UUID(),
            eventID: UUID(),
            generation: 0,
            sourceText: sourceText,
            speaker: nil,
            sourceLanguage: sourceLanguage,
            targetLanguage: targetLanguage,
            context: [],
            customPrompt: "",
            outputTokenLimit: 32,
            contextWindowTokens: 4_096,
            contextWindowProvenance: .unknown
        )
        var completed: String?
        for try await event in translate(request, using: configuration) where event.kind == .completed {
            completed = event.text
        }
        guard let completed else { throw TranslationError.emptyCompletion }
        return completed
    }

    private func provider(for configuration: ProviderConfiguration) throws -> any TranslationProvider {
        try configuration.validate()
        guard let provider = providers[configuration.kind] else { throw TranslationError.unsupportedCapability("provider") }
        return provider
    }

    private func credential(for configuration: ProviderConfiguration) async throws -> String? {
        if let reference = configuration.credentialReference {
            guard let secret = try await credentialStore.secret(for: reference), !secret.isEmpty else {
                throw TranslationError.missingCredential
            }
            return secret
        }
        guard !configuration.kind.requiresCredential(at: configuration.baseURL) else { throw TranslationError.missingCredential }
        return nil
    }
}

private func serviceEventStream(
    _ operation: @escaping @Sendable (AsyncThrowingStream<TranslationEvent, any Error>.Continuation) async throws -> Void
) -> AsyncThrowingStream<TranslationEvent, any Error> {
    let (stream, continuation) = AsyncThrowingStream<TranslationEvent, any Error>.makeStream(bufferingPolicy: .bufferingOldest(256))
    let task = Task {
        do {
            try await operation(continuation)
            continuation.finish()
        } catch {
            continuation.finish(throwing: error)
        }
    }
    continuation.onTermination = { termination in
        if case .cancelled = termination { task.cancel() }
    }
    return stream
}

private func yieldServiceEvent(
    _ event: TranslationEvent,
    to continuation: AsyncThrowingStream<TranslationEvent, any Error>.Continuation
) throws {
    switch continuation.yield(event) {
    case .enqueued: break
    case .dropped: throw TranslationError.streamConsumerTooSlow
    case .terminated: throw CancellationError()
    @unknown default: throw TranslationError.streamConsumerTooSlow
    }
}
