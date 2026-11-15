#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
"""Validate NativeLunaHook CI artifacts and stage them as the app's LunaHook payload."""
import argparse
import hashlib
import json
from pathlib import Path
import shutil
import struct
import sys

ARCHITECTURES = {'x86': ('32', 0x014C), 'x64': ('64', 0x8664)}
SHARED_FILES = ('LunaTmpFontLoader.dll',)


def expected_files(arch):
    bits = ARCHITECTURES[arch][0]
    return (f'MaltoriaLunaHost{bits}.exe', f'LunaHost{bits}.dll', f'LunaHook{bits}.dll') + SHARED_FILES


def digest(path):
    value = hashlib.sha256()
    with path.open('rb') as stream:
        for block in iter(lambda: stream.read(1 << 20), b''):
            value.update(block)
    return value.hexdigest()


def pe_machine(path):
    with path.open('rb') as stream:
        header = stream.read(4096)
    if len(header) < 64 or header[:2] != b'MZ':
        raise ValueError(f'Not a PE image: {path.name}')
    offset = struct.unpack_from('<I', header, 0x3C)[0]
    if offset + 6 > len(header) or header[offset:offset + 4] != b'PE\0\0':
        raise ValueError(f'Missing PE signature: {path.name}')
    return struct.unpack_from('<H', header, offset + 4)[0]


def stage(source, output, root):
    source, output = Path(source), Path(output)
    if output.exists() or output.is_symlink():
        raise ValueError(f'Choose a new payload directory: {output}')
    files = {}
    for arch, (_, machine) in ARCHITECTURES.items():
        for name in expected_files(arch):
            path = source / arch / name
            if path.is_symlink() or not path.is_file():
                raise ValueError(f'Missing native payload file: {arch}/{name}')
            if pe_machine(path) != machine:
                raise ValueError(f'PE architecture mismatch: {arch}/{name}')
            files[f'{arch}/{name}'] = path
    sources = source / 'sources/Maltoria'
    commit = sources / 'SOURCE-COMMIT.txt'
    if not commit.is_file():
        raise ValueError('Native artifacts must include the corresponding source package')
    for relative in ('ThirdParty/manifest.json', 'THIRD-PARTY-NOTICES.md', 'LICENSE'):
        if not (sources / relative).is_file():
            raise ValueError(f'Native source package is missing {relative}')
    if (sources / 'ThirdParty/manifest.json').read_bytes() != (root / 'ThirdParty/manifest.json').read_bytes():
        raise ValueError('Native source package was built from a different third-party manifest')
    output.mkdir(parents=True)
    inventory = {}
    for relative, path in files.items():
        target = output / relative
        target.parent.mkdir(exist_ok=True)
        shutil.copyfile(path, target)
        target.chmod(0o644)
        inventory[relative] = dict(sha256=digest(target), size=target.stat().st_size)
    shutil.copytree(sources, output / 'sources/Maltoria', symlinks=False)
    manifest = dict(formatVersion=1, component='LunaHook', architectures=sorted(ARCHITECTURES),
                    sourceCommit=commit.read_text().strip(), files=inventory,
                    sources='sources/Maltoria', notices='sources/Maltoria/THIRD-PARTY-NOTICES.md')
    (output / 'manifest.json').write_text(json.dumps(manifest, indent=2, sort_keys=True) + '\n')
    return manifest


def verify(payload):
    payload = Path(payload)
    manifest = json.loads((payload / 'manifest.json').read_text())
    if manifest.get('formatVersion') != 1 or manifest.get('component') != 'LunaHook':
        raise ValueError('Unsupported LunaHook payload manifest')
    expected = {f'{arch}/{name}' for arch in ARCHITECTURES for name in expected_files(arch)}
    if set(manifest['files']) != expected:
        raise ValueError('LunaHook payload inventory is incomplete')
    for relative, entry in manifest['files'].items():
        path = payload / relative
        if path.is_symlink() or not path.is_file() or digest(path) != entry['sha256'] \
                or path.stat().st_size != entry['size']:
            raise ValueError(f'LunaHook payload hash mismatch: {relative}')
        if pe_machine(path) != ARCHITECTURES[relative.split('/')[0]][1]:
            raise ValueError(f'PE architecture mismatch: {relative}')
    if not (payload / manifest['notices']).is_file():
        raise ValueError('LunaHook notices are missing')
    return manifest


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--artifacts', required=True, type=Path,
                        help='Directory holding merged NativeLunaHook artifacts (x86/, x64/, sources/)')
    parser.add_argument('--output', required=True, type=Path)
    args = parser.parse_args(argv)
    try:
        manifest = stage(args.artifacts, args.output, Path(__file__).resolve().parents[1])
        verify(args.output)
    except (OSError, KeyError, ValueError) as error:
        print(error, file=sys.stderr)
        return 1
    print(f"Staged LunaHook payload for {manifest['sourceCommit']}: {args.output}")
    return 0


if __name__ == '__main__':
    sys.exit(main())
