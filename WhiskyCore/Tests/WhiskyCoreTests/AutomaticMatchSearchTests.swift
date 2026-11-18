//
//  AutomaticMatchSearchTests.swift
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

@Suite("Automatic metadata search queries")
struct AutomaticMatchSearchTests {
    @Test func vndbSearchesEnglishTitleInEitherPosition() async {
        let game = GameMetadata(source: .vndb, sourceID: "v1", title: "Summer Pockets")
        for name in ["夏日口袋 (Summer Pockets)", "Summer Pockets / 夏日口袋"] {
            let provider = QueryProvider(source: .vndb, responses: ["Summer Pockets": [game]])
            let outcome = await GameMetadataSearch.automaticMatch(name, providers: [provider])
            #expect(outcome.candidate == game)
            #expect(await provider.queries == [name, "Summer Pockets"])
        }
    }

    @Test func vndbPrefersKanaAndPreservesFirstPureHanAlternative() async {
        let japanese = QueryProvider(source: .vndb)
        let title = "樱之诗（サクラノ詩）"
        _ = await GameMetadataSearch.automaticMatch(title, providers: [japanese])
        #expect(await japanese.queries == [title, "サクラノ詩"])

        // A pure Han Japanese title has no reliable script signal, so retain order.
        let han = QueryProvider(source: .vndb)
        let hanTitle = "星之梦 / 星夢"
        _ = await GameMetadataSearch.automaticMatch(hanTitle, providers: [han])
        #expect(await han.queries == [hanTitle, "星之梦"])
    }

    @Test func exactPrimaryMatchStopsBeforeFallback() async {
        let title = "夏日口袋 (Summer Pockets)"
        let game = GameMetadata(source: .vndb, sourceID: "v1", title: title)
        let provider = QueryProvider(source: .vndb, responses: [title: [game]])
        let outcome = await GameMetadataSearch.automaticMatch(title, providers: [provider])
        #expect(outcome.candidate == game)
        #expect(await provider.queries == [title])
    }

    @Test func fuzzyPrimaryStillSearchesAlternativeWithinBudget() async {
        let title = "夏日口袋 (Summer Pockets)"
        let near = GameMetadata(source: .vndb, sourceID: "v1", title: "Summer Pockets")
        let exact = GameMetadata(
            source: .vndb, sourceID: "v2", title: "Summer Pockets", alternateTitles: ["夏日口袋"]
        )
        let provider = QueryProvider(source: .vndb, responses: [title: [near], "Summer Pockets": [exact]])
        let outcome = await GameMetadataSearch.automaticMatch(title, providers: [provider])
        #expect(outcome.candidate == exact)
        #expect(await provider.queries == [title, "Summer Pockets"])
    }

    @Test func otherSourcesKeepTheirFallbackAndNextMoeUsesOneRequest() async {
        let title = "夏日口袋 (Summer Pockets)"
        let bangumi = QueryProvider(source: .bangumi)
        let vndb = QueryProvider(source: .vndb)
        let nextMoe = QueryProvider(source: .nextmoe)
        _ = await GameMetadataSearch.automaticMatch(title, providers: [bangumi, vndb, nextMoe])
        #expect(await bangumi.queries == [title, "夏日口袋"])
        #expect(await vndb.queries == [title, "Summer Pockets"])
        #expect(await nextMoe.queries == ["夏日口袋"])
    }

    @Test func chineseSourcesPreferChineseInEitherPosition() async {
        for title in ["夏日口袋 / Summer Pockets", "Summer Pockets / 夏日口袋"] {
            let bangumi = QueryProvider(source: .bangumi)
            let ymgal = QueryProvider(source: .ymgal)
            let nextMoe = QueryProvider(source: .nextmoe)
            _ = await GameMetadataSearch.automaticMatch(title, providers: [bangumi, ymgal, nextMoe])
            #expect(await bangumi.queries == [title, "夏日口袋"])
            #expect(await ymgal.queries == [title, "夏日口袋"])
            #expect(await nextMoe.queries == ["夏日口袋"])
        }
    }

    @Test func fallbackLanguageSelectionRecognizesCharacterWidths() async {
        let englishTitle = "夏日口袋 / Ｓｕｍｍｅｒ Ｐｏｃｋｅｔｓ"
        let english = QueryProvider(source: .vndb)
        _ = await GameMetadataSearch.automaticMatch(englishTitle, providers: [english])
        #expect(await english.queries == [englishTitle, "Ｓｕｍｍｅｒ Ｐｏｃｋｅｔｓ"])

        let japaneseTitle = "樱之诗 / ｻｸﾗﾉ詩"
        let japanese = QueryProvider(source: .vndb)
        _ = await GameMetadataSearch.automaticMatch(japaneseTitle, providers: [japanese])
        #expect(await japanese.queries == [japaneseTitle, "ｻｸﾗﾉ詩"])

        let parentheticalTitle = "樱之诗（ｻｸﾗﾉ詩）"
        let parenthetical = QueryProvider(source: .vndb)
        _ = await GameMetadataSearch.automaticMatch(parentheticalTitle, providers: [parenthetical])
        #expect(await parenthetical.queries == [parentheticalTitle, "ｻｸﾗﾉ詩"])
    }
}

private actor QueryProvider: GameMetadataProvider {
    let source: GameMetadataSource
    let responses: [String: [GameMetadata]]
    private(set) var queries: [String] = []

    init(source: GameMetadataSource, responses: [String: [GameMetadata]] = [:]) {
        self.source = source
        self.responses = responses
    }

    func search(_ query: String) async throws -> [GameMetadata] {
        queries.append(query)
        return responses[query] ?? []
    }
}
