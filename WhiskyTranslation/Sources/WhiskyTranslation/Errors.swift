//
//  Errors.swift
//
//  This file is part of Maltoria, licensed under GPL-3.0-or-later.
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
    case sessionOnly
}

public protocol CredentialStore: Sendable {
    func availability() async -> CredentialStoreAvailability
    func store(_ secret: String, for reference: String) async throws
    func secret(for reference: String) async throws -> String?
    func remove(reference: String) async throws
}
