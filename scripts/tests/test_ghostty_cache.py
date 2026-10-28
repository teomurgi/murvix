import os
import re
import shutil
import signal
import subprocess
import sys
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


def check_compiler_identity():
    zig = shutil.which("zig")
    if not zig:
        raise ValueError("Install Zig 0.15.2 before running the native compiler identity regression")
    compiler = subprocess.check_output([zig, "env"], text=True)
    zig = Path(re.search(r'\.zig_exe = "([^"]+)"', compiler)[1]).resolve()
    library = re.search(r'\.lib_dir = "([^"]+)"', compiler)[1]
    host = subprocess.check_output(["rustc", "--print", "host-tuple"], text=True).strip()
    target = ROOT / "target"
    target.mkdir(exist_ok=True)
    with tempfile.TemporaryDirectory(prefix="zig-relocation-", dir=target) as temporary:
        work = Path(temporary)
        driver = work / "fingerprint"
        subprocess.run(
            ["rustc", "--edition=2024", str(ROOT / "vendor/libghostty-vt-sys/build.rs"),
             "-o", str(driver)], check=True, timeout=60,
        )
        env = {
            key: value for key, value in os.environ.items()
            if key not in {"CC", "CXX", "CPP", "AR", "LD", "RANLIB"}
            and not key.startswith(("CC_", "CXX_", "AR_"))
        }
        env.update(
            CARGO_MANIFEST_DIR=str(ROOT / "vendor/libghostty-vt-sys"),
            HOST=host, TARGET=host, LIBGHOSTTY_VT_SYS_OPTIMIZE="ReleaseFast",
        )
        keys = []
        for name in ("first-install", "second-install"):
            prefix = work / name
            (prefix / "bin").mkdir(parents=True)
            (prefix / "lib").mkdir()
            os.link(zig, prefix / "bin/zig")
            (prefix / "lib/zig").symlink_to(library, target_is_directory=True)
            output = subprocess.check_output(
                [str(driver), "--cache-key"],
                env={**env, "PATH": f"{prefix / 'bin'}:/usr/bin:/bin:/usr/sbin:/sbin"},
                text=True, timeout=30,
            )
            keys.append(output.splitlines()[-1])
        assert re.fullmatch(r"[0-9a-f]{16}", keys[0])
        assert keys[0] == keys[1], "Relocating the same Zig compiler changed native cache identity"
    print("Native cache fingerprint survives identical Zig compiler relocation")


if __name__ == "__main__":
    if sys.argv[1:] == ["--compiler-identity"]:
        check_compiler_identity()
    else:
        unittest.main()
