# Bundled extensions

`files` and `git` are vendored from [`muxy-app/extensions`](https://github.com/muxy-app/extensions)
and adapted to Murvix: the `murvix` manifest key, the `window.murvix` JS API,
`--murvix-*` theme variables, and `@murvix/ui` for the shared component library.
No `muxy` references remain outside historical git-ref names (see below).

## Layout

- `files/`, `git/` — extension sources (`src/`, `panel/`, `modal/`, …).
- `shared/` — `@murvix/ui` tokens and components the git extension imports.
  `git/vite.config.js` aliases it via `../shared/ui`.
- `manifest.json` — the marketplace index the app fetches from the release tag:
  `https://raw.githubusercontent.com/teomurgi/murvix/v<app-version>/extensions/manifest.json`.
- `pack.mjs` — builds `dist/<name>-<version>.zip` archives and regenerates
  `manifest.json`. Commit both the manifest and any source changes.
- `dist/` — local pack output (gitignored); the release workflow rebuilds and
  uploads these as GitHub release assets.

## Workflow

```sh
cd extensions/files && npm ci && npm run build
cd ../git && npm ci && npm run build
cd .. && node pack.mjs   # writes dist/*.zip + manifest.json
```

The release job rebuilds, verifies each archive's size/sha256 against
`manifest.json`, and `scripts/publish-release.sh` uploads the zips alongside
the app artifacts. The app constructs each `download_url` from the tag, so the
manifest carries no per-release URLs.

## Deliberately unrenamed

- `refs/muxy/pr/*` fetch refspecs and `branch.*.muxy-pr-number` git config
  written into user repos by the git extension: renaming orphans existing
  PR-tracking branches.
- `murvix-color-picker*` class names, the `murvix-open-replace` custom event,
  and the `murvix-diffs` highlighter theme: self-contained internals, never
  cross the app boundary.
