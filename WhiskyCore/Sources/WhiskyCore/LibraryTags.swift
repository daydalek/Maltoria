//
//  LibraryTags.swift
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

/// Per-bottle custom tag assignments keyed by `LibraryEntry.metadataIdentity`.
public struct LibraryTags: Codable, Sendable, Equatable {
    public static let maxTagsPerEntry = 32
    public static let maxTagLength = 32

    public var entryTags: [String: [String]] = [:]

    private enum CodingKeys: String, CodingKey {
        case entryTags
    }

    public init(entryTags: [String: [String]] = [:]) {
        self.entryTags = entryTags
    }

    public init(from decoder: Decoder) throws {
        let values = try decoder.container(keyedBy: CodingKeys.self)
        entryTags = try values.decodeIfPresent([String: [String]].self, forKey: .entryTags) ?? [:]
    }

    public static func location(_ bottleURL: URL) -> URL {
        bottleURL.appendingPathComponent("Library Tags.json")
    }

    public static func load(bottleURL: URL) throws -> Self {
        let url = location(bottleURL)
        guard FileManager.default.fileExists(atPath: url.path) else { return Self() }
        return try JSONDecoder().decode(Self.self, from: Data(contentsOf: url))
    }

    public func save(bottleURL: URL) throws {
        try JSONEncoder().encode(self).write(to: Self.location(bottleURL), options: .atomic)
    }

    public static func normalize(_ raw: String) -> String? {
        guard !raw.unicodeScalars.contains(where: { $0.value == 0 }) else { return nil }
        let collapsed = raw
            .components(separatedBy: .whitespacesAndNewlines)
            .filter { !$0.isEmpty }
            .joined(separator: " ")
        guard !collapsed.isEmpty else { return nil }
        let clamped = String(collapsed.prefix(maxTagLength))
            .trimmingCharacters(in: .whitespacesAndNewlines)
        return clamped.isEmpty ? nil : clamped
    }

    public static func normalizeList(_ raw: [String]) -> [String] {
        var seen = Set<String>()
        var result: [String] = []
        for item in raw {
            guard let normalized = normalize(item) else { continue }
            let key = normalized.lowercased()
            guard seen.insert(key).inserted else { continue }
            result.append(normalized)
            if result.count >= maxTagsPerEntry { break }
        }
        return result
    }

    public func tags(for entry: LibraryEntry) -> [String] {
        tags(forIdentity: entry.metadataIdentity)
    }

    public func tags(forIdentity identity: String) -> [String] {
        entryTags[identity] ?? []
    }

    @discardableResult
    public mutating func setTags(_ rawTags: [String], for entry: LibraryEntry) -> [String] {
        setTags(rawTags, forIdentity: entry.metadataIdentity)
    }

    @discardableResult
    public mutating func setTags(_ rawTags: [String], forIdentity identity: String) -> [String] {
        let normalized = Self.normalizeList(rawTags)
        if normalized.isEmpty {
            entryTags.removeValue(forKey: identity)
        } else {
            entryTags[identity] = normalized
        }
        return normalized
    }

    @discardableResult
    public mutating func renameTag(from oldName: String, to newName: String) -> Bool {
        guard let oldNormalized = Self.normalize(oldName),
              let newNormalized = Self.normalize(newName)
        else { return false }
        let oldKey = oldNormalized.lowercased()
        var changed = false
        for identity in Array(entryTags.keys) {
            guard let current = entryTags[identity] else { continue }
            guard current.contains(where: { $0.lowercased() == oldKey }) else { continue }
            let mapped = current.map { $0.lowercased() == oldKey ? newNormalized : $0 }
            let updated = Self.normalizeList(mapped)
            if updated != current {
                if updated.isEmpty {
                    entryTags.removeValue(forKey: identity)
                } else {
                    entryTags[identity] = updated
                }
                changed = true
            }
        }
        return changed
    }

    @discardableResult
    public mutating func deleteTag(_ name: String) -> Bool {
        guard let normalized = Self.normalize(name) else { return false }
        let targetKey = normalized.lowercased()
        var changed = false
        for identity in Array(entryTags.keys) {
            guard let current = entryTags[identity] else { continue }
            let filtered = current.filter { $0.lowercased() != targetKey }
            if filtered.count != current.count {
                if filtered.isEmpty {
                    entryTags.removeValue(forKey: identity)
                } else {
                    entryTags[identity] = filtered
                }
                changed = true
            }
        }
        return changed
    }
}
