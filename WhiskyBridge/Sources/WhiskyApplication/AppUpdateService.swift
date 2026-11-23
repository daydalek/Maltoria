//
//  AppUpdateService.swift
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

@MainActor
final class AppUpdateService {
    static let methods = ["appUpdate.check", "appUpdate.download"]
    private let preferences: PreferencesService?
    private let jobs: BridgeJobStore
    private let transport: AppUpdateTransport
    private let cacheDirectory: URL
    private var candidate: AppUpdateRelease.Candidate?
    private var job: String?

    init(
        preferences: PreferencesService?, jobs: BridgeJobStore, transport: AppUpdateTransport = .configured,
        cacheDirectory: URL? = nil
    ) {
        self.preferences = preferences
        self.jobs = jobs
        self.transport = transport
        self.cacheDirectory = cacheDirectory ?? FileManager.default.urls(for: .cachesDirectory, in: .userDomainMask)[0]
            .appendingPathComponent("Maltoria/AppUpdates", isDirectory: true)
    }

    func call(_ method: String, params: [String: JSONValue]) throws -> JSONValue {
        let values = try requireNetwork()
        if let job, jobs.isActive(job) {
            throw BridgeFailure("busy", "An app update operation is already running")
        }
        switch method {
        case "appUpdate.check":
            if params["automatic"] == .bool(true), values["checkMaltoriaAppUpdates"] != .bool(true) {
                return .object(["skipped": .bool(true)])
            }
            guard let current = params["currentVersion"]?.string.flatMap(AppUpdateVersion.init),
                  let format = params["format"]?.string.flatMap(AppUpdateFormat.init(rawValue:))
            else { throw BridgeFailure("invalid_request", "The installed app version or package type is unknown") }
            candidate = nil
            return try start(method) { progress in
                _ = try self.requireNetwork()
                progress(.object(["stage": .string("checking")]))
                let metadata = try await self.transport.fetch(self.transport.releasesURL, limit: 4_194_304)
                defer { try? FileManager.default.removeItem(at: metadata) }
                _ = try self.requireNetwork()
                let release = try AppUpdateRelease.select(
                    Data(contentsOf: metadata),
                    format: format,
                    current: current,
                    downloadBase: self.transport.downloadBase
                )
                self.candidate = release
                return .object(["availableVersion": release.map { .string($0.version.description) } ?? .null])
            }
        case "appUpdate.download":
            guard let candidate, params["version"]?.string == candidate.version.description else {
                throw BridgeFailure("release_unavailable", "Check for an app update before downloading")
            }
            return try start(method) { progress in
                try await self.download(candidate, progress: progress)
            }
        default:
            throw BridgeFailure("method_unavailable", "Unknown app update method")
        }
    }

    func preferencesChanged() throws {
        guard let job, jobs.isActive(job), try preferences?.call("preferences.read", params: [:])
            .object?["offlineMode"] == .bool(true)
        else { return }
        _ = try jobs.call("jobs.cancel", params: ["jobID": .string(job)])
    }

    private func start(_ method: String, operation: @escaping BridgeJobStore.Operation) throws -> JSONValue {
        let response = try jobs.start(name: method) { progress in
            defer { self.job = nil }
            return try await operation(progress)
        }
        job = response.object?["jobID"]?.string
        return response
    }

    private func requireNetwork() throws -> [String: JSONValue] {
        try Task.checkCancellation()
        guard let values = try preferences?.call("preferences.read", params: [:]).object,
              values["offlineMode"] == .bool(false)
        else { throw BridgeFailure("offline", "App updates are disabled in offline mode") }
        return values
    }

    private func download(
        _ candidate: AppUpdateRelease.Candidate, progress: BridgeJobStore.Progress
    ) async throws -> JSONValue {
        progress(.object(["stage": .string("checking")]))
        _ = try requireNetwork()
        let sums = try await transport.fetch(
            candidate.sums.browserDownloadURL, limit: 1_048_576, expected: candidate.sums.size
        )
        defer { try? FileManager.default.removeItem(at: sums) }
        try await AppUpdateTransport.verify(sums, asset: candidate.sums)
        try AppUpdateRelease.verifySums(Data(contentsOf: sums), asset: candidate.asset)
        _ = try requireNetwork()
        progress(.object(["stage": .string("downloading")]))
        let package = try await transport.fetch(
            candidate.asset.browserDownloadURL, limit: candidate.asset.size, expected: candidate.asset.size
        )
        defer { try? FileManager.default.removeItem(at: package) }
        progress(.object(["stage": .string("verifying")]))
        try await AppUpdateTransport.verify(package, asset: candidate.asset)
        _ = try requireNetwork()
        let manager = FileManager.default
        let directory = cacheDirectory.appendingPathComponent(UUID().uuidString, isDirectory: true)
        try manager.createDirectory(
            at: directory, withIntermediateDirectories: true, attributes: [.posixPermissions: 0o700]
        )
        let destination = directory.appendingPathComponent(candidate.asset.name)
        do {
            try manager.moveItem(at: package, to: destination)
        } catch {
            try? manager.removeItem(at: directory)
            throw error
        }
        return .object([
            "path": .string(destination.path),
            "version": .string(candidate.version.description),
            "sha256": candidate.asset.sha256.map(JSONValue.string) ?? .null
        ])
    }
}
