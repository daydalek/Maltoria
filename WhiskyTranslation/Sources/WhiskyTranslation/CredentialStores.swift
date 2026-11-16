//
//  CredentialStores.swift
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

#if canImport(Security)
import Security

public actor KeychainCredentialStore: CredentialStore {
    private let service: String
    private var cache: [String: String] = [:]
    private var knownPresence: [String: Bool] = [:]

    public init(service: String = "com.daydalek.Maltoria.translation") {
        self.service = service
    }

    public func availability() async -> CredentialStoreAvailability { .persistentKeychain }

    public func store(_ secret: String, for reference: String) async throws {
        try validateCredentialInput(reference: reference, secret: secret)
        let query = baseQuery(reference)
        let attributes: [String: Any] = [kSecValueData as String: Data(secret.utf8)]
        let updateStatus = SecItemUpdate(query as CFDictionary, attributes as CFDictionary)
        if updateStatus == errSecItemNotFound {
            var insert = query
            insert[kSecValueData as String] = Data(secret.utf8)
            let status = SecItemAdd(insert as CFDictionary, nil)
            guard status == errSecSuccess else { throw TranslationError.credentialOperationFailed(Int(status)) }
        } else if updateStatus != errSecSuccess {
            throw TranslationError.credentialOperationFailed(Int(updateStatus))
        }
        cache[reference] = secret
        knownPresence[reference] = true
    }

    public func hasSecret(for reference: String) async throws -> Bool {
        if cache[reference] != nil { return true }
        if let known = knownPresence[reference] { return known }
        var query = baseQuery(reference)
        query[kSecReturnAttributes as String] = true
        query[kSecMatchLimit as String] = kSecMatchLimitOne
        var result: CFTypeRef?
        let status = SecItemCopyMatching(query as CFDictionary, &result)
        if status == errSecItemNotFound {
            knownPresence[reference] = false
            return false
        }
        guard status == errSecSuccess else { throw TranslationError.credentialOperationFailed(Int(status)) }
        knownPresence[reference] = true
        return true
    }

    public func secret(for reference: String) async throws -> String? {
        if let cached = cache[reference] { return cached }
        if knownPresence[reference] == false { return nil }
        var query = baseQuery(reference)
        query[kSecReturnData as String] = true
        query[kSecMatchLimit as String] = kSecMatchLimitOne
        var result: CFTypeRef?
        let status = SecItemCopyMatching(query as CFDictionary, &result)
        if status == errSecItemNotFound {
            cache.removeValue(forKey: reference)
            knownPresence[reference] = false
            return nil
        }
        guard status == errSecSuccess else { throw TranslationError.credentialOperationFailed(Int(status)) }
        guard let data = result as? Data, let value = String(data: data, encoding: .utf8) else {
            throw TranslationError.credentialUnavailable
        }
        cache[reference] = value
        knownPresence[reference] = true
        return value
    }

    public func remove(reference: String) async throws {
        let status = SecItemDelete(baseQuery(reference) as CFDictionary)
        guard status == errSecSuccess || status == errSecItemNotFound else {
            throw TranslationError.credentialOperationFailed(Int(status))
        }
        cache.removeValue(forKey: reference)
        knownPresence[reference] = false
    }

    private func baseQuery(_ reference: String) -> [String: Any] {
        [
            kSecClass as String: kSecClassGenericPassword,
            kSecAttrService as String: service,
            kSecAttrAccount as String: reference
        ]
    }
}
#endif

public actor LocalFileCredentialStore: CredentialStore {
    private let fileURL: URL
    private let fileManager = FileManager.default
    private var cached: [String: String]?

    public init(fileURL: URL) {
        self.fileURL = fileURL
    }

    public func availability() async -> CredentialStoreAvailability { .localPlaintextFile }

    public func store(_ secret: String, for reference: String) async throws {
        try validateCredentialInput(reference: reference, secret: secret)
        var values = try loadIfNeeded()
        values[reference] = secret
        try persist(values)
        cached = values
    }

    public func hasSecret(for reference: String) async throws -> Bool {
        let values = try loadIfNeeded()
        return values[reference] != nil
    }

    public func secret(for reference: String) async throws -> String? {
        let values = try loadIfNeeded()
        return values[reference]
    }

    public func remove(reference: String) async throws {
        var values = try loadIfNeeded()
        guard values.removeValue(forKey: reference) != nil else { return }
        try persist(values)
        cached = values
    }

    private func loadIfNeeded() throws -> [String: String] {
        if let cached { return cached }
        guard fileManager.fileExists(atPath: fileURL.path) else {
            cached = [:]
            return [:]
        }
        do {
            let data = try Data(contentsOf: fileURL)
            let decoded = try JSONDecoder().decode([String: String].self, from: data)
            cached = decoded
            return decoded
        } catch {
            throw TranslationError.credentialUnavailable
        }
    }

    private func persist(_ values: [String: String]) throws {
        do {
            let directory = fileURL.deletingLastPathComponent()
            try fileManager.createDirectory(
                at: directory,
                withIntermediateDirectories: true,
                attributes: [.posixPermissions: 0o700]
            )
            let encoder = JSONEncoder()
            encoder.outputFormatting = [.sortedKeys]
            let data = try encoder.encode(values)
            try data.write(to: fileURL, options: .atomic)
            try fileManager.setAttributes([.posixPermissions: 0o600], ofItemAtPath: fileURL.path)
        } catch {
            throw TranslationError.credentialUnavailable
        }
    }
}

public actor SessionOnlyCredentialStore: CredentialStore {
    private var values: [String: String] = [:]

    public init() {}

    public func availability() async -> CredentialStoreAvailability { .sessionOnly }

    public func store(_ secret: String, for reference: String) async throws {
        try validateCredentialInput(reference: reference, secret: secret)
        values[reference] = secret
    }

    public func secret(for reference: String) async throws -> String? { values[reference] }

    public func remove(reference: String) async throws { values[reference] = nil }
}

private func validateCredentialInput(reference: String, secret: String) throws {
    guard !reference.isEmpty, reference.utf8.count <= 256, !secret.isEmpty, secret.utf8.count <= 16_384 else {
        throw TranslationError.credentialUnavailable
    }
}
