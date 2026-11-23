//
//  CapturePopupStrings.swift
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

import Foundation
import WhiskyBridgeProtocol

@MainActor
final class CapturePopupStrings {
    private var values: [String: String] = [:]
    private static let fallback: [String: String] = [
        "hud_title": "Game Translation",
        "overlay_title": "Native Subtitle Overlay",
        "capture_unavailable": "Capture service unavailable",
        "no_provider": "No translation provider configured; configure one in Maltoria first",
        "pick_provider": "Select a translation provider above before scanning",
        "pick_game": "Select a game from the menu before starting capture.",
        "preset_target_missing": "No game process matches the preset; attach manually or refresh",
        "attached": "Attached",
        "standby": "Standby",
        "expand": "Expand",
        "collapse": "Collapse",
        "close": "Close",
        "subtitle_overlay": "Subtitle Overlay",
        "sub_on": "Subs On",
        "sub_off": "Subs Off",
        "draggable": "Draggable (Unlocked)",
        "locked": "Locked",
        "refresh": "Refresh",
        "waiting_game": "Waiting for game…",
        "error": "Error",
        "pick_process": "Pick Process",
        "connecting": "Connecting…",
        "attached_target": "Attached · {exe}",
        "standby_scan": "Standby",
        "architecture_label": "Architecture: {arch}",
        "waiting_launch_detail": "Waiting for the game process…\nScanning will start automatically in 1 second.",
        "attaching_detail": "Attaching to the game process…",
        "starting_helper_detail": "Starting the capture helper…",
        "starting_label": "Starting…",
        "start_scan": "Start Scan",
        "preset_attach_label": "Auto-attach Preset",
        "preset_attach_target": "Auto-attach Preset ({exe})",
        "process_empty_detail": "No attachable game processes found. Wait for the game window to open, then click Refresh.",
        "process_metadata": "{arch} · Process ID {pid}",
        "attach": "Attach Selected",
        "game_process": "Game process",
        "detach": "Detach",
        "collapse_ball": "Collapse to Floating Ball",
        "waiting_text": "Waiting for game text output…",
        "thread": "Thread",
        "empty_preview": "(No preview text)",
        "role_body": "Body",
        "role_speaker": "Speaker",
        "role_choice": "Choice",
        "role_ignore": "Ignore",
        "translating": "Translating…",
        "translation_failed": "Translation failed",
        "translation_cancelled": "Translation cancelled"
    ]

    func update(_ strings: [String: JSONValue]) {
        values = strings.compactMapValues { value in
            guard let text = value.string, !text.isEmpty else { return nil }
            return text
        }
    }

    func text(_ id: String, _ arguments: [String: String] = [:]) -> String {
        let template = values[id] ?? Self.fallback[id] ?? id
        var result = ""
        var remainder = template[...]
        while let open = remainder.firstIndex(of: "{"),
              let close = remainder[open...].firstIndex(of: "}") {
            result += String(remainder[..<open])
            let name = String(remainder[remainder.index(after: open) ..< close])
            result += arguments[name] ?? String(remainder[open ... close])
            remainder = remainder[remainder.index(after: close)...]
        }
        return result + String(remainder)
    }

    func errorText(_ error: String) -> String {
        values["error:" + error] ?? error
    }
}
