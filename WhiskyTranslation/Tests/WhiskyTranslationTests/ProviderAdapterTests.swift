//
//  ProviderAdapterTests.swift
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

@Suite("Provider adapters")
struct ProviderAdapterTests {
    @Test func openAIChatNormalizesEndpointAndParsesByteSplitUnicode() async throws {
        // swiftlint:disable:next line_length
        let sse = "data: {\"choices\":[{\"delta\":{\"content\":\"世\"}}]}\r\n\r\ndata: {\"choices\":[{\"delta\":{\"content\":\"界🌸\"}}]}\r\n\r\ndata: {\"choices\":[{\"finish_reason\":\"stop\"}]}\r\n\r\ndata: [DONE]\r\n\r\n"
        let transport = QueueHTTPTransport([makeHTTPResponse(
            sse,
            headers: ["Content-Type": "text/event-stream"],
            chunkSize: 1
        )])
        let service = try await TranslationService(transport: transport, credentialStore: testCredentialStore())
        let config = ProviderConfiguration(
            kind: .openAIChat,
            baseURL: URL(string: "http://localhost:8765/v1/")!,
            modelID: "fixture-model",
            credentialReference: "credential-ref"
        )
        let requestData = testTranslationRequest(
            context: [TranslationContextTurn(sourceText: "past source", translatedText: "past translation")],
            customPrompt: "Keep names"
        )
        let events = try await collectEvents(service.translate(requestData, using: config))
        #expect(events.last == .completed("世界🌸"))
        #expect(events.filter { $0.kind == .completed }.count == 1)
        let request = try #require(await transport.capturedRequests().first)
        #expect(request.url?.path == "/v1/chat/completions")
        #expect(request.headers.first { $0.key.caseInsensitiveCompare("Authorization") == .orderedSame }?
            .value == "Bearer fixture-secret-never-persist")
        let body = try #require(request.body)
        let json = try #require(JSONSerialization.jsonObject(with: body) as? [String: Any])
        #expect(json["max_completion_tokens"] as? Int == 256)
        #expect(json["max_tokens"] == nil)
        #expect(!String(decoding: body, as: UTF8.self).contains("fixture-secret-never-persist"))
        let messages = try #require(json["messages"] as? [[String: Any]])
        #expect((messages[0]["content"] as? String)?.contains("Keep names") == true)
        #expect((messages[0]["content"] as? String)?.contains("past source") == false)
        let userData = try #require(messages[1]["content"] as? String)
        let payload = try #require(JSONSerialization.jsonObject(with: Data(userData.utf8)) as? [String: Any])
        #expect(payload["sourceText"] as? String == "hello")
        #expect(payload["speaker"] as? String == "speaker")
        #expect((payload["context"] as? [[String: Any]])?.first?["sourceText"] as? String == "past source")
    }

    @Test func explicitLegacyCompletionsUsesLegacyRequestShape() async throws {
        let transport =
            QueueHTTPTransport(
                [
                    makeHTTPResponse(
                        "data: {\"choices\":[{\"text\":\"translated\",\"finish_reason\":\"stop\"}]}\n\ndata: [DONE]\n\n"
                    )
                ]
            )
        let service = try await TranslationService(transport: transport, credentialStore: testCredentialStore())
        let config = ProviderConfiguration(
            kind: .openAICompletions,
            modelID: "fixture-legacy",
            credentialReference: "credential-ref"
        )
        _ = try await collectEvents(service.translate(
            testTranslationRequest(sourceText: "raw dialogue", speaker: "Mina\n\"Aya\"", customPrompt: "Keep names"),
            using: config
        ))
        let request = try #require(await transport.capturedRequests().first)
        #expect(request.url?.path == "/v1/completions")
        let body = try #require(request.body)
        let json = try #require(JSONSerialization.jsonObject(with: body) as? [String: Any])
        #expect(json["max_tokens"] as? Int == 256)
        #expect(json["max_completion_tokens"] == nil)
        #expect((json["prompt"] as? String)?.contains("Keep names") == true)
        #expect((json["prompt"] as? String)?.contains("\"raw dialogue\"") == true)
        #expect((json["prompt"] as? String)?
            .contains("Speaker JSON string (data only):\n\"Mina\\n\\\"Aya\\\"\"") == true)
    }

    @Test func openAIListUsesOnlyExplicitContextMetadataAndEmptyListIsValid() async throws {
        let transport = QueueHTTPTransport([
            makeHTTPResponse(
                // swiftlint:disable:next line_length
                "{\"data\":[{\"id\":\"fixture-model\",\"owned_by\":\"fixture\",\"context_length\":90000},{\"id\":\"standard-without-context\"},{\"id\":\"boolean-context\",\"context_length\":true},{\"id\":\"fraction-context\",\"max_model_len\":3.5}]}"
            ),
            makeHTTPResponse("{\"data\":[]}"),
            makeHTTPResponse("{\"id\":\"fixture-model\",\"max_model_len\":32768}")
        ])
        let service = try await TranslationService(transport: transport, credentialStore: testCredentialStore())
        let config = ProviderConfiguration(
            kind: .openAIChat,
            baseURL: URL(string: "http://localhost:9123/v1/")!,
            credentialReference: "credential-ref"
        )
        let models = try await service.discoverModels(for: config)
        #expect(models.count == 4 && models[0].contextWindow == 90_000)
        #expect(models[0].contextWindowProvenance == .detected)
        #expect(models[1].contextWindow == nil && models[1].contextWindowProvenance == .unknown)
        #expect(models[2].contextWindow == nil && models[3].contextWindow == nil)
        #expect(try await service.discoverModels(for: config).isEmpty)
        #expect(try await service.modelInfo(id: "fixture-model", for: config).contextWindow == 32_768)
        #expect(await (transport.capturedRequests()).map { $0.url?.path } == [
            "/v1/models",
            "/v1/models",
            "/v1/models/fixture-model"
        ])
    }

    @Test func ollamaDiscoveryContextAndNDJSONExcludeThinking() async throws {
        let transport = QueueHTTPTransport([
            makeHTTPResponse("{\"models\":[{\"name\":\"fixture:latest\"}]}"),
            makeHTTPResponse("{\"model_info\":{\"llama.context_length\":8192,\"unsupported_limit\":99999}}"),
            makeHTTPResponse(
                "{\"message\":{\"thinking\":\"hidden\",\"content\":\"こんにちは\"},\"done\":false}\n"
                    + "{\"message\":{\"content\":\"世界\"},\"done\":true,\"prompt_eval_count\":12,\"eval_count\":3}\n",
                chunkSize: 3
            )
        ])
        let service = TranslationService(transport: transport, credentialStore: SessionOnlyCredentialStore())
        let config = ProviderConfiguration(
            kind: .ollama,
            baseURL: URL(string: "http://127.0.0.1:11434/")!,
            modelID: "fixture:latest"
        )
        #expect(try await service.discoverModels(for: config).map(\.id) == ["fixture:latest"])
        let info = try await service.modelInfo(id: "fixture:latest", for: config)
        #expect(info.contextWindow == 8_192 && info.contextWindowProvenance == .detected)
        let events = try await collectEvents(service.translate(testTranslationRequest(), using: config))
        #expect(events.last == .completed("こんにちは世界"))
        #expect(events.filter { $0.kind == .completed }.count == 1)
        #expect(!events.compactMap(\.text).joined().contains("hidden"))
        let requests = await transport.capturedRequests()
        #expect(requests.map { $0.url?.path } == ["/api/tags", "/api/show", "/api/chat"])
    }

    @Test func anthropicAndGeminiModelListsReportDocumentedContextAndSkipThinking() async throws {
        let anthropicTransport = QueueHTTPTransport([
            makeHTTPResponse(
                "{\"data\":[{\"id\":\"fixture-claude\",\"display_name\":\"Fixture Claude\",\"max_input_tokens\":100000}],\"has_more\":false}"
            ),
            makeHTTPResponse(
                // swiftlint:disable:next line_length
                "event: content_block_delta\ndata: {\"delta\":{\"type\":\"text_delta\",\"text\":\"bonjour\"}}\n\nevent: content_block_delta\ndata: {\"delta\":{\"type\":\"thinking_delta\",\"thinking\":\"hidden\"}}\n\nevent: message_stop\ndata: {\"type\":\"message_stop\"}\n\n"
            )
        ])
        let anthropic = try await TranslationService(
            transport: anthropicTransport,
            credentialStore: testCredentialStore()
        )
        let claude = ProviderConfiguration(
            kind: .anthropic,
            modelID: "fixture-claude",
            credentialReference: "credential-ref"
        )
        #expect(try await anthropic.discoverModels(for: claude).first?.contextWindow == 100_000)
        let chatRequest = testTranslationRequest(
            context: [TranslationContextTurn(sourceText: "prior source", translatedText: "prior result")],
            customPrompt: "Keep names"
        )
        let claudeEvents = try await collectEvents(anthropic.translate(chatRequest, using: claude))
        #expect(claudeEvents.last == .completed("bonjour"))
        #expect(claudeEvents.filter { $0.kind == .completed }.count == 1)
        #expect(!claudeEvents.compactMap(\.text).joined().contains("hidden"))
        let claudeBody = try #require(await anthropicTransport.capturedRequests().last?.body)
        let claudeJSON = try #require(JSONSerialization.jsonObject(with: claudeBody) as? [String: Any])
        #expect((claudeJSON["system"] as? String)?.contains("Keep names") == true)
        #expect((claudeJSON["system"] as? String)?.contains("prior source") == false)
        let claudeMessages = try #require(claudeJSON["messages"] as? [[String: Any]])
        #expect((claudeMessages.first?["content"] as? String)?.contains("prior source") == true)

        let geminiTransport = QueueHTTPTransport([
            makeHTTPResponse(
                "{\"models\":[{\"name\":\"models/fixture-gemini\",\"displayName\":\"Fixture Gemini\","
                    + "\"inputTokenLimit\":32000,\"supportedGenerationMethods\":[\"generateContent\"]}]}"
            ),
            makeHTTPResponse(
                // swiftlint:disable:next line_length
                "data: {\"candidates\":[{\"content\":{\"parts\":[{\"text\":\"hidden thought\",\"thought\":true},{\"text\":\"hola\"}]}}]}\n\ndata: {\"candidates\":[{\"content\":{\"parts\":[]},\"finishReason\":\"STOP\"}]}\n\n"
            )
        ])
        let gemini = try await TranslationService(transport: geminiTransport, credentialStore: testCredentialStore())
        let config = ProviderConfiguration(
            kind: .gemini,
            modelID: "fixture-gemini",
            credentialReference: "credential-ref"
        )
        let models = try await gemini.discoverModels(for: config)
        #expect(models.first?.contextWindow == 32_000 && models.first?.contextWindowProvenance == .detected)
        let events = try await collectEvents(gemini.translate(chatRequest, using: config))
        #expect(events.last == .completed("hola"))
        #expect(events.filter { $0.kind == .completed }.count == 1)
        #expect(!events.compactMap(\.text).joined().contains("hidden thought"))
        let request = try #require(await geminiTransport.capturedRequests().last)
        #expect(request.url?.path.contains("streamGenerateContent") == true)
        #expect(request.url?.query?.contains("alt=sse") == true)
        #expect(request.headers.keys.contains { $0.caseInsensitiveCompare("x-goog-api-key") == .orderedSame })
        let geminiBody = try #require(request.body)
        let geminiJSON = try #require(JSONSerialization.jsonObject(with: geminiBody) as? [String: Any])
        let systemParts = (geminiJSON["systemInstruction"] as? [String: Any])?["parts"] as? [[String: Any]]
        #expect((systemParts?.first?["text"] as? String)?.contains("Keep names") == true)
        #expect((systemParts?.first?["text"] as? String)?.contains("prior source") == false)
        let userParts = ((geminiJSON["contents"] as? [[String: Any]])?.first?["parts"] as? [[String: Any]])
        #expect((userParts?.first?["text"] as? String)?.contains("prior source") == true)
    }

    @Test func deepLUsesNativeTranslationJSONAndNoModelList() async throws {
        let transport = QueueHTTPTransport([makeHTTPResponse("{\"translations\":[{\"text\":\"translated\"}]}")])
        let service = try await TranslationService(transport: transport, credentialStore: testCredentialStore())
        let config = ProviderConfiguration(kind: .deepL, credentialReference: "credential-ref")
        let request = testTranslationRequest(context: [TranslationContextTurn(
            sourceText: "previous",
            translatedText: "prior"
        )])
        let events = try await collectEvents(service.translate(request, using: config))
        #expect(events.last == .completed("translated"))
        #expect(events.filter { $0.kind == .completed }.count == 1)
        let captured = try #require(await transport.capturedRequests().first)
        #expect(captured.url?.path == "/v2/translate")
        #expect(captured.headers.first { $0.key.caseInsensitiveCompare("Authorization") == .orderedSame }?
            .value == "DeepL-Auth-Key fixture-secret-never-persist")
        let body = try #require(captured.body)
        let json = try #require(JSONSerialization.jsonObject(with: body) as? [String: Any])
        #expect(json["target_lang"] as? String == "JA")
        #expect((json["context"] as? String)?.contains("previous") == true)
        #expect(!String(decoding: body, as: UTF8.self).contains("customPrompt"))
        do { _ = try await service.discoverModels(for: config)
            Issue.record("DeepL advertised model discovery")
        } catch let error as TranslationError { #expect(error == .modelDiscoveryUnsupported) }
    }

    @Test func chatTokenLimitFieldIsExplicitAndLocalOpenAICanBeAnonymous() async throws {
        let transport =
            QueueHTTPTransport(
                [makeHTTPResponse("data: {\"choices\":[{\"delta\":{\"content\":\"ok\"}}]}\n\ndata: [DONE]\n\n")]
            )
        let service = TranslationService(transport: transport, credentialStore: SessionOnlyCredentialStore())
        let config = ProviderConfiguration(
            kind: .openAIChat,
            baseURL: URL(string: "http://localhost:8765/v1")!,
            modelID: "manual-compatible-model",
            openAIChatTokenLimitField: .maxTokens
        )
        let events = try await collectEvents(service.translate(testTranslationRequest(), using: config))
        #expect(events.filter { $0.kind == .completed }.count == 1)
        let captured = try #require(await transport.capturedRequests().first)
        #expect(captured.headers.keys.allSatisfy { $0.caseInsensitiveCompare("Authorization") != .orderedSame })
        let body = try #require(captured.body)
        let json = try #require(JSONSerialization.jsonObject(with: body) as? [String: Any])
        #expect(json["max_tokens"] as? Int == 256)
        #expect(json["max_completion_tokens"] == nil)
    }

    @Test func terminalDoneCancelsPersistentResponseAfterExactlyOneCompletion() async throws {
        let transport = PersistentTerminalHTTPTransport()
        let service = TranslationService(transport: transport, credentialStore: SessionOnlyCredentialStore())
        let config = ProviderConfiguration(
            kind: .openAIChat,
            baseURL: URL(string: "http://localhost:8765/v1")!,
            modelID: "fixture"
        )
        let task = Task { try await collectEvents(service.translate(testTranslationRequest(), using: config)) }
        var cancelled = false
        for _ in 0 ..< 100 {
            if await transport.wasCancelled() { cancelled = true
                break
            }
            try await Task.sleep(for: .milliseconds(5))
        }
        #expect(cancelled)
        if !cancelled { task.cancel() }
        let events = try? await task.value
        #expect(events?.filter { $0.kind == .completed }.count == 1)
    }

    @Test func ollamaSendsOptionalConfiguredBearerCredential() async throws {
        let transport = QueueHTTPTransport([makeHTTPResponse("{\"models\":[]}")])
        let service = try await TranslationService(transport: transport, credentialStore: testCredentialStore())
        let config = ProviderConfiguration(
            kind: .ollama,
            baseURL: URL(string: "http://localhost:11434")!,
            credentialReference: "credential-ref"
        )
        _ = try await service.discoverModels(for: config)
        let request = try #require(await transport.capturedRequests().first)
        #expect(request.headers.first { $0.key.caseInsensitiveCompare("Authorization") == .orderedSame }?
            .value == "Bearer fixture-secret-never-persist")
    }

    @Test func explicitMissingCredentialAndDeepLCustomPromptFailClearly() async throws {
        let service = TranslationService(
            transport: QueueHTTPTransport([]),
            credentialStore: SessionOnlyCredentialStore()
        )
        let local = ProviderConfiguration(
            kind: .openAIChat,
            baseURL: URL(string: "http://127.0.0.1:9000/v1")!,
            credentialReference: "missing"
        )
        do { _ = try await service.discoverModels(for: local)
            Issue.record("missing referenced credential accepted")
        } catch let error as TranslationError { #expect(error == .missingCredential) }
        let deepL = ProviderConfiguration(kind: .deepL, credentialReference: "missing")
        do {
            _ = try await collectEvents(service.translate(testTranslationRequest(customPrompt: "custom"), using: deepL))
            Issue.record("DeepL silently accepted custom prompt")
        } catch let error as TranslationError { #expect(error == .unsupportedCapability("custom prompt")) }
    }

    @Test func malformedRequiredModelListIsRejected() async throws {
        let service = TranslationService(
            transport: QueueHTTPTransport([makeHTTPResponse("{}")]),
            credentialStore: SessionOnlyCredentialStore()
        )
        let config = ProviderConfiguration(kind: .openAIChat, baseURL: URL(string: "http://localhost:8765/v1")!)
        do { _ = try await service.discoverModels(for: config)
            Issue.record("missing data container was treated as empty")
        } catch let error as TranslationError { #expect(error == .malformedResponse) }
    }

    @Test func nativeProviderErrorFramesAndTruncatedStreamsAreExplicit() async throws {
        let credentials = try await testCredentialStore()
        let errorFixtures: [(ProviderKind, String, URL?)] = [
            (.openAIChat, "event: error\ndata: {\"message\":\"private\"}\n\n", URL(string: "http://localhost:8765/v1")),
            (.ollama, "{\"error\":\"private\"}\n", URL(string: "http://localhost:11434")),
            (.anthropic, "event: error\ndata: {\"type\":\"error\"}\n\n", nil),
            (.gemini, "data: {\"error\":{\"message\":\"private\"}}\n\n", nil)
        ]
        for (kind, fixture, baseURL) in errorFixtures {
            let transport = QueueHTTPTransport([makeHTTPResponse(fixture)])
            let service = TranslationService(transport: transport, credentialStore: credentials)
            let config = ProviderConfiguration(
                kind: kind,
                baseURL: baseURL,
                modelID: "fixture",
                credentialReference: kind == .ollama ? nil : "credential-ref"
            )
            do { _ = try await collectEvents(service.translate(testTranslationRequest(), using: config))
                Issue.record("provider error frame accepted: \(kind)")
            } catch let error as TranslationError {
                #expect(error == .providerRejected)
                #expect(error.localizedDescription.contains("private") == false)
            }
        }

        let truncatedFixtures: [(ProviderKind, String, URL?)] = [
            (
                .openAIChat,
                "data: {\"choices\":[{\"delta\":{\"content\":\"x\"}}]}\n\n",
                URL(string: "http://localhost:8765/v1")
            ),
            (.ollama, "{\"message\":{\"content\":\"x\"},\"done\":false}\n", URL(string: "http://localhost:11434")),
            (
                .anthropic,
                "event: content_block_delta\ndata: {\"delta\":{\"type\":\"text_delta\",\"text\":\"x\"}}\n\n",
                nil
            ),
            (.gemini, "data: {\"candidates\":[{\"content\":{\"parts\":[{\"text\":\"x\"}]}}]}\n\n", nil)
        ]
        for (kind, fixture, baseURL) in truncatedFixtures {
            let service = TranslationService(
                transport: QueueHTTPTransport([makeHTTPResponse(fixture)]),
                credentialStore: credentials
            )
            let config = ProviderConfiguration(
                kind: kind,
                baseURL: baseURL,
                modelID: "fixture",
                credentialReference: kind == .ollama ? nil : "credential-ref"
            )
            do { _ = try await collectEvents(service.translate(testTranslationRequest(), using: config))
                Issue.record("truncated stream accepted: \(kind)")
            } catch let error as TranslationError { #expect(error == .incompleteStream) }
        }
        let deepL = TranslationService(
            transport: QueueHTTPTransport([makeHTTPResponse("{")]),
            credentialStore: credentials
        )
        do { _ = try await collectEvents(deepL.translate(
            testTranslationRequest(),
            using: ProviderConfiguration(kind: .deepL, credentialReference: "credential-ref")
        ))
        Issue.record("truncated DeepL JSON accepted")
        } catch let error as TranslationError { #expect(error == .malformedResponse) }
    }

    @Test func retryAfterIsUsedAndAuthenticationFailureIsNotRetried() async throws {
        let transport = QueueHTTPTransport([
            makeHTTPResponse("", status: 429, headers: ["Retry-After": "0"]),
            makeHTTPResponse("{\"data\":[]}")
        ])
        let clock = ImmediateRetryClock()
        let provider = HTTPTranslationProvider(kind: .openAIChat, transport: transport, retryClock: clock)
        let config = ProviderConfiguration(kind: .openAIChat, credentialReference: "credential-ref")
        #expect(try await provider.discoverModels(configuration: config, credential: "secret").isEmpty)
        #expect(await clock.delays.count == 1)
        let deniedClock = ImmediateRetryClock()
        let denied = HTTPTranslationProvider(
            kind: .openAIChat,
            transport: QueueHTTPTransport([makeHTTPResponse("", status: 401)]),
            retryClock: deniedClock
        )
        do { _ = try await denied.discoverModels(configuration: config, credential: "secret")
            Issue.record("401 succeeded")
        } catch let error as TranslationError { #expect(error == .httpStatus(401)) }
        #expect(await deniedClock.delays.isEmpty)
    }

    @Test func quota429IsNotRetriedAndRetriesAndTimeoutsAreFinite() async throws {
        let config = ProviderConfiguration(kind: .openAIChat, baseURL: URL(string: "http://localhost:8765/v1")!)
        let quotaClock = ImmediateRetryClock()
        let quotaTransport = QueueHTTPTransport([makeHTTPResponse(
            "{\"error\":{\"type\":\"insufficient_quota\",\"message\":\"private billing details\"}}",
            status: 429
        )])
        let quota = HTTPTranslationProvider(kind: .openAIChat, transport: quotaTransport, retryClock: quotaClock)
        do { _ = try await quota.discoverModels(configuration: config, credential: nil)
            Issue.record("quota failure succeeded")
        } catch let error as TranslationError {
            #expect(error == .quotaOrBillingLimit)
            #expect(error.localizedDescription.contains("private") == false)
        }
        #expect(await quotaTransport.capturedRequests().count == 1)
        #expect(await quotaClock.delays.isEmpty)

        let exhaustedClock = ImmediateRetryClock()
        let exhaustedTransport = QueueHTTPTransport((0 ..< 3).map { _ in makeHTTPResponse(
            "{\"error\":{\"type\":\"rate_limit_error\"}}",
            status: 429
        ) })
        let exhausted = HTTPTranslationProvider(
            kind: .openAIChat,
            transport: exhaustedTransport,
            retryClock: exhaustedClock
        )
        do { _ = try await exhausted.discoverModels(configuration: config, credential: nil)
            Issue.record("retry exhaustion succeeded")
        } catch let error as TranslationError { #expect(error == .httpStatus(429)) }
        #expect(await exhaustedTransport.capturedRequests().count == 3)
        #expect(await exhaustedClock.delays.count == 2)

        let timeoutClock = ImmediateRetryClock()
        let timeoutTransport = FailingHTTPTransport(.timedOut)
        let timeout = HTTPTranslationProvider(kind: .openAIChat, transport: timeoutTransport, retryClock: timeoutClock)
        do { _ = try await timeout.discoverModels(configuration: config, credential: nil)
            Issue.record("timeout unexpectedly succeeded")
        } catch let error as TranslationError { #expect(error == .transportFailure) }
        #expect(await timeoutTransport.requestCount == 3)
        #expect(await timeoutClock.delays.count == 2)
    }

    @Test func overBudgetCurrentTextFailsWithoutTruncationOrRequest() async throws {
        let transport = QueueHTTPTransport([])
        let service = try await TranslationService(transport: transport, credentialStore: testCredentialStore())
        let config = ProviderConfiguration(
            kind: .openAIChat,
            modelID: "fixture-model",
            credentialReference: "credential-ref"
        )
        let request = testTranslationRequest(
            sourceText: String(repeating: "界", count: 2_000),
            contextWindowTokens: 1_000,
            outputTokenLimit: 256
        )
        do { _ = try await collectEvents(service.translate(request, using: config))
            Issue.record("oversized text accepted")
        } catch let error as TranslationError { #expect(error == .sourceExceedsBudget(maximumBytes: 616)) }
        #expect(await transport.capturedRequests().isEmpty)
    }

    @Test func legacyProviderConfigurationDecodesDefaultThinkingLevel() throws {
        let legacyJSON = """
        {
          "id": "00000000-0000-0000-0000-000000000001",
          "name": "Legacy OpenAI",
          "kind": "openAIChat",
          "baseURL": "https://api.openai.com/v1",
          "modelID": "gpt-4o",
          "defaultSourceLanguage": "ja",
          "defaultTargetLanguage": "en",
          "openAIChatTokenLimitField": "max_completion_tokens"
        }
        """
        let decoded = try JSONDecoder().decode(ProviderConfiguration.self, from: Data(legacyJSON.utf8))
        #expect(decoded.thinkingLevel == .default)
    }

    // swiftlint:disable:next function_body_length
    @Test func thinkingLevelMapsAcrossProvidersAndOmitsWhenDefault() async throws {
        let openAISSE = "data: {\"choices\":[{\"delta\":{\"content\":\"ok\"}}]}\n\ndata: [DONE]\n\n"
        let openAICases: [(ThinkingLevel, String?)] = [
            (.default, nil),
            (.off, "none"),
            (.low, "low"),
            (.medium, "medium"),
            (.high, "high"),
            (.xhigh, "xhigh"),
            (.max, "max"),
            (.ultra, "ultra")
        ]
        for (level, expected) in openAICases {
            let transport = QueueHTTPTransport([makeHTTPResponse(openAISSE)])
            let service = TranslationService(transport: transport, credentialStore: SessionOnlyCredentialStore())
            let config = ProviderConfiguration(
                kind: .openAIChat,
                baseURL: URL(string: "http://localhost:8765/v1")!,
                modelID: "fixture",
                thinkingLevel: level
            )
            _ = try await collectEvents(service.translate(testTranslationRequest(), using: config))
            let body = try #require(await transport.capturedRequests().first?.body)
            let json = try #require(JSONSerialization.jsonObject(with: body) as? [String: Any])
            #expect(json["reasoning_effort"] as? String == expected)
        }

        let anthropicSSE =
            "event: content_block_delta\ndata: {\"delta\":{\"type\":\"text_delta\",\"text\":\"ok\"}}\n\n"
                + "event: message_stop\ndata: {\"type\":\"message_stop\"}\n\n"
        let anthropicBudgets: [(ThinkingLevel, Int)] = [
            (.low, 1_024),
            (.medium, 4_096),
            (.high, 8_192),
            (.xhigh, 16_384),
            (.max, 32_768),
            (.ultra, 65_536)
        ]
        for (level, expectedBudget) in anthropicBudgets {
            let transport = QueueHTTPTransport([makeHTTPResponse(anthropicSSE)])
            let service = try await TranslationService(transport: transport, credentialStore: testCredentialStore())
            let config = ProviderConfiguration(
                kind: .anthropic,
                modelID: "fixture-claude",
                credentialReference: "credential-ref",
                thinkingLevel: level
            )
            _ = try await collectEvents(service.translate(testTranslationRequest(outputTokenLimit: 512), using: config))
            let body = try #require(await transport.capturedRequests().first?.body)
            let json = try #require(JSONSerialization.jsonObject(with: body) as? [String: Any])
            let thinking = try #require(json["thinking"] as? [String: Any])
            #expect(thinking["type"] as? String == "enabled")
            #expect(thinking["budget_tokens"] as? Int == expectedBudget)
            #expect(json["max_tokens"] as? Int == expectedBudget + 1_024)
        }
        do {
            let transport = QueueHTTPTransport([makeHTTPResponse(anthropicSSE), makeHTTPResponse(anthropicSSE)])
            let service = try await TranslationService(transport: transport, credentialStore: testCredentialStore())
            for level in [ThinkingLevel.default, ThinkingLevel.off] {
                let config = ProviderConfiguration(
                    kind: .anthropic,
                    modelID: "fixture-claude",
                    credentialReference: "credential-ref",
                    thinkingLevel: level
                )
                _ = try await collectEvents(
                    service.translate(testTranslationRequest(outputTokenLimit: 512), using: config)
                )
            }
            let requests = await transport.capturedRequests()
            let defaultBody = try #require(requests[0].body)
            let defaultJSON = try #require(JSONSerialization.jsonObject(with: defaultBody) as? [String: Any])
            #expect(defaultJSON["thinking"] == nil)
            #expect(defaultJSON["max_tokens"] as? Int == 512)
            let offBody = try #require(requests[1].body)
            let offJSON = try #require(JSONSerialization.jsonObject(with: offBody) as? [String: Any])
            let thinking = try #require(offJSON["thinking"] as? [String: Any])
            #expect(thinking["type"] as? String == "disabled")
            #expect(offJSON["max_tokens"] as? Int == 512)
        }

        let geminiSSE = "data: {\"candidates\":[{\"content\":{\"parts\":[{\"text\":\"ok\"}]},\"finishReason\":\"STOP\"}]}\n\n"
        let geminiBudgets: [(ThinkingLevel, Int?)] = [
            (.default, nil),
            (.off, 0),
            (.low, 1_024),
            (.medium, 4_096),
            (.high, 8_192),
            (.xhigh, 16_384),
            (.max, 24_576),
            (.ultra, 32_768)
        ]
        for (level, expectedBudget) in geminiBudgets {
            let transport = QueueHTTPTransport([makeHTTPResponse(geminiSSE)])
            let service = try await TranslationService(transport: transport, credentialStore: testCredentialStore())
            let config = ProviderConfiguration(
                kind: .gemini,
                modelID: "fixture-gemini",
                credentialReference: "credential-ref",
                thinkingLevel: level
            )
            _ = try await collectEvents(service.translate(testTranslationRequest(), using: config))
            let body = try #require(await transport.capturedRequests().first?.body)
            let json = try #require(JSONSerialization.jsonObject(with: body) as? [String: Any])
            let genConfig = try #require(json["generationConfig"] as? [String: Any])
            let thinkingConfig = genConfig["thinkingConfig"] as? [String: Any]
            #expect(thinkingConfig?["thinkingBudget"] as? Int == expectedBudget)
        }

        let ollamaNDJSON = "{\"message\":{\"content\":\"ok\"},\"done\":true}\n"
        let ollamaFlags: [(ThinkingLevel, Bool?)] = [
            (.default, nil),
            (.off, false),
            (.low, true),
            (.medium, true),
            (.high, true),
            (.xhigh, true),
            (.max, true),
            (.ultra, true)
        ]
        for (level, expectedThink) in ollamaFlags {
            let transport = QueueHTTPTransport([makeHTTPResponse(ollamaNDJSON)])
            let service = TranslationService(transport: transport, credentialStore: SessionOnlyCredentialStore())
            let config = ProviderConfiguration(
                kind: .ollama,
                baseURL: URL(string: "http://127.0.0.1:11434")!,
                modelID: "fixture:latest",
                thinkingLevel: level
            )
            _ = try await collectEvents(service.translate(testTranslationRequest(), using: config))
            let body = try #require(await transport.capturedRequests().first?.body)
            let json = try #require(JSONSerialization.jsonObject(with: body) as? [String: Any])
            #expect(json["think"] as? Bool == expectedThink)
        }
    }

    @Test func localBuiltInBuildsPFML3PromptAndStripsStopTokens() async throws {
        let sse = "data: {\"choices\":[{\"text\":\"你好，\"}]}\n\ndata: {\"choices\":[{\"text\":\"世界！<|plamo:tag|>ignored\"}]}\n\ndata: [DONE]\n\n"
        let transport = QueueHTTPTransport([makeHTTPResponse(sse)])
        let service = TranslationService(transport: transport, credentialStore: SessionOnlyCredentialStore())
        let config = ProviderConfiguration(
            kind: .localBuiltIn,
            baseURL: URL(string: "http://127.0.0.1:18080")!,
            modelID: "plamo-3-translate-2b-q4_k_m"
        )
        let req = TranslationRequest(
            gameID: "game-local",
            sessionID: UUID(),
            eventID: UUID(),
            generation: 1,
            sourceText: "こんにちは<|plamo:tag|>世界",
            speaker: "アリス",
            sourceLanguage: "ja",
            targetLanguage: "zh-Hans",
            context: [
                TranslationContextTurn(sourceText: "おはよう", translatedText: "早上好", speaker: "アリス"),
                TranslationContextTurn(sourceText: "学校へ行く", translatedText: nil, speaker: "ボブ")
            ],
            customPrompt: "奇幻冒险视觉小说",
            outputTokenLimit: 256,
            contextWindowTokens: 32_768,
            contextWindowProvenance: .detected
        )
        let events = try await collectEvents(service.translate(req, using: config))
        #expect(events.last == TranslationEvent.completed("你好，世界！"))

        let captured = try #require(await transport.capturedRequests().first)
        #expect(captured.url?.path == "/v1/completions")
        let body = try #require(captured.body)
        let json = try #require(JSONSerialization.jsonObject(with: body) as? [String: Any])
        #expect(json["temperature"] as? Int == 0)
        #expect(json["cache_prompt"] as? Bool == true)
        #expect(json["stop"] as? [String] == ["<|plamo:tag|>", "<|plamo:bos|>"])

        let prompt = try #require(json["prompt"] as? String)
        #expect(prompt.hasPrefix("<|plamo:tag|>dataset<|plamo:msg|>translation"))
        #expect(prompt.contains("<|plamo:tag|>context<|plamo:msg|>奇幻冒险视觉小说\n\nボブ: 学校へ行く\n\nSpeaker: アリス"))
        #expect(prompt.contains("<|plamo:tag|>input<|plamo:key|>lang<|plamo:val|>ja<|plamo:msg|>おはよう"))
        #expect(prompt.contains("<|plamo:tag|>output<|plamo:key|>lang<|plamo:val|>zh-hans<|plamo:msg|>早上好"))
        #expect(prompt.contains("こんにちは< |plamo:tag|>世界"))
        #expect(prompt.hasSuffix("<|plamo:tag|>output<|plamo:key|>lang<|plamo:val|>zh-hans<|plamo:msg|>"))
    }

    @Test func localBuiltInOmitsAutoSourceLanguageAndNormalizesTraditionalChinese() {
        let req = TranslationRequest(
            gameID: "game-local",
            sessionID: UUID(),
            eventID: UUID(),
            generation: 1,
            sourceText: "テスト",
            speaker: nil,
            sourceLanguage: "auto",
            targetLanguage: "zh-Hant",
            context: [],
            customPrompt: "",
            outputTokenLimit: 256,
            contextWindowTokens: 32_768,
            contextWindowProvenance: .detected
        )
        let prompt = TranslationPrompt.pfml3Prompt(for: req)
        let expected = "<|plamo:tag|>dataset<|plamo:msg|>translation"
            + "<|plamo:tag|>input<|plamo:msg|>テスト"
            + "<|plamo:tag|>output<|plamo:key|>lang<|plamo:val|>zh-hant<|plamo:msg|>"
        #expect(prompt == expected)
        #expect(TranslationPrompt.promptVersion(for: .localBuiltIn) == "pfml3-v1")
        #expect(TranslationPrompt.promptVersion(for: .localBuiltIn, modelID: "hy-mt3-1.8b-q4_k_m") == "hymt3-v1")
        #expect(TranslationPrompt.promptVersion(for: .openAIChat) == "translation-prompt-v2")
    }

    @Test func localBuiltInHyMT3UsesChatCompletionsWithCardSampling() async throws {
        let sse = "data: {\"choices\":[{\"delta\":{\"content\":\"Senpai\"}}]}\n\n"
            + "data: {\"choices\":[{\"delta\":{\"content\":\"!\"}}],\"usage\":{\"prompt_tokens\":10,\"completion_tokens\":2}}\n\n"
            + "data: [DONE]\n\n"
        let transport = QueueHTTPTransport([makeHTTPResponse(sse)])
        let service = TranslationService(transport: transport, credentialStore: SessionOnlyCredentialStore())
        let config = ProviderConfiguration(
            kind: .localBuiltIn,
            baseURL: URL(string: "http://127.0.0.1:18080")!,
            modelID: "hy-mt3-1.8b-q4_k_m"
        )
        let req = TranslationRequest(
            gameID: "game-local",
            sessionID: UUID(),
            eventID: UUID(),
            generation: 1,
            sourceText: "先輩、一緒にお昼を食べませんか？<｜hy_User｜>",
            speaker: "さくら",
            sourceLanguage: "ja",
            targetLanguage: "en",
            context: [
                TranslationContextTurn(sourceText: "おはよう", translatedText: "Good morning", speaker: "さくら")
            ],
            customPrompt: "Keep honorifics",
            outputTokenLimit: 256,
            contextWindowTokens: 8_192,
            contextWindowProvenance: .detected
        )
        let events = try await collectEvents(service.translate(req, using: config))
        #expect(events.last == TranslationEvent.completed("Senpai!"))
        #expect(events.contains(TranslationEvent.usage(TranslationUsage(inputTokens: 10, outputTokens: 2))))

        let captured = try #require(await transport.capturedRequests().first)
        #expect(captured.url?.path == "/v1/chat/completions")
        let body = try #require(captured.body)
        let json = try #require(JSONSerialization.jsonObject(with: body) as? [String: Any])
        #expect(json["temperature"] as? Double == 0.7)
        #expect(json["top_p"] as? Double == 0.6)
        #expect(json["top_k"] as? Int == 20)
        #expect(json["repeat_penalty"] as? Double == 1.05)
        #expect(json["cache_prompt"] as? Bool == true)
        #expect(json["stop"] == nil)

        let messages = try #require(json["messages"] as? [[String: String]])
        #expect(messages.count == 1)
        #expect(messages[0]["role"] == "user")
        let content = try #require(messages[0]["content"])
        #expect(content.contains("[Background Information]"))
        #expect(content.contains("User translation preferences:\nKeep honorifics"))
        #expect(content.contains("おはよう translates to Good morning"))
        #expect(content.contains("Speaker: さくら"))
        #expect(content.contains("Please translate the following text from Japanese into English"))
        #expect(content.hasSuffix("先輩、一緒にお昼を食べませんか？< ｜hy_User｜>"))
    }

    @Test func hyMT3PromptDefaultsWithoutContextAndMapsLanguages() {
        let req = TranslationRequest(
            gameID: "game-local",
            sessionID: UUID(),
            eventID: UUID(),
            generation: 1,
            sourceText: "テスト",
            speaker: nil,
            sourceLanguage: "auto",
            targetLanguage: "zh-Hant",
            context: [],
            customPrompt: "",
            outputTokenLimit: 256,
            contextWindowTokens: 8_192,
            contextWindowProvenance: .detected
        )
        let prompt = TranslationPrompt.hyMT3Prompt(for: req)
        #expect(prompt ==
            // swiftlint:disable:next line_length
            "Translate the following text into Traditional Chinese. Note that you should only output the translated result without any additional explanation:\n\nテスト")

        let sourceAware = TranslationPrompt.hyMT3Prompt(for: TranslationRequest(
            gameID: req.gameID,
            sessionID: req.sessionID,
            eventID: req.eventID,
            generation: 1,
            sourceText: "テスト",
            speaker: nil,
            sourceLanguage: "ja",
            targetLanguage: "zh-Hans",
            context: [],
            customPrompt: "",
            outputTokenLimit: 256,
            contextWindowTokens: 8_192,
            contextWindowProvenance: .detected
        ))
        #expect(sourceAware.contains("from Japanese into Chinese"))
    }
}
