#!/usr/bin/env python3
"""Real macOS Unix-NFS/Seatbelt smoke; needs an ordinary user with sudo -n.

Build murvix + murvix-server first. Run: python3 scripts/workspace-macos-smoke.py
[target/debug/murvix]. No mount, server, sandbox, or assertion error is skipped.
The controller stays outside confinement and changes the authoritative catalog;
only the agent subprocess performs filesystem assertions through the mount.
"""
import errno
import json
import os
from pathlib import Path
import platform
import shlex
import signal
import socket
import stat
import subprocess
import sys
import tempfile
import time
import traceback

SCRIPT = Path(__file__).resolve()


def wait_until(action, description, timeout=60):
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        value = action()
        if value:
            return value
        time.sleep(0.05)
    raise RuntimeError(f"timed out: {description}")


def denied(action):
    try:
        action()
    except OSError as error:
        assert error.errno in (errno.EPERM, errno.EACCES, errno.EROFS), error
    else:
        raise AssertionError("operation escaped confinement")


def absent(path):
    try:
        path.stat()
    except OSError as error:
        assert error.errno in (errno.ENOENT, errno.ESTALE), error
        return
    raise AssertionError(f"removed root still accessible: {path}")


def agent(config, second):
    root = Path.cwd()
    primary = root / "primary"
    assert root == Path(os.environ["MURVIX_WORKSPACE_ROOT"])
    assert root.parent.parent == Path("/private/tmp")
    assert Path(os.environ["HOME"]) == root / ".state/home"
    for variable in ("HOME", "TMPDIR", "TMP", "TEMP", "XDG_CACHE_HOME", "XDG_DATA_HOME",
                     "XDG_CONFIG_HOME", "XDG_STATE_HOME", "XDG_RUNTIME_DIR"):
        assert Path(os.environ[variable]).is_relative_to(root / ".state"), variable
    persistent = Path(os.environ["HOME"]) / "persistent"
    if second:
        assert persistent.read_text() == "preserved"
        print("PERSISTENT_STATE_OK", flush=True)
        return
    document = json.loads((root / "workspace.json").read_text())
    authoritative = config["snapshot"]
    assert document["server"] == authoritative["server"]
    assert str(document["session"]) == str(authoritative["session"])
    assert document["project"] == authoritative["project"]
    assert document["revision"] == authoritative["revision"]
    mapped = {bytes(item["path_bytes"]): item for item in document["roots"]}
    for item in authoritative["roots"]:
        entry = mapped[bytes(item["path"])]
        assert entry["key"] == item["key"]
        assert entry["directory"] == ("primary" if item["primary"] else item["key"])
    denied(lambda: (root / "workspace.json").write_text("bad"))
    denied(lambda: (Path(config["primary"]) / "escape").write_text("bad"))
    denied(lambda: (Path(config["outside"]) / "escape").write_text("bad"))
    denied(lambda: (Path(config["profile"]) / "escape").write_text("bad"))
    denied(lambda: (root.parent / "control/escape").write_text("bad"))
    denied(lambda: (Path(config["state_backing"]) / "home/escape").write_text("bad"))
    denied(lambda: os.kill(config["host_pid"], signal.SIGCONT))
    for path in (root.parent / "control/nfs.sock", Path(config["profile"]) / "server.sock"):
        with socket.socket(socket.AF_UNIX, socket.SOCK_STREAM) as client:
            denied(lambda: client.connect(str(path)))
    entry = mapped[os.fsencode(config["extra"])]
    extra = root / entry["directory"]
    assert (extra / "sentinel").read_text() == "extra"
    denied(lambda: (extra / "sentinel").write_text("bad"))
    locked = primary / "locked"
    locked_status = locked.stat()
    assert stat.S_ISDIR(locked_status.st_mode), locked_status
    denied(lambda: (locked / "sentinel").write_text("bad"))
    (primary / "crud").mkdir()
    (primary / "crud/a").write_text("content")
    assert (primary / "crud/a").read_text() == "content"
    (primary / "crud/a").rename(primary / "crud/b")
    (primary / "crud/b").unlink()
    (primary / "crud").rmdir()
    repository = primary / "repository"
    repository.mkdir()
    subprocess.run(["/usr/bin/git", "init", str(repository)], check=True)
    objects = repository / ".git/objects"
    object_directory = objects / "murvix"
    object_directory.mkdir()
    source = object_directory / "source"
    source.write_text("object")
    linked = object_directory / "linked"
    os.link(source, linked)
    linked.rename(object_directory / "renamed")
    source.unlink()
    (object_directory / "renamed").unlink()
    object_directory.rmdir()
    (repository / "tracked").write_text("git data")
    subprocess.run(["/usr/bin/git", "-C", str(repository), "add", "tracked"], check=True)
    subprocess.run(["/usr/bin/git", "-C", str(repository), "-c", "user.name=Smoke",
                    "-c", "user.email=smoke@example.invalid", "commit", "-m", "smoke"], check=True)
    persistent.write_text("preserved")
    # Cache both a positive extra-root lookup and a negative lookup for a root
    # learned from an earlier real snapshot. Catalog edits must change both.
    added = root / config["added_key"]
    absent(added)
    held = (extra / "sentinel").open()
    assert held.read() == "extra"
    (primary / "agent-ready").write_text("ready")
    wait_until(lambda: (primary / "catalog-added").exists(), "catalog addition")
    assert (added / "sentinel").read_text() == "added", "cached miss survived root addition"
    (primary / "agent-added").write_text("ready")
    wait_until(lambda: (primary / "catalog-removed").exists(), "catalog removal")
    absent(extra)
    try:
        held.seek(0)
        held.read()
    except OSError as error:
        assert error.errno in (errno.ESTALE, errno.ENOENT, errno.EIO), error
    else:
        raise AssertionError("removed root's open file still reads")
    held.close()
    assert extra.name not in os.listdir(root), "cached directory hit survived removal"
    assert added.name in os.listdir(root)
    current = json.loads((root / "workspace.json").read_text())
    assert current["revision"] != document["revision"]
    assert all(bytes(item["path_bytes"]) != os.fsencode(config["extra"]) for item in current["roots"])
    assert any(bytes(item["path_bytes"]) == os.fsencode(config["added"]) for item in current["roots"])
    print("NATIVE_UNIX_NFS_SEATBELT_OK", flush=True)


def terminal(config_path):
    config = json.loads(config_path.read_text())
    binary = config["binary"]
    try:
        config["host_pid"] = os.getpid()
        config["snapshot"] = json.loads(subprocess.check_output([binary, "workspace", "snapshot", "--json"], text=True))
        snapshot = config["snapshot"]
        config["state_backing"] = str(Path(config["profile"]) / "agent-workspaces" /
                                      str(snapshot["server"]) / str(snapshot["project"]) / "state")
        # The initially added root's stable key comes from the authoritative
        # snapshot, not a fixture or reconstruction of the opaque key algorithm.
        config["added_key"] = next(root["key"] for root in snapshot["roots"]
                                   if bytes(root["path"]) == os.fsencode(config["added"]))
        config_path.write_text(json.dumps(config))
        Path(config["snapshot_ready"]).write_text("ready")
        wait_until(lambda: Path(config["initial_ready"]).exists(), "initial authoritative roots")
        config["snapshot"] = json.loads(subprocess.check_output([binary, "workspace", "snapshot", "--json"], text=True))
        config_path.write_text(json.dumps(config))
        output = []
        for mode in ("--agent", "--agent-second"):
            result = subprocess.run([binary, "workspace", "run", "--", sys.executable, str(SCRIPT), mode, str(config_path)],
                                    stdin=subprocess.DEVNULL, stdout=subprocess.PIPE, stderr=subprocess.STDOUT,
                                    text=True, timeout=180)
            output.append(result.stdout)
            if result.returncode:
                raise RuntimeError(f"workspace failed ({result.returncode}):\n{result.stdout}")
        Path(config["result"]).write_text(json.dumps({"ok": True, "output": output}))
    except BaseException:
        Path(config["result"]).write_text(json.dumps({"ok": False, "error": traceback.format_exc()}))
        raise


def smoke(binary):
    assert platform.system() == "Darwin", "this smoke requires real macOS"
    assert os.geteuid() != 0, "the CLI and broker must run as an ordinary user"
    subprocess.run(["/usr/bin/sudo", "-n", "--", "/usr/bin/true"], check=True)
    with tempfile.TemporaryDirectory(prefix="murvix-nfs-smoke-") as temporary:
        base = Path(temporary).resolve()
        paths = {name: base / name for name in ("profile", "primary", "extra", "added", "outside")}
        for path in paths.values():
            path.mkdir(mode=0o700)
        locked = paths["primary"] / "locked"
        locked.mkdir()
        (locked / "sentinel").write_text("locked")
        for name in ("extra", "added"):
            (paths[name] / "sentinel").write_text(name)
        env = {**os.environ, "MUXY_DIR": str(paths["profile"]), "SHELL": "/bin/sh"}
        config = {key: str(value) for key, value in paths.items()}
        config.update(binary=str(binary), result=str(base / "result.json"),
                      snapshot_ready=str(base / "snapshot-ready"), initial_ready=str(base / "initial-ready"))
        config_path = base / "config.json"
        config_path.write_text(json.dumps(config))
        def cli(*args):
            return subprocess.check_output([str(binary), *map(str, args)], env=env, text=True, timeout=30)
        with (base / "server.log").open("wb") as log:
            server = subprocess.Popen([str(binary.with_name("murvix-server"))], env=env, stdout=log, stderr=log)
            try:
                def ready():
                    if server.poll() is not None:
                        raise RuntimeError("foreground server exited")
                    return (paths["profile"] / "server.sock").exists()
                wait_until(ready, "authoritative server startup")
                project = json.loads(cli("project", "add", paths["primary"], "--name", "NFS smoke", "--json"))["id"]
                cli("project", "workspace", project, paths["extra"], locked, paths["added"])
                command = shlex.join(["exec", sys.executable, str(SCRIPT), "--terminal", str(config_path)])
                session = json.loads(cli("session", "create", project, "--json", "--", command))["id"]
                def milestone(path):
                    if Path(config["result"]).exists():
                        result = json.loads(Path(config["result"]).read_text())
                        if not result["ok"]:
                            raise RuntimeError(result["error"])
                    return path.exists()
                wait_until(lambda: milestone(Path(config["snapshot_ready"])), "real session binding")
                cli("project", "workspace", project, paths["extra"], locked)
                Path(config["initial_ready"]).write_text("ready")
                wait_until(lambda: milestone(paths["primary"] / "agent-ready"), "mounted agent assertions", 150)
                cli("project", "workspace", project, paths["extra"], locked, paths["added"])
                (paths["primary"] / "catalog-added").write_text("ready")
                wait_until(lambda: milestone(paths["primary"] / "agent-added"), "live cached-miss invalidation")
                cli("project", "workspace", project, locked, paths["added"])
                (paths["primary"] / "catalog-removed").write_text("ready")
                wait_until(lambda: Path(config["result"]).exists(), "agent and remount completion", 180)
                result = json.loads(Path(config["result"]).read_text())
                assert result["ok"], result
                print("\n".join(result["output"]))
                assert "NATIVE_UNIX_NFS_SEATBELT_OK" in result["output"][0]
                assert "PERSISTENT_STATE_OK" in result["output"][1]
                for path in (paths["primary"] / "escape", paths["outside"] / "escape"):
                    assert not path.exists(), path
                assert (locked / "sentinel").read_text() == "locked"
                assert (paths["extra"] / "sentinel").read_text() == "extra"
                cli("session", "wait", session, "--exit", "--timeout-ms", "30000")
            finally:
                if server.poll() is None:
                    server.send_signal(signal.SIGTERM)
                try:
                    status = server.wait(timeout=20)
                except subprocess.TimeoutExpired:
                    server.kill()
                    server.wait()
                    raise
                if status:
                    raise RuntimeError(f"foreground server exited with {status}: {(base / 'server.log').read_text()}")
    print("Real sudo Unix-socket NFS mount, authoritative live roots, Seatbelt and persistent state passed")


if __name__ == "__main__":
    if len(sys.argv) > 1 and sys.argv[1] in ("--agent", "--agent-second"):
        agent(json.loads(Path(sys.argv[2]).read_text()), sys.argv[1] == "--agent-second")
    elif len(sys.argv) > 1 and sys.argv[1] == "--terminal":
        terminal(Path(sys.argv[2]))
    else:
        smoke(Path(sys.argv[1] if len(sys.argv) > 1 else "target/debug/murvix").resolve())
