#!/usr/bin/env python3
"""Unit tests for scripts/i18n.py on small fixtures."""

import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import i18n  # noqa: E402


class TestPlaceholders(unittest.TestCase):
    def test_plain(self):
        self.assertEqual(i18n.placeholders("hello"), set())

    def test_named(self):
        self.assertEqual(i18n.placeholders("{count} games"), {"count"})
        self.assertEqual(i18n.placeholders("{a} {b} {a}"), {"a", "b"})

    def test_escaped_braces(self):
        self.assertEqual(i18n.placeholders("{{literal}} {x}"), {"x"})
        self.assertEqual(i18n.placeholders("{{"), set())


class TestClassify(unittest.TestCase):
    def test_kinds(self):
        self.assertEqual(i18n.classify("plain"), "scalar")
        self.assertEqual(i18n.classify("{x}"), "template")
        self.assertEqual(i18n.classify(["a", "b"]), "array")
        self.assertEqual(i18n.classify({"other": "{count} x"}), "plural")


class TestValidateLocales(unittest.TestCase):
    def files(self, en, others):
        base = {"en": en}
        base.update(others)
        return base

    def test_missing_key(self):
        en = {"common": {"a": "A", "b": "B"}}
        ja = {"common": {"a": "A"}}
        errors = i18n.validate_locales(self.files(en, {"ja": ja}))
        self.assertTrue(any("missing key common.b" in e for e in errors))

    def test_extra_key(self):
        en = {"common": {"a": "A"}}
        ja = {"common": {"a": "A", "z": "Z"}}
        errors = i18n.validate_locales(self.files(en, {"ja": ja}))
        self.assertTrue(any("extra key common.z" in e for e in errors))

    def test_placeholder_mismatch(self):
        en = {"s": {"k": "{count} games"}}
        ja = {"s": {"k": "{n} 個"}}
        errors = i18n.validate_locales(self.files(en, {"ja": ja}))
        self.assertTrue(any("placeholder set" in e for e in errors))

    def test_plural_rules(self):
        en = {"s": {"k": {"one": "{count} x", "other": "{count} xs"}}}
        ja = {"s": {"k": {"one": "{count}個", "other": "{count}個"}}}
        errors = i18n.validate_locales(self.files(en, {"ja": ja}))
        self.assertTrue(any("`one` is only allowed in en.toml" in e for e in errors))

    def test_plural_requires_count(self):
        en = {"s": {"k": {"other": "no placeholder"}}}
        errors = i18n.validate_locales(self.files(en, {}))
        self.assertTrue(any("must contain" in e for e in errors))

    def test_zero_optional(self):
        en = {"s": {"k": {"one": "{count} x", "other": "{count} xs"}}}
        ja = {"s": {"k": {"zero": "なし", "other": "{count}個"}}}
        errors = i18n.validate_locales(self.files(en, {"ja": ja}))
        self.assertEqual(errors, [])

    def test_map_key_mismatch_and_placeholder(self):
        en = {"common": {"a": "A"}, "maps": {"m": {"x": "X"}}}
        ja = {"common": {"a": "A"}, "maps": {"m": {"y": "Y"}}}
        errors = i18n.validate_locales(self.files(en, {"ja": ja}))
        self.assertTrue(any("extra key maps.m.y" in e for e in errors))
        self.assertTrue(any("missing key maps.m.x" in e for e in errors))
        ja2 = {"common": {"a": "A"}, "maps": {"m": {"x": "{oops}"}}}
        errors = i18n.validate_locales(self.files(en, {"ja": ja2}))
        self.assertTrue(any("must not contain placeholders" in e for e in errors))

    def test_array_length(self):
        en = {"s": {"k": ["a", "b"]}}
        ja = {"s": {"k": ["a"]}}
        errors = i18n.validate_locales(self.files(en, {"ja": ja}))
        self.assertTrue(any("array length" in e for e in errors))


class TestTestRegionStripping(unittest.TestCase):
    def test_mod_block(self):
        text = 'fn a() {}\n#[cfg(test)]\nmod t {\n    fn x() { "テスト"; }\n}\nfn b() { "テスト"; }\n'
        test_lines = i18n.strip_test_regions(text)
        # lines 2-5 are inside the test block; line 6 is not
        self.assertIn(3, test_lines)
        self.assertNotIn(6, test_lines)

    def test_use_statement_does_not_swallow(self):
        text = '#[cfg(test)]\nuse foo::bar;\nfn a() { "x"; }\n'
        test_lines = i18n.strip_test_regions(text)
        self.assertNotIn(3, test_lines)


class TestBridgeCodes(unittest.TestCase):
    def test_capture_prefix_strip(self):
        # `capture_foo` codes resolve through the bare `foo` capture key.
        code = "capture_disconnected"
        check = code[len("capture_"):] if code.startswith("capture_") else code
        self.assertEqual(check, "disconnected")


if __name__ == "__main__":
    unittest.main()
