#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
"""Verify third-party notices, locked revisions, preserved sources, and bundles."""
import argparse
import hashlib
import json
from pathlib import Path
import sys

ROOT = Path(__file__).resolve().parent.parent


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def verify(resources=None):
    directory = ROOT / "ThirdParty"
    manifest = json.loads((directory / "manifest.json").read_text())
    errors = []

    def check(condition, message):
        if not condition:
            errors.append(message)

    expected = manifest["files"]
    actual = {p.relative_to(directory).as_posix() for p in directory.rglob("*") if p.is_file()}
    check(actual == set(expected) | {"manifest.json"}, "ThirdParty file inventory changed")
    for name, sha in expected.items():
        path = directory / name
        check(path.is_file() and digest(path) == sha, f"ThirdParty hash mismatch: {name}")
    for name, sha in manifest["repository_files"].items():
        path = ROOT / name
        check(path.is_file() and digest(path) == sha, f"Repository hash mismatch: {name}")
    check((directory / "THIRD-PARTY-NOTICES.md").read_bytes()
          == (ROOT / "THIRD-PARTY-NOTICES.md").read_bytes(), "Notice copies differ")
    check((directory / "licenses/Whisky-GPL-3.0.txt").read_bytes()
          == (ROOT / "LICENSE").read_bytes(), "Whisky GPL copies differ")

    packages = {p["identity"]: p for p in manifest["packages"]}
    for lock in [ROOT / "WhiskyKit/Package.resolved",
                 ROOT / "Whisky.xcodeproj/project.xcworkspace/xcshareddata/swiftpm/Package.resolved"]:
        for pin in json.loads(lock.read_text())["pins"]:
            entry = packages.get(pin["identity"])
            check(entry is not None, f"Dependency requires attribution: {pin['identity']}")
            if entry:
                check(entry["revision"] == pin["state"]["revision"],
                      f"Refresh upstream notices for {pin['identity']}")
                check(entry["version"] == pin["state"]["version"],
                      f"Refresh dependency version for {pin['identity']}")
                check(entry["license_file"] in expected,
                      f"License missing for {pin['identity']}")

    native_packages = manifest["native_packages"]
    check(len({p["name"] for p in native_packages}) == len(native_packages),
          "Duplicate native package name")
    for package in native_packages:
        archive = package["archive"]
        check(package["revision"] and len(package["revision"]) == 40,
              f"Invalid native revision: {package['name']}")
        license_files = [package["license_file"], *package.get("additional_license_files", [])]
        for license_file in license_files:
            check(license_file in expected,
                  f"Native license missing: {package['name']}: {license_file}")
        if package["name"] == "uchardet":
            check(package.get("license") ==
                  "MPL-1.1 OR GPL-2.0-or-later OR LGPL-2.1-or-later",
                  "uchardet license alternatives are inaccurate")
            check(package.get("distribution_license") == "GPL-2.0-or-later",
                  "uchardet must use the GPL-compatible option in this distribution")
        check(archive in expected and expected.get(archive) == package["sha256"],
              f"Native source hash not in file inventory: {package['name']}")
        path = directory / archive
        check(path.is_file() and digest(path) == package["sha256"],
              f"Native source archive hash mismatch: {package['name']}")

    if resources:
        # Compare immutable resources. Code signing changes executable hashes.
        bundled = Path(resources) / "ThirdParty"
        for name in [*expected, "manifest.json"]:
            path = bundled / name
            check(path.is_file() and digest(path) == digest(directory / name),
                  f"Bundled third-party file missing or changed: {name}")
        for name in ["winetricks", "verbs.txt", "cabextract"]:
            check((Path(resources) / name).is_file(), f"Bundled tool missing: {name}")
        for name in ["winetricks", "verbs.txt"]:
            path = Path(resources) / name
            if path.is_file():
                check(digest(path) == digest(ROOT / "Libraries" / name),
                      f"Bundled source/index changed: {name}")

    if errors:
        raise ValueError("\n".join(errors))
    print(f"Third-party notices verified: {len(packages)} Swift packages, "
          f"{len(native_packages)} native packages, {len(expected)} preserved files"
          + (", bundle checked" if resources else ""))


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--resources", type=Path, help="Built app Contents/Resources directory")
    args = parser.parse_args()
    try:
        verify(args.resources)
    except (ValueError, KeyError, OSError) as error:
        print(error, file=sys.stderr)
        sys.exit(1)
