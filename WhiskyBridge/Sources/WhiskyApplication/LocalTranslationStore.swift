//
//  LocalTranslationStore.swift
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
#if canImport(CryptoKit)
import CryptoKit
#endif
#if canImport(FoundationNetworking)
import FoundationNetworking
#endif

public struct LocalDownloadProgress: Codable, Equatable, Sendable {
    public let stage: String
    public let downloadedBytes: Int64
    public let totalBytes: Int64
    public let bytesPerSecond: Int64
    public let etaSeconds: Int?
    public let jobID: String

    public init(
        stage: String,
        downloadedBytes: Int64,
        totalBytes: Int64,
        bytesPerSecond: Int64,
        etaSeconds: Int?,
        jobID: String
    ) {
        self.stage = stage
        self.downloadedBytes = downloadedBytes
        self.totalBytes = totalBytes
        self.bytesPerSecond = bytesPerSecond
        self.etaSeconds = etaSeconds
        self.jobID = jobID
    }

    public var jsonValue: JSONValue {
        .object([
            "stage": .string(stage),
            "downloadedBytes": .integer(downloadedBytes),
            "totalBytes": .integer(totalBytes),
            "bytesPerSecond": .integer(bytesPerSecond),
            "etaSeconds": etaSeconds.map { .integer(Int64($0)) } ?? .null,
            "jobID": .string(jobID)
        ])
    }
}

public struct LocalTranslationPreferences: Codable, Equatable, Sendable {
    public var useChinaMirror: Bool

    public init(useChinaMirror: Bool = false) {
        self.useChinaMirror = useChinaMirror
    }
}

public struct InstalledLocalRuntime: Codable, Equatable, Sendable {
    public let version: String
    public let platform: String
    public let sha256: String
    public let binaryRelativePath: String
    public let gpuAcceleration: String
}

public actor LocalTranslationStore {
    public let catalog: LocalTranslationCatalog
    public let stateDirectory: URL
    public let modelsDirectory: URL
    public let runtimeDirectory: URL
    private let fileManager: FileManager
    private let sessionConfiguration: URLSessionConfiguration
    private var activeDownloads: [String: LocalDownloadProgress] = [:]
    private var activeDownloadTokens: [String: UUID] = [:]
    private var lastDownloadErrors: [String: String] = [:]

    public init(
        stateDirectory: URL,
        modelsDirectory: URL? = nil,
        runtimeDirectory: URL? = nil,
        catalog: LocalTranslationCatalog = .default(),
        sessionConfiguration: URLSessionConfiguration = .ephemeral,
        fileManager: FileManager = .default
    ) {
        self.stateDirectory = stateDirectory
        self.catalog = catalog
        self.fileManager = fileManager
        sessionConfiguration.timeoutIntervalForRequest = 60
        sessionConfiguration.timeoutIntervalForResource = 7_200
        self.sessionConfiguration = sessionConfiguration
        if let modelsDirectory, let runtimeDirectory {
            self.modelsDirectory = modelsDirectory
            self.runtimeDirectory = runtimeDirectory
        } else {
            let defaults = Self.defaultAssetDirectories(stateDirectory: stateDirectory)
            self.modelsDirectory = modelsDirectory ?? defaults.models
            self.runtimeDirectory = runtimeDirectory ?? defaults.runtime
        }
    }

    public static func defaultAssetDirectories(
        stateDirectory: URL,
        environment: [String: String] = ProcessInfo.processInfo.environment,
        home: URL = FileManager.default.homeDirectoryForCurrentUser
    ) -> (models: URL, runtime: URL) {
        #if os(macOS)
        return (
            models: stateDirectory.appendingPathComponent("LocalModels", isDirectory: true),
            runtime: stateDirectory.appendingPathComponent("LocalRuntime", isDirectory: true)
        )
        #else
        let dataRoot = environment["XDG_DATA_HOME"]
            .flatMap { $0.hasPrefix("/") ? URL(fileURLWithPath: $0, isDirectory: true) : nil }
            ?? home.appendingPathComponent(".local/share", isDirectory: true)
        let base = dataRoot.appendingPathComponent("maltoria/translation", isDirectory: true)
        return (
            models: base.appendingPathComponent("models", isDirectory: true),
            runtime: base.appendingPathComponent("runtime", isDirectory: true)
        )
        #endif
    }

    public func preferences() -> LocalTranslationPreferences {
        let file = stateDirectory.appendingPathComponent("local-translation-prefs.json")
        guard let data = try? Data(contentsOf: file),
              let decoded = try? JSONDecoder().decode(LocalTranslationPreferences.self, from: data)
        else { return LocalTranslationPreferences() }
        return decoded
    }

    public func savePreferences(_ prefs: LocalTranslationPreferences) throws {
        try fileManager.createDirectory(
            at: stateDirectory,
            withIntermediateDirectories: true,
            attributes: [.posixPermissions: 0o700]
        )
        let file = stateDirectory.appendingPathComponent("local-translation-prefs.json")
        try JSONEncoder().encode(prefs).write(to: file, options: .atomic)
        try? fileManager.setAttributes([.posixPermissions: 0o600], ofItemAtPath: file.path)
    }

    public func modelFileURL(for entry: LocalModelCatalogEntry) -> URL {
        modelsDirectory.appendingPathComponent(entry.filename, isDirectory: false)
    }

    public func isModelInstalled(_ entry: LocalModelCatalogEntry) -> Bool {
        let path = modelFileURL(for: entry).path
        guard fileManager.fileExists(atPath: path) else { return false }
        let attrs = try? fileManager.attributesOfItem(atPath: path)
        let size = (attrs?[.size] as? NSNumber)?.int64Value ?? 0
        return size > 0
    }

    public func installedModelSize(_ entry: LocalModelCatalogEntry) -> Int64? {
        let path = modelFileURL(for: entry).path
        guard fileManager.fileExists(atPath: path),
              let attrs = try? fileManager.attributesOfItem(atPath: path),
              let size = (attrs[.size] as? NSNumber)?.int64Value,
              size > 0
        else { return nil }
        return size
    }

    public func installedRuntime() -> (manifest: InstalledLocalRuntime, binaryURL: URL)? {
        let manifestURL = runtimeDirectory.appendingPathComponent("active.json")
        guard let data = try? Data(contentsOf: manifestURL),
              let manifest = try? JSONDecoder().decode(InstalledLocalRuntime.self, from: data)
        else { return nil }
        let binaryURL = runtimeDirectory.appendingPathComponent(manifest.binaryRelativePath)
        guard fileManager.isExecutableFile(atPath: binaryURL.path) else { return nil }
        return (manifest, binaryURL)
    }

    public func activeDownload(for modelID: String) -> LocalDownloadProgress? {
        activeDownloads[modelID]
    }

    public func lastError(for modelID: String) -> String? {
        lastDownloadErrors[modelID]
    }

    public func setDownloadProgress(
        _ progress: LocalDownloadProgress,
        for modelID: String,
        token: UUID? = nil
    ) {
        if let token, activeDownloadTokens[modelID] != token {
            return
        }
        activeDownloads[modelID] = progress
        lastDownloadErrors.removeValue(forKey: modelID)
    }

    public func finishDownload(for modelID: String, error: String? = nil, token: UUID? = nil) {
        if let token, activeDownloadTokens[modelID] != token {
            return
        }
        activeDownloadTokens.removeValue(forKey: modelID)
        activeDownloads.removeValue(forKey: modelID)
        if let error {
            lastDownloadErrors[modelID] = error
        } else {
            lastDownloadErrors.removeValue(forKey: modelID)
        }
    }

    public func deleteModel(id: String) throws -> Bool {
        guard let entry = catalog.model(id: id) else {
            throw BridgeFailure("model_not_found", "Unknown local model: \(id)")
        }
        guard activeDownloads[id] == nil else {
            throw BridgeFailure("download_in_progress", "Cancel the active download before deleting this model")
        }
        let file = modelFileURL(for: entry)
        let part = modelsDirectory.appendingPathComponent(entry.filename + ".part")
        var removed = false
        if fileManager.fileExists(atPath: file.path) {
            try fileManager.removeItem(at: file)
            removed = true
        }
        if fileManager.fileExists(atPath: part.path) {
            try? fileManager.removeItem(at: part)
        }
        lastDownloadErrors.removeValue(forKey: id)
        return removed
    }

    public func ensureRuntimeInstalled(
        modelID: String,
        jobID: String,
        token: UUID? = nil,
        onProgress: @escaping @Sendable (LocalDownloadProgress) async -> Void
    ) async throws -> URL {
        if let existing = installedRuntime(),
           existing.manifest.version == catalog.runtimeAsset.version,
           existing.manifest.sha256 == catalog.runtimeAsset.sha256 {
            return existing.binaryURL
        }
        let asset = catalog.runtimeAsset
        guard let url = URL(string: asset.downloadURL) else {
            throw BridgeFailure("invalid_runtime_url", "Local runtime download URL is invalid")
        }
        try fileManager.createDirectory(
            at: runtimeDirectory,
            withIntermediateDirectories: true,
            attributes: [.posixPermissions: 0o700]
        )
        let archivePart = runtimeDirectory.appendingPathComponent("\(asset.filename).part")
        try await downloadFile(
            from: url,
            toPart: archivePart,
            expectedSize: asset.sizeBytes,
            stage: "downloadingRuntime",
            modelID: modelID,
            jobID: jobID,
            token: token,
            onProgress: onProgress
        )

        let verifyProgress = LocalDownloadProgress(
            stage: "verifyingRuntime",
            downloadedBytes: asset.sizeBytes,
            totalBytes: asset.sizeBytes,
            bytesPerSecond: 0,
            etaSeconds: nil,
            jobID: jobID
        )
        setDownloadProgress(verifyProgress, for: modelID, token: token)
        await onProgress(verifyProgress)

        let actualHash = try await computeSHA256(of: archivePart)
        guard actualHash.caseInsensitiveCompare(asset.sha256) == .orderedSame else {
            try? fileManager.removeItem(at: archivePart)
            throw BridgeFailure(
                "checksum_mismatch",
                "Local runtime SHA-256 verification failed (expected \(asset.sha256.prefix(12)), got \(actualHash.prefix(12)))"
            )
        }

        let extractProgress = LocalDownloadProgress(
            stage: "extractingRuntime",
            downloadedBytes: asset.sizeBytes,
            totalBytes: asset.sizeBytes,
            bytesPerSecond: 0,
            etaSeconds: nil,
            jobID: jobID
        )
        setDownloadProgress(extractProgress, for: modelID, token: token)
        await onProgress(extractProgress)

        let binaryURL = try await extractRuntimeArchive(archivePart, asset: asset)
        try? fileManager.removeItem(at: archivePart)
        return binaryURL
    }

    public func downloadModel(
        id: String,
        useChinaMirror: Bool?,
        jobID: String,
        onProgress: @escaping @Sendable (LocalDownloadProgress) async -> Void
    ) async throws -> URL {
        guard let entry = catalog.model(id: id) else {
            throw BridgeFailure("model_not_found", "Unknown local model: \(id)")
        }
        let effectiveMirror = useChinaMirror ?? preferences().useChinaMirror
        guard let url = entry.downloadURL(useChinaMirror: effectiveMirror) else {
            throw BridgeFailure("invalid_model_url", "Model download URL is invalid")
        }

        let token = UUID()
        activeDownloadTokens[id] = token

        do {
            _ = try await ensureRuntimeInstalled(modelID: id, jobID: jobID, token: token, onProgress: onProgress)
            try Task.checkCancellation()

            try fileManager.createDirectory(
                at: modelsDirectory,
                withIntermediateDirectories: true,
                attributes: [.posixPermissions: 0o700]
            )
            let destination = modelFileURL(for: entry)
            if isModelInstalled(entry) {
                let existingHash = try await computeSHA256(of: destination)
                if existingHash.caseInsensitiveCompare(entry.sha256) == .orderedSame {
                    finishDownload(for: id, error: nil, token: token)
                    return destination
                }
                try? fileManager.removeItem(at: destination)
            }

            let partURL = modelsDirectory.appendingPathComponent(entry.filename + ".part")
            try await downloadFile(
                from: url,
                toPart: partURL,
                expectedSize: entry.sizeBytes,
                stage: "downloadingModel",
                modelID: id,
                jobID: jobID,
                token: token,
                onProgress: onProgress
            )
            try Task.checkCancellation()

            let verifyProgress = LocalDownloadProgress(
                stage: "verifyingModel",
                downloadedBytes: entry.sizeBytes,
                totalBytes: entry.sizeBytes,
                bytesPerSecond: 0,
                etaSeconds: nil,
                jobID: jobID
            )
            setDownloadProgress(verifyProgress, for: id, token: token)
            await onProgress(verifyProgress)

            let actualHash = try await computeSHA256(of: partURL)
            guard actualHash.caseInsensitiveCompare(entry.sha256) == .orderedSame else {
                try? fileManager.removeItem(at: partURL)
                throw BridgeFailure(
                    "checksum_mismatch",
                    "Model SHA-256 verification failed (expected \(entry.sha256.prefix(12)), got \(actualHash.prefix(12)))"
                )
            }

            if fileManager.fileExists(atPath: destination.path) {
                try fileManager.removeItem(at: destination)
            }
            try fileManager.moveItem(at: partURL, to: destination)
            try? fileManager.setAttributes([.posixPermissions: 0o600], ofItemAtPath: destination.path)
            finishDownload(for: id, error: nil, token: token)
            return destination
        } catch is CancellationError {
            finishDownload(for: id, error: nil, token: token)
            throw CancellationError()
        } catch {
            finishDownload(for: id, error: error.localizedDescription, token: token)
            throw error
        }
    }

    private func downloadFile(
        from url: URL,
        toPart partURL: URL,
        expectedSize: Int64,
        stage: String,
        modelID: String,
        jobID: String,
        token: UUID?,
        onProgress: @escaping @Sendable (LocalDownloadProgress) async -> Void
    ) async throws {
        var existingBytes: Int64 = 0
        if fileManager.fileExists(atPath: partURL.path),
           let attrs = try? fileManager.attributesOfItem(atPath: partURL.path),
           let size = (attrs[.size] as? NSNumber)?.int64Value,
           size > 0 {
            if size < expectedSize {
                existingBytes = size
            } else if size == expectedSize {
                let done = LocalDownloadProgress(
                    stage: stage,
                    downloadedBytes: size,
                    totalBytes: expectedSize,
                    bytesPerSecond: 0,
                    etaSeconds: 0,
                    jobID: jobID
                )
                setDownloadProgress(done, for: modelID, token: token)
                await onProgress(done)
                return
            } else {
                try? fileManager.removeItem(at: partURL)
            }
        }

        let initial = LocalDownloadProgress(
            stage: stage,
            downloadedBytes: existingBytes,
            totalBytes: expectedSize,
            bytesPerSecond: 0,
            etaSeconds: nil,
            jobID: jobID
        )
        setDownloadProgress(initial, for: modelID, token: token)
        await onProgress(initial)

        var request = URLRequest(url: url)
        request.httpMethod = "GET"
        request.timeoutInterval = 60
        request.setValue("Maltoria/LocalTranslation", forHTTPHeaderField: "User-Agent")
        if existingBytes > 0 {
            request.setValue("bytes=\(existingBytes)-", forHTTPHeaderField: "Range")
        }

        let downloader = StreamingFileDownloader(
            configuration: sessionConfiguration,
            partURL: partURL,
            resumeOffset: existingBytes,
            expectedTotalBytes: expectedSize
        )
        try await downloader.run(request: request) { [weak self] downloaded, total, speed, eta in
            let progress = LocalDownloadProgress(
                stage: stage,
                downloadedBytes: downloaded,
                totalBytes: total > 0 ? total : expectedSize,
                bytesPerSecond: speed,
                etaSeconds: eta,
                jobID: jobID
            )
            await self?.setDownloadProgress(progress, for: modelID, token: token)
            await onProgress(progress)
        }
    }

    private func extractRuntimeArchive(_ archiveURL: URL, asset: LocalRuntimeAsset) async throws -> URL {
        let staging = runtimeDirectory.appendingPathComponent(".extract-\(UUID().uuidString)", isDirectory: true)
        try fileManager.createDirectory(
            at: staging,
            withIntermediateDirectories: true,
            attributes: [.posixPermissions: 0o700]
        )
        defer { try? fileManager.removeItem(at: staging) }

        let tarPath = fileManager.isExecutableFile(atPath: "/usr/bin/tar") ? "/usr/bin/tar" : "/bin/tar"
        let result = try await HostCommand.run(
            executable: URL(fileURLWithPath: tarPath),
            arguments: ["-xzf", archiveURL.path, "-C", staging.path],
            environment: ProcessInfo.processInfo.environment,
            timeout: 120
        )
        guard result.status == 0, !result.signalled else {
            let output = String(data: result.output, encoding: .utf8) ?? ""
            throw BridgeFailure("extract_failed", "Failed to extract llama.cpp runtime: \(output.prefix(300))")
        }

        guard let foundBinary = findLlamaServerBinary(in: staging) else {
            throw BridgeFailure("invalid_runtime_archive", "Extracted archive does not contain llama-server")
        }
        let binaryFolder = foundBinary.deletingLastPathComponent()
        let versionsDir = runtimeDirectory.appendingPathComponent("versions", isDirectory: true)
        try fileManager.createDirectory(at: versionsDir, withIntermediateDirectories: true)
        let folderName = "\(asset.version)-\(asset.sha256.prefix(12))-\(UUID().uuidString.prefix(8))"
        let destinationDir = versionsDir.appendingPathComponent(folderName, isDirectory: true)
        try fileManager.moveItem(at: binaryFolder, to: destinationDir)

        let finalBinary = destinationDir.appendingPathComponent(foundBinary.lastPathComponent)
        try fileManager.setAttributes([.posixPermissions: 0o755], ofItemAtPath: finalBinary.path)

        let relPath = "versions/\(folderName)/\(foundBinary.lastPathComponent)"
        let manifest = InstalledLocalRuntime(
            version: asset.version,
            platform: asset.platform,
            sha256: asset.sha256,
            binaryRelativePath: relPath,
            gpuAcceleration: asset.gpuAcceleration
        )
        let manifestURL = runtimeDirectory.appendingPathComponent("active.json")
        try JSONEncoder().encode(manifest).write(to: manifestURL, options: .atomic)
        return finalBinary
    }

    private func findLlamaServerBinary(in directory: URL) -> URL? {
        guard let enumerator = fileManager.enumerator(
            at: directory,
            includingPropertiesForKeys: [.isRegularFileKey, .isSymbolicLinkKey],
            options: [.skipsHiddenFiles]
        )
        else { return nil }
        for case let item as URL in enumerator where item.lastPathComponent == "llama-server" {
            let values = try? item.resourceValues(forKeys: [.isRegularFileKey])
            if values?.isRegularFile == true {
                return item
            }
        }
        return nil
    }

    public func computeSHA256(of fileURL: URL) async throws -> String {
        #if canImport(CryptoKit)
        return try await Task.detached(priority: .utility) {
            let handle = try FileHandle(forReadingFrom: fileURL)
            defer { try? handle.close() }
            var hasher = SHA256()
            while true {
                try Task.checkCancellation()
                let chunk = try handle.read(upToCount: 1_048_576) ?? Data()
                if chunk.isEmpty { break }
                hasher.update(data: chunk)
            }
            return hasher.finalize().map { String(format: "%02x", $0) }.joined()
        }.value
        #else
        let tool = fileManager.isExecutableFile(atPath: "/usr/bin/sha256sum")
            ? URL(fileURLWithPath: "/usr/bin/sha256sum")
            : URL(fileURLWithPath: "/bin/sha256sum")
        let result = try await HostCommand.run(
            executable: tool,
            arguments: [fileURL.path],
            environment: ProcessInfo.processInfo.environment,
            timeout: 180
        )
        guard result.status == 0,
              let text = String(data: result.output, encoding: .utf8),
              let token = text.split(whereSeparator: \.isWhitespace).first,
              token.count == 64
        else {
            throw BridgeFailure("checksum_failed", "Failed to compute SHA-256 for downloaded file")
        }
        return String(token).lowercased()
        #endif
    }
}

private final class StreamingFileDownloader: NSObject, URLSessionDataDelegate, @unchecked Sendable {
    typealias ProgressCallback = @Sendable (Int64, Int64, Int64, Int?) async -> Void

    private let configuration: URLSessionConfiguration
    private let partURL: URL
    private let resumeOffset: Int64
    private let expectedTotalBytes: Int64
    private let lock = NSLock()
    private var continuation: CheckedContinuation<Void, any Error>?
    private var session: URLSession?
    private var task: URLSessionDataTask?
    private var fileHandle: FileHandle?
    private var downloadedBytes: Int64 = 0
    private var totalBytes: Int64 = 0
    private var lastTickTime: Date = .distantPast
    private var lastTickBytes: Int64 = 0
    private var smoothedSpeed: Double = 0
    private var onProgress: ProgressCallback?
    private var finished = false

    init(
        configuration: URLSessionConfiguration,
        partURL: URL,
        resumeOffset: Int64,
        expectedTotalBytes: Int64
    ) {
        self.configuration = configuration
        self.partURL = partURL
        self.resumeOffset = resumeOffset
        self.expectedTotalBytes = expectedTotalBytes
    }

    func run(request: URLRequest, onProgress: @escaping ProgressCallback) async throws {
        try await withTaskCancellationHandler {
            try await withCheckedThrowingContinuation { (cont: CheckedContinuation<Void, any Error>) in
                lock.lock()
                self.continuation = cont
                self.onProgress = onProgress
                let urlSession = URLSession(configuration: configuration, delegate: self, delegateQueue: nil)
                self.session = urlSession
                let dataTask = urlSession.dataTask(with: request)
                self.task = dataTask
                lock.unlock()
                dataTask.resume()
            }
        } onCancel: {
            self.cancel()
        }
    }

    func cancel() {
        lock.lock()
        let activeTask = task
        let activeSession = session
        let handle = fileHandle
        fileHandle = nil
        let cont = continuation
        continuation = nil
        finished = true
        lock.unlock()
        activeTask?.cancel()
        activeSession?.invalidateAndCancel()
        try? handle?.close()
        cont?.resume(throwing: CancellationError())
    }

    func urlSession(
        _: URLSession,
        dataTask: URLSessionDataTask,
        didReceive response: URLResponse,
        completionHandler: @escaping (URLSession.ResponseDisposition) -> Void
    ) {
        guard let http = response as? HTTPURLResponse else {
            completionHandler(.cancel)
            fail(BridgeFailure("download_failed", "Download returned a non-HTTP response"))
            return
        }
        guard http.statusCode == 200 || http.statusCode == 206 else {
            completionHandler(.cancel)
            fail(BridgeFailure("download_http_error", "Download failed with HTTP status \(http.statusCode)"))
            return
        }
        do {
            let manager = FileManager.default
            let appendMode = http.statusCode == 206 && resumeOffset > 0 && manager.fileExists(atPath: partURL.path)
            if !appendMode {
                if manager.fileExists(atPath: partURL.path) {
                    try manager.removeItem(at: partURL)
                }
                _ = manager.createFile(atPath: partURL.path, contents: nil, attributes: [.posixPermissions: 0o600])
            }
            let handle = try FileHandle(forWritingTo: partURL)
            if appendMode {
                try handle.seekToEnd()
            }
            let initialDownloaded: Int64 = appendMode ? resumeOffset : 0
            let contentLength = http.expectedContentLength
            let resolvedTotal: Int64 = if contentLength > 0 {
                appendMode ? resumeOffset + contentLength : contentLength
            } else {
                expectedTotalBytes
            }

            lock.lock()
            fileHandle = handle
            downloadedBytes = initialDownloaded
            totalBytes = resolvedTotal
            lastTickTime = Date()
            lastTickBytes = initialDownloaded
            smoothedSpeed = 0
            lock.unlock()
            completionHandler(.allow)
        } catch {
            completionHandler(.cancel)
            fail(BridgeFailure("download_io_error", "Unable to open download file: \(error.localizedDescription)"))
        }
    }

    private struct ProgressEmission {
        let downloaded: Int64
        let total: Int64
        let speed: Int64
        let eta: Int?
        let callback: ProgressCallback
    }

    func urlSession(_: URLSession, dataTask _: URLSessionDataTask, didReceive data: Data) {
        lock.lock()
        guard !finished, let handle = fileHandle else {
            lock.unlock()
            return
        }
        do {
            try handle.write(contentsOf: data)
            downloadedBytes += Int64(data.count)
            let now = Date()
            let elapsed = now.timeIntervalSince(lastTickTime)
            var emit: ProgressEmission?
            if elapsed >= 0.25 {
                let deltaBytes = downloadedBytes - lastTickBytes
                let instantSpeed = Double(max(0, deltaBytes)) / elapsed
                smoothedSpeed = smoothedSpeed <= 0 ? instantSpeed : (0.35 * instantSpeed + 0.65 * smoothedSpeed)
                let speedInt = Int64(smoothedSpeed.rounded())
                let remaining = max(0, totalBytes - downloadedBytes)
                let eta: Int? = speedInt > 0 ? Int(ceil(Double(remaining) / Double(speedInt))) : nil
                lastTickTime = now
                lastTickBytes = downloadedBytes
                if let callback = onProgress {
                    emit = ProgressEmission(
                        downloaded: downloadedBytes,
                        total: totalBytes,
                        speed: speedInt,
                        eta: eta,
                        callback: callback
                    )
                }
            }
            lock.unlock()
            if let emit {
                Task { await emit.callback(emit.downloaded, emit.total, emit.speed, emit.eta) }
            }
        } catch {
            lock.unlock()
            cancelWithError(BridgeFailure(
                "download_io_error",
                "Failed writing downloaded data: \(error.localizedDescription)"
            ))
        }
    }

    func urlSession(_: URLSession, task _: URLSessionTask, didCompleteWithError error: (any Error)?) {
        lock.lock()
        guard !finished else {
            lock.unlock()
            return
        }
        finished = true
        let handle = fileHandle
        fileHandle = nil
        let activeSession = session
        session = nil
        let cont = continuation
        continuation = nil
        onProgress = nil
        lock.unlock()

        try? handle?.close()
        activeSession?.finishTasksAndInvalidate()

        if let error {
            if (error as? URLError)?.code == .cancelled {
                cont?.resume(throwing: CancellationError())
            } else {
                cont?.resume(throwing: BridgeFailure("download_failed", error.localizedDescription))
            }
            return
        }
        cont?.resume(returning: ())
    }

    private func cancelWithError(_ failure: any Error) {
        lock.lock()
        let activeTask = task
        let activeSession = session
        let handle = fileHandle
        fileHandle = nil
        let cont = continuation
        continuation = nil
        finished = true
        lock.unlock()
        activeTask?.cancel()
        activeSession?.invalidateAndCancel()
        try? handle?.close()
        cont?.resume(throwing: failure)
    }

    private func fail(_ failure: any Error) {
        cancelWithError(failure)
    }
}
