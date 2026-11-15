//
//  Providers.swift
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
#if canImport(FoundationNetworking)
import FoundationNetworking
#endif

public protocol TranslationProvider: Sendable {
    var kind: ProviderKind { get }
    func discoverModels(configuration: ProviderConfiguration, credential: String?) async throws -> [ModelDescriptor]
    func modelInfo(id: String, configuration: ProviderConfiguration, credential: String?) async throws
        -> ModelDescriptor
    func translate(
        _ request: TranslationRequest,
        configuration: ProviderConfiguration,
        credential: String?
    ) -> AsyncThrowingStream<TranslationEvent, any Error>
}

public struct HTTPTranslationProvider: TranslationProvider, Sendable {
    public let kind: ProviderKind
    private let transport: any HTTPTransport
    private let retryClock: any RetryClock

    public init(kind: ProviderKind, transport: any HTTPTransport, retryClock: any RetryClock = SystemRetryClock()) {
        self.kind = kind
        self.transport = transport
        self.retryClock = retryClock
    }

    public func discoverModels(
        configuration: ProviderConfiguration,
        credential: String?
    ) async throws -> [ModelDescriptor] {
        try validate(configuration, credential: credential, needsModel: false)
        guard kind.supportsModelDiscovery else { throw TranslationError.modelDiscoveryUnsupported }
        switch kind {
        case .openAIChat, .openAICompletions:
            let response = try await get(configuration, path: "models", version: "v1", credential: credential)
            defer { response.cancel() }
            let root = try await jsonDictionary(collect(response.body))
            return try requiredDictionaryArray(root, key: "data").compactMap { item in
                guard let id = item["id"] as? String, !id.isEmpty else { return nil }
                return ModelDescriptor(id: id, displayName: id, contextWindow: openAIContextWindow(item))
            }
        case .ollama:
            let response = try await get(configuration, path: "api/tags", version: nil, credential: credential)
            defer { response.cancel() }
            let root = try await jsonDictionary(collect(response.body))
            return try requiredDictionaryArray(root, key: "models").compactMap { item in
                guard let id = (item["model"] as? String) ?? (item["name"] as? String), !id.isEmpty else { return nil }
                return ModelDescriptor(id: id, displayName: id)
            }
        case .anthropic:
            return try await discoverAnthropicModels(configuration: configuration, credential: credential)
        case .gemini:
            return try await discoverGeminiModels(configuration: configuration, credential: credential)
        case .deepL:
            throw TranslationError.modelDiscoveryUnsupported
        }
    }

    public func modelInfo(
        id: String,
        configuration: ProviderConfiguration,
        credential: String?
    ) async throws -> ModelDescriptor {
        try validate(configuration, credential: credential, needsModel: false)
        guard !id.isEmpty, id.utf8.count <= 512 else { throw TranslationError.invalidModel }
        switch kind {
        case .openAIChat, .openAICompletions:
            let response = try await get(
                configuration,
                path: "models/\(pathSegment(id))",
                version: "v1",
                credential: credential
            )
            defer { response.cancel() }
            let root = try await jsonDictionary(collect(response.body))
            guard let returnedID = root["id"] as? String, returnedID == id else { throw TranslationError.invalidModel }
            return ModelDescriptor(id: id, displayName: id, contextWindow: openAIContextWindow(root))
        case .ollama:
            let body = try jsonData(["model": id])
            let response = try await post(
                configuration,
                path: "api/show",
                version: nil,
                credential: credential,
                body: body
            )
            defer { response.cancel() }
            let root = try await jsonDictionary(collect(response.body))
            let info = root["model_info"] as? [String: Any] ?? [:]
            let context = info.compactMap { key, value -> Int? in
                (key == "context_length" || key == "max_context_length" || key == "max_model_len" || key
                    .hasSuffix(".context_length"))
                    ? positiveInteger(value) : nil
            }
            let explicit = context + ["context_length", "max_context_length", "max_model_len"]
                .compactMap { positiveInteger(root[$0]) }
            return ModelDescriptor(id: id, displayName: id, contextWindow: explicit.min())
        case .anthropic:
            let response = try await get(
                configuration,
                path: "models/\(pathSegment(id))",
                version: "v1",
                credential: credential
            )
            defer { response.cancel() }
            return try await anthropicModel(jsonDictionary(collect(response.body)))
        case .gemini:
            let response = try await get(
                configuration,
                path: "models/\(pathSegment(id))",
                version: "v1beta",
                credential: credential
            )
            defer { response.cancel() }
            return try await geminiModel(jsonDictionary(collect(response.body)))
        case .deepL:
            throw TranslationError.modelDiscoveryUnsupported
        }
    }

    public func translate(
        _ request: TranslationRequest,
        configuration: ProviderConfiguration,
        credential: String?
    ) -> AsyncThrowingStream<TranslationEvent, any Error> {
        eventStream { continuation in
            try request.validate()
            try configuration.validate()
            guard configuration.kind == kind
            else { throw TranslationError.invalidConfiguration("Provider kind does not match configuration") }
            let fitted = try TranslationPrompt.fitting(request, kind: kind)
            try await self.translate(
                fitted,
                configuration: configuration,
                credential: credential,
                continuation: continuation
            )
        }
    }

    private func translate(
        _ request: TranslationRequest,
        configuration: ProviderConfiguration,
        credential: String?,
        continuation: AsyncThrowingStream<TranslationEvent, any Error>.Continuation
    ) async throws {
        try validate(configuration, credential: credential, needsModel: kind != .deepL)
        if !configuration.capabilities.customPrompt, !request.customPrompt.isEmpty {
            throw TranslationError.unsupportedCapability("custom prompt")
        }
        switch kind {
        case .openAIChat:
            try await translateOpenAI(
                request,
                configuration: configuration,
                credential: credential,
                continuation: continuation,
                legacy: false
            )
        case .openAICompletions:
            try await translateOpenAI(
                request,
                configuration: configuration,
                credential: credential,
                continuation: continuation,
                legacy: true
            )
        case .ollama:
            try await translateOllama(
                request,
                configuration: configuration,
                credential: credential,
                continuation: continuation
            )
        case .deepL:
            try await translateDeepL(
                request,
                configuration: configuration,
                credential: credential,
                continuation: continuation
            )
        case .anthropic:
            try await translateAnthropic(
                request,
                configuration: configuration,
                credential: credential,
                continuation: continuation
            )
        case .gemini:
            try await translateGemini(
                request,
                configuration: configuration,
                credential: credential,
                continuation: continuation
            )
        }
    }

    private func translateOpenAI(
        _ request: TranslationRequest,
        configuration: ProviderConfiguration,
        credential: String?,
        continuation: AsyncThrowingStream<TranslationEvent, any Error>.Continuation,
        legacy: Bool
    ) async throws {
        guard let model = configuration.modelID, !model.isEmpty else { throw TranslationError.missingModel }
        let body: [String: Any]
        let path: String
        if legacy {
            path = "completions"
            body = try [
                "model": model,
                "prompt": TranslationPrompt.legacyPrompt(for: request),
                "max_tokens": request.outputTokenLimit,
                "stream": true
            ]
        } else {
            path = "chat/completions"
            var chatBody: [String: Any] = try [
                "model": model,
                "messages": [
                    ["role": "system", "content": TranslationPrompt.systemInstructions(for: request)],
                    ["role": "user", "content": TranslationPrompt.userMessage(for: request)]
                ],
                "stream": true
            ]
            chatBody[configuration.openAIChatTokenLimitField.rawValue] = request.outputTokenLimit
            body = chatBody
        }
        let response = try await post(
            configuration,
            path: path,
            version: "v1",
            credential: credential,
            body: jsonData(body),
            accept: "text/event-stream"
        )
        defer { response.cancel() }
        try await consumeSSE(response.body, continuation: continuation) { frame, aggregate in
            if frame.data == "[DONE]" { return .done }
            let root = try jsonDictionary(Data(frame.data.utf8))
            if frame.event == "error" || root["error"] != nil { throw TranslationError.providerRejected }
            if let choices = root["choices"] as? [[String: Any]] {
                for choice in choices {
                    let fragment: String?
                    if legacy { fragment = choice["text"] as? String } else { fragment = (choice["delta"] as? [String: Any])?["content"] as? String }
                    if let fragment, !fragment.isEmpty {
                        aggregate.append(fragment)
                        try yield(.delta(fragment), to: continuation)
                    }
                }
            }
            if let usage = root["usage"] as? [String: Any] {
                try yield(.usage(TranslationUsage(
                    inputTokens: positiveInteger(usage["prompt_tokens"]),
                    outputTokens: positiveInteger(usage["completion_tokens"])
                )), to: continuation)
            }
            return .continue
        }
    }

    private func translateOllama(
        _ request: TranslationRequest,
        configuration: ProviderConfiguration,
        credential: String?,
        continuation: AsyncThrowingStream<TranslationEvent, any Error>.Continuation
    ) async throws {
        guard let model = configuration.modelID, !model.isEmpty else { throw TranslationError.missingModel }
        var options: [String: Any] = ["num_predict": request.outputTokenLimit]
        if request.contextWindowProvenance != .unknown { options["num_ctx"] = request.contextWindowTokens }
        let body: [String: Any] = try [
            "model": model,
            "messages": [
                ["role": "system", "content": TranslationPrompt.systemInstructions(for: request)],
                ["role": "user", "content": TranslationPrompt.userMessage(for: request)]
            ],
            "stream": true,
            "options": options
        ]
        let response = try await post(
            configuration,
            path: "api/chat",
            version: nil,
            credential: credential,
            body: jsonData(body),
            accept: "application/x-ndjson"
        )
        defer { response.cancel() }
        var parser = NDJSONParser()
        var text = ""
        var done = false
        outer: for try await chunk in response.body {
            for line in parser.append(chunk) {
                try consumeOllamaLine(line, text: &text, done: &done, continuation: continuation)
                if done { break outer }
            }
        }
        if !done {
            for line in parser.finish() {
                try consumeOllamaLine(
                    line,
                    text: &text,
                    done: &done,
                    continuation: continuation
                )
            }
        }
        guard done else { throw TranslationError.incompleteStream }
        guard !text.isEmpty else { throw TranslationError.emptyCompletion }
        try yield(.completed(text), to: continuation)
    }

    private func consumeOllamaLine(
        _ line: Data,
        text: inout String,
        done: inout Bool,
        continuation: AsyncThrowingStream<TranslationEvent, any Error>.Continuation
    ) throws {
        let root = try jsonDictionary(line)
        if root["error"] != nil { throw TranslationError.providerRejected }
        if let message = root["message"] as? [String: Any], let fragment = message["content"] as? String,
           !fragment.isEmpty {
            text.append(fragment)
            try yield(.delta(fragment), to: continuation)
        }
        if root["done"] as? Bool == true {
            done = true
            try yield(.usage(TranslationUsage(
                inputTokens: positiveInteger(root["prompt_eval_count"]),
                outputTokens: positiveInteger(root["eval_count"])
            )), to: continuation)
        }
    }

    private func translateDeepL(
        _ request: TranslationRequest,
        configuration: ProviderConfiguration,
        credential: String?,
        continuation: AsyncThrowingStream<TranslationEvent, any Error>.Continuation
    ) async throws {
        var body: [String: Any] = ["text": [request.sourceText], "target_lang": deeplLanguage(request.targetLanguage)]
        if request.sourceLanguage.lowercased() != "auto" { body["source_lang"] = deeplLanguage(request.sourceLanguage) }
        let context = request.context.compactMap { turn -> String? in
            let parts = [turn.speaker, turn.sourceText, turn.translatedText].compactMap { $0 }.filter { !$0.isEmpty }
            return parts.isEmpty ? nil : parts.joined(separator: "\n")
        }.joined(separator: "\n")
        if !context.isEmpty { body["context"] = context }
        let response = try await post(
            configuration,
            path: "v2/translate",
            version: nil,
            credential: credential,
            body: jsonData(body)
        )
        defer { response.cancel() }
        let root = try await jsonDictionary(collect(response.body))
        guard let first = (root["translations"] as? [[String: Any]])?.first,
              let text = first["text"] as? String, !text.isEmpty
        else { throw TranslationError.emptyCompletion }
        try yield(.completed(text), to: continuation)
    }

    private func translateAnthropic(
        _ request: TranslationRequest,
        configuration: ProviderConfiguration,
        credential: String?,
        continuation: AsyncThrowingStream<TranslationEvent, any Error>.Continuation
    ) async throws {
        guard let model = configuration.modelID, !model.isEmpty else { throw TranslationError.missingModel }
        let body: [String: Any] = try [
            "model": model,
            "max_tokens": request.outputTokenLimit,
            "system": TranslationPrompt.systemInstructions(for: request),
            "messages": [["role": "user", "content": TranslationPrompt.userMessage(for: request)]],
            "stream": true
        ]
        let response = try await post(
            configuration,
            path: "messages",
            version: "v1",
            credential: credential,
            body: jsonData(body),
            accept: "text/event-stream"
        )
        defer { response.cancel() }
        var sawStop = false
        var aggregate = ""
        try await consumeSSE(response.body, continuation: continuation) { frame, accumulated in
            switch frame.event {
            case "content_block_delta":
                let root = try jsonDictionary(Data(frame.data.utf8))
                guard let delta = root["delta"] as? [String: Any], delta["type"] as? String == "text_delta",
                      let fragment = delta["text"] as? String, !fragment.isEmpty
                else { return .continue }
                accumulated.append(fragment)
                aggregate.append(fragment)
                try yield(.delta(fragment), to: continuation)
                return .continue
            case "message_delta":
                let root = try jsonDictionary(Data(frame.data.utf8))
                if let usage = root["usage"] as? [String: Any] {
                    try yield(
                        .usage(TranslationUsage(outputTokens: positiveInteger(usage["output_tokens"]))),
                        to: continuation
                    )
                }
                return .continue
            case "message_stop":
                sawStop = true
                return .finished
            case "error": throw TranslationError.providerRejected
            default: return .continue
            }
        }
        guard sawStop else { throw TranslationError.incompleteStream }
        guard !aggregate.isEmpty else { throw TranslationError.emptyCompletion }
    }

    private func translateGemini(
        _ request: TranslationRequest,
        configuration: ProviderConfiguration,
        credential: String?,
        continuation: AsyncThrowingStream<TranslationEvent, any Error>.Continuation
    ) async throws {
        guard let model = configuration.modelID, !model.isEmpty else { throw TranslationError.missingModel }
        let body: [String: Any] = try [
            "systemInstruction": ["parts": [["text": TranslationPrompt.systemInstructions(for: request)]]],
            "contents": [["role": "user", "parts": [["text": TranslationPrompt.userMessage(for: request)]]]],
            "generationConfig": ["maxOutputTokens": request.outputTokenLimit]
        ]
        let response = try await post(
            configuration,
            path: "models/\(pathSegment(model)):streamGenerateContent",
            version: "v1beta",
            credential: credential,
            body: jsonData(body),
            accept: "text/event-stream",
            query: [URLQueryItem(name: "alt", value: "sse")]
        )
        defer { response.cancel() }
        var aggregate = ""
        var sawCandidate = false
        try await consumeSSE(response.body, continuation: continuation) { frame, accumulated in
            let root = try jsonDictionary(Data(frame.data.utf8))
            if frame.event == "error" || root["error"] != nil { throw TranslationError.providerRejected }
            if let candidates = root["candidates"] as? [[String: Any]] {
                for candidate in candidates {
                    guard let content = candidate["content"] as? [String: Any],
                          let parts = content["parts"] as? [[String: Any]]
                    else { continue }
                    sawCandidate = true
                    for part in parts where part["thought"] as? Bool != true {
                        guard let fragment = part["text"] as? String, !fragment.isEmpty else { continue }
                        accumulated.append(fragment)
                        aggregate.append(fragment)
                        try yield(.delta(fragment), to: continuation)
                    }
                }
            }
            if let usage = root["usageMetadata"] as? [String: Any] {
                try yield(.usage(TranslationUsage(
                    inputTokens: positiveInteger(usage["promptTokenCount"]),
                    outputTokens: positiveInteger(usage["candidatesTokenCount"])
                )), to: continuation)
            }
            return (root["candidates"] as? [[String: Any]])?.contains { $0["finishReason"] is String } == true
                ? .finished : .continue
        }
        guard sawCandidate else { throw TranslationError.emptyCompletion }
        guard !aggregate.isEmpty else { throw TranslationError.emptyCompletion }
    }

    private func discoverAnthropicModels(
        configuration: ProviderConfiguration,
        credential: String?
    ) async throws -> [ModelDescriptor] {
        var result: [ModelDescriptor] = []
        var cursor: String?
        for _ in 0 ..< 20 {
            var query = [URLQueryItem(name: "limit", value: "1000")]
            if let cursor { query.append(URLQueryItem(name: "after_id", value: cursor)) }
            let response = try await get(
                configuration,
                path: "models",
                version: "v1",
                credential: credential,
                query: query
            )
            defer { response.cancel() }
            let root = try await jsonDictionary(collect(response.body))
            for item in try requiredDictionaryArray(root, key: "data") {
                try result.append(anthropicModel(item))
            }
            guard root["has_more"] as? Bool == true, let lastID = root["last_id"] as? String,
                  lastID != cursor
            else { break }
            cursor = lastID
        }
        return result
    }

    private func discoverGeminiModels(
        configuration: ProviderConfiguration,
        credential: String?
    ) async throws -> [ModelDescriptor] {
        var result: [ModelDescriptor] = []
        var pageToken: String?
        for _ in 0 ..< 20 {
            var query: [URLQueryItem] = [URLQueryItem(name: "pageSize", value: "1000")]
            if let pageToken { query.append(URLQueryItem(name: "pageToken", value: pageToken)) }
            let response = try await get(
                configuration,
                path: "models",
                version: "v1beta",
                credential: credential,
                query: query
            )
            defer { response.cancel() }
            let root = try await jsonDictionary(collect(response.body))
            for item in try requiredDictionaryArray(root, key: "models") {
                let methods = Set(item["supportedGenerationMethods"] as? [String] ?? [])
                guard methods.isEmpty || methods.contains("generateContent") else { continue }
                try result.append(geminiModel(item))
            }
            guard let next = root["nextPageToken"] as? String, !next.isEmpty, next != pageToken else { break }
            pageToken = next
        }
        return result
    }

    private func anthropicModel(_ item: [String: Any]) throws -> ModelDescriptor {
        guard let id = item["id"] as? String, !id.isEmpty else { throw TranslationError.malformedResponse }
        return ModelDescriptor(
            id: id,
            displayName: item["display_name"] as? String ?? id,
            contextWindow: positiveInteger(item["max_input_tokens"])
        )
    }

    private func geminiModel(_ item: [String: Any]) throws -> ModelDescriptor {
        guard let name = item["name"] as? String, !name.isEmpty else { throw TranslationError.malformedResponse }
        let id = name.hasPrefix("models/") ? String(name.dropFirst("models/".count)) : name
        var capabilities: Set<ModelCapability> = []
        let methods = item["supportedGenerationMethods"] as? [String] ?? []
        if methods.contains("generateContent") { capabilities.insert(.generateContent) }
        return ModelDescriptor(
            id: id,
            displayName: item["displayName"] as? String ?? id,
            contextWindow: positiveInteger(item["inputTokenLimit"]),
            capabilities: capabilities
        )
    }

    private func validate(_ configuration: ProviderConfiguration, credential: String?, needsModel: Bool) throws {
        try configuration.validate()
        guard configuration.kind == kind
        else { throw TranslationError.invalidConfiguration("Provider kind does not match configuration") }
        if needsModel, configuration.modelID?.isEmpty != false { throw TranslationError.missingModel }
        if kind.requiresCredential(at: configuration.baseURL),
           credential?.isEmpty != false { throw TranslationError.missingCredential }
        if configuration.credentialReference != nil,
           credential?.isEmpty != false { throw TranslationError.missingCredential }
    }

    private func get(
        _ configuration: ProviderConfiguration,
        path: String,
        version: String?,
        credential: String?,
        query: [URLQueryItem] = []
    ) async throws -> HTTPResponse {
        try await send(
            configuration,
            path: path,
            version: version,
            credential: credential,
            method: "GET",
            body: nil,
            accept: "application/json",
            query: query
        )
    }

    private func post(
        _ configuration: ProviderConfiguration,
        path: String,
        version: String?,
        credential: String?,
        body: Data,
        accept: String = "application/json",
        query: [URLQueryItem] = []
    ) async throws -> HTTPResponse {
        try await send(
            configuration,
            path: path,
            version: version,
            credential: credential,
            method: "POST",
            body: body,
            accept: accept,
            query: query
        )
    }

    private func send(
        _ configuration: ProviderConfiguration,
        path: String,
        version: String?,
        credential: String?,
        method: String,
        body: Data?,
        accept: String,
        query: [URLQueryItem]
    ) async throws -> HTTPResponse {
        let url = try endpointURL(configuration.baseURL, version: version, path: path, query: query)
        var request = URLRequest(url: url)
        request.httpMethod = method
        request.timeoutInterval = 30
        request.setValue(accept, forHTTPHeaderField: "Accept")
        if let body {
            request.httpBody = body
            request.setValue("application/json", forHTTPHeaderField: "Content-Type")
        }
        switch kind {
        case .openAIChat, .openAICompletions:
            if let credential { request.setValue("Bearer \(credential)", forHTTPHeaderField: "Authorization") }
        case .anthropic:
            if let credential { request.setValue(credential, forHTTPHeaderField: "x-api-key") }
            request.setValue("2023-06-01", forHTTPHeaderField: "anthropic-version")
        case .gemini:
            if let credential { request.setValue(credential, forHTTPHeaderField: "x-goog-api-key") }
        case .ollama:
            if let credential { request.setValue("Bearer \(credential)", forHTTPHeaderField: "Authorization") }
        case .deepL:
            if let credential { request.setValue("DeepL-Auth-Key \(credential)", forHTTPHeaderField: "Authorization") }
        }
        return try await executeWithRetry(request, transport: transport, clock: retryClock)
    }
}

func endpointURL(_ baseURL: URL, version: String?, path: String, query: [URLQueryItem] = []) throws -> URL {
    guard var components = URLComponents(url: baseURL, resolvingAgainstBaseURL: false) else {
        throw TranslationError.invalidConfiguration("Provider URL is invalid")
    }
    var basePath = components.percentEncodedPath.trimmingCharacters(in: CharacterSet(charactersIn: "/"))
    let lastComponent = basePath.split(separator: "/").last.map(String.init)
    if let version, lastComponent?.caseInsensitiveCompare(version) != .orderedSame {
        basePath = [basePath, version].filter { !$0.isEmpty }.joined(separator: "/")
    }
    let endpoint = path.trimmingCharacters(in: CharacterSet(charactersIn: "/"))
    components.percentEncodedPath = "/" + [basePath, endpoint].filter { !$0.isEmpty }.joined(separator: "/")
    components.queryItems = query.isEmpty ? nil : query
    guard let url = components.url else { throw TranslationError.invalidConfiguration("Provider URL is invalid") }
    return url
}

private func pathSegment(_ value: String) -> String {
    let allowed = CharacterSet.alphanumerics.union(CharacterSet(charactersIn: "-._~:@"))
    return value.addingPercentEncoding(withAllowedCharacters: allowed) ?? ""
}

private func deeplLanguage(_ value: String) -> String {
    value.replacingOccurrences(of: "_", with: "-").uppercased()
}

private func jsonData(_ value: [String: Any]) throws -> Data {
    try JSONSerialization.data(withJSONObject: value, options: [.sortedKeys, .fragmentsAllowed])
}

private func collect(
    _ stream: AsyncThrowingStream<Data, any Error>,
    maximumBytes: Int = 1_048_576
) async throws -> Data {
    var data = Data()
    for try await chunk in stream {
        guard data.count + chunk.count <= maximumBytes else { throw TranslationError.responseTooLarge }
        data.append(chunk)
    }
    return data
}

private enum ParsedFrame: Equatable { case `continue`, done, finished }

private func consumeSSE(
    _ stream: AsyncThrowingStream<Data, any Error>,
    continuation: AsyncThrowingStream<TranslationEvent, any Error>.Continuation,
    consume: (SSEFrame, inout String) throws -> ParsedFrame
) async throws {
    var parser = SSEParser()
    var aggregate = ""
    var complete = false
    outer: for try await chunk in stream {
        for frame in try parser.append(chunk) {
            // swiftlint:disable:next for_where
            if try consume(frame, &aggregate) != .continue {
                complete = true
                break outer
            }
        }
    }
    if !complete {
        for frame in try parser.finish() {
            // swiftlint:disable:next for_where
            if try consume(frame, &aggregate) != .continue {
                complete = true
                break
            }
        }
    }
    guard complete else { throw TranslationError.incompleteStream }
    guard !aggregate.isEmpty else { throw TranslationError.emptyCompletion }
    try yield(.completed(aggregate), to: continuation)
}

private func openAIContextWindow(_ model: [String: Any]) -> Int? {
    ["context_length", "max_context_length", "max_model_len"]
        .compactMap { positiveInteger(model[$0]) }
        .min()
}

private func eventStream(
    _ operation: @escaping @Sendable (AsyncThrowingStream<TranslationEvent, any Error>.Continuation) async throws
        -> Void
) -> AsyncThrowingStream<TranslationEvent, any Error> {
    let (stream, continuation) = AsyncThrowingStream<TranslationEvent, any Error>
        .makeStream(bufferingPolicy: .bufferingOldest(256))
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

private func yield(
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
