import importlib.util
import os
import re
import shutil
import subprocess
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[2]
SPEC = importlib.util.spec_from_file_location("release", ROOT / "scripts/release.py")
release = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(release)


class SemverVersionTests(unittest.TestCase):
    def test_version_is_beta_version_file_contents(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            (root / "BETA_VERSION").write_text("2.1.0\n")
            self.assertEqual(release.checkout_version(root), "2.1.0")
            for invalid in ("", "2.1", "v2.1.0", "2.1.0-beta", "2.1.0-beta.7"):
                (root / "BETA_VERSION").write_text(invalid)
                with self.subTest(base=invalid), self.assertRaises(ValueError):
                    release.checkout_version(root)

    def test_release_versions_reject_channels_and_unsafe_inputs(self):
        for version in (
            "2.0.0-beta.0", "2.0.0-beta.001", "2.0.0-beta-1", "2.0-beta.1", "2.0.0.0-beta.1",
            "2.0.0-alpha.1", "2.0.0-beta.1/../x", "2.0.0-beta.1\n", "2.0.0-beta.1-beta.2",
            "2.0.0-beta.7", "2.0.0-rc.1", "2.1", "v2.1.0", "2.1.0\n", "",
        ):
            with self.subTest(version=version), self.assertRaises(ValueError):
                release.check_version(version)
        self.assertEqual(release.check_version("2.0.0"), "2.0.0")
        self.assertEqual(release.check_version("2.1.0"), "2.1.0")

    def test_only_semver_is_valid(self):
        self.assertEqual(release.check_version("2.1.0"), "2.1.0")
        for version in ("2.0.0-beta-0", "2.1", "v2.1.0", "2.1.0-rc.1", "2.1.0\n"):
            with self.subTest(version=version), self.assertRaises(ValueError):
                release.check_version(version)

    def test_app_identity_is_unconditional(self):
        keys = ("CFBundleName", "CFBundleIdentifier", "CFBundleShortVersionString", "CFBundleVersion")
        info = release.bundle_info("2.1.0", "1234")
        self.assertEqual([info[key] for key in keys], ["Murvix", "com.murvix.app", "2.1.0", "1234"])
        self.assertNotIn("SUPublicEDKey", info)
        self.assertIn("NSLocalNetworkUsageDescription", info)
        self.assertEqual(info["NSAppTransportSecurity"], {"NSAllowsArbitraryLoadsInWebContent": True})
        for version, build in (("2.1.0-beta.7", "7"), ("2.1.0", "0"), ("2.1.0", "x"), ("2.1", "3")):
            with self.subTest(version=version, build=build), self.assertRaises(ValueError):
                release.bundle_info(version, build)

    def test_cli_has_no_channel_commands(self):
        for command in ("channel", "promote", "build-number"):
            with self.subTest(command=command):
                result = subprocess.run(
                    ["python3", str(ROOT / "scripts/release.py"), command],
                    capture_output=True, text=True)
                self.assertNotEqual(result.returncode, 0)
                self.assertIn("invalid choice", result.stderr)


class StampTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        for source in [ROOT / "Cargo.toml", ROOT / "Cargo.lock", *ROOT.glob("crates/*/Cargo.toml")]:
            destination = self.root / source.relative_to(ROOT)
            destination.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(source, destination)

    def test_stamp_changes_only_workspace_versions_and_is_repeatable(self):
        target = "2.1.0"
        release.stamp_version(self.root, target)
        first_stamp = {
            Path("Cargo.toml"): (self.root / "Cargo.toml").read_text(),
            Path("Cargo.lock"): (self.root / "Cargo.lock").read_text(),
        }
        for manifest in self.root.glob("crates/*/Cargo.toml"):
            first_stamp[manifest.relative_to(self.root)] = manifest.read_text()
        release.stamp_version(self.root, target)
        for path, text in first_stamp.items():
            self.assertEqual((self.root / path).read_text(), text, f"{path} changed on second stamp")
        self.assertIn(f'version = "{target}"', first_stamp[Path("Cargo.toml")])
        lock = first_stamp[Path("Cargo.lock")]
        for manifest in self.root.glob("crates/*/Cargo.toml"):
            name = re.search(r'^name = "([^"]+)"', manifest.read_text(), re.MULTILINE)[1]
            self.assertIn(f'name = "{name}"\nversion = "{target}"', lock)


if __name__ == "__main__":
    unittest.main()
