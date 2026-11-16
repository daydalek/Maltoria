//
//  LinuxWineProcessHost.swift
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

import Foundation
import WhiskyBridgeProtocol

/// `ProcessSnapshotProviding` for a Linux Wine runtime, using the runtime's
/// built-in `tasklist.exe /FO CSV /NH` (or `winedbg info proc`) and `taskkill.exe`, guarding the
/// query behind `wineserver -k0` so polling an idle bottle never boots a
/// server or exceeds the frontend's call timeout.
@MainActor
public final class LinuxWineProcessHost: ProcessSnapshotProviding {
    private let resolveHost: @MainActor (URL) throws -> LinuxWineHost

    private nonisolated static let services: Set<String> = [
        "services.exe", "winedevice.exe", "plugplay.exe", "svchost.exe",
        "rpcss.exe", "tabtip.exe", "conhost.exe"
    ]
    private nonisolated static let system: Set<String> = [
        "explorer.exe", "start.exe", "csrss.exe", "wininit.exe",
        "winlogon.exe", "lsass.exe", "smss.exe"
    ]

    public init(host: LinuxWineHost) {
        resolveHost = { _ in host }
    }

    public init(resolveHost: @escaping @MainActor (URL) throws -> LinuxWineHost) {
        self.resolveHost = resolveHost
    }

    public convenience init(engineStore: LinuxWineEngineStore) {
        self.init { container in
            try engineStore.resolveHost(for: container).host
        }
    }

    public func listProcesses(in container: URL) async throws -> [ProcessRecord] {
        let host = try resolveHost(container)
        guard try await host.probeServer(prefix: container) else { return [] }
        let result = try await host.run(
            host.loader,
            arguments: ["tasklist.exe", "/FO", "CSV", "/NH"],
            prefix: container,
            environment: ["WINEDEBUG": "-all"],
            timeout: .seconds(4)
        )
        try Self.check(result, operation: "Query Wine processes")
        if !result.output.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty {
            return Self.parseTasklist(result.output)
        }
        let debug = try await host.run(
            host.loader,
            arguments: ["winedbg", "--command", "info proc"],
            prefix: container,
            environment: ["WINEDEBUG": "-all"],
            timeout: .seconds(4)
        )
        try Self.check(debug, operation: "Query Wine debugger processes")
        return try Self.parseWineDebug(debug.output)
    }

    public func terminateProcess(in container: URL, pid: Int32, force: Bool) async throws {
        let host = try resolveHost(container)
        var arguments = ["taskkill.exe", "/PID", String(pid)]
        if force {
            arguments.append("/F")
        }
        let result = try await host.run(
            host.loader,
            arguments: arguments,
            prefix: container,
            environment: ["WINEDEBUG": "-all"],
            timeout: .seconds(4)
        )
        try Self.check(result, operation: "Terminate Wine process \(pid)")
    }

    public func terminateAllProcesses(in container: URL) async throws {
        let host = try resolveHost(container)
        let result = try await host.run(
            host.wineserver,
            arguments: ["-k"],
            prefix: container,
            timeout: .seconds(5)
        )
        try Self.check(result, operation: "Terminate container processes")
        let waited = try await host.run(host.wineserver, arguments: ["-w"], prefix: container, timeout: .seconds(5))
        try Self.check(waited, operation: "Wait for container processes to exit")
    }

    nonisolated static func check(_ result: LinuxWineHost.Output, operation: String) throws {
        guard !result.timedOut, result.status == 0 else {
            throw BridgeFailure(
                "operation_failed",
                "\(operation) failed (exit \(result.status), timeout \(result.timedOut)): \(result.output.suffix(500))"
            )
        }
    }

    /// Parses quoted CSV rows produced by `tasklist.exe /FO CSV`.
    public nonisolated static func parseTasklist(_ output: String) -> [ProcessRecord] {
        var records: [ProcessRecord] = []
        for line in output.split(omittingEmptySubsequences: true, whereSeparator: \.isNewline) {
            let fields = csvFields(String(line))
            guard fields.count >= 5, fields[1] != "PID", let pid = Int32(fields[1]) else { continue }
            let name = fields[0]
            let lower = name.lowercased()
            guard lower != "tasklist.exe" else { continue }
            let kind = services.contains(lower) ? "service" : system.contains(lower) ? "system" : "app"
            records.append(ProcessRecord(
                pid: pid,
                name: name,
                memory: fields[4],
                kind: kind,
                source: "untracked"
            ))
        }
        return records
    }

    public nonisolated static func parseWineDebug(_ output: String) throws -> [ProcessRecord] {
        let lines = output.split(omittingEmptySubsequences: true, whereSeparator: \.isNewline)
        guard let header = lines.first, header.contains("pid"), header.contains("threads"),
              header.contains("executable"), header.contains("hex")
        else {
            throw BridgeFailure("process_query_failed", "Wine did not return a process list")
        }
        return try lines.dropFirst().compactMap { line in
            let fields = line.split(maxSplits: 2, whereSeparator: \.isWhitespace)
            guard fields.count == 3,
                  let pid = Int32(fields[0].drop(while: { $0 == "=" }), radix: 16), pid > 0,
                  Int(fields[1]) != nil,
                  let start = fields[2].firstIndex(of: "'"),
                  let end = fields[2].lastIndex(of: "'"), start < end
            else {
                throw BridgeFailure("process_query_failed", "Wine returned an invalid process record")
            }
            let name = String(fields[2][fields[2].index(after: start) ..< end])
            guard !name.isEmpty else {
                throw BridgeFailure("process_query_failed", "Wine returned an empty process name")
            }
            let lower = name.lowercased()
            guard lower != "winedbg.exe", lower != "tasklist.exe" else { return nil }
            let kind = services.contains(lower) ? "service" : system.contains(lower) ? "system" : "app"
            return ProcessRecord(pid: pid, name: name, memory: "", kind: kind, source: "untracked")
        }
    }

    private nonisolated static func csvFields(_ line: String) -> [String] {
        var fields: [String] = []
        var field = ""
        var quoted = false
        var index = line.startIndex
        while index < line.endIndex {
            let character = line[index]
            let next = line.index(after: index)
            if character == "\"" {
                if quoted, next < line.endIndex, line[next] == "\"" {
                    field.append(character)
                    index = line.index(after: next)
                    continue
                }
                quoted.toggle()
            } else if character == ",", !quoted {
                fields.append(field.trimmingCharacters(in: .whitespacesAndNewlines))
                field = ""
            } else {
                field.append(character)
            }
            index = next
        }
        guard !quoted else { return [] }
        fields.append(field.trimmingCharacters(in: .whitespacesAndNewlines))
        return fields
    }
}
