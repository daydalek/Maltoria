# SPDX-License-Identifier: GPL-3.0-or-later
"""Validate the published Linux Wine payload inside a private staging directory."""
import hashlib
import json
from pathlib import Path, PurePosixPath
import stat
import sys
import tarfile


def digest(path):
    result = hashlib.sha256()
    with path.open('rb') as stream:
        for block in iter(lambda: stream.read(1024 * 1024), b''):
            result.update(block)
    return result.hexdigest()


def extract(archive, destination, expected):
    if digest(archive) != expected:
        raise ValueError('checksum_mismatch')
    data_filter = getattr(tarfile, 'data_filter', None)
    root = destination / 'wine-linux-amd64'
    with tarfile.open(archive, 'r:gz') as source:
        members = []
        size = 0
        for member in source:
            path = PurePosixPath(member.name)
            if (path.is_absolute() or '..' in path.parts or not path.parts
                    or path.parts[0] != root.name
                    or not (member.isfile() or member.isdir() or member.issym() or member.islnk())):
                raise ValueError('invalid_archive_member')
            if data_filter is None and (member.issym() or member.islnk()):
                # Older interpreters lack tarfile.data_filter: reject links whose
                # target leaves the payload root (checked again post-extraction).
                link = PurePosixPath(member.linkname)
                if link.is_absolute() or '..' in link.parts:
                    raise ValueError('invalid_archive_member')
            size += member.size
            members.append(member)
            if size > 8 * 1024**3 or len(members) > 100000:
                raise ValueError('archive_limit_exceeded')
            if data_filter is not None:
                # Preflight against the same boundary that extraction enforces.
                data_filter(member, str(destination))
        if data_filter is not None:
            source.extractall(destination, members=members, filter='data')
        else:
            source.extractall(destination, members=members)
            # data_filter also clamps recorded modes; mirror that on old interpreters.
            for path in destination.rglob('*'):
                if path.is_symlink():
                    continue
                mode = stat.S_IMODE(path.stat().st_mode)
                path.chmod(mode & 0o755 if (path.is_dir() or mode & 0o111) else mode & 0o644)
    if root.is_symlink() or not root.is_dir():
        raise ValueError('invalid_runtime_root')
    # Only links inside this payload can participate in runtime discovery.
    for path in root.rglob('*'):
        if not path.resolve().is_relative_to(root.resolve()):
            raise ValueError('external_runtime_link')
    for name in ('bin/wine', 'bin/wineserver'):
        path = root / name
        with path.open('rb') as stream:
            header = stream.read(20)
        if (len(header) != 20 or header[:6] != b'\x7fELF\x02\x01'
                or header[18:20] != b'\x3e\x00' or not path.stat().st_mode & stat.S_IXUSR):
            raise ValueError('invalid_amd64_runtime')
    launcher = root / 'run'
    if not launcher.is_file() or not launcher.stat().st_mode & stat.S_IXUSR:
        raise ValueError('missing_runtime_launcher')
    print(json.dumps({'root': str(root)}))


if __name__ == '__main__':
    try:
        if sys.argv[1] == 'digest':
            print(digest(Path(sys.argv[2])))
        elif sys.argv[1] == 'extract':
            extract(Path(sys.argv[2]), Path(sys.argv[3]), sys.argv[4])
        else:
            raise ValueError('unknown_archive_operation')
    except Exception as error:
        print(str(error))
        sys.exit(3 if str(error) == 'checksum_mismatch' else 2)
