//
//  HyMT3EndToEndTests.swift
//  WhiskyBridgeTests
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
@testable import WhiskyApplication
import WhiskyTranslation
#if canImport(FoundationNetworking)
import FoundationNetworking
#endif

/// Real-runtime end-to-end check for the Hy-MT3 prompt family. Skipped unless
/// `MT3_E2E_MODEL` (path to a Hy-MT3 GGUF) and `MT3_E2E_RUNTIME_DIR` (directory
/// containing an extracted `llama-server` plus its shared libraries) are set.
@Suite("Hy-MT3 end-to-end (requires seeded model + runtime)")
struct HyMT3EndToEndTests {
    @Test func managedProviderTranslatesWithRealRuntime() async throws {
        let env = ProcessInfo.processInfo.environment
        guard let modelPath = env["MT3_E2E_MODEL"], !modelPath.isEmpty,
              let runtimeDir = env["MT3_E2E_RUNTIME_DIR"], !runtimeDir.isEmpty
        else {
            return
        }
        let modelID = "hy-mt3-1.8b-q4_k_m"
        let entry = try #require(LocalTranslationCatalog.default().model(id: modelID))

        let root = FileManager.default.temporaryDirectory
            .appendingPathComponent("maltoria-mt3-e2e-\(UUID().uuidString)", isDirectory: true)
        defer { try? FileManager.default.removeItem(at: root) }
        let modelsDir = root.appendingPathComponent("models", isDirectory: true)
        let runtimesDir = root.appendingPathComponent("runtime", isDirectory: true)
        let fileManager = FileManager.default
        try fileManager.createDirectory(at: modelsDir, withIntermediateDirectories: true)
        try fileManager.createDirectory(at: runtimesDir, withIntermediateDirectories: true)
        try fileManager.linkItem(
            at: URL(fileURLWithPath: modelPath),
            to: modelsDir.appendingPathComponent(entry.filename)
        )
        for item in try fileManager.contentsOfDirectory(atPath: runtimeDir) {
            try fileManager.linkItem(
                at: URL(fileURLWithPath: runtimeDir).appendingPathComponent(item),
                to: runtimesDir.appendingPathComponent(item)
            )
        }
        let manifest = InstalledLocalRuntime(
            version: "e2e-seeded",
            platform: "linux-x64-cpu",
            sha256: "",
            binaryRelativePath: "llama-server",
            gpuAcceleration: "cpu"
        )
        try JSONEncoder().encode(manifest)
            .write(to: runtimesDir.appendingPathComponent("active.json"))

        let store = LocalTranslationStore(
            stateDirectory: root,
            modelsDirectory: modelsDir,
            runtimeDirectory: runtimesDir,
            catalog: LocalTranslationCatalog.default()
        )
        let runtime = LocalTranslationRuntime(store: store, idleTimeoutSeconds: 60)
        let provider = LocalManagedTranslationProvider(
            store: store,
            runtime: runtime,
            transport: URLSessionStreamTransport()
        )
        defer { Task { await runtime.shutdown() } }

        let configuration = ProviderConfiguration(
            kind: .localBuiltIn,
            modelID: modelID,
            defaultSourceLanguage: "ja",
            defaultTargetLanguage: "en"
        )
        let request = TranslationRequest(
            gameID: "mt3-e2e",
            sessionID: UUID(),
            eventID: UUID(),
            generation: 1,
            sourceText: "さくら「先輩、一緒にお昼を食べませんか？」",
            speaker: "さくら",
            sourceLanguage: "ja",
            targetLanguage: "en",
            context: [],
            customPrompt: "",
            outputTokenLimit: 128,
            contextWindowTokens: 4_096,
            contextWindowProvenance: .detected
        )

        var completed: String?
        var sawDelta = false
        for try await event in provider.translate(request, configuration: configuration, credential: nil) {
            switch event.kind {
            case .delta: sawDelta = true
            case .completed: completed = event.text
            default: break
            }
        }
        let output = try #require(completed)
        #expect(sawDelta)
        #expect(!output.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty)
        #expect(output.range(of: "[\\p{Han}\\p{Hiragana}\\p{Katakana}]", options: .regularExpression) == nil)
        await runtime.shutdown()
    }
}
