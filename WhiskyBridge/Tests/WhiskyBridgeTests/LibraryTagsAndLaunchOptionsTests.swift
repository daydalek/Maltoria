import Foundation
import Testing
import WhiskyApplication
import WhiskyBridgeProtocol
import WhiskyCore

@Suite("Entry tags and multi-exe launch options")
@MainActor
struct LibraryTagsAndLaunchOptionsTests {
    @MainActor
    private struct Fixture {
        let root: URL
        let bottle: URL
        let secondBottle: URL
        let gameDir: URL
        let exe1: URL
        let exe2: URL
        let secondGameDir: URL
        let secondExe: URL
        let workspace: ApplicationWorkspace
        let service: ApplicationService

        init() throws {
            root = FileManager.default.temporaryDirectory
                .appendingPathComponent("WhiskyBridge-Tags-\(UUID().uuidString)", isDirectory: true)
            bottle = root.appendingPathComponent("BottleA", isDirectory: true)
            secondBottle = root.appendingPathComponent("BottleB", isDirectory: true)
            let gamesRoot = bottle.appendingPathComponent("drive_c/Games", isDirectory: true)
            gameDir = gamesRoot.appendingPathComponent("MultiEntryWork", isDirectory: true)
            exe1 = gameDir.appendingPathComponent("Game.exe")
            exe2 = gameDir.appendingPathComponent("Config.exe")

            let secondRoot = secondBottle.appendingPathComponent("drive_c/Games", isDirectory: true)
            secondGameDir = secondRoot.appendingPathComponent("OtherWork", isDirectory: true)
            secondExe = secondGameDir.appendingPathComponent("Play.exe")

            try FileManager.default.createDirectory(at: gameDir, withIntermediateDirectories: true)
            try FileManager.default.createDirectory(at: secondGameDir, withIntermediateDirectories: true)
            for url in [exe1, exe2, secondExe] {
                try Data([0x4D, 0x5A]).write(to: url)
            }

            try Self.writeMetadata(name: "Bottle A", to: bottle.appendingPathComponent("Metadata.plist"))
            try Self.writeMetadata(name: "Bottle B", to: secondBottle.appendingPathComponent("Metadata.plist"))

            workspace = ApplicationWorkspace(containers: [bottle, secondBottle])
            service = ApplicationService(workspace: workspace)
        }

        func cleanup() {
            try? FileManager.default.removeItem(at: root)
        }

        private static func writeMetadata(name: String, to url: URL) throws {
            let plist: [String: Any] = ["info": ["name": name]]
            let data = try PropertyListSerialization.data(fromPropertyList: plist, format: .xml, options: 0)
            try data.write(to: url)
        }
    }

    @Test("Describe advertises tag and launch option methods and capabilities")
    func describeAdvertisesCapabilities() async throws {
        let fixture = try Fixture()
        defer { fixture.cleanup() }

        let result = try await fixture.service.call("bridge.describe", params: [:])
        let methods = Set(result.object?["methods"]?.array?.compactMap(\.string) ?? [])
        let capabilities = Set(result.object?["capabilities"]?.array?.compactMap(\.string) ?? [])

        #expect(methods.contains("entry.tags"))
        #expect(methods.contains("entry.launchOptions"))
        #expect(methods.contains("library.tags.rename"))
        #expect(methods.contains("library.tags.delete"))
        #expect(capabilities.contains("tags.edit"))
        #expect(capabilities.contains("launchOptions.edit"))
    }

    @Test("Entry tags set, rename, delete, and entry.launchOptions persist across library.entries")
    func tagsAndLaunchOptionsLifecycle() async throws {
        let fixture = try Fixture()
        defer { fixture.cleanup() }

        _ = try await fixture.service.call("folders.addRoot", params: [
            "container": .string(fixture.bottle.path),
            "path": .string(fixture.gameDir.deletingLastPathComponent().path)
        ])
        _ = try await fixture.service.call("folders.addRoot", params: [
            "container": .string(fixture.secondBottle.path),
            "path": .string(fixture.secondGameDir.deletingLastPathComponent().path)
        ])

        let initialEntries = try await fixture.service.call("library.entries", params: [
            "container": .string(fixture.bottle.path)
        ])
        let entryID = try #require(initialEntries.array?.first?.object?["id"]?.string)
        #expect(initialEntries.array?.first?.object?["needsExecutable"]?.bool == true)
        #expect(initialEntries.array?.first?.object?["multiExecutable"]?.bool == false)

        let secondEntries = try await fixture.service.call("library.entries", params: [
            "container": .string(fixture.secondBottle.path)
        ])
        let secondEntryID = try #require(secondEntries.array?.first?.object?["id"]?.string)

        let tagReply = try await fixture.service.call("entry.tags", params: [
            "container": .string(fixture.bottle.path),
            "entryID": .string(entryID),
            "tags": .array([.string("  RPG  "), .string("rpg"), .string("Dual-Exe")])
        ])
        #expect(tagReply.object?["tags"]?.array?.compactMap(\.string) == ["RPG", "Dual-Exe"])

        _ = try await fixture.service.call("entry.tags", params: [
            "container": .string(fixture.secondBottle.path),
            "entryID": .string(secondEntryID),
            "tags": .array([.string("RPG")])
        ])

        let renameReply = try await fixture.service.call("library.tags.rename", params: [
            "oldTag": .string("rpg"),
            "newTag": .string("JRPG")
        ])
        #expect(renameReply.object?["updated"]?.integer == 2)

        let launchReply = try await fixture.service.call("entry.launchOptions", params: [
            "container": .string(fixture.bottle.path),
            "entryID": .string(entryID),
            "multiExecutable": .bool(true),
            "executables": .array([.string(fixture.exe1.path), .string(fixture.exe2.path)])
        ])
        #expect(launchReply.object?["multiExecutable"]?.bool == true)
        #expect(launchReply.object?["executables"]?.array?.compactMap(\.string).count == 2)
        #expect(launchReply.object?["lastExecutable"] == .null)

        // Simulate recording a launch of the second executable
        let workspaceEntry = try await fixture.workspace.entry([
            "container": .string(fixture.bottle.path),
            "entryID": .string(entryID)
        ])
        try fixture.workspace.recordLastExecutable(fixture.exe2, entry: workspaceEntry)

        let reloaded = try await fixture.service.call("library.entries", params: [
            "container": .string(fixture.bottle.path)
        ])
        let entryObj = try #require(reloaded.array?.first?.object)
        #expect(entryObj["tags"]?.array?.compactMap(\.string) == ["JRPG", "Dual-Exe"])
        #expect(entryObj["multiExecutable"]?.bool == true)
        #expect(entryObj["needsExecutable"]?.bool == false)
        #expect(entryObj["lastExecutable"]?.string == fixture.exe2.standardizedFileURL.path)
        #expect(entryObj["launchExecutables"]?.array?.compactMap(\.string).count == 2)

        let secondReloaded = try await fixture.service.call("library.entries", params: [
            "container": .string(fixture.secondBottle.path)
        ])
        #expect(secondReloaded.array?.first?.object?["tags"]?.array?.compactMap(\.string) == ["JRPG"])

        let deleteReply = try await fixture.service.call("library.tags.delete", params: [
            "container": .string(fixture.bottle.path),
            "tag": .string("jrpg")
        ])
        #expect(deleteReply.object?["updated"]?.integer == 1)

        let afterDelete = try await fixture.service.call("library.entries", params: [
            "container": .string(fixture.bottle.path)
        ])
        #expect(afterDelete.array?.first?.object?["tags"]?.array?.compactMap(\.string) == ["Dual-Exe"])
    }
}
