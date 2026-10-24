# Agents

## Repository

- Before finishing: `cargo fmt --all --check`, `cargo clippy --workspace --all-targets --all-features -- -D warnings`, `cargo test --workspace --all-features`, `cargo doc --workspace --no-deps` with `RUSTDOCFLAGS=-D warnings`.

## Commits and versioning

- Use Conventional Commits (`type(scope): description`, e.g. `fix(ags): ...`, `feat(ui): ...`).
- Follow SemVer (`MAJOR.MINOR.PATCH`); do not use `-beta*` (or any pre-release) suffixes — releases are stable versions only.
- Bump the version in `BETA_VERSION` (and keep `Cargo.toml`/`Cargo.lock` in sync via `scripts/release.py stamp`) whenever a change warrants a new release, choosing the bump level from the conventional commit types (breaking → major, feat → minor, fix/perf/refactor → patch).
- Never commit directly to `main` unless asked; work on a branch and open a PR.

## Main Guides

- Only test real scenarios and risks (data loss, security, compatibility, concurrency, regressions). Don't test obvious code or defaults.

## Third-party dependencies

When adding new third-party dependencies, provide user with their github url and stars and metrics if they are being maintained so user can approve adding it.

## Performance

- Do not ignore performance and memory efficiency. with GPUI it is easy to cause high CPU and memory usage.

#
