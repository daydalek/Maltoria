//
//  LocalTranslationRuntime.swift
//  WhiskyApplication
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
import WhiskyBridgeProtocol
import WhiskyTranslation
#if canImport(Darwin)
import Darwin
#elseif canImport(Glibc)
import Glibc
#endif
#if canImport(FoundationNetworking)
import FoundationNetworking
#endif

public struct LocalRuntimeSnapshot: Equatable, Sendable {
    public let runtimeInstalled: Bool
    public let runtimeVersion: String?
    public let gpuAcceleration: String?
    public let running: Bool
    public let loadedModelID: String?
    public let port: Int?
    public let lastError: String?
}

public actor LocalTranslationRuntime {
    private let store: LocalTranslationStore
    private let idleTimeoutSeconds: TimeInterval
    private let healthTimeoutSeconds: TimeInterval
    private let healthSession: URLSession

    private var process: Process?
    private var loadedModelID: String?
    private var port: Int?
    private var apiKey: String?
    private var lastError: String?
    private var stderrTail = BoundedLogBuffer(maxBytes: 16_384)
    private var idleTask: Task<Void, Never>?
    private var startupTask: Task<(baseURL: URL, apiKey: String), any Error>?
    private var startupModelID: String?

    public init(
        store: LocalTranslationStore,
        idleTimeoutSeconds: TimeInterval = 300,
        healthTimeoutSeconds: TimeInterval = 90
    ) {
        self.store = store
        self.idleTimeoutSeconds = max(10, idleTimeoutSeconds)
        self.healthTimeoutSeconds = max(5, healthTimeoutSeconds)
        let config = URLSessionConfiguration.ephemeral
        config.timeoutIntervalForRequest = 3
        config.timeoutIntervalForResource = 5
        self.healthSession = URLSession(configuration: config)
    }

    public func snapshot() async -> LocalRuntimeSnapshot {
        let installed = await store.installedRuntime()
        let isRunning = process?.isRunning == true
        if !isRunning, process != nil {
            cleanupDeadProcess()
        }
        return LocalRuntimeSnapshot(
            runtimeInstalled: installed != nil,
            runtimeVersion: installed?.manifest.version,
            gpuAcceleration: installed?.manifest.gpuAcceleration,
            running: isRunning,
            loadedModelID: isRunning ? loadedModelID : nil,
            port: isRunning ? port : nil,
            lastError: lastError
        )
    }

    public func ensureRunning(modelID: String) async throws -> (baseURL: URL, apiKey: String) {
        if let proc = process, proc.isRunning,
           loadedModelID == modelID,
           let port, let apiKey,
           let url = URL(string: "http://127.0.0.1:\(port)") {
            touchActivity()
            return (url, apiKey)
        }

        if let existingTask = startupTask, startupModelID == modelID {
            return try await existingTask.value
        }

        if let existingTask = startupTask {
            existingTask.cancel()
            _ = try? await existingTask.value
            startupTask = nil
            startupModelID = nil
        }

        await unload()

        guard let entry = store.catalog.model(id: modelID) else {
            throw TranslationError.invalidModel
        }
        guard await store.isModelInstalled(entry) else {
            throw TranslationError.invalidConfiguration(
                "本地模型 \(entry.displayName) 尚未下载，请先在翻译设置中下载模型。"
            )
        }
        guard let installed = await store.installedRuntime() else {
            throw TranslationError.invalidConfiguration(
                "本地推理引擎尚未就绪，请先在翻译设置中下载模型以自动安装引擎。"
            )
        }

        let modelURL = await store.modelFileURL(for: entry)
        let binaryURL = installed.binaryURL
        let gpuAcceleration = installed.manifest.gpuAcceleration

        startupModelID = modelID
        let task = Task<(baseURL: URL, apiKey: String), any Error> {
            try await self.launchAndWait(
                modelID: modelID,
                modelURL: modelURL,
                binaryURL: binaryURL,
                contextWindow: entry.contextWindow,
                gpuAcceleration: gpuAcceleration
            )
        }
        startupTask = task
        defer {
            if startupModelID == modelID {
                startupTask = nil
                startupModelID = nil
            }
        }
        return try await task.value
    }

    public func touchActivity() {
        idleTask?.cancel()
        guard process?.isRunning == true else { return }
        let timeout = idleTimeoutSeconds
        idleTask = Task { [weak self] in
            do {
                try await Task.sleep(nanoseconds: UInt64(timeout * 1_000_000_000))
                await self?.unload()
            } catch {}
        }
    }

    public func unload() async {
        idleTask?.cancel()
        idleTask = nil
        guard let proc = process else {
            loadedModelID = nil
            port = nil
            apiKey = nil
            return
        }
        process = nil
        loadedModelID = nil
        port = nil
        apiKey = nil

        if proc.isRunning {
            proc.terminate()
            let deadline = Date().addingTimeInterval(3.0)
            while proc.isRunning, Date() < deadline {
                try? await Task.sleep(nanoseconds: 50_000_000)
            }
            if proc.isRunning {
                #if canImport(Darwin) || canImport(Glibc)
                kill(proc.processIdentifier, SIGKILL)
                #endif
            }
        }
    }

    public func shutdown() async {
        startupTask?.cancel()
        startupTask = nil
        startupModelID = nil
        await unload()
        healthSession.invalidateAndCancel()
    }

    private func launchAndWait(
        modelID: String,
        modelURL: URL,
        binaryURL: URL,
        contextWindow: Int,
        gpuAcceleration: String
    ) async throws -> (baseURL: URL, apiKey: String) {
        let allocatedPort = try Self.allocateEphemeralLoopbackPort()
        let token = Self.randomAPIKey()
        let buffer = BoundedLogBuffer(maxBytes: 16_384)
        stderrTail = buffer
        lastError = nil

        let proc = Process()
        proc.executableURL = binaryURL
        proc.currentDirectoryURL = binaryURL.deletingLastPathComponent()

        var args = [
            "-m", modelURL.path,
            "--host", "127.0.0.1",
            "--port", String(allocatedPort),
            "--api-key", token,
            "-c", String(contextWindow),
            "-np", "1",
            "--no-webui"
        ]
        if gpuAcceleration == "metal" || gpuAcceleration == "vulkan" {
            args += ["-ngl", "99"]
        } else {
            args += ["-ngl", "0"]
        }
        proc.arguments = args

        var env = ProcessInfo.processInfo.environment
        let libDir = binaryURL.deletingLastPathComponent().path
        #if os(Linux)
        let existing = env["LD_LIBRARY_PATH"] ?? ""
        env["LD_LIBRARY_PATH"] = existing.isEmpty ? libDir : "\(libDir):\(existing)"
        #elseif os(macOS)
        let existing = env["DYLD_LIBRARY_PATH"] ?? ""
        env["DYLD_LIBRARY_PATH"] = existing.isEmpty ? libDir : "\(libDir):\(existing)"
        #endif
        proc.environment = env

        let pipe = Pipe()
        proc.standardInput = FileHandle.nullDevice
        proc.standardOutput = pipe
        proc.standardError = pipe
        pipe.fileHandleForReading.readabilityHandler = { handle in
            let data = handle.availableData
            if data.isEmpty {
                handle.readabilityHandler = nil
            } else {
                buffer.append(data)
            }
        }

        do {
            try proc.run()
        } catch {
            pipe.fileHandleForReading.readabilityHandler = nil
            lastError = error.localizedDescription
            throw TranslationError.invalidConfiguration("无法启动本地推理引擎：\(error.localizedDescription)")
        }

        process = proc
        loadedModelID = modelID
        port = allocatedPort
        apiKey = token

        guard let baseURL = URL(string: "http://127.0.0.1:\(allocatedPort)"),
              let healthURL = URL(string: "http://127.0.0.1:\(allocatedPort)/health")
        else {
            await unload()
            throw TranslationError.invalidConfiguration("Invalid loopback server URL")
        }

        let deadline = Date().addingTimeInterval(healthTimeoutSeconds)
        while Date() < deadline {
            try Task.checkCancellation()
            if !proc.isRunning {
                let diag = buffer.snapshot().trimmingCharacters(in: .whitespacesAndNewlines)
                let summary = diag.isEmpty ? "exit code \(proc.terminationStatus)" : String(diag.suffix(400))
                lastError = summary
                await unload()
                throw TranslationError.invalidConfiguration("本地推理引擎启动失败：\(summary)")
            }
            if await checkHealth(url: healthURL) {
                touchActivity()
                return (baseURL, token)
            }
            try await Task.sleep(nanoseconds: 150_000_000)
        }

        let diag = buffer.snapshot().trimmingCharacters(in: .whitespacesAndNewlines)
        lastError = diag.isEmpty ? "Timed out waiting for llama-server /health" : String(diag.suffix(400))
        await unload()
        throw TranslationError.invalidConfiguration("本地推理引擎启动超时，请检查内存是否充足。")
    }

    private func cleanupDeadProcess() {
        idleTask?.cancel()
        idleTask = nil
        process = nil
        loadedModelID = nil
        port = nil
        apiKey = nil
    }

    private func checkHealth(url: URL) async -> Bool {
        var req = URLRequest(url: url)
        req.httpMethod = "GET"
        req.timeoutInterval = 2
        return await withCheckedContinuation { continuation in
            let task = healthSession.dataTask(with: req) { _, response, _ in
                let status = (response as? HTTPURLResponse)?.statusCode ?? 0
                continuation.resume(returning: status == 200)
            }
            task.resume()
        }
    }

    private static func allocateEphemeralLoopbackPort() throws -> Int {
        #if canImport(Darwin)
        let sock = Darwin.socket(AF_INET, SOCK_STREAM, 0)
        #else
        let sock = Glibc.socket(AF_INET, Int32(SOCK_STREAM.rawValue), 0)
        #endif
        guard sock >= 0 else {
            throw TranslationError.transportFailure
        }
        defer { close(sock) }

        var addr = sockaddr_in()
        addr.sin_family = sa_family_t(AF_INET)
        addr.sin_port = 0
        addr.sin_addr = in_addr(s_addr: inet_addr("127.0.0.1"))
        var length = socklen_t(MemoryLayout<sockaddr_in>.size)

        let bindResult = withUnsafePointer(to: &addr) { ptr in
            ptr.withMemoryRebound(to: sockaddr.self, capacity: 1) { sockPtr in
                bind(sock, sockPtr, length)
            }
        }
        guard bindResult == 0 else {
            throw TranslationError.transportFailure
        }

        let nameResult = withUnsafeMutablePointer(to: &addr) { ptr in
            ptr.withMemoryRebound(to: sockaddr.self, capacity: 1) { sockPtr in
                getsockname(sock, sockPtr, &length)
            }
        }
        guard nameResult == 0 else {
            throw TranslationError.transportFailure
        }
        return Int(UInt16(bigEndian: addr.sin_port))
    }

    private static func randomAPIKey() -> String {
        var generator = SystemRandomNumberGenerator()
        return (0 ..< 4).map { _ in
            String(format: "%016llx", generator.next())
        }.joined()
    }
}

public struct LocalManagedTranslationProvider: TranslationProvider, Sendable {
    public let kind: ProviderKind = .localBuiltIn
    private let store: LocalTranslationStore
    private let runtime: LocalTranslationRuntime
    private let httpProvider: HTTPTranslationProvider

    public init(
        store: LocalTranslationStore,
        runtime: LocalTranslationRuntime,
        transport: any HTTPTransport
    ) {
        self.store = store
        self.runtime = runtime
        self.httpProvider = HTTPTranslationProvider(kind: .localBuiltIn, transport: transport)
    }

    public func discoverModels(
        configuration _: ProviderConfiguration,
        credential _: String?
    ) async throws -> [ModelDescriptor] {
        var descriptors: [ModelDescriptor] = []
        for entry in store.catalog.models where await store.isModelInstalled(entry) {
            descriptors.append(entry.modelDescriptor)
        }
        return descriptors
    }

    public func modelInfo(
        id: String,
        configuration _: ProviderConfiguration,
        credential _: String?
    ) async throws -> ModelDescriptor {
        guard let entry = store.catalog.model(id: id) else {
            throw TranslationError.invalidModel
        }
        return entry.modelDescriptor
    }

    public func translate(
        _ request: TranslationRequest,
        configuration: ProviderConfiguration,
        credential _: String?
    ) -> AsyncThrowingStream<TranslationEvent, any Error> {
        let store = self.store
        let runtime = self.runtime
        let httpProvider = self.httpProvider
        let (stream, continuation) = AsyncThrowingStream<TranslationEvent, any Error>
            .makeStream(bufferingPolicy: .bufferingOldest(256))
        let task = Task {
            do {
                guard let modelID = configuration.modelID, !modelID.isEmpty else {
                    throw TranslationError.missingModel
                }
                guard store.catalog.model(id: modelID) != nil else {
                    throw TranslationError.invalidModel
                }
                let live = try await runtime.ensureRunning(modelID: modelID)
                var liveConfig = configuration
                liveConfig.baseURL = live.baseURL
                let inner = httpProvider.translate(request, configuration: liveConfig, credential: live.apiKey)
                for try await event in inner {
                    switch continuation.yield(event) {
                    case .enqueued: break
                    case .dropped: throw TranslationError.streamConsumerTooSlow
                    case .terminated: throw CancellationError()
                    @unknown default: throw TranslationError.streamConsumerTooSlow
                    }
                }
                await runtime.touchActivity()
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
}

private final class BoundedLogBuffer: @unchecked Sendable {
    private let lock = NSLock()
    private let maxBytes: Int
    private var storage = Data()

    init(maxBytes: Int) {
        self.maxBytes = maxBytes
    }

    func append(_ data: Data) {
        lock.lock()
        storage.append(data)
        if storage.count > maxBytes {
            storage.removeFirst(storage.count - maxBytes)
        }
        lock.unlock()
    }

    func snapshot() -> String {
        lock.lock()
        let copy = storage
        lock.unlock()
        return String(decoding: copy, as: UTF8.self)
    }
}
