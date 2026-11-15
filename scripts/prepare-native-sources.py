#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
"""Verify and safely extract locked LunaHook build sources."""
import argparse
import hashlib
import json
from pathlib import Path, PurePosixPath
import shutil
import sys
import tarfile
import tempfile

ROOT = Path(__file__).resolve().parent.parent
MANIFEST = ROOT / "ThirdParty/manifest.json"
PATCH = ROOT / "ThirdParty/lunahook-minhook-pin.patch"
MINHOOK_REVISION = "c3fcafdc10146beb5919319d0683e44e3c30d537"


def digest(path):
    value = hashlib.sha256()
    with path.open("rb") as stream:
        for block in iter(lambda: stream.read(1024 * 1024), b""):
            value.update(block)
    return value.hexdigest()


def safe_extract(archive_path, destination, revision):
    with tarfile.open(archive_path, "r:gz") as archive:
        members = archive.getmembers()
        if not members or len(members) > 100_000:
            raise ValueError(f"Unexpected archive member count: {archive_path.name}")
        roots = set()
        entries = {}
        planned = []
        total_size = 0
        for member in members:
            name = member.name
            if not name or "\0" in name or "\\" in name or name.startswith("/"):
                raise ValueError(f"Unsafe archive path: {member.name}")
            if member.issym() or member.islnk() or not (member.isfile() or member.isdir()):
                raise ValueError(f"Unsupported archive member: {member.name}")
            parts = name.split("/")
            if member.isdir() and parts[-1] == "":
                parts.pop()
            if (not parts or any(part in ("", ".", "..") or ":" in part
                                 for part in parts)):
                raise ValueError(f"Unsafe archive path: {member.name}")
            path = PurePosixPath(name)
            if path.is_absolute() or not path.parts:
                raise ValueError(f"Unsafe archive path: {member.name}")
            roots.add(parts[0])
            if len(parts) == 1:
                if not member.isdir():
                    raise ValueError(f"Archive root is not a directory: {member.name}")
                continue
            relative = tuple(parts[1:])
            if relative in entries:
                raise ValueError(f"Duplicate archive path: {member.name}")
            if member.isfile() and (member.size < 0 or member.size > 100 * 1024 * 1024):
                raise ValueError(f"Oversized archive member: {member.name}")
            total_size += member.size if member.isfile() else 0
            if total_size > 2 * 1024 * 1024 * 1024:
                raise ValueError(f"Oversized archive contents: {archive_path.name}")
            entries[relative] = member.isdir()
            planned.append((member, relative))
        if len(roots) != 1 or not next(iter(roots)).endswith(revision):
            raise ValueError(f"Unexpected archive root for {archive_path.name}: {sorted(roots)}")
        for relative, is_directory in entries.items():
            for count in range(1, len(relative)):
                parent = relative[:count]
                if parent in entries and not entries[parent]:
                    raise ValueError(f"Archive path traverses a file: {'/'.join(relative)}")

        if destination.exists():
            raise ValueError(f"Extraction destination already exists: {destination}")
        destination.parent.mkdir(parents=True, exist_ok=True)
        staging = Path(tempfile.mkdtemp(prefix=f"{destination.name}.extract-",
                                        dir=destination.parent))
        try:
            for member, relative in planned:
                target = staging.joinpath(*relative)
                if member.isdir():
                    target.mkdir(parents=True, exist_ok=True)
                    continue
                target.parent.mkdir(parents=True, exist_ok=True)
                source = archive.extractfile(member)
                if source is None:
                    raise ValueError(f"Missing archive data: {member.name}")
                written = 0
                with source, target.open("xb") as output:
                    while chunk := source.read(1024 * 1024):
                        written += len(chunk)
                        if written > member.size:
                            raise ValueError(f"Unexpected archive data length: {member.name}")
                        output.write(chunk)
                if written != member.size:
                    raise ValueError(f"Truncated archive member: {member.name}")
            staging.replace(destination)
        except BaseException:
            shutil.rmtree(staging, ignore_errors=True)
            raise


def prepare(output):
    manifest = json.loads(MANIFEST.read_text())
    packages = {package["name"]: package for package in manifest["native_packages"]}
    if output.exists() and any(output.iterdir()):
        raise ValueError(f"Output directory must be empty: {output}")
    output.mkdir(parents=True, exist_ok=True)
    for name in packages:
        package = packages[name]
        archive = ROOT / "ThirdParty" / package["archive"]
        if not archive.is_file() or digest(archive) != package["sha256"]:
            raise ValueError(f"Source archive hash mismatch: {package['archive']}")
        if package["sha256"] != manifest["files"].get(package["archive"]):
            raise ValueError(f"Source archive inventory mismatch: {package['archive']}")
        safe_extract(archive, output / package["directory"], package["revision"])

    luna = output / packages["lunatranslator"]["directory"]
    cmake = luna / "src/NativeImpl/LunaHook/LunaHook/CMakeLists.txt"
    text = cmake.read_text(encoding="utf-8")
    patch = PATCH.read_text(encoding="utf-8")
    old = "\tGIT_TAG master\n\tGIT_SHALLOW TRUE"
    new = f"\tGIT_TAG {MINHOOK_REVISION}\n\tGIT_SHALLOW FALSE"
    if (text.count(old) != 1 or patch.count("-\tGIT_TAG master\n-\tGIT_SHALLOW TRUE") != 1
            or f"+\tGIT_TAG {MINHOOK_REVISION}\n+\tGIT_SHALLOW FALSE" not in patch):
        raise ValueError("Pinned MinHook patch no longer matches the locked LunaTranslator source")
    patched = text.replace(old, new)
    cmake.write_text(patched, encoding="utf-8")
    final_text = cmake.read_text(encoding="utf-8")
    if MINHOOK_REVISION not in final_text or "GIT_TAG master" in final_text:
        raise ValueError("MinHook pin patch verification failed")
    print(f"Prepared {len(packages)} pinned native source trees: {output}")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    try:
        prepare(args.output.resolve())
    except (KeyError, OSError, tarfile.TarError, ValueError) as error:
        print(error, file=sys.stderr)
        sys.exit(1)
