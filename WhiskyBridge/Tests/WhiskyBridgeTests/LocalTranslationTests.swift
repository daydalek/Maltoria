//
//  LocalTranslationTests.swift
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
import WhiskyBridgeProtocol
import WhiskyTranslation
#if canImport(CryptoKit)
import CryptoKit
#endif
#if canImport(FoundationNetworking)
import FoundationNetworking
#endif

private final class FixtureURLProtocol: URLProtocol, @unchecked Sendable {
    nonisolated(unsafe) static var payloads: [URL: Data] = [:]

    override static func canInit(with request: URLRequest) -> Bool {
        guard let url = request.url else { return false }
        return payloads[url] != nil
    }

    override static func canonicalRequest(for request: URLRequest) -> URLRequest {
        request
    }

    override func startLoading() {
        guard let url = request.url, let data = Self.payloads[url] else {
            client?.urlProtocol(self, didFailWithError: URLError(.fileDoesNotExist))
            return
        }
        var offset = 0
        var statusCode = 200
        if let range = request.value(forHTTPHeaderField: "Range"),
           range.hasPrefix("bytes="),
           let parsed = Int(range.dropFirst("bytes=".count).split(separator: "-").first ?? ""),
           parsed > 0, parsed < data.count {
            offset = parsed
            statusCode = 206
        }
        let slice = data.subdata(in: offset ..< data.count)
        let headers = ["Content-Length": String(slice.count)]
        if let response = HTTPURLResponse(
            url: url,
            statusCode: statusCode,
            httpVersion: "HTTP/1.1",
            headerFields: headers
        ) {
            client?.urlProtocol(self, didReceive: response, cacheStoragePolicy: .notAllowed)
        }
        client?.urlProtocol(self, didLoad: slice)
        client?.urlProtocolDidFinishLoading(self)
    }

    override func stopLoading() {}
}

private func sha256Hex(_ data: Data) -> String {
    #if canImport(CryptoKit)
    return SHA256.hash(data: data).map { String(format: "%02x", $0) }.joined()
    #else
    return ""
    #endif
}

private func makeRuntimeTarGz(binaryScript: String) async throws -> Data {
    let fileManager = FileManager.default
    let dir = fileManager.temporaryDirectory.appendingPathComponent(
        "local-rt-fixture-\(UUID().uuidString)",
        isDirectory: true
    )
    let inner = dir.appendingPathComponent("llama-fixture/bin", isDirectory: true)
    try fileManager.createDirectory(at: inner, withIntermediateDirectories: true)
    defer { try? fileManager.removeItem(at: dir) }

    let bin = inner.appendingPathComponent("llama-server")
    try Data(binaryScript.utf8).write(to: bin)
    try fileManager.setAttributes([.posixPermissions: 0o755], ofItemAtPath: bin.path)

    let archive = dir.appendingPathComponent("runtime.tar.gz")
    let tarPath = fileManager.isExecutableFile(atPath: "/usr/bin/tar") ? "/usr/bin/tar" : "/bin/tar"
    let result = try await HostCommand.run(
        executable: URL(fileURLWithPath: tarPath),
        arguments: ["-czf", archive.path, "-C", dir.path, "llama-fixture"],
        environment: ProcessInfo.processInfo.environment,
        timeout: 30
    )
    #expect(result.status == 0)
    return try Data(contentsOf: archive)
}

private let mockLlamaServerScript = """
#!/usr/bin/env python3
import http.server, json, sys

args = sys.argv[1:]
port = int(args[args.index('--port') + 1])
expected_key = args[args.index('--api-key') + 1]

class Handler(http.server.BaseHTTPRequestHandler):
    def log_message(self, fmt, *args):
        pass
    def do_GET(self):
        if self.path == '/health':
            self.send_response(200)
            self.end_headers()
            self.wfile.write(b'{"status":"ok"}')
        else:
            self.send_response(404)
            self.end_headers()
    def do_POST(self):
        auth = self.headers.get('Authorization', '')
        if auth != f'Bearer {expected_key}':
            self.send_response(401)
            self.end_headers()
            return
        length = int(self.headers.get('Content-Length', '0'))
        payload = json.loads(self.rfile.read(length).decode('utf-8'))
        prompt = payload.get('prompt', '')
        ok_pfml = '<|plamo:tag|>dataset<|plamo:msg|>translation' in prompt
        text = '你好，世界。' if ok_pfml else 'BAD_PROMPT'
        self.send_response(200)
        self.send_header('Content-Type', 'text/event-stream')
        self.end_headers()
        chunk = json.dumps({'choices': [{'text': text}]}).encode('utf-8')
        self.wfile.write(b'data: ' + chunk + b'\\n\\n')
        self.wfile.write(b'data: [DONE]\\n\\n')
        self.wfile.flush()

http.server.HTTPServer(('127.0.0.1', port), Handler).serve_forever()
"""

@Suite("Local translation catalog, store, and runtime", .serialized)
struct LocalTranslationTests {
    @Test func catalogResolvesMirrorAndEnvOverride() {
        let model = LocalTranslationCatalog.defaultModels[0]
        #expect(
            model.downloadURL(useChinaMirror: false, environment: [:])?.absoluteString
                == "https://huggingface.co/pfnet/plamo-3-translate-2b-GGUF/resolve/main/plamo-3-translate-2b-Q4_K_M.gguf?download=true"
        )
        #expect(
            model.downloadURL(useChinaMirror: true, environment: [:])?.absoluteString
                == "https://hf-mirror.com/pfnet/plamo-3-translate-2b-GGUF/resolve/main/plamo-3-translate-2b-Q4_K_M.gguf?download=true"
        )
        #expect(
            model.downloadURL(
                useChinaMirror: false,
                environment: ["MALTORIA_HF_ENDPOINT": "https://mirror.example.org/"]
            )?.absoluteString
                == "https://mirror.example.org/pfnet/plamo-3-translate-2b-GGUF/resolve/main/plamo-3-translate-2b-Q4_K_M.gguf?download=true"
        )
    }

    @Test func storeDownloadsVerifiesSHA256RejectsCorruptionAndDeletes() async throws {
        #if canImport(CryptoKit)
        let root = FileManager.default.temporaryDirectory
            .appendingPathComponent("maltoria-local-store-\(UUID().uuidString)", isDirectory: true)
        defer { try? FileManager.default.removeItem(at: root) }

        let runtimeTar = try await makeRuntimeTarGz(binaryScript: "#!/bin/sh\nexit 0\n")
        let modelBytes = Data("GGUF-FIXTURE-PAYLOAD-FOR-LOCAL-TEST".utf8)
        let runtimeHash = sha256Hex(runtimeTar)
        let modelHash = sha256Hex(modelBytes)

        let runtimeURL = try #require(URL(string: "https://fixture.local/llama-runtime.tar.gz"))
        let modelEntry = LocalModelCatalogEntry(
            id: "fixture-plamo",
            displayName: "Fixture PLaMo",
            subtitle: "Test",
            recommended: true,
            repoID: "pfnet/fixture",
            filename: "fixture.gguf",
            sizeBytes: Int64(modelBytes.count),
            sha256: modelHash,
            contextWindow: 32_768,
            recommendedRamGB: 4,
            licenseName: "PLaMo Community License",
            licenseURL: "https://plamo.preferredai.jp/info/plamo-community-license-en"
        )
        let modelURL = try #require(modelEntry.downloadURL(useChinaMirror: false, environment: [:]))

        FixtureURLProtocol.payloads = [runtimeURL: runtimeTar, modelURL: modelBytes]
        defer { FixtureURLProtocol.payloads = [:] }

        let sessionConfig = URLSessionConfiguration.ephemeral
        sessionConfig.protocolClasses = [FixtureURLProtocol.self]

        let catalog = LocalTranslationCatalog(
            runtimeAsset: LocalRuntimeAsset(
                version: "fixture-1",
                platform: "test",
                filename: "llama-runtime.tar.gz",
                downloadURL: runtimeURL.absoluteString,
                sizeBytes: Int64(runtimeTar.count),
                sha256: runtimeHash,
                gpuAcceleration: "cpu"
            ),
            models: [modelEntry]
        )
        let store = LocalTranslationStore(
            stateDirectory: root,
            modelsDirectory: root.appendingPathComponent("models", isDirectory: true),
            runtimeDirectory: root.appendingPathComponent("runtime", isDirectory: true),
            catalog: catalog,
            sessionConfiguration: sessionConfig
        )

        let downloaded = try await store.downloadModel(
            id: "fixture-plamo",
            useChinaMirror: false,
            jobID: "job-1"
        ) { _ in }
        #expect(FileManager.default.fileExists(atPath: downloaded.path))
        #expect(await store.isModelInstalled(modelEntry))
        #expect(await store.installedRuntime()?.manifest.version == "fixture-1")

        #expect(try await store.deleteModel(id: "fixture-plamo") == true)
        #expect(await store.isModelInstalled(modelEntry) == false)

        // Corrupt payload must fail closed on SHA-256 mismatch
        FixtureURLProtocol.payloads[modelURL] = Data("CORRUPTED-BYTES".utf8)
        await #expect(throws: BridgeFailure.self) {
            _ = try await store.downloadModel(
                id: "fixture-plamo",
                useChinaMirror: false,
                jobID: "job-2"
            ) { _ in }
        }
        #expect(await store.isModelInstalled(modelEntry) == false)
        #endif
    }

    @Test @MainActor func applicationServiceExposesLocalConfigStatusAndMockRuntimeInference() async throws {
        #if canImport(CryptoKit)
        let root = FileManager.default.temporaryDirectory
            .appendingPathComponent("maltoria-local-svc-\(UUID().uuidString)", isDirectory: true)
        defer { try? FileManager.default.removeItem(at: root) }

        let runtimeTar = try await makeRuntimeTarGz(binaryScript: mockLlamaServerScript)
        let modelBytes = Data("MOCK-GGUF".utf8)
        let runtimeURL = try #require(URL(string: "https://fixture.local/mock-llama.tar.gz"))
        let modelEntry = LocalModelCatalogEntry(
            id: "plamo-3-translate-0.8b-q4_k_m",
            displayName: "PLaMo 3 Translate 0.8B (Q4_K_M)",
            subtitle: "Mock",
            recommended: false,
            repoID: "pfnet/mock",
            filename: "mock.gguf",
            sizeBytes: Int64(modelBytes.count),
            sha256: sha256Hex(modelBytes),
            contextWindow: 32_768,
            recommendedRamGB: 4,
            licenseName: "PLaMo Community License",
            licenseURL: "https://plamo.preferredai.jp/info/plamo-community-license-en"
        )
        let modelURL = try #require(modelEntry.downloadURL(useChinaMirror: false, environment: [:]))
        FixtureURLProtocol.payloads = [runtimeURL: runtimeTar, modelURL: modelBytes]
        defer { FixtureURLProtocol.payloads = [:] }

        let sessionConfig = URLSessionConfiguration.ephemeral
        sessionConfig.protocolClasses = [FixtureURLProtocol.self]

        let catalog = LocalTranslationCatalog(
            runtimeAsset: LocalRuntimeAsset(
                version: "mock-1",
                platform: "test",
                filename: "mock-llama.tar.gz",
                downloadURL: runtimeURL.absoluteString,
                sizeBytes: Int64(runtimeTar.count),
                sha256: sha256Hex(runtimeTar),
                gpuAcceleration: "cpu"
            ),
            models: [modelEntry]
        )

        let service = ApplicationTranslationService(
            directory: root,
            localCatalog: catalog,
            localModelsDirectory: root.appendingPathComponent("models", isDirectory: true),
            localRuntimeDirectory: root.appendingPathComponent("runtime", isDirectory: true),
            localURLSessionConfiguration: sessionConfig
        )
        defer {
            Task { await service.shutdown() }
        }

        // Toggle China mirror preference and verify persistence
        let updated = try await service.call("translation.local.setConfig", params: ["useChinaMirror": .bool(true)])
        #expect(updated.object?["useChinaMirror"]?.bool == true)
        #expect(updated.object?["hfEndpoint"]?.string == "https://hf-mirror.com")

        // Reset mirror to false so FixtureURLProtocol URL matches default hf URL
        _ = try await service.call("translation.local.setConfig", params: ["useChinaMirror": .bool(false)])

        // Download directly via store, then verify testConnection launches mock llama-server and translates
        _ = try await service.localStore.downloadModel(
            id: "plamo-3-translate-0.8b-q4_k_m",
            useChinaMirror: false,
            jobID: "job-mock"
        ) { _ in }

        let savedProvider = try await service.call("translation.providers/save", params: [
            "provider": .object([
                "id": .string(UUID().uuidString.lowercased()),
                "name": .string("Local PLaMo"),
                "kind": .string("localBuiltIn"),
                "baseURL": .string("http://127.0.0.1:18080"),
                "modelID": .string("plamo-3-translate-0.8b-q4_k_m"),
                "defaultSourceLanguage": .string("ja"),
                "defaultTargetLanguage": .string("zh-Hans"),
                "openAIChatTokenLimitField": .string("max_completion_tokens"),
                "thinkingLevel": .string("default")
            ])
        ])
        let providerID = try #require(savedProvider.object?["id"]?.string)

        let testResult = try await service.call("translation.testConnection", params: [
            "providerID": .string(providerID)
        ])
        #expect(testResult.object?["translation"]?.string == "你好，世界。")

        let runningStatus = try await service.call("translation.local.status", params: [:])
        #expect(runningStatus.object?["runtime"]?.object?["running"]?.bool == true)
        #expect(runningStatus.object?["runtime"]?.object?["loadedModelID"]?.string == "plamo-3-translate-0.8b-q4_k_m")

        let unloadedStatus = try await service.call("translation.local.unload", params: [:])
        #expect(unloadedStatus.object?["runtime"]?.object?["running"]?.bool == false)
        await service.shutdown()
        #endif
    }
}
