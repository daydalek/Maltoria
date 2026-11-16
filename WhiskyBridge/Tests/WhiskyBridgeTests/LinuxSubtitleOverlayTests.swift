import Foundation
import Testing
import WhiskyBridgeProtocol
@testable import WhiskyApplication

@Suite("Linux subtitle overlay")
@MainActor
struct LinuxSubtitleOverlayTests {
    @Test func readConfigureValidationAndPersistence() async throws {
        let directory = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        defer { try? FileManager.default.removeItem(at: directory) }
        let native = LinuxApplicationServices(overlayDirectory: directory)
        #expect(native.capabilities.contains("translation.overlay"))
        #expect(native.methods.contains("translation.overlay.configure"))
        let workspace = ApplicationWorkspace(containers: [])
        let initial = try await native.call("translation.overlay.read", params: [:], workspace: workspace)
        #expect(initial.object?["platform"] == .string("Linux"))
        #expect(initial.object?["mode"] == .string("gpui"))
        let configured = try await native.call("translation.overlay.configure", params: ["settings": .object([
            "visible": .bool(true), "width": .integer(720), "x": .integer(42), "textColor": .string("#12ABef")
        ])], workspace: workspace)
        #expect(configured.object?["settings"]?.object?["visible"] == .bool(true))
        let file = directory.appendingPathComponent("subtitle-overlay.json")
        let permissions = try FileManager.default.attributesOfItem(atPath: file.path)[.posixPermissions] as? Int
        #expect(permissions == 0o600)
        let restored = LinuxSubtitleOverlay(directory: directory)
        #expect(restored.settings.visible == false)
        #expect(restored.settings.width == 720 && restored.settings.x == 42)
        #expect(restored.settings.textColor == "#12ABef")
        #expect(throws: (any Error).self) {
            try restored.call("translation.overlay.configure", params: ["opacity": .integer(2)])
        }
        #expect(restored.settings.opacity == 0.85)
        #expect(throws: (any Error).self) {
            try restored.call("translation.overlay.configure", params: ["showSource": .bool(false), "showTranslation": .bool(false)])
        }
    }

    @Test func capabilitiesAndCaptureLifecycle() throws {
        let directory = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        let x11 = LinuxSubtitleOverlay(directory: directory, environment: ["DISPLAY": ":0"])
        let wayland = LinuxSubtitleOverlay(directory: directory, environment: ["DISPLAY": ":0", "WAYLAND_DISPLAY": "wayland-0"])
        #expect(try x11.call("translation.overlay.read", params: [:]).object?["supportsClickThrough"] == .bool(true))
        #expect(try wayland.call("translation.overlay.read", params: [:]).object?["supportsAlwaysOnTop"] == .bool(false))
        for state in ["stopped", "targetExited", "selecting", "error"] {
            x11.updateCapture(.object(["captureState": .string("attached"), "gameID": .string("game")]))
            #expect(x11.settings.visible)
            x11.updateCapture(.object(["captureState": .string(state)]))
            #expect(!x11.settings.visible)
        }
        x11.updateCapture(.object(["captureState": .string("attached"), "gameID": .string("game")]))
        x11.runtimeEntriesEnded(["other"], stoppedCapture: false)
        #expect(x11.settings.visible)
        x11.runtimeEntriesEnded(["game"], stoppedCapture: false)
        #expect(!x11.settings.visible)
    }

    @Test func persistenceFailureDoesNotApplyChanges() throws {
        let overlay = LinuxSubtitleOverlay(directory: URL(fileURLWithPath: "/dev/null/overlay"))
        #expect(throws: (any Error).self) { try overlay.call("translation.overlay.configure", params: ["visible": .bool(true)]) }
        #expect(!overlay.settings.visible)
    }
}
