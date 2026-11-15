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
    public var width: Double = 900
    public var height: Double = 180
    // swiftlint:disable:next identifier_name
    public var x: Double?
    // swiftlint:disable:next identifier_name
    public var y: Double?
    public init() {}

    public func validate() throws {
        guard fontSize.isFinite, (12 ... 72).contains(fontSize),
              opacity.isFinite, (0.1 ... 1).contains(opacity),
              width.isFinite, (320 ... 3_840).contains(width),
              height.isFinite, (100 ... 1_200).contains(height),
              x == nil || x!.isFinite, y == nil || y!.isFinite,
              showSource || showTranslation
        else {
            throw BridgeFailure(
                "invalid_overlay_settings",
                "Invalid subtitle size, opacity, position or text visibility"
            )
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
