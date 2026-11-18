//
//  GameMetadata.swift
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

/// These identifiers describe remote works, never executables or Wine settings.
public enum GameMetadataSource: String, Codable, CaseIterable, Sendable, Identifiable {
    case bangumi, vndb, steam, ymgal, hikarinagi, nextmoe

    public var id: String {
        rawValue
    }

    public var title: String {
        switch self {
        case .bangumi: "Bangumi"
        case .vndb: "VNDB"
        case .steam: "Steam"
        case .ymgal: "月幕"
        case .nextmoe: "NextMoe · 鲲 Galgame 论坛"
        case .hikarinagi: "Hikarinagi"
        }
    }
}

/// A character appearing in a game.
public struct GameCharacter: Codable, Hashable, Sendable, Identifiable {
    public var id: String
    public var name: String
    public var originalName: String?
    public var imageURL: URL?
    public var voiceActor: String?
    public var summary: String?
    public var role: String?
    public var pageURL: URL?

    public init(
        id: String = UUID().uuidString,
        name: String,
        originalName: String? = nil,
        role: String? = nil,
        imageURL: URL? = nil,
        pageURL: URL? = nil,
        voiceActor: String? = nil,
        summary: String? = nil
    ) {
        self.pageURL = pageURL
        self.id = id
        self.name = name
        self.originalName = originalName
        self.imageURL = imageURL
        self.voiceActor = voiceActor
        self.summary = summary
        self.role = role
    }
}

/// A complete source snapshot. We deliberately do not merge unconfirmed search
/// results: similarly named releases from different sites may be different works.
public struct GameMetadata: Codable, Hashable, Sendable, Identifiable {
    public let source: GameMetadataSource
    public let sourceID: String
    public var title: String
    public var originalTitle: String
    public var summary: String
    public var developer: String
    public var releaseDate: String
    public var coverURL: URL?
    public var thumbnailURL: URL?
    public var pageURL: URL?
    /// Optional fields keep snapshots written by older app versions readable.
    public var alternateTitles: [String]?
    public var titleLanguage: String?
    public var characters: [GameCharacter]?
    public var provenance: [String: MetadataProvenance]?
    public var fetchedAt: Date

    public var id: String {
        "\(source.rawValue):\(sourceID)"
    }

    public init(
        source: GameMetadataSource,
        sourceID: String,
        title: String,
        originalTitle: String = "",
        summary: String = "",
        developer: String = "",
        releaseDate: String = "",
        coverURL: URL? = nil,
        thumbnailURL: URL? = nil,
        pageURL: URL? = nil,
        fetchedAt: Date = Date(),
        alternateTitles: [String]? = nil,
        titleLanguage: String? = nil,
        characters: [GameCharacter]? = nil
    ) {
        self.source = source
        self.sourceID = sourceID
        self.title = title
        self.originalTitle = originalTitle
        self.summary = summary
        self.developer = developer
        self.releaseDate = releaseDate
        self.coverURL = coverURL
        self.thumbnailURL = thumbnailURL
        self.pageURL = pageURL
        self.fetchedAt = fetchedAt
        self.alternateTitles = alternateTitles
        self.titleLanguage = titleLanguage
        self.characters = characters
    }

    public var matchingTitles: [String] {
        ([title, originalTitle] + (alternateTitles ?? [])).filter { !$0.isEmpty }
    }

    /// Keep source order within each group while showing characters with artwork first.
    public var charactersInDisplayOrder: [GameCharacter] {
        let characters = characters ?? []
        return characters.filter { $0.imageURL != nil } + characters.filter { $0.imageURL == nil }
    }

    /// Keep a useful local Chinese display name when VNDB only supplies another
    /// language. All remote names remain available for matching and provenance.
    public func usingLibraryTitle(_ localName: String) -> GameMetadata {
        guard source == .vndb, let titleLanguage, !titleLanguage.hasPrefix("zh") else { return self }
        let local = FolderGameMatching.localDisplayTitle(localName)
        guard local.unicodeScalars.contains(where: { (0x3400 ... 0x9FFF).contains($0.value) }),
              !local.unicodeScalars.contains(where: {
                  (0x3040 ... 0x30FF).contains($0.value) && $0.value != 0x30FB
              })
        else { return self }
        var result = self
        result.alternateTitles = Array(Set(matchingTitles)).sorted()
        result.title = local
        return result
    }
}

public protocol GameMetadataProvider: Sendable {
    var source: GameMetadataSource { get }
    func search(_ query: String) async throws -> [GameMetadata]
}

public struct GameMetadataSearchResult: Sendable {
    public let source: GameMetadataSource
    public let games: [GameMetadata]
    public let error: String?

    public init(source: GameMetadataSource, games: [GameMetadata], error: String? = nil) {
        self.source = source
        self.games = games
        self.error = error
    }
}

public struct GameMetadataAutomaticMatchOutcome: Sendable {
    public let candidate: GameMetadata?
    public let results: [GameMetadataSearchResult]
    /// True when at least one provider was queried and every queried provider failed with an error.
    public var allProvidersFailed: Bool {
        !results.isEmpty && results.allSatisfy { $0.error != nil }
    }
}

public enum GameMetadataSearch {
    /// Caller chooses the providers. No background scan or implicit library upload.
    public static func search(_ query: String, providers: [any GameMetadataProvider]) async
        -> [GameMetadataSearchResult] {
        guard !query.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty else { return [] }
        return await withTaskGroup(of: GameMetadataSearchResult.self) { group in
            for provider in providers {
                group.addTask {
                    do {
                        let games = try await provider.search(query)
                        return GameMetadataSearchResult(source: provider.source, games: games, error: nil)
                    } catch {
                        return GameMetadataSearchResult(
                            source: provider.source, games: [], error: error.localizedDescription
                        )
                    }
                }
            }
            var results: [GameMetadataSearchResult] = []
            for await result in group {
                results.append(result)
            }
            return results.sorted { $0.source.rawValue < $1.source.rawValue }
        }
    }

    /// Queries the user's preferred provider first, then remaining fast providers
    /// concurrently, and falls back to NextMoe at most once only if fast sources miss.
    public static func automaticMatch(_ query: String, providers: [any GameMetadataProvider]) async
        -> GameMetadataAutomaticMatchOutcome {
        let trimmed = query.trimmingCharacters(in: .whitespacesAndNewlines)
        let queries = FolderGameMatching.searchQueries(trimmed)
        guard let primaryQuery = queries.first, let preferred = providers.first else {
            return GameMetadataAutomaticMatchOutcome(candidate: nil, results: [])
        }
        let sources = providers.map(\.source)
        var results: [GameMetadataSearchResult] = []

        if !Task.isCancelled {
            let preferredOutcome = await searchProvider(
                preferred,
                matchingName: trimmed,
                primaryQuery: primaryQuery,
                fallbackQuery: fallbackQuery(for: preferred.source, queries: queries)
            )
            results.append(preferredOutcome.result)
            if let decision = preferredOutcome.exactDecision {
                return GameMetadataAutomaticMatchOutcome(candidate: decision, results: results)
            }
        }

        let remaining = Array(providers.dropFirst())
        let fastProviders = remaining.enumerated().filter { $0.element.source != .nextmoe }
        let nextMoeProvider = remaining.first { $0.source == .nextmoe }

        if !fastProviders.isEmpty, !Task.isCancelled {
            let fastResults = await searchProvidersConcurrently(
                fastProviders,
                matchingName: trimmed,
                primaryQuery: primaryQuery,
                queries: queries
            )
            results.append(contentsOf: fastResults)
        }

        if hasAmbiguousExactMatch(query: trimmed, in: results) {
            return GameMetadataAutomaticMatchOutcome(candidate: nil, results: results)
        }
        if let candidate = FolderGameMatching.candidate(query: trimmed, results: results, sources: sources) {
            return GameMetadataAutomaticMatchOutcome(candidate: candidate, results: results)
        }

        if let nextMoeProvider, !Task.isCancelled {
            let nextMoeQuery = fallbackQuery(for: .nextmoe, queries: queries) ?? primaryQuery
            let nextMoeResult = await fetchSearchResult(provider: nextMoeProvider, query: nextMoeQuery)
            results.append(nextMoeResult)
        }

        let candidate = FolderGameMatching.candidate(query: trimmed, results: results, sources: sources)
        return GameMetadataAutomaticMatchOutcome(candidate: candidate, results: results)
    }

    private struct ProviderSearchOutcome: Sendable {
        let result: GameMetadataSearchResult
        /// `nil`: no short-circuit; `.some(nil)`: ambiguous exact match; `.some(game)`: unique exact match.
        let exactDecision: GameMetadata??
    }

    private static func searchProvider(
        _ provider: any GameMetadataProvider,
        matchingName: String,
        primaryQuery: String,
        fallbackQuery: String?
    ) async -> ProviderSearchOutcome {
        let firstResult = await fetchSearchResult(provider: provider, query: primaryQuery)
        guard firstResult.error == nil else {
            return ProviderSearchOutcome(result: firstResult, exactDecision: nil)
        }
        let firstExact = FolderGameMatching.exactMatches(query: matchingName, in: firstResult.games)
        if firstExact.count == 1 {
            return ProviderSearchOutcome(result: firstResult, exactDecision: .some(firstExact[0]))
        }
        if firstExact.count > 1 {
            return ProviderSearchOutcome(result: firstResult, exactDecision: .some(nil))
        }
        if let fallbackQuery, !Task.isCancelled {
            let fallbackResult = await fetchSearchResult(provider: provider, query: fallbackQuery)
            let mergedGames = fallbackResult.error == nil
                ? deduplicatedGames(firstResult.games + fallbackResult.games)
                : firstResult.games
            let mergedResult = GameMetadataSearchResult(source: provider.source, games: mergedGames, error: nil)
            let mergedExact = FolderGameMatching.exactMatches(query: matchingName, in: mergedGames)
            if mergedExact.count == 1 {
                return ProviderSearchOutcome(result: mergedResult, exactDecision: .some(mergedExact[0]))
            }
            if mergedExact.count > 1 {
                return ProviderSearchOutcome(result: mergedResult, exactDecision: .some(nil))
            }
            return ProviderSearchOutcome(result: mergedResult, exactDecision: nil)
        }
        return ProviderSearchOutcome(result: firstResult, exactDecision: nil)
    }

    private static func searchProvidersConcurrently(
        _ indexedProviders: [(offset: Int, element: any GameMetadataProvider)],
        matchingName: String,
        primaryQuery: String,
        queries: [String]
    ) async -> [GameMetadataSearchResult] {
        await withTaskGroup(of: (Int, GameMetadataSearchResult).self) { group in
            for (index, provider) in indexedProviders {
                group.addTask {
                    let outcome = await searchProvider(
                        provider,
                        matchingName: matchingName,
                        primaryQuery: primaryQuery,
                        fallbackQuery: fallbackQuery(for: provider.source, queries: queries)
                    )
                    return (index, outcome.result)
                }
            }
            var collected: [(Int, GameMetadataSearchResult)] = []
            for await item in group {
                collected.append(item)
            }
            return collected.sorted { $0.0 < $1.0 }.map(\.1)
        }
    }

    /// Select one decomposed query within the per-provider request budget.
    private static func fallbackQuery(for source: GameMetadataSource, queries: [String]) -> String? {
        let alternatives = Array(queries.dropFirst())
        guard alternatives.count > 1 else { return alternatives.first }

        switch source {
        case .vndb:
            return alternatives.first { FolderGameMatching.scriptFlags($0).hasKana }
                ?? alternatives.first { FolderGameMatching.scriptFlags($0).hasLatin }
                ?? alternatives.first
        case .steam:
            return alternatives.first { FolderGameMatching.scriptFlags($0).hasLatin } ?? alternatives.first
        case .bangumi, .ymgal, .hikarinagi, .nextmoe:
            return alternatives.first {
                let flags = FolderGameMatching.scriptFlags($0)
                return flags.hasHan && !flags.hasKana
            } ?? alternatives.first
        }
    }

    private static func hasAmbiguousExactMatch(
        query: String,
        in results: [GameMetadataSearchResult]
    ) -> Bool {
        results.contains {
            $0.error == nil && FolderGameMatching.exactMatches(query: query, in: $0.games).count > 1
        }
    }

    private static func fetchSearchResult(
        provider: any GameMetadataProvider,
        query: String
    ) async -> GameMetadataSearchResult {
        do {
            let games = try await provider.search(query)
            return GameMetadataSearchResult(source: provider.source, games: games, error: nil)
        } catch {
            return GameMetadataSearchResult(
                source: provider.source, games: [], error: error.localizedDescription
            )
        }
    }

    private static func deduplicatedGames(_ games: [GameMetadata]) -> [GameMetadata] {
        var seen = Set<String>()
        return games.filter { seen.insert($0.id).inserted }
    }
}
