//
//  CaptureOverlayAndHUDTests.swift
//  WhiskyNativeBridgeTests
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

import AppKit
import Foundation
import Testing
import WhiskyApplication
import WhiskyBridgeProtocol
@testable import WhiskyNativeBridge

@Test @MainActor func subtitleOverlayUsesScreenSaverLevelAndFullscreenCollectionBehavior() throws {
    let domain = "com.daydalek.Maltoria.overlay.test.\(UUID().uuidString)"
    defer { UserDefaults(suiteName: domain)?.removePersistentDomain(forName: domain) }
    let overlay = MacOSSubtitleOverlay(domain: domain)
    defer {
        overlay.hide()
        overlay.invalidate()
    }

    #expect(!overlay.isVisible)
    #expect(overlay.clickThrough)

    overlay.setVisible(true)
    #expect(overlay.isVisible)

    let panel = try #require(overlay.panelForTesting)
    #expect(panel.level == FullscreenOverlaySupport.windowLevel)
    #expect(panel.level.rawValue > Int(CGShieldingWindowLevel()) + 2)
    #expect(!panel.isFloatingPanel)
    #expect(!panel.hidesOnDeactivate)
    #expect(!panel.canBecomeKey)
    #expect(!panel.canBecomeMain)
    #expect(panel.styleMask.contains(.borderless))
    #expect(panel.styleMask.contains(.nonactivatingPanel))
    #expect(panel.styleMask.contains(.resizable))
    #expect(panel.collectionBehavior.contains(.canJoinAllSpaces))
    #expect(panel.collectionBehavior.contains(.fullScreenAuxiliary))
    #expect(panel.collectionBehavior.contains(.ignoresCycle))
    #expect(panel.ignoresMouseEvents)

    overlay.setClickThrough(false)
    #expect(!overlay.clickThrough)
    #expect(!panel.ignoresMouseEvents)

    overlay.update(.object([
        "source": .string("原文"),
        "translation": .string("译文"),
        "status": .string("completed")
    ]))
    #expect(panel.level == FullscreenOverlaySupport.windowLevel)

    overlay.setVisible(false)
    #expect(!overlay.isVisible)
}

@Test @MainActor func captureHUDPanelConfigurationAndServiceMethodGating() async throws {
    let domain = "com.daydalek.Maltoria.hud.test.\(UUID().uuidString)"
    defer { UserDefaults(suiteName: domain)?.removePersistentDomain(forName: domain) }

    let wineDisabled = MacOSApplicationServices(
        wineEnabled: false,
        preferencesDomain: domain,
        jobs: BridgeJobStore()
    )
    #expect(!wineDisabled.methods.contains("translation.capture.hud/show"))
    #expect(!wineDisabled.methods.contains("translation.capture.hud/hide"))

    let wineEnabled = MacOSApplicationServices(
        wineEnabled: true,
        preferencesDomain: domain,
        jobs: BridgeJobStore()
    )
    #expect(wineEnabled.methods.contains("translation.capture.hud/show"))
    #expect(wineEnabled.methods.contains("translation.capture.hud/hide"))

    let overlay = MacOSSubtitleOverlay(domain: domain)
    let hud = MacOSCaptureHUD(subtitleOverlay: overlay)
    defer {
        hud.hide()
        hud.invalidate()
        overlay.hide()
        overlay.invalidate()
    }

    let root = FileManager.default.temporaryDirectory.appendingPathComponent("whisky-hud-\(UUID())")
    try FileManager.default.createDirectory(at: root, withIntermediateDirectories: true)
    defer { try? FileManager.default.removeItem(at: root) }

    let translation = ApplicationTranslationService(directory: root.appendingPathComponent("Translation"))
    let capture = ApplicationCaptureService(profiles: translation, requirePayload: false) { _, _, _ in
        TranslationHelperHandle {}
    }
    let workspace = ApplicationWorkspace(containers: [])
    hud.configure(capture: capture, translation: translation, workspace: workspace)

    let showResult = try await hud.show(params: [:], workspace: workspace)
    #expect(showResult.object?["visible"] == .bool(true))
    #expect(hud.isVisibleForTesting)
    #expect(!hud.isCollapsedForTesting)

    let panel = try #require(hud.panelForTesting)
    #expect(panel.level == FullscreenOverlaySupport.windowLevel)
    #expect(panel.level.rawValue > Int(CGShieldingWindowLevel()) + 2)
    #expect(!panel.isFloatingPanel)
    #expect(!panel.hidesOnDeactivate)
    #expect(!panel.canBecomeKey)
    #expect(!panel.canBecomeMain)
    #expect(panel.styleMask.contains(.borderless))
    #expect(panel.styleMask.contains(.nonactivatingPanel))
    #expect(panel.collectionBehavior.contains(.canJoinAllSpaces))
    #expect(panel.collectionBehavior.contains(.fullScreenAuxiliary))
    #expect(panel.collectionBehavior.contains(.ignoresCycle))

    hud.hide()
    #expect(!hud.isVisibleForTesting)
}

@Test @MainActor func subtitleOverlayCustomizationShadowBorderDragAndCallback() throws {
    let domain = "com.daydalek.Maltoria.overlay.custom.test.\(UUID().uuidString)"
    defer { UserDefaults(suiteName: domain)?.removePersistentDomain(forName: domain) }
    let overlay = MacOSSubtitleOverlay(domain: domain)
    defer {
        overlay.hide()
        overlay.invalidate()
    }

    var callbackCount = 0
    overlay.onSettingsChanged = { _ in callbackCount += 1 }

    _ = try overlay.call("translation.overlay.configure", params: [
        "visible": .bool(true),
        "clickThrough": .bool(false),
        "opacity": .number(0.0),
        "textColor": .string("#33CC66"),
        "sourceTextColor": .string("#112233")
    ])
    #expect(callbackCount == 1)

    let panel = try #require(overlay.panelForTesting)
    #expect(!panel.hasShadow)
    #expect(panel.isMovableByWindowBackground)
    #expect(panel.contentView?.mouseDownCanMoveWindow == true)
    #expect(panel.contentView?.layer?.borderWidth == 1.5)

    let translationColor = try #require(overlay.translationLabelForTesting?.textColor?.usingColorSpace(.sRGB))
    #expect(abs(translationColor.redComponent - (0x33 / 255.0)) < 0.01)
    #expect(abs(translationColor.greenComponent - (0xCC / 255.0)) < 0.01)
    #expect(abs(translationColor.blueComponent - (0x66 / 255.0)) < 0.01)

    let sourceColor = try #require(overlay.sourceLabelForTesting?.textColor?.usingColorSpace(.sRGB))
    #expect(abs(sourceColor.redComponent - (0x11 / 255.0)) < 0.01)
    #expect(abs(sourceColor.greenComponent - (0x22 / 255.0)) < 0.01)
    #expect(abs(sourceColor.blueComponent - (0x33 / 255.0)) < 0.01)

    overlay.setClickThrough(true)
    #expect(callbackCount == 2)
    #expect(panel.contentView?.layer?.borderWidth == 0)

    _ = try overlay.call("translation.overlay.configure", params: ["opacity": .number(0.5)])
    #expect(callbackCount == 3)
    #expect(panel.hasShadow)

    overlay.setVisible(false)
    #expect(callbackCount == 4)
}

@Test @MainActor func captureHUDExpandedAndPillTogglesSyncWithOverlay() async throws {
    let domain = "com.daydalek.Maltoria.hud.toggle.test.\(UUID().uuidString)"
    defer { UserDefaults(suiteName: domain)?.removePersistentDomain(forName: domain) }
    let overlay = MacOSSubtitleOverlay(domain: domain)
    let hud = MacOSCaptureHUD(subtitleOverlay: overlay)
    defer {
        hud.hide()
        hud.invalidate()
        overlay.hide()
        overlay.invalidate()
    }

    let root = FileManager.default.temporaryDirectory.appendingPathComponent("whisky-hud-toggle-\(UUID())")
    try FileManager.default.createDirectory(at: root, withIntermediateDirectories: true)
    defer { try? FileManager.default.removeItem(at: root) }

    let translation = ApplicationTranslationService(directory: root.appendingPathComponent("Translation"))
    let capture = ApplicationCaptureService(profiles: translation, requirePayload: false) { _, _, _ in
        TranslationHelperHandle {}
    }
    let workspace = ApplicationWorkspace(containers: [])
    hud.configure(capture: capture, translation: translation, workspace: workspace)

    _ = try await hud.show(params: [:], workspace: workspace)
    let panel = try #require(hud.panelForTesting)

    func buttons(in view: NSView?) -> [NSButton] {
        guard let view else { return [] }
        var result: [NSButton] = []
        if let button = view as? NSButton { result.append(button) }
        for subview in view.subviews {
            result.append(contentsOf: buttons(in: subview))
        }
        return result
    }

    func buttonTitled(_ title: String) -> NSButton? {
        buttons(in: panel.contentView).first { $0.attributedTitle.string == title }
    }

    // Expanded card starts with 字幕关 and 锁定
    #expect(buttonTitled("字幕关") != nil)
    #expect(buttonTitled("锁定") != nil)

    // Toggle from expanded card
    try #require(buttonTitled("字幕关")).performClick(nil)
    #expect(overlay.isVisible)
    #expect(buttonTitled("字幕开") != nil)

    try #require(buttonTitled("锁定")).performClick(nil)
    #expect(!overlay.clickThrough)
    #expect(buttonTitled("可拖动") != nil)

    // Collapse into floating pill and verify width & quick toggles
    try #require(buttonTitled("收起")).performClick(nil)
    #expect(hud.isCollapsedForTesting)
    #expect(panel.frame.width == 286)
    #expect(buttonTitled("字幕开") != nil)
    #expect(buttonTitled("可拖动") != nil)
    #expect(buttonTitled("展开") != nil)

    // Toggle from pill
    try #require(buttonTitled("可拖动")).performClick(nil)
    #expect(overlay.clickThrough)
    #expect(buttonTitled("锁定") != nil)

    // External GPUI configure call syncs HUD pill automatically
    _ = try overlay.call("translation.overlay.configure", params: [
        "visible": .bool(false),
        "clickThrough": .bool(false)
    ])
    #expect(buttonTitled("字幕关") != nil)
    #expect(buttonTitled("可拖动") != nil)
}

@Test @MainActor func subtitleOverlayMultiLineWrappingTracksPanelWidth() throws {
    let domain = "com.daydalek.Maltoria.overlay.wrapping.test.\(UUID().uuidString)"
    defer { UserDefaults(suiteName: domain)?.removePersistentDomain(forName: domain) }
    let overlay = MacOSSubtitleOverlay(domain: domain)
    defer {
        overlay.hide()
        overlay.invalidate()
    }

    _ = try overlay.call("translation.overlay.configure", params: [
        "visible": .bool(true),
        "width": .number(600),
        "height": .number(180)
    ])

    let panel = try #require(overlay.panelForTesting)
    let source = try #require(overlay.sourceLabelForTesting)
    let translation = try #require(overlay.translationLabelForTesting)
    let status = try #require(overlay.statusLabelForTesting)

    for label in [source, translation, status] {
        #expect(label.lineBreakMode == .byWordWrapping)
        #expect(label.maximumNumberOfLines == 0)
        #expect(!label.usesSingleLineMode)
        #expect(label.cell?.wraps == true)
        #expect(label.cell?.isScrollable == false)
        #expect(label.cell?.truncatesLastVisibleLine == true)
        #expect(abs(label.preferredMaxLayoutWidth - (panel.frame.width - 36)) < 0.5)
    }

    // Resizing via configure updates preferredMaxLayoutWidth
    _ = try overlay.call("translation.overlay.configure", params: [
        "width": .number(420)
    ])
    for label in [source, translation, status] {
        #expect(abs(label.preferredMaxLayoutWidth - (panel.frame.width - 36)) < 0.5)
    }

    // Resizing via window frame + delegate callback updates preferredMaxLayoutWidth
    var resizedFrame = panel.frame
    resizedFrame.size.width = 500
    panel.setFrame(resizedFrame, display: true)
    overlay.windowDidResize(Notification(name: NSWindow.didResizeNotification, object: panel))
    for label in [source, translation, status] {
        #expect(abs(label.preferredMaxLayoutWidth - 464) < 0.5)
    }
}

@Test @MainActor func overlayResizeHitTestEdgesCoversAllCornersEdgesAndInterior() {
    let bounds = NSRect(x: 0, y: 0, width: 600, height: 180)

    // 4 corners (cornerSize = 14)
    #expect(OverlayResizeHandleView.hitTestEdges(at: NSPoint(x: 4, y: 176), in: bounds) == .topLeft)
    #expect(OverlayResizeHandleView.hitTestEdges(at: NSPoint(x: 12, y: 168), in: bounds) == .topLeft)
    #expect(OverlayResizeHandleView.hitTestEdges(at: NSPoint(x: 596, y: 176), in: bounds) == .topRight)
    #expect(OverlayResizeHandleView.hitTestEdges(at: NSPoint(x: 588, y: 168), in: bounds) == .topRight)
    #expect(OverlayResizeHandleView.hitTestEdges(at: NSPoint(x: 4, y: 4), in: bounds) == .bottomLeft)
    #expect(OverlayResizeHandleView.hitTestEdges(at: NSPoint(x: 12, y: 12), in: bounds) == .bottomLeft)
    #expect(OverlayResizeHandleView.hitTestEdges(at: NSPoint(x: 596, y: 4), in: bounds) == .bottomRight)
    #expect(OverlayResizeHandleView.hitTestEdges(at: NSPoint(x: 588, y: 12), in: bounds) == .bottomRight)

    // 4 edges (edgeThickness = 8, outside cornerSize = 14)
    #expect(OverlayResizeHandleView.hitTestEdges(at: NSPoint(x: 4, y: 90), in: bounds) == .left)
    #expect(OverlayResizeHandleView.hitTestEdges(at: NSPoint(x: 596, y: 90), in: bounds) == .right)
    #expect(OverlayResizeHandleView.hitTestEdges(at: NSPoint(x: 300, y: 4), in: bounds) == .bottom)
    #expect(OverlayResizeHandleView.hitTestEdges(at: NSPoint(x: 300, y: 176), in: bounds) == .top)

    // Interior & out-of-bounds / degenerate bounds
    #expect(OverlayResizeHandleView.hitTestEdges(at: NSPoint(x: 300, y: 90), in: bounds) == [])
    #expect(OverlayResizeHandleView.hitTestEdges(at: NSPoint(x: 20, y: 12), in: bounds) == [])
    #expect(OverlayResizeHandleView.hitTestEdges(at: NSPoint(x: -1, y: 90), in: bounds) == [])
    #expect(OverlayResizeHandleView.hitTestEdges(at: NSPoint(x: 0, y: 0), in: .zero) == [])
}

// swiftlint:disable:next function_body_length
@Test @MainActor func overlayResizedFrameSupportsAllEightDirectionsAndClamping() {
    let initial = NSRect(x: 400, y: 200, width: 600, height: 180)
    let minSize = NSSize(width: 320, height: 100)
    let maxSize = NSSize(width: 3_840, height: 1_200)
    let visibleBounds = NSRect(x: 100, y: 50, width: 1_440, height: 900)

    // 4 single edges
    let left = OverlayResizeHandleView.resizedFrame(
        from: initial,
        edges: .left,
        delta: NSPoint(x: -50, y: 20),
        minSize: minSize,
        maxSize: maxSize,
        visibleBounds: visibleBounds
    )
    #expect(left == NSRect(x: 350, y: 200, width: 650, height: 180))
    #expect(left.maxX == initial.maxX)

    let right = OverlayResizeHandleView.resizedFrame(
        from: initial,
        edges: .right,
        delta: NSPoint(x: 80, y: -30),
        minSize: minSize,
        maxSize: maxSize,
        visibleBounds: visibleBounds
    )
    #expect(right == NSRect(x: 400, y: 200, width: 680, height: 180))
    #expect(right.minX == initial.minX)

    let bottom = OverlayResizeHandleView.resizedFrame(
        from: initial,
        edges: .bottom,
        delta: NSPoint(x: 40, y: -40),
        minSize: minSize,
        maxSize: maxSize,
        visibleBounds: visibleBounds
    )
    #expect(bottom == NSRect(x: 400, y: 160, width: 600, height: 220))
    #expect(bottom.maxY == initial.maxY)

    let top = OverlayResizeHandleView.resizedFrame(
        from: initial,
        edges: .top,
        delta: NSPoint(x: -40, y: 70),
        minSize: minSize,
        maxSize: maxSize,
        visibleBounds: visibleBounds
    )
    #expect(top == NSRect(x: 400, y: 200, width: 600, height: 250))
    #expect(top.minY == initial.minY)

    // 4 corners
    let topLeft = OverlayResizeHandleView.resizedFrame(
        from: initial,
        edges: .topLeft,
        delta: NSPoint(x: -40, y: 60),
        minSize: minSize,
        maxSize: maxSize,
        visibleBounds: visibleBounds
    )
    #expect(topLeft == NSRect(x: 360, y: 200, width: 640, height: 240))
    #expect(topLeft.maxX == initial.maxX && topLeft.minY == initial.minY)

    let topRight = OverlayResizeHandleView.resizedFrame(
        from: initial,
        edges: .topRight,
        delta: NSPoint(x: 40, y: 60),
        minSize: minSize,
        maxSize: maxSize,
        visibleBounds: visibleBounds
    )
    #expect(topRight == NSRect(x: 400, y: 200, width: 640, height: 240))
    #expect(topRight.minX == initial.minX && topRight.minY == initial.minY)

    let bottomLeft = OverlayResizeHandleView.resizedFrame(
        from: initial,
        edges: .bottomLeft,
        delta: NSPoint(x: -40, y: -30),
        minSize: minSize,
        maxSize: maxSize,
        visibleBounds: visibleBounds
    )
    #expect(bottomLeft == NSRect(x: 360, y: 170, width: 640, height: 210))
    #expect(bottomLeft.maxX == initial.maxX && bottomLeft.maxY == initial.maxY)

    let bottomRight = OverlayResizeHandleView.resizedFrame(
        from: initial,
        edges: .bottomRight,
        delta: NSPoint(x: 40, y: -30),
        minSize: minSize,
        maxSize: maxSize,
        visibleBounds: visibleBounds
    )
    #expect(bottomRight == NSRect(x: 400, y: 170, width: 640, height: 210))
    #expect(bottomRight.minX == initial.minX && bottomRight.maxY == initial.maxY)

    // minSize clamping keeps opposite corner anchored
    let clampedMin = OverlayResizeHandleView.resizedFrame(
        from: initial,
        edges: .topLeft,
        delta: NSPoint(x: 500, y: -500),
        minSize: minSize,
        maxSize: maxSize,
        visibleBounds: visibleBounds
    )
    #expect(clampedMin.size == minSize)
    #expect(clampedMin.maxX == initial.maxX)
    #expect(clampedMin.minY == initial.minY)

    // maxSize clamping when visibleBounds is larger than maxSize
    let wideBounds = NSRect(x: -5_000, y: -5_000, width: 10_000, height: 10_000)
    let clampedMax = OverlayResizeHandleView.resizedFrame(
        from: initial,
        edges: .bottomRight,
        delta: NSPoint(x: 5_000, y: -5_000),
        minSize: minSize,
        maxSize: maxSize,
        visibleBounds: wideBounds
    )
    #expect(clampedMax.size == maxSize)
    #expect(clampedMax.minX == initial.minX)
    #expect(clampedMax.maxY == initial.maxY)

    // visibleBounds clamping stops moved edges at display bounds
    let clampedBounds = OverlayResizeHandleView.resizedFrame(
        from: initial,
        edges: .bottomLeft,
        delta: NSPoint(x: -1_000, y: -1_000),
        minSize: minSize,
        maxSize: maxSize,
        visibleBounds: visibleBounds
    )
    #expect(clampedBounds.minX == visibleBounds.minX)
    #expect(clampedBounds.minY == visibleBounds.minY)
    #expect(clampedBounds.maxX == initial.maxX)
    #expect(clampedBounds.maxY == initial.maxY)

    // Empty edges returns initialFrame untouched
    #expect(OverlayResizeHandleView.resizedFrame(
        from: initial,
        edges: [],
        delta: NSPoint(x: 100, y: 100),
        minSize: minSize,
        maxSize: maxSize,
        visibleBounds: visibleBounds
    ) == initial)
}

@Test @MainActor func subtitleOverlayResizeHandleHitTestingLabelsAppearanceAndPersistence() throws {
    let domain = "com.daydalek.Maltoria.overlay.resize.test.\(UUID().uuidString)"
    defer { UserDefaults(suiteName: domain)?.removePersistentDomain(forName: domain) }
    let overlay = MacOSSubtitleOverlay(domain: domain)
    defer {
        overlay.hide()
        overlay.invalidate()
    }

    _ = try overlay.call("translation.overlay.configure", params: [
        "visible": .bool(true),
        "clickThrough": .bool(false),
        "width": .number(600),
        "height": .number(180)
    ])

    let panel = try #require(overlay.panelForTesting)
    let content = try #require(panel.contentView)
    let handle = try #require(overlay.resizeHandleForTesting)
    overlay.update(.object([
        "source": .string("原文テキスト"),
        "translation": .string("Translated subtitle text"),
        "status": .string("completed")
    ]))
    content.layoutSubtreeIfNeeded()

    #expect(panel.appearance?.name == .darkAqua)
    #expect(content.mouseDownCanMoveWindow)
    #expect(content.acceptsFirstMouse(for: nil))
    #expect(!handle.mouseDownCanMoveWindow)
    #expect(handle.acceptsFirstMouse(for: nil))
    #expect(handle.isResizingEnabled)
    #expect(!handle.isHidden)

    let source = try #require(overlay.sourceLabelForTesting)
    let translation = try #require(overlay.translationLabelForTesting)
    let status = try #require(overlay.statusLabelForTesting)
    #expect(!source.isHidden)
    #expect(!translation.isHidden)
    #expect(source.bounds.width > 0 && source.bounds.height > 0)
    #expect(translation.bounds.width > 0 && translation.bounds.height > 0)
    for label in [source, translation, status] {
        #expect(!label.isSelectable)
        #expect(label.mouseDownCanMoveWindow)
        #expect(label.contentCompressionResistancePriority(for: .vertical) == .defaultLow)
    }

    func hitPointInContentSuperview(from view: NSView, point: NSPoint) -> NSPoint {
        let inContent = content.convert(point, from: view)
        return content.superview?.convert(inContent, from: content) ?? inContent
    }

    // Unlocked: perimeter points hit handle, interior points (empty background + labels) hit contentView
    let bounds = handle.bounds
    let cornerPoint = NSPoint(x: bounds.maxX - 4, y: bounds.maxY - 4)
    let edgePoint = NSPoint(x: bounds.midX, y: bounds.minY + 4)
    let interiorPoint = NSPoint(x: bounds.midX, y: bounds.minY + 24)
    let sourcePoint = hitPointInContentSuperview(
        from: source,
        point: NSPoint(x: source.bounds.midX, y: source.bounds.midY)
    )
    let translationPoint = hitPointInContentSuperview(
        from: translation,
        point: NSPoint(x: translation.bounds.midX, y: translation.bounds.midY)
    )
    #expect(handle.hitTest(cornerPoint) === handle)
    #expect(handle.hitTest(edgePoint) === handle)
    #expect(handle.hitTest(interiorPoint) == nil)
    #expect(content.hitTest(hitPointInContentSuperview(from: handle, point: cornerPoint)) === handle)
    #expect(content.hitTest(hitPointInContentSuperview(from: handle, point: edgePoint)) === handle)
    #expect(content.hitTest(hitPointInContentSuperview(from: handle, point: interiorPoint)) === content)
    #expect(content.hitTest(sourcePoint) === content)
    #expect(content.hitTest(translationPoint) === content)

    // Locked: handle is hidden and passes through everywhere
    overlay.setClickThrough(true)
    #expect(!handle.isResizingEnabled)
    #expect(handle.isHidden)
    #expect(handle.hitTest(cornerPoint) == nil)
    #expect(handle.hitTest(edgePoint) == nil)
    #expect(handle.hitTest(interiorPoint) == nil)

    // Unlock again and test resizing down to minSize.height (100) with long text + persistence
    overlay.setClickThrough(false)
    overlay.update(.object([
        "source": .string(String(repeating: "長い原文テキスト ", count: 20)),
        "translation": .string(String(repeating: "Very long translated subtitle text ", count: 20)),
        "status": .string("pending")
    ]))

    let updatedFrame = NSRect(x: panel.frame.origin.x + 25, y: panel.frame.origin.y + 15, width: 420, height: 100)
    panel.setFrame(updatedFrame, display: true)
    overlay.windowDidMove(Notification(name: NSWindow.didMoveNotification, object: panel))
    overlay.windowDidResize(Notification(name: NSWindow.didResizeNotification, object: panel))

    for label in [source, translation, status] {
        #expect(abs(label.preferredMaxLayoutWidth - 384) < 0.5)
    }

    let response = try overlay.call("translation.overlay.get", params: [:])
    let persistedSettings = try #require(response.object?["settings"]).decoded(SubtitleOverlaySettings.self)
    #expect(abs(persistedSettings.width - panel.frame.width) < 0.5)
    #expect(abs(persistedSettings.height - panel.frame.height) < 0.5)
    #expect(abs((persistedSettings.x ?? -1) - panel.frame.origin.x) < 0.5)
    #expect(abs((persistedSettings.y ?? -1) - panel.frame.origin.y) < 0.5)

    let storedData = try #require(UserDefaults(suiteName: domain)?.data(forKey: "translationOverlay"))
    let storedSettings = try JSONDecoder().decode(SubtitleOverlaySettings.self, from: storedData)
    #expect(abs(storedSettings.width - panel.frame.width) < 0.5)
    #expect(abs(storedSettings.height - panel.frame.height) < 0.5)
    #expect(abs((storedSettings.x ?? -1) - panel.frame.origin.x) < 0.5)
    #expect(abs((storedSettings.y ?? -1) - panel.frame.origin.y) < 0.5)
}

@Test @MainActor func captureHUDTargetExitedHidesOverlayWithoutClobberingManualPreview() throws {
    let domain = "com.daydalek.Maltoria.hud.exited.test.\(UUID().uuidString)"
    defer { UserDefaults(suiteName: domain)?.removePersistentDomain(forName: domain) }
    let overlay = MacOSSubtitleOverlay(domain: domain)
    let hud = MacOSCaptureHUD(subtitleOverlay: overlay)
    defer {
        hud.hide()
        hud.invalidate()
        overlay.hide()
        overlay.invalidate()
    }

    var callbackStates: [Bool] = []
    overlay.onSettingsChanged = { callbackStates.append($0.visible) }

    // Manual overlay preview when idle must not be clobbered by "stopped" updates
    _ = try overlay.call("translation.overlay.configure", params: ["visible": .bool(true)])
    #expect(overlay.isVisible)
    hud.handleStateUpdate(.object(["captureState": .string("stopped")]))
    #expect(overlay.isVisible)

    // Transitioning into "targetExited" hides the visible overlay and notifies settings listeners
    hud.handleStateUpdate(.object(["captureState": .string("attached")]))
    #expect(overlay.isVisible)
    hud.handleStateUpdate(.object(["captureState": .string("targetExited")]))
    #expect(!overlay.isVisible)
    #expect(overlay.panelForTesting?.isVisible != true)
    #expect(callbackStates.last == false)

    // Subsequent manual preview must stay visible even if another "targetExited" snapshot arrives
    _ = try overlay.call("translation.overlay.configure", params: ["visible": .bool(true)])
    #expect(overlay.isVisible)
    hud.handleStateUpdate(.object(["captureState": .string("targetExited")]))
    #expect(overlay.isVisible)
}

@Test @MainActor func captureHUDHandleRuntimeEntriesEndedCleansUpMatchingEntryAndCapture() async throws {
    let domain = "com.daydalek.Maltoria.hud.ended.test.\(UUID().uuidString)"
    defer { UserDefaults(suiteName: domain)?.removePersistentDomain(forName: domain) }
    let overlay = MacOSSubtitleOverlay(domain: domain)
    let hud = MacOSCaptureHUD(subtitleOverlay: overlay)
    defer {
        hud.hide()
        hud.invalidate()
        overlay.hide()
        overlay.invalidate()
    }

    let root = FileManager.default.temporaryDirectory.appendingPathComponent("whisky-hud-ended-\(UUID())")
    try FileManager.default.createDirectory(at: root, withIntermediateDirectories: true)
    defer { try? FileManager.default.removeItem(at: root) }

    let container = root.appendingPathComponent("Bottle")
    let executable = container.appendingPathComponent("drive_c/Game/play.exe")
    try FileManager.default.createDirectory(
        at: executable.deletingLastPathComponent(),
        withIntermediateDirectories: true
    )
    try Data([0x4D, 0x5A]).write(to: executable)
    let metadata: [String: Any] = ["info": ["name": "Test", "pins": [], "blocklist": []]]
    try PropertyListSerialization.data(fromPropertyList: metadata, format: .xml, options: 0)
        .write(to: container.appendingPathComponent("Metadata.plist"))

    let workspace = ApplicationWorkspace(containers: [container])
    let translation = ApplicationTranslationService(directory: root.appendingPathComponent("Translation"))
    let capture = ApplicationCaptureService(profiles: translation, requirePayload: false) { _, _, _ in
        TranslationHelperHandle {}
    }
    let service = ApplicationService(workspace: workspace, translationService: translation)
    _ = try await service.call("programs.pin", params: [
        "container": .string(container.path),
        "executable": .string(executable.path),
        "name": .string("Game"),
        "pinned": .bool(true)
    ])
    let entryID = try #require(try await workspace.entries(in: container).first?.id)

    hud.configure(capture: capture, translation: translation, workspace: workspace)
    _ = try await hud.show(params: [
        "container": .string(container.path),
        "entryID": .string(entryID)
    ], workspace: workspace)
    overlay.setVisible(true)
    #expect(hud.isVisibleForTesting)
    #expect(overlay.isVisible)

    // Non-matching entry without stoppedCapture leaves both HUD and overlay open
    #expect(!hud.handleRuntimeEntriesEnded(["other-entry"], stoppedCapture: false))
    #expect(hud.isVisibleForTesting)
    #expect(overlay.isVisible)

    // Non-matching entry with stoppedCapture closes the overlay while leaving HUD alone
    #expect(hud.handleRuntimeEntriesEnded(["other-entry"], stoppedCapture: true))
    #expect(hud.isVisibleForTesting)
    #expect(!overlay.isVisible)

    // Matching active HUD entry closes both HUD and overlay
    overlay.setVisible(true)
    #expect(hud.handleRuntimeEntriesEnded([entryID], stoppedCapture: false))
    #expect(!hud.isVisibleForTesting)
    #expect(hud.panelForTesting?.isVisible != true)
    #expect(!overlay.isVisible)
}
