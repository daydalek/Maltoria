#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
"""Wire-format helpers shared by the Windows host smoke test and unit tests."""
import json
import ntpath
import re

MAX_FRAME_BYTES = 64 * 1024
_FILETIME_HEX = re.compile(r"^[0-9A-Fa-f]{16}$")


class ProtocolError(ValueError):
    pass


class FrameDecoder:
    def __init__(self):
        self._buffer = bytearray()
        self._failed = False

    def feed(self, chunk):
        if self._failed:
            raise ProtocolError("decoder is closed after a protocol error")
        if not isinstance(chunk, (bytes, bytearray)):
            self._fail("frame data must be bytes")
        self._buffer.extend(chunk)
        frames = []
        while True:
            newline = self._buffer.find(b"\n")
            if newline < 0:
                break
            if newline + 1 > MAX_FRAME_BYTES:
                self._fail("frame exceeds 64 KiB")
            line = bytes(self._buffer[:newline])
            del self._buffer[:newline + 1]
            if line.endswith(b"\r"):
                line = line[:-1]
            if not line:
                self._fail("empty frame")
            try:
                message = json.loads(line.decode("utf-8", errors="strict"))
            except (UnicodeDecodeError, json.JSONDecodeError) as error:
                self._fail(f"invalid JSON frame: {error}")
            if not isinstance(message, dict):
                self._fail("frame must be a JSON object")
            frames.append(message)
        if len(self._buffer) >= MAX_FRAME_BYTES:
            self._fail("unterminated frame exceeds 64 KiB")
        return frames

    def finish(self):
        if self._failed:
            raise ProtocolError("decoder is closed after a protocol error")
        if self._buffer:
            self._fail("connection ended in a partial frame")

    def _fail(self, message):
        self._failed = True
        raise ProtocolError(message)


def encode_frame(message):
    if not isinstance(message, dict):
        raise ProtocolError("frame must be a JSON object")
    try:
        payload = json.dumps(message, ensure_ascii=False, allow_nan=False,
                             separators=(",", ":")).encode("utf-8", errors="strict")
    except (TypeError, ValueError, UnicodeEncodeError) as error:
        raise ProtocolError("message cannot be encoded as JSON") from error
    frame = payload + b"\n"
    if len(frame) > MAX_FRAME_BYTES:
        raise ProtocolError("frame exceeds 64 KiB")
    return frame


def validate_hello(message, token, session, architecture):
    return (
        isinstance(message, dict)
        and message.get("type") == "hello"
        and type(message.get("version")) is int
        and message["version"] == 1
        and message.get("token") == token
        and message.get("session") == session
        and message.get("arch") == architecture
    )


def validate_process_record(record):
    return (
        isinstance(record, dict)
        and type(record.get("guestPID")) is int
        and record["guestPID"] > 0
        and isinstance(record.get("exePath"), str)
        and ntpath.isabs(record["exePath"])
        and bool(ntpath.splitdrive(record["exePath"])[0])
        and record.get("arch") in {"x86", "x64", "arm64", "unknown"}
        and isinstance(record.get("creationTime"), str)
        and _FILETIME_HEX.fullmatch(record["creationTime"]) is not None
    )
