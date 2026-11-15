//
//  HookIPC.swift
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
#if canImport(Darwin)
import Darwin
#else
import Glibc
#endif

public protocol HookClient: Sendable {
    func ready() async throws
    func request(_ command: String, params: [String: JSONValue]) async throws -> JSONValue
    func events() async -> AsyncThrowingStream<JSONValue, any Error>
    func close() async
}

public struct HookClientBootstrap: Sendable {
    public let client: any HookClient
    public let environment: [String: String]
    public init(client: any HookClient, environment: [String: String]) {
        self.client = client
        self.environment = environment
    }
}

struct HookFrameDecoder {
    static let maximum = 64 * 1_024
    private var pending = Data()

    mutating func append(_ data: Data) throws -> [JSONValue] {
        pending.append(data)
        var values: [JSONValue] = []
        while let newline = pending.firstIndex(of: 10) {
            let count = pending.distance(from: pending.startIndex, to: newline) + 1
            guard count <= Self.maximum else { throw hookFailure("frame_too_large") }
            var line = Data(pending.prefix(count - 1))
            pending.removeFirst(count)
            if line.last == 13 { line.removeLast() }
            guard !line.isEmpty, String(data: line, encoding: .utf8) != nil,
                  let value = try? JSONDecoder().decode(JSONValue.self, from: line), value.object != nil
            else { throw hookFailure("invalid_frame") }
            values.append(value)
        }
        guard pending.count < Self.maximum else { throw hookFailure("frame_too_large") }
        return values
    }

    func finish() throws {
        if !pending.isEmpty { throw hookFailure("truncated_frame") }
    }

    static func encode(_ value: JSONValue) throws -> Data {
        var data = try JSONEncoder().encode(value)
        data.append(10)
        guard data.count <= maximum else { throw hookFailure("frame_too_large") }
        return data
    }
}

func hookFailure(_ code: String) -> BridgeFailure {
    BridgeFailure("capture_" + code, "Text capture: " + code)
}

/// File descriptors are guarded by a lock; blocking I/O runs on private queues.
/// stop() shuts down descriptors to wake reads; the reader alone closes them.
private final class HookSocket: @unchecked Sendable {
    let port: UInt16
    let frames: AsyncThrowingStream<JSONValue, any Error>
    private let continuation: AsyncThrowingStream<JSONValue, any Error>.Continuation
    private let lock = NSLock()
    private let writes = DispatchQueue(label: "maltoria.hook.writes")
    private var listener: Int32
    private var peer: Int32 = -1
    private var stopped = false

    init() throws {
        #if canImport(Darwin)
        let descriptor = socket(AF_INET, SOCK_STREAM, 0)
        #else
        let descriptor = socket(AF_INET, Int32(SOCK_STREAM.rawValue), 0)
        #endif
        guard descriptor >= 0 else { throw hookFailure("socket_unavailable") }
        var address = sockaddr_in()
        address.sin_family = sa_family_t(AF_INET)
        address.sin_addr.s_addr = UInt32(0x7F00_0001).bigEndian
        #if canImport(Darwin)
        address.sin_len = UInt8(MemoryLayout<sockaddr_in>.size)
        #endif
        var length = socklen_t(MemoryLayout<sockaddr_in>.size)
        let bound = withUnsafePointer(to: &address) {
            $0.withMemoryRebound(to: sockaddr.self, capacity: 1) { bind(descriptor, $0, length) }
        }
        guard bound == 0, listen(descriptor, 1) == 0 else {
            systemClose(descriptor)
            throw hookFailure("socket_unavailable")
        }
        let named = withUnsafeMutablePointer(to: &address) {
            $0.withMemoryRebound(to: sockaddr.self, capacity: 1) { getsockname(descriptor, $0, &length) }
        }
        guard named == 0 else { systemClose(descriptor)
            throw hookFailure("socket_unavailable")
        }
        port = UInt16(bigEndian: address.sin_port)
        listener = descriptor
        _ = fcntl(descriptor, F_SETFD, FD_CLOEXEC)
        _ = fcntl(descriptor, F_SETFL, O_NONBLOCK)
        let stream = AsyncThrowingStream<JSONValue, any Error>.makeStream(bufferingPolicy: .bufferingOldest(2048))
        frames = stream.stream
        continuation = stream.continuation
        DispatchQueue(label: "maltoria.hook.reads").async { [self] in readFrames() }
    }

    func stop() {
        lock.withLock {
            stopped = true
            if listener >= 0 { _ = shutdown(listener, Int32(SHUT_RDWR)) }
            if peer >= 0 { _ = shutdown(peer, Int32(SHUT_RDWR)) }
        }
    }

    func send(_ value: JSONValue) async throws {
        let data = try HookFrameDecoder.encode(value)
        try await withCheckedThrowingContinuation { (reply: CheckedContinuation<Void, any Error>) in
            writes.async { [self] in
                let descriptor = lock.withLock { !stopped && peer >= 0 ? dup(peer) : -1 }
                guard descriptor >= 0 else { reply.resume(throwing: hookFailure("disconnected"))
                    return
                }
                defer { systemClose(descriptor) }
                do {
                    try data.withUnsafeBytes { raw in
                        var offset = 0
                        while offset < raw.count {
                            #if canImport(Darwin)
                            let flags: Int32 = 0
                            #else
                            let flags = Int32(MSG_NOSIGNAL)
                            #endif
                            let count = systemSend(
                                descriptor,
                                raw.baseAddress!.advanced(by: offset),
                                raw.count - offset,
                                flags
                            )
                            if count < 0, errno == EINTR { continue }
                            guard count > 0 else { throw hookFailure("send_failed") }
                            offset += count
                        }
                    }
                    reply.resume()
                } catch { reply.resume(throwing: error) }
            }
        }
    }

    private func readFrames() {
        defer {
            lock.withLock {
                if peer >= 0 { systemClose(peer)
                    peer = -1
                }
                if listener >= 0 { systemClose(listener)
                    listener = -1
                }
                stopped = true
            }
        }
        do {
            let deadline = Date().addingTimeInterval(15)
            var accepted: Int32 = -1
            while accepted < 0 {
                if lock.withLock({ stopped }) { throw CancellationError() }
                guard Date() < deadline else { throw hookFailure("connect_timeout") }
                var descriptor = pollfd(fd: listener, events: Int16(POLLIN), revents: 0)
                let available = poll(&descriptor, 1, 200)
                if available < 0, errno != EINTR { throw hookFailure("accept_failed") }
                if available > 0 { accepted = accept(listener, nil, nil) }
            }
            lock.withLock { peer = accepted }
            _ = fcntl(accepted, F_SETFD, FD_CLOEXEC)
            // BSD sockets may inherit O_NONBLOCK from the listener. Reads below
            // use SO_RCVTIMEO; keep them blocking so idle sessions do not spin.
            let flags = fcntl(accepted, F_GETFL)
            guard flags >= 0, fcntl(accepted, F_SETFL, flags & ~O_NONBLOCK) == 0 else {
                throw hookFailure("socket_unavailable")
            }
            #if canImport(Darwin)
            var enabled: Int32 = 1
            _ = setsockopt(accepted, SOL_SOCKET, SO_NOSIGPIPE, &enabled, socklen_t(MemoryLayout<Int32>.size))
            #endif
            var readTimeout = timeval(tv_sec: 1, tv_usec: 0)
            var writeTimeout = timeval(tv_sec: 5, tv_usec: 0)
            guard setsockopt(accepted, SOL_SOCKET, SO_RCVTIMEO, &readTimeout, socklen_t(MemoryLayout<timeval>.size)) ==
                0,
                setsockopt(accepted, SOL_SOCKET, SO_SNDTIMEO, &writeTimeout, socklen_t(MemoryLayout<timeval>.size)) ==
                0
            else { throw hookFailure("socket_unavailable") }
            var decoder = HookFrameDecoder()
            var bytes = [UInt8](repeating: 0, count: 4_096)
            while !lock.withLock({ stopped }) {
                let count = recv(accepted, &bytes, bytes.count, 0)
                if count < 0, [EINTR, EAGAIN, EWOULDBLOCK].contains(errno) { continue }
                guard count >= 0 else { throw hookFailure("receive_failed") }
                if count == 0 { try decoder.finish()
                    break
                }
                for value in try decoder.append(Data(bytes.prefix(count))) {
                    if case .dropped = continuation.yield(value) { throw hookFailure("events_dropped") }
                }
            }
            continuation.finish()
        } catch { continuation.finish(throwing: error) }
    }
}

private func systemClose(_ descriptor: Int32) {
    #if canImport(Darwin)
    _ = Darwin.close(descriptor)
    #else
    _ = Glibc.close(descriptor)
    #endif
}

// swiftlint:disable:next identifier_name
private func systemSend(_ fd: Int32, _ bytes: UnsafeRawPointer, _ count: Int, _ flags: Int32) -> Int {
    #if canImport(Darwin)
    Darwin.send(fd, bytes, count, flags)
    #else
    Glibc.send(fd, bytes, count, flags)
    #endif
}

public actor HookIPCClient: HookClient {
    private let socket: HookSocket
    private let session: String
    private let token: String
    private let architecture: String
    private let stream: AsyncThrowingStream<JSONValue, any Error>
    private let continuation: AsyncThrowingStream<JSONValue, any Error>.Continuation
    private var reader: Task<Void, Never>?
    private var hello: CheckedContinuation<Void, any Error>?
    private var authenticated = false
    private var closed = false
    private var pending: [String: CheckedContinuation<JSONValue, any Error>] = [:]

    public static func make(session: String, architecture: String) throws -> HookClientBootstrap {
        let socket = try HookSocket()
        // Swift's system generator uses the platform CSPRNG. Never place this in args.
        let token = (0 ..< 32).map { _ in String(format: "%02x", UInt8.random(in: .min ... .max)) }.joined()
        let client = HookIPCClient(socket: socket, session: session, token: token, architecture: architecture)
        return HookClientBootstrap(client: client, environment: [
            "MALTORIA_HOOK_PORT": String(socket.port), "MALTORIA_HOOK_TOKEN": token, "MALTORIA_HOOK_SESSION": session
        ])
    }

    private init(socket: HookSocket, session: String, token: String, architecture: String) {
        self.socket = socket
        self.session = session
        self.token = token
        self.architecture = architecture
        let events = AsyncThrowingStream<JSONValue, any Error>.makeStream(bufferingPolicy: .bufferingOldest(2048))
        stream = events.stream
        continuation = events.continuation
    }

    public func events() -> AsyncThrowingStream<JSONValue, any Error> { stream }

    public func ready() async throws {
        if authenticated { return }
        guard !closed, hello == nil else { throw hookFailure("disconnected") }
        try await withCheckedThrowingContinuation { waiter in
            hello = waiter
            reader = Task { [weak self, socket] in
                do {
                    for try await value in socket.frames {
                        try await self?.receive(value)
                    }
                    await self?.fail(hookFailure("disconnected"))
                } catch { await self?.fail(error) }
            }
            Task { [weak self] in
                try? await Task.sleep(for: .seconds(15))
                await self?.expireHello()
            }
        }
    }

    public func request(_ command: String, params: [String: JSONValue]) async throws -> JSONValue {
        guard authenticated, !closed, pending.count < 16 else { throw hookFailure("disconnected") }
        let id = UUID().uuidString
        return try await withTaskCancellationHandler {
            try await withCheckedThrowingContinuation { waiter in
                pending[id] = waiter
                Task { [weak self, socket, session] in
                    do {
                        try await socket.send(.object([
                            "type": .string("command"), "id": .string(id), "session": .string(session),
                            "command": .string(command), "params": .object(params)
                        ]))
                        try await Task.sleep(for: .seconds(command == "attach" ? 25 : 10))
                        await self?.expire(id)
                    } catch { await self?.fail(error) }
                }
            }
        } onCancel: { Task { await self.close() } }
    }

    public func close() { fail(CancellationError()) }

    private func receive(_ value: JSONValue) async throws {
        guard !closed, let frame = value.object else { throw hookFailure("invalid_frame") }
        if !authenticated {
            guard frame["type"] == .string("hello"), frame["version"] == .integer(1),
                  frame["session"] == .string(session), frame["token"] == .string(token),
                  frame["arch"] == .string(architecture)
            else { throw hookFailure("authentication_failed") }
            try await socket.send(.object([
                "type": .string("hello"), "version": .integer(1), "session": .string(session), "ok": .bool(true)
            ]))
            authenticated = true
            hello?.resume()
            hello = nil
            return
        }
        guard frame["session"] == .string(session) else { throw hookFailure("wrong_session") }
        if frame["type"] == .string("response") {
            guard let id = frame["id"]?.string, let waiter = pending.removeValue(forKey: id) else {
                throw hookFailure("unexpected_response")
            }
            if frame["ok"] == .bool(true), let result = frame["result"] { waiter.resume(returning: result) } else {
                // Only forward the bounded code. Never trust native diagnostics as display text.
                let code = frame["error"]?.object?["code"]?.string ?? "operation_failed"
                waiter.resume(throwing: hookFailure(String(code.prefix(80))))
            }
        } else if ["text", "thread", "process", "error"].contains(frame["type"]?.string ?? "") {
            if case .dropped = continuation.yield(value) { throw hookFailure("events_dropped") }
        } else { throw hookFailure("unexpected_frame") }
    }

    private func expireHello() {
        if !authenticated { fail(hookFailure("connect_timeout")) }
    }

    private func expire(_ id: String) {
        if pending[id] != nil { fail(hookFailure("request_timeout")) }
    }

    private func fail(_ error: any Error) {
        guard !closed else { return }
        closed = true
        socket.stop()
        reader?.cancel()
        reader = nil
        hello?.resume(throwing: error)
        hello = nil
        let waiters = pending.values
        pending.removeAll()
        for waiter in waiters {
            waiter.resume(throwing: error)
        }
        continuation.finish(throwing: error)
    }
}
