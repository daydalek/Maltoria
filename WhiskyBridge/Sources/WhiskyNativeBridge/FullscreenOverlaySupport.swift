//
//  FullscreenOverlaySupport.swift
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
import Carbon.HIToolbox
import CoreGraphics

private let mouseReleaseHotKeySignature: OSType = 0x4D4C_5452 // 'MLTR'
private let mouseReleaseHotKeyID: UInt32 = 1

private func handleMouseReleaseHotKeyEvent(
    _: EventHandlerCallRef?,
    event: EventRef?,
    _: UnsafeMutableRawPointer?
) -> OSStatus {
    guard let event else { return OSStatus(eventNotHandledErr) }
    var hotKeyID = EventHotKeyID()
    let status = GetEventParameter(
        event,
        EventParamName(kEventParamDirectObject),
        EventParamType(typeEventHotKeyID),
        nil,
        MemoryLayout<EventHotKeyID>.size,
        nil,
        &hotKeyID
    )
    guard status == noErr,
          hotKeyID.signature == mouseReleaseHotKeySignature,
          hotKeyID.id == mouseReleaseHotKeyID
    else {
        return OSStatus(eventNotHandledErr)
    }
    Task { @MainActor in
        FullscreenOverlaySupport.releaseWineMouseCapture(container: nil, resyncWindow: true)
    }
    return noErr
}

/// Registers a global Ctrl+Option+M shortcut via Carbon EventHotKey (which requires
/// no Accessibility permission) so users can always ungrab a trapped cursor and
/// resynchronize fullscreen/windowed coordinates mid-game.
@MainActor
final class GlobalMouseReleaseHotKey {
    static let shared = GlobalMouseReleaseHotKey()

    private var eventHandlerRef: EventHandlerRef?
    private var hotKeyRef: EventHotKeyRef?

    private init() {}

    func installIfNeeded() {
        guard hotKeyRef == nil else { return }
        var eventSpec = EventTypeSpec(
            eventClass: OSType(kEventClassKeyboard),
            eventKind: UInt32(kEventHotKeyPressed)
        )
        let installStatus = InstallEventHandler(
            GetApplicationEventTarget(),
            handleMouseReleaseHotKeyEvent,
            1,
            &eventSpec,
            nil,
            &eventHandlerRef
        )
        guard installStatus == noErr else { return }

        let hotKeyID = EventHotKeyID(signature: mouseReleaseHotKeySignature, id: mouseReleaseHotKeyID)
        let registerStatus = RegisterEventHotKey(
            UInt32(kVK_ANSI_M),
            UInt32(controlKey | optionKey),
            hotKeyID,
            GetApplicationEventTarget(),
            0,
            &hotKeyRef
        )
        if registerStatus != noErr, let handler = eventHandlerRef {
            RemoveEventHandler(handler)
            eventHandlerRef = nil
        }
    }
}

/// Shared configuration and lifecycle helpers for non-activating overlay panels
/// that must remain visible above Wine display-captured and Space fullscreen games.
@MainActor
enum FullscreenOverlaySupport {
    static let releaseMouseNotification = Notification.Name("WineExternalReleaseMouseCaptureNotification")

    /// Broadcasts an immediate mouse-capture release (and optional window-geometry
    /// resynchronization) to running Wine processes.
    static func releaseWineMouseCapture(container: URL? = nil, resyncWindow: Bool = false) {
        let userInfo: [String: Any] = [
            "WinePrefix": container?.path ?? "",
            "ResyncWindow": resyncWindow
        ]
        DistributedNotificationCenter.default().postNotificationName(
            releaseMouseNotification,
            object: nil,
            userInfo: userInfo,
            deliverImmediately: true
        )
    }
    /// Wine's `winemac.drv` captures displays with `CGShieldingWindowLevel()` and places
    /// fullscreen windows at `CGShieldingWindowLevel() + 1` or `+ 2`. Using the maximum
    /// WindowServer level keeps our subtitle and capture HUD panels above them without
    /// calling `isFloatingPanel = true` (which AppKit resets to `.floating`).
    static var windowLevel: NSWindow.Level {
        let level = max(
            Int(CGShieldingWindowLevel()) + 3,
            Int(CGWindowLevelForKey(.maximumWindow))
        )
        return NSWindow.Level(rawValue: level)
    }

    static let collectionBehavior: NSWindow.CollectionBehavior = [
        .canJoinAllSpaces,
        .fullScreenAuxiliary,
        .ignoresCycle
    ]

    static func configurePanel(_ panel: NSPanel, title: String, delegate: any NSWindowDelegate) {
        panel.isFloatingPanel = false
        panel.level = windowLevel
        panel.hidesOnDeactivate = false
        panel.isOpaque = false
        panel.backgroundColor = .clear
        panel.hasShadow = true
        panel.isReleasedWhenClosed = false
        panel.isMovableByWindowBackground = true
        panel.collectionBehavior = collectionBehavior
        panel.title = title
        panel.delegate = delegate
    }

    static func applyOrderAndLevel(_ panel: NSPanel) {
        panel.level = windowLevel
        panel.collectionBehavior = collectionBehavior
        panel.orderFrontRegardless()
    }
}

/// Manages workspace Space/activation and display-parameter observers for overlay panels.
@MainActor
final class OverlayReassertObserver {
    private nonisolated(unsafe) var workspaceTokens: [any NSObjectProtocol] = []
    private nonisolated(unsafe) var defaultTokens: [any NSObjectProtocol] = []

    init(onReassert: @escaping @MainActor () -> Void) {
        let workspaceCenter = NSWorkspace.shared.notificationCenter
        let workspaceNames: [Notification.Name] = [
            NSWorkspace.activeSpaceDidChangeNotification,
            NSWorkspace.didActivateApplicationNotification
        ]
        workspaceTokens = workspaceNames.map { name in
            workspaceCenter.addObserver(forName: name, object: nil, queue: .main) { _ in
                Task { @MainActor in onReassert() }
            }
        }

        defaultTokens = [
            NotificationCenter.default.addObserver(
                forName: NSApplication.didChangeScreenParametersNotification,
                object: nil,
                queue: .main
            ) { _ in
                Task { @MainActor in onReassert() }
            }
        ]
    }

    deinit {
        let workspaceCenter = NSWorkspace.shared.notificationCenter
        for token in workspaceTokens {
            workspaceCenter.removeObserver(token)
        }
        for token in defaultTokens {
            NotificationCenter.default.removeObserver(token)
        }
    }

    func invalidate() {
        let workspaceCenter = NSWorkspace.shared.notificationCenter
        for token in workspaceTokens {
            workspaceCenter.removeObserver(token)
        }
        workspaceTokens.removeAll()
        for token in defaultTokens {
            NotificationCenter.default.removeObserver(token)
        }
        defaultTokens.removeAll()
    }
}
