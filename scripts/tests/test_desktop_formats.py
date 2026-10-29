"""Unit tests for the Wayland gating in verify-desktop-formats (no root, FUSE, or compositor needed)."""

import importlib.util
import sys
import unittest
from pathlib import Path
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))


def load_formats():
    path = Path(__file__).resolve().parent.parent / "verify-desktop-formats.py"
    spec = importlib.util.spec_from_file_location("verify_desktop_formats", path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


_formats = load_formats()
ensure_wayland = _formats.ensure_wayland
stop_wayland = _formats.stop_wayland
class FakeProcess:
    def __init__(self, pid=4242, returncode=None):
        self.pid = pid
        self._returncode = returncode

    def poll(self):
        return self._returncode


class WaylandGatingTests(unittest.TestCase):
    def test_existing_display_passes_through_untouched(self):
        env = {"WAYLAND_DISPLAY": "wayland-0", "PATH": "/usr/bin:/bin"}
        with patch.object(_formats.shutil, "which") as which:
            self.assertEqual(ensure_wayland(env), env)
            which.assert_not_called()

    def test_missing_weston_skips_loudly(self):
        with patch.object(_formats.shutil, "which", return_value=None), \
             patch("builtins.print") as printed:
            self.assertIsNone(ensure_wayland({}))
            printed.assert_called_once()
            self.assertIn("SKIP", printed.call_args[0][0])

    def test_compositor_that_exits_immediately_skips(self):
        with patch.object(_formats.shutil, "which", return_value="/usr/bin/weston"), \
             patch.object(_formats.subprocess, "Popen", return_value=FakeProcess(returncode=1)), \
             patch.object(_formats.tempfile, "mkdtemp", return_value="/tmp/weston-test"), \
             patch("builtins.print"):
            self.assertIsNone(ensure_wayland({}))

    def test_successful_compositor_sets_display_and_stops_it(self):
        with patch.object(_formats.shutil, "which", return_value="/usr/bin/weston"), \
             patch.object(_formats.subprocess, "Popen", return_value=FakeProcess(pid=4242)), \
             patch.object(_formats.tempfile, "mkdtemp", return_value="/tmp/weston-test"), \
             patch.object(_formats.Path, "exists", return_value=True):
            env = ensure_wayland({"PATH": "/usr/bin:/bin"})
            self.assertEqual(env["WAYLAND_DISPLAY"], "murvix-check")
            self.assertEqual(env["_WESTON_PID"], "4242")
            with patch.object(_formats.os, "killpg") as killpg, \
                 patch("shutil.rmtree") as rmtree:
                stop_wayland(env)
                killpg.assert_called_once()
                self.assertEqual(killpg.call_args[0][0], 4242)
                rmtree.assert_called_once()
            self.assertNotIn("_WESTON_PID", env)

if __name__ == "__main__":
    unittest.main()
