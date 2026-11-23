//
//  AppUpdateRelease.swift
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

struct AppUpdateVersion: Comparable, Sendable, CustomStringConvertible {
    private let parts: [UInt32]

    init?(_ version: String) {
        let fields = version.split(separator: ".", omittingEmptySubsequences: false)
        guard fields.count == 3 else { return nil }
        var parts: [UInt32] = []
        for field in fields {
            guard let part = UInt32(field), String(part) == field else { return nil }
            parts.append(part)
        }
        self.parts = parts
    }

    static func < (lhs: Self, rhs: Self) -> Bool {
        lhs.parts.lexicographicallyPrecedes(rhs.parts)
    }

    var description: String {
        parts.map(String.init).joined(separator: ".")
    }
}

enum AppUpdateFormat: String, Sendable {
    case macOS = "macos"
    case appImage = "appimage"
    case deb
    case rpm

    func assetName(version: AppUpdateVersion) -> String {
        switch self {
        case .macOS: "Maltoria-\(version)-arm64.zip"
        case .appImage: "Maltoria-\(version)-x86_64.AppImage"
        case .deb: "whisky_\(version)_amd64.deb"
        case .rpm: "whisky-\(version)-1.x86_64.rpm"
        }
    }

    var sumsName: String {
        self == .macOS ? "SHA256SUMS-macos-arm64" : "SHA256SUMS-linux-amd64"
    }
}

struct AppUpdateAsset: Decodable, Sendable {
    let name: String
    let state: String
    let size: Int64
    let digest: String?
    let browserDownloadURL: URL

    enum CodingKeys: String, CodingKey {
        case name, state, size, digest
        case browserDownloadURL = "browser_download_url"
    }

    var sha256: String? {
        guard let digest, digest.hasPrefix("sha256:") else { return nil }
        let hash = String(digest.dropFirst(7))
        return Self.isHash(hash) ? hash : nil
    }

    static func isHash(_ hash: String) -> Bool {
        hash.utf8.count == 64 && hash.utf8.allSatisfy { (48 ... 57).contains($0) || (97 ... 102).contains($0) }
    }
}

struct AppUpdateRelease: Decodable, Sendable {
    static let repository = "daydalek/Maltoria"
    let tag: String
    let draft: Bool
    let assets: [AppUpdateAsset]

    enum CodingKeys: String, CodingKey {
        case tag = "tag_name"
        case draft, assets
    }

    struct Candidate: Sendable {
        let version: AppUpdateVersion
        let asset: AppUpdateAsset
        let sums: AppUpdateAsset
    }

    static func select(
        _ data: Data, format: AppUpdateFormat, current: AppUpdateVersion, downloadBase: URL
    ) throws -> Candidate? {
        let releases = try JSONDecoder().decode([Self].self, from: data)
        return releases.compactMap { release in
            release.candidate(format: format, current: current, downloadBase: downloadBase)
        }.max { $0.version < $1.version }
    }

    private func candidate(
        format: AppUpdateFormat, current: AppUpdateVersion, downloadBase: URL
    ) -> Candidate? {
        guard !draft, tag.hasPrefix("v"), let version = AppUpdateVersion(String(tag.dropFirst())), version > current
        else { return nil }
        for platform: AppUpdateFormat in [.macOS, .appImage, .deb, .rpm] {
            guard asset(platform.assetName(version: version), base: downloadBase, limit: 1_073_741_824) != nil,
                  asset(platform.sumsName, base: downloadBase, limit: 1_048_576) != nil
            else { return nil }
        }
        guard let archive = asset(format.assetName(version: version), base: downloadBase, limit: 1_073_741_824),
              let sums = asset(format.sumsName, base: downloadBase, limit: 1_048_576)
        else { return nil }
        return Candidate(version: version, asset: archive, sums: sums)
    }

    private func asset(_ name: String, base: URL, limit: Int64) -> AppUpdateAsset? {
        let matches = assets.filter { $0.name == name }
        guard matches.count == 1, let asset = matches.first,
              asset.state == "uploaded", asset.sha256 != nil, (1 ... limit).contains(asset.size),
              asset.browserDownloadURL.absoluteString == base.appendingPathComponent("\(tag)/\(name)").absoluteString
        else { return nil }
        return asset
    }

    static func verifySums(_ data: Data, asset: AppUpdateAsset) throws {
        guard let text = String(data: data, encoding: .utf8) else { throw invalidSums }
        var checksums: [String: String] = [:]
        for line in text.split(separator: "\n") {
            let fields = line.split(separator: " ", omittingEmptySubsequences: true)
            guard fields.count == 2, let digest = fields.first, let file = fields.last,
                  AppUpdateAsset.isHash(String(digest)), checksums[String(file)] == nil
            else { throw invalidSums }
            checksums[String(file)] = String(digest)
        }
        guard let expected = asset.sha256, checksums[asset.name] == expected else { throw invalidSums }
    }

    private static var invalidSums: BridgeFailure {
        BridgeFailure("checksum_mismatch", "The app update checksums disagree with GitHub release metadata")
    }
}
