import json
import os
from pathlib import Path
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]

# Execute the real release shell script; only Apple's platform commands and the
# retry delay are substituted. The stateful command records actual invocations.
FAKE_COMMAND = r'''#!/usr/bin/env python3
import json
import os
from pathlib import Path
import sys

root = Path(os.environ["FAKE_ROOT"])
name = Path(sys.argv[0]).name
args = sys.argv[1:]
with (root / "calls.jsonl").open("a") as stream:
    # Never record notarization credentials.
    stream.write(json.dumps([name, *args[:2]]) + "\n")
if name == "xcrun":
    if args[:2] == ["notarytool", "submit"]:
        print(json.dumps({"id": "accepted-submission", "status": "Accepted"}))
    elif args[:2] == ["notarytool", "log"]:
        print("Ready for distribution")
    elif args[:2] == ["stapler", "staple"]:
        state = root / "attempts"
        attempt = int(state.read_text()) + 1 if state.exists() else 1
        state.write_text(str(attempt))
        mode = os.environ["FAKE_STAPLE_MODE"]
        if mode == "permanent":
            print("The staple and validate action failed! Error 68.")
            sys.exit(68)
        if mode == "exhaust" or (mode == "recover" and attempt == 1):
            print('Error Domain=NSURLErrorDomain Code=-1005 "The network connection was lost."', file=sys.stderr)
            sys.exit(68)
    elif args[:2] == ["stapler", "validate"]:
        sys.exit(int(os.environ.get("FAKE_VALIDATE_STATUS", "0")))
    else:
        sys.exit(99)
elif name == "spctl":
    sys.exit(int(os.environ.get("FAKE_SPCTL_STATUS", "0")))
elif name not in ("ditto", "codesign", "sleep"):
    sys.exit(99)
'''


class NotarizationTests(unittest.TestCase):
    def run_release(self, suffix, mode, **overrides):
        directory = tempfile.TemporaryDirectory(prefix="murvix-notarization-")
        self.addCleanup(directory.cleanup)
        root = Path(directory.name)
        bin_dir = root / "bin"
        bin_dir.mkdir()
        for name in ("xcrun", "ditto", "codesign", "spctl", "sleep"):
            command = bin_dir / name
            command.write_text(FAKE_COMMAND)
            command.chmod(0o755)
        artifact = root / ("Signed application" + suffix)
        if suffix == ".app":
            artifact.mkdir()
        else:
            artifact.touch()
        environment = os.environ.copy()
        environment.pop("NOTARY_KEYCHAIN_PROFILE", None)
        environment.update(
            PATH=str(bin_dir) + os.pathsep + environment["PATH"],
            FAKE_ROOT=str(root),
            FAKE_STAPLE_MODE=mode,
            APPLE_ID="private-apple-id@example.invalid",
            APPLE_APP_SPECIFIC_PASSWORD="private-test-password",
            APPLE_TEAM_ID="private-team-id",
            **overrides,
        )
        result = subprocess.run(
            ["bash", str(ROOT / "scripts/notarize-release.sh"), str(artifact)],
            env=environment, capture_output=True, text=True, check=False,
        )
        calls = [json.loads(line) for line in (root / "calls.jsonl").read_text().splitlines()]
        output = result.stdout + result.stderr
        for secret in (environment["APPLE_ID"], environment["APPLE_APP_SPECIFIC_PASSWORD"], environment["APPLE_TEAM_ID"]):
            self.assertNotIn(secret, output)
        self.assertEqual(calls.count(["xcrun", "notarytool", "submit"]), 1)
        self.assertEqual(calls.count(["xcrun", "notarytool", "log"]), 1)
        return result, calls

    def test_transient_ticket_network_failure_recovers_for_app_and_dmg(self):
        for suffix in (".app", ".dmg"):
            with self.subTest(suffix=suffix):
                result, calls = self.run_release(suffix, "recover")
                self.assertEqual(result.returncode, 0, result.stderr)
                self.assertEqual(calls.count(["xcrun", "stapler", "staple"]), 2)
                self.assertEqual(calls.count(["sleep", "5"]), 1)
                self.assertIn("NSURLErrorDomain Code=-1005", result.stdout)
                self.assertIn(["xcrun", "stapler", "validate"], calls)
                self.assertIn(["spctl", "--assess", "--type"], calls)
                if suffix == ".app":
                    self.assertIn(["codesign", "--verify", "--deep"], calls)
                self.assertLess(calls.index(["xcrun", "stapler", "validate"]), calls.index(["spctl", "--assess", "--type"]))

    def test_permanent_ticket_failure_is_not_retried(self):
        result, calls = self.run_release(".dmg", "permanent")
        self.assertEqual(result.returncode, 68)
        self.assertEqual(calls.count(["xcrun", "stapler", "staple"]), 1)
        self.assertNotIn(["sleep", "5"], calls)
        self.assertNotIn(["xcrun", "stapler", "validate"], calls)
        self.assertNotIn(["spctl", "--assess", "--type"], calls)
        self.assertIn("Error 68", result.stdout)

    def test_transient_failure_stops_after_three_attempts(self):
        result, calls = self.run_release(".app", "exhaust")
        self.assertEqual(result.returncode, 68)
        self.assertEqual(calls.count(["xcrun", "stapler", "staple"]), 3)
        self.assertEqual(calls.count(["sleep", "5"]), 2)
        self.assertNotIn(["xcrun", "stapler", "validate"], calls)
        self.assertNotIn(["spctl", "--assess", "--type"], calls)

    def test_validation_and_gatekeeper_failures_remain_fatal(self):
        for suffix in (".app", ".dmg"):
            for gate, status in (("FAKE_VALIDATE_STATUS", 23), ("FAKE_SPCTL_STATUS", 24)):
                with self.subTest(suffix=suffix, gate=gate):
                    result, calls = self.run_release(suffix, "recover", **{gate: str(status)})
                    self.assertEqual(result.returncode, status)
                    self.assertIn(["xcrun", "stapler", "validate"], calls)
                    if gate == "FAKE_VALIDATE_STATUS":
                        self.assertNotIn(["spctl", "--assess", "--type"], calls)
                    else:
                        self.assertIn(["spctl", "--assess", "--type"], calls)


if __name__ == "__main__":
    unittest.main()
