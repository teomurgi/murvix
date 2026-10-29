#!/bin/bash
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
if [[ $# -ne 3 ]]; then
    echo "Usage: $0 <version> <arm64|x86_64> <package-directory>" >&2
    exit 1
fi
export VERSION="$1" ARCH="$2"
PACKAGES="$(cd "$3" && pwd)"
docker run --rm -i --cap-add SYS_PTRACE -v "$ROOT:/src:ro" -v "$PACKAGES:/packages:ro" \
    -e VERSION -e ARCH ubuntu:24.04 bash -se <<'VERIFY_RUNTIME'
apt-get update
DEBIAN_FRONTEND=noninteractive apt-get install -y --no-install-recommends \
    python3 ca-certificates mesa-vulkan-drivers xvfb xauth dbus-x11 weston
useradd --create-home package-check
DEB_ARCH="$ARCH"
[[ "$ARCH" != x86_64 ]] || DEB_ARCH=amd64
MUXY_DISPOSABLE_PACKAGE_TEST=1 python3 /src/scripts/verify-desktop-formats.py deb \
    "/packages/murvix-desktop_${VERSION}_${DEB_ARCH}.deb" --version "$VERSION"
runuser -u package-check -- dbus-run-session -- xvfb-run -a \
    python3 /src/scripts/verify-desktop-formats.py appimage \
    "/packages/Murvix-$VERSION-linux-$ARCH.AppImage" --version "$VERSION" --desktop
runuser -u package-check -- python3 /src/scripts/verify-desktop-linux.py \
    "/packages/murvix-desktop-$VERSION-linux-$ARCH.tar.gz" --version "$VERSION"
VERIFY_RUNTIME
