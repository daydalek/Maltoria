//
//  AppUpdateTransport.swift
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
#if canImport(FoundationNetworking)
import FoundationNetworking
#endif
#if canImport(CryptoKit)
import CryptoKit
#endif

struct AppUpdateTransport: Sendable {
    let releasesURL: URL
    let downloadBase: URL
    private let loopback: Bool

    init(testOrigin: URL? = nil) {
        let origin = testOrigin.flatMap { url -> URL? in
            guard let parts = URLComponents(url: url, resolvingAgainstBaseURL: false),
                  ["http", "https"].contains(parts.scheme),
                  ["localhost", "127.0.0.1", "::1"].contains(parts.host),
                  parts.user == nil, parts.password == nil, parts.query == nil, parts.fragment == nil,
                  parts.path.isEmpty || parts.path == "/"
            else { return nil }
            return url
        }
        let api = origin ?? URL(string: "https://api.github.com")!
        let web = origin ?? URL(string: "https://github.com")!
        var endpoint = URLComponents(
            url: api.appendingPathComponent("repos/\(AppUpdateRelease.repository)/releases"),
            resolvingAgainstBaseURL: false
        )!
        endpoint.queryItems = [URLQueryItem(name: "per_page", value: "100")]
        releasesURL = endpoint.url!
        downloadBase = web.appendingPathComponent("\(AppUpdateRelease.repository)/releases/download")
        loopback = origin != nil
    }

    static var configured: Self {
        #if DEBUG
        if let origin = ProcessInfo.processInfo.environment["MALTORIA_TEST_UPDATE_ORIGIN"].flatMap(URL.init(string:)) {
            return Self(testOrigin: origin)
        }
        #endif
        return Self()
    }

    func fetch(_ url: URL, limit: Int64, expected: Int64? = nil) async throws -> URL {
        let delegate = DownloadPolicy(limit: limit, allowCDN: !loopback && url != releasesURL)
        let config = URLSessionConfiguration.ephemeral
        config.timeoutIntervalForResource = 600
        let session = URLSession(configuration: config, delegate: delegate, delegateQueue: nil)
        defer { session.finishTasksAndInvalidate() }
        var request = URLRequest(url: url)
        request.setValue("Maltoria-Updater", forHTTPHeaderField: "User-Agent")
        if url == releasesURL {
            request.setValue("application/vnd.github+json", forHTTPHeaderField: "Accept")
        }
        let (file, response) = try await session.download(for: request)
        do {
            try Task.checkCancellation()
            guard let http = response as? HTTPURLResponse, http.statusCode == 200 else {
                throw BridgeFailure("download_failed", "The release server did not return an update asset")
            }
            let size = try Int64(file.resourceValues(forKeys: [.fileSizeKey]).fileSize ?? -1)
            guard (1 ... limit).contains(size), expected == nil || size == expected else {
                throw BridgeFailure("download_failed", "The app update download has the wrong size")
            }
            return file
        } catch {
            try? FileManager.default.removeItem(at: file)
            throw error
        }
    }

    static func verify(_ file: URL, asset: AppUpdateAsset) async throws {
        #if canImport(CryptoKit)
        let work = Task.detached(priority: .utility) {
            let handle = try FileHandle(forReadingFrom: file)
            defer { try? handle.close() }
            var hasher = SHA256()
            while let chunk = try handle.read(upToCount: 1_048_576), !chunk.isEmpty {
                try Task.checkCancellation()
                hasher.update(data: chunk)
            }
            return hasher.finalize().map { String(format: "%02x", $0) }.joined()
        }
        let hash = try await withTaskCancellationHandler { try await work.value } onCancel: { work.cancel() }
        #else
        let result = try await HostCommand.run(
            executable: URL(fileURLWithPath: "/usr/bin/sha256sum"),
            arguments: [file.path],
            environment: [:],
            timeout: 180
        )
        let hash = result.status == 0 ? String(data: result.output, encoding: .utf8)?
            .split(whereSeparator: \.isWhitespace).first.map(String.init) : nil
        #endif
        guard let expected = asset.sha256, hash == expected else {
            throw BridgeFailure("checksum_mismatch", "The downloaded app update failed its SHA-256 check")
        }
    }
}

private final class DownloadPolicy: NSObject, URLSessionDownloadDelegate, Sendable {
    private let limit: Int64
    private let allowCDN: Bool

    init(limit: Int64, allowCDN: Bool) {
        self.limit = limit
        self.allowCDN = allowCDN
    }

    func urlSession(
        _: URLSession,
        task _: URLSessionTask,
        willPerformHTTPRedirection _: HTTPURLResponse,
        newRequest request: URLRequest,
        completionHandler: @escaping @Sendable (URLRequest?) -> Void
    ) {
        guard allowCDN, let url = request.url, url.scheme == "https", url.port == nil,
              url.user == nil, url.password == nil,
              ["release-assets.githubusercontent.com", "objects.githubusercontent.com"].contains(url.host)
        else {
            completionHandler(nil)
            return
        }
        completionHandler(request)
    }

    func urlSession(
        _: URLSession, downloadTask: URLSessionDownloadTask, didWriteData _: Int64,
        totalBytesWritten: Int64, totalBytesExpectedToWrite: Int64
    ) {
        if totalBytesWritten > limit || totalBytesExpectedToWrite > limit {
            downloadTask.cancel()
        }
    }

    func urlSession(_: URLSession, downloadTask _: URLSessionDownloadTask, didFinishDownloadingTo _: URL) {}
}
