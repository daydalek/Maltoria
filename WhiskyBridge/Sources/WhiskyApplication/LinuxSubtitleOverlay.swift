import Foundation
import WhiskyBridgeProtocol
import WhiskyTranslation

@MainActor
final class LinuxSubtitleOverlay {
    static let methods = ["translation.overlay.read", "translation.overlay.configure"]
    private(set) var settings = SubtitleOverlaySettings()
    private let file: URL
    private let isX11: Bool
    private var captureState = "stopped"
    private var gameID: String?

    init(directory: URL = FileTranslationRepository.defaultDirectory(), environment: [String: String] = ProcessInfo.processInfo.environment) {
        file = directory.appendingPathComponent("subtitle-overlay.json")
        isX11 = environment["WAYLAND_DISPLAY"] == nil && environment["DISPLAY"]?.isEmpty == false
        if let data = try? Data(contentsOf: file),
           var restored = try? JSONDecoder().decode(SubtitleOverlaySettings.self, from: data),
           (try? restored.validate()) != nil {
            restored.visible = false
            settings = restored
        }
    }

    func call(_ method: String, params: [String: JSONValue]) throws -> JSONValue {
        if method == "translation.overlay.configure" {
            let next = try settings.merging(params["settings"]?.object ?? params)
            try persist(next)
            settings = next
        }
        return try .object([
            "settings": .encoded(settings), "platform": .string("Linux"), "mode": .string("gpui"),
            "supportsClickThrough": .bool(isX11), "supportsAlwaysOnTop": .bool(isX11)
        ])
    }

    func updateCapture(_ snapshot: JSONValue) {
        let next = snapshot.object?["captureState"]?.string ?? "stopped"
        gameID = snapshot.object?["gameID"]?.string
        if next != captureState {
            if next == "attached" { settings.visible = true }
            if ["stopped", "targetExited", "error", "selecting"].contains(next) { hide() }
        }
        captureState = next
    }

    func runtimeEntriesEnded(_ ids: Set<String>, stoppedCapture: Bool) {
        if stoppedCapture || gameID.map(ids.contains) == true { hide() }
    }

    func hide() { settings.visible = false }

    private func persist(_ value: SubtitleOverlaySettings) throws {
        do {
            let manager = FileManager.default
            try manager.createDirectory(at: file.deletingLastPathComponent(), withIntermediateDirectories: true,
                                        attributes: [.posixPermissions: 0o700])
            try JSONEncoder().encode(value).write(to: file, options: .atomic)
            try manager.setAttributes([.posixPermissions: 0o600], ofItemAtPath: file.path)
        } catch {
            throw BridgeFailure("overlay_persistence_failed", "Could not save subtitle overlay settings")
        }
    }
}
