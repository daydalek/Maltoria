//
//  Models.swift
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

public enum ProviderKind: String, Codable, CaseIterable, Hashable, Sendable {
    case openAIChat
    case openAICompletions
    case ollama
    case deepL
    case anthropic
    case gemini
    case localBuiltIn

    public var supportsModelDiscovery: Bool { self != .deepL }

    public var defaultBaseURL: URL {
        let value = switch self {
        case .openAIChat, .openAICompletions:
            "https://api.openai.com/v1"
        case .ollama:
            "http://localhost:11434"
        case .deepL:
            "https://api.deepl.com"
        case .anthropic:
            "https://api.anthropic.com/v1"
        case .gemini:
            "https://generativelanguage.googleapis.com/v1beta"
        case .localBuiltIn:
            "http://127.0.0.1:18080"
        }
        return URL(string: value)!
    }

    public var capabilities: ProviderCapabilities {
        switch self {
        case .openAIChat:
            ProviderCapabilities(
                modelDiscovery: true,
                contextDiscovery: false,
                streaming: true,
                customPrompt: true,
                context: true
            )
        case .openAICompletions:
            ProviderCapabilities(
                modelDiscovery: true,
                contextDiscovery: false,
                streaming: true,
                customPrompt: true,
                context: true
            )
        case .ollama:
            ProviderCapabilities(
                modelDiscovery: true,
                contextDiscovery: true,
                streaming: true,
                customPrompt: true,
                context: true
            )
        case .deepL:
            ProviderCapabilities(
                modelDiscovery: false,
                contextDiscovery: false,
                streaming: false,
                customPrompt: false,
                context: true
            )
        case .anthropic:
            ProviderCapabilities(
                modelDiscovery: true,
                contextDiscovery: true,
                streaming: true,
                customPrompt: true,
                context: true
            )
        case .gemini:
            ProviderCapabilities(
                modelDiscovery: true,
                contextDiscovery: true,
                streaming: true,
                customPrompt: true,
                context: true
            )
        case .localBuiltIn:
            ProviderCapabilities(
                modelDiscovery: true,
                contextDiscovery: true,
                streaming: true,
                customPrompt: true,
                context: true
            )
        }
    }

    public func requiresCredential(at baseURL: URL) -> Bool {
        switch self {
        case .deepL, .anthropic, .gemini: return true
        case .ollama, .localBuiltIn: return false
        case .openAIChat, .openAICompletions:
            guard let host = URLComponents(url: baseURL, resolvingAgainstBaseURL: false)?.host?.lowercased()
            else { return true }
            return !ProviderConfiguration.isLoopback(host)
        }
    }
}

public enum OpenAIChatTokenLimitField: String, Codable, CaseIterable, Sendable {
    case maxCompletionTokens = "max_completion_tokens"
    case maxTokens = "max_tokens"
}

public enum ThinkingLevel: String, Codable, CaseIterable, Hashable, Sendable {
    case `default`
    case off
    case low
    case medium
    case high
    case xhigh
    case max
    case ultra
}

public struct ProviderCapabilities: Codable, Equatable, Sendable {
    public let modelDiscovery: Bool
    public let contextDiscovery: Bool
    public let streaming: Bool
    public let customPrompt: Bool
    public let context: Bool

    public init(
        modelDiscovery: Bool,
        contextDiscovery: Bool,
        streaming: Bool,
        customPrompt: Bool,
        context: Bool
    ) {
        self.modelDiscovery = modelDiscovery
        self.contextDiscovery = contextDiscovery
        self.streaming = streaming
        self.customPrompt = customPrompt
        self.context = context
    }
}

public enum ContextWindowProvenance: String, Codable, Equatable, Sendable {
    case detected
    case manual
    case unknown
}

public enum ModelCapability: String, Codable, Hashable, Sendable {
    case generateContent
    case chatCompletion
    case textCompletion
    case streaming
    case tools
}

public struct ModelDescriptor: Codable, Equatable, Identifiable, Sendable {
    public let id: String
    public var displayName: String
    public var contextWindow: Int?
    public var contextWindowProvenance: ContextWindowProvenance
    public var capabilities: Set<ModelCapability>

    public init(
        id: String,
        displayName: String? = nil,
        contextWindow: Int? = nil,
        contextWindowProvenance: ContextWindowProvenance? = nil,
        capabilities: Set<ModelCapability> = []
    ) {
        self.id = id
        self.displayName = displayName ?? id
        self.contextWindow = contextWindow.flatMap { $0 > 0 ? $0 : nil }
        self.contextWindowProvenance = contextWindowProvenance
            ?? (contextWindow == nil ? .unknown : .detected)
        if self.contextWindow == nil {
            self.contextWindowProvenance = .unknown
        }
        self.capabilities = capabilities
    }
}

public struct ProviderConfiguration: Codable, Equatable, Identifiable, Sendable {
    public var id: UUID
    public var name: String
    public var kind: ProviderKind
    public var baseURL: URL
    public var modelID: String?
    public var credentialReference: String?
    public var defaultSourceLanguage: String
    public var defaultTargetLanguage: String
    public var modelMetadata: ModelDescriptor?
    public var openAIChatTokenLimitField: OpenAIChatTokenLimitField
    public var thinkingLevel: ThinkingLevel

    public var capabilities: ProviderCapabilities { kind.capabilities }

    public init(
        id: UUID = UUID(),
        name: String? = nil,
        kind: ProviderKind,
        baseURL: URL? = nil,
        modelID: String? = nil,
        credentialReference: String? = nil,
        defaultSourceLanguage: String = "auto",
        defaultTargetLanguage: String = "en",
        modelMetadata: ModelDescriptor? = nil,
        openAIChatTokenLimitField: OpenAIChatTokenLimitField = .maxCompletionTokens,
        thinkingLevel: ThinkingLevel = .default
    ) {
        self.id = id
        self.name = name ?? Self.defaultName(for: kind)
        self.kind = kind
        self.baseURL = baseURL ?? kind.defaultBaseURL
        self.modelID = modelID
        self.credentialReference = credentialReference
        self.defaultSourceLanguage = defaultSourceLanguage
        self.defaultTargetLanguage = defaultTargetLanguage
        self.modelMetadata = modelMetadata
        self.openAIChatTokenLimitField = openAIChatTokenLimitField
        self.thinkingLevel = thinkingLevel
    }

    private enum CodingKeys: String, CodingKey {
        case id, name, kind, baseURL, modelID, credentialReference, defaultSourceLanguage, defaultTargetLanguage,
             modelMetadata, openAIChatTokenLimitField, thinkingLevel
    }

    public init(from decoder: Decoder) throws {
        let values = try decoder.container(keyedBy: CodingKeys.self)
        id = try values.decode(UUID.self, forKey: .id)
        kind = try values.decode(ProviderKind.self, forKey: .kind)
        name = try values.decodeIfPresent(String.self, forKey: .name) ?? Self.defaultName(for: kind)
        baseURL = try values.decode(URL.self, forKey: .baseURL)
        modelID = try values.decodeIfPresent(String.self, forKey: .modelID)
        credentialReference = try values.decodeIfPresent(String.self, forKey: .credentialReference)
        defaultSourceLanguage = try values.decodeIfPresent(String.self, forKey: .defaultSourceLanguage) ?? "auto"
        defaultTargetLanguage = try values.decodeIfPresent(String.self, forKey: .defaultTargetLanguage) ?? "en"
        modelMetadata = try values.decodeIfPresent(ModelDescriptor.self, forKey: .modelMetadata)
        openAIChatTokenLimitField = try values.decodeIfPresent(
            OpenAIChatTokenLimitField.self,
            forKey: .openAIChatTokenLimitField
        ) ?? .maxCompletionTokens
        thinkingLevel = try values.decodeIfPresent(
            ThinkingLevel.self,
            forKey: .thinkingLevel
        ) ?? .default
    }

    public func validate() throws {
        guard !name.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty, name.utf8.count <= 128 else {
            throw TranslationError.invalidConfiguration("Provider name is empty or too long")
        }
        guard let components = URLComponents(url: baseURL, resolvingAgainstBaseURL: false),
              let scheme = components.scheme?.lowercased(),
              let host = components.host?.lowercased(), !host.isEmpty,
              components.user == nil, components.password == nil,
              components.query == nil, components.fragment == nil
        else {
            throw TranslationError.invalidConfiguration("Provider URL must not contain credentials, query, or fragment")
        }
        guard scheme == "https" || (scheme == "http" && Self.isLoopback(host)) else {
            throw TranslationError.insecureEndpoint
        }
        guard baseURL.absoluteString.utf8.count <= 2_048 else {
            throw TranslationError.invalidConfiguration("Provider URL is too long")
        }
        if let modelID, modelID.isEmpty || modelID.utf8.count > 512 {
            throw TranslationError.invalidConfiguration("Model ID is empty or too long")
        }
        if let credentialReference, credentialReference.isEmpty || credentialReference.utf8.count > 256 {
            throw TranslationError.invalidConfiguration("Credential reference is invalid")
        }
        try Self.validateLanguage(defaultSourceLanguage, allowAuto: true)
        try Self.validateLanguage(defaultTargetLanguage, allowAuto: false)
    }

    public func withDiscoveredModels(_ models: [ModelDescriptor]) -> ProviderConfiguration {
        var copy = self
        guard let modelID, let selected = models.first(where: { $0.id == modelID }) else { return copy }
        copy.modelMetadata = selected
        return copy
    }

    public func effectiveModelDescriptor() -> ModelDescriptor? {
        guard let modelID else { return nil }
        if let modelMetadata, modelMetadata.id == modelID { return modelMetadata }
        return ModelDescriptor(id: modelID, contextWindowProvenance: .unknown)
    }

    private static func defaultName(for kind: ProviderKind) -> String {
        switch kind {
        case .openAIChat: "OpenAI-compatible Chat Completions"
        case .openAICompletions: "OpenAI-compatible Legacy Completions"
        case .ollama: "Ollama"
        case .deepL: "DeepL"
        case .anthropic: "Anthropic Messages"
        case .gemini: "Gemini"
        case .localBuiltIn: "Built-in Local Model"
        }
    }

    fileprivate static func isLoopback(_ host: String) -> Bool {
        if host == "localhost" || host.hasSuffix(".localhost") || host == "::1" { return true }
        let components = host.split(separator: ".")
        return components.count == 4 && components.first == "127"
            && components.allSatisfy { Int($0).map { (0 ... 255).contains($0) } == true }
    }

    fileprivate static func validateLanguage(_ value: String, allowAuto: Bool) throws {
        let language = value.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !language.isEmpty, language.utf8.count <= 64,
              allowAuto || language.lowercased() != "auto"
        else {
            throw TranslationError.invalidConfiguration("Language setting is invalid")
        }
    }
}

public enum CredentialStorageMode: String, Codable, CaseIterable, Sendable {
    case keychain
    case localFile
    case sessionOnly
}

public struct TranslationSettings: Codable, Equatable, Sendable {
    public var contextWindowOverride: Int?
    public var outputTokenLimit: Int
    public var historyRetentionTurns: Int
    public var contextTurnLimit: Int
    public var includeSourceHistory: Bool
    public var includeTranslatedHistory: Bool
    public var customPrompt: String
    public var sourceLanguage: String
    public var targetLanguage: String
    public var credentialStorageMode: CredentialStorageMode
    public var translationDebounceMs: Int
    public var sentenceCacheEnabled: Bool

    public init(
        contextWindowOverride: Int? = nil,
        outputTokenLimit: Int = 512,
        historyRetentionTurns: Int = 20,
        contextTurnLimit: Int = 8,
        includeSourceHistory: Bool = true,
        includeTranslatedHistory: Bool = true,
        customPrompt: String = "",
        sourceLanguage: String = "auto",
        targetLanguage: String = "en",
        credentialStorageMode: CredentialStorageMode = .keychain,
        translationDebounceMs: Int = 150,
        sentenceCacheEnabled: Bool = true
    ) {
        self.contextWindowOverride = contextWindowOverride
        self.outputTokenLimit = outputTokenLimit
        self.historyRetentionTurns = historyRetentionTurns
        self.contextTurnLimit = contextTurnLimit
        self.includeSourceHistory = includeSourceHistory
        self.includeTranslatedHistory = includeTranslatedHistory
        self.customPrompt = customPrompt
        self.sourceLanguage = sourceLanguage
        self.targetLanguage = targetLanguage
        self.credentialStorageMode = credentialStorageMode
        self.translationDebounceMs = translationDebounceMs
        self.sentenceCacheEnabled = sentenceCacheEnabled
    }

    private enum CodingKeys: String, CodingKey {
        case contextWindowOverride, outputTokenLimit, historyRetentionTurns, contextTurnLimit,
             includeSourceHistory, includeTranslatedHistory, customPrompt, sourceLanguage, targetLanguage,
             credentialStorageMode, translationDebounceMs, sentenceCacheEnabled
    }

    public init(from decoder: Decoder) throws {
        let values = try decoder.container(keyedBy: CodingKeys.self)
        contextWindowOverride = try values.decodeIfPresent(Int.self, forKey: .contextWindowOverride)
        outputTokenLimit = try values.decodeIfPresent(Int.self, forKey: .outputTokenLimit) ?? 512
        historyRetentionTurns = try values.decodeIfPresent(Int.self, forKey: .historyRetentionTurns) ?? 20
        contextTurnLimit = try values.decodeIfPresent(Int.self, forKey: .contextTurnLimit) ?? 8
        includeSourceHistory = try values.decodeIfPresent(Bool.self, forKey: .includeSourceHistory) ?? true
        includeTranslatedHistory = try values.decodeIfPresent(Bool.self, forKey: .includeTranslatedHistory) ?? true
        customPrompt = try values.decodeIfPresent(String.self, forKey: .customPrompt) ?? ""
        sourceLanguage = try values.decodeIfPresent(String.self, forKey: .sourceLanguage) ?? "auto"
        targetLanguage = try values.decodeIfPresent(String.self, forKey: .targetLanguage) ?? "en"
        credentialStorageMode = try values.decodeIfPresent(
            CredentialStorageMode.self,
            forKey: .credentialStorageMode
        ) ?? .keychain
        translationDebounceMs = try values.decodeIfPresent(Int.self, forKey: .translationDebounceMs) ?? 150
        sentenceCacheEnabled = try values.decodeIfPresent(Bool.self, forKey: .sentenceCacheEnabled) ?? true
    }

    public static func defaults(for configuration: ProviderConfiguration) -> TranslationSettings {
        TranslationSettings(
            sourceLanguage: configuration.defaultSourceLanguage,
            targetLanguage: configuration.defaultTargetLanguage
        )
    }

    public func validate() throws {
        if let contextWindowOverride, !(1 ... 1_000_000).contains(contextWindowOverride) {
            throw TranslationError.invalidConfiguration("Context-window override must be between 1 and 1000000")
        }
        guard (1 ... 32_768).contains(outputTokenLimit) else {
            throw TranslationError.invalidConfiguration("Output token limit must be between 1 and 32768")
        }
        guard (0 ... 1_000).contains(historyRetentionTurns), (0 ... 1_000).contains(contextTurnLimit) else {
            throw TranslationError.invalidConfiguration("History limits must be between 0 and 1000 turns")
        }
        guard (0 ... 5_000).contains(translationDebounceMs) else {
            throw TranslationError.invalidConfiguration("Translation debounce delay must be between 0 and 5000 ms")
        }
        guard customPrompt.utf8.count <= 16_384 else {
            throw TranslationError.invalidConfiguration("Custom prompt is too long")
        }
        try ProviderConfiguration.validateLanguage(sourceLanguage, allowAuto: true)
        try ProviderConfiguration.validateLanguage(targetLanguage, allowAuto: false)
    }

    public func effectiveContextWindow(for model: ModelDescriptor?)
        -> (tokens: Int, provenance: ContextWindowProvenance) {
        if let contextWindowOverride { return (contextWindowOverride, .manual) }
        if let window = model?.contextWindow,
           let provenance = model?.contextWindowProvenance,
           provenance == .detected || provenance == .manual {
            return (window, provenance)
        }
        return (4_096, .unknown)
    }

    public func contextWindowLabel(for model: ModelDescriptor?) -> String {
        if contextWindowOverride != nil { return "Manual context-window override" }
        if model?.contextWindowProvenance == .detected { return "Detected provider context window" }
        if model?.contextWindowProvenance == .manual { return "Manual model context window" }
        return "Estimated conservative fallback (4096)"
    }
}

public struct TranslationContextTurn: Codable, Equatable, Sendable {
    public let sourceText: String?
    public let translatedText: String?
    public let speaker: String?

    public init(sourceText: String?, translatedText: String?, speaker: String? = nil) {
        self.sourceText = sourceText
        self.translatedText = translatedText
        self.speaker = speaker
    }
}

public struct TranslationRequest: Codable, Equatable, Sendable {
    public let requestID: UUID
    public let gameID: String
    public let sessionID: UUID
    public let eventID: UUID
    public let generation: Int
    public let sourceText: String
    public let speaker: String?
    public let sourceLanguage: String
    public let targetLanguage: String
    public let context: [TranslationContextTurn]
    public let customPrompt: String
    public let outputTokenLimit: Int
    public let contextWindowTokens: Int
    public let contextWindowProvenance: ContextWindowProvenance

    public init(
        requestID: UUID = UUID(),
        gameID: String,
        sessionID: UUID,
        eventID: UUID,
        generation: Int,
        sourceText: String,
        speaker: String?,
        sourceLanguage: String,
        targetLanguage: String,
        context: [TranslationContextTurn],
        customPrompt: String,
        outputTokenLimit: Int,
        contextWindowTokens: Int,
        contextWindowProvenance: ContextWindowProvenance
    ) {
        self.requestID = requestID
        self.gameID = gameID
        self.sessionID = sessionID
        self.eventID = eventID
        self.generation = generation
        self.sourceText = sourceText
        self.speaker = speaker
        self.sourceLanguage = sourceLanguage
        self.targetLanguage = targetLanguage
        self.context = context
        self.customPrompt = customPrompt
        self.outputTokenLimit = outputTokenLimit
        self.contextWindowTokens = contextWindowTokens
        self.contextWindowProvenance = contextWindowProvenance
    }

    public func validate() throws {
        guard !gameID.isEmpty, gameID.utf8.count <= 256,
              !sourceText.isEmpty, sourceText.utf8.count <= 1_048_576,
              generation >= 0,
              (1 ... 32_768).contains(outputTokenLimit),
              (1 ... 1_000_000).contains(contextWindowTokens),
              context.count <= 1_000,
              customPrompt.utf8.count <= 16_384,
              (speaker?.utf8.count ?? 0) <= 1_024
        else {
            throw TranslationError.invalidConfiguration("Translation request fields are outside supported limits")
        }
        try ProviderConfiguration.validateLanguage(sourceLanguage, allowAuto: true)
        try ProviderConfiguration.validateLanguage(targetLanguage, allowAuto: false)
        guard context.allSatisfy({
            ($0.sourceText?.utf8.count ?? 0) <= 1_048_576
                && ($0.translatedText?.utf8.count ?? 0) <= 1_048_576
                && ($0.speaker?.utf8.count ?? 0) <= 1_024
        })
        else {
            throw TranslationError.invalidConfiguration("Translation context exceeds supported limits")
        }
    }
}

public struct TranslationUsage: Codable, Equatable, Sendable {
    public let inputTokens: Int?
    public let outputTokens: Int?

    public init(inputTokens: Int? = nil, outputTokens: Int? = nil) {
        self.inputTokens = inputTokens
        self.outputTokens = outputTokens
    }
}

public enum TranslationEventKind: String, Codable, Equatable, Sendable {
    case delta
    case completed
    case usage
}

public struct TranslationEvent: Codable, Equatable, Sendable {
    public let kind: TranslationEventKind
    public let text: String?
    public let usage: TranslationUsage?

    public init(kind: TranslationEventKind, text: String? = nil, usage: TranslationUsage? = nil) {
        self.kind = kind
        self.text = text
        self.usage = usage
    }

    public static func delta(_ text: String) -> TranslationEvent { .init(kind: .delta, text: text) }
    public static func completed(_ text: String) -> TranslationEvent { .init(kind: .completed, text: text) }
    public static func usage(_ usage: TranslationUsage) -> TranslationEvent { .init(kind: .usage, usage: usage) }
}

public struct FinalizedTranslationSource: Codable, Equatable, Sendable {
    public let eventID: UUID
    public let revision: Int
    public let streamID: String
    public let generation: Int
    public let text: String
    public let speaker: String?
    public let capturedAt: Date

    public init(
        eventID: UUID = UUID(),
        revision: Int,
        streamID: String,
        generation: Int,
        text: String,
        speaker: String? = nil,
        capturedAt: Date = .now
    ) {
        self.eventID = eventID
        self.revision = revision
        self.streamID = streamID
        self.generation = generation
        self.text = text
        self.speaker = speaker
        self.capturedAt = capturedAt
    }
}

public enum TranslationEntryStatus: String, Codable, Equatable, Sendable {
    case pending
    case completed
    case failed
    case cancelled
}

public struct TranslationHistoryEntry: Codable, Equatable, Identifiable, Sendable {
    public let id: UUID
    public let source: FinalizedTranslationSource
    public var translatedText: String?
    public var status: TranslationEntryStatus
    public var includeInContext: Bool
    public var failureDescription: String?
    public let createdAt: Date

    public init(
        id: UUID = UUID(),
        source: FinalizedTranslationSource,
        translatedText: String? = nil,
        status: TranslationEntryStatus = .pending,
        includeInContext: Bool = true,
        failureDescription: String? = nil,
        createdAt: Date = .now
    ) {
        self.id = id
        self.source = source
        self.translatedText = translatedText
        self.status = status
        self.includeInContext = includeInContext
        self.failureDescription = failureDescription
        self.createdAt = createdAt
    }
}

public struct TranslationSessionSnapshot: Codable, Equatable, Sendable {
    public let gameID: String
    public let sessionID: UUID
    public let entries: [TranslationHistoryEntry]

    public init(gameID: String, sessionID: UUID, entries: [TranslationHistoryEntry]) {
        self.gameID = gameID
        self.sessionID = sessionID
        self.entries = entries
    }
}
