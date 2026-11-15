//
//  HookIPCTests.swift
//  WhiskyBridgeTests
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
import Testing
@testable import WhiskyApplication
import WhiskyBridgeProtocol

@Test(arguments: ["valid", "bad-token", "wrong-session"])
func hookIPCAuthenticatesAndCorrelatesResponses(mode: String) async throws {
    let bootstrap = try HookIPCClient.make(session: UUID().uuidString, architecture: "x64")
    let process = Process()
    process.executableURL = URL(fileURLWithPath: "/usr/bin/python3")
    process.arguments = ["-c", #"""
    import json, os, socket
    s = socket.create_connection(('127.0.0.1', int(os.environ['MALTORIA_HOOK_PORT'])), timeout=5)
    s.settimeout(5)
    session = os.environ['MALTORIA_HOOK_SESSION']
    mode = os.environ['HOOK_TEST_MODE']
    token = os.environ['MALTORIA_HOOK_TOKEN'] if mode != 'bad-token' else 'wrong-token'
    hello = dict(type='hello', version=1, session=session, token=token, arch='x64')
    s.sendall((json.dumps(hello) + '\n').encode())
    f = s.makefile('rb')
    ack = f.readline()
    if mode == 'bad-token':
        assert not ack
    else:
        assert json.loads(ack)['ok']
        command = json.loads(f.readline())
        assert command['command'] == 'ping' and command['session'] == session
        response = dict(type='response', id=command['id'], session=session if mode == 'valid' else 'other', ok=True, result={})
        s.sendall((json.dumps(response) + '\n').encode())
    s.close()
    """#]
    process.environment = ProcessInfo.processInfo.environment.merging(bootstrap.environment) { _, new in new }
    process.environment?["HOOK_TEST_MODE"] = mode
    process.standardOutput = FileHandle.nullDevice
    process.standardError = FileHandle.nullDevice
    try process.run()
    if mode == "bad-token" {
        await #expect(throws: BridgeFailure.self) { try await bootstrap.client.ready() }
    } else {
        try await bootstrap.client.ready()
        if mode == "valid" {
            #expect(try await bootstrap.client.request("ping", params: [:]) == .object([:]))
        } else {
            await #expect(throws: BridgeFailure.self) { try await bootstrap.client.request("ping", params: [:]) }
        }
    }
    await bootstrap.client.close()
    await Task.detached { process.waitUntilExit() }.value
    #expect(process.terminationStatus == 0)
}
