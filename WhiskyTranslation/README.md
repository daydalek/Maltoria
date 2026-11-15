# WhiskyTranslation

`WhiskyTranslation` is a Swift 6 package shared by macOS and Linux (macOS deployment target 15). It contains provider-neutral translation models, model discovery, streaming adapters, local session/history management, context budgeting, credentials, and tests. It has no GPUI, Wine, or hook dependency.

## Providers and transport

The package implements OpenAI-compatible Chat Completions (default), explicit legacy `/completions`, Ollama native chat, Anthropic Messages, Gemini `streamGenerateContent` (SSE), and DeepL `/v2/translate` (JSON response). DeepL does not advertise a model list. OpenAI `/models` discovers IDs only; it does not report standard context-window metadata or prove that a model accepts chat. Call `TranslationService.testConnection` explicitly to test an actual selected model.

The adapters use Foundation `URLSession` through a bounded `URLSessionDataDelegate` transport, with `FoundationNetworking` on Linux. MacPaw/OpenAI and SwiftOpenAI were evaluated briefly; neither provides the native Anthropic, Gemini, Ollama, and DeepL wire protocols needed here. SwiftOpenAI also brings a second Linux HTTP stack. No SDK was added: narrowly scoped adapters plus Foundation transport preserve custom endpoints, cancellation, fixture testing, and a dependency-free package. No new third-party notices are required.

Provider configurations store only a credential reference. macOS uses Keychain; Linux reports session-only credential availability and does not claim Secret Service persistence. The default repository writes atomic JSON under the user's application/state directory with private directory/file permissions. API keys are sent in provider-specific headers, never URL query strings, logs, settings, or history. Remote cleartext HTTP and URL credentials are rejected; loopback HTTP is allowed for local Ollama-compatible endpoints.

## Context and history

Context provenance is `detected`, `manual`, or `unknown`. Only documented metadata is treated as detected: Ollama `/api/show` model metadata, Anthropic model `max_input_tokens`, and Gemini `inputTokenLimit`. Standard OpenAI model listing remains unknown. Manual override takes precedence; unknown models use a 4096-token *estimated* budget with output reserve. The implementation uses a conservative UTF-8 byte budget, not provider tokenization. Old selected context is trimmed first; current source text is rejected with `sourceExceedsBudget` rather than truncated. Omitting session settings seeds source/target language from the provider configuration; explicit settings remain authoritative.

History is isolated by game/session, retained locally according to `historyRetentionTurns` (zero disables disk retention), and each record can be excluded from context. Source and translated history can be included independently. Custom prompt preferences are separate from source dialogue; legacy completions use a deterministic JSON-serialized prompt template. Cache keys include provider instance, endpoint/model, language pair, source/speaker, custom prompt, selected-context digest, prompt/normalizer versions, and generation limits.

## Build and tests

```sh
swift test --package-path WhiskyTranslation
./test.sh swift translation
```

Tests use injected HTTP fixtures and deterministic test-only providers; they make no real provider calls and require no credentials. Official API references used for wire shapes:

- [OpenAI Chat Completions](https://platform.openai.com/docs/api-reference/chat/create)
- [OpenAI model list](https://platform.openai.com/docs/api-reference/models/list)
- [Anthropic streaming](https://platform.claude.com/docs/en/api/streaming) and [models](https://platform.claude.com/docs/en/api/models/list)
- [Gemini content generation](https://ai.google.dev/api/generate-content)
- [Ollama chat](https://docs.ollama.com/api/chat), [tags](https://docs.ollama.com/api/tags), and [show model details](https://docs.ollama.com/api-reference/show-model-details)
- [DeepL translate](https://developers.deepl.com/api-reference/translate) and [context parameter](https://developers.deepl.com/docs/learning-how-tos/examples-and-guides/how-to-use-context-parameter)

This package does not establish provider-account availability, game-hook compatibility, Wine behavior, or overlay behavior; those belong to later integration stages.
