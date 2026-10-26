# Contributing to Murvix

Thank you for your interest in contributing to Murvix! This guide will help you get started.

## AI Welcome Policy

AI-assisted and AI-generated contributions are welcome, including code, documentation, commits, issues, pull requests, comments, and reviews. AI agents may participate on behalf of contributors.

The same standards apply to every contribution, regardless of how it was created:

- Review contributions for correctness, security, and relevance before submitting.
- Verify changes with the appropriate checks and describe what was actually tested.
- Take responsibility for submitted work and address review feedback.
- Do not submit secrets, private data, fabricated results, or content you do not have permission to share.

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

4. Commit your changes, push your branch, and open a pull request yourself

## Code Standards

- **No comments in the codebase** — all code must be self-explanatory and cleanly structured
- **Early returns** over nested conditionals
- **Fix root causes**, not symptoms
- **Follow existing patterns** but suggest refactors if they improve quality
- **Security first** — no command injection, XSS, or other vulnerabilities

## Checks

All PRs must pass CI. Run its main checks locally:

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

## Pull Request Guidelines

- Keep PRs focused on a single change
- Write a clear title and description yourself, in your own words, explaining the "why"
- Ensure all checks pass before requesting review
- Link any related issues
- If you used AI to help write code, name the LLM in your own PR description

## Reporting Issues

- Report bugs and ideas on [GitHub Issues](https://github.com/teomurgi/Murvix/issues)
- Search existing issues before creating a new one

## License

By contributing, you agree that your contributions will be licensed under the [MIT License](LICENSE).
