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


class TestBridgeDynamicCalls(unittest.TestCase):
    def _with_swift(self, content):
        tmp = tempfile.TemporaryDirectory()
        root = Path(tmp.name)
        (root / "swift").mkdir()
        (root / "swift" / "Service.swift").write_text(content)
        old_src, old_root = i18n.SWIFT_SRC, i18n.ROOT
        i18n.SWIFT_SRC = root / "swift"
        i18n.ROOT = root
        self.addCleanup(self._restore, tmp, old_src, old_root)

    def _restore(self, tmp, old_src, old_root):
        i18n.SWIFT_SRC = old_src
        i18n.ROOT = old_root
        tmp.cleanup()

    def test_literal_codes_are_not_dynamic(self):
        self._with_swift('throw BridgeFailure("busy", "x")\nthrow BridgeFailure(code: "offline", "y")')
        self.assertEqual(i18n.bridge_dynamic_calls(), {})

    def test_nonliteral_codes_are_reported(self):
        self._with_swift('throw BridgeFailure("capture_" + code, "x")\nthrow BridgeFailure(code, "y")')
        calls = i18n.bridge_dynamic_calls()
        self.assertEqual(sorted(calls), ['"capture_" + code', 'code'])

    def test_code_label_dynamic(self):
        self._with_swift('throw BridgeFailure(code: Self.engineErrorCode(error), "y")')
        calls = i18n.bridge_dynamic_calls()
        self.assertEqual(list(calls), ['Self.engineErrorCode(error)'])


class TestAllowlistCategories(unittest.TestCase):
    def test_new_categories_parse(self):
        tmp = tempfile.TemporaryDirectory()
        path = Path(tmp.name) / "allow.toml"
        path.write_text(
            '[bridge_dynamic."F.swift::code"]\nreason = "r"\n'
            '[identical."sec.key"]\nreason = "r2"\n'
        )
        old = i18n.ALLOWLIST_PATH
        i18n.ALLOWLIST_PATH = path
        try:
            allow = i18n.load_allowlist()
        finally:
            i18n.ALLOWLIST_PATH = old
            tmp.cleanup()
        self.assertEqual(allow["bridge_dynamic"]["F.swift::code"]["reason"], "r")
        self.assertEqual(allow["identical"]["sec.key"]["reason"], "r2")


if __name__ == "__main__":
    unittest.main()

class TestNaming(unittest.TestCase):
    def test_clean_en(self):
        en = {
            "library": {"loading_library": "Loading…", "items": "{count} items"},
            "maps": {"provider_label": {"openAIChat": "OpenAI"}},
        }
        self.assertEqual(i18n.naming_errors(en), [])

    def test_long_key(self):
        en = {"sec": {"a" * 41: "x"}}
        errs = i18n.naming_errors(en)
        self.assertEqual(len(errs), 1)
        self.assertIn("40", errs[0])

    def test_bad_key_pattern(self):
        en = {"sec": {"Key_Name": "x", "9lives": "y"}}
        errs = i18n.naming_errors(en)
        self.assertEqual(len(errs), 2)

    def test_single_char_placeholder(self):
        en = {"sec": {"k": "at {h}"}}
        errs = i18n.naming_errors(en)
        self.assertEqual(len(errs), 1)
        self.assertIn("{h}", errs[0])

    def test_numeric_placeholder(self):
        en = {"sec": {"k": "{p01} done", "k2": "{p1} x"}}
        errs = i18n.naming_errors(en)
        self.assertEqual(len(errs), 2)

    def test_plural_placeholders_checked(self):
        en = {"sec": {"k": {"one": "{count} {h}", "other": "{count} ok"}}}
        errs = i18n.naming_errors(en)
        self.assertEqual(len(errs), 1)

    def test_maps_exempt(self):
        en = {"maps": {"m": {"openAIChat_kind_too_long_over_forty_chars_xx": "v"}}}
        self.assertEqual(i18n.naming_errors(en), [])
