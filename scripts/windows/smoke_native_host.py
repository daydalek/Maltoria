#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
"""Exercise authenticated capture against our own Unicode GDI fixture only."""
import argparse
from collections import deque
import ctypes
import ntpath
import os
from pathlib import Path
import secrets
import socket
import subprocess
import sys
import time
import uuid

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "scripts/distribution"))
from native_host_protocol import FrameDecoder, encode_frame, validate_hello, validate_process_record

WM_CLOSE = 0x0010
USER32 = ctypes.WinDLL("user32", use_last_error=True) if os.name == "nt" else None
if USER32 is not None:
    USER32.FindWindowW.argtypes = (ctypes.c_wchar_p, ctypes.c_wchar_p)
    USER32.FindWindowW.restype = ctypes.c_void_p
    USER32.PostMessageW.argtypes = (ctypes.c_void_p, ctypes.c_uint,
                                    ctypes.c_size_t, ctypes.c_ssize_t)
    USER32.PostMessageW.restype = ctypes.c_bool


def fixture_window():
    return USER32.FindWindowW(None, "Maltoria Unicode Fixture")


class Peer:
    def __init__(self, connection):
        self.connection = connection
        self.decoder = FrameDecoder()
        self.messages = deque()
        self.events = deque(maxlen=512)

    def receive(self):
        while not self.messages:
            chunk = self.connection.recv(4096)
            if not chunk:
                self.decoder.finish()
                raise EOFError("host closed the protocol connection")
            self.messages.extend(self.decoder.feed(chunk))
        return self.messages.popleft()

    def response(self, request_id, session):
        while True:
            message = self.receive()
            if message.get("session") != session:
                raise AssertionError("unexpected event session")
            if message.get("type") != "response":
                self.events.append(message)
                continue
            if message.get("id") != request_id:
                raise AssertionError("unexpected response ID")
            return message


def normalized_windows_path(path):
    path = str(path)
    if path.startswith("\\\\?\\"):
        path = path[4:]
    return ntpath.normcase(ntpath.normpath(path))


def start_host(host, server, architecture):
    token = secrets.token_urlsafe(32)
    session = uuid.uuid4().hex
    environment = os.environ.copy()
    environment.update({
        "MALTORIA_HOOK_PORT": str(server.getsockname()[1]),
        "MALTORIA_HOOK_TOKEN": token,
        "MALTORIA_HOOK_SESSION": session,
    })
    process = subprocess.Popen([str(host)], env=environment,
                               stdin=subprocess.DEVNULL,
                               stdout=subprocess.PIPE, stderr=subprocess.PIPE)
    connection = None
    try:
        connection, _ = server.accept()
        connection.settimeout(10)
        peer = Peer(connection)
        hello = peer.receive()
        if not validate_hello(hello, token, session, architecture):
            raise AssertionError("host hello did not match the authenticated session")
        connection.sendall(encode_frame({
            "type": "hello", "version": 1, "session": session, "ok": True,
        }))
        return process, connection, peer, token, session
    except BaseException:
        if connection is not None:
            connection.close()
        try:
            process.wait(timeout=35)
        except subprocess.TimeoutExpired:
            pass
        raise


def finish_host(process, token, expected_code=0):
    stdout, stderr = process.communicate(timeout=10)
    if process.returncode != expected_code:
        raise AssertionError(f"host exited with code {process.returncode}, expected {expected_code}")
    if token.encode("ascii") in stdout + stderr:
        raise AssertionError("host emitted the session token")


def send_command(connection, command, request_id, session, **fields):
    request = {"type": "command", "id": request_id, "session": session,
               "command": command, **fields}
    connection.sendall(encode_frame(request))


def assert_error(peer, request_id, session, code):
    response = peer.response(request_id, session)
    if response.get("ok") is not False or response.get("error", {}).get("code") != code:
        raise AssertionError(f"expected {code}, received {response.get('error', {}).get('code')}")


def assert_success(peer, request_id, session):
    response = peer.response(request_id, session)
    if response.get("ok") is not True:
        raise AssertionError(f"command failed: {response.get('error', {}).get('code')}")
    return response


def close_fixture(process):
    window = fixture_window()
    if window:
        USER32.PostMessageW(window, WM_CLOSE, 0, 0)
    process.wait(timeout=10)
    if process.returncode not in (0, None):
        raise AssertionError("Unicode fixture did not close cleanly")


def run(host, fixture, architecture):
    if os.name != "nt":
        raise RuntimeError("This smoke test requires Windows")
    fixture_process = subprocess.Popen([str(fixture)], stdin=subprocess.DEVNULL,
                                       stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    try:
        deadline = time.monotonic() + 10
        while not fixture_window():
            if fixture_process.poll() is not None:
                raise AssertionError("Unicode fixture exited before creating its GDI window")
            if time.monotonic() >= deadline:
                raise TimeoutError("Unicode fixture window did not appear")
            time.sleep(0.05)

        with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as server:
            server.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
            server.bind(("127.0.0.1", 0))
            server.listen(1)
            server.settimeout(10)

            process, connection, peer, token, session = start_host(host, server, architecture)
            wrong_session = dict(type="command", id="wrong-session", session="not-this-session",
                                 command="listProcesses")
            connection.sendall(encode_frame(wrong_session))
            connection.close()
            finish_host(process, token, expected_code=7)

            process, connection, peer, token, session = start_host(host, server, architecture)
            send_command(connection, "listProcesses", "list-1", session)
            response = peer.response("list-1", session)
            if (response.get("type") != "response" or response.get("id") != "list-1"
                    or response.get("session") != session or response.get("ok") is not True):
                raise AssertionError("listProcesses response did not match its request")
            records = response.get("result", {}).get("processes", [])
            fixture_record = next((record for record in records
                                   if record.get("guestPID") == fixture_process.pid), None)
            if not validate_process_record(fixture_record):
                raise AssertionError("process list omitted or malformed the Unicode fixture identity")
            if normalized_windows_path(fixture_record["exePath"]) != normalized_windows_path(fixture):
                raise AssertionError("process list did not report the fixture's full image path")
            if fixture_record["arch"] != architecture:
                raise AssertionError("process list reported the wrong fixture architecture")

            stale_identity = {**fixture_record, "creationTime": "0000000000000000"}
            send_command(connection, "attach", "stale", session, params={"target": stale_identity})
            assert_error(peer, "stale", session, "stale_identity")
            wrong_path = {**fixture_record, "exePath": r"C:\not-the-fixture.exe"}
            send_command(connection, "attach", "wrong-path", session, params={"target": wrong_path})
            assert_error(peer, "wrong-path", session, "stale_identity")
            send_command(connection, "insertHook", "unattached", session, params={"hookCode": "HTEST"})
            assert_error(peer, "unattached", session, "not_attached")
            send_command(connection, "attach", "attach", session, params={"target": fixture_record})
            assert_success(peer, "attach", session)
            expected = "こんにちは、世界。雪国"
            deadline = time.monotonic() + 15
            captured = False
            while time.monotonic() < deadline:
                event = peer.events.popleft() if peer.events else peer.receive()
                if event.get("session") != session:
                    raise AssertionError("capture event escaped its session")
                if event.get("type") == "text" and expected in event.get("text", ""):
                    thread = event.get("thread", {})
                    if thread.get("guestPID") != fixture_process.pid:
                        raise AssertionError("capture belongs to another process")
                    for field in ("addressHex", "contextHex", "context2Hex"):
                        if not isinstance(thread.get(field), str) or not thread[field].startswith("0x"):
                            raise AssertionError("address fields lost their hexadecimal encoding")
                    captured = True
                    break
            if not captured:
                raise AssertionError("Unicode GDI fixture text was not captured")
            send_command(connection, "ping", "ping", session)
            assert_success(peer, "ping", session)
            send_command(connection, "detach", "detach", session)
            assert_success(peer, "detach", session)
            if fixture_process.poll() is not None:
                raise AssertionError("fixture exited during detach")

            send_command(connection, "shutdown", "shutdown-1", session)
            shutdown = peer.response("shutdown-1", session)
            if shutdown.get("id") != "shutdown-1" or shutdown.get("ok") is not True:
                raise AssertionError("host did not acknowledge shutdown")
            connection.close()
            finish_host(process, token)
            if fixture_process.poll() is not None:
                raise AssertionError("fixture was terminated when the helper shut down")

            process, connection, peer, token, session = start_host(host, server, architecture)
            send_command(connection, "attach", "attach-eof", session, params={"target": fixture_record})
            assert_success(peer, "attach-eof", session)
            connection.close()
            finish_host(process, token)
            if fixture_process.poll() is not None:
                raise AssertionError("fixture exited during attached-helper EOF cleanup")

            # A fresh helper must be able to attach after EOF released our hook.
            process, connection, peer, token, session = start_host(host, server, architecture)
            send_command(connection, "attach", "attach-exit", session, params={"target": fixture_record})
            assert_success(peer, "attach-exit", session)
            close_fixture(fixture_process)
            deadline = time.monotonic() + 10
            disconnected = False
            while time.monotonic() < deadline:
                event = peer.events.popleft() if peer.events else peer.receive()
                if (event.get("type") == "process" and event.get("action") == "disconnected"
                        and event.get("guestPID") == fixture_process.pid):
                    disconnected = True
                    break
            if not disconnected:
                raise AssertionError("target exit did not produce a disconnection event")
            send_command(connection, "ping", "after-exit", session)
            assert_success(peer, "after-exit", session)
            send_command(connection, "shutdown", "exit-shutdown", session)
            assert_success(peer, "exit-shutdown", session)
            connection.close()
            finish_host(process, token)

        print(f"Native host Unicode capture/identity/detach/EOF/target-exit smoke passed (arch={architecture}).")
    finally:
        if fixture_process.poll() is None:
            close_fixture(fixture_process)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--host", type=Path, required=True)
    parser.add_argument("--fixture", type=Path, required=True)
    parser.add_argument("--architecture", choices=("x86", "x64"), required=True)
    args = parser.parse_args()
    run(args.host.resolve(), args.fixture.resolve(), args.architecture)
