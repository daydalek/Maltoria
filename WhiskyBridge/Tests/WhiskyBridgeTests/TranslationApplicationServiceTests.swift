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
@testable import WhiskyApplication
import WhiskyBridgeProtocol
import WhiskyTranslation

private actor MemoryCredentialStore: CredentialStore {
    private var values: [String: String] = [:]
    func availability() async -> CredentialStoreAvailability { .sessionOnly }
    func store(_ secret: String, for reference: String) async throws { values[reference] = secret }
    func secret(for reference: String) async throws -> String? { values[reference] }
    func remove(reference: String) async throws { values[reference] = nil }
}

private actor SpyKeychainCredentialStore: CredentialStore {
    private var values: [String: String] = [:]
    private(set) var storeCount = 0
    private(set) var secretCount = 0
    private(set) var hasSecretCount = 0
    private(set) var removeCount = 0

    func availability() async -> CredentialStoreAvailability { .persistentKeychain }

    func store(_ secret: String, for reference: String) async throws {
        storeCount += 1
        values[reference] = secret
    }

    func secret(for reference: String) async throws -> String? {
        secretCount += 1
        return values[reference]
    }

    func hasSecret(for reference: String) async throws -> Bool {
        hasSecretCount += 1
        return values[reference] != nil
    }

    func remove(reference: String) async throws {
        removeCount += 1
        values[reference] = nil
    }

    func resetCounts() {
        storeCount = 0
        secretCount = 0
        hasSecretCount = 0
        removeCount = 0
    }
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
        openAIChatTokenLimitField: .maxTokens,
        thinkingLevel: .xhigh
    )
    let saved = try await translation.call("translation.providers/save", params: [
        "provider": .encoded(profile)
    ])
    #expect(saved.object?["modelID"] == .string("manual-model"))
    #expect(saved.object?["openAIChatTokenLimitField"] == .string("max_tokens"))
    #expect(saved.object?["thinkingLevel"] == .string("xhigh"))
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

@Test @MainActor func switchingStorageModeRoutesCredentialsAndStopsKeychainReads() async throws {
    let root = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
    defer { try? FileManager.default.removeItem(at: root) }

    let spyKeychain = SpyKeychainCredentialStore()
    let service = ApplicationTranslationService(directory: root, keychainStore: spyKeychain)

    let profile = ProviderConfiguration(
        kind: .openAIChat,
        baseURL: URL(string: "http://127.0.0.1:8765/v1")!,
        modelID: "local-model"
    )
    _ = try await service.call("translation.providers/save", params: ["provider": .encoded(profile)])
    _ = try await service.call("translation.credential.set", params: [
        "providerID": .string(profile.id.uuidString),
        "key": .string("keychain-secret")
    ])

    let listedKeychain = try await service.call("translation.providers/list", params: [:])
    #expect(listedKeychain.array?.first?.object?["hasCredential"] == .bool(true))
    #expect(await spyKeychain.hasSecretCount == 1)
    #expect(await spyKeychain.secretCount == 0)

    let localSettings = TranslationSettings(credentialStorageMode: .localFile)
    let savedLocal = try await service.call("translation.settings/save", params: [
        "settings": .encoded(localSettings)
    ])
    #expect(savedLocal.object?["credentialStorage"] == .string("localPlaintextFile"))

    await spyKeychain.resetCounts()
    let listedLocalBeforeSet = try await service.call("translation.providers/list", params: [:])
    #expect(listedLocalBeforeSet.array?.first?.object?["hasCredential"] == .bool(false))
    #expect(await spyKeychain.hasSecretCount == 0)
    #expect(await spyKeychain.secretCount == 0)

    let setLocal = try await service.call("translation.credential.set", params: [
        "providerID": .string(profile.id.uuidString),
        "key": .string("file-secret")
    ])
    #expect(setLocal.object?["storage"] == .string("localPlaintextFile"))
    #expect(await spyKeychain.storeCount == 0)

    let credFile = root.appendingPathComponent("bridge-credentials.json")
    let attrs = try FileManager.default.attributesOfItem(atPath: credFile.path)
    #expect((attrs[.posixPermissions] as? Int) == 0o600)

    let reloaded = ApplicationTranslationService(directory: root, keychainStore: spyKeychain)
    let reloadedSettings = try await reloaded.call("translation.settings/get", params: [:])
    #expect(reloadedSettings.object?["credentialStorage"] == .string("localPlaintextFile"))
    let reloadedList = try await reloaded.call("translation.providers/list", params: [:])
    #expect(reloadedList.array?.first?.object?["hasCredential"] == .bool(true))
    #expect(await spyKeychain.hasSecretCount == 0)
    #expect(await spyKeychain.secretCount == 0)

    let sessionSettings = TranslationSettings(credentialStorageMode: .sessionOnly)
    let savedSession = try await reloaded.call("translation.settings/save", params: [
        "settings": .encoded(sessionSettings)
    ])
    #expect(savedSession.object?["credentialStorage"] == .string("sessionOnly"))
    let listedSession = try await reloaded.call("translation.providers/list", params: [:])
    #expect(listedSession.array?.first?.object?["hasCredential"] == .bool(false))
    #expect(await spyKeychain.hasSecretCount == 0)
}

@Test @MainActor func catalogLoadSkipsUnreadableProfilesAndRepairsStorage() async throws {
    let root = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
    defer { try? FileManager.default.removeItem(at: root) }
    try FileManager.default.createDirectory(at: root, withIntermediateDirectories: true)
    let fileURL = root.appendingPathComponent("bridge-profiles.json")
    try Data(#"""
    {"version":1,"remoteProvidersEnabled":true,
     "profiles":[
       {"id":"F42821E7-364C-4168-9FAD-BB75CA19389B","name":"Legacy ChatGPT","kind":"chatGPTPlan",
        "baseURL":"https:\/\/api.openai.com\/v1","defaultSourceLanguage":"auto","defaultTargetLanguage":"en",
        "modelID":"gpt-6-luna","openAIChatTokenLimitField":"max_completion_tokens","thinkingLevel":"default"},
       {"id":"729F95B1-D17E-473D-B6B8-497851528CDD","name":"Local","kind":"openAIChat",
        "baseURL":"http:\/\/127.0.0.1:8765\/v1","defaultSourceLanguage":"auto","defaultTargetLanguage":"en",
        "modelID":"local-model","openAIChatTokenLimitField":"max_completion_tokens","thinkingLevel":"default"},
       {"id":"11111111-1111-4111-8111-111111111111","name":"Insecure","kind":"openAIChat",
        "baseURL":"http:\/\/example.com\/v1","defaultSourceLanguage":"auto","defaultTargetLanguage":"en",
        "modelID":"bad","openAIChatTokenLimitField":"max_completion_tokens","thinkingLevel":"default"}
     ],
     "settings":{"sourceLanguage":"auto","targetLanguage":"en","outputTokenLimit":512,
        "historyRetentionTurns":20,"contextTurnLimit":8,"includeSourceHistory":true,
        "includeTranslatedHistory":true,"customPrompt":"","credentialStorageMode":"sessionOnly",
        "translationDebounceMs":150,"sentenceCacheEnabled":true}}
    """#.utf8).write(to: fileURL)

    let service = ApplicationTranslationService(directory: root, credentials: MemoryCredentialStore())
    let listed = try await service.call("translation.providers/list", params: [:])
    #expect(listed.array?.compactMap { $0.object?["name"]?.string } == ["Local"])
    let settings = try await service.call("translation.settings/get", params: [:])
    #expect(settings.object?["settings"] != nil)

    let repaired = try String(decoding: Data(contentsOf: fileURL), as: UTF8.self)
    #expect(!repaired.contains("chatGPTPlan"))
    #expect(!repaired.contains("Insecure"))

    let reloaded = ApplicationTranslationService(directory: root, credentials: MemoryCredentialStore())
    let relisted = try await reloaded.call("translation.providers/list", params: [:])
    #expect(relisted.array?.count == 1)
}

@Test @MainActor func sentenceCacheListAndDeleteRPCs() async throws {
    let root = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
    defer { try? FileManager.default.removeItem(at: root) }

    let service = ApplicationTranslationService(directory: root, credentials: MemoryCredentialStore())
    let cache = PersistentSentenceCache(fileURL: root.appendingPathComponent("bridge-translation-cache.json"))
    let first = try #require(await cache.store(
        gameID: "game-one",
        sourceText: "おはよう",
        translatedText: "Good morning",
        speaker: "Alice",
        sourceLanguage: "ja",
        targetLanguage: "en",
        providerName: "Local",
        providerKind: .openAIChat,
        modelID: "model-1",
        customPrompt: ""
    ))
    let second = try #require(await cache.store(
        gameID: "game-two",
        sourceText: "さようなら",
        translatedText: "Goodbye",
        speaker: nil,
        sourceLanguage: "ja",
        targetLanguage: "en",
        providerName: "Remote",
        providerKind: .deepL,
        modelID: "",
        customPrompt: ""
    ))

    let allListed = try await service.call("translation.cache/list", params: [:])
    #expect(allListed.object?["totalCount"] == .integer(2))
    #expect(allListed.object?["gameCount"] == .integer(2))
    #expect((allListed.object?["totalBytes"]?.integer ?? 0) > 0)
    #expect(allListed.object?["entries"]?.array?.count == 2)

    let filteredByGame = try await service.call("translation.cache/list", params: [
        "gameID": .string("game-one"),
        "query": .string("morning")
    ])
    #expect(filteredByGame.object?["totalCount"] == .integer(2))
    #expect(filteredByGame.object?["gameCount"] == .integer(1))
    #expect(filteredByGame.object?["entries"]?.array?.count == 1)
    #expect(filteredByGame.object?["entries"]?.array?.first?.object?["id"] == .string(first.id))

    let deleted = try await service.call("translation.cache/delete", params: [
        "id": .string(first.id)
    ])
    #expect(deleted.object?["removed"] == .bool(true))
    #expect(deleted.object?["totalCount"] == .integer(1))

    let deletedMissing = try await service.call("translation.cache/delete", params: [
        "id": .string(first.id)
    ])
    #expect(deletedMissing.object?["removed"] == .bool(false))
    #expect(deletedMissing.object?["totalCount"] == .integer(1))

    let remaining = try await service.call("translation.cache/list", params: [:])
    #expect(remaining.object?["entries"]?.array?.map { $0.object?["id"]?.string } == [second.id])
}
