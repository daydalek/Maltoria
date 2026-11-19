//
//  LibraryMetadataService+Sync.swift
//  WhiskyApplication
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
#if canImport(FoundationNetworking)
import FoundationNetworking
#endif
import WhiskyBridgeProtocol
import WhiskyCore
import WhiskyMetadata

extension LibraryMetadataService {
    /// Automatic matching for every folder entry that has no metadata yet.
    /// The manual "scan and sync game info" action passes `automatic: false`
    /// and keeps its "everything missing" meaning; `automatic: true` honours
    /// the folder library's own switch and only touches entries still pending
    /// their initial match, retiring the marker once providers answered
    /// (`LibraryView.loadMetadata`).
    /// The retained job owns the scan counters and pending-marker commit.
    func syncMetadata(
        params: [String: JSONValue],
        workspace: ApplicationWorkspace
    ) throws -> JSONValue {
        let container = try workspace.resolve(params)
        let baseRecord = try defaultRecord()
        let preferred = baseRecord.defaultSource
        let includeCharacters = try includeCharacters(params)
        let enabled = try enabledSources(params).filter { $0 != .hikarinagi }
        let sources = enabled.filter { $0 == preferred } + enabled.filter { $0 != preferred }
        let providers = sources.map { PublicGameMetadataProvider(
            source: $0,
            session: session,
            networkAccess: networkAccess
        ) }
        let automatic = params["automatic"]?.bool ?? false
        return try jobs.start(name: "metadata.sync") { [self] progress in
            guard !providers.isEmpty else {
                return .object(["matched": .integer(0), "failed": .integer(0), "scanned": .integer(0)])
            }
            let entries = try await workspace.entries(in: container)
            let visibility = try? LibraryVisibility.load(bottleURL: container)
            let library = automatic ? try? FolderGameLibrary.load(bottleURL: container) : nil
            let filtered = try await collectSyncCandidates(
                entries: entries,
                container: container,
                automatic: automatic,
                library: library
            )
            let batch = await runConcurrentSync(
                candidates: filtered.candidates,
                providers: providers,
                baseRecord: baseRecord,
                visibility: visibility,
                includeCharacters: includeCharacters,
                automatic: automatic,
                progress: progress
            )
            try Task.checkCancellation()
            if automatic {
                try Self.retire(filtered.completedKeys.union(batch.completedKeys), in: container)
            }
            return .object([
                "matched": .integer(Int64(batch.matched)),
                "failed": .integer(Int64(batch.failed)),
                "scanned": .integer(Int64(filtered.scanned))
            ])
        }
    }

    private struct SyncCandidate: Sendable {
        let entry: LibraryEntry
        let store: GameMetadataStore
        let folderKey: String?
    }

    private struct SyncFilterResult: Sendable {
        let candidates: [SyncCandidate]
        let completedKeys: Set<String>
        let scanned: Int
    }

    private struct SyncBatchOutcome: Sendable {
        let matched: Int
        let failed: Int
        let completedKeys: Set<String>
    }

    private struct MatchResult: Sendable {
        let entry: LibraryEntry
        let store: GameMetadataStore
        let folderKey: String?
        let matched: Int
        let failed: Int
        let record: JSONValue?
        let details: GameMetadata?
        let allProvidersFailed: Bool
    }

    private func collectSyncCandidates(
        entries: [LibraryEntry],
        container: URL,
        automatic: Bool,
        library: FolderGameLibrary?
    ) async throws -> SyncFilterResult {
        var candidates: [SyncCandidate] = []
        var completedKeys: Set<String> = []
        var scanned = 0
        for candidateEntry in entries {
            try Task.checkCancellation()
            guard candidateEntry.source == .folder else { continue }
            scanned += 1
            let candidateStore = GameMetadataStore(
                directory: candidateEntry.metadataDirectory,
                networkAccess: networkAccess
            )
            let folderKey: String? = if automatic,
                                        case let .program(folder) = candidateEntry.launch {
                FolderGameLibrary.key(for: folder, bottleURL: container)
            } else {
                nil
            }
            if automatic, !Self.isPending(folderKey, in: library) {
                continue
            }
            let hasMetadata = await (try? candidateStore.load(
                for: candidateEntry.metadataIdentity
            )) != nil
            if hasMetadata {
                if automatic, let folderKey {
                    completedKeys.insert(folderKey)
                }
                continue
            }
            guard await (try? candidateStore.allowsAutomaticMatch(
                for: candidateEntry.metadataIdentity
            )) == true
            else { continue }
            candidates.append(SyncCandidate(entry: candidateEntry, store: candidateStore, folderKey: folderKey))
        }
        return SyncFilterResult(candidates: candidates, completedKeys: completedKeys, scanned: scanned)
    }

    private func runConcurrentSync(
        candidates: [SyncCandidate],
        providers: [PublicGameMetadataProvider],
        baseRecord: GameMetadataRecord,
        visibility: LibraryVisibility?,
        includeCharacters: Bool,
        automatic: Bool,
        progress: @escaping BridgeJobStore.Progress
    ) async -> SyncBatchOutcome {
        var matched = 0
        var failed = 0
        var processed = 0
        var completedKeys: Set<String> = []
        let session = self.session
        let networkAccess = self.networkAccess
        await withTaskGroup(of: Void.self) { portraitGroup in
            await withTaskGroup(of: MatchResult.self) { matchGroup in
                var nextIndex = 0
                let maxConcurrent = min(4, candidates.count)
                while nextIndex < maxConcurrent {
                    let candidate = candidates[nextIndex]
                    nextIndex += 1
                    matchGroup.addTask {
                        await Self.matchCandidate(
                            candidate,
                            providers: providers,
                            baseRecord: baseRecord,
                            session: session,
                            networkAccess: networkAccess,
                            visibility: visibility,
                            includeCharacters: includeCharacters
                        )
                    }
                }
                for await outcome in matchGroup {
                    processed += 1
                    matched += outcome.matched
                    failed += outcome.failed
                    if automatic,
                       let folderKey = outcome.folderKey,
                       outcome.failed == 0,
                       !outcome.allProvidersFailed {
                        // Providers answered; the initial match is complete even
                        // without a candidate, so do not retry on the next pass.
                        completedKeys.insert(folderKey)
                    }
                    var payload: [String: JSONValue] = [
                        "stage": .string("syncing"),
                        "scanned": .integer(Int64(processed)),
                        "matched": .integer(Int64(matched)),
                        "failed": .integer(Int64(failed))
                    ]
                    if let freshRecord = outcome.record {
                        payload["entryID"] = .string(outcome.entry.id)
                        payload["entry"] = freshRecord
                    }
                    progress(.object(payload))
                    if let details = outcome.details {
                        let store = outcome.store
                        portraitGroup.addTask {
                            await self.cacheCharacterImages(details, store: store)
                        }
                    }
                    if !Task.isCancelled, nextIndex < candidates.count {
                        let candidate = candidates[nextIndex]
                        nextIndex += 1
                        matchGroup.addTask {
                            await Self.matchCandidate(
                                candidate,
                                providers: providers,
                                baseRecord: baseRecord,
                                session: session,
                                networkAccess: networkAccess,
                                visibility: visibility,
                                includeCharacters: includeCharacters
                            )
                        }
                    } else if Task.isCancelled {
                        matchGroup.cancelAll()
                    }
                }
            }
            if Task.isCancelled {
                portraitGroup.cancelAll()
            }
        }
        return SyncBatchOutcome(matched: matched, failed: failed, completedKeys: completedKeys)
    }

    private nonisolated static func matchCandidate(
        _ candidateItem: SyncCandidate,
        providers: [PublicGameMetadataProvider],
        baseRecord: GameMetadataRecord,
        session: URLSession,
        networkAccess: @escaping MetadataNetworkAccess,
        visibility: LibraryVisibility?,
        includeCharacters: Bool
    ) async -> MatchResult {
        let outcome = await GameMetadataSearch.automaticMatch(
            candidateItem.entry.name,
            providers: providers
        )
        guard let candidate = outcome.candidate else {
            return MatchResult(
                entry: candidateItem.entry,
                store: candidateItem.store,
                folderKey: candidateItem.folderKey,
                matched: 0,
                failed: 0,
                record: nil,
                details: nil,
                allProvidersFailed: outcome.allProvidersFailed
            )
        }
        do {
            let details = try await PublicGameMetadataProvider(
                source: candidate.source,
                session: session,
                networkAccess: networkAccess
            ).details(for: candidate, includeCharacters: includeCharacters)
            var metadataRecord = baseRecord
            metadataRecord.defaultSource = details.source
            metadataRecord.bindings[details.source.rawValue] = details.sourceID
            metadataRecord.snapshots[details.source.rawValue] = details
            let display = details.usingLibraryTitle(candidateItem.entry.name)
            if display.title != details.title {
                metadataRecord.manualTitle = display.title
            }
            try await candidateItem.store.saveAutomaticRecord(
                metadataRecord,
                for: candidateItem.entry.metadataIdentity
            )
            if let cover = details.coverURL {
                try? await candidateItem.store.cacheCover(from: cover, session: session)
            }
            let record = visibility.flatMap { try? EntryRecord(candidateItem.entry, visibility: $0) }
            return MatchResult(
                entry: candidateItem.entry,
                store: candidateItem.store,
                folderKey: candidateItem.folderKey,
                matched: 1,
                failed: 0,
                record: record.flatMap { try? .encoded($0) },
                details: details,
                allProvidersFailed: outcome.allProvidersFailed
            )
        } catch {
            return MatchResult(
                entry: candidateItem.entry,
                store: candidateItem.store,
                folderKey: candidateItem.folderKey,
                matched: 0,
                failed: 1,
                record: nil,
                details: nil,
                allProvidersFailed: outcome.allProvidersFailed
            )
        }
    }

    private static func isPending(_ key: String?, in library: FolderGameLibrary?) -> Bool {
        library?.automaticMetadata == true && key.flatMap { library?.pendingMetadata.contains($0) } == true
    }

    private static func retire(_ completedKeys: Set<String>, in container: URL) throws {
        guard !completedKeys.isEmpty else { return }
        let url = container.appendingPathComponent("Folder Games.json")
        try HostFileLock().withExclusiveLock(at: url.appendingPathExtension("lock")) {
            guard FileManager.default.fileExists(atPath: url.path) else { return }
            guard var current = try JSONSerialization.jsonObject(with: ApplicationWorkspace.read(url))
                as? [String: Any], let pending = current["pendingMetadata"] as? [String]
            else { throw BridgeFailure("invalid_settings", "Folder metadata queue is unreadable") }
            current["pendingMetadata"] = pending.filter { !completedKeys.contains($0) }
            try JSONSerialization.data(withJSONObject: current, options: [.sortedKeys])
                .write(to: url, options: .atomic)
        }
    }
}
