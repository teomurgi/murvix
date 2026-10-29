"""Desktop verification must fail closed and clean up compositor processes."""

import importlib.util
import os
from pathlib import Path
import sys
import tempfile
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))
spec = importlib.util.spec_from_file_location(
    "verify_desktop_formats", Path(__file__).resolve().parent.parent / "verify-desktop-formats.py")
formats = importlib.util.module_from_spec(spec)
spec.loader.exec_module(formats)


class DesktopDisplayTests(unittest.TestCase):
    def test_missing_compositor_is_a_failure_not_a_skipped_gate(self):
        with tempfile.TemporaryDirectory() as empty:
            with self.assertRaisesRegex(RuntimeError, "requires Weston"):
                with formats.wayland_display({"PATH": empty}):
                    self.fail("Desktop verification ran without a compositor")

    def test_startup_failure_reports_the_compositor_diagnostic(self):
        with tempfile.TemporaryDirectory() as temporary:
            binary = Path(temporary) / "weston"
            binary.write_text(f"#!{sys.executable}\nimport sys\nprint('renderer unavailable', flush=True)\nsys.exit(93)\n")
            binary.chmod(0o755)
            with self.assertRaisesRegex(RuntimeError, "renderer unavailable"):
                with formats.wayland_display({**os.environ, "PATH": temporary, "WAYLAND_DISPLAY": ""}):
                    self.fail("A dead compositor was accepted")

    def test_check_failure_reaps_compositor_and_removes_private_runtime(self):
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary)
            binary = directory / "weston"
            pid_file = directory / "pid"
            binary.write_text(f"#!{sys.executable}\n" + """import os, signal, socket
from pathlib import Path
Path(os.environ['PID_FILE']).write_text(str(os.getpid()))
listener = socket.socket(socket.AF_UNIX)
listener.bind(str(Path(os.environ['XDG_RUNTIME_DIR']) / 'murvix-check'))
listener.listen()
signal.pause()
""")
            binary.chmod(0o755)
            env = {**os.environ, "PATH": temporary, "WAYLAND_DISPLAY": "", "PID_FILE": str(pid_file)}
            with self.assertRaisesRegex(ValueError, "browser check failed"):
                with formats.wayland_display(env) as display:
                    runtime = Path(display["XDG_RUNTIME_DIR"])
                    self.assertTrue((runtime / display["WAYLAND_DISPLAY"]).is_socket())
                    self.assertEqual(runtime.stat().st_mode & 0o777, 0o700)
                    pid = int(pid_file.read_text())
                    raise ValueError("browser check failed")
            self.assertFalse(runtime.exists())
            with self.assertRaises(ProcessLookupError):
                os.kill(pid, 0)


if __name__ == "__main__":
    unittest.main()
