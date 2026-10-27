# Contributing to Murvix

Thank you for your interest in contributing to Murvix! This guide will help you get started.

## AI-assisted contributions

AI tools and agents may assist with code, commits, pushes, issues, pull requests,
and reviews when authorized by the contributor or maintainer responsible for
those actions. That person remains accountable for accuracy, security, licensing,
and compliance with project policies. AI assistance does not grant repository
permissions or replace maintainer review and CI authorization.

## Getting Started

Read [docs/product](docs/product/README.md) and [docs/tech](docs/tech/README.md) first. They describe how Murvix works and how it is built.

### Prerequisites

- macOS 14+ for the desktop app, or Linux for the CLI and server
- [Rust](https://rustup.rs) (`rustup` installs the version pinned in `rust-toolchain.toml`)
- [Zig](https://ziglang.org) 0.15.2, used to build the Ghostty terminal library

### Setup

```bash
git clone https://github.com/teomurgi/Murvix.git
cd Murvix
```

On macOS, build the whole workspace with `cargo build --workspace`. On Linux,
build the CLI and server with `cargo build -p murvix-cli -p murvix-server`; the
desktop app is macOS only.

### Running

```bash
cargo run -p murvix-app     # desktop app (macOS)
cargo run -p murvix-cli     # murvix terminal UI
```

Build as in [Setup](#setup) first, so `murvix-server` sits next to the binary you
run.
Development builds keep their data in a separate `Murvix Dev` profile (`murvix-dev`
on Linux).

## Development Workflow

1. Fork the repository and create a branch from `main`
2. Make your changes
3. Run checks before committing (on Linux, see [Checks](#checks)):

```bash
cargo fmt --all
cargo xcheck              # clippy with warnings as errors
cargo xtest               # all tests
```

4. Commit your changes, push your branch, and open a pull request

### Review and CI authorization

The current maintainer is [@teomurgi](https://github.com/teomurgi). See
[GOVERNANCE.md](GOVERNANCE.md) for decision-making, maintainer membership,
repository permissions, and the CI authorization runbook.

Only maintainers may merge PRs, authorize upstream workflow execution, or publish
releases. Maintainer-opened or updated same-repository PRs targeting `main` run CI
automatically. Fork PRs need a maintainer to review the exact commit and dispatch
`ci` from `main` with the PR number and reviewed full head SHA after each update.
Both `macos` and `omarchy` must pass on the current revision, the PR branch must
include the latest `main`, and review conversations must be resolved before merge.
While the project has one maintainer, an independent approval is not mandatory;
`CODEOWNERS` requests review but does not enforce merge authority.

## Code Standards

- **No comments in the codebase** — all code must be self-explanatory and cleanly structured
- **Early returns** over nested conditionals
- **Fix root causes**, not symptoms
- **Follow existing patterns** but suggest refactors if they improve quality
- **Security first** — no command injection, XSS, or other vulnerabilities

## Checks

All PRs must pass maintainer-authorized CI. Contributors should run its main checks locally before requesting review:

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps
```

On Linux, CI runs the whole workspace natively on Arch (Omarchy's distro) via
`scripts/check-desktop-linux.py`, including the
[experimental desktop app](docs/tech/constraints.md), under xvfb. macOS runs
`scripts/check-desktop-macos.py`.

TUI regression tests run the real CLI and server under a PTY. Synchronize tab
interactions on displayed shortcut numbers or session state, not shell-generated
tab titles: a shell may replace a directory title with a host-qualified title
after startup. To exercise session exit during a blocked resize:

```bash
cargo test --locked -p murvix-cli --test tui exit_during_resize_keeps_tab_selection_bound_to_the_displayed_pane
```

## Pull Request Guidelines

- Keep PRs focused on a single change
- Write a clear title and description explaining the "why"
- Run local checks before requesting review; upstream CI needs maintainer authorization
- Link any related issues
- If you used AI assistance, name the LLM in the PR description

## Reporting Issues

- Report bugs and ideas on [GitHub Issues](https://github.com/teomurgi/Murvix/issues)
- Search existing issues before creating a new one

## License

By contributing, you agree that your contributions will be licensed under the [MIT License](LICENSE).
