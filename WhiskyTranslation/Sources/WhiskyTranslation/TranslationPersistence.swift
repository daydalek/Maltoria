//
//  TranslationPersistence.swift
//
//  This file is part of Maltoria, licensed under GPL-3.0-or-later.
//

import Foundation

public protocol TranslationRepository: Sendable {
    func saveConfiguration(_ configuration: ProviderConfiguration) async throws
    func loadConfiguration(id: UUID) async throws -> ProviderConfiguration?
    func saveSettings(_ settings: TranslationSettings) async throws
    func loadSettings() async throws -> TranslationSettings?
    func saveHistory(_ snapshot: TranslationSessionSnapshot) async throws
    func loadHistory(gameID: String, sessionID: UUID) async throws -> TranslationSessionSnapshot?
    func clearHistory(gameID: String, sessionID: UUID) async throws
}

public actor FileTranslationRepository: TranslationRepository {
    public let directory: URL
    private let fileManager: FileManager

    public init(directory: URL = FileTranslationRepository.defaultDirectory(), fileManager: FileManager = .default) {
        self.directory = directory
        self.fileManager = fileManager
    }

    public static func defaultDirectory(
        environment: [String: String] = ProcessInfo.processInfo.environment,
        home: URL = FileManager.default.homeDirectoryForCurrentUser
    ) -> URL {
        #if os(macOS)
        home.appending(path: "Library/Application Support/Maltoria/Translation", directoryHint: .isDirectory)
        #else
        let stateRoot = environment["XDG_STATE_HOME"].map(URL.init(fileURLWithPath:))
            ?? home.appending(path: ".local/state", directoryHint: .isDirectory)
        stateRoot.appending(path: "maltoria/translation", directoryHint: .isDirectory)
        #endif
    }

    public func saveConfiguration(_ configuration: ProviderConfiguration) async throws {
        do {
            try configuration.validate()
            try write(JSONEncoder.sorted.encode(configuration), to: "provider-\(configuration.id.uuidString).json")
        } catch let error as TranslationError { throw error }
        catch { throw TranslationError.persistenceUnavailable }
    }

    public func loadConfiguration(id: UUID) async throws -> ProviderConfiguration? {
        guard fileManager.fileExists(atPath: url("provider-\(id.uuidString).json").path) else { return nil }
        do {
            let value = try JSONDecoder().decode(ProviderConfiguration.self, from: Data(contentsOf: url("provider-\(id.uuidString).json")))
            try value.validate()
            return value
        } catch let error as TranslationError { throw error }
        catch { throw TranslationError.persistenceUnavailable }
    }

    public func saveSettings(_ settings: TranslationSettings) async throws {
        do {
            try settings.validate()
            try write(JSONEncoder.sorted.encode(settings), to: "settings.json")
        } catch let error as TranslationError { throw error }
        catch { throw TranslationError.persistenceUnavailable }
    }

    public func loadSettings() async throws -> TranslationSettings? {
        guard fileManager.fileExists(atPath: url("settings.json").path) else { return nil }
        do {
            let value = try JSONDecoder().decode(TranslationSettings.self, from: Data(contentsOf: url("settings.json")))
            try value.validate()
            return value
        } catch let error as TranslationError { throw error }
        catch { throw TranslationError.persistenceUnavailable }
    }

    public func saveHistory(_ snapshot: TranslationSessionSnapshot) async throws {
        do { try write(JSONEncoder.sorted.encode(snapshot), to: historyFilename(gameID: snapshot.gameID, sessionID: snapshot.sessionID)) }
        catch { throw TranslationError.persistenceUnavailable }
    }

    public func loadHistory(gameID: String, sessionID: UUID) async throws -> TranslationSessionSnapshot? {
        let location = url(historyFilename(gameID: gameID, sessionID: sessionID))
        guard fileManager.fileExists(atPath: location.path) else { return nil }
        do {
            let snapshot = try JSONDecoder().decode(TranslationSessionSnapshot.self, from: Data(contentsOf: location))
            guard snapshot.gameID == gameID, snapshot.sessionID == sessionID else { return nil }
            return snapshot
        } catch { throw TranslationError.persistenceUnavailable }
    }

    public func clearHistory(gameID: String, sessionID: UUID) async throws {
        let location = url(historyFilename(gameID: gameID, sessionID: sessionID))
        guard fileManager.fileExists(atPath: location.path) else { return }
        do { try fileManager.removeItem(at: location) }
        catch { throw TranslationError.persistenceUnavailable }
    }

    private func write(_ data: Data, to filename: String) throws {
        try fileManager.createDirectory(at: directory, withIntermediateDirectories: true)
        try fileManager.setAttributes([.posixPermissions: 0o700], ofItemAtPath: directory.path)
        let destination = url(filename)
        try data.write(to: destination, options: .atomic)
        try fileManager.setAttributes([.posixPermissions: 0o600], ofItemAtPath: destination.path)
    }

    private func url(_ filename: String) -> URL { directory.appending(path: filename, directoryHint: .notDirectory) }
    private func historyFilename(gameID: String, sessionID: UUID) -> String {
        var hash: UInt64 = 14_695_981_039_346_656_037
        for byte in gameID.utf8 { hash = (hash ^ UInt64(byte)) &* 1_099_511_628_211 }
        return String(format: "history-%016llx-%@.json", hash, sessionID.uuidString)
    }
}
