#!/bin/bash
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
if [[ $# -ne 2 ]]; then
    echo "Usage: $0 <X.Y.Z> <artifact-directory>" >&2
    echo "Run on the tag's commit with the tag present; creates v<X.Y.Z> with" >&2
    echo "generated notes when it is missing, then uploads the artifacts." >&2
    exit 1
fi
VERSION="$1"
python3 "$ROOT/scripts/release.py" check-version "$VERSION"
ARTIFACTS="$(cd "$2" && pwd)"
TAG="v$VERSION"
: "${GITHUB_REPOSITORY:?GITHUB_REPOSITORY is required}"
cd "$ROOT"
# The pushed tag is the release: it must exist locally and name this version.
SOURCE="$(git rev-parse --verify "refs/tags/$TAG^{commit}")"
if [[ "$(git rev-parse HEAD)" != "$SOURCE" ]]; then
    echo "Error: checkout does not match $TAG" >&2
    exit 1
fi

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

if ! gh release view "$TAG" --repo "$GITHUB_REPOSITORY" \
    --json isDraft,isPrerelease,targetCommitish > release.json; then
    # GitHub's generated changes, cleaned; GitHub chooses the previous release.
    gh api --method POST "repos/$GITHUB_REPOSITORY/releases/generate-notes" \
        -f "tag_name=$TAG" -f "target_commitish=$SOURCE" --jq .body \
        | bash "$ROOT/scripts/clean-changelog.sh" > release-notes.md
    gh release create "$TAG" --repo "$GITHUB_REPOSITORY" --target "$SOURCE" \
        --title "Murvix $VERSION" --latest=false --notes-file release-notes.md
    gh release view "$TAG" --repo "$GITHUB_REPOSITORY" \
        --json isDraft,isPrerelease,targetCommitish > release.json
fi
python3 -c 'import json, sys; r = json.load(sys.stdin); sys.exit(0 if not r["isPrerelease"] and r["targetCommitish"] == sys.argv[1] else 1)' \
    "$SOURCE" < release.json
if [[ "$(python3 -c 'import json, sys; print(json.load(sys.stdin)["isDraft"])' < release.json)" == False ]]; then
    echo "==> $TAG is already published; leaving its assets unchanged"
    exit 0
fi

# A failed upload leaves an unfinished draft, never a half-populated public release.
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
# The latest release is the update feed of the desktop updater.
gh release edit "$TAG" --repo "$GITHUB_REPOSITORY" --target "$SOURCE" \
    --draft=false --prerelease=false --latest
