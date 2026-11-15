#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
import json
import importlib.util
import io
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
bundle_spec = importlib.util.spec_from_file_location(
    "package_native_sources", ROOT / "scripts/package-native-sources.py")
package_native_sources = importlib.util.module_from_spec(bundle_spec)
bundle_spec.loader.exec_module(package_native_sources)


class NativeSourceTests(unittest.TestCase):
    def test_locked_sources_prepare_and_pin_patch(self):
        with tempfile.TemporaryDirectory() as temporary:
            output = Path(temporary) / "prepared"
            prepare_native_sources.prepare(output)
            self.assertTrue((output / "LunaTranslator/src/NativeImpl/LunaHook/LunaHook/CMakeLists.txt").is_file())
            self.assertTrue((output / "MinHook/src/hook.c").is_file())
            self.assertTrue((output / "nlohmann-json/single_include/nlohmann/json.hpp").is_file())
            cmake = (output / "LunaTranslator/src/NativeImpl/LunaHook/LunaHook/CMakeLists.txt").read_text()
            self.assertIn(prepare_native_sources.MINHOOK_REVISION, cmake)
            main = (output / "LunaTranslator/src/NativeImpl/LunaHook/LunaHook/main.cc").read_text()
            self.assertIn("LPVOID reserved)", main)
            self.assertIn("if (reserved) return TRUE;", main)
            self.assertIn("detachall();", main)

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

    def test_windows_drive_and_ads_paths_are_rejected_before_extraction(self):
        unsafe_names = [
            "repo-123/src/C:/escape.txt",
            "repo-123/src/file.txt:stream",
            "C:/repo-123/file.txt",
        ]
        for unsafe_name in unsafe_names:
            with self.subTest(name=unsafe_name), tempfile.TemporaryDirectory() as temporary:
                root = Path(temporary)
                archive = root / "bad.tar.gz"
                with tarfile.open(archive, "w:gz") as stream:
                    good = tarfile.TarInfo("repo-123/first.txt")
                    good.size = 1
                    stream.addfile(good, io.BytesIO(b"x"))
                    bad = tarfile.TarInfo(unsafe_name)
                    bad.size = 1
                    stream.addfile(bad, io.BytesIO(b"x"))
                destination = root / "out"
                with self.assertRaisesRegex(ValueError, "Unsafe archive path"):
                    prepare_native_sources.safe_extract(archive, destination, "123")
                self.assertFalse(destination.exists())

    def test_invalid_or_multiple_archive_roots_are_rejected_before_extraction(self):
        cases = [
            (["repo-123/first.txt", "other-123/second.txt"], "123"),
            (["repo-wrong/first.txt"], "123"),
        ]
        for names, revision in cases:
            with self.subTest(names=names), tempfile.TemporaryDirectory() as temporary:
                root = Path(temporary)
                archive = root / "bad.tar.gz"
                with tarfile.open(archive, "w:gz") as stream:
                    for name in names:
                        member = tarfile.TarInfo(name)
                        member.size = 1
                        stream.addfile(member, io.BytesIO(b"x"))
                destination = root / "out"
                with self.assertRaisesRegex(ValueError, "Unexpected archive root"):
                    prepare_native_sources.safe_extract(archive, destination, revision)
                self.assertFalse(destination.exists())

    def test_native_payload_inventory_and_notices(self):
        manifest = json.loads((ROOT / "ThirdParty/manifest.json").read_text())
        self.assertEqual({"lunatranslator", "minhook", "uchardet", "nlohmann-json"},
                         {package["name"] for package in manifest["native_packages"]})
        uchardet = next(package for package in manifest["native_packages"]
                        if package["name"] == "uchardet")
        self.assertEqual("GPL-2.0-or-later", uchardet["distribution_license"])
        self.assertEqual("MPL-1.1 OR GPL-2.0-or-later OR LGPL-2.1-or-later",
                         uchardet["license"])
        subprocess.run([sys.executable, str(ROOT / "scripts/check-third-party.py")], cwd=ROOT, check=True)

    def test_native_source_bundle_preserves_build_paths_and_commit_identity(self):
        revision = "a" * 40
        with tempfile.TemporaryDirectory() as temporary:
            output = Path(temporary) / "payload"
            package_native_sources.package(output, revision)
            source = output / "Maltoria"
            self.assertEqual(revision + "\n", (source / "SOURCE-COMMIT.txt").read_text())
            required = [
                "scripts/build-lunahook.ps1",
                "scripts/prepare-native-sources.py",
                "Native/MaltoriaLunaHost/CMakeLists.txt",
                "Native/MaltoriaLunaHost/src/main.cpp",
                "Native/MaltoriaLunaHost/tests/UnicodeFixture.cpp",
                "ThirdParty/sources/native/nlohmann-json-55f93686c01528224f448c19128836e7df245f72.tar.gz",
                "ThirdParty/licenses/uchardet-license-options.txt",
                ".github/workflows/NativeLunaHook.yml",
            ]
            for relative in required:
                with self.subTest(path=relative):
                    self.assertEqual((ROOT / relative).read_bytes(),
                                     (source / relative).read_bytes())


if __name__ == "__main__":
    unittest.main()
