#!/bin/bash
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
if [[ $# -ne 2 ]]; then
    echo "Usage: $0 <X.Y.Z> <artifact-directory>" >&2
    echo "DRAFT=true keeps the release a draft instead of publishing it." >&2
    echo "RELEASE_NOTES, when set, replaces the generated release notes." >&2
    exit 1
fi
VERSION="$1"
python3 "$ROOT/scripts/release.py" check-version "$VERSION"
ARTIFACTS="$(cd "$2" && pwd)"
TAG="v$VERSION"
: "${GITHUB_REPOSITORY:?GITHUB_REPOSITORY is required}"
: "${GITHUB_SHA:?GITHUB_SHA is required}"
if [[ "${GITHUB_REF:-}" != refs/heads/main ]]; then
    echo "Error: releases are restricted to main" >&2
    exit 1
fi
cd "$ROOT"
SOURCE="$GITHUB_SHA"
if [[ "$(git rev-parse HEAD)" != "$SOURCE" ]]; then
    echo "Error: checkout does not match the release source" >&2
    exit 1
fi

check_source() {
    git fetch origin '+refs/heads/main:refs/remotes/origin/main' --tags
    git merge-base --is-ancestor "$SOURCE" origin/main
    if git show-ref --verify --quiet "refs/tags/$TAG"; then
        if [[ "$(git rev-parse "refs/tags/$TAG^{}")" != "$SOURCE" ]]; then
            echo "Error: $TAG already points to a different commit" >&2
            exit 1
        fi
    fi
}
check_source

cd "$ARTIFACTS"
ASSETS=(install-murvix.sh install-desktop-linux.py murvix-desktop.apparmor)
for ARCH in arm64 x86_64; do
    ASSETS+=("murvix-${VERSION}-linux-${ARCH}.tar.gz")
    ASSETS+=("murvix-desktop-${VERSION}-linux-${ARCH}.tar.gz" "Murvix-${VERSION}-linux-${ARCH}.AppImage")
    DEB_ARCH="$ARCH"
    [[ "$ARCH" != x86_64 ]] || DEB_ARCH=amd64
    ASSETS+=("murvix-desktop_${VERSION}_${DEB_ARCH}.deb")
    ASSETS+=("Murvix-${VERSION}-${ARCH}.dmg" "murvix-${VERSION}-macos-${ARCH}.zip")
done
for ASSET in "${ASSETS[@]}"; do
    if [[ ! -f "$ASSET" || ! -s "$ASSET" || -L "$ASSET" ]]; then
        echo "Error: missing regular release asset: $ASSET" >&2
        exit 1
    fi
done
python3 "$ROOT/scripts/release.py" update "$VERSION" "$GITHUB_REPOSITORY" "$ARTIFACTS"
ASSETS+=(update.json)
shasum -a 256 "${ASSETS[@]}" > SHA256SUMS
ASSETS+=(SHA256SUMS)

if gh release view "$TAG" --repo "$GITHUB_REPOSITORY" \
    --json isDraft,isPrerelease,targetCommitish > release.json; then
    python3 -c 'import json, sys; r = json.load(sys.stdin); sys.exit(0 if not r["isPrerelease"] and r["targetCommitish"] == sys.argv[1] else 1)' \
        "$SOURCE" < release.json
    if [[ "$(python3 -c 'import json, sys; print(json.load(sys.stdin)["isDraft"])' < release.json)" == False ]]; then
        echo "==> $TAG is already published; leaving its assets unchanged"
        exit 0
    fi
else
    # RELEASE_NOTES, when given, replaces the generated notes. Otherwise GitHub's
    # generated changes, cleaned; GitHub chooses the previous release.
    NOTES_ARGS=(-f "tag_name=$TAG" -f "target_commitish=$SOURCE")
    : > release-notes.md
    if [[ "${RELEASE_NOTES:-}" =~ [^[:space:]] ]]; then
        printf '%s\n' "$RELEASE_NOTES" >> release-notes.md
    else
        gh api --method POST "repos/$GITHUB_REPOSITORY/releases/generate-notes" "${NOTES_ARGS[@]}" --jq .body \
            | bash "$ROOT/scripts/clean-changelog.sh" >> release-notes.md
    fi
    gh release create "$TAG" --repo "$GITHUB_REPOSITORY" --target "$SOURCE" \
        --title "Murvix $VERSION" --draft --latest=false --notes-file release-notes.md
fi

# A failed upload leaves a resumable draft, never a half-populated public release.
gh release upload "$TAG" --repo "$GITHUB_REPOSITORY" --clobber "${ASSETS[@]}"
gh release view "$TAG" --repo "$GITHUB_REPOSITORY" --json assets > uploaded-assets.json
python3 - uploaded-assets.json "${ASSETS[@]}" <<'PY'
import json, pathlib, sys
remote = json.loads(pathlib.Path(sys.argv[1]).read_text())["assets"]
for name in sys.argv[2:]:
    matches = [asset for asset in remote if asset["name"] == name]
    if len(matches) != 1 or matches[0]["size"] != pathlib.Path(name).stat().st_size:
        sys.exit(f"Error: missing or incomplete uploaded asset: {name}")
PY
cd "$ROOT"
check_source
if [[ "${DRAFT:-false}" == true ]]; then
    echo "==> $TAG is a complete draft."
else
    # The latest release is the update feed of the desktop updater.
    gh release edit "$TAG" --repo "$GITHUB_REPOSITORY" --target "$SOURCE" \
        --draft=false --prerelease=false --latest
fi
