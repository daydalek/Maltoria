import Foundation
import Testing
@testable import WhiskyTranslation

@Suite("Streaming protocol framing")
struct StreamParserTests {
    @Test func sseHandlesCRLFChunksAndSplitUnicode() throws {
        let payload = "data: {\"text\":\"你好🌸\"}\r\n\r\ndata: [DONE]\r\n\r\n"
        var parser = SSEParser()
        var frames: [SSEFrame] = []
        for byte in Data(payload.utf8) { frames += try parser.append(Data([byte])) }
        frames += try parser.finish()
        #expect(frames.count == 2)
        #expect(frames[0].data.contains("你好🌸"))
        #expect(frames[1].data == "[DONE]")
    }

    @Test func ndjsonHandlesSplitMultibyteAndFinalUnterminatedLine() {
        let data = Data("{\"text\":\"日本語🌙\"}\r\n{\"done\":true}".utf8)
        var parser = NDJSONParser()
        var lines: [Data] = []
        for byte in data { lines += parser.append(Data([byte])) }
        lines += parser.finish()
        #expect(lines.count == 2)
        #expect(String(decoding: lines[0], as: UTF8.self).contains("日本語🌙"))
    }

    @Test func invalidUTF8IsRejected() throws {
        var parser = SSEParser()
        do { _ = try parser.append(Data([0xFF, 0x0A])); Issue.record("invalid UTF-8 accepted") }
        catch let error as TranslationError { #expect(error == .invalidUTF8) }
    }

    @Test func contextBudgetDropsOldestSelectedTurnsBeforeCurrentSource() throws {
        let request = testTranslationRequest(
            sourceText: "current source",
            context: [
                TranslationContextTurn(sourceText: String(repeating: "old", count: 250), translatedText: "old translation"),
                TranslationContextTurn(sourceText: "recent source", translatedText: "recent translation")
            ],
            contextWindowTokens: 1_000,
            outputTokenLimit: 100
        )
        let fitted = try TranslationPrompt.fitting(request)
        #expect(fitted.sourceText == "current source")
        #expect(fitted.context.count == 1)
        #expect(fitted.context[0].sourceText == "recent source")
    }

    @Test func malformedAndTruncatedSSEAreNotTreatedAsCompletion() async throws {
        let malformed = makeHTTPResponse("data: {not-json}\n\n")
        let transport = QueueHTTPTransport([malformed])
        let service = TranslationService(transport: transport, credentialStore: try await testCredentialStore())
        let configuration = ProviderConfiguration(kind: .openAIChat, modelID: "fixture-model", credentialReference: "credential-ref")
        do {
            _ = try await collectEvents(service.translate(testTranslationRequest(), using: configuration))
            Issue.record("malformed stream accepted")
        } catch let error as TranslationError { #expect(error == .malformedResponse) }

        let truncated = makeHTTPResponse("data: {\"choices\":[{\"delta\":{\"content\":\"partial\"}}]}\n\n")
        let second = QueueHTTPTransport([truncated])
        let service2 = TranslationService(transport: second, credentialStore: try await testCredentialStore())
        do {
            _ = try await collectEvents(service2.translate(testTranslationRequest(), using: configuration))
            Issue.record("truncated stream accepted")
        } catch let error as TranslationError { #expect(error == .incompleteStream) }
    }

    @Test func positiveIntegerRejectsBooleansFractionsAndOutOfRangeValues() {
        #expect(positiveInteger(12) == 12)
        #expect(positiveInteger(true) == nil)
        #expect(positiveInteger(1.5) == nil)
        #expect(positiveInteger(0) == nil)
        #expect(positiveInteger(1_000_000_001) == nil)
        #expect(positiveInteger(UInt64.max) == nil)
    }

    @Test func actualInstructionsAndEncodedSourceCountTowardBudget() throws {
        let request = testTranslationRequest(
            sourceText: "短い",
            customPrompt: String(repeating: "instruction", count: 100),
            contextWindowTokens: 1_000,
            outputTokenLimit: 256
        )
        do { _ = try TranslationPrompt.fitting(request); Issue.record("oversized instructions fit by undercounting") }
        catch let error as TranslationError { #expect(error == .sourceExceedsBudget(maximumBytes: 616)) }
    }
}
