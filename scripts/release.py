#!/usr/bin/env python3
"""Versions and packaging metadata of semver (X.Y.Z) releases."""
import argparse
import hashlib
import json
import plistlib
import re
import subprocess
import tomllib
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
VERSION_PATTERN = re.compile(r"(?:0|[1-9][0-9]*)\.(?:0|[1-9][0-9]*)\.(?:0|[1-9][0-9]*)")
COMMIT_PATTERN = re.compile(r"([a-z][a-z0-9-]*)(?:\([^()\n]+\))?(!)?: [^\n]+")
TRAILER_PATTERN = re.compile(r"(?:[A-Za-z0-9-]+|BREAKING CHANGE)(?:: | #)\S")
BREAKING_PATTERN = re.compile(r"^BREAKING(?: CHANGE|-CHANGE): \S", re.MULTILINE)
RELEASE_PATHS = ("VERSION", "Cargo.toml", "Cargo.lock")
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
        raise ValueError("release version must be X.Y.Z without leading zeros")
    return version


def git(root, *args):
    return subprocess.check_output(["git", "-C", str(root), *args], text=True).strip()


def git_bytes(root, *args):
    return subprocess.check_output(["git", "-C", str(root), *args])


def resolve_commit(root, ref):
    return git(root, "rev-parse", "--verify", "--end-of-options", f"{ref}^{{commit}}")


def commit_bump(message):
    """Classify a Conventional Commit, including multiline breaking-change footers."""
    paragraphs = re.split(r"\n[ \t]*\n", message.strip())
    header = COMMIT_PATTERN.fullmatch(paragraphs[0].split("\n", 1)[0])
    if not header:
        return None
    breaking = any(
        TRAILER_PATTERN.match(paragraph) and BREAKING_PATTERN.search(paragraph)
        for paragraph in paragraphs[1:]
    )
    if header[2] or breaking:
        return "major"
    if header[1] == "feat":
        return "minor"
    if header[1] in ("fix", "perf", "refactor"):
        return "patch"
    return None


def plan_release(root, source="HEAD"):
    """Derive the next release from stable ancestor tags and complete commit messages."""
    source = resolve_commit(root, source)
    tags = git(root, "tag", "--merged", source).splitlines()
    stable = [tag for tag in tags if tag.startswith("v") and VERSION_PATTERN.fullmatch(tag[1:])]
    tag = max(stable, key=lambda value: version_key(value[1:]), default=None)
    previous = tag[1:] if tag else check_version(
        git_bytes(root, "show", f"{source}:VERSION").decode().strip()
    )
    revision = f"refs/tags/{tag}..{source}" if tag else source
    messages = git_bytes(root, "log", "--format=%B%x00", revision).decode().split("\0")
    bumps = {commit_bump(message) for message in messages}
    bump = next((kind for kind in ("major", "minor", "patch") if kind in bumps), None)
    version = None
    if bump:
        major, minor, patch = version_key(previous)
        if bump == "major":
            version = f"{major + 1}.0.0"
        elif bump == "minor":
            version = f"{major}.{minor + 1}.0"
        else:
            version = f"{major}.{minor}.{patch + 1}"
    return {
        "version": version,
        "tag": f"v{version}" if version else None,
        "source": source,
        "previous": previous,
        "bump": bump,
    }


def checkout_version(root):
    # VERSION records the version prepared by the release PR.
    version = (root / "VERSION").read_text().strip()
    return check_version(version)


def version_key(version):
    return tuple(int(part) for part in check_version(version).split("."))


def replace_once(pattern, replacement, text, label):
    updated, count = re.subn(pattern, replacement, text, flags=re.MULTILINE)
    if count != 1:
        raise ValueError(f"expected exactly one {label}, found {count}")
    return updated


def stamped_files(version, manifest, lock, manifests):
    """Return only the three stamped files, leaving dependency resolutions untouched."""
    check_version(version)
    pattern = r'(\[workspace\.package\]\nversion = ")([^"\n]+)(")'
    current = re.search(pattern, manifest)
    if not current:
        raise ValueError("workspace package version not found")
    manifest = replace_once(
        pattern, lambda match: match[1] + version + match[3], manifest, "workspace version"
    )
    if not manifests:
        raise ValueError("workspace crates not found")
    names = set()
    for path, contents in sorted(manifests.items()):
        package = tomllib.loads(contents).get("package", {})
        name = package.get("name")
        if not name:
            raise ValueError(f"package name not found in {path}")
        if package.get("version") != {"workspace": True}:
            raise ValueError(f"{path} must inherit the workspace version")
        if name in names:
            raise ValueError(f"duplicate workspace package {name}")
        names.add(name)
        pattern = (
            r'(\[\[package\]\]\nname = "' + re.escape(name)
            + r'"\nversion = ")' + re.escape(current[2]) + r'(")'
        )
        lock = replace_once(
            pattern, lambda match: match[1] + version + match[2], lock, name
        )
    # Only local package versions change. All third-party resolutions stay locked.
    return {
        "VERSION": (version + "\n").encode(),
        "Cargo.toml": manifest.encode(),
        "Cargo.lock": lock.encode(),
    }


def stamp_version(root, version):
    files = stamped_files(
        version,
        (root / "Cargo.toml").read_bytes().decode(),
        (root / "Cargo.lock").read_bytes().decode(),
        {str(path.relative_to(root)): path.read_bytes().decode()
         for path in root.glob("crates/*/Cargo.toml")},
    )
    for path, contents in files.items():
        (root / path).write_bytes(contents)


def committed_tree(root, commit):
    """Map all committed paths to their mode, object type and object ID."""
    entries = {}
    for entry in git_bytes(root, "ls-tree", "-r", "-z", "--full-tree", commit).split(b"\0"):
        if entry:
            metadata, path = entry.split(b"\t", 1)
            entries[path.decode(errors="surrogateescape")] = tuple(metadata.decode().split())
    return entries


def verify_release(root, source, release):
    """Verify the complete committed tree against the precisely stamped source tree."""
    plan = plan_release(root, source)
    if not plan["version"]:
        raise ValueError("source has no release changes")
    release = resolve_commit(root, release)
    before = committed_tree(root, plan["source"])
    after = committed_tree(root, release)

    def blob(tree, path):
        entry = tree.get(path)
        if not entry or entry[1] != "blob" or entry[0] not in ("100644", "100755"):
            raise ValueError(f"expected a regular committed file: {path}")
        return git_bytes(root, "cat-file", "blob", entry[2])

    originals = {path: blob(before, path) for path in RELEASE_PATHS}
    manifests = {
        path: blob(before, path).decode() for path in before
        if re.fullmatch(r"crates/[^/]+/Cargo\.toml", path)
    }
    expected = stamped_files(
        plan["version"], originals["Cargo.toml"].decode(), originals["Cargo.lock"].decode(),
        manifests,
    )
    changed = {path for path in before.keys() | after.keys() if before.get(path) != after.get(path)}
    expected_changed = {path for path in RELEASE_PATHS if originals[path] != expected[path]}
    if changed != expected_changed:
        raise ValueError(
            "release changes do not match version stamp: "
            f"expected {sorted(expected_changed)}, found {sorted(changed)}"
        )
    for path in RELEASE_PATHS:
        if before[path][:2] != after[path][:2] or blob(after, path) != expected[path]:
            raise ValueError(f"release does not contain the expected stamp: {path}")
    return {**plan, "source": release}


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
    plan = commands.add_parser("plan")
    plan.add_argument("--source", default="HEAD")
    verify = commands.add_parser("verify")
    verify.add_argument("--source", required=True)
    verify.add_argument("--release", required=True)
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
        if args.command == "plan":
            print(json.dumps(plan_release(ROOT, args.source)))
        elif args.command == "verify":
            print(json.dumps(verify_release(ROOT, args.source, args.release)))
        elif args.command == "version":
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
