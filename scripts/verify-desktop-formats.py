#!/usr/bin/env python3
"""Exercise native packages in a disposable Linux guest or container, never on a user's desktop."""

import argparse
from contextlib import contextmanager, ExitStack
import json
import os
import shutil
from pathlib import Path
import pwd
import shlex
import signal
import subprocess
import sys
import tempfile
import time
from linux_packages import identity, digest
from release import check_build

ROOT = Path(__file__).resolve().parent.parent


def wait_for(check, message, timeout=30, process=None):
    deadline = time.monotonic() + timeout
    while not check():
        if process is not None and process.poll() is not None:
            raise RuntimeError(f"{message}: desktop exited with status {process.returncode}")
        if time.monotonic() >= deadline:
            raise RuntimeError(message)
        time.sleep(0.05)


def environment(home):
    env = {key: value for key, value in os.environ.items() if not key.startswith("MUXY_")}
    env.update(HOME=str(home), XDG_DATA_HOME=str(home / "data"), MUXY_DIR=str(home / "profile"),
               SHELL="/bin/sh", PATH="/usr/bin:/bin")
    return env


@contextmanager
def wayland_display(env):
    if env.get("WAYLAND_DISPLAY"):
        yield env
        return
    weston = shutil.which("weston", path=env.get("PATH"))
    if weston is None:
        raise RuntimeError("Desktop verification requires Weston or an existing WAYLAND_DISPLAY")
    with tempfile.TemporaryDirectory(prefix="murvix-weston-") as temporary:
        runtime = Path(temporary)
        display = "murvix-check"
        display_env = dict(env, XDG_RUNTIME_DIR=str(runtime), WAYLAND_DISPLAY=display,
                           GDK_BACKEND="x11")
        log_path = runtime / "weston.log"
        with log_path.open("w+") as log:
            compositor = subprocess.Popen(
                [weston, "--backend=x11", "--use-pixman", "--no-config",
                 "--idle-time=0", "--width=1280", "--height=1024", f"--socket={display}"],
                env=display_env, start_new_session=True, stdout=log, stderr=subprocess.STDOUT,
            )
            try:
                deadline = time.monotonic() + 20
                while not (runtime / display).exists():
                    if compositor.poll() is not None or time.monotonic() >= deadline:
                        log.seek(0)
                        raise RuntimeError(f"Nested Weston failed to start:\n{log.read()}")
                    time.sleep(0.05)
                yield display_env
            finally:
                if compositor.poll() is None:
                    os.killpg(compositor.pid, signal.SIGTERM)
                try:
                    compositor.wait(timeout=5)
                except subprocess.TimeoutExpired:
                    os.killpg(compositor.pid, signal.SIGKILL)
                    compositor.wait(timeout=5)


def appimage(archive, version, desktop, fuse):
    if os.getuid() == 0:
        raise ValueError("Run the AppImage check as a non-root test user")
    with tempfile.TemporaryDirectory(prefix="murvix-appimage-check-") as temporary:
        home = Path(temporary)
        env = environment(home)
        command = [str(archive)] + ([] if fuse else ["--appimage-extract-and-run"])
        result = json.loads(subprocess.check_output([*command, "--build-info"], env=env, timeout=60))
        assert result["version"] == version
        assert not (home / "profile").exists() and not (home / "data").exists()
        gui = None
        binaries = None
        resources = ExitStack()
        try:
            if desktop:
                env = resources.enter_context(wayland_display(env))
                gui = subprocess.Popen(command, env=env, start_new_session=True, umask=0o027)
                wait_for(lambda: (home / "profile/server.sock").exists(),
                         "AppImage did not connect to its server", process=gui)
                assert gui.poll() is None, "AppImage exited before its desktop was ready"
            else:
                subprocess.run([*command, "--cli", "project", "list"], env=env, check=True, timeout=60, umask=0o027)
            bundles = list((home / "data/murvix-appimage").glob("*/usr/bin"))
            assert len(bundles) == 1
            binaries = bundles[0]
            for binary in ("murvix-app", "murvix", "murvix-server"):
                check_build(version, binaries / binary)
            def cli(*args):
                return subprocess.check_output([str(binaries / "murvix"), *args], env=env, text=True, timeout=15)
            shell_env = home / "shell-environment"
            session = json.loads(cli("session", "create", "Home", "--json", "--directory", str(home), "--",
                                     f"env > {shlex.quote(str(shell_env))}"))["id"]
            wait_for(shell_env.exists, "Packaged terminal did not run its command")
            assert shell_env.stat().st_mode & 0o777 == 0o640, "AppImage changed the terminal's inherited umask"
            inherited = shell_env.read_text().splitlines()
            assert not any(line.startswith(("MUXY_APPIMAGE_", "WEBKIT_EXEC_PATH=", "WEBKIT_INJECTED_BUNDLE_PATH=",
                                            "GIO_EXTRA_MODULES=", "LD_LIBRARY_PATH=")) for line in inherited), inherited
            if gui is not None:
                wait_for(lambda: (home / "profile/app.sock").exists(),
                         "AppImage browser API did not start", process=gui)
                tab = cli("browser", "open", "about:blank", "--project", "Home").strip()
                assert json.loads(cli("browser", "wait", tab, "--function", "document.readyState === 'complete'",
                                      "--timeout-ms", "10000")), "Packaged browser did not finish loading"
                cli("browser", "eval", tab, "document.title = 'Murvix package check'")
                assert json.loads(cli("browser", "read", tab, "--json"))["title"] == "Murvix package check"
                cli("browser", "close", tab)
                os.killpg(gui.pid, signal.SIGTERM)
                gui.wait(timeout=15)
                gui = None
            continued = home / "continued"
            cli("session", "send", str(session), f"printf survived > {shlex.quote(str(continued))}")
            cli("session", "send-keys", str(session), "Enter")
            wait_for(continued.exists, "Terminal stopped when the AppImage exited")
            assert continued.read_text() == "survived"
            cli("session", "end", str(session), "--yes")
            if desktop:
                subprocess.run([sys.executable, str(ROOT / "scripts/verify-webkit-linux.py")],
                               env=env, check=True, timeout=60)
            print("Verified AppImage: matching helpers, durable terminal, clean shell environment" +
                  (", GUI/server startup and system WebKit/TLS" if desktop else ""))
        except Exception:
            if gui is not None:
                print(f"Packaged desktop exit status: {gui.poll()}", file=sys.stderr, flush=True)
            for diagnostic in sorted((home / "profile").glob("app-diagnostics-*.log")):
                print(f"Desktop diagnostic ({diagnostic.name}):\n{diagnostic.read_text(errors='replace')}",
                      file=sys.stderr, flush=True)
            raise
        finally:
            try:
                if gui is not None and gui.poll() is None:
                    os.killpg(gui.pid, signal.SIGTERM)
                    gui.wait(timeout=15)
                for cli in (home / "data/murvix-appimage").glob("*/usr/bin/murvix"):
                    subprocess.run([str(cli), "server", "stop", "--force"], env=env, check=True, timeout=15)
            finally:
                resources.close()


def deb(archive, version, user):
    if os.getuid() != 0 or os.environ.get("MUXY_DISPOSABLE_PACKAGE_TEST") != "1":
        raise ValueError("Debian lifecycle checks require root and MUXY_DISPOSABLE_PACKAGE_TEST=1 in a disposable guest")
    _, identifier, suffix = identity(version)
    package = "murvix-desktop" + suffix
    status = subprocess.run(["dpkg-query", "-W", "-f=${db:Status-Status}", package], capture_output=True, text=True)
    if status.stdout == "installed":
        raise ValueError("Refusing to replace a package that was installed before this check")
    account = pwd.getpwnam(user)
    with tempfile.TemporaryDirectory(prefix="murvix-deb-check-") as temporary:
        home = Path(temporary)
        os.chown(home, account.pw_uid, account.pw_gid)
        env = environment(home)
        def run(*args, **kwargs):
            return subprocess.run(["/usr/sbin/runuser", "-u", user, "--preserve-environment", "--", *map(str, args)], env=env, **kwargs)
        binaries = Path("/usr/lib") / package
        installed = False
        try:
            subprocess.run(["apt-get", "install", "-y", "--no-install-recommends", str(archive)], check=True)
            installed = True
            for binary in ("murvix-app", "murvix", "murvix-server"):
                check_build(version, binaries / binary)
            assert Path(f"/usr/share/applications/{identifier}.desktop").is_file()
            run(sys.executable, ROOT / "scripts/smoke-headless.py", binaries / "murvix", check=True)
            run(binaries / "murvix", "project", "add", home, "--name", "Retained project", check=True)
            before = {binary: digest(binaries / binary) for binary in ("murvix-app", "murvix", "murvix-server")}
            for command in (["dpkg", "--remove", package], ["dpkg", "--install", str(archive)]):
                blocked = subprocess.run(command, capture_output=True)
                assert blocked.returncode != 0 and b"stop its server" in blocked.stderr, blocked
                assert before == {binary: digest(binaries / binary) for binary in before}
                run(binaries / "murvix", "project", "list", check=True)
            run(binaries / "murvix", "server", "stop", "--force", check=True)
            subprocess.run(["apt-get", "install", "--reinstall", "-y", str(archive)], check=True)
            retained = {path: path.read_bytes() for path in (home / "profile").rglob("*")
                        if path.is_file() and not path.is_symlink()}
            subprocess.run(["dpkg", "--remove", package], check=True)
            installed = False
            assert not binaries.exists()
            assert all(path.read_bytes() == contents for path, contents in retained.items())
            print("Verified Debian package: APT dependencies, helpers, live-server upgrade/removal guard, reinstall and retained data")
        finally:
            if installed:
                run(binaries / "murvix", "server", "stop", "--force", check=False, timeout=15)
                subprocess.run(["dpkg", "--remove", package], check=True)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("format", choices=("appimage", "deb"))
    parser.add_argument("archive", type=Path)
    parser.add_argument("--version", required=True)
    parser.add_argument("--desktop", action="store_true", help="Also launch the AppImage GUI and test system WebKit under a display")
    parser.add_argument("--fuse", action="store_true", help="Use real FUSE mounting instead of extraction mode")
    parser.add_argument("--user", default="package-check", help="Non-root test account for Debian lifecycle checks")
    args = parser.parse_args()
    if args.format == "appimage":
        appimage(args.archive.resolve(), args.version, args.desktop, args.fuse)
    else:
        deb(args.archive.resolve(), args.version, args.user)
