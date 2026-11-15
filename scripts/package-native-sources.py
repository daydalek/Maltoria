#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
"""Stage the exact repository inputs needed to reproduce a native build."""
import argparse
import hashlib
import json
from pathlib import Path
import re
import shutil
import subprocess
import sys

ROOT = Path(__file__).resolve().parent.parent
SOURCE_PATHS = (
    Path(".gitattributes"),
    Path(".github/workflows/NativeLunaHook.yml"),
    Path("LICENSE"),
    Path("Libraries/cabextract"),
    Path("Libraries/verbs.txt"),
    Path("Libraries/winetricks"),
    Path("Native/MaltoriaLunaHost"),
    Path("THIRD-PARTY-NOTICES.md"),
    Path("ThirdParty"),
    Path("Whisky.xcodeproj/project.xcworkspace/xcshareddata/swiftpm/Package.resolved"),
    Path("WhiskyKit/Package.resolved"),
    Path("scripts/build-lunahook.ps1"),
    Path("scripts/check-third-party.py"),
    Path("scripts/distribution/native_host_protocol.py"),
    Path("scripts/distribution/test_native_host_protocol.py"),
    Path("scripts/distribution/test_native_sources.py"),
    Path("scripts/package-native-sources.py"),
    Path("scripts/prepare-native-sources.py"),
    Path("scripts/windows/smoke_native_host.py"),
)


def digest(path):
    value = hashlib.sha256()
    with path.open("rb") as source:
        for block in iter(lambda: source.read(1024 * 1024), b""):
            value.update(block)
    return value.hexdigest()


def package(output, revision=None):
    output = Path(output)
    if output.exists() and any(output.iterdir()):
        raise ValueError(f"Output directory must be empty: {output}")
    output.mkdir(parents=True, exist_ok=True)
    if revision is None:
        revision = subprocess.check_output(
            ["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip()
    if not re.fullmatch(r"[0-9a-f]{40}", revision):
        raise ValueError("Source commit identity must be a full lowercase Git SHA")

    manifest_path = ROOT / "ThirdParty/manifest.json"
    manifest = json.loads(manifest_path.read_text())
    for relative, expected in manifest["files"].items():
        source = ROOT / "ThirdParty" / relative
        if not source.is_file() or digest(source) != expected:
            raise ValueError(f"Third-party source differs from manifest: {relative}")
    for relative, expected in manifest["repository_files"].items():
        source = ROOT / relative
        if not source.is_file() or digest(source) != expected:
            raise ValueError(f"Repository source differs from manifest: {relative}")

    source_root = output / "Maltoria"
    source_root.mkdir()
    for relative in SOURCE_PATHS:
        source = ROOT / relative
        if not source.exists():
            raise ValueError(f"Required native build source is missing: {relative}")
        destination = source_root / relative
        destination.parent.mkdir(parents=True, exist_ok=True)
        if source.is_dir():
            shutil.copytree(source, destination)
        else:
            shutil.copy2(source, destination)
    (source_root / "SOURCE-COMMIT.txt").write_text(revision + "\n", encoding="ascii")
    print(f"Packaged reproducible native sources for {revision}: {source_root}")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--revision")
    args = parser.parse_args()
    try:
        package(args.output, args.revision)
    except (KeyError, OSError, subprocess.CalledProcessError, ValueError) as error:
        print(error, file=sys.stderr)
        sys.exit(1)
