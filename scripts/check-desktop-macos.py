import argparse
import os
from pathlib import Path
import platform
import resource
import subprocess
import shlex
import sys
import time

ROOT = Path(__file__).resolve().parent.parent


def run_command(command, env, timings, capture_output=False):
    label = shlex.join(command)
    print("+ " + label, flush=True)
    started = time.monotonic()
    status = "failed"
    try:
        if capture_output:
            result = subprocess.check_output(command, cwd=ROOT, env=env, text=True)
        else:
            result = subprocess.run(command, cwd=ROOT, env=env, check=True)
        status = "passed"
        return result
    finally:
        elapsed = time.monotonic() - started
        timings.append((label, elapsed, status))
        print(f"Timing: {label}: {elapsed:.2f}s ({status})", flush=True)


def report_timings(phase, timings, elapsed, status):
    print(f"Timing: macOS guard ({phase}) total: {elapsed:.2f}s ({status})", flush=True)
    summary = os.environ.get("GITHUB_STEP_SUMMARY")
    if summary:
        try:
            with open(summary, "a", encoding="utf-8") as output:
                output.write(f"\n### macOS guard: {phase}\n\n| Command | Seconds | Result |\n| --- | ---: | --- |\n")
                for label, seconds, result in timings:
                    output.write(f"| `{label}` | {seconds:.2f} | {result} |\n")
                output.write(f"| **Total** | **{elapsed:.2f}** | **{status}** |\n")
        except OSError as error:
            print(f"Could not write Actions timing summary: {error}", file=sys.stderr)


def native_environment(timings):
    if platform.system() != "Darwin":
        raise ValueError("The macOS guard must run on macOS; Linux checks do not replace it")
    compiler = run_command(["rustc", "-vV"], os.environ.copy(), timings, capture_output=True)
    host = next((line.removeprefix("host: ") for line in compiler.splitlines() if line.startswith("host: ")), "")
    if host not in {"aarch64-apple-darwin", "x86_64-apple-darwin"}:
        raise ValueError("The macOS guard requires a native macOS Rust toolchain")
    if os.environ.get("CARGO_BUILD_TARGET", host) != host:
        raise ValueError("Remove CARGO_BUILD_TARGET or set it to the native macOS host")
    soft, hard = resource.getrlimit(resource.RLIMIT_NOFILE)
    requested = 10240 if hard == resource.RLIM_INFINITY else min(10240, hard)
    if soft != resource.RLIM_INFINITY and soft < requested:
        resource.setrlimit(resource.RLIMIT_NOFILE, (requested, hard))
    env = {
        **os.environ,
        "RUSTDOCFLAGS": (os.environ.get("RUSTDOCFLAGS", "") + " -D warnings").strip(),
        "RUST_TEST_THREADS": "4",
        "GHOSTTY_BUILD_CACHE_DIR": os.environ.get("GHOSTTY_BUILD_CACHE_DIR") or str(ROOT / ".cache/ghostty-native"),
    }
    return host, env


def main():
    parser = argparse.ArgumentParser(description="Run the macOS desktop regression guard.")
    parser.add_argument(
        "--phase", choices=("all", "cheap", "native"), default="all",
        help="all (default): full guard; cheap: Python tests and fmt only; native: native macOS checks only",
    )
    args = parser.parse_args()
    started = time.monotonic()
    timings = []
    status = "failed"
    try:
        env = os.environ.copy()
        if args.phase != "cheap":
            host, env = native_environment(timings)
        commands = []
        if args.phase != "native":
            commands.extend([
                [sys.executable, "-m", "unittest", "discover", "-s", "scripts/tests"],
                ["cargo", "fmt", "--all", "--check"],
            ])
        if args.phase != "cheap":
            commands.extend([
                [sys.executable, "scripts/tests/test_ghostty_cache.py", "--compiler-identity"],
                ["cargo", "clippy", "--locked", "--workspace", "--all-targets", "--all-features", "--", "-D", "warnings"],
                ["cargo", "test", "--locked", "--workspace", "--all-features"],
                ["cargo", "doc", "--locked", "--workspace", "--no-deps"],
                # Workspace tests enable test-support dependencies; their binaries
                # cannot replace these default-feature smoke executables.
                ["cargo", "build", "--locked", "-p", "murvix-app", "-p", "murvix-cli", "-p", "murvix-server"],
            ])
        for command in commands:
            run_command(command, env, timings)
        if args.phase == "cheap":
            print("Cheap Python and formatting checks passed; native macOS checks were not run.", flush=True)
        else:
            print(f"macOS automated guard ({args.phase}) passed on {host}; record required human checks separately.", flush=True)
        status = "passed"
    finally:
        report_timings(args.phase, timings, time.monotonic() - started, status)


if __name__ == "__main__":
    try:
        main()
    except (ValueError, OSError, subprocess.CalledProcessError) as error:
        sys.exit(str(error))
