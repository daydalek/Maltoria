//
//  Errors.swift
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

public enum TranslationError: Error, Equatable, Sendable, LocalizedError {
    case invalidConfiguration(String)
    case insecureEndpoint
    case missingModel
    case missingCredential
    case staleSource
    case modelDiscoveryUnsupported
    case invalidModel
    case unsupportedCapability(String)
    case httpStatus(Int)
    case quotaOrBillingLimit
    case providerRejected
    case transportFailure
    case malformedResponse
    case invalidUTF8
    case incompleteStream
    case emptyCompletion
    case responseTooLarge
    case streamConsumerTooSlow
    case sourceExceedsBudget(maximumBytes: Int)
    case persistenceUnavailable
    case credentialUnavailable
    case credentialOperationFailed(Int)

    public var errorDescription: String? {
        switch self {
        case let .invalidConfiguration(message): message
        case .insecureEndpoint: "Remote provider endpoints must use HTTPS; HTTP is allowed only for loopback servers"
        case .missingModel: "Select a model before translating"
        case .missingCredential: "A credential is required for this provider"
        case .staleSource: "An older source revision was rejected"
        case .modelDiscoveryUnsupported: "This provider does not expose model discovery"
        case .invalidModel: "The selected model is unavailable or invalid"
        case let .unsupportedCapability(name): "This provider does not support \(name)"
        case let .httpStatus(status): "The provider returned HTTP status \(status)"
        case .quotaOrBillingLimit: "The provider rejected the request because of a quota or billing limit"
        case .providerRejected: "The provider returned an error response"
        case .transportFailure: "The provider network request failed"
        case .malformedResponse: "The provider returned an invalid response"
        case .invalidUTF8: "The provider returned invalid UTF-8"
        case .incompleteStream: "The provider stream ended before completion"
        case .emptyCompletion: "The provider returned no translation text"
        case .responseTooLarge: "The provider response exceeded the size limit"
        case .streamConsumerTooSlow: "The provider stream exceeded the bounded receive buffer"
        case let .sourceExceedsBudget(maximumBytes): "Current text exceeds the conservative input budget (\(maximumBytes) UTF-8 bytes)"
        case .persistenceUnavailable: "Translation history could not be saved"
        case .credentialUnavailable: "Secure credential storage is unavailable"
        case let .credentialOperationFailed(status): "Secure credential operation failed (\(status))"
        }
    }
}

public enum CredentialStoreAvailability: String, Codable, Sendable {
    case persistentKeychain
    case localPlaintextFile
    case sessionOnly
}

public protocol CredentialStore: Sendable {
    func availability() async -> CredentialStoreAvailability
    func store(_ secret: String, for reference: String) async throws
    func secret(for reference: String) async throws -> String?
    func hasSecret(for reference: String) async throws -> Bool
    func remove(reference: String) async throws
}

public extension CredentialStore {
    func hasSecret(for reference: String) async throws -> Bool {
        try await secret(for: reference) != nil
    }
}
