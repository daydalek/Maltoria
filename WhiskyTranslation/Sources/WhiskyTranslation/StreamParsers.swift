//
//  StreamParsers.swift
//  WhiskyTranslation
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

import CoreFoundation
import Foundation

struct LineFramer {
    private var buffer = Data()

    mutating func append(_ data: Data) -> [Data] {
        buffer.append(data)
        var lines: [Data] = []
        while let newline = buffer.firstIndex(of: 0x0A) {
            let line = Data(buffer[..<newline])
            let end = buffer.index(after: newline)
            buffer.removeSubrange(buffer.startIndex ..< end)
            lines.append(Self.withoutCarriageReturn(line))
        }
        return lines
    }

    mutating func finish() -> [Data] {
        guard !buffer.isEmpty else { return [] }
        let line = Self.withoutCarriageReturn(buffer)
        buffer.removeAll(keepingCapacity: false)
        return [line]
    }

    private static func withoutCarriageReturn(_ data: Data) -> Data {
        guard data.last == 0x0D else { return data }
        return Data(data.dropLast())
    }
}

struct SSEFrame: Equatable {
    let event: String?
    let data: String
}

struct SSEParser {
    private var framer = LineFramer()
    private var eventName: String?
    private var dataLines: [String] = []

    mutating func append(_ data: Data) throws -> [SSEFrame] {
        var frames: [SSEFrame] = []
        for line in framer.append(data) {
            try frames.append(contentsOf: consume(line))
        }
        return frames
    }

    mutating func finish() throws -> [SSEFrame] {
        var frames: [SSEFrame] = []
        for line in framer.finish() {
            try frames.append(contentsOf: consume(line))
        }
        if let frame = try dispatch() { frames.append(frame) }
        return frames
    }

    private mutating func consume(_ lineData: Data) throws -> [SSEFrame] {
        guard let line = String(data: lineData, encoding: .utf8) else { throw TranslationError.invalidUTF8 }
        if line.isEmpty {
            return try dispatch().map { [$0] } ?? []
        }
        if line.first == ":" { return [] }
        let separator = line.firstIndex(of: ":")
        let field = separator.map { String(line[..<$0]) } ?? line
        let value: String
        if let separator {
            var start = line.index(after: separator)
            if start < line.endIndex, line[start] == " " { start = line.index(after: start) }
            value = String(line[start...])
        } else {
            value = ""
        }
        switch field {
        case "event": eventName = value
        case "data": dataLines.append(value)
        default: break
        }
        return []
    }

    private mutating func dispatch() throws -> SSEFrame? {
        guard !dataLines.isEmpty || eventName != nil else { return nil }
        let frame = SSEFrame(event: eventName, data: dataLines.joined(separator: "\n"))
        eventName = nil
        dataLines.removeAll(keepingCapacity: true)
        return frame
    }
}

struct NDJSONParser {
    private var framer = LineFramer()

    mutating func append(_ data: Data) -> [Data] { framer.append(data).filter { !$0.isEmpty } }
    mutating func finish() -> [Data] { framer.finish().filter { !$0.isEmpty } }
}

func jsonDictionary(_ data: Data) throws -> [String: Any] {
    let value: Any
    do { value = try JSONSerialization.jsonObject(with: data, options: [.fragmentsAllowed]) } catch {
        throw TranslationError.malformedResponse
    }
    guard let object = value as? [String: Any] else {
        throw TranslationError.malformedResponse
    }
    return object
}

func requiredDictionaryArray(_ root: [String: Any], key: String) throws -> [[String: Any]] {
    guard let values = root[key] as? [[String: Any]] else { throw TranslationError.malformedResponse }
    return values
}

func positiveInteger(_ value: Any?, maximum: Int = 1_000_000_000) -> Int? {
    guard let number = value as? NSNumber,
          CFGetTypeID(number) != CFBooleanGetTypeID(),
          !["d", "f"].contains(String(cString: number.objCType))
    else { return nil }
    let integer = number.int64Value
    guard integer > 0, integer <= Int64(maximum) else { return nil }
    return Int(integer)
}
