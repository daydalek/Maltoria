//
//  LibraryTagsTests.swift
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

// SPDX-License-Identifier: GPL-3.0-or-later
import Foundation
import Testing
@testable import WhiskyCore

@Suite("LibraryTags & LibraryLaunchOptions")
struct LibraryTagsTests {
    private func temporaryBottle() throws -> URL {
        let dir = FileManager.default.temporaryDirectory
            .appendingPathComponent("WhiskyCore-LibraryTags-\(UUID().uuidString)", isDirectory: true)
        try FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        return dir
    }

    @Test("Normalize collapses whitespace, rejects nul, deduplicates case-insensitively and limits count/length")
    func normalization() {
        #expect(LibraryTags.normalize("   ") == nil)
        #expect(LibraryTags.normalize("bad\0tag") == nil)
        #expect(LibraryTags.normalize("  Visual   Novel \n ") == "Visual Novel")
        let tooLong = String(repeating: "a", count: 40)
        #expect(LibraryTags.normalize(tooLong)?.count == LibraryTags.maxTagLength)

        let raw = ["RPG", "rpg", "  Visual   Novel ", "", "Action", "action"]
        #expect(LibraryTags.normalizeList(raw) == ["RPG", "Visual Novel", "Action"])

        let forty = (1 ... 40).map { "Tag-\($0)" }
        #expect(LibraryTags.normalizeList(forty).count == LibraryTags.maxTagsPerEntry)
    }

    @Test("Round-trip save/load, renameTag (including merge), and deleteTag")
    func roundTripRenameAndDelete() throws {
        let bottle = try temporaryBottle()
        defer { try? FileManager.default.removeItem(at: bottle) }

        var store = LibraryTags()
        #expect(store.setTags(["RPG", "日系", "rpg"], forIdentity: "entry-1") == ["RPG", "日系"])
        #expect(store.setTags(["RPG", "Action"], forIdentity: "entry-2") == ["RPG", "Action"])
        try store.save(bottleURL: bottle)

        var reloaded = try LibraryTags.load(bottleURL: bottle)
        #expect(reloaded.tags(forIdentity: "entry-1") == ["RPG", "日系"])
        #expect(reloaded.tags(forIdentity: "entry-2") == ["RPG", "Action"])

        let renamed = reloaded.renameTag(from: "rpg", to: "Action")
        #expect(renamed)
        #expect(reloaded.tags(forIdentity: "entry-1") == ["Action", "日系"])
        // Renaming RPG -> Action deduplicates entry-2 cleanly
        #expect(reloaded.tags(forIdentity: "entry-2") == ["Action"])

        let deleted = reloaded.deleteTag("action")
        #expect(deleted)
        #expect(reloaded.tags(forIdentity: "entry-1") == ["日系"])
        #expect(reloaded.tags(forIdentity: "entry-2").isEmpty)
        #expect(reloaded.entryTags["entry-2"] == nil)
    }

    @Test("LibraryLaunchOptions resolves relative bottle keys, filters missing exes, and tracks lastExecutable")
    func launchOptionsResolution() throws {
        let bottle = try temporaryBottle()
        defer { try? FileManager.default.removeItem(at: bottle) }

        let gameDir = bottle.appendingPathComponent("drive_c/Games/Multi", isDirectory: true)
        try FileManager.default.createDirectory(at: gameDir, withIntermediateDirectories: true)
        let exe1 = gameDir.appendingPathComponent("Game.exe")
        let exe2 = gameDir.appendingPathComponent("Config.exe")
        let nonExe = gameDir.appendingPathComponent("Readme.txt")
        let missingExe = gameDir.appendingPathComponent("Missing.exe")
        for url in [exe1, exe2, nonExe] {
            FileManager.default.createFile(atPath: url.path, contents: Data())
        }

        let entry = LibraryEntry(
            id: "folder:\(gameDir.path)",
            name: "Multi",
            iconURL: nil,
            bottleURL: bottle,
            source: .folder,
            launch: .program(gameDir)
        )

        var options = LibraryLaunchOptions()
        options.setOptions(
            EntryLaunchOptions(
                multiExecutable: true,
                executables: [
                    FolderGameLibrary.key(for: exe1, bottleURL: bottle),
                    FolderGameLibrary.key(for: exe2, bottleURL: bottle),
                    FolderGameLibrary.key(for: nonExe, bottleURL: bottle),
                    FolderGameLibrary.key(for: missingExe, bottleURL: bottle)
                ],
                lastExecutable: FolderGameLibrary.key(for: exe2, bottleURL: bottle)
            ),
            for: entry
        )
        try options.save(bottleURL: bottle)

        let reloaded = try LibraryLaunchOptions.load(bottleURL: bottle)
        #expect(reloaded.options(for: entry).multiExecutable)
        #expect(reloaded.resolvedExecutables(for: entry).map(\.standardizedFileURL) == [
            exe1.standardizedFileURL,
            exe2.standardizedFileURL
        ])
        #expect(reloaded.resolvedExecutables(for: entry, existingOnly: false).map(\.standardizedFileURL) == [
            exe1.standardizedFileURL,
            exe2.standardizedFileURL,
            missingExe.standardizedFileURL
        ])
        #expect(reloaded.resolvedLastExecutable(for: entry)?.standardizedFileURL == exe2.standardizedFileURL)

        // Setting default removes entry key
        var cleared = reloaded
        cleared.setOptions(EntryLaunchOptions(), for: entry)
        #expect(cleared.entries.isEmpty)
    }
}
