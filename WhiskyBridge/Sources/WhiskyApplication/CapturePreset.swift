//
//  CapturePreset.swift
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

// SPDX-License-Identifier: GPL-3.0-or-later
import Foundation
import WhiskyBridgeProtocol

public struct CaptureThreadBinding: Codable, Equatable, Sendable {
    public var role: String
    public var hookCode: String
    public var hookName: String
    public var contextHex: String
    public var context2Hex: String

    public init(
        role: String,
        hookCode: String,
        hookName: String,
        contextHex: String,
        context2Hex: String
    ) {
        self.role = role
        self.hookCode = hookCode
        self.hookName = hookName
        self.contextHex = contextHex
        self.context2Hex = context2Hex
    }
}

public struct CapturePreset: Codable, Equatable, Sendable {
    public var providerID: UUID?
    public var architecture: String
    public var restoreHistory: Bool
    public var autoAttach: Bool
    public var targetExePath: String?
    public var customHookCodes: [String]
    public var threadBindings: [CaptureThreadBinding]
    public var translationEnabled: Bool
    public var promptAttachOnLaunch: Bool

    public init(
        providerID: UUID? = nil,
        architecture: String = "x64",
        restoreHistory: Bool = false,
        autoAttach: Bool = true,
        targetExePath: String? = nil,
        customHookCodes: [String] = [],
        threadBindings: [CaptureThreadBinding] = [],
        translationEnabled: Bool = false,
        promptAttachOnLaunch: Bool = true
    ) {
        self.providerID = providerID
        self.architecture = architecture
        self.restoreHistory = restoreHistory
        self.autoAttach = autoAttach
        self.targetExePath = targetExePath
        self.customHookCodes = customHookCodes
        self.threadBindings = threadBindings
        self.translationEnabled = translationEnabled
        self.promptAttachOnLaunch = promptAttachOnLaunch
    }

    private enum CodingKeys: String, CodingKey {
        case providerID
        case architecture
        case restoreHistory
        case autoAttach
        case targetExePath
        case customHookCodes
        case threadBindings
        case translationEnabled
        case promptAttachOnLaunch
    }

    public init(from decoder: any Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        let providerID = try container.decodeIfPresent(UUID.self, forKey: .providerID)
        let architecture = try container.decodeIfPresent(String.self, forKey: .architecture) ?? "x64"
        let restoreHistory = try container.decodeIfPresent(Bool.self, forKey: .restoreHistory) ?? false
        let autoAttach = try container.decodeIfPresent(Bool.self, forKey: .autoAttach) ?? true
        let targetExePath = try container.decodeIfPresent(String.self, forKey: .targetExePath)
        let customHookCodes = try container.decodeIfPresent([String].self, forKey: .customHookCodes) ?? []
        let threadBindings = try container.decodeIfPresent([CaptureThreadBinding].self, forKey: .threadBindings) ?? []
        let translationEnabled = try container.decodeIfPresent(Bool.self, forKey: .translationEnabled)
            ?? (providerID != nil || targetExePath != nil || !customHookCodes.isEmpty || !threadBindings.isEmpty)
        let promptAttachOnLaunch = try container.decodeIfPresent(Bool.self, forKey: .promptAttachOnLaunch) ?? true
        self.init(
            providerID: providerID,
            architecture: architecture,
            restoreHistory: restoreHistory,
            autoAttach: autoAttach,
            targetExePath: targetExePath,
            customHookCodes: customHookCodes,
            threadBindings: threadBindings,
            translationEnabled: translationEnabled,
            promptAttachOnLaunch: promptAttachOnLaunch
        )
    }
}

actor CapturePresetRepository {
    private static let maxFileBytes = 1_048_576
    let file: URL
    let gameID: String

    init(directory: URL, gameID: String) {
        self.gameID = gameID
        file = directory.appendingPathComponent("capture-presets").appendingPathComponent(gameID + ".json")
    }

    func load() throws -> CapturePreset? {
        try validateGameID()
        guard FileManager.default.fileExists(atPath: file.path) else { return nil }
        let data = try Data(contentsOf: file)
        guard data.count <= Self.maxFileBytes else { throw hookFailure("preset_too_large") }
        return try JSONDecoder().decode(CapturePreset.self, from: data)
    }

    func save(_ preset: CapturePreset) throws {
        try validateGameID()
        let manager = FileManager.default
        try manager.createDirectory(
            at: file.deletingLastPathComponent(),
            withIntermediateDirectories: true,
            attributes: [.posixPermissions: 0o700]
        )
        let encoder = JSONEncoder()
        encoder.outputFormatting = [.sortedKeys]
        try encoder.encode(preset).write(to: file, options: .atomic)
        try manager.setAttributes([.posixPermissions: 0o600], ofItemAtPath: file.path)
    }

    func clear() throws {
        try validateGameID()
        if FileManager.default.fileExists(atPath: file.path) {
            try FileManager.default.removeItem(at: file)
        }
    }

    private func validateGameID() throws {
        guard !gameID.isEmpty, !gameID.contains("/"), !gameID.contains("\\"), !gameID.contains("..") else {
            throw hookFailure("invalid_game_id")
        }
    }
}

enum CapturePresetMatcher {
    static func matchProcess(
        targetExePath: String,
        preferredArchitecture: String,
        in processes: [JSONValue]
    ) -> JSONValue? {
        let exactMatches = processes.filter { candidate in
            guard let path = candidate.object?["exePath"]?.string else { return false }
            return path.caseInsensitiveCompare(targetExePath) == .orderedSame
        }
        if let match = preferredProcess(in: exactMatches, preferredArchitecture: preferredArchitecture) {
            return match
        }
        let targetName = windowsBasename(targetExePath)
        guard !targetName.isEmpty else { return nil }
        let basenameMatches = processes.filter { candidate in
            guard let path = candidate.object?["exePath"]?.string else { return false }
            return windowsBasename(path).caseInsensitiveCompare(targetName) == .orderedSame
        }
        return preferredProcess(in: basenameMatches, preferredArchitecture: preferredArchitecture)
    }

    static func matchPendingBinding(
        for thread: [String: JSONValue],
        in pendingBindings: [CaptureThreadBinding],
        activeRoles: [String: String]
    ) -> Int? {
        let hookCode = thread["hookCode"]?.string ?? ""
        let hookName = thread["hookName"]?.string ?? ""
        let contextHex = thread["contextHex"]?.string ?? ""
        let context2Hex = thread["context2Hex"]?.string ?? ""

        if !hookCode.isEmpty,
           let index = pendingBindings.firstIndex(where: { binding in
               isRoleSlotAvailable(binding.role, activeRoles: activeRoles)
                   && !binding.hookCode.isEmpty
                   && binding.hookCode == hookCode
                   && binding.contextHex == contextHex
                   && binding.context2Hex == context2Hex
           }) {
            return index
        }

        if !hookCode.isEmpty,
           let index = pendingBindings.firstIndex(where: { binding in
               isRoleSlotAvailable(binding.role, activeRoles: activeRoles)
                   && !binding.hookCode.isEmpty
                   && binding.hookCode == hookCode
                   && binding.contextHex == contextHex
                   && pendingBindings.count(where: {
                       $0.hookCode == binding.hookCode && $0.contextHex == binding.contextHex
                   }) == 1
           }) {
            return index
        }

        if !hookName.isEmpty, !hookName.hasPrefix("UserHook"),
           let index = pendingBindings.firstIndex(where: { binding in
               isRoleSlotAvailable(binding.role, activeRoles: activeRoles)
                   && !binding.hookName.isEmpty
                   && !binding.hookName.hasPrefix("UserHook")
                   && binding.hookName == hookName
                   && binding.contextHex == contextHex
                   && binding.context2Hex == context2Hex
           }) {
            return index
        }

        return nil
    }

    static func isRoleSlotAvailable(_ role: String, activeRoles: [String: String]) -> Bool {
        switch role {
        case "body": !activeRoles.values.contains("body")
        case "speaker": !activeRoles.values.contains("speaker")
        case "choice": true
        default: false
        }
    }

    private static func preferredProcess(
        in candidates: [JSONValue],
        preferredArchitecture: String
    ) -> JSONValue? {
        candidates.first { $0.object?["arch"]?.string == preferredArchitecture } ?? candidates.first
    }

    private static func windowsBasename(_ path: String) -> String {
        path.split { $0 == "\\" || $0 == "/" }.last.map(String.init) ?? ""
    }
}
