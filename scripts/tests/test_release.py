import json
import os
import shutil
import subprocess
import sys
import tempfile
import tomllib
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "scripts"))
import release as RELEASE


class BuildMetadataTests(unittest.TestCase):
    def test_packaged_protocol_uses_current_version_not_a_historical_declaration(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            source = root / "crates/murvix-protocol/src"
            source.mkdir(parents=True)
            (source / "version.rs").write_text(
                "pub const V1: Version = Version(1);\n"
                "pub const V12: Version = Version(12);\n"
                "pub const CURRENT: Version = V12;\n"
            )
            (source / "build.rs").write_text("pub const COMPATIBILITY: u64 = 37;\n")
            self.assertEqual(
                RELEASE.build_metadata(root), {"compatibility": 37, "protocol": [12]}
            )


class ReleaseScenarioTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.env = {
            **os.environ,
            "GIT_CONFIG_NOSYSTEM": "1",
            "GIT_CONFIG_GLOBAL": os.devnull,
            "GIT_AUTHOR_NAME": "Release Test",
            "GIT_AUTHOR_EMAIL": "release@example.invalid",
            "GIT_COMMITTER_NAME": "Release Test",
            "GIT_COMMITTER_EMAIL": "release@example.invalid",
            "GIT_AUTHOR_DATE": "2025-01-01T00:00:00+00:00",
            "GIT_COMMITTER_DATE": "2025-01-01T00:00:00+00:00",
        }
        self.write("VERSION", "0.2.2\n")
        self.write("Cargo.toml", '''[workspace]
resolver = "3"
members = ["crates/example-app", "crates/example-core"]

[workspace.package]
version = "0.2.2"
edition = "2024"
''')
        self.write("crates/example-app/Cargo.toml", '''[package]
name = "example-app"
version.workspace = true
edition.workspace = true

[dependencies]
example-core = { path = "../example-core" }
serde = "1"
''')
        self.write("crates/example-core/Cargo.toml", '''[package]
name = "example-core"
version.workspace = true
edition.workspace = true
''')
        self.write("crates/example-app/src/main.rs", "fn main() {}\n")
        self.write("crates/example-core/src/lib.rs", "pub const NAME: &str = \"core\";\n")
        self.write("Cargo.lock", '''version = 4

[[package]]
name = "example-app"
version = "0.2.2"
dependencies = [
 "example-core",
 "serde",
]

[[package]]
name = "example-core"
version = "0.2.2"

[[package]]
name = "serde"
version = "1.0.219"
source = "registry+https://github.com/rust-lang/crates.io-index"
checksum = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"

[[package]]
name = "version-matched-dependency"
version = "0.2.2"
source = "registry+https://github.com/rust-lang/crates.io-index"
checksum = "abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789"
''')
        self.script = self.root / "scripts/release.py"
        self.script.parent.mkdir()
        shutil.copyfile(ROOT / "scripts/release.py", self.script)
        self.git("init", "--initial-branch=main", "--template=")
        self.git("config", "commit.gpgsign", "false")
        self.sequence = 0
        self.baseline = self.commit("chore: initialize workspace")

    def write(self, path, content):
        destination = self.root / path
        destination.parent.mkdir(parents=True, exist_ok=True)
        destination.write_text(content)

    def git(self, *args):
        result = subprocess.run(
            ["git", "-C", str(self.root), *args],
            env=self.env, capture_output=True, text=True, check=True,
        )
        return result.stdout.strip()

    def commit(self, message, change_source=True):
        if change_source:
            self.sequence += 1
            self.write("source.txt", f"revision {self.sequence}\n")
        self.git("add", "--all")
        self.git("commit", "--quiet", "-m", message)
        return self.git("rev-parse", "HEAD")

    def cli(self, *args, success=True):
        result = subprocess.run(
            [sys.executable, str(self.script), *args],
            cwd=self.root, env=self.env, capture_output=True, text=True,
        )
        if success:
            self.assertEqual(result.returncode, 0, result.stderr)
        else:
            self.assertNotEqual(result.returncode, 0, result.stdout)
        return result

    def plan(self, source="HEAD"):
        return json.loads(self.cli("plan", "--source", source).stdout)

    def snapshot(self):
        files = {
            path.relative_to(self.root).as_posix(): path.read_bytes()
            for path in self.root.rglob("*")
            if path.is_file() and ".git" not in path.relative_to(self.root).parts
        }
        return self.git("rev-parse", "HEAD"), self.git("status", "--porcelain=v1"), files

    def assert_plan(self, source, previous, bump, version):
        self.assertEqual(self.plan(source), {
            "version": version,
            "tag": f"v{version}" if version else None,
            "source": self.git("rev-parse", f"{source}^{{commit}}"),
            "previous": previous,
            "bump": bump,
        })

    def test_conventional_commit_types_and_breaking_trailers(self):
        self.git("tag", "v0.2.2")
        cases = (
            ("fix(ui): preserve selection", "patch", "0.2.3"),
            ("perf: avoid redundant allocation", "patch", "0.2.3"),
            ("refactor(core): simplify session ownership", "patch", "0.2.3"),
            ("feat(cli): add workspace selection", "minor", "0.3.0"),
            ("feat(api)!: replace session identifiers", "major", "1.0.0"),
            ("fix!: remove obsolete API", "major", "1.0.0"),
            ("feat: replace identifiers\n\nMigration details.\n\n"
             "BREAKING CHANGE: callers must send UUIDs\n instead of integers.\n"
             "Reviewed-by: Release Test <release@example.invalid>", "major", "1.0.0"),
            ("docs: document migration\n\nBREAKING-CHANGE: remove legacy protocol", "major", "1.0.0"),
            ("fix: replace identifiers\n\nBREAKING CHANGE: old identifiers are removed\n\n"
             "Migration details span a second paragraph.", "major", "1.0.0"),
            ("fix: replace identifiers\n\nBREAKING CHANGE: use UUIDs\n\n"
             "Refs: #42", "major", "1.0.0"),
        )
        for index, (message, bump, version) in enumerate(cases):
            with self.subTest(message=message):
                self.git("checkout", "--quiet", "-b", f"case-{index}", self.baseline)
                source = self.commit(message)
                self.assert_plan(source, "0.2.2", bump, version)

    def test_highest_bump_wins_across_commit_bodies(self):
        self.git("tag", "v0.2.2")
        self.commit("fix: repair rendering")
        self.commit("feat: add terminal search")
        self.commit("perf: cache glyph lookup")
        self.assert_plan("HEAD", "0.2.2", "minor", "0.3.0")
        self.commit("refactor(protocol): replace wire format\n\n"
                    "BREAKING CHANGE: protocol messages require a new envelope")
        source = self.commit("fix: repair follow-up regression")
        self.assert_plan(source, "0.2.2", "major", "1.0.0")

    def test_highest_reachable_stable_tag_is_baseline(self):
        self.git("tag", "v0.2.2")
        highest = self.commit("feat!: introduce stable API")
        self.git("tag", "--annotate", "v1.4.0", "-m", "Stable baseline", highest)
        lower = self.commit("docs: explain stable API")
        self.git("tag", "v1.3.9", lower)
        source = self.commit("fix: preserve compatibility")
        self.git("tag", "v9.0.0-rc.1", source)
        self.git("tag", "v8.0.0+build.1", source)
        self.git("tag", "v07.0.0", source)
        self.git("checkout", "--quiet", "-b", "unrelated", self.baseline)
        unrelated = self.commit("feat!: unrelated future API")
        self.git("tag", "v99.0.0", unrelated)
        self.git("checkout", "--quiet", "main")
        self.assert_plan("main", "1.4.0", "patch", "1.4.1")

    def test_docs_chore_and_nonconventional_history_do_not_release(self):
        self.git("tag", "v0.2.2")
        for message in (
            "docs: clarify setup",
            "chore(deps): maintain tooling",
            "test: expand fixture coverage",
            "ci: update workflow permissions",
            "Update terminal code without a conventional subject",
            "Merge pull request #17 from example/topic",
            "docs: show migration syntax\n\nExample text:\n"
            "BREAKING CHANGE: shown here as documentation.\n"
            "This line is ordinary prose.\n\nThere is no API change.",
        ):
            source = self.commit(message)
        self.assert_plan(source, "0.2.2", None, None)

    def test_without_stable_tags_uses_source_version_and_full_history(self):
        self.commit("feat: add session list")
        source = self.commit("docs: describe session list")
        self.git("tag", "v20.0.0-beta.1")
        self.cli("stamp", "9.9.9")
        self.commit("chore: prepare unrelated checkout version")
        before = self.snapshot()
        self.assert_plan(source, "0.2.2", "minor", "0.3.0")
        self.assertEqual(self.snapshot(), before)

    def test_without_stable_tags_and_release_changes_does_not_release(self):
        self.commit("docs: describe development setup")
        self.assert_plan("HEAD", "0.2.2", None, None)

    def test_plan_does_not_mutate_dirty_worktree_or_index(self):
        self.git("tag", "v0.2.2")
        source = self.commit("fix: repair terminal resize")
        self.write("source.txt", "staged local edit\n")
        self.git("add", "source.txt")
        self.write("source.txt", "unstaged local edit\n")
        self.write("untracked.txt", "keep me\n")
        before = self.snapshot(), self.git("write-tree")
        self.assert_plan(source, "0.2.2", "patch", "0.2.3")
        self.assertEqual((self.snapshot(), self.git("write-tree")), before)

    def test_stamp_synchronizes_workspace_without_dependency_resolution_changes(self):
        manifests = {
            path.relative_to(self.root): path.read_bytes()
            for path in self.root.glob("crates/*/Cargo.toml")
        }
        original_lock = (self.root / "Cargo.lock").read_text()
        original_packages = tomllib.loads(original_lock)["package"]
        original_third_party = original_lock[original_lock.index('[[package]]\nname = "serde"'):]
        self.cli("stamp", "1.2.3")
        self.assertEqual((self.root / "VERSION").read_text(), "1.2.3\n")
        workspace = tomllib.loads((self.root / "Cargo.toml").read_text())
        self.assertEqual(workspace["workspace"]["package"]["version"], "1.2.3")
        stamped_lock = (self.root / "Cargo.lock").read_text()
        packages = tomllib.loads(stamped_lock)["package"]
        for original, stamped in zip(original_packages, packages, strict=True):
            expected = dict(original)
            if original["name"] in ("example-app", "example-core"):
                expected["version"] = "1.2.3"
            self.assertEqual(stamped, expected)
        self.assertEqual(stamped_lock[stamped_lock.index('[[package]]\nname = "serde"'):], original_third_party)
        for path, content in manifests.items():
            self.assertEqual((self.root / path).read_bytes(), content)
        self.assertEqual(set(self.git("diff", "--name-only").splitlines()),
                         {"VERSION", "Cargo.toml", "Cargo.lock"})
        first = self.snapshot()
        self.cli("stamp", "1.2.3")
        self.assertEqual(self.snapshot(), first)

    def test_invalid_stamp_versions_fail_without_partial_changes(self):
        before = self.snapshot()
        for version in ("", "1.2", "v1.2.3", "1.2.3-beta.1", "01.2.3", "1.02.3", "1.2.03", "1.2.3\n"):
            with self.subTest(version=version):
                self.cli("stamp", version, success=False)
                self.assertEqual(self.snapshot(), before)

    def prepare_release(self):
        self.git("tag", "v0.2.2")
        source = self.commit("feat: add workspace selection")
        self.git("checkout", "--quiet", "-b", "release/v0.3.0")
        self.cli("stamp", "0.3.0")
        self.git("add", "VERSION", "Cargo.toml", "Cargo.lock")
        self.git("commit", "--quiet", "-m", "chore(release): v0.3.0")
        return source, self.git("rev-parse", "HEAD")

    def prepare_retry(self, annotated=False):
        _, release = self.prepare_release()
        self.git("checkout", "--quiet", "main")
        self.git("merge", "--ff-only", "--quiet", release)
        if annotated:
            self.git("tag", "--annotate", "v0.3.0", "-m", "Stable release", release)
        else:
            self.git("tag", "v0.3.0", release)
        self.commit("ci: repair release workflow")
        return release

    def retry_snapshot(self):
        state = self.snapshot()
        return state, (self.root / ".git/index").read_bytes(), self.git("show-ref")

    def test_retry_uses_existing_tag_not_fixed_main_or_dirty_checkout(self):
        release = self.prepare_retry()
        self.write("source.txt", "staged local edit\n")
        self.git("add", "source.txt")
        self.write("source.txt", "unstaged local edit\n")
        self.write("VERSION", "9.9.9\n")
        self.write("untracked.txt", "preserve this file\n")
        before = self.retry_snapshot()
        result = json.loads(self.cli("retry", "0.3.0", "--main", "main").stdout)
        self.assertEqual(result, {
            "version": "0.3.0", "source": release,
            "build_number": int(self.git("rev-list", "--count", release)),
        })
        self.assertNotEqual(result["source"], self.git("rev-parse", "main"))
        self.assertEqual(self.retry_snapshot(), before)

    def test_retry_accepts_annotated_tag_and_exact_trusted_commit(self):
        release = self.prepare_retry(annotated=True)
        trusted = self.git("rev-parse", "main")
        result = json.loads(self.cli("retry", "0.3.0", "--main", trusted).stdout)
        self.assertEqual(result["source"], release)
        self.assertEqual(result["version"], "0.3.0")
        self.assertEqual(result["build_number"], int(self.git("rev-list", "--count", release)))

    def test_retry_rejects_tag_outside_trusted_main_without_mutation(self):
        release = self.prepare_retry()
        self.git("branch", "trusted-main", self.baseline)
        before = self.retry_snapshot()
        result = self.cli("retry", "0.3.0", "--main", "trusted-main", success=False)
        self.assertIn("not an ancestor", result.stderr)
        self.assertEqual(self.retry_snapshot(), before)
        self.assertEqual(self.git("rev-parse", "refs/tags/v0.3.0"), release)

    def test_retry_rejects_nonexistent_unsafe_and_prerelease_tags(self):
        self.prepare_retry()
        self.git("tag", "v0.3.0-rc.1")
        self.git("branch", "v8.8.8")
        before = self.retry_snapshot()
        for version in (
            "8.8.8", "0.3.0-rc.1", "v0.3.0", "../0.3.0", "0.3.0^{commit}",
            "0.3.0+build.1", "00.3.0", "--bad-option", "",
        ):
            with self.subTest(version=version):
                self.cli("retry", version, "--main", "main", success=False)
                self.assertEqual(self.retry_snapshot(), before)
        self.cli("retry", "0.3.0", "--main", "missing-main", success=False)
        self.assertEqual(self.retry_snapshot(), before)

    def test_retry_rejects_inconsistent_committed_versions(self):
        cases = (
            ("VERSION", "0.2.2", "0.2.1"),
            ("Cargo.toml", 'version = "0.2.2"', 'version = "0.2.1"'),
            ("Cargo.lock", 'name = "example-app"\nversion = "0.2.2"',
             'name = "example-app"\nversion = "0.2.1"'),
            ("Cargo.lock", 'name = "example-core"\nversion = "0.2.2"',
             'name = "example-core"\nversion = "0.2.1"'),
            ("crates/example-core/Cargo.toml", "version.workspace = true",
             'version = "0.2.2"'),
        )
        for index, (path, old, new) in enumerate(cases):
            with self.subTest(path=path, replacement=new):
                self.git("checkout", "--quiet", "-b", f"retry-invalid-{index}", self.baseline)
                target = self.root / path
                self.write(path, target.read_text().replace(old, new))
                invalid = self.commit("chore: commit inconsistent release")
                self.git("update-ref", "refs/tags/v0.2.2", invalid)
                before = self.retry_snapshot()
                self.cli("retry", "0.2.2", "--main", "HEAD", success=False)
                self.assertEqual(self.retry_snapshot(), before)

    def test_verify_accepts_exact_release_merge_without_checkout_changes(self):
        source, release = self.prepare_release()
        self.git("checkout", "--quiet", "main")
        self.git("merge", "--no-ff", "--quiet", "-m", "Merge release PR", release)
        merged = self.git("rev-parse", "HEAD")
        self.git("checkout", "--quiet", "--detach", source)
        self.write("untracked.txt", "preserve local work\n")
        before = self.snapshot(), self.git("write-tree")
        result = json.loads(self.cli("verify", "--source", source, "--release", merged).stdout)
        self.assertEqual(result, {
            "version": "0.3.0", "tag": "v0.3.0", "source": merged,
            "previous": "0.2.2", "bump": "minor",
        })
        self.assertEqual((self.snapshot(), self.git("write-tree")), before)

    def test_verify_rejects_unrelated_source_change_in_release(self):
        source, _ = self.prepare_release()
        altered = self.commit("fix: sneak source change into release")
        before = self.snapshot()
        self.cli("verify", "--source", source, "--release", altered, success=False)
        self.assertEqual(self.snapshot(), before)

    def test_verify_rejects_wrong_stamp_and_incomplete_stamp(self):
        source, _ = self.prepare_release()
        self.cli("stamp", "0.3.1")
        wrong = self.commit("chore(release): incorrect version", change_source=False)
        self.cli("verify", "--source", source, "--release", wrong, success=False)
        self.git("checkout", "--quiet", "-b", "incomplete-release", source)
        self.write("VERSION", "0.3.0\n")
        incomplete = self.commit("chore(release): incomplete stamp", change_source=False)
        self.cli("verify", "--source", source, "--release", incomplete, success=False)

    def test_verify_rejects_release_without_releasable_changes(self):
        self.git("tag", "v0.2.2")
        source = self.commit("docs: explain release procedure")
        self.cli("stamp", "0.2.3")
        self.git("add", "VERSION", "Cargo.toml", "Cargo.lock")
        self.git("commit", "--quiet", "-m", "chore(release): v0.2.3")
        self.cli("verify", "--source", source, "--release", "HEAD", success=False)


if __name__ == "__main__":
    unittest.main()
