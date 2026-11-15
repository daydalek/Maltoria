#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
import unittest

from native_host_protocol import (
    MAX_FRAME_BYTES,
    FrameDecoder,
    ProtocolError,
    encode_frame,
    validate_hello,
    validate_process_record,
)


class NativeHostProtocolTests(unittest.TestCase):
    def test_split_multibyte_utf8_and_crlf(self):
        frame = encode_frame({"text": "こんにちは、世界。"})[:-1] + b"\r\n"
        decoder = FrameDecoder()
        result = []
        for byte in frame:
            result.extend(decoder.feed(bytes([byte])))
        decoder.finish()
        self.assertEqual([{"text": "こんにちは、世界。"}], result)

    def test_multiple_frames_in_one_chunk(self):
        decoder = FrameDecoder()
        frames = encode_frame({"id": "one"}) + encode_frame({"id": "two"})
        self.assertEqual([{"id": "one"}, {"id": "two"}], decoder.feed(frames))
        decoder.finish()

    def test_oversize_and_truncated_frames_are_rejected(self):
        decoder = FrameDecoder()
        with self.assertRaisesRegex(ProtocolError, "64 KiB"):
            decoder.feed(b"{" + b"x" * MAX_FRAME_BYTES)
        truncated = FrameDecoder()
        truncated.feed(b'{"id":')
        with self.assertRaisesRegex(ProtocolError, "partial frame"):
            truncated.finish()

    def test_invalid_utf8_json_and_non_object_frames_are_rejected(self):
        for frame in (b"\xff\n", b"{\n", b"[]\n", b"\n"):
            with self.subTest(frame=frame), self.assertRaises(ProtocolError):
                FrameDecoder().feed(frame)

    def test_hello_requires_exact_version_token_session_and_architecture(self):
        hello = {"type": "hello", "version": 1, "token": "secret",
                 "session": "session-1", "arch": "x64"}
        self.assertTrue(validate_hello(hello, "secret", "session-1", "x64"))
        self.assertFalse(validate_hello(hello, "wrong", "session-1", "x64"))
        self.assertFalse(validate_hello([], "secret", "session-1", "x64"))
        self.assertFalse(validate_hello({**hello, "version": True}, "secret", "session-1", "x64"))

    def test_process_identity_schema_keeps_unicode_path_and_hex_filetime(self):
        record = {"guestPID": 72, "exePath": r"C:\ゲーム\fixture.exe",
                  "arch": "x86", "creationTime": "01DC123456789ABC"}
        self.assertTrue(validate_process_record(record))
        self.assertFalse(validate_process_record({**record, "creationTime": "123"}))
        self.assertFalse(validate_process_record({**record, "exePath": "fixture.exe"}))
        self.assertFalse(validate_process_record({**record, "guestPID": True}))


if __name__ == "__main__":
    unittest.main()
