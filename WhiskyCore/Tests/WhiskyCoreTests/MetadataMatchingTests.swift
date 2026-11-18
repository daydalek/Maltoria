//
//  MetadataMatchingTests.swift
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
import Testing
@testable import WhiskyCore

@Suite("Portable metadata matching")
struct MetadataMatchingTests {
    @Test func nameCleanupRetainsSubtitles() {
        #expect(FolderGameMatching.query("星之终途 汉化版 v1.2") == "星之终途")
        #expect(FolderGameMatching.query("Game (Another Story)") == "Game (Another Story)")
        #expect(FolderGameMatching.query("[测试汉化组] 夏日物语 官方中文版 v1.2") == "夏日物语")
        #expect(FolderGameMatching.query("[230526][新新汉化] 苍之彼方的四重奏 免安装绿色版 Ver1.02 [CHS]") == "苍之彼方的四重奏")
        #expect(FolderGameMatching.query("Game 2 Build 1024") == "Game 2")
        #expect(FolderGameMatching.query("[Another Story] Game") == "[Another Story] Game")
        #expect(FolderGameMatching.normalized("ＧＡＭＥ: One") == "gameone")
        #expect(FolderGameMatching.normalized("櫻之詩 桜の詩") == "樱之诗樱の诗")
    }

    @Test func canonicalEditionsAndShinjitaiMatchAcrossSources() {
        let sequel = GameMetadata(source: .vndb, sourceID: "v2", title: "WHITE ALBUM 2")
        let remake = GameMetadata(source: .bangumi, sourceID: "10", title: "Summer Story Remake")
        let results = [
            GameMetadataSearchResult(source: .vndb, games: [sequel], error: nil),
            GameMetadataSearchResult(source: .bangumi, games: [remake], error: nil)
        ]
        #expect(FolderGameMatching.candidate(
            query: "White Album II", results: results, sources: [.vndb, .bangumi]
        ) == sequel)
        #expect(FolderGameMatching.candidate(
            query: "Summer Story 重制版", results: results, sources: [.vndb, .bangumi]
        ) == remake)
    }

    @Test func onlyUniqueExactNamesAreAutomaticallyMatched() {
        let exact = GameMetadata(source: .bangumi, sourceID: "1", title: "Game")
        let other = GameMetadata(source: .bangumi, sourceID: "2", title: "Game 2")
        let result = GameMetadataSearchResult(source: .bangumi, games: [exact, other], error: nil)
        #expect(FolderGameMatching.candidate(query: "game", results: [result], sources: [.bangumi]) == exact)
        #expect(FolderGameMatching.candidate(query: "gam", results: [result], sources: [.bangumi]) == nil)
        let duplicate = GameMetadataSearchResult(source: .bangumi, games: [exact, exact], error: nil)
        #expect(FolderGameMatching.candidate(query: "Game", results: [duplicate], sources: [.bangumi]) == nil)
    }

    @Test func matchesEveryAliasAndTraditionalChinese() {
        let game = GameMetadata(
            source: .vndb,
            sourceID: "v1",
            title: "Romanized",
            originalTitle: "日本語",
            alternateTitles: ["夏日物語", "Summer Tale"]
        )
        let results = [GameMetadataSearchResult(source: .vndb, games: [game], error: nil)]
        for query in ["夏日物语", "Summer Tale", "日本語", "Romanized"] {
            #expect(FolderGameMatching.candidate(query: query, results: results, sources: [.vndb]) == game)
        }
    }

    @Test func longTypoMatchesWhileCloseAlternativesAndEditionsRequireConfirmation() {
        let title = "A Wonderful Summer Story"
        let game = GameMetadata(source: .vndb, sourceID: "v1", title: title)
        func candidate(_ query: String, games: [GameMetadata] = []) -> GameMetadata? {
            FolderGameMatching.candidate(query: query, results: [
                GameMetadataSearchResult(source: .vndb, games: games.isEmpty ? [game] : games, error: nil)
            ], sources: [.vndb])
        }
        #expect(candidate("A Wonderfull Summer Story") == game)
        for query in [title + " 2", title + " II", title + " FD", title + "+", title + " Complete"] {
            #expect(candidate(query) == nil, "Edition query: \(query)")
        }
        let close = GameMetadata(source: .vndb, sourceID: "v2", title: "A Wonderful Summer Stori")
        #expect(candidate("A Wonderfull Summer Story", games: [close, game]) == nil)
        #expect(FolderGameMatching.ranked(query: title, games: [close, game]).first == game)
    }

    @Test func exactLaterSourceWinsAndFailedSourcesAllowFallback() {
        let close = GameMetadata(source: .bangumi, sourceID: "1", title: "A Wonderfull Summer Story")
        let exact = GameMetadata(source: .vndb, sourceID: "v1", title: "A Wonderful Summer Story")
        for error in [nil, "HTTP 500"] {
            let results = [
                GameMetadataSearchResult(source: .bangumi, games: [close], error: error),
                GameMetadataSearchResult(source: .vndb, games: [exact], error: nil)
            ]
            #expect(FolderGameMatching.candidate(
                query: exact.title,
                results: results,
                sources: [.bangumi, .vndb]
            ) == exact)
        }
    }

    @Test func galgamePrefixAndSuffixCleanup() {
        #expect(FolderGameMatching.query("[ゆずソフト] 千恋＊万花") == "千恋＊万花")
        #expect(
            FolderGameMatching.query("[2023.05.26][心愿屋] 夏日口袋 官中去码版 Full Voice") == "夏日口袋"
        )
        #expect(
            FolderGameMatching.query("[RJ123456][C101] 天使☆騒々 RE-BOOT! [Kirikiri2]") == "天使☆騒々 RE-BOOT!"
        )
        #expect(FolderGameMatching.query("(18禁ゲーム) サクラノ詩 [All-Ages][v1.02]") == "サクラノ詩")
        #expect(FolderGameMatching.query("夏之雨 精翻汉化硬盘版 R18+") == "夏之雨")
        // Edition-bearing suffixes must be preserved so editions stay distinct.
        #expect(FolderGameMatching.query("夏日口袋 完全版") == "夏日口袋 完全版")
        #expect(FolderGameMatching.query("Summer Story 完整版") == "Summer Story 完整版")
        #expect(FolderGameMatching.query("Summer Story 重制版") == "Summer Story 重制版")
        #expect(FolderGameMatching.query("Summer Story Remake") == "Summer Story Remake")
        #expect(FolderGameMatching.query("Summer Story Remaster") == "Summer Story Remaster")
        #expect(FolderGameMatching.query("千恋＊万花 FD") == "千恋＊万花 FD")
        #expect(FolderGameMatching.query("千恋＊万花 Fandisc") == "千恋＊万花 Fandisc")
    }

    @Test func searchQueriesDecomposeBilingualAndSubtitleTitles() {
        #expect(
            FolderGameMatching.searchQueries("樱之诗（サクラノ詩）") == [
                "樱之诗（サクラノ詩）", "樱之诗", "サクラノ詩"
            ]
        )
        #expect(
            FolderGameMatching.searchQueries("夏日口袋 (Summer Pockets)") == [
                "夏日口袋 (Summer Pockets)", "夏日口袋", "Summer Pockets"
            ]
        )
        #expect(
            FolderGameMatching.searchQueries("Senren＊Banka / 千恋＊万花") == [
                "Senren＊Banka / 千恋＊万花", "Senren＊Banka", "千恋＊万花"
            ]
        )
        #expect(
            FolderGameMatching.searchQueries("[Unknown] Game Name") == [
                "[Unknown] Game Name", "Game Name"
            ]
        )
        #expect(
            FolderGameMatching.searchQueries("サクラノ詩 -櫻の森の上を舞う-") == [
                "サクラノ詩 -櫻の森の上を舞う-", "サクラノ詩"
            ]
        )
        #expect(FolderGameMatching.searchQueries("Game (Another Story)") == ["Game (Another Story)"])
        #expect(FolderGameMatching.searchQueries("Re:Zero") == ["Re:Zero"])
        #expect(FolderGameMatching.searchQueries("   ") == [])
    }

    @Test func shinjitaiIterationKanaParticlesRomanNumeralsAndSteamSplits() {
        #expect(FolderGameMatching.normalized("処対実辺郷嬢") == "处对实边乡娘")
        #expect(FolderGameMatching.normalized("天使☆騒々") == "天使骚骚")
        #expect(FolderGameMatching.normalized("ダ・カーポII") == "ダカーポ2")
        #expect(FolderGameMatching.normalized("D.C.Ⅱ ～ダ・カーポⅡ～") == "dc2ダカーポ2")
        #expect(FolderGameMatching.normalized("白色相簿Ⅲ") == "白色相簿3")

        let tenshi = GameMetadata(source: .vndb, sourceID: "v1", title: "天使骚骚")
        let yoru = GameMetadata(source: .bangumi, sourceID: "2", title: "魔女的夜宴")
        let sakura = GameMetadata(source: .vndb, sourceID: "v3", title: "さくらの詩")
        let dc2 = GameMetadata(
            source: .vndb, sourceID: "v4", title: "D.C.II",
            originalTitle: "D.C.II ～ダ・カーポII～", alternateTitles: ["ダ・カーポII"]
        )
        let senren = GameMetadata(source: .steam, sourceID: "1144400", title: "Senren＊Banka / 千恋＊万花")

        func match(_ query: String, in game: GameMetadata) -> GameMetadata? {
            FolderGameMatching.candidate(
                query: query,
                results: [GameMetadataSearchResult(source: game.source, games: [game], error: nil)],
                sources: [game.source]
            )
        }

        #expect(match("天使☆騒々", in: tenshi) == tenshi)
        #expect(match("魔女の夜宴", in: yoru) == yoru)
        #expect(match("サクラノ詩", in: sakura) == sakura)
        #expect(match("D.C.Ⅱ ～ダ・カーポⅡ～", in: dc2) == dc2)
        #expect(match("ダ・カーポⅡ", in: dc2) == dc2)
        #expect(match("D.C.II", in: dc2) == dc2)
        #expect(match("千恋＊万花", in: senren) == senren)
        #expect(match("Senren＊Banka", in: senren) == senren)

        // Repeated edition tokens in D.C.II ～ダ・カーポII～ deduplicate so edition penalty is not applied.
        let dc2FullOnly = GameMetadata(source: .vndb, sourceID: "v5", title: "D.C.II ～ダ・カーポII～")
        let dc1 = GameMetadata(source: .vndb, sourceID: "v6", title: "D.C. ～ダ・カーポ～")
        #expect(FolderGameMatching.ranked(query: "D.C.II", games: [dc1, dc2FullOnly]).first == dc2FullOnly)
    }

    @Test func mainSubtitleMatchingAndSeriesAmbiguityRejection() {
        let sakura = GameMetadata(source: .vndb, sourceID: "v1", title: "サクラノ詩 -櫻の森の上を舞う-")
        #expect(FolderGameMatching.candidate(
            query: "サクラノ詩",
            results: [GameMetadataSearchResult(source: .vndb, games: [sakura], error: nil)],
            sources: [.vndb]
        ) == sakura)

        let nineEp1 = GameMetadata(source: .vndb, sourceID: "v10", title: "9-nine-ここのつここのかここのいろ")
        let nineEp2 = GameMetadata(source: .vndb, sourceID: "v11", title: "9-nine-そらいろそらうたそらのおと")
        #expect(FolderGameMatching.candidate(
            query: "9-nine-",
            results: [GameMetadataSearchResult(source: .vndb, games: [nineEp1, nineEp2], error: nil)],
            sources: [.vndb]
        ) == nil)

        let wa2Intro = GameMetadata(source: .bangumi, sourceID: "20", title: "WHITE ALBUM 2 -introductory chapter-")
        #expect(FolderGameMatching.candidate(
            query: "WHITE ALBUM 2 -closing chapter-",
            results: [GameMetadataSearchResult(source: .bangumi, games: [wa2Intro], error: nil)],
            sources: [.bangumi]
        ) == nil)
    }

    @Test func cjkWeightedSimilarityToleratesSingleTypoAndRejectsShortOrTwoCharDiffs() {
        let aokana = GameMetadata(source: .bangumi, sourceID: "1", title: "苍之彼方的四重奏")
        #expect(FolderGameMatching.candidate(
            query: "苍之彼方四重奏",
            results: [GameMetadataSearchResult(source: .bangumi, games: [aokana], error: nil)],
            sources: [.bangumi]
        ) == aokana)

        // Two-character difference in a 8-char CJK title stays below 0.92 threshold.
        #expect(FolderGameMatching.candidate(
            query: "苍之彼重奏",
            results: [GameMetadataSearchResult(source: .bangumi, games: [aokana], error: nil)],
            sources: [.bangumi]
        ) == nil)

        // Short CJK titles (< 5 chars) never fuzzy-match.
        let yosuga = GameMetadata(source: .bangumi, sourceID: "2", title: "夜之空")
        #expect(FolderGameMatching.candidate(
            query: "缘之空",
            results: [GameMetadataSearchResult(source: .bangumi, games: [yosuga], error: nil)],
            sources: [.bangumi]
        ) == nil)
    }

    @Test func developerCorroborationAndBilingualPartialMatch() {
        let game = GameMetadata(
            source: .vndb, sourceID: "v100", title: "Summer Pockets", developer: "Key"
        )
        let exactByDev = FolderGameMatching.exactMatches(query: "[Key Visual] Summer Pockets", in: [game])
        #expect(exactByDev == [game])

        let bilingualCandidate = FolderGameMatching.candidate(
            query: "夏日口袋 (Summer Pockets)",
            results: [GameMetadataSearchResult(source: .vndb, games: [game], error: nil)],
            sources: [.vndb]
        )
        #expect(bilingualCandidate == game)

        let localClean = GameMetadata(
            source: .vndb, sourceID: "v101", title: "Sakura no Uta",
            originalTitle: "サクラノ詩", titleLanguage: "en"
        ).usingLibraryTitle("[UnknownGroup] 樱之诗（サクラノ詩） 汉化版")
        #expect(localClean.title == "樱之诗")
    }

    @Test func automaticMatchUsesSingleFallbackQueryPerProvider() async {
        let game = GameMetadata(
            source: .bangumi, sourceID: "123", title: "樱之诗", originalTitle: "サクラノ詩"
        )
        let provider = MockMetadataProvider(source: .bangumi, responses: [
            "樱之诗（サクラノ詩）": [],
            "樱之诗": [game]
        ])
        let second = MockMetadataProvider(source: .vndb, responses: [:])
        let outcome = await GameMetadataSearch.automaticMatch(
            "樱之诗（サクラノ詩）",
            providers: [provider, second]
        )
        #expect(outcome.candidate == game)
        #expect(await provider.queries == ["樱之诗（サクラノ詩）", "樱之诗"])
        #expect(await second.queries.isEmpty)

        // Single-query clean names do not trigger a second call even when empty.
        let single = MockMetadataProvider(source: .vndb, responses: [:])
        let emptyOutcome = await GameMetadataSearch.automaticMatch("CLANNAD", providers: [single])
        #expect(emptyOutcome.candidate == nil)
        #expect(await single.queries == ["CLANNAD"])
    }
}

private actor MockMetadataProvider: GameMetadataProvider {
    let source: GameMetadataSource
    private let responses: [String: [GameMetadata]]
    private(set) var queries: [String] = []

    init(source: GameMetadataSource, responses: [String: [GameMetadata]]) {
        self.source = source
        self.responses = responses
    }

    func search(_ query: String) async throws -> [GameMetadata] {
        queries.append(query)
        return responses[query] ?? []
    }
}
