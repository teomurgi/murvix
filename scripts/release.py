#!/usr/bin/env python3
"""Versions and packaging metadata of semver (X.Y.Z) releases."""
import argparse
import hashlib
import json
import plistlib
import re
import subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
VERSION_PATTERN = re.compile(r"[0-9]+\.[0-9]+\.[0-9]+")
APP_NAME = "Murvix"
APP_IDENTIFIER = "com.murvix.app"
# Linux is branded as Murvix and uses the same application ID as macOS.
LINUX_APPS = {"stable": (APP_NAME, APP_IDENTIFIER)}


def build_metadata(root=ROOT):
    """The --build-info every executable of this checkout reports."""
    build = (root / "crates/murvix-protocol/src/build.rs").read_text()
    version = (root / "crates/murvix-protocol/src/version.rs").read_text()
    current = re.search(r"pub const CURRENT: Version = V\d+;", version)[1]
    number = int(re.search(rf"pub const {current}: Version = Version\((\d+)\);", version)[1])
    return {
        "compatibility": int(re.search(r"pub const COMPATIBILITY: u64 = (\d+);", build)[1]),
        "protocol": [number],
    }


def check_version(version):
    if not VERSION_PATTERN.fullmatch(version or ""):
        raise ValueError("release version must be X.Y.Z")
    return version


def git(root, *args):
    return subprocess.check_output(["git", "-C", str(root), *args], text=True).strip()


def checkout_version(root):
    # Releases are versioned as plain X.Y.Z. The BETA_VERSION file holds the
    # current version (kept under its historic name; workflows read it too).
    version = (root / "BETA_VERSION").read_text().strip()
    return check_version(version)


def version_key(version):
    return tuple(int(part) for part in check_version(version).split("."))


def replace_once(pattern, replacement, text, label):
    updated, count = re.subn(pattern, replacement, text, flags=re.MULTILINE)
    if count != 1:
        raise ValueError(f"expected exactly one {label}, found {count}")
    return updated


def stamp_version(root, version):
    check_version(version)
    manifest_path = root / "Cargo.toml"
    lock_path = root / "Cargo.lock"
    manifest = manifest_path.read_text()
    pattern = r'(\[workspace\.package\]\nversion = ")([^"\n]+)(")'
    current = re.search(pattern, manifest)
    if not current:
        raise ValueError("workspace package version not found")
    manifest = replace_once(
        pattern, lambda match: match[1] + version + match[3], manifest, "workspace version"
    )
    lock = lock_path.read_text()
    manifests = sorted(root.glob("crates/*/Cargo.toml"))
    if not manifests:
        raise ValueError("workspace crates not found")
    for crate in manifests:
        name = re.search(r'^name = "([^"]+)"$', crate.read_text(), re.MULTILINE)
        if not name:
            raise ValueError(f"package name not found in {crate}")
        pattern = (
            r'(\[\[package\]\]\nname = "' + re.escape(name[1])
            + r'"\nversion = ")' + re.escape(current[2]) + r'(")'
        )
        lock = replace_once(
            pattern, lambda match: match[1] + version + match[2], lock, name[1]
        )
    # Only local package versions change. All third-party resolutions stay locked.
    manifest_path.write_text(manifest)
    lock_path.write_text(lock)


def bundle_info(version, build):
    """Info.plist of the app. Its bundle version is the build number passed via --build."""
    check_version(version)
    if not re.fullmatch(r"[1-9][0-9]*", build or ""):
        raise ValueError("build number must be a positive integer")
    return {
        "CFBundleDevelopmentRegion": "en",
        "CFBundleDisplayName": APP_NAME,
        "CFBundleName": APP_NAME,
        "CFBundleExecutable": "murvix-app",
        "CFBundleIdentifier": APP_IDENTIFIER,
        "CFBundleIconFile": "AppIcon",
        "CFBundleInfoDictionaryVersion": "6.0",
        "CFBundlePackageType": "APPL",
        "CFBundleShortVersionString": version,
        "CFBundleVersion": build,
        "CFBundleGetInfoString": f"{APP_NAME} {version}",
        "MurvixVersion": version,
        "LSMinimumSystemVersion": "14.0",
        "LSApplicationCategoryType": "public.app-category.developer-tools",
        "NSHighResolutionCapable": True,
        "NSSupportsAutomaticGraphicsSwitching": True,
        "NSPrincipalClass": "NSApplication",
        "NSMicrophoneUsageDescription": "Murvix uses your microphone to dictate text into Composer.",
        "NSSpeechRecognitionUsageDescription": "Murvix transcribes your dictation on this device and inserts it into Composer.",
        "NSLocalNetworkUsageDescription": "Commands you run in Murvix terminals can connect to devices on your local network.",
        "NSAppTransportSecurity": {"NSAllowsArbitraryLoadsInWebContent": True},
    }


def update_metadata(version, repository, directory):
    check_version(version)
    if not re.fullmatch(r"[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+", repository):
        raise ValueError("invalid GitHub repository")
    platforms = {}
    for platform, arch in (("macos-aarch64", "arm64"), ("macos-x86_64", "x86_64")):
        filename = f"Murvix-{version}-{arch}.dmg"
        if arch == "x86_64" and not (directory / filename).exists():
            continue
        size = (directory / filename).stat().st_size
        if not 0 < size <= 2 * 1024**3:
            raise ValueError(f"invalid update size for {arch}")
        platforms[platform] = {
            "url": f"https://github.com/{repository}/releases/download/v{version}/{filename}",
            "size": size,
        }
    for platform, arch in (("linux-aarch64", "arm64"), ("linux-x86_64", "x86_64")):
        filename = f"murvix-desktop-{version}-linux-{arch}.tar.gz"
        path = directory / filename
        size = path.stat().st_size
        if not 0 < size <= 2 * 1024**3 or path.is_symlink():
            raise ValueError(f"invalid desktop update for {arch}")
        checksum = hashlib.sha256()
        with path.open("rb") as source:
            for block in iter(lambda: source.read(1024 * 1024), b""):
                checksum.update(block)
        platforms[platform] = {
            "url": f"https://github.com/{repository}/releases/download/v{version}/{filename}",
            "size": size,
            "sha256": checksum.hexdigest(),
        }
    return {"schema": 1, "version": version, "platforms": platforms}


def check_build(version, executable):
    check_version(version)
    # The first run of an x86_64 build on Apple silicon waits for Rosetta to translate it.
    metadata = json.loads(subprocess.check_output([str(executable), "--build-info"], timeout=60))
    if metadata != {"version": version, **build_metadata()}:
        raise ValueError("Packaged executable build metadata does not match this release")


def main():
    parser = argparse.ArgumentParser()
    commands = parser.add_subparsers(dest="command", required=True)
    commands.add_parser("version")
    check = commands.add_parser("check-build")
    check.add_argument("version")
    check.add_argument("executable", type=Path)
    metadata = commands.add_parser("update")
    metadata.add_argument("version")
    metadata.add_argument("repository")
    metadata.add_argument("directory", type=Path)
    for command in ("stamp", "check-version", "app-name", "plist"):
        subparser = commands.add_parser(command)
        subparser.add_argument("version")
        if command == "plist":
            subparser.add_argument("output", type=Path)
            subparser.add_argument("--build", required=True, help="build number recorded as CFBundleVersion")
    args = parser.parse_args()
    try:
        if args.command == "version":
            version = checkout_version(ROOT)
            print(f"version={version}")
            print(f"tag=v{version}")
        elif args.command == "check-build":
            check_build(args.version, args.executable)
        elif args.command == "stamp":
            stamp_version(ROOT, args.version)
        elif args.command == "check-version":
            check_version(args.version)
        elif args.command == "app-name":
            check_version(args.version)
            print(APP_NAME)
        elif args.command == "update":
            metadata = update_metadata(args.version, args.repository, args.directory)
            (args.directory / "update.json").write_text(json.dumps(metadata, indent=2) + "\n")
        else:
            args.output.write_bytes(plistlib.dumps(bundle_info(args.version, args.build)))
    except (ValueError, OSError, subprocess.CalledProcessError) as error:
        parser.exit(1, f"error: {error}\n")


if __name__ == "__main__":
    main()
