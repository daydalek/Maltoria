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
import CoreGraphics

/// Shared configuration and lifecycle helpers for non-activating overlay panels
/// that must remain visible above Wine display-captured and Space fullscreen games.
@MainActor
enum FullscreenOverlaySupport {
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
