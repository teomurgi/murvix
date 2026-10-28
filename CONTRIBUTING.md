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
automatically. Fork PRs and automation PRs without a maintainer-triggered CI run
need a maintainer to review the exact commit and dispatch `ci` from `main` with
the PR number and reviewed full head SHA after each update.
Both `macos` and `omarchy` must pass on the current revision, the PR branch must
include the latest `main`, and review conversations must be resolved before merge.
While the project has one maintainer, an independent approval is not mandatory;
`CODEOWNERS` requests review but does not enforce merge authority.

### Commit messages and releases

Use Conventional Commits in the history merged into `main`, for example
`fix(terminal): preserve selection` or `feat(cli): add workspace selection`.
When squash-merging, preserve the intended Conventional Commit subject and any
breaking-change footer in the resulting commit.

The release planner considers all commits since the highest stable `vX.Y.Z` tag
reachable from the selected source. `feat` requests a minor release;
`fix`, `perf`, and `refactor` request a patch. A Conventional Commit subject
containing `!` or a `BREAKING CHANGE:` / `BREAKING-CHANGE:` footer requests a major
release, including before 1.0. The largest requested bump wins. Other types and
nonconventional commits do not request a release, so documentation-only or
maintenance-only changes do not produce one. Prerelease and unrelated branch
tags are ignored. Without a stable ancestor tag, the planner uses that source
commit's `VERSION` as the baseline and considers its full history.

Maintainers release through **Actions → Release → Run workflow** on `main`.
The workflow creates an automated `release/vX.Y.Z` version PR; contributors do
not manually bump versions or push release tags. The PR synchronizes `VERSION`,
the workspace package version in `Cargo.toml`, and local package versions in
`Cargo.lock`, without editing inherited crate manifests or resolving dependencies.
The configured maintainer-owned `RELEASE_TOKEN` creates the PR, allowing the
existing maintainer CI authorization to run normally. If CI does not run, review
the exact head and use the [manual CI runbook](GOVERNANCE.md#authorizing-ci);
the standard `GITHUB_TOKEN` cannot trigger PR CI when it creates or updates a PR.
Both required checks must pass on the exact current head, and the release branch
must include the latest `main`. If `main` advances, close the stale PR, delete
its release branch, and dispatch **Release** again; do not merge `main` into the
version branch. Merge only after review: the merge event verifies the version-only
change, tags the exact merged commit, and builds and publishes the release.
If a build or publication fails, rerun the original merged-PR workflow for
transient failures or corrected secrets. After merging workflow-only repairs, use
**Release → retry_version** to rebuild the unchanged existing tag with repaired
workflow steps, without bumping or moving the tag. Retries still check out the
tag's release scripts and vendored build code; fixes to those files require a
new release from the corrected source, not a retry of the broken tag.
Local release tooling requires Python 3.11 or newer; Actions installs Python 3.12
explicitly.
See the [release runbook](GOVERNANCE.md#releases) for credentials and operational
details. Main-branch protection is never bypassed.

Release metadata checks resolve the protocol's `CURRENT` constant to its wire
version before comparing each executable's `--build-info`. The Python regression
suite covers this resolution independently of the installed Rust binaries.
The vendored Ghostty build applies `lazy-benchmarks.patch` so disabled benchmarks
do not fetch their unused GUI dependencies (`vaxis`, `freetype`, and their
transitive packages) during terminal-library builds. Keep this patch when
updating the pinned Ghostty revision until the upstream build does the same.

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
