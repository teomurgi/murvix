import os
import signal
import subprocess
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]


class GhosttyCacheTests(unittest.TestCase):
    def test_native_cache_publication_and_process_safety(self):
        with tempfile.TemporaryDirectory(prefix="murvix-cache-regressions-") as temporary:
            binary = Path(temporary) / "native-cache-tests"
            subprocess.run(
                ["rustc", "--edition=2024", "--test", str(ROOT / "vendor/libghostty-vt-sys/cache.rs"),
                 "-o", str(binary)],
                cwd=ROOT, check=True, timeout=60,
            )
            with subprocess.Popen([str(binary)], cwd=ROOT, start_new_session=True) as process:
                try:
                    status = process.wait(timeout=60)
                    if status:
                        raise subprocess.CalledProcessError(status, str(binary))
                finally:
                    try:
                        os.killpg(process.pid, signal.SIGKILL)
                    except ProcessLookupError:
                        pass


if __name__ == "__main__":
    unittest.main()
