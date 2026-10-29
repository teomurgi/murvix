#!/bin/bash
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
if [[ $# -ne 3 ]]; then
    echo "Usage: $0 <version> <arm64|x86_64> <package-directory>" >&2
    exit 1
fi
export VERSION="$1" ARCH="$2"
PACKAGES="$(cd "$3" && pwd)"
device_args=()
if [[ -c /dev/fuse ]]; then
    device_args=(--device /dev/fuse --cap-add SYS_ADMIN)
fi
docker run --rm --cap-add SYS_PTRACE "${device_args[@]}" -v "$ROOT:/src:ro" -v "$PACKAGES:/packages:ro" -e VERSION -e ARCH ubuntu:24.04 bash -euc '
    apt-get update
    DEBIAN_FRONTEND=noninteractive apt-get install -y --no-install-recommends \
      python3 ca-certificates mesa-vulkan-drivers xvfb xauth dbus-x11 weston
    useradd --create-home package-check
    DEB_ARCH="$ARCH"
    [[ "$ARCH" != x86_64 ]] || DEB_ARCH=amd64
    PACKAGE=murvix-desktop
    [[ "$VERSION" != *-beta.* ]] || PACKAGE=murvix-desktop-beta
    MUXY_DISPOSABLE_PACKAGE_TEST=1 python3 /src/scripts/verify-desktop-formats.py deb \
      "/packages/${PACKAGE}_${VERSION}_${DEB_ARCH}.deb" --version "$VERSION"
    fuse_args=()
    if [[ -c /dev/fuse ]]; then
        fuse_args=(--fuse)
    else
        echo "No /dev/fuse in the verification container; checking the AppImage in extraction mode"
    fi
    runuser -u package-check -- dbus-run-session -- xvfb-run -a \
      python3 /src/scripts/verify-desktop-formats.py appimage \
      "/packages/Murvix-$VERSION-linux-$ARCH.AppImage" --version "$VERSION" --desktop "${fuse_args[@]}"
    runuser -u package-check -- python3 /src/scripts/verify-desktop-linux.py \
      "/packages/murvix-desktop-$VERSION-linux-$ARCH.tar.gz" --version "$VERSION"
'
