//
//  TranslationApplicationServiceTests.swift
//  WhiskyBridgeTests
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
import WhiskyApplication
import WhiskyBridgeProtocol
import WhiskyTranslation

private actor MemoryCredentialStore: CredentialStore {
    private var values: [String: String] = [:]
    func availability() async -> CredentialStoreAvailability { .sessionOnly }
    func store(_ secret: String, for reference: String) async throws { values[reference] = secret }
    func secret(for reference: String) async throws -> String? { values[reference] }
    func remove(reference: String) async throws { values[reference] = nil }
}

@Test @MainActor func translationProfilesCredentialsAndSettingsRoundTrip() async throws {
    let root = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
    defer { try? FileManager.default.removeItem(at: root) }
    let credentials = MemoryCredentialStore()
    let translation = ApplicationTranslationService(directory: root, credentials: credentials)
    let profile = ProviderConfiguration(
        kind: .openAIChat,
        baseURL: URL(string: "http://127.0.0.1:8765/v1")!,
        modelID: "manual-model",
        openAIChatTokenLimitField: .maxTokens
    )
    let saved = try await translation.call("translation.providers/save", params: [
        "provider": .encoded(profile)
    ])
    #expect(saved.object?["modelID"] == .string("manual-model"))
    #expect(saved.object?["openAIChatTokenLimitField"] == .string("max_tokens"))
    #expect(saved.object?["hasCredential"] == .bool(false))

    let secret = "write-only-secret"
    let credential = try await translation.call("translation.credential.set", params: [
        "providerID": .string(profile.id.uuidString), "key": .string(secret)
    ])
    #expect(credential.object?["credentialReference"]?.string != nil)
    #expect(!String(describing: credential).contains(secret))
    let listed = try await translation.call("translation.providers/list", params: [:])
    #expect(listed.array?.first?.object?["hasCredential"] == .bool(true))

    let settings = TranslationSettings(contextWindowOverride: 8_192, customPrompt: "Keep names stable")
    _ = try await translation.call("translation.settings/save", params: [
        "settings": .encoded(settings), "remoteProvidersEnabled": .bool(false)
    ])
    let secondInstance = ApplicationTranslationService(directory: root, credentials: credentials)
    let persisted = try await secondInstance.call("translation.settings/get", params: [:])
    #expect(try persisted.object?["settings"]?.decoded(TranslationSettings.self) == settings)

    let bytes = try Data(contentsOf: root.appendingPathComponent("bridge-profiles.json"))
    #expect(!String(decoding: bytes, as: UTF8.self).contains(secret))
}
