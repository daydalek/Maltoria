#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
"""Verify and safely extract locked LunaHook build sources."""
import argparse
import hashlib
import json
from pathlib import Path, PurePosixPath
import sys
import tarfile

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
    destination.mkdir(parents=True)
    with tarfile.open(archive_path, "r:gz") as archive:
        members = archive.getmembers()
        if not members or len(members) > 100_000:
            raise ValueError(f"Unexpected archive member count: {archive_path.name}")
        roots = set()
        seen = set()
        for member in members:
            path = PurePosixPath(member.name)
            if (path.is_absolute() or ".." in path.parts or "\\" in member.name
                    or not path.parts or ":" in path.parts[0]):
                raise ValueError(f"Unsafe archive path: {member.name}")
            if member.issym() or member.islnk() or not (member.isfile() or member.isdir()):
                raise ValueError(f"Unsupported archive member: {member.name}")
            roots.add(path.parts[0])
            relative = Path(*path.parts[1:])
            if not relative.parts:
                continue
            if relative in seen:
                raise ValueError(f"Duplicate archive path: {member.name}")
            seen.add(relative)
            target = destination / relative
            if member.isdir():
                target.mkdir(parents=True, exist_ok=True)
                continue
            if member.size > 100 * 1024 * 1024:
                raise ValueError(f"Oversized archive member: {member.name}")
            target.parent.mkdir(parents=True, exist_ok=True)
            source = archive.extractfile(member)
            if source is None:
                raise ValueError(f"Missing archive data: {member.name}")
            with source, target.open("xb") as output:
                output.write(source.read())
        if len(roots) != 1 or revision not in next(iter(roots)):
            raise ValueError(f"Unexpected archive root for {archive_path.name}: {sorted(roots)}")


def prepare(output):
    manifest = json.loads(MANIFEST.read_text())
    packages = {package["name"]: package for package in manifest["native_packages"]}
    if output.exists() and any(output.iterdir()):
        raise ValueError(f"Output directory must be empty: {output}")
    output.mkdir(parents=True, exist_ok=True)
    for name in ("lunatranslator", "minhook", "uchardet"):
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
    print(f"Prepared 3 pinned native source trees: {output}")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    try:
        prepare(args.output.resolve())
    except (KeyError, OSError, tarfile.TarError, ValueError) as error:
        print(error, file=sys.stderr)
        sys.exit(1)
