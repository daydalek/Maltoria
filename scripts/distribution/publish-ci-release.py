#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
"""Promote verified main-branch CI artifacts to an immutable prerelease."""
import argparse
import base64
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import tempfile

MARKER = '<!-- maltoria-ci-release -->'


def gh(*args, missing=False):
    result = subprocess.run(['gh', *args], capture_output=True, text=True)
    if result.returncode:
        if missing and 'HTTP 404' in result.stderr:
            return None
        # Redirect URLs can contain temporary credentials; keep them out of logs.
        raise RuntimeError('GitHub operation failed: ' + ' '.join(args[:2]))
    return result.stdout


def api(path, missing=False, **fields):
    args = ['api', path]
    if fields:
        args += ['--method', 'POST']
        for key, value in fields.items():
            args += ['-f', f'{key}={value}']
    value = gh(*args, missing=missing)
    return None if value is None else json.loads(value)


def validate_run(run, repository):
    if not (run['status'] == 'completed' and run['conclusion'] == 'success'
            and run['event'] == 'push' and run['head_branch'] == 'main'
            and run['path'] == '.github/workflows/CI.yml'
            and run['repository']['full_name'] == repository
            and run['head_repository']['full_name'] == repository
            and re.fullmatch(r'[0-9a-f]{40}', run['head_sha'])):
        raise ValueError('Only a successful CI push from this repository main branch can publish')


def verify_file(directory, name, expected):
    if not name or Path(name).name != name or name in ('.', '..'):
        raise ValueError('Unsafe artifact filename')
    path = directory / name
    if path.is_symlink() or not path.is_file():
        raise ValueError('Missing regular artifact file: ' + name)
    with path.open('rb') as stream:
        actual = hashlib.file_digest(stream, 'sha256').hexdigest()
    if actual != expected:
        raise ValueError('Artifact checksum mismatch: ' + name)
    return path


def prepare_assets(root, version, commit):
    output = root / 'release'
    output.mkdir()
    mac = root / 'mac'
    info = json.loads((mac / 'build-info.json').read_text())
    if not (info['app_version'] == version and info['architecture'] == 'arm64'
            and info['commit'] == commit and info['checkout_dirty'] is False):
        raise ValueError('macOS artifact version or source mismatch')
    expected_mac = {f'Maltoria-{version}-arm64.zip', f'Maltoria-{version}-sources.tar.gz',
                    'build-info.json', 'RELEASE-NOTES.md'}
    records = {}
    for line in (mac / 'SHA256SUMS').read_text().splitlines():
        digest, name = line.split(maxsplit=1)
        name = name.lstrip('* ')
        if name in records:
            raise ValueError('Duplicate checksum entry')
        records[name] = digest
    if set(records) != expected_mac:
        raise ValueError('Incomplete macOS asset inventory')
    mac_names = []
    for name, digest in records.items():
        path = verify_file(mac, name, digest)
        destination = 'RELEASE-NOTES-macos.md' if name == 'RELEASE-NOTES.md' else name
        shutil.copyfile(path, output / destination)
        mac_names.append(destination)
    with (output / 'SHA256SUMS-macos-arm64').open('w') as checksums:
        for name in mac_names:
            with (output / name).open('rb') as stream:
                checksums.write(hashlib.file_digest(stream, 'sha256').hexdigest() + '  ' + name + '\n')

    expected_linux = {'deb': f'whisky_{version}_amd64.deb',
                      'rpm': f'whisky-{version}-1.x86_64.rpm',
                      'appimage': f'Maltoria-{version}-x86_64.AppImage'}
    reference = None
    for fmt in ('deb', 'rpm', 'appimage'):
        folder = root / fmt
        report = json.loads((folder / 'packages.json').read_text())
        if not (report['passed'] is True and report['version'] == version
                and report['sourceCommit'] == commit and report['sourceTreeDirty'] is False
                and report['architecture'] == 'x86_64'
                and {a['format'] for a in report['assets']} == {'deb', 'rpm', 'appimage'}):
            raise ValueError('Linux artifact version, source or validation mismatch')
        if reference is not None and report != reference:
            raise ValueError('Linux formats originate from different payload reports')
        reference = report
        asset, = [a for a in report['assets'] if a['format'] == fmt]
        if asset['name'] != expected_linux[fmt]:
            raise ValueError('Unexpected Linux package filename')
        path = verify_file(folder, asset['name'], asset['sha256'])
        shutil.copyfile(path, output / path.name)
    for name in ('packages.json', 'elf-abi.json'):
        shutil.copyfile(root / 'deb' / name, output / name)
    # Include hashes for the Linux reports as well as the installable packages.
    linux_names = [a['name'] for a in reference['assets']] + ['packages.json', 'elf-abi.json']
    with (output / 'SHA256SUMS-linux-amd64').open('w') as checksums:
        for name in linux_names:
            with (output / name).open('rb') as stream:
                checksums.write(hashlib.file_digest(stream, 'sha256').hexdigest() + '  ' + name + '\n')
    return output


def publish(repository, run_id):
    if not re.fullmatch(r'[\w.-]+/[\w.-]+', repository) or not run_id.isdigit():
        raise ValueError('Invalid repository or CI run ID')
    run = api(f'repos/{repository}/actions/runs/{run_id}')
    validate_run(run, repository)
    commit = run['head_sha']
    source = api(f'repos/{repository}/contents/apps/whisky-gpui/version.env?ref={commit}')
    versions = re.findall(r'^WHISKY_DEFAULT_VERSION=([0-9]+\.[0-9]+\.[0-9]+)$',
                          base64.b64decode(source['content']).decode(), re.MULTILINE)
    if len(versions) != 1:
        raise ValueError('CI source must declare exactly one package version')
    version, = versions
    tag = 'v' + version
    existing = api(f'repos/{repository}/releases/tags/{tag}', missing=True)
    if existing and not existing['draft']:
        print(f'::notice::Release {tag} is already published; bump version.env for the next release')
        return
    tag_ref = api(f'repos/{repository}/git/ref/tags/{tag}', missing=True)
    target = api(f'repos/{repository}/commits/{tag}') if tag_ref else None
    if target and target['sha'] != commit:
        raise ValueError('Release tag points to a different source; refusing to move it')
    if existing and MARKER not in (existing.get('body') or ''):
        raise ValueError('An unmanaged draft exists; refusing to overwrite it')

    with tempfile.TemporaryDirectory(prefix='maltoria-ci-release-') as temporary:
        root = Path(temporary)
        for folder, artifact in [('mac', f'macos-arm64-{version}'),
                                 ('deb', 'linux-deb'), ('rpm', 'linux-rpm'), ('appimage', 'linux-appimage')]:
            gh('run', 'download', run_id, '--repo', repository, '--name', artifact,
               '--dir', str(root / folder))
        assets = prepare_assets(root, version, commit)
        notes = root / 'notes.md'
        notes.write_text(
            f'{MARKER}\nMaltoria {version} (prerelease).\n\n'
            '- macOS: Apple Silicon ZIP, ad-hoc signed.\n'
            '- Linux amd64: deb (Ubuntu 24.04), rpm (Fedora 44), and AppImage.\n'
            '- Linux baseline: glibc 2.39 or newer; Wine downloads separately.\n'
            '- Custom Wine engines, feature gating and folder executable discovery improvements.\n'
            '- Fixes for engine lease races, process termination timeouts and game folder classification.\n'
            '- Sources, package reports and platform-specific SHA256SUMS are attached.\n\n'
            f'Source: `{commit}`\n\nVerified CI: {run["html_url"]}\n'
        )
        if not target:
            api(f'repos/{repository}/git/refs', ref='refs/tags/' + tag, sha=commit)
        files = [str(p) for p in sorted(assets.iterdir())]
        if not existing:
            gh('release', 'create', tag, '--repo', repository, '--verify-tag', '--draft',
               '--prerelease', '--title', 'Maltoria ' + version, '--notes-file', str(notes), *files)
        else:
            gh('release', 'upload', tag, '--repo', repository, '--clobber', *files)
        gh('release', 'edit', tag, '--repo', repository, '--draft=false', '--prerelease',
           '--notes-file', str(notes))
        url = f'https://github.com/{repository}/releases/tag/{tag}'
        print('Published verified CI packages: ' + url)
        if os.environ.get('GITHUB_STEP_SUMMARY'):
            with open(os.environ['GITHUB_STEP_SUMMARY'], 'a') as stream:
                stream.write(f'Published [{tag}]({url}) from CI [{run_id}]({run["html_url"]}).\n')


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--repository', required=True)
    parser.add_argument('--run-id', required=True)
    args = parser.parse_args()
    publish(args.repository, args.run_id)
