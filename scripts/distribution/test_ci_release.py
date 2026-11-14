#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
"""Guard release provenance, inventories and checksums before publishing."""
import hashlib
import importlib.util
import json
from pathlib import Path
import tempfile
import unittest

spec = importlib.util.spec_from_file_location('ci_release', Path(__file__).with_name('publish-ci-release.py'))
release = importlib.util.module_from_spec(spec)
spec.loader.exec_module(release)
COMMIT = 'a' * 40


class CIReleaseTests(unittest.TestCase):
    def test_only_successful_same_repository_main_push_can_publish(self):
        run = dict(status='completed', conclusion='success', event='push', head_branch='main',
                   path='.github/workflows/CI.yml', head_sha=COMMIT,
                   repository={'full_name': 'owner/repo'}, head_repository={'full_name': 'owner/repo'})
        release.validate_run(run, 'owner/repo')
        for key, value in [('conclusion', 'failure'), ('event', 'pull_request'),
                           ('event', 'workflow_dispatch'), ('head_branch', 'feature'),
                           ('head_repository', {'full_name': 'fork/repo'}), ('path', 'other.yml')]:
            with self.subTest(key=key, value=value), self.assertRaises(ValueError):
                release.validate_run(dict(run, **{key: value}), 'owner/repo')

    def fixtures(self, root):
        mac = root / 'mac'
        mac.mkdir()
        records = {}
        for name in ['Maltoria-0.6.0-arm64.zip', 'Maltoria-0.6.0-sources.tar.gz',
                     'RELEASE-NOTES.md', 'build-info.json']:
            data = b'fixture'
            if name == 'build-info.json':
                data = json.dumps(dict(app_version='0.6.0', architecture='arm64',
                                       commit=COMMIT, checkout_dirty=False)).encode()
            (mac / name).write_bytes(data)
            records[name] = hashlib.sha256(data).hexdigest()
        (mac / 'SHA256SUMS').write_text(''.join(f'{digest}  {name}\n' for name, digest in records.items()))
        names = {'deb': 'whisky_0.6.0_amd64.deb', 'rpm': 'whisky-0.6.0-1.x86_64.rpm',
                 'appimage': 'Maltoria-0.6.0-x86_64.AppImage'}
        assets = [dict(format=fmt, name=names[fmt],
                       sha256=hashlib.sha256(fmt.encode()).hexdigest()) for fmt in ('deb', 'rpm', 'appimage')]
        report = dict(passed=True, version='0.6.0', sourceCommit=COMMIT,
                      sourceTreeDirty=False, architecture='x86_64', assets=assets)
        for asset in assets:
            directory = root / asset['format']
            directory.mkdir()
            (directory / asset['name']).write_bytes(asset['format'].encode())
            (directory / 'packages.json').write_text(json.dumps(report))
            (directory / 'elf-abi.json').write_text('{}')

    def test_verified_platform_assets_have_separate_checksum_manifests(self):
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder)
            self.fixtures(root)
            output = release.prepare_assets(root, '0.6.0', COMMIT)
            self.assertTrue((output / 'SHA256SUMS-linux-amd64').is_file())
            self.assertTrue((output / 'SHA256SUMS-macos-arm64').is_file())
            self.assertEqual(3, sum(len(list(output.glob('*.' + fmt))) for fmt in ('deb', 'rpm', 'AppImage')))

    def test_wrong_commit_dirty_source_and_tampered_payload_are_rejected(self):
        for mutation in ('commit', 'dirty', 'bytes'):
            with self.subTest(mutation=mutation), tempfile.TemporaryDirectory() as folder:
                root = Path(folder)
                self.fixtures(root)
                if mutation == 'bytes':
                    (root / 'rpm/whisky-0.6.0-1.x86_64.rpm').write_bytes(b'tampered')
                else:
                    path = root / 'deb/packages.json'
                    report = json.loads(path.read_text())
                    report['sourceCommit' if mutation == 'commit' else 'sourceTreeDirty'] = 'b' * 40 if mutation == 'commit' else True
                    path.write_text(json.dumps(report))
                with self.assertRaises(ValueError):
                    release.prepare_assets(root, '0.6.0', COMMIT)

    def test_artifact_paths_cannot_escape_or_follow_symlinks(self):
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder)
            (root / 'file').write_bytes(b'fixture')
            (root / 'alias').symlink_to(root / 'file')
            for name in ('../file', 'alias'):
                with self.assertRaises(ValueError):
                    release.verify_file(root, name, hashlib.sha256(b'fixture').hexdigest())


if __name__ == '__main__':
    unittest.main()
