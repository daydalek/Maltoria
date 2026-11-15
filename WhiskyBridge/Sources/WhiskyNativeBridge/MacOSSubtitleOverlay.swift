//
//  MacOSSubtitleOverlay.swift
//  WhiskyNativeBridge
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
import AppKit
import WhiskyApplication
import WhiskyBridgeProtocol

private final class SubtitlePanel: NSPanel {
    override var canBecomeKey: Bool { false }
    override var canBecomeMain: Bool { false }
}

@MainActor
final class MacOSSubtitleOverlay: NSObject, NSWindowDelegate {
    private var settings = SubtitleOverlaySettings()
    private let defaults: UserDefaults
    private var panel: SubtitlePanel?
    private var source: NSTextField?
    private var translation: NSTextField?
    private var status: NSTextField?
    private var latest: JSONValue = .object([:])
    private var applying = false

    init(domain: String) {
        defaults = UserDefaults(suiteName: domain) ?? .standard
        if let data = defaults.data(forKey: "translationOverlay"),
           var restored = try? JSONDecoder().decode(SubtitleOverlaySettings.self, from: data),
           (try? restored.validate()) != nil {
            restored.visible = false
            settings = restored
        }
        super.init()
    }

    func call(_ method: String, params: [String: JSONValue]) throws -> JSONValue {
        if method == "translation.overlay.configure" {
            settings = try settings.merging(params["settings"]?.object ?? params)
            apply()
            persist()
        }
        return try .object([
            "settings": .encoded(settings), "platform": .string("macOS"), "mode": .string("appkit"),
            "supportsClickThrough": .bool(true), "supportsAlwaysOnTop": .bool(true)
        ])
    }

    func update(_ value: JSONValue) {
        latest = value
        refreshText()
    }

    func hide() { settings.visible = false
        panel?.orderOut(nil)
    }

    private func makePanel() -> SubtitlePanel {
        let panel = SubtitlePanel(
            contentRect: .zero,
            styleMask: [.borderless, .nonactivatingPanel, .resizable],
            backing: .buffered,
            defer: false
        )
        panel.level = .floating
        panel.isFloatingPanel = true
        panel.hidesOnDeactivate = false
        panel.isOpaque = false
        panel.backgroundColor = .clear
        panel.hasShadow = true
        panel.isReleasedWhenClosed = false
        panel.isMovableByWindowBackground = true
        panel.collectionBehavior = [.canJoinAllSpaces, .fullScreenAuxiliary, .ignoresCycle]
        panel.minSize = NSSize(width: 320, height: 100)
        panel.maxSize = NSSize(width: 3_840, height: 1_200)
        panel.title = "Maltoria subtitles"
        panel.delegate = self
        let content = NSView()
        content.wantsLayer = true
        content.layer?.cornerRadius = 12
        content.layer?.masksToBounds = true
        panel.contentView = content
        let stack = NSStackView()
        stack.orientation = .vertical
        stack.alignment = .leading
        stack.spacing = 8
        stack.translatesAutoresizingMaskIntoConstraints = false
        content.addSubview(stack)
        NSLayoutConstraint.activate([
            stack.leadingAnchor.constraint(equalTo: content.leadingAnchor, constant: 18),
            stack.trailingAnchor.constraint(equalTo: content.trailingAnchor, constant: -18),
            stack.topAnchor.constraint(equalTo: content.topAnchor, constant: 12),
            stack.bottomAnchor.constraint(lessThanOrEqualTo: content.bottomAnchor, constant: -12)
        ])
        func label() -> NSTextField {
            let value = NSTextField(wrappingLabelWithString: "")
            value.textColor = .white
            value.maximumNumberOfLines = 5
            value.lineBreakMode = .byTruncatingTail
            value.setContentCompressionResistancePriority(.defaultLow, for: .horizontal)
            stack.addArrangedSubview(value)
            value.widthAnchor.constraint(equalTo: stack.widthAnchor).isActive = true
            return value
        }
        source = label()
        translation = label()
        status = label()
        status?.font = .systemFont(ofSize: 13)
        status?.textColor = .lightGray
        return panel
    }

    private func apply() {
        guard settings.visible else { panel?.orderOut(nil)
            return
        }
        if panel == nil { panel = makePanel() }
        guard let panel else { return }
        applying = true
        defer { applying = false }
        let screen = panel.screen ?? NSScreen.main
        let bounds = screen?.visibleFrame ?? NSRect(x: 0, y: 0, width: 1_440, height: 900)
        let width = min(settings.width, bounds.width)
        let height = min(settings.height, bounds.height)
        var frame = NSRect(
            x: settings.x ?? bounds.midX - width / 2,
            y: settings.y ?? bounds.minY + 40,
            width: width,
            height: height
        )
        // Recover panels whose previous display has been disconnected.
        if !NSScreen.screens.contains(where: { $0.visibleFrame.intersects(frame) }) {
            frame.origin = NSPoint(x: bounds.midX - width / 2, y: bounds.minY + 40)
        }
        panel.setFrame(frame, display: true)
        panel.ignoresMouseEvents = settings.clickThrough
        panel.contentView?.layer?.backgroundColor = NSColor.black.withAlphaComponent(settings.opacity).cgColor
        source?.font = .systemFont(ofSize: max(12, settings.fontSize * 0.75))
        source?.textColor = NSColor.white.withAlphaComponent(0.8)
        translation?.font = .systemFont(ofSize: settings.fontSize, weight: .medium)
        refreshText()
        // Neither activation nor key status changes when showing subtitles.
        panel.orderFrontRegardless()
    }

    private func refreshText() {
        let display = latest.object ?? [:]
        source?.stringValue = String((display["source"]?.string ?? "").prefix(4_096))
        translation?.stringValue = String((display["translation"]?.string ?? "").prefix(4_096))
        source?.isHidden = !settings.showSource
        translation?.isHidden = !settings.showTranslation || translation?.stringValue.isEmpty == true
        let message: String = switch display["status"]?.string {
        case "pending": "Translating…"
        case "failed": display["error"]?.string ?? "Translation failed"
        case "cancelled": "Translation cancelled"
        default: ""
        }
        status?.stringValue = message
        status?.isHidden = message.isEmpty
    }

    func windowDidMove(_ notification: Notification) { rememberFrame() }
    func windowDidResize(_ notification: Notification) { rememberFrame() }
    private func rememberFrame() {
        guard !applying, let frame = panel?.frame else { return }
        settings.x = frame.origin.x
        settings.y = frame.origin.y
        settings.width = frame.width
        settings.height = frame.height
        persist()
    }

    private func persist() {
        if let data = try? JSONEncoder().encode(settings) { defaults.set(data, forKey: "translationOverlay") }
    }
}
