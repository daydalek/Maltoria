//
//  AutomaticMatchScoringTests.swift
//  WhiskyCoreTests
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

import Testing
@testable import WhiskyCore

@Suite("Automatic metadata identity safety")
struct AutomaticMatchScoringTests {
    private func candidate(_ query: String, _ games: [GameMetadata]) -> GameMetadata? {
        let sources = GameMetadataSource.allCases.filter { source in games.contains { $0.source == source } }
        let results = sources.map { source in
            GameMetadataSearchResult(source: source, games: games.filter { $0.source == source })
        }
        return FolderGameMatching.candidate(query: query, results: results, sources: sources)
    }

    @Test func chaptersRemainDistinctFromSeriesAndOtherChapters() {
        let series = GameMetadata(source: .bangumi, sourceID: "1", title: "WHITE ALBUM 2")
        let intro = GameMetadata(
            source: .bangumi, sourceID: "2", title: "WHITE ALBUM 2 -introductory chapter-",
            alternateTitles: ["WHITE ALBUM 2", "白色相簿2 开场篇", "WHITE ALBUM 2 -closing chapter-"]
        )
        #expect(candidate("WHITE ALBUM 2 -closing chapter-", [series]) == nil)
        #expect(candidate("WHITE ALBUM 2 -ｃｌｏｓｉｎｇ ｃｈａｐｔｅｒ-", [series]) == nil)
        #expect(candidate("WHITE ALBUM 2", [intro]) == nil)
        #expect(candidate("WHITE ALBUM 2 -closing chapter-", [intro]) == nil)
        #expect(candidate("白色相簿2 开场篇", [intro]) == intro)
        #expect(candidate("WHITE ALBUM 2 -introductory chapter-", [intro]) == intro)

        let longChapter = GameMetadata(
            source: .vndb, sourceID: "v1", title: "A Very Long Wonderful Summer Story -part 1-"
        )
        #expect(candidate("A Very Long Wonderful Summer Story -part 2-", [longChapter]) == nil)
        let undelimited = GameMetadata(
            source: .vndb,
            sourceID: "v2",
            title: "A Long Title Chapter 1",
            alternateTitles: ["A Long Title"]
        )
        #expect(candidate("A Long Title", [undelimited]) == nil)
        let generic = GameMetadata(
            source: .vndb,
            sourceID: "v3",
            title: "A Long Title",
            alternateTitles: ["A Long Title Chapter 1"]
        )
        #expect(candidate("A Long Title Chapter 1", [generic]) == nil)
    }

    @Test func namedStudioRequiresCompatibleDeveloper() {
        let wrong = GameMetadata(
            source: .bangumi,
            sourceID: "1",
            title: "Imaginary Love Story",
            developer: "Other Studio"
        )
        let missing = GameMetadata(source: .bangumi, sourceID: "2", title: "Imaginary Love Story")
        let right = GameMetadata(source: .vndb, sourceID: "v1", title: "Imaginary Love Story", developer: "Key")
        #expect(candidate("[Key] Imaginary Love Story", [wrong]) == nil)
        #expect(candidate("[Key] Imaginary Love Story", [missing]) == nil)
        #expect(candidate("[Key] Imaginary Love Story", [wrong, missing, right]) == right)
        #expect(candidate("[2024.04.26 / Key] Imaginary Love Story", [wrong]) == nil)
        #expect(candidate("[2024.04.26 / Key] Imaginary Love Story", [right]) == right)
        for prefix in ["[2024.04.26 Key]", "[Ｋｅｙ]"] {
            #expect(candidate("\(prefix) Imaginary Love Story", [wrong]) == nil)
            #expect(candidate("\(prefix) Imaginary Love Story", [right]) == right)
        }
        let misleading = GameMetadata(
            source: .vndb,
            sourceID: "v3",
            title: "Imaginary Love Story",
            developer: "Keyhole Studio"
        )
        #expect(candidate("[Key] Imaginary Love Story", [misleading]) == nil)
        let yuzu = GameMetadata(source: .vndb, sourceID: "v2", title: "千恋万花", developer: "Yuzusoft")
        #expect(candidate("[ゆずソフト] 千恋万花", [yuzu]) == yuzu)
        #expect(candidate("[心愿屋] Imaginary Love Story 汉化版", [missing]) == missing)
    }

    @Test func ordinaryAfterTitlesRemainComparable() {
        let game = GameMetadata(source: .vndb, sourceID: "v1", title: "After the Rain - A Wonderful Tale")
        #expect(candidate("After the Rain", [game]) == game)
    }

    @Test func unknownTitlePrefixRequiresIdentityEvidence() {
        let base = GameMetadata(source: .vndb, sourceID: "v1", title: "A Very Long Wonderful Summer Story")
        #expect(candidate("[Another Story] A Very Long Wonderful Summer Story", [base]) == nil)
        let full = GameMetadata(
            source: .vndb,
            sourceID: "v2",
            title: "[Another Story] A Very Long Wonderful Summer Story"
        )
        #expect(candidate(full.title, [full]) == full)
    }

    @Test func fuzzyCandidatesCompeteAcrossSources() {
        let query = "星空彼方寻找失落的回忆"
        let first = GameMetadata(source: .bangumi, sourceID: "1", title: "星空彼方寻找失落的回音")
        let second = GameMetadata(source: .vndb, sourceID: "v2", title: "星空彼方寻找失落的回声")
        #expect(candidate(query, [first]) == first)
        #expect(candidate(query, [first, second]) == nil)
    }

    @Test func sameWorkNeedsCorroboratingTitleDeveloperAndRelease() {
        let query = "A Wonderfull Summer Story"
        let first = GameMetadata(
            source: .bangumi,
            sourceID: "1",
            title: "A Wonderful Summer Story",
            developer: "Example Studio",
            releaseDate: "2020-01-01"
        )
        let second = GameMetadata(
            source: .vndb,
            sourceID: "v1",
            title: "A Wonderful Summer Story",
            developer: "Example Studio",
            releaseDate: "2020-05-01"
        )
        #expect(candidate(query, [first, second]) == first)
        var differentYear = second
        differentYear.releaseDate = "2021-01-01"
        #expect(candidate(query, [first, differentYear]) == nil)
        var unknownDeveloper = second
        unknownDeveloper.developer = ""
        #expect(candidate(query, [first, unknownDeveloper]) == nil)
        let duplicateSource = GameMetadata(
            source: .bangumi,
            sourceID: "2",
            title: first.title,
            developer: first.developer
        )
        #expect(candidate(query, [first, duplicateSource]) == nil)
        var unknownYear = first
        unknownYear.releaseDate = ""
        let third = GameMetadata(
            source: .steam,
            sourceID: "3",
            title: first.title,
            developer: first.developer,
            releaseDate: "2021-01-01"
        )
        #expect(candidate(query, [unknownYear, second, third]) == nil)
    }
}
