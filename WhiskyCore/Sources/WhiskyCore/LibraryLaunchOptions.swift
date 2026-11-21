//
//  LibraryLaunchOptions.swift
//  WhiskyCore
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

/// Per-entry launch configuration for works that expose multiple `.exe` entry points.
public struct EntryLaunchOptions: Codable, Sendable, Equatable {
    public var multiExecutable: Bool = false
    public var executables: [String] = []
    public var lastExecutable: String?

    private enum CodingKeys: String, CodingKey {
        case multiExecutable, executables, lastExecutable
    }

    public init(
        multiExecutable: Bool = false,
        executables: [String] = [],
        lastExecutable: String? = nil
    ) {
        self.multiExecutable = multiExecutable
        self.executables = executables
        self.lastExecutable = lastExecutable
    }

    public init(from decoder: Decoder) throws {
        let values = try decoder.container(keyedBy: CodingKeys.self)
        multiExecutable = try values.decodeIfPresent(Bool.self, forKey: .multiExecutable) ?? false
        executables = try values.decodeIfPresent([String].self, forKey: .executables) ?? []
        lastExecutable = try values.decodeIfPresent(String.self, forKey: .lastExecutable)
    }

    public var isDefault: Bool {
        !multiExecutable && executables.isEmpty && lastExecutable == nil
    }
}

/// Per-bottle launch entry options keyed by `LibraryEntry.metadataIdentity`.
public struct LibraryLaunchOptions: Codable, Sendable, Equatable {
    public var entries: [String: EntryLaunchOptions] = [:]

    private enum CodingKeys: String, CodingKey {
        case entries
    }

    public init(entries: [String: EntryLaunchOptions] = [:]) {
        self.entries = entries
    }

    public init(from decoder: Decoder) throws {
        let values = try decoder.container(keyedBy: CodingKeys.self)
        entries = try values.decodeIfPresent([String: EntryLaunchOptions].self, forKey: .entries) ?? [:]
    }

    public static func location(_ bottleURL: URL) -> URL {
        bottleURL.appendingPathComponent("Library Launch Options.json")
    }

    public static func load(bottleURL: URL) throws -> Self {
        let url = location(bottleURL)
        guard FileManager.default.fileExists(atPath: url.path) else { return Self() }
        return try JSONDecoder().decode(Self.self, from: Data(contentsOf: url))
    }

    public func save(bottleURL: URL) throws {
        try JSONEncoder().encode(self).write(to: Self.location(bottleURL), options: .atomic)
    }

    public func options(for entry: LibraryEntry) -> EntryLaunchOptions {
        options(forIdentity: entry.metadataIdentity)
    }

    public func options(forIdentity identity: String) -> EntryLaunchOptions {
        entries[identity] ?? EntryLaunchOptions()
    }

    public mutating func setOptions(_ options: EntryLaunchOptions, for entry: LibraryEntry) {
        setOptions(options, forIdentity: entry.metadataIdentity)
    }

    public mutating func setOptions(_ options: EntryLaunchOptions, forIdentity identity: String) {
        if options.isDefault {
            entries.removeValue(forKey: identity)
        } else {
            entries[identity] = options
        }
    }

    public func resolvedExecutables(for entry: LibraryEntry, existingOnly: Bool = true) -> [URL] {
        var seen = Set<String>()
        var resolved: [URL] = []
        for key in options(for: entry).executables {
            let url = FolderGameLibrary.url(for: key, bottleURL: entry.bottleURL).standardizedFileURL
            guard url.pathExtension.lowercased() == "exe" else { continue }
            guard seen.insert(url.path).inserted else { continue }
            if existingOnly {
                guard (try? url.resourceValues(forKeys: [.isRegularFileKey]).isRegularFile) == true else {
                    continue
                }
            }
            resolved.append(url)
        }
        return resolved
    }

    public func resolvedLastExecutable(for entry: LibraryEntry, existingOnly: Bool = true) -> URL? {
        guard let key = options(for: entry).lastExecutable else { return nil }
        let url = FolderGameLibrary.url(for: key, bottleURL: entry.bottleURL).standardizedFileURL
        guard url.pathExtension.lowercased() == "exe" else { return nil }
        let allowed = resolvedExecutables(for: entry, existingOnly: existingOnly)
        if !allowed.isEmpty {
            return allowed.first(where: { $0.path == url.path })
        }
        if existingOnly {
            guard (try? url.resourceValues(forKeys: [.isRegularFileKey]).isRegularFile) == true else {
                return nil
            }
        }
        return url
    }
}
