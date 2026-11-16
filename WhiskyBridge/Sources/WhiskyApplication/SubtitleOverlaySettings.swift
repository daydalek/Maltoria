//
//  SubtitleOverlaySettings.swift
//  WhiskyApplication
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
import WhiskyBridgeProtocol

public struct SubtitleOverlaySettings: Codable, Equatable, Sendable {
    public var visible = false
    public var clickThrough = true
    public var showSource = true
    public var showTranslation = true
    public var fontSize: Double = 28
    public var opacity: Double = 0.85
    public var textColor: String = "#FFFFFF"
    public var sourceTextColor: String = "#D4D4D8"
    public var width: Double = 900
    public var height: Double = 180
    // swiftlint:disable:next identifier_name
    public var x: Double?
    // swiftlint:disable:next identifier_name
    public var y: Double?

    public init() {}

    private enum CodingKeys: String, CodingKey {
        case visible, clickThrough, showSource, showTranslation, fontSize, opacity,
             // swiftlint:disable:next identifier_name
             textColor, sourceTextColor, width, height, x, y
    }

    public init(from decoder: Decoder) throws {
        let values = try decoder.container(keyedBy: CodingKeys.self)
        visible = try values.decodeIfPresent(Bool.self, forKey: .visible) ?? false
        clickThrough = try values.decodeIfPresent(Bool.self, forKey: .clickThrough) ?? true
        showSource = try values.decodeIfPresent(Bool.self, forKey: .showSource) ?? true
        showTranslation = try values.decodeIfPresent(Bool.self, forKey: .showTranslation) ?? true
        fontSize = try values.decodeIfPresent(Double.self, forKey: .fontSize) ?? 28
        opacity = try values.decodeIfPresent(Double.self, forKey: .opacity) ?? 0.85
        textColor = try values.decodeIfPresent(String.self, forKey: .textColor) ?? "#FFFFFF"
        sourceTextColor = try values.decodeIfPresent(String.self, forKey: .sourceTextColor) ?? "#D4D4D8"
        width = try values.decodeIfPresent(Double.self, forKey: .width) ?? 900
        height = try values.decodeIfPresent(Double.self, forKey: .height) ?? 180
        x = try values.decodeIfPresent(Double.self, forKey: .x)
        y = try values.decodeIfPresent(Double.self, forKey: .y)
    }

    public func validate() throws {
        guard fontSize.isFinite, (12 ... 72).contains(fontSize),
              opacity.isFinite, (0.0 ... 1.0).contains(opacity),
              Self.isValidHexColor(textColor),
              Self.isValidHexColor(sourceTextColor),
              width.isFinite, (320 ... 3_840).contains(width),
              height.isFinite, (100 ... 1_200).contains(height),
              x == nil || x!.isFinite, y == nil || y!.isFinite,
              showSource || showTranslation
        else {
            throw BridgeFailure(
                "invalid_overlay_settings",
                "Invalid subtitle size, opacity, color, position or text visibility"
            )
        }
    }

    private static func isValidHexColor(_ value: String) -> Bool {
        guard value.count == 7, value.first == "#" else { return false }
        return value.dropFirst().unicodeScalars.allSatisfy { scalar in
            switch scalar.value {
            case 48 ... 57, 65 ... 70, 97 ... 102: true
            default: false
            }
        }
    }

    public func merging(_ changes: [String: JSONValue]) throws -> Self {
        var object = try JSONValue.encoded(self).object!
        for (key, value) in changes {
            object[key] = value
        }
        let next = try JSONValue.object(object).decoded(Self.self)
        try next.validate()
        return next
    }
}
