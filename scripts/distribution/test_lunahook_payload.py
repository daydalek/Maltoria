# SPDX-License-Identifier: GPL-3.0-or-later
import importlib.util
import json
from pathlib import Path
import struct
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]
spec = importlib.util.spec_from_file_location('stage_lunahook', ROOT / 'scripts/stage-lunahook-payload.py')
stage_lunahook = importlib.util.module_from_spec(spec)
spec.loader.exec_module(stage_lunahook)


def pe(machine, body=b''):
    header = bytearray(0x100)
    header[:2] = b'MZ'
    struct.pack_into('<I', header, 0x3C, 0x80)
    header[0x80:0x84] = b'PE\0\0'
    struct.pack_into('<H', header, 0x84, machine)
    return bytes(header) + body


def artifacts(folder, machines=None):
    machines = machines or {'x86': 0x014C, 'x64': 0x8664}
    for arch in ('x86', 'x64'):
        (folder / arch).mkdir(parents=True)
        for name in stage_lunahook.expected_files(arch):
            machine = 0x014C if name in stage_lunahook.SHARED_FILES else machines[arch]
            (folder / arch / name).write_bytes(pe(machine, name.encode()))
        (folder / arch / 'MaltoriaUnicodeFixture.exe').write_bytes(pe(machines[arch]))
    sources = folder / 'sources/Maltoria'
    (sources / 'ThirdParty').mkdir(parents=True)
    (sources / 'ThirdParty/manifest.json').write_bytes((ROOT / 'ThirdParty/manifest.json').read_bytes())
    (sources / 'THIRD-PARTY-NOTICES.md').write_text('notices')
    (sources / 'LICENSE').write_text('GPL')
    (sources / 'SOURCE-COMMIT.txt').write_text('a' * 40 + '\n')


class LunaHookPayloadTests(unittest.TestCase):
    def test_stages_both_architectures_with_sources_and_manifest(self):
        with tempfile.TemporaryDirectory() as folder:
            folder = Path(folder)
            artifacts(folder / 'in')
            manifest = stage_lunahook.stage(folder / 'in', folder / 'out', ROOT)
            self.assertEqual(manifest, stage_lunahook.verify(folder / 'out'))
            self.assertEqual('a' * 40, manifest['sourceCommit'])
            self.assertTrue((folder / 'out/x86/MaltoriaLunaHost32.exe').is_file())
            self.assertTrue((folder / 'out/x64/LunaHook64.dll').is_file())
            self.assertFalse((folder / 'out/x64/MaltoriaUnicodeFixture.exe').exists())
            self.assertTrue((folder / 'out/sources/Maltoria/THIRD-PARTY-NOTICES.md').is_file())
            self.assertEqual(8, len(json.loads((folder / 'out/manifest.json').read_text())['files']))

    def test_accepts_anycpu_font_loader_only_for_shared_files(self):
        self.assertTrue(stage_lunahook.machine_allowed('x64', 'LunaTmpFontLoader.dll', 0x014C))
        self.assertFalse(stage_lunahook.machine_allowed('x64', 'LunaHook64.dll', 0x014C))

    def test_rejects_wrong_pe_architecture(self):
        with tempfile.TemporaryDirectory() as folder:
            folder = Path(folder)
            artifacts(folder / 'in', {'x86': 0x8664, 'x64': 0x8664})
            with self.assertRaisesRegex(ValueError, 'PE architecture mismatch'):
                stage_lunahook.stage(folder / 'in', folder / 'out', ROOT)
            self.assertFalse((folder / 'out').exists())

    def test_rejects_missing_file_or_source_package(self):
        with tempfile.TemporaryDirectory() as folder:
            folder = Path(folder)
            artifacts(folder / 'in')
            (folder / 'in/x64/LunaHost64.dll').unlink()
            with self.assertRaisesRegex(ValueError, 'Missing native payload file'):
                stage_lunahook.stage(folder / 'in', folder / 'out', ROOT)
        with tempfile.TemporaryDirectory() as folder:
            folder = Path(folder)
            artifacts(folder / 'in')
            (folder / 'in/sources/Maltoria/SOURCE-COMMIT.txt').unlink()
            with self.assertRaisesRegex(ValueError, 'corresponding source package'):
                stage_lunahook.stage(folder / 'in', folder / 'out', ROOT)

    def test_verify_detects_tampering_and_existing_output_is_preserved(self):
        with tempfile.TemporaryDirectory() as folder:
            folder = Path(folder)
            artifacts(folder / 'in')
            stage_lunahook.stage(folder / 'in', folder / 'out', ROOT)
            (folder / 'out/x86/LunaHook32.dll').write_bytes(pe(0x014C, b'changed'))
            with self.assertRaisesRegex(ValueError, 'hash mismatch'):
                stage_lunahook.verify(folder / 'out')
            with self.assertRaisesRegex(ValueError, 'new payload directory'):
                stage_lunahook.stage(folder / 'in', folder / 'out', ROOT)

    def test_bridge_payload_lookup_matches_packaged_layout(self):
        capture = (ROOT / 'WhiskyBridge/Sources/WhiskyApplication/TranslationCapture.swift').read_text()
        self.assertIn('appendingPathComponent("LunaHook")', capture)
        self.assertIn('Contents/Resources/LunaHook', (ROOT / 'apps/whisky-gpui/tools/package-app.sh').read_text())
        self.assertIn("bin/LunaHook", (ROOT / 'apps/whisky-gpui/tools/package-linux.py').read_text())


if __name__ == '__main__':
    unittest.main()
