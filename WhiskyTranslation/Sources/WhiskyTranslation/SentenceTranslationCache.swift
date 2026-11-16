//
//  SentenceTranslationCache.swift
//  WhiskyTranslation
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

public struct SentenceCacheRecord: Codable, Equatable, Hashable, Identifiable, Sendable {
    public let id: String
    public let gameID: String
    public let speaker: String?
    public let sourceText: String
    public let translatedText: String
    public let sourceLanguage: String
    public let targetLanguage: String
    public let providerName: String
    public let providerKind: ProviderKind
    public let modelID: String
    public let customPrompt: String
    public var hitCount: Int
    public let createdAt: Date
    public var updatedAt: Date

    public init(
        id: String,
        gameID: String,
        speaker: String?,
        sourceText: String,
        translatedText: String,
        sourceLanguage: String,
        targetLanguage: String,
        providerName: String,
        providerKind: ProviderKind,
        modelID: String,
        customPrompt: String,
        hitCount: Int = 1,
        createdAt: Date = Date(),
        updatedAt: Date = Date()
    ) {
        self.id = id
        self.gameID = gameID
        self.speaker = speaker
        self.sourceText = sourceText
        self.translatedText = translatedText
        self.sourceLanguage = sourceLanguage
        self.targetLanguage = targetLanguage
        self.providerName = providerName
        self.providerKind = providerKind
        self.modelID = modelID
        self.customPrompt = customPrompt
        self.hitCount = hitCount
        self.createdAt = createdAt
        self.updatedAt = updatedAt
    }

    public static func makeID(
        gameID: String,
        sourceText: String,
        speaker: String?,
        sourceLanguage: String,
        targetLanguage: String,
        providerKind: ProviderKind,
        modelID: String,
        customPrompt: String
    ) -> String {
        let normalizedSource = sourceText.trimmingCharacters(in: .whitespacesAndNewlines)
        let components = [
            gameID,
            normalizedSource,
            speaker ?? "",
            sourceLanguage,
            targetLanguage,
            providerKind.rawValue,
            modelID,
            customPrompt
        ]
        var data = Data()
        for component in components {
            let utf8 = Data(component.utf8)
            var length = UInt32(utf8.count).bigEndian
            withUnsafeBytes(of: &length) { data.append(contentsOf: $0) }
            data.append(utf8)
        }
        return SentenceCacheHash.sha256Hex(data)
    }
}

public protocol SentenceTranslationCaching: Sendable {
    func lookup(
        gameID: String,
        sourceText: String,
        speaker: String?,
        sourceLanguage: String,
        targetLanguage: String,
        providerKind: ProviderKind,
        modelID: String,
        customPrompt: String
    ) async throws -> SentenceCacheRecord?

    @discardableResult
    // swiftlint:disable:next function_parameter_count
    func store(
        gameID: String,
        sourceText: String,
        translatedText: String,
        speaker: String?,
        sourceLanguage: String,
        targetLanguage: String,
        providerName: String,
        providerKind: ProviderKind,
        modelID: String,
        customPrompt: String
    ) async throws -> SentenceCacheRecord?

    func list(gameID: String?, query: String?, limit: Int?) async throws -> [SentenceCacheRecord]

    @discardableResult
    func delete(id: String) async throws -> Bool

    func clear(gameID: String?) async throws

    func stats(gameID: String?) async throws -> (totalCount: Int, gameCount: Int, totalBytes: Int)
}

public extension SentenceTranslationCaching {
    func clear() async throws {
        try await clear(gameID: nil)
    }

    func stats() async throws -> (totalCount: Int, gameCount: Int, totalBytes: Int) {
        try await stats(gameID: nil)
    }
}

public actor PersistentSentenceCache: SentenceTranslationCaching {
    private struct Snapshot: Codable {
        var version: Int = 1
        var records: [SentenceCacheRecord] = []
    }

    public let fileURL: URL
    public let maximumEntries: Int
    private let fileManager: FileManager
    private var loaded = false
    private var records: [String: SentenceCacheRecord] = [:]
    private var recencyOrder: [String] = []

    public init(
        fileURL: URL,
        maximumEntries: Int = 5_000,
        fileManager: FileManager = .default
    ) {
        self.fileURL = fileURL
        self.maximumEntries = max(1, maximumEntries)
        self.fileManager = fileManager
    }

    public func lookup(
        gameID: String,
        sourceText: String,
        speaker: String?,
        sourceLanguage: String,
        targetLanguage: String,
        providerKind: ProviderKind,
        modelID: String,
        customPrompt: String
    ) async throws -> SentenceCacheRecord? {
        try ensureLoaded()
        let normalizedSource = sourceText.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !normalizedSource.isEmpty else { return nil }
        let id = SentenceCacheRecord.makeID(
            gameID: gameID,
            sourceText: normalizedSource,
            speaker: speaker,
            sourceLanguage: sourceLanguage,
            targetLanguage: targetLanguage,
            providerKind: providerKind,
            modelID: modelID,
            customPrompt: customPrompt
        )
        guard var record = records[id] else { return nil }
        record.hitCount += 1
        record.updatedAt = Date()
        records[id] = record
        touchRecency(id)
        try persist()
        return record
    }

    @discardableResult
    // swiftlint:disable:next function_parameter_count
    public func store(
        gameID: String,
        sourceText: String,
        translatedText: String,
        speaker: String?,
        sourceLanguage: String,
        targetLanguage: String,
        providerName: String,
        providerKind: ProviderKind,
        modelID: String,
        customPrompt: String
    ) async throws -> SentenceCacheRecord? {
        try ensureLoaded()
        let normalizedSource = sourceText.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !normalizedSource.isEmpty,
              !translatedText.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
        else { return nil }
        let id = SentenceCacheRecord.makeID(
            gameID: gameID,
            sourceText: normalizedSource,
            speaker: speaker,
            sourceLanguage: sourceLanguage,
            targetLanguage: targetLanguage,
            providerKind: providerKind,
            modelID: modelID,
            customPrompt: customPrompt
        )
        let now = Date()
        let record: SentenceCacheRecord
        if var existing = records[id] {
            existing = SentenceCacheRecord(
                id: id,
                gameID: gameID,
                speaker: speaker,
                sourceText: normalizedSource,
                translatedText: translatedText,
                sourceLanguage: sourceLanguage,
                targetLanguage: targetLanguage,
                providerName: providerName,
                providerKind: providerKind,
                modelID: modelID,
                customPrompt: customPrompt,
                hitCount: existing.hitCount,
                createdAt: existing.createdAt,
                updatedAt: now
            )
            record = existing
        } else {
            record = SentenceCacheRecord(
                id: id,
                gameID: gameID,
                speaker: speaker,
                sourceText: normalizedSource,
                translatedText: translatedText,
                sourceLanguage: sourceLanguage,
                targetLanguage: targetLanguage,
                providerName: providerName,
                providerKind: providerKind,
                modelID: modelID,
                customPrompt: customPrompt,
                hitCount: 1,
                createdAt: now,
                updatedAt: now
            )
        }
        records[id] = record
        touchRecency(id)
        while recencyOrder.count > maximumEntries {
            let evicted = recencyOrder.removeFirst()
            records.removeValue(forKey: evicted)
        }
        try persist()
        return record
    }

    public func list(
        gameID: String? = nil,
        query: String? = nil,
        limit: Int? = nil
    ) async throws -> [SentenceCacheRecord] {
        try ensureLoaded()
        let trimmedGameID = gameID?.trimmingCharacters(in: .whitespacesAndNewlines)
        let filterGameID = (trimmedGameID?.isEmpty == false) ? trimmedGameID : nil
        let trimmedQuery = query?.trimmingCharacters(in: .whitespacesAndNewlines)
        let filterQuery = (trimmedQuery?.isEmpty == false) ? trimmedQuery : nil

        let filtered = records.values.filter { record in
            if let filterGameID, record.gameID != filterGameID { return false }
            if let filterQuery {
                let matches = record.sourceText.localizedCaseInsensitiveContains(filterQuery)
                    || record.translatedText.localizedCaseInsensitiveContains(filterQuery)
                    || (record.speaker?.localizedCaseInsensitiveContains(filterQuery) == true)
                    || record.modelID.localizedCaseInsensitiveContains(filterQuery)
                    || record.providerName.localizedCaseInsensitiveContains(filterQuery)
                if !matches { return false }
            }
            return true
        }
        let sorted = filtered.sorted { lhs, rhs in
            if lhs.updatedAt != rhs.updatedAt { return lhs.updatedAt > rhs.updatedAt }
            return lhs.id < rhs.id
        }
        if let limit {
            return Array(sorted.prefix(max(0, limit)))
        }
        return sorted
    }

    @discardableResult
    public func delete(id: String) async throws -> Bool {
        try ensureLoaded()
        guard records.removeValue(forKey: id) != nil else { return false }
        recencyOrder.removeAll { $0 == id }
        try persist()
        return true
    }

    public func clear(gameID: String? = nil) async throws {
        try ensureLoaded()
        let trimmedGameID = gameID?.trimmingCharacters(in: .whitespacesAndNewlines)
        if let trimmedGameID, !trimmedGameID.isEmpty {
            let matchingIDs = Set(records.values.filter { $0.gameID == trimmedGameID }.map(\.id))
            guard !matchingIDs.isEmpty else { return }
            for id in matchingIDs {
                records.removeValue(forKey: id)
            }
            recencyOrder.removeAll { matchingIDs.contains($0) }
            try persist()
        } else {
            records.removeAll()
            recencyOrder.removeAll()
            try persist()
        }
    }

    public func stats(gameID: String? = nil) async throws -> (totalCount: Int, gameCount: Int, totalBytes: Int) {
        try ensureLoaded()
        let totalCount = records.count
        let trimmedGameID = gameID?.trimmingCharacters(in: .whitespacesAndNewlines)
        let gameCount: Int = if let trimmedGameID, !trimmedGameID.isEmpty {
            records.values.filter { $0.gameID == trimmedGameID }.count
        } else {
            totalCount
        }
        let totalBytes: Int = if fileManager.fileExists(atPath: fileURL.path),
                                 let attributes = try? fileManager.attributesOfItem(atPath: fileURL.path),
                                 let size = attributes[.size] as? NSNumber {
            size.intValue
        } else {
            0
        }
        return (totalCount: totalCount, gameCount: gameCount, totalBytes: totalBytes)
    }

    private func touchRecency(_ id: String) {
        recencyOrder.removeAll { $0 == id }
        recencyOrder.append(id)
    }

    private func ensureLoaded() throws {
        guard !loaded else { return }
        guard fileManager.fileExists(atPath: fileURL.path) else {
            records = [:]
            recencyOrder = []
            loaded = true
            return
        }
        do {
            let data = try Data(contentsOf: fileURL)
            let snapshot = try JSONDecoder().decode(Snapshot.self, from: data)
            guard snapshot.version == 1 else {
                throw TranslationError.persistenceUnavailable
            }
            let ordered = snapshot.records.sorted { lhs, rhs in
                if lhs.updatedAt != rhs.updatedAt { return lhs.updatedAt < rhs.updatedAt }
                return lhs.id < rhs.id
            }
            records = Dictionary(uniqueKeysWithValues: ordered.map { ($0.id, $0) })
            recencyOrder = ordered.map(\.id)
            while recencyOrder.count > maximumEntries {
                let evicted = recencyOrder.removeFirst()
                records.removeValue(forKey: evicted)
            }
            loaded = true
        } catch let error as TranslationError {
            throw error
        } catch {
            throw TranslationError.persistenceUnavailable
        }
    }

    private func persist() throws {
        do {
            if records.isEmpty {
                if fileManager.fileExists(atPath: fileURL.path) {
                    try fileManager.removeItem(at: fileURL)
                }
                return
            }
            let directory = fileURL.deletingLastPathComponent()
            try fileManager.createDirectory(at: directory, withIntermediateDirectories: true)
            try fileManager.setAttributes([.posixPermissions: 0o700], ofItemAtPath: directory.path)
            let orderedRecords = recencyOrder.compactMap { records[$0] }
            let data = try JSONEncoder.sorted.encode(Snapshot(records: orderedRecords))
            try data.write(to: fileURL, options: .atomic)
            try fileManager.setAttributes([.posixPermissions: 0o600], ofItemAtPath: fileURL.path)
        } catch {
            throw TranslationError.persistenceUnavailable
        }
    }
}

private enum SentenceCacheHash {
    private static let constants: [UInt32] = [
        0x428A_2F98, 0x7137_4491, 0xB5C0_FBCF, 0xE9B5_DBA5, 0x3956_C25B, 0x59F1_11F1, 0x923F_82A4, 0xAB1C_5ED5,
        0xD807_AA98, 0x1283_5B01, 0x2431_85BE, 0x550C_7DC3, 0x72BE_5D74, 0x80DE_B1FE, 0x9BDC_06A7, 0xC19B_F174,
        0xE49B_69C1, 0xEFBE_4786, 0x0FC1_9DC6, 0x240C_A1CC, 0x2DE9_2C6F, 0x4A74_84AA, 0x5CB0_A9DC, 0x76F9_88DA,
        0x983E_5152, 0xA831_C66D, 0xB003_27C8, 0xBF59_7FC7, 0xC6E0_0BF3, 0xD5A7_9147, 0x06CA_6351, 0x1429_2967,
        0x27B7_0A85, 0x2E1B_2138, 0x4D2C_6DFC, 0x5338_0D13, 0x650A_7354, 0x766A_0ABB, 0x81C2_C92E, 0x9272_2C85,
        0xA2BF_E8A1, 0xA81A_664B, 0xC24B_8B70, 0xC76C_51A3, 0xD192_E819, 0xD699_0624, 0xF40E_3585, 0x106A_A070,
        0x19A4_C116, 0x1E37_6C08, 0x2748_774C, 0x34B0_BCB5, 0x391C_0CB3, 0x4ED8_AA4A, 0x5B9C_CA4F, 0x682E_6FF3,
        0x748F_82EE, 0x78A5_636F, 0x84C8_7814, 0x8CC7_0208, 0x90BE_FFFA, 0xA450_6CEB, 0xBEF9_A3F7, 0xC671_78F2
    ]

    static func sha256Hex(_ data: Data) -> String {
        var bytes = Array(data)
        let bitCount = UInt64(bytes.count) * 8
        bytes.append(0x80)
        while bytes.count % 64 != 56 {
            bytes.append(0)
        }
        bytes += (0 ..< 8).reversed().map { UInt8(truncatingIfNeeded: bitCount >> ($0 * 8)) }
        var hash: [UInt32] = [
            0x6A09_E667, 0xBB67_AE85, 0x3C6E_F372, 0xA54F_F53A,
            0x510E_527F, 0x9B05_688C, 0x1F83_D9AB, 0x5BE0_CD19
        ]
        for start in stride(from: 0, to: bytes.count, by: 64) {
            compress(bytes, start: start, hash: &hash)
        }
        return hash.map { String(format: "%08x", $0) }.joined()
    }

    private static func compress(_ bytes: [UInt8], start: Int, hash: inout [UInt32]) {
        var words = [UInt32](repeating: 0, count: 64)
        for index in 0 ..< 16 {
            let offset = start + index * 4
            words[index] = (0 ..< 4).reduce(UInt32(0)) { ($0 << 8) | UInt32(bytes[offset + $1]) }
        }
        for index in 16 ..< 64 {
            let word15 = words[index - 15], word2 = words[index - 2]
            let sigma0 = rotate(word15, 7) ^ rotate(word15, 18) ^ (word15 >> 3)
            let sigma1 = rotate(word2, 17) ^ rotate(word2, 19) ^ (word2 >> 10)
            words[index] = words[index - 16] &+ sigma0 &+ words[index - 7] &+ sigma1
        }
        var stateA = hash[0], stateB = hash[1], stateC = hash[2], stateD = hash[3]
        var stateE = hash[4], stateF = hash[5], stateG = hash[6], stateH = hash[7]
        for index in 0 ..< 64 {
            let sum1 = rotate(stateE, 6) ^ rotate(stateE, 11) ^ rotate(stateE, 25)
            let choice = (stateE & stateF) ^ (~stateE & stateG)
            let temp1 = stateH &+ sum1 &+ choice &+ constants[index] &+ words[index]
            let sum0 = rotate(stateA, 2) ^ rotate(stateA, 13) ^ rotate(stateA, 22)
            let majority = (stateA & stateB) ^ (stateA & stateC) ^ (stateB & stateC)
            stateH = stateG
            stateG = stateF
            stateF = stateE
            stateE = stateD &+ temp1
            stateD = stateC
            stateC = stateB
            stateB = stateA
            stateA = temp1 &+ sum0 &+ majority
        }
        for (index, value) in [stateA, stateB, stateC, stateD, stateE, stateF, stateG, stateH].enumerated() {
            hash[index] &+= value
        }
    }

    private static func rotate(_ value: UInt32, _ count: UInt32) -> UInt32 {
        (value >> count) | (value << (32 - count))
    }
}
