//
//  AppUpdateTests.swift
//  WhiskyBridge
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

@Suite("App update release selection")
struct AppUpdateTests {
    private let base = URL(string: "https://github.com/daydalek/Maltoria/releases/download")!
    private let hash = String(repeating: "a", count: 64)

    private func release(
        _ tag: String, version: String, draft: Bool = false, missing: String? = nil, duplicate: String? = nil
    ) throws -> [String: Any] {
        let version = try #require(AppUpdateVersion(version))
        var names = Set(["SHA256SUMS-macos-arm64", "SHA256SUMS-linux-amd64"])
        let formats: [AppUpdateFormat] = [.macOS, .appImage, .deb, .rpm]
        names.formUnion(formats.map { $0.assetName(version: version) })
        if let missing { names.remove(missing) }
        return [
            "tag_name": tag, "draft": draft, "prerelease": true,
            "assets": (Array(names) + (duplicate.map { [$0] } ?? [])).map { name in
                [
                    "name": name, "state": "uploaded", "size": 20, "digest": "sha256:\(hash)",
                    "browser_download_url": "\(base.absoluteString)/\(tag)/\(name)"
                ] as [String: Any]
            }
        ]
    }

    @Test func selectsNumericUnifiedPrereleasesAndSkipsBrokenInventories() throws {
        let data = try JSONSerialization.data(withJSONObject: [
            release("v0.9.2", version: "0.9.2"),
            release("v0.10.0", version: "0.10.0"),
            release("v0.11.0", version: "0.11.0", missing: "SHA256SUMS-macos-arm64"),
            release("v0.12.0", version: "0.12.0", draft: true),
            release("v0.13.0", version: "0.13.0", duplicate: "SHA256SUMS-linux-amd64"),
            release("linux-v1.0.0", version: "1.0.0")
        ])
        let current = try #require(AppUpdateVersion("0.9.1"))
        let selection = try AppUpdateRelease.select(data, format: .appImage, current: current, downloadBase: base)
        #expect(selection?.version.description == "0.10.0")
        #expect(selection?.asset.name == "Maltoria-0.10.0-x86_64.AppImage")
        #expect(AppUpdateVersion("00.10.0") == nil)
        let future = try #require(AppUpdateVersion("1.0.0"))
        #expect(try AppUpdateRelease.select(data, format: .deb, current: future, downloadBase: base) == nil)
    }

    @Test func checksumsMustMatchAPIDigestAndCannotRepeatNames() throws {
        let data = try JSONSerialization.data(withJSONObject: [release("v0.7.0", version: "0.7.0")])
        let current = try #require(AppUpdateVersion("0.6.3"))
        let selection = try #require(try AppUpdateRelease.select(
            data,
            format: .macOS,
            current: current,
            downloadBase: base
        ))
        let line = "\(hash)  \(selection.asset.name)\n"
        try AppUpdateRelease.verifySums(Data(line.utf8), asset: selection.asset)
        #expect(throws: BridgeFailure.self) {
            try AppUpdateRelease.verifySums(Data((line + line).utf8), asset: selection.asset)
        }
        let mismatch = "\(String(repeating: "b", count: 64))  \(selection.asset.name)\n"
        #expect(throws: BridgeFailure.self) {
            try AppUpdateRelease.verifySums(Data(mismatch.utf8), asset: selection.asset)
        }
    }

    @Test @MainActor func cachingReplacesOlderPackagesOnlyAfterSuccessfulMove() throws {
        let root = FileManager.default.temporaryDirectory.appendingPathComponent("maltoria-update-\(UUID())")
        let cache = root.appendingPathComponent("cache")
        let manager = FileManager.default
        try manager.createDirectory(at: cache, withIntermediateDirectories: true)
        defer { try? manager.removeItem(at: root) }
        let service = AppUpdateService(preferences: nil, jobs: BridgeJobStore(), cacheDirectory: cache)
        let package = root.appendingPathComponent("download")
        let unrelated = cache.appendingPathComponent("unrelated")
        try Data().write(to: unrelated)
        try Data("old".utf8).write(to: package)
        let old = try service.cacheVerifiedPackage(package, named: "Maltoria-0.6.4-x86_64.AppImage")
        #expect(throws: (any Error).self) {
            try service.cacheVerifiedPackage(package, named: "Maltoria-0.6.5-x86_64.AppImage")
        }
        #expect(manager.fileExists(atPath: old.path))
        try Data("new".utf8).write(to: package)
        let current = try service.cacheVerifiedPackage(package, named: "Maltoria-0.6.5-x86_64.AppImage")
        #expect(try Data(contentsOf: current) == Data("new".utf8))
        #expect(!manager.fileExists(atPath: old.deletingLastPathComponent().path))
        #expect(manager.fileExists(atPath: unrelated.path))
    }

    @Test @MainActor func automaticChecksRequireOptInButManualChecksDoNot() throws {
        let root = FileManager.default.temporaryDirectory.appendingPathComponent("maltoria-update-\(UUID())")
        defer { try? FileManager.default.removeItem(at: root) }
        let preferences = PreferencesService(
            fileURL: root.appendingPathComponent("prefs.plist"),
            defaultBottleDirectory: root
        )
        let service = AppUpdateService(preferences: preferences, jobs: BridgeJobStore())
        let automatic = ["automatic": JSONValue.bool(true)]
        let defaults = try preferences.call("preferences.read", params: [:])
        #expect(defaults.object?["checkMaltoriaAppUpdates"] == .bool(false))
        let skipped = try service.call("appUpdate.check", params: automatic)
        #expect(skipped.object?["skipped"] == .bool(true))
        let invalid = BridgeFailure("invalid_request", "The installed app version or package type is unknown")
        #expect(throws: invalid) {
            try service.call("appUpdate.check", params: [:])
        }
        _ = try preferences.call(
            "preferences.set", params: ["key": .string("checkMaltoriaAppUpdates"), "value": .bool(true)]
        )
        #expect(throws: invalid) {
            try service.call("appUpdate.check", params: automatic)
        }
    }

    @Test @MainActor func offlineBlocksManualCheckAndDownloadAndOptOutSkipsAutomaticCheck() throws {
        let root = FileManager.default.temporaryDirectory.appendingPathComponent("maltoria-update-\(UUID())")
        defer { try? FileManager.default.removeItem(at: root) }
        let preferences = PreferencesService(
            fileURL: root.appendingPathComponent("prefs.plist"),
            defaultBottleDirectory: root
        )
        let service = AppUpdateService(preferences: preferences, jobs: BridgeJobStore())
        _ = try preferences.call("preferences.set", params: ["key": .string("offlineMode"), "value": .bool(true)])
        for method in AppUpdateService.methods {
            do {
                _ = try service.call(method, params: [:])
                Issue.record("Offline update calls must fail without starting a job")
            } catch let error as BridgeFailure {
                #expect(error.code == "offline")
            }
        }
        _ = try preferences.call("preferences.setMany", params: ["values": .object([
            "offlineMode": .bool(false), "checkMaltoriaAppUpdates": .bool(false)
        ])])
        let skipped = try service.call("appUpdate.check", params: ["automatic": .bool(true)])
        #expect(skipped.object?["skipped"] == .bool(true))
    }
}
