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

private final class DraggableOverlayContentView: NSView {
    override var mouseDownCanMoveWindow: Bool { true }

    override func acceptsFirstMouse(for event: NSEvent?) -> Bool { true }

    override func mouseDown(with event: NSEvent) {
        if let window, !window.ignoresMouseEvents {
            window.performDrag(with: event)
        } else {
            super.mouseDown(with: event)
        }
    }
}

private final class PassthroughStackView: NSStackView {
    override var mouseDownCanMoveWindow: Bool { true }
    override func hitTest(_ point: NSPoint) -> NSView? { nil }
}

private final class PassthroughLabel: NSTextField {
    override var mouseDownCanMoveWindow: Bool { true }
    override func hitTest(_ point: NSPoint) -> NSView? { nil }
}

struct OverlayResizeEdges: OptionSet, Sendable {
    let rawValue: Int

    static let left = OverlayResizeEdges(rawValue: 1 << 0)
    static let right = OverlayResizeEdges(rawValue: 1 << 1)
    static let bottom = OverlayResizeEdges(rawValue: 1 << 2)
    static let top = OverlayResizeEdges(rawValue: 1 << 3)

    static let topLeft: OverlayResizeEdges = [.top, .left]
    static let topRight: OverlayResizeEdges = [.top, .right]
    static let bottomLeft: OverlayResizeEdges = [.bottom, .left]
    static let bottomRight: OverlayResizeEdges = [.bottom, .right]
}

final class OverlayResizeHandleView: NSView {
    nonisolated static let defaultEdgeThickness: CGFloat = 8
    nonisolated static let defaultCornerSize: CGFloat = 14

    var isResizingEnabled: Bool = false {
        didSet {
            isHidden = !isResizingEnabled
            if !isResizingEnabled {
                hoveredEdges = []
                activeEdges = []
                activeVisibleBounds = nil
                isActivelyResizing = false
            }
            window?.invalidateCursorRects(for: self)
            needsDisplay = true
        }
    }

    private var hoveredEdges: OverlayResizeEdges = []
    private var activeEdges: OverlayResizeEdges = []
    private var initialMouseScreen: NSPoint = .zero
    private var initialFrame: NSRect = .zero
    private var activeVisibleBounds: NSRect?
    private var isActivelyResizing: Bool = false {
        didSet {
            if oldValue != isActivelyResizing {
                needsDisplay = true
            }
        }
    }

    private var resizeTrackingArea: NSTrackingArea?

    override var mouseDownCanMoveWindow: Bool { false }

    override func acceptsFirstMouse(for event: NSEvent?) -> Bool { true }

    nonisolated static func hitTestEdges(
        at localPoint: NSPoint,
        in bounds: NSRect,
        edgeThickness: CGFloat = defaultEdgeThickness,
        cornerSize: CGFloat = defaultCornerSize
    ) -> OverlayResizeEdges {
        guard !bounds.isEmpty,
              bounds.width > 0,
              bounds.height > 0,
              edgeThickness > 0,
              cornerSize > 0,
              localPoint.x >= bounds.minX,
              localPoint.x <= bounds.maxX,
              localPoint.y >= bounds.minY,
              localPoint.y <= bounds.maxY
        else {
            return []
        }

        let leftDist = localPoint.x - bounds.minX
        let rightDist = bounds.maxX - localPoint.x
        let bottomDist = localPoint.y - bounds.minY
        let topDist = bounds.maxY - localPoint.y

        let cornerX = min(cornerSize, bounds.width / 2)
        let cornerY = min(cornerSize, bounds.height / 2)
        if leftDist <= cornerX, topDist <= cornerY { return .topLeft }
        if rightDist <= cornerX, topDist <= cornerY { return .topRight }
        if leftDist <= cornerX, bottomDist <= cornerY { return .bottomLeft }
        if rightDist <= cornerX, bottomDist <= cornerY { return .bottomRight }

        let edgeX = min(edgeThickness, bounds.width / 2)
        let edgeY = min(edgeThickness, bounds.height / 2)
        var edges: OverlayResizeEdges = []
        if leftDist <= edgeX {
            edges.insert(.left)
        } else if rightDist <= edgeX {
            edges.insert(.right)
        }
        if bottomDist <= edgeY {
            edges.insert(.bottom)
        } else if topDist <= edgeY {
            edges.insert(.top)
        }
        return edges
    }

    nonisolated static func resizedFrame(
        from initialFrame: NSRect,
        edges: OverlayResizeEdges,
        delta: NSPoint,
        minSize: NSSize,
        maxSize: NSSize,
        visibleBounds: NSRect?
    ) -> NSRect {
        guard !edges.isEmpty else { return initialFrame }

        var minX = initialFrame.minX
        var maxX = initialFrame.maxX
        var minY = initialFrame.minY
        var maxY = initialFrame.maxY

        let minWidth = max(0, minSize.width)
        let maxWidth = max(minWidth, maxSize.width)
        let minHeight = max(0, minSize.height)
        let maxHeight = max(minHeight, maxSize.height)

        if edges.contains(.left), !edges.contains(.right) {
            let lower = visibleBounds.map { max($0.minX, maxX - maxWidth) } ?? (maxX - maxWidth)
            let upper = maxX - minWidth
            minX = min(max(initialFrame.minX + delta.x, lower), upper)
        } else if edges.contains(.right), !edges.contains(.left) {
            let lower = minX + minWidth
            let upper = visibleBounds.map { min($0.maxX, minX + maxWidth) } ?? (minX + maxWidth)
            maxX = max(lower, min(initialFrame.maxX + delta.x, upper))
        }

        if edges.contains(.bottom), !edges.contains(.top) {
            let lower = visibleBounds.map { max($0.minY, maxY - maxHeight) } ?? (maxY - maxHeight)
            let upper = maxY - minHeight
            minY = min(max(initialFrame.minY + delta.y, lower), upper)
        } else if edges.contains(.top), !edges.contains(.bottom) {
            let lower = minY + minHeight
            let upper = visibleBounds.map { min($0.maxY, minY + maxHeight) } ?? (minY + maxHeight)
            maxY = max(lower, min(initialFrame.maxY + delta.y, upper))
        }

        return NSRect(x: minX, y: minY, width: maxX - minX, height: maxY - minY)
    }

    static func cursor(for edges: OverlayResizeEdges) -> NSCursor? {
        if edges == .topLeft || edges == .bottomRight {
            if #available(macOS 15.0, *) {
                let position: NSCursor.FrameResizePosition = edges == .topLeft ? .topLeft : .bottomRight
                return .frameResize(position: position, directions: .all)
            }
            return .crosshair
        }
        if edges == .topRight || edges == .bottomLeft {
            if #available(macOS 15.0, *) {
                let position: NSCursor.FrameResizePosition = edges == .topRight ? .topRight : .bottomLeft
                return .frameResize(position: position, directions: .all)
            }
            return .crosshair
        }
        if edges == .left || edges == .right {
            return .resizeLeftRight
        }
        if edges == .top || edges == .bottom {
            return .resizeUpDown
        }
        return nil
    }

    override func hitTest(_ point: NSPoint) -> NSView? {
        guard !isHidden, isResizingEnabled, let superview else { return nil }
        let localPoint = convert(point, from: superview)
        let edges = Self.hitTestEdges(at: localPoint, in: bounds)
        return edges.isEmpty ? nil : self
    }

    override func updateTrackingAreas() {
        super.updateTrackingAreas()
        if let resizeTrackingArea {
            removeTrackingArea(resizeTrackingArea)
        }
        let area = NSTrackingArea(
            rect: .zero,
            options: [.mouseMoved, .mouseEnteredAndExited, .cursorUpdate, .activeAlways, .inVisibleRect],
            owner: self,
            userInfo: nil
        )
        addTrackingArea(area)
        resizeTrackingArea = area
    }

    override func resetCursorRects() {
        super.resetCursorRects()
        guard !isHidden, isResizingEnabled, bounds.width > 0, bounds.height > 0 else { return }

        let cornerX = min(Self.defaultCornerSize, bounds.width / 2)
        let cornerY = min(Self.defaultCornerSize, bounds.height / 2)
        let edgeX = min(Self.defaultEdgeThickness, bounds.width / 2)
        let edgeY = min(Self.defaultEdgeThickness, bounds.height / 2)

        let regions: [(NSRect, OverlayResizeEdges)] = [
            (NSRect(x: bounds.minX, y: bounds.maxY - cornerY, width: cornerX, height: cornerY), .topLeft),
            (NSRect(x: bounds.maxX - cornerX, y: bounds.maxY - cornerY, width: cornerX, height: cornerY), .topRight),
            (NSRect(x: bounds.minX, y: bounds.minY, width: cornerX, height: cornerY), .bottomLeft),
            (NSRect(x: bounds.maxX - cornerX, y: bounds.minY, width: cornerX, height: cornerY), .bottomRight),
            (
                NSRect(x: bounds.minX, y: bounds.minY + cornerY, width: edgeX, height: max(
                    0,
                    bounds.height - 2 * cornerY
                )),
                .left
            ),
            (
                NSRect(x: bounds.maxX - edgeX, y: bounds.minY + cornerY, width: edgeX, height: max(
                    0,
                    bounds.height - 2 * cornerY
                )),
                .right
            ),
            (
                NSRect(
                    x: bounds.minX + cornerX,
                    y: bounds.minY,
                    width: max(0, bounds.width - 2 * cornerX),
                    height: edgeY
                ),
                .bottom
            ),
            (
                NSRect(
                    x: bounds.minX + cornerX,
                    y: bounds.maxY - edgeY,
                    width: max(0, bounds.width - 2 * cornerX),
                    height: edgeY
                ),
                .top
            )
        ]

        for (rect, edges) in regions where !rect.isEmpty {
            if let cursor = Self.cursor(for: edges) {
                addCursorRect(rect, cursor: cursor)
            }
        }
    }

    override func cursorUpdate(with event: NSEvent) {
        updateHoverCursor(for: event)
    }

    override func mouseMoved(with event: NSEvent) {
        updateHoverCursor(for: event)
    }

    override func mouseExited(with event: NSEvent) {
        if !hoveredEdges.isEmpty {
            hoveredEdges = []
            if !isActivelyResizing {
                NSCursor.arrow.set()
            }
        }
        super.mouseExited(with: event)
    }

    override func mouseDown(with event: NSEvent) {
        guard !isHidden, isResizingEnabled, let window else {
            super.mouseDown(with: event)
            return
        }
        let local = convert(event.locationInWindow, from: nil)
        let edges = Self.hitTestEdges(at: local, in: bounds)
        guard !edges.isEmpty else {
            super.mouseDown(with: event)
            return
        }
        activeEdges = edges
        initialMouseScreen = NSEvent.mouseLocation
        initialFrame = window.frame
        activeVisibleBounds = window.screen?.visibleFrame
            ?? NSScreen.main?.visibleFrame
            ?? NSScreen.screens.first?.visibleFrame
        isActivelyResizing = true
        Self.cursor(for: edges)?.set()
    }

    override func mouseDragged(with event: NSEvent) {
        guard isActivelyResizing, !activeEdges.isEmpty, let window else {
            super.mouseDragged(with: event)
            return
        }
        let currentMouse = NSEvent.mouseLocation
        let delta = NSPoint(
            x: currentMouse.x - initialMouseScreen.x,
            y: currentMouse.y - initialMouseScreen.y
        )
        let targetFrame = Self.resizedFrame(
            from: initialFrame,
            edges: activeEdges,
            delta: delta,
            minSize: window.minSize,
            maxSize: window.maxSize,
            visibleBounds: activeVisibleBounds
        )
        if window.frame != targetFrame {
            window.setFrame(targetFrame, display: true)
        }
        Self.cursor(for: activeEdges)?.set()
    }

    override func mouseUp(with event: NSEvent) {
        guard isActivelyResizing else {
            super.mouseUp(with: event)
            return
        }
        activeEdges = []
        activeVisibleBounds = nil
        isActivelyResizing = false
        updateHoverCursor(for: event)
    }

    override func draw(_ dirtyRect: NSRect) {
        super.draw(dirtyRect)
        guard !isHidden, isResizingEnabled, bounds.width > 24, bounds.height > 24 else { return }

        let inset: CGFloat = 6
        let arm: CGFloat = 8
        let path = NSBezierPath()
        path.lineWidth = 1.5
        path.lineCapStyle = .round
        path.lineJoinStyle = .round

        // swiftlint:disable:next large_tuple
        let corners: [(origin: NSPoint, dx: CGFloat, dy: CGFloat)] = [
            (NSPoint(x: bounds.minX + inset, y: bounds.minY + inset), arm, arm),
            (NSPoint(x: bounds.maxX - inset, y: bounds.minY + inset), -arm, arm),
            (NSPoint(x: bounds.minX + inset, y: bounds.maxY - inset), arm, -arm),
            (NSPoint(x: bounds.maxX - inset, y: bounds.maxY - inset), -arm, -arm)
        ]
        for corner in corners {
            path.move(to: NSPoint(x: corner.origin.x, y: corner.origin.y + corner.dy))
            path.line(to: corner.origin)
            path.line(to: NSPoint(x: corner.origin.x + corner.dx, y: corner.origin.y))
        }

        NSColor.white.withAlphaComponent(isActivelyResizing ? 0.85 : 0.55).setStroke()
        path.stroke()
    }

    private func updateHoverCursor(for event: NSEvent) {
        guard !isHidden, isResizingEnabled else {
            if !hoveredEdges.isEmpty {
                hoveredEdges = []
                NSCursor.arrow.set()
            }
            return
        }
        let edges: OverlayResizeEdges
        if isActivelyResizing {
            edges = activeEdges
        } else {
            let local = convert(event.locationInWindow, from: nil)
            edges = Self.hitTestEdges(at: local, in: bounds)
        }
        if let cursor = Self.cursor(for: edges) {
            hoveredEdges = edges
            cursor.set()
        } else if !hoveredEdges.isEmpty {
            hoveredEdges = []
            NSCursor.arrow.set()
        }
    }
}

@MainActor
final class MacOSSubtitleOverlay: NSObject, NSWindowDelegate {
    let strings = CapturePopupStrings()

    func refreshLocalization() {
        panel?.title = strings.text("overlay_title")
        refreshText()
    }

    private var settings = SubtitleOverlaySettings()
    private let defaults: UserDefaults
    private var panel: SubtitlePanel?
    private var source: NSTextField?
    private var translation: NSTextField?
    private var status: NSTextField?
    private var resizeHandle: OverlayResizeHandleView?
    private var latest: JSONValue = .object([:])
    private var applying = false
    private var lastWrappingWidth: CGFloat?
    private var reassertObserver: OverlayReassertObserver?

    var onSettingsChanged: ((SubtitleOverlaySettings) -> Void)?
    var isVisible: Bool { settings.visible }
    var clickThrough: Bool { settings.clickThrough }
    var panelForTesting: NSPanel? { panel }
    var sourceLabelForTesting: NSTextField? { source }
    var translationLabelForTesting: NSTextField? { translation }
    var statusLabelForTesting: NSTextField? { status }
    var resizeHandleForTesting: OverlayResizeHandleView? { resizeHandle }

    init(domain: String) {
        defaults = UserDefaults(suiteName: domain) ?? .standard
        if let data = defaults.data(forKey: "translationOverlay"),
           var restored = try? JSONDecoder().decode(SubtitleOverlaySettings.self, from: data),
           (try? restored.validate()) != nil {
            restored.visible = false
            settings = restored
        }
        super.init()
        reassertObserver = OverlayReassertObserver { [weak self] in
            self?.reassertPanelForFullscreen()
        }
    }

    func invalidate() {
        reassertObserver?.invalidate()
        reassertObserver = nil
    }

    func setVisible(_ visible: Bool) {
        settings.visible = visible
        apply()
        persist()
        onSettingsChanged?(settings)
    }

    func setClickThrough(_ enabled: Bool) {
        settings.clickThrough = enabled
        apply()
        persist()
        onSettingsChanged?(settings)
    }

    func call(_ method: String, params: [String: JSONValue]) throws -> JSONValue {
        if method == "translation.overlay.configure" {
            settings = try settings.merging(params["settings"]?.object ?? params)
            apply()
            persist()
            onSettingsChanged?(settings)
        }
        return try .object([
            "settings": .encoded(settings), "platform": .string("macOS"), "mode": .string("appkit"),
            "supportsClickThrough": .bool(true), "supportsAlwaysOnTop": .bool(true)
        ])
    }

    func update(_ value: JSONValue) {
        latest = value
        refreshText()
        guard settings.visible else { return }
        if panel == nil {
            apply()
            return
        }
        guard let panel else { return }
        let targetScreen = NSScreen.main ?? NSScreen.screens.first
        let screenChanged = targetScreen != nil && panel.screen != nil && panel.screen != targetScreen
        if !panel.isVisible || !panel.isOnActiveSpace || screenChanged || panel.level != FullscreenOverlaySupport
            .windowLevel {
            reassertPanelForFullscreen()
        }
    }

    func hide() {
        settings.visible = false
        panel?.orderOut(nil)
    }

    private func makePanel() -> SubtitlePanel {
        let panel = SubtitlePanel(
            contentRect: .zero,
            styleMask: [.borderless, .nonactivatingPanel, .resizable],
            backing: .buffered,
            defer: false
        )
        FullscreenOverlaySupport.configurePanel(panel, title: strings.text("overlay_title"), delegate: self)
        panel.appearance = NSAppearance(named: .darkAqua)
        panel.isMovableByWindowBackground = true
        panel.minSize = NSSize(width: 320, height: 100)
        panel.maxSize = NSSize(width: 3_840, height: 1_200)
        let content = DraggableOverlayContentView()
        content.wantsLayer = true
        content.layer?.cornerRadius = 12
        content.layer?.masksToBounds = true
        panel.contentView = content
        let stack = PassthroughStackView()
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
            let value = PassthroughLabel(wrappingLabelWithString: "")
            value.isBezeled = false
            value.isEditable = false
            value.isSelectable = false
            value.drawsBackground = false
            value.textColor = .white
            value.usesSingleLineMode = false
            value.cell?.wraps = true
            value.cell?.isScrollable = false
            value.maximumNumberOfLines = 0
            value.lineBreakMode = .byWordWrapping
            value.cell?.truncatesLastVisibleLine = true
            value.setContentCompressionResistancePriority(.defaultLow, for: .horizontal)
            value.setContentCompressionResistancePriority(.defaultLow, for: .vertical)
            value.setContentHuggingPriority(.required, for: .vertical)
            stack.addArrangedSubview(value)
            value.widthAnchor.constraint(equalTo: stack.widthAnchor).isActive = true
            return value
        }
        source = label()
        translation = label()
        status = label()
        status?.font = .systemFont(ofSize: 13)
        status?.textColor = .lightGray
        let handle = OverlayResizeHandleView()
        handle.translatesAutoresizingMaskIntoConstraints = false
        content.addSubview(handle)
        NSLayoutConstraint.activate([
            handle.leadingAnchor.constraint(equalTo: content.leadingAnchor),
            handle.trailingAnchor.constraint(equalTo: content.trailingAnchor),
            handle.topAnchor.constraint(equalTo: content.topAnchor),
            handle.bottomAnchor.constraint(equalTo: content.bottomAnchor)
        ])
        resizeHandle = handle
        return panel
    }

    private func resolvedFrame(for panel: SubtitlePanel?) -> NSRect {
        let screen = panel?.screen ?? NSScreen.main ?? NSScreen.screens.first
        let bounds = screen?.visibleFrame ?? NSRect(x: 0, y: 0, width: 1_440, height: 900)
        let width = min(settings.width, bounds.width)
        let height = min(settings.height, bounds.height)
        var frame = NSRect(
            x: settings.x ?? bounds.midX - width / 2,
            y: settings.y ?? bounds.minY + 40,
            width: width,
            height: height
        )
        // Recover panels whose previous display has been disconnected or whose display shrank in fullscreen.
        if !NSScreen.screens.isEmpty,
           !NSScreen.screens.contains(where: { $0.visibleFrame.intersects(frame) }) {
            frame.origin = NSPoint(x: bounds.midX - width / 2, y: bounds.minY + 40)
        }
        frame.origin.x = max(bounds.minX, min(frame.origin.x, bounds.maxX - width))
        frame.origin.y = max(bounds.minY, min(frame.origin.y, bounds.maxY - height))
        return frame
    }

    private func updateWrappingWidths(for panelWidth: CGFloat) {
        let wrappingWidth = max(120, panelWidth - 36)
        let widthChanged = lastWrappingWidth != wrappingWidth
        lastWrappingWidth = wrappingWidth
        source?.preferredMaxLayoutWidth = wrappingWidth
        translation?.preferredMaxLayoutWidth = wrappingWidth
        status?.preferredMaxLayoutWidth = wrappingWidth
        if widthChanged {
            panel?.contentView?.layoutSubtreeIfNeeded()
        }
    }

    private func reassertPanelForFullscreen() {
        guard settings.visible else { return }
        if panel == nil {
            apply()
            return
        }
        guard let panel else { return }
        applying = true
        defer { applying = false }
        let targetFrame = resolvedFrame(for: panel)
        if panel.frame != targetFrame {
            panel.setFrame(targetFrame, display: true)
        }
        updateWrappingWidths(for: panel.frame.width)
        FullscreenOverlaySupport.applyOrderAndLevel(panel)
    }

    private func apply() {
        guard settings.visible else {
            panel?.orderOut(nil)
            return
        }
        if panel == nil { panel = makePanel() }
        guard let panel else { return }
        applying = true
        defer { applying = false }
        let targetFrame = resolvedFrame(for: panel)
        panel.setFrame(targetFrame, display: true)
        panel.ignoresMouseEvents = settings.clickThrough
        panel.isMovableByWindowBackground = true
        resizeHandle?.isResizingEnabled = !settings.clickThrough
        panel.hasShadow = settings.opacity > 0.02
        let effectiveOpacity = settings.clickThrough ? settings.opacity : max(0.02, settings.opacity)
        panel.contentView?.layer?.backgroundColor = NSColor.black.withAlphaComponent(effectiveOpacity).cgColor
        if settings.clickThrough {
            panel.contentView?.layer?.borderWidth = 0
            panel.contentView?.layer?.borderColor = nil
        } else {
            panel.contentView?.layer?.borderWidth = 1.5
            panel.contentView?.layer?.borderColor = NSColor.white.withAlphaComponent(0.35).cgColor
        }
        source?.font = .systemFont(ofSize: max(12, settings.fontSize * 0.75))
        source?.textColor = Self.color(
            fromHex: settings.sourceTextColor,
            fallback: NSColor(srgbRed: 0.831, green: 0.831, blue: 0.847, alpha: 1.0)
        )
        translation?.font = .systemFont(ofSize: settings.fontSize, weight: .medium)
        translation?.textColor = Self.color(fromHex: settings.textColor, fallback: .white)
        refreshText()
        updateWrappingWidths(for: panel.frame.width)
        // Neither activation nor key status changes when showing subtitles.
        FullscreenOverlaySupport.applyOrderAndLevel(panel)
    }

    private static func color(fromHex hex: String, fallback: NSColor) -> NSColor {
        guard hex.count == 7, hex.first == "#",
              hex.unicodeScalars.dropFirst().allSatisfy({
                  (48 ... 57).contains($0.value) || (65 ... 70).contains($0.value) || (97 ... 102).contains($0.value)
              }),
              let value = UInt32(String(hex.dropFirst()), radix: 16)
        else { return fallback }
        let red = CGFloat((value >> 16) & 0xFF) / 255.0
        let green = CGFloat((value >> 8) & 0xFF) / 255.0
        let blue = CGFloat(value & 0xFF) / 255.0
        return NSColor(srgbRed: red, green: green, blue: blue, alpha: 1.0)
    }

    private func refreshText() {
        let display = latest.object ?? [:]
        source?.stringValue = String((display["source"]?.string ?? "").prefix(4_096))
        translation?.stringValue = String((display["translation"]?.string ?? "").prefix(4_096))
        source?.isHidden = !settings.showSource
        translation?.isHidden = !settings.showTranslation || translation?.stringValue.isEmpty == true
        let message: String = switch display["status"]?.string {
        case "pending": strings.text("translating")
        case "failed": (display["error"]?.string).map { strings.errorText($0) } ?? strings.text("translation_failed")
        case "cancelled": strings.text("translation_cancelled")
        default: ""
        }
        status?.stringValue = message
        status?.isHidden = message.isEmpty
    }

    func windowDidMove(_ notification: Notification) { rememberFrame() }
    func windowDidResize(_ notification: Notification) {
        rememberFrame()
        if let width = panel?.frame.width {
            updateWrappingWidths(for: width)
        }
    }

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
