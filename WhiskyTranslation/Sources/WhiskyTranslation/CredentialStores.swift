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

public struct KeychainCredentialStore: CredentialStore, Sendable {
    private let service: String

    public init(service: String = "com.daydalek.Maltoria.translation") {
        self.service = service
    }

    public func availability() async -> CredentialStoreAvailability { .persistentKeychain }

    public func store(_ secret: String, for reference: String) async throws {
        guard !reference.isEmpty, reference.utf8.count <= 256, !secret.isEmpty, secret.utf8.count <= 16_384 else {
            throw TranslationError.credentialUnavailable
        }
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
    }

    public func secret(for reference: String) async throws -> String? {
        var query = baseQuery(reference)
        query[kSecReturnData as String] = true
        query[kSecMatchLimit as String] = kSecMatchLimitOne
        var result: CFTypeRef?
        let status = SecItemCopyMatching(query as CFDictionary, &result)
        if status == errSecItemNotFound { return nil }
        guard status == errSecSuccess else { throw TranslationError.credentialOperationFailed(Int(status)) }
        guard let data = result as? Data, let value = String(data: data, encoding: .utf8) else {
            throw TranslationError.credentialUnavailable
        }
        return value
    }

    public func remove(reference: String) async throws {
        let status = SecItemDelete(baseQuery(reference) as CFDictionary)
        guard status == errSecSuccess || status == errSecItemNotFound else {
            throw TranslationError.credentialOperationFailed(Int(status))
        }
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

public actor SessionOnlyCredentialStore: CredentialStore {
    private var values: [String: String] = [:]

    public init() {}

    public func availability() async -> CredentialStoreAvailability { .sessionOnly }

    public func store(_ secret: String, for reference: String) async throws {
        guard !reference.isEmpty, reference.utf8.count <= 256, !secret.isEmpty, secret.utf8.count <= 16_384 else {
            throw TranslationError.credentialUnavailable
        }
        values[reference] = secret
    }

    public func secret(for reference: String) async throws -> String? { values[reference] }

    public func remove(reference: String) async throws { values[reference] = nil }
}
