import contextlib
import importlib.util
import io
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[2]
SPEC = importlib.util.spec_from_file_location("desktop_macos", ROOT / "scripts/check-desktop-macos.py")
GUARD = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(GUARD)


class DesktopMacosTests(unittest.TestCase):
    def environment(self, system="Darwin", host="aarch64-apple-darwin", target=None,
                    phase="all", environ=None, limits=(256, 20480)):
        stack = contextlib.ExitStack()
        self.addCleanup(stack.close)
        argv = ["check-desktop-macos.py"]
        if phase != "all":
            argv.extend(["--phase", phase])
        stack.enter_context(patch.object(sys, "argv", argv))
        stack.enter_context(patch.object(GUARD.platform, "system", return_value=system))
        stack.enter_context(patch.object(GUARD.subprocess, "check_output", return_value=f"host: {host}\n"))
        stack.enter_context(patch.object(GUARD.resource, "getrlimit", return_value=limits))
        limit = stack.enter_context(patch.object(GUARD.resource, "setrlimit"))
        env = {"RUSTDOCFLAGS": "--cfg regression_guard"}
        if target is not None:
            env["CARGO_BUILD_TARGET"] = target
        env.update(environ or {})
        stack.enter_context(patch.dict(os.environ, env, clear=True))
        self.log = io.StringIO()
        stack.enter_context(contextlib.redirect_stdout(self.log))
        run = stack.enter_context(patch.object(GUARD.subprocess, "run"))
        return run, limit

    def test_other_platforms_cannot_satisfy_the_macos_guard(self):
        for phase in ("all", "native"):
            for settings in [
                {"system": "Linux"},
                {"host": "aarch64-unknown-linux-gnu"},
                {"target": "aarch64-unknown-linux-gnu"},
            ]:
                with self.subTest(phase=phase, settings=settings):
                    run, _ = self.environment(phase=phase, **settings)
                    with self.assertRaises(ValueError):
                        GUARD.main()
                    run.assert_not_called()
                    self.doCleanups()

    def test_any_failed_check_stops_the_guard(self):
        for phase, steps in (("all", 7), ("cheap", 2), ("native", 5)):
            for failed_step in range(steps):
                with self.subTest(phase=phase, failed_step=failed_step):
                    run, _ = self.environment(phase=phase)
                    run.side_effect = [None] * failed_step + [subprocess.CalledProcessError(1, "check")]
                    with self.assertRaises(subprocess.CalledProcessError):
                        GUARD.main()
                    self.assertEqual(run.call_count, failed_step + 1)
                    self.doCleanups()

    def test_cheap_phase_needs_no_native_platform_toolchain_or_resources(self):
        _, limit = self.environment(system="Linux", phase="cheap")
        with patch.object(GUARD.subprocess, "check_output", side_effect=AssertionError("native toolchain")), \
                patch.object(GUARD.resource, "getrlimit", side_effect=AssertionError("native resources")):
            GUARD.main()
        limit.assert_not_called()

    def test_native_fd_limit_respects_existing_and_hard_limits(self):
        infinity = GUARD.resource.RLIM_INFINITY
        for soft, hard, requested in [
            (256, 20480, 10240),
            (256, 4096, 4096),
            (256, infinity, 10240),
            (16384, 20480, None),
            (infinity, infinity, None),
        ]:
            with self.subTest(soft=soft, hard=hard):
                _, limit = self.environment(phase="native", limits=(soft, hard))
                GUARD.main()
                if requested is None:
                    limit.assert_not_called()
                else:
                    limit.assert_called_once_with(GUARD.resource.RLIMIT_NOFILE, (requested, hard))
                self.doCleanups()

    def test_timings_survive_success_and_command_failure_in_logs_and_summary(self):
        for failed in (False, True):
            with self.subTest(failed=failed), tempfile.TemporaryDirectory() as temporary:
                summary = Path(temporary) / "summary"
                run, _ = self.environment(phase="cheap", environ={"GITHUB_STEP_SUMMARY": str(summary)})
                if failed:
                    run.side_effect = [None, subprocess.CalledProcessError(1, "check")]
                with patch.object(GUARD.time, "monotonic", side_effect=range(6)):
                    if failed:
                        with self.assertRaises(subprocess.CalledProcessError):
                            GUARD.main()
                    else:
                        GUARD.main()
                status = "failed" if failed else "passed"
                log = self.log.getvalue()
                self.assertIn(f"Timing: cargo fmt --all --check: 1.00s ({status})", log)
                self.assertIn(f"total: 5.00s ({status})", log)
                contents = summary.read_text()
                self.assertIn(f"| `cargo fmt --all --check` | 1.00 | {status} |", contents)
                self.assertIn(f"| **Total** | **5.00** | **{status}** |", contents)
                self.doCleanups()


if __name__ == "__main__":
    unittest.main()
