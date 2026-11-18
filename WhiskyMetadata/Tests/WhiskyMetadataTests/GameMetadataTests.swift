// swiftlint:disable file_length
//
//  GameMetadataTests.swift
//  WhiskyMetadataTests
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
import WhiskyCore
@testable import WhiskyMetadata
#if canImport(FoundationNetworking)
import FoundationNetworking
#endif

@Suite("Game Metadata")
struct GameMetadataTests {
    @Test func characterDisplayOrderPrioritizesArtworkAndPreservesSourceOrder() throws {
        let image = try #require(URL(string: "https://example.com/portrait.png"))
        let characters = [
            GameCharacter(id: "1", name: "No image 1"),
            GameCharacter(id: "2", name: "Image 1", imageURL: image),
            GameCharacter(id: "3", name: "No image 2"),
            GameCharacter(id: "4", name: "Image 2", imageURL: image)
        ]
        let game = GameMetadata(source: .bangumi, sourceID: "1", title: "Test", characters: characters)
        let restored = try JSONDecoder().decode(GameMetadata.self, from: JSONEncoder().encode(game))

        #expect(restored.charactersInDisplayOrder.map(\.id) == ["2", "4", "1", "3"])
        #expect(restored.characters == characters)
        #expect(GameMetadata(source: .bangumi, sourceID: "2", title: "Empty").charactersInDisplayOrder.isEmpty)
    }

    @Test func characterRefreshRetriesTimeoutAndReturnsCharacters() async throws {
        let configuration = URLSessionConfiguration.ephemeral
        configuration.protocolClasses = [CharacterTimeoutProtocol.self]
        let session = URLSession(configuration: configuration)
        defer { session.invalidateAndCancel() }
        let provider = PublicGameMetadataProvider(source: .bangumi, session: session, networkAccess: {})
        let characters = try await provider.fetchCharacters(
            for: GameMetadata(source: .bangumi, sourceID: "retry", title: "Test")
        )
        #expect(characters.map(\.name) == ["Character"])
    }

    @Test func characterRefreshSurfacesPersistentTimeout() async throws {
        let configuration = URLSessionConfiguration.ephemeral
        configuration.protocolClasses = [CharacterTimeoutProtocol.self]
        let session = URLSession(configuration: configuration)
        defer { session.invalidateAndCancel() }
        let provider = PublicGameMetadataProvider(source: .bangumi, session: session, networkAccess: {})
        do {
            _ = try await provider.fetchCharacters(
                for: GameMetadata(source: .bangumi, sourceID: "persistent", title: "Test")
            )
            Issue.record("Expected timeout")
        } catch let error as URLError {
            #expect(error.code == .timedOut)
        }
    }

    @Test func roundTripAndRemoval() async throws {
        let directory = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        defer { try? FileManager.default.removeItem(at: directory) }
        let store = GameMetadataStore(directory: directory)
        let game = GameMetadata(source: .vndb, sourceID: "v17", title: "作品", summary: "简介")
        #expect(try await store.load(for: "A") == nil)
        try await store.save(game, for: "A")
        #expect(try await store.load(for: "A") == GameMetadataRecord(legacy: game).resolved().metadata)
        #expect(try await store.load(for: "B") == nil)
        try await store.remove(for: "A")
        #expect(try await store.load(for: "A") == nil)
        try await store.remove(for: "A")
    }

    @Test func identitySurvivesBottleMoveAndSeparatesExecutables() {
        func item(root: String, path: String) -> LibraryEntry {
            let bottle = URL(fileURLWithPath: root)
            let executable = bottle.appendingPathComponent(path)
            return LibraryEntry(
                id: executable.path,
                name: "Launch",
                iconURL: executable,
                bottleURL: bottle,
                source: .pinned,
                launch: .program(executable)
            )
        }
        let first = item(root: "/old/bottle", path: "drive_c/A/Launch.exe")
        let moved = item(root: "/new/bottle", path: "drive_c/A/Launch.exe")
        let other = item(root: "/old/bottle", path: "drive_c/B/Launch.exe")
        #expect(first.metadataIdentity == moved.metadataIdentity)
        #expect(first.metadataIdentity != other.metadataIdentity)
    }

    @Test func failureDoesNotDiscardSuccessfulSource() async {
        let results = await GameMetadataSearch.search("test", providers: [
            FixtureProvider(source: .bangumi, fails: false),
            FixtureProvider(source: .vndb, fails: true)
        ])
        #expect(results.count == 2)
        #expect(results.first { $0.source == .bangumi }?.games.count == 1)
        #expect(results.first { $0.source == .vndb }?.error != nil)
    }

    @Test func emptyQueryDoesNotCallProviders() async {
        let result = await GameMetadataSearch.search(" \n", providers: [
            FixtureProvider(source: .bangumi, fails: false)
        ])
        #expect(result.isEmpty)
    }

    @Test func unconfiguredSourcesReportAnError() async {
        let results = await GameMetadataSearch.search("test", providers: [
            PublicGameMetadataProvider(source: .hikarinagi)
        ])
        #expect(results.count == 1)
        #expect(results.allSatisfy { $0.error != nil && $0.games.isEmpty })
    }

    @Test func decodesPublicSourceResponses() async throws {
        let configuration = URLSessionConfiguration.ephemeral
        configuration.protocolClasses = [MetadataFixtureProtocol.self]
        let session = URLSession(configuration: configuration)
        defer { session.invalidateAndCancel() }
        for source in [GameMetadataSource.bangumi, .vndb, .steam, .ymgal] {
            let provider = PublicGameMetadataProvider(source: source, session: session)
            let games = try await provider.search("作品 & title")
            #expect(games.count == 1)
            #expect(games.first?.source == source)
            #expect(games.first?.coverURL?.scheme == "https")
        }
    }

    @Test func ymgalLoadsSelectedDetailsAndPreservesLargeID() async throws {
        let configuration = URLSessionConfiguration.ephemeral
        configuration.protocolClasses = [MetadataFixtureProtocol.self]
        let session = URLSession(configuration: configuration)
        defer { session.invalidateAndCancel() }
        let provider = PublicGameMetadataProvider(source: .ymgal, session: session)
        let candidate = try #require(try await provider.search("作品 & title").first)
        #expect(candidate.sourceID == "9007199254740993")
        let detail = try await provider.details(for: candidate)
        #expect(detail.sourceID == candidate.sourceID)
        #expect(detail.summary == "完整简介")
        #expect(detail.developer == "Studio")
        #expect(detail.pageURL?.absoluteString == "https://www.ymgal.games/ga9007199254740993")
    }

    @Test func ymgalReportsBusinessErrorsAndBoundsAuthenticationRetry() async {
        let configuration = URLSessionConfiguration.ephemeral
        configuration.protocolClasses = [MetadataFixtureProtocol.self]
        let session = URLSession(configuration: configuration)
        defer { session.invalidateAndCancel() }
        for query in ["rate-limit", "expired"] {
            let result = await GameMetadataSearch.search(query, providers: [
                PublicGameMetadataProvider(source: .ymgal, session: session)
            ])
            #expect(result.first?.games.isEmpty == true)
            #expect(result.first?.error?.contains(query == "expired" ? "401" : "429") == true)
        }
    }

    @Test(arguments: ["zh-Hans", "zh-Hant", "ja"])
    func vndbPreservesNamesAndChoosesDisplayLanguage(_ language: String) async throws {
        let configuration = URLSessionConfiguration.ephemeral
        configuration.protocolClasses = [VNDBNamesProtocol.self]
        let session = URLSession(configuration: configuration)
        defer { session.invalidateAndCancel() }
        let game = try #require(try await PublicGameMetadataProvider(source: .vndb, session: session)
            .search(language).first)
        #expect(game.originalTitle == "夏の物語")
        #expect(game.titleLanguage == language)
        #expect(game.title == (language == "zh-Hans" ? "夏日物语" : language == "zh-Hant" ? "夏日物語" : "夏の物語"))
        for title in ["Natsu no Monogatari", "夏の物語", "夏日故事", "Summer Story"] {
            #expect(game.matchingTitles.contains(title))
        }
        let displayed = game.usingLibraryTitle("夏日故事 汉化版 v1.2")
        #expect(displayed.title == (language == "ja" ? "夏日故事" : game.title))
        #expect(displayed.originalTitle == "夏の物語")
        #expect(displayed.sourceID == game.sourceID)
        #expect(displayed.matchingTitles.contains(game.title))
        #expect(game.usingLibraryTitle("夏の物語").title == game.title)
    }

    @Test func olderMetadataSnapshotsRemainReadable() throws {
        let game = GameMetadata(source: .vndb, sourceID: "v1", title: "Older")
        var json = try #require(try JSONSerialization.jsonObject(with: JSONEncoder().encode(game)) as? [String: Any])
        json.removeValue(forKey: "alternateTitles")
        json.removeValue(forKey: "titleLanguage")
        let restored = try JSONDecoder().decode(GameMetadata.self, from: JSONSerialization.data(withJSONObject: json))
        #expect(restored.matchingTitles == ["Older"])
        #expect(restored.usingLibraryTitle("本地名称").title == "Older")
    }

    @Test func invalidJSONIsNotAnEmptyResult() async {
        let configuration = URLSessionConfiguration.ephemeral
        configuration.protocolClasses = [MetadataInvalidProtocol.self]
        let session = URLSession(configuration: configuration)
        defer { session.invalidateAndCancel() }
        let results = await GameMetadataSearch.search("game", providers: [
            PublicGameMetadataProvider(source: .bangumi, session: session)
        ])
        #expect(results.first?.error != nil)
    }

    @Test func automaticMatchShortCircuitsAfterExactHitAndReportsAllProviderFailure() async {
        let first = CountingProvider(source: .bangumi, games: [
            GameMetadata(source: .bangumi, sourceID: "1", title: "CLANNAD")
        ])
        let second = CountingProvider(source: .vndb, games: [
            GameMetadata(source: .vndb, sourceID: "v4", title: "CLANNAD")
        ])
        let outcome = await GameMetadataSearch.automaticMatch("CLANNAD", providers: [first, second])
        #expect(outcome.candidate?.source == .bangumi)
        #expect(!outcome.allProvidersFailed)
        #expect(await first.callCount == 1)
        #expect(await second.callCount == 0)

        let failing1 = CountingProvider(source: .bangumi, fails: true)
        let failing2 = CountingProvider(source: .vndb, fails: true)
        let failedOutcome = await GameMetadataSearch.automaticMatch("CLANNAD", providers: [failing1, failing2])
        #expect(failedOutcome.candidate == nil)
        #expect(failedOutcome.allProvidersFailed)
        #expect(await failing1.callCount == 1)
        #expect(await failing2.callCount == 1)
    }

    @Test func providerCachesRepeatedCaseFoldedQueriesWithinSession() async throws {
        let configuration = URLSessionConfiguration.ephemeral
        configuration.protocolClasses = [CountingNetworkProtocol.self]
        let session = URLSession(configuration: configuration)
        defer { session.invalidateAndCancel() }
        CountingNetworkProtocol.reset()
        let provider = PublicGameMetadataProvider(source: .bangumi, session: session)
        let first = try await provider.search("Clannad")
        let second = try await provider.search("  ＣＬＡＮＮＡＤ ")
        #expect(first == second)
        #expect(first.first?.thumbnailURL?.absoluteString == "https://example.org/thumb.jpg")
        #expect(CountingNetworkProtocol.requestCount == 1)
        #expect(PublicGameMetadataProvider.parseRetryAfter("45") == 45)
        #expect(PublicGameMetadataProvider.parseRetryAfter("9999") == 9_999)
        #expect(PublicGameMetadataProvider.parseRetryAfter("inf") == nil)
        #expect(PublicGameMetadataProvider.parseRetryAfter("nan") == nil)
        let formatter = DateFormatter()
        formatter.locale = Locale(identifier: "en_US_POSIX")
        formatter.timeZone = TimeZone(secondsFromGMT: 0)
        formatter.dateFormat = "EEE',' dd MMM yyyy HH':'mm':'ss z"
        let longHTTPDate = formatter.string(from: Date().addingTimeInterval(3_600))
        #expect((PublicGameMetadataProvider.parseRetryAfter(longHTTPDate) ?? 0) > 3_500)
    }

    @Test func localChineseTitleAllowsKatakanaMiddleDot() {
        let game = GameMetadata(
            source: .vndb, sourceID: "v2002", title: "Steins;Gate 0",
            originalTitle: "STEINS;GATE 0", titleLanguage: "en"
        )
        let displayed = game.usingLibraryTitle("命运石之门・零 官方中文版 v1.0")
        #expect(displayed.title == "命运石之门・零")
    }

    @Test func gatekeeperPacesRequestsAcrossFailedSearchAndDetailsAttempts() async throws {
        let gatekeeper = SourceGatekeeper(minInterval: 0.08)
        let clock = ContinuousClock()
        let searchStart = clock.now
        await #expect(throws: URLError.self) {
            _ = try await gatekeeper.search("first") { throw URLError(.badServerResponse) }
        }
        let found = try await gatekeeper.search("second") {
            [GameMetadata(source: .bangumi, sourceID: "1", title: "Recovered")]
        }
        #expect(found.first?.title == "Recovered")
        #expect(searchStart.duration(to: clock.now) >= .milliseconds(70))

        let detailStart = clock.now
        await #expect(throws: URLError.self) {
            _ = try await gatekeeper.details("10") { throw URLError(.badServerResponse) }
        }
        let detail = try await gatekeeper.details("11") {
            GameMetadata(source: .steam, sourceID: "11", title: "Recovered Detail")
        }
        #expect(detail.title == "Recovered Detail")
        #expect(detailStart.duration(to: clock.now) >= .milliseconds(140))
    }

    @Test func steamExtractsAlternateTitlesFromBilingualSeparators() async throws {
        let configuration = URLSessionConfiguration.ephemeral
        configuration.protocolClasses = [BilingualMetadataProtocol.self]
        let session = URLSession(configuration: configuration)
        defer { session.invalidateAndCancel() }
        let provider = PublicGameMetadataProvider(source: .steam, session: session)
        let games = try await provider.search("千恋＊万花")
        #expect(games.count == 4)
        let senren = try #require(games.first { $0.sourceID == "1144400" })
        #expect(senren.alternateTitles == ["Senren＊Banka", "千恋＊万花"])
        let nekopara = try #require(games.first { $0.sourceID == "1144401" })
        #expect(nekopara.alternateTitles == ["NEKOPARA Vol. 1", "ネコぱら Vol.1"])
        #expect(games.first { $0.sourceID == "999" }?.alternateTitles == nil)
        #expect(games.first { $0.sourceID == "998" }?.alternateTitles == nil)

        let result = GameMetadataSearchResult(source: .steam, games: games)
        #expect(FolderGameMatching.candidate(
            query: "千恋＊万花", results: [result], sources: [.steam]
        ) == senren)
        #expect(FolderGameMatching.candidate(
            query: "Senren＊Banka", results: [result], sources: [.steam]
        ) == senren)
    }

    @Test func ymgalDecodesExtensionNameStringsObjectsAndUnknownValues() async throws {
        let configuration = URLSessionConfiguration.ephemeral
        configuration.protocolClasses = [BilingualMetadataProtocol.self]
        let session = URLSession(configuration: configuration)
        defer { session.invalidateAndCancel() }
        let provider = PublicGameMetadataProvider(source: .ymgal, session: session)

        let stringGames = try await provider.search("string-aliases")
        let senren = try #require(stringGames.first)
        #expect(senren.alternateTitles == ["Senren*Banka", "千恋万花"])
        let stringResult = GameMetadataSearchResult(source: .ymgal, games: stringGames)
        #expect(FolderGameMatching.candidate(
            query: "Senren*Banka", results: [stringResult], sources: [.ymgal]
        ) == senren)

        let objectGames = try await provider.search("object-aliases")
        let sakura = try #require(objectGames.first { $0.sourceID == "202" })
        #expect(sakura.alternateTitles == ["Cherry Blossom Poem", "Sakura no Uta", "櫻之詩"])
        #expect(objectGames.first { $0.sourceID == "203" }?.alternateTitles == nil)
        let objectResult = GameMetadataSearchResult(source: .ymgal, games: objectGames)
        for alias in ["Sakura no Uta", "Cherry Blossom Poem", "樱之诗", "サクラノ詩"] {
            #expect(FolderGameMatching.candidate(
                query: alias, results: [objectResult], sources: [.ymgal]
            ) == sakura)
        }
        let detail = try await provider.details(for: sakura)
        #expect(detail.alternateTitles == ["Sakura no Uta -Detail-"])
    }

    @Test func usingLibraryTitleCleansBrandAndBilingualSuffixes() {
        let game = GameMetadata(
            source: .vndb,
            sourceID: "v101",
            title: "Sakura no Uta",
            originalTitle: "サクラノ詩",
            titleLanguage: "en"
        )
        let cleaned = game.usingLibraryTitle("[Makura] 樱之诗（サクラノ詩） 汉化版 v1.0")
        #expect(cleaned.title == "樱之诗")
        #expect(cleaned.originalTitle == "サクラノ詩")
        #expect(cleaned.alternateTitles?.contains("Sakura no Uta") == true)

        let parenthetical = game.usingLibraryTitle("[Key] 夏日口袋 (Summer Pockets) 官方中文版")
        #expect(parenthetical.title == "夏日口袋")
    }

    @Test func automaticMatchQueriesFastSourcesConcurrentlyAndFallsBackToSingleNextMoeQuery() async {
        let barrier = FastSourceBarrier(targetCount: 2)
        let preferred = BarrierQueryProvider(source: .bangumi, games: [])
        let vndb = BarrierQueryProvider(source: .vndb, games: [], barrier: barrier)
        let steam = BarrierQueryProvider(
            source: .steam,
            games: [GameMetadata(source: .steam, sourceID: "777", title: "CLANNAD")],
            barrier: barrier
        )
        let nextmoe = BarrierQueryProvider(
            source: .nextmoe,
            games: [GameMetadata(source: .nextmoe, sourceID: "9", title: "CLANNAD")]
        )
        let hitOutcome = await GameMetadataSearch.automaticMatch(
            "CLANNAD",
            providers: [preferred, vndb, steam, nextmoe]
        )
        #expect(hitOutcome.candidate?.source == .steam)
        #expect(await barrier.peakConcurrent == 2)
        #expect(await nextmoe.queries.isEmpty)

        // When all fast sources miss on a bilingual name, NextMoe runs once with the cleaner query.
        let bangumiMiss = BarrierQueryProvider(source: .bangumi, games: [])
        let vndbMiss = BarrierQueryProvider(source: .vndb, games: [])
        let nextMoeFallback = BarrierQueryProvider(
            source: .nextmoe,
            games: [GameMetadata(source: .nextmoe, sourceID: "12", title: "樱之诗")]
        )
        let fallbackOutcome = await GameMetadataSearch.automaticMatch(
            "樱之诗（サクラノ詩）",
            providers: [bangumiMiss, vndbMiss, nextMoeFallback]
        )
        #expect(fallbackOutcome.candidate?.source == .nextmoe)
        #expect(await nextMoeFallback.queries == ["樱之诗"])
    }

    @Test func sourceGatekeeperTokenBucketBurstAndAdaptiveAIMDRecovery() async throws {
        let gatekeeper = SourceGatekeeper(minInterval: 0.08, burstCapacity: 3)
        let clock = ContinuousClock()
        let burstStart = clock.now
        for idx in 0 ..< 3 {
            _ = try await gatekeeper.search("burst-\(idx)") { [] }
        }
        #expect(burstStart.duration(to: clock.now) < .milliseconds(50))

        let fourthStart = clock.now
        _ = try await gatekeeper.search("fourth") { [] }
        #expect(fourthStart.duration(to: clock.now) >= .milliseconds(50))

        do {
            _ = try await gatekeeper.search("rate-limit-hit") {
                throw ProviderError.rateLimited(429, retryAfter: nil)
            }
            Issue.record("Expected HTTP 429")
        } catch let ProviderError.http(code) {
            #expect(code == 429)
        }
        #expect(await gatekeeper.consecutiveRateLimits == 1)
        #expect(await gatekeeper.adaptiveInterval == 0.5)
        let cooldown = try #require(await gatekeeper.cooldownDate)
        let remaining = cooldown.timeIntervalSinceNow
        #expect(remaining > 1.0 && remaining <= 2.2)

        await #expect(throws: ProviderError.self) {
            _ = try await gatekeeper.search("during-cooldown") { [] }
        }

        // Verify progressive cooldown escalation (2s -> 5s -> 12s) and AIMD decay on 2xx success.
        var bucket = AdaptiveTokenBucket(minInterval: 0.10, burstCapacity: 3)
        bucket.recordRateLimit(429, retryAfter: nil)
        #expect(bucket.consecutiveRateLimits == 1)
        #expect(bucket.adaptiveInterval == 0.5)
        #expect((bucket.cooldownUntil?.date.timeIntervalSinceNow ?? 0) <= 2.2)

        bucket.recordRateLimit(429, retryAfter: nil)
        #expect(bucket.consecutiveRateLimits == 2)
        #expect(bucket.adaptiveInterval == 1.0)
        #expect((bucket.cooldownUntil?.date.timeIntervalSinceNow ?? 0) > 4.0)

        bucket.recordRateLimit(429, retryAfter: nil)
        #expect(bucket.consecutiveRateLimits == 3)
        #expect(bucket.adaptiveInterval == 2.0)
        #expect((bucket.cooldownUntil?.date.timeIntervalSinceNow ?? 0) > 10.5)

        bucket.recordSuccess()
        #expect(bucket.consecutiveRateLimits == 0)
        #expect(bucket.adaptiveInterval == 1.6)
        for _ in 0 ..< 16 {
            bucket.recordSuccess()
        }
        #expect(bucket.adaptiveInterval == 0.10)
    }
}

private actor FastSourceBarrier {
    private let targetCount: Int
    private var active = 0
    private(set) var peakConcurrent = 0
    private var waiters: [CheckedContinuation<Void, Never>] = []

    init(targetCount: Int) {
        self.targetCount = targetCount
    }

    func enterAndWait() async {
        active += 1
        peakConcurrent = max(peakConcurrent, active)
        if active >= targetCount {
            let pending = waiters
            waiters.removeAll()
            pending.forEach { $0.resume() }
        } else {
            await withCheckedContinuation { waiters.append($0) }
        }
    }
}

private actor BarrierQueryProvider: GameMetadataProvider {
    nonisolated let source: GameMetadataSource
    private let games: [GameMetadata]
    private let barrier: FastSourceBarrier?
    private(set) var queries: [String] = []

    init(source: GameMetadataSource, games: [GameMetadata], barrier: FastSourceBarrier? = nil) {
        self.source = source
        self.games = games
        self.barrier = barrier
    }

    func search(_ query: String) async throws -> [GameMetadata] {
        queries.append(query)
        if let barrier {
            await barrier.enterAndWait()
        }
        return games
    }
}

private actor CountingProvider: GameMetadataProvider {
    nonisolated let source: GameMetadataSource
    let games: [GameMetadata]
    let fails: Bool
    private(set) var callCount = 0

    init(source: GameMetadataSource, games: [GameMetadata] = [], fails: Bool = false) {
        self.source = source
        self.games = games
        self.fails = fails
    }

    func search(_ query: String) async throws -> [GameMetadata] {
        callCount += 1
        if fails {
            throw URLError(.notConnectedToInternet)
        }
        return games
    }
}

private class CountingNetworkProtocol: URLProtocol {
    private static let lock = NSLock()
    private nonisolated(unsafe) static var count = 0
    static var requestCount: Int {
        lock.withLock { count }
    }

    static func reset() {
        lock.withLock { count = 0 }
    }

    override class func canInit(with request: URLRequest) -> Bool {
        true
    }

    override class func canonicalRequest(for request: URLRequest) -> URLRequest {
        request
    }

    override func startLoading() {
        Self.lock.withLock { Self.count += 1 }
        let body = """
        {"data":[{"id":1,"name":"CLANNAD","name_cn":"CLANNAD","summary":"简介",\
        "images":{"large":"https://example.org/cover.jpg","common":"https://example.org/thumb.jpg"}}]}
        """
        guard let url = request.url,
              let response = HTTPURLResponse(
                  url: url,
                  statusCode: 200,
                  httpVersion: nil,
                  headerFields: ["Content-Type": "application/json"]
              )
        else { return }
        client?.urlProtocol(self, didReceive: response, cacheStoragePolicy: .notAllowed)
        client?.urlProtocol(self, didLoad: Data(body.utf8))
        client?.urlProtocolDidFinishLoading(self)
    }

    override func stopLoading() {}
}

private class MetadataFixtureProtocol: URLProtocol {
    override class func canInit(with request: URLRequest) -> Bool {
        true
    }

    override class func canonicalRequest(for request: URLRequest) -> URLRequest {
        request
    }

    // swiftlint:disable:next function_body_length
    override func startLoading() {
        let body: String
        switch request.url?.host {
        case "api.bgm.tv":
            body = """
            {"data":[{"id":1,"name":"Original","name_cn":"作品","summary":"简介",\
            "images":{"large":"https://example.org/cover.jpg"}}]}
            """
        case "api.vndb.org":
            body = """
            {"results":[{"id":"v1","title":"Original","titles":[{"lang":"zh-Hans","title":"作品"}],\
            "developers":[{"name":"Studio","original":null}],"image":{"url":"https://example.org/cover.jpg"}}]}
            """
        case "www.ymgal.games":
            if request.url?.path == "/oauth/token" {
                body = #"{"access_token":"fixture-token"}"#
            } else if request.url?.path == "/open/archive" {
                body = """
                {"success":true,"code":0,"data":{"game":{"gid":9007199254740993,"name":"Original",\
                "chineseName":"作品","introduction":"完整简介","mainImg":"https://example.org/cover.jpg"}}}
                """
            } else {
                guard request.value(forHTTPHeaderField: "Authorization") == "Bearer fixture-token",
                      request.value(forHTTPHeaderField: "version") == "1"
                else {
                    client?.urlProtocol(self, didFailWithError: URLError(.userAuthenticationRequired))
                    return
                }
                let query = request.url.flatMap { URLComponents(url: $0, resolvingAgainstBaseURL: false) }?
                    .queryItems?.first { $0.name == "keyword" }?.value
                if query == "rate-limit" || query == "expired" {
                    body = query == "expired"
                        ? #"{"success":false,"code":401,"msg":"expired"}"#
                        : #"{"success":false,"code":429,"msg":"rate limited"}"#
                } else {
                    body = """
                    {"success":true,"code":0,"data":{"result":[{"id":"9007199254740993","name":"Original",\
                    "chineseName":"作品","orgName":"Studio","mainImg":"https://example.org/cover.jpg"}]}}
                    """
                }
            }
        default:
            body = #"{"items":[{"id":1,"name":"作品"}]}"#
        }
        guard let url = request.url,
              let response = HTTPURLResponse(
                  url: url,
                  statusCode: 200,
                  httpVersion: nil,
                  headerFields: ["Content-Type": "application/json"]
              )
        else { return }
        client?.urlProtocol(self, didReceive: response, cacheStoragePolicy: .notAllowed)
        client?.urlProtocol(self, didLoad: Data(body.utf8))
        client?.urlProtocolDidFinishLoading(self)
    }

    override func stopLoading() {}
}

private class MetadataInvalidProtocol: URLProtocol {
    override class func canInit(with request: URLRequest) -> Bool {
        true
    }

    override class func canonicalRequest(for request: URLRequest) -> URLRequest {
        request
    }

    override func startLoading() {
        guard let url = request.url,
              let response = HTTPURLResponse(
                  url: url,
                  statusCode: 200,
                  httpVersion: nil,
                  headerFields: nil
              )
        else { return }
        client?.urlProtocol(self, didReceive: response, cacheStoragePolicy: .notAllowed)
        client?.urlProtocol(self, didLoad: Data("not JSON".utf8))
        client?.urlProtocolDidFinishLoading(self)
    }

    override func stopLoading() {}
}

private struct FixtureProvider: GameMetadataProvider {
    let source: GameMetadataSource
    let fails: Bool
    func search(_ query: String) async throws -> [GameMetadata] {
        if fails {
            throw URLError(.timedOut)
        }
        return [GameMetadata(source: source, sourceID: "1", title: query)]
    }
}

private class VNDBNamesProtocol: URLProtocol {
    override class func canInit(with request: URLRequest) -> Bool {
        true
    }

    override class func canonicalRequest(for request: URLRequest) -> URLRequest {
        request
    }

    override func startLoading() {
        do {
            let body: Data
            if let data = request.httpBody {
                body = data
            } else if let stream = request.httpBodyStream {
                stream.open()
                defer { stream.close() }
                var data = Data()
                var bytes = [UInt8](repeating: 0, count: 2_048)
                while stream.hasBytesAvailable {
                    let count = stream.read(&bytes, maxLength: bytes.count)
                    guard count > 0 else { break }
                    data.append(contentsOf: bytes.prefix(count))
                }
                body = data
            } else {
                throw URLError(.badServerResponse)
            }
            guard let payload = try JSONSerialization.jsonObject(with: body) as? [String: Any],
                  let fields = payload["fields"] as? String,
                  ["alttitle", "aliases", "titles.latin", "titles.main"].allSatisfy(fields.contains),
                  let filters = payload["filters"] as? [String],
                  filters.count > 2
            else {
                throw URLError(.badServerResponse)
            }
            let language = filters[2]
            var titles: [[String: Any]] = [
                ["lang": "ja", "title": "夏の物語", "latin": "Natsu no Monogatari", "main": true],
                ["lang": "en", "title": "Summer Story", "latin": NSNull(), "main": false]
            ]
            if language != "ja" {
                titles.append(["lang": "zh-Hant", "title": "夏日物語", "main": false])
            }
            if language == "zh-Hans" {
                titles.append(["lang": "zh-Hans", "title": "夏日物语", "main": false])
            }
            let result: [String: Any] = ["results": [[
                "id": "v123", "title": "Natsu no Monogatari", "alttitle": "夏の物語",
                "aliases": ["夏日故事"], "titles": titles
            ]]]
            guard let url = request.url,
                  let response = HTTPURLResponse(url: url, statusCode: 200, httpVersion: nil, headerFields: nil)
            else { return }
            client?.urlProtocol(self, didReceive: response, cacheStoragePolicy: .notAllowed)
            try client?.urlProtocol(self, didLoad: JSONSerialization.data(withJSONObject: result))
            client?.urlProtocolDidFinishLoading(self)
        } catch { client?.urlProtocol(self, didFailWithError: error) }
    }

    override func stopLoading() {}
}

extension PublicGameMetadataProvider {
    init(source: GameMetadataSource, session: URLSession = .shared) {
        self.init(source: source, session: session, networkAccess: {})
    }
}

extension GameMetadataStore {
    init(directory: URL) {
        self.init(directory: directory, networkAccess: {})
    }
}

private class CharacterTimeoutProtocol: URLProtocol {
    private static let lock = NSLock()
    private nonisolated(unsafe) static var attempts: [String: Int] = [:]

    override class func canInit(with request: URLRequest) -> Bool {
        true
    }

    override class func canonicalRequest(for request: URLRequest) -> URLRequest {
        request
    }

    override func startLoading() {
        guard let url = request.url else { return }
        let attempt = Self.lock.withLock {
            Self.attempts[url.path, default: 0] += 1
            return Self.attempts[url.path, default: 0]
        }
        guard request.timeoutInterval == 45 else {
            client?.urlProtocol(self, didFailWithError: URLError(.badURL))
            return
        }
        if url.path.contains("persistent") || attempt == 1 {
            client?.urlProtocol(self, didFailWithError: URLError(.timedOut))
            return
        }
        guard let response = HTTPURLResponse(url: url, statusCode: 200, httpVersion: nil, headerFields: nil)
        else {
            client?.urlProtocol(self, didFailWithError: URLError(.badServerResponse))
            return
        }
        client?.urlProtocol(self, didReceive: response, cacheStoragePolicy: .notAllowed)
        client?.urlProtocol(self, didLoad: Data(#"[{"id":1,"name":"Character","relation":"主角"}]"#.utf8))
        client?.urlProtocolDidFinishLoading(self)
    }

    override func stopLoading() {}
}

private final class BilingualMetadataProtocol: URLProtocol {
    override static func canInit(with request: URLRequest) -> Bool {
        true
    }

    override static func canonicalRequest(for request: URLRequest) -> URLRequest {
        request
    }

    override func startLoading() {
        guard let url = request.url,
              let response = HTTPURLResponse(
                  url: url, statusCode: 200, httpVersion: nil,
                  headerFields: ["Content-Type": "application/json"]
              )
        else { return }
        let body = responseBody(for: url)
        client?.urlProtocol(self, didReceive: response, cacheStoragePolicy: .notAllowed)
        client?.urlProtocol(self, didLoad: Data(body.utf8))
        client?.urlProtocolDidFinishLoading(self)
    }

    override func stopLoading() {}

    private func responseBody(for url: URL) -> String {
        if url.host == "www.ymgal.games" {
            return ymgalBody(for: url)
        }
        return #"""
        {"items":[
            {"id":1144400,"name":"Senren＊Banka / 千恋＊万花"},
            {"id":1144401,"name":"NEKOPARA Vol. 1 | ネコぱら Vol.1"},
            {"id":999,"name":"Fate/stay night"},
            {"id":998,"name":"Clannad"}
        ]}
        """#
    }

    private func ymgalBody(for url: URL) -> String {
        if url.path == "/oauth/token" {
            return #"{"access_token":"fixture-token"}"#
        }
        if url.path == "/open/archive" {
            return #"""
            {"success":true,"code":0,"data":{"game":{
                "gid":202,"name":"サクラノ詩","chineseName":"樱之诗",
                "extensionName":[{"title":"Sakura no Uta -Detail-"},{"other":true}]
            }}}
            """#
        }
        let keyword = URLComponents(url: url, resolvingAgainstBaseURL: false)?
            .queryItems?.first { $0.name == "keyword" }?.value
        if keyword == "string-aliases" {
            return #"""
            {"success":true,"code":0,"data":{"result":[{
                "id":201,"name":"千恋＊万花","chineseName":"千恋＊万花",
                "extensionName":["Senren*Banka","  千恋万花  ","Senren*Banka"," ","千恋＊万花"]
            }]}}
            """#
        }
        return #"""
        {"success":true,"code":0,"data":{"result":[
            {
                "id":"202","name":"サクラノ詩","chineseName":"樱之诗",
                "extensionName":[
                    {"name":"Sakura no Uta"},
                    {"extensionName":"櫻之詩"},
                    {"title":"Cherry Blossom Poem"},
                    {"unknown":123},
                    42,
                    null
                ]
            },
            {"id":"203","name":"Null Aliases","extensionName":null}
        ]}}
        """#
    }
}
