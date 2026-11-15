#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
import json
import importlib.util
from pathlib import Path
import subprocess
import sys
import tarfile
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]
spec = importlib.util.spec_from_file_location(
    "prepare_native_sources", ROOT / "scripts/prepare-native-sources.py")
prepare_native_sources = importlib.util.module_from_spec(spec)
spec.loader.exec_module(prepare_native_sources)


class NativeSourceTests(unittest.TestCase):
    def test_locked_sources_prepare_and_pin_patch(self):
        with tempfile.TemporaryDirectory() as temporary:
            output = Path(temporary) / "prepared"
            prepare_native_sources.prepare(output)
            self.assertTrue((output / "LunaTranslator/src/NativeImpl/LunaHook/LunaHook/CMakeLists.txt").is_file())
            self.assertTrue((output / "MinHook/src/hook.c").is_file())
            cmake = (output / "LunaTranslator/src/NativeImpl/LunaHook/LunaHook/CMakeLists.txt").read_text()
            self.assertIn(prepare_native_sources.MINHOOK_REVISION, cmake)

    def test_traversal_archive_is_rejected(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            archive = root / "bad.tar.gz"
            with tarfile.open(archive, "w:gz") as stream:
                member = tarfile.TarInfo("repo-123/../escape")
                member.size = 1
                import io
                stream.addfile(member, io.BytesIO(b"x"))
            with self.assertRaisesRegex(ValueError, "Unsafe archive path"):
                prepare_native_sources.safe_extract(archive, root / "out", "123")

    def test_native_payload_inventory_and_notices(self):
        manifest = json.loads((ROOT / "ThirdParty/manifest.json").read_text())
        self.assertEqual({"lunatranslator", "minhook", "uchardet"},
                         {package["name"] for package in manifest["native_packages"]})
        subprocess.run([sys.executable, str(ROOT / "scripts/check-third-party.py")], cwd=ROOT, check=True)


if __name__ == "__main__":
    unittest.main()
