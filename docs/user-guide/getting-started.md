# Getting Started

Start with [Building from source](#building-from-source). Release packaging:
[macOS](#macos-desktop-app) · [Linux](#linux-desktop-app) ·
[CLI and server only](#cli-and-server-only).

After installing: [First steps](#first-steps) · [Importing from 1.x](#importing-from-murvix-1x)

This fork has no published GitHub release at present. Build from source below.
The later package names and installer commands document the repository's release
packaging, not currently available downloads. Use them only after a release
containing the required assets has been published.

## Requirements

| Part | Runs on |
| --- | --- |
| Desktop app | macOS 14 or newer, on Apple Silicon or Intel; experimental Linux packages target Ubuntu 24.04 on x86_64 and ARM64 |
| `murvix` CLI and server | macOS 14 or newer, or Linux with glibc 2.35 or newer, on x86_64 or ARM64. musl is unsupported. |

## Building from source

Install Git, [Rust](https://rustup.rs) (the repository pins its toolchain), and
[Zig 0.15.2](https://ziglang.org/download/). On macOS, also install the Xcode
Command Line Tools (`xcode-select --install`).
On Linux, install a C/C++ build toolchain and `pkg-config` as well.

```sh
git clone https://github.com/teomurgi/Murvix.git
cd Murvix
```

For the CLI and server on macOS or Linux:

```sh
cargo build --locked -p murvix-cli -p murvix-server
./target/debug/murvix
```

For the macOS desktop app, build the matching app and helpers together:

```sh
cargo build --locked -p murvix-app -p murvix-cli -p murvix-server
./target/debug/murvix-app
```

On Arch Linux / Omarchy, the repository includes a source package. Install the
Arch build tools (`base-devel`), `rustup`, and the desktop WebKit runtime
(`webkit2gtk-4.1`), then, from the checkout:

```sh
cd packaging/linux
makepkg -si
```

This recipe fetches the fork's `main` branch and Zig 0.15.2, builds the desktop
app and helpers, and installs them in `/usr/bin`. It is not a prebuilt release.
Other Linux desktop source builds can use `scripts/build-desktop-linux.sh` with
the required native development libraries installed; see
[Linux platform constraints](../tech/constraints.md#experimental-linux-desktop).

Debug builds keep data separate from installed release builds: `Murvix Dev` on
macOS and `murvix-dev` on Linux. See [Contributing](../../CONTRIBUTING.md) for
development details.

## macOS desktop app

1. From the [latest release](https://github.com/teomurgi/Murvix/releases/latest),
   download `Murvix-<version>-arm64.dmg` for Apple Silicon or
   `Murvix-<version>-x86_64.dmg` for Intel.
2. Drag **Murvix** to `/Applications` and open it.

To use [`murvix`](../features/murvix-cli.md) in your shell, choose **Install Command
Line Tool…** from the app menu. It links `~/.local/bin/murvix`.

Packaged macOS builds include an in-app updater; it requires a published update
feed. Running terminals normally remain on the existing server until it restarts.

## Linux desktop app

Linux support is **experimental**, targeting Ubuntu 24.04 on x86_64 and ARM64,
with glibc 2.39 or newer and a working Vulkan driver. See the
[platform limits and WebKit sandbox setup](../tech/constraints.md#experimental-linux-desktop).

Choose the [quick installer](#quick-installer-with-in-app-updates) for in-app
updates, a [Debian package](#debian-package), or an [AppImage](#appimage).
All three include the desktop app, CLI and server. Use one installation format
to avoid conflicting CLI commands.

### Quick installer with in-app updates

Install the Ubuntu dependencies once:

```sh
sudo apt update
sudo apt install curl python3 ca-certificates libfreetype6 libfontconfig1 libxcb1 \
  libxkbcommon0 libxkbcommon-x11-0 libwayland-client0 libvulkan1 mesa-vulkan-drivers \
  libwebkit2gtk-4.1-0 xdg-utils xdg-desktop-portal bubblewrap xdg-dbus-proxy
```

Install the **latest stable release** as your normal user, without `sudo`.
The installer detects your architecture and verifies the download:

```sh
curl -fsSL --proto '=https' --proto-redir '=https' \
  https://github.com/teomurgi/Murvix/releases/latest/download/install-desktop-linux.py \
  | python3 - install
```

Open **Murvix** from Applications. The app, CLI and server install into
`~/.local/share/murvix-desktop` by default. Add `~/.local/bin` to your shell's
`PATH` to use `murvix <folder>` from a terminal.

### Debian package

Download `murvix-desktop_<version>_amd64.deb` or
`murvix-desktop_<version>_arm64.deb` from the
[latest stable release](https://github.com/teomurgi/Murvix/releases/latest).
Use `dpkg --print-architecture` to check which you need. In your download
directory, replace the filename below with your download:

```sh
sudo apt install './murvix-desktop_<version>_amd64.deb'
```

APT installs the dependencies and adds **Murvix** to Applications, with `murvix`
and `murvix-server` in `/usr/bin`.

### AppImage

Install the [runtime libraries above](#quick-installer-with-in-app-updates), then
download `Murvix-<version>-linux-x86_64.AppImage` or
`Murvix-<version>-linux-arm64.AppImage` from the
[latest stable release](https://github.com/teomurgi/Murvix/releases/latest).
Use `uname -m` to check: `aarch64` means ARM64. Replace the filename below with
your download and run it without `sudo`:

```sh
chmod +x './Murvix-<version>-linux-x86_64.AppImage'
'./Murvix-<version>-linux-x86_64.AppImage'
```

The first launch adds **Murvix** to Applications. AppImage uses **system WebKitGTK**.
If FUSE is unavailable, append `--appimage-extract-and-run` when launching it.

Murvix keeps an unpacked copy in
`${XDG_DATA_HOME:-$HOME/.local/share}/murvix-appimage/<bundle-id>` so terminals can
keep running after the window closes. This takes extra disk space. Use
`./Murvix-<version>-linux-x86_64.AppImage --cli <arguments>` for the bundled CLI,
or choose **Install Command Line Tool…** in the app to link it into `~/.local/bin`.

### Updates

- **Quick installer:** the app checks automatically; **Check for Updates** checks now.
- **Debian package:** stop the server and quit Murvix, then download the newer `.deb`
  and install it with `sudo apt install`. There is no Murvix APT repository, so
  `apt upgrade` alone does not update Murvix.
- **AppImage:** quit the app, then download and run the newer file. Running
  terminals stay alive. If you linked the CLI, choose **Install Command Line Tool…**
  again from the new app.

Debian packages and AppImages do not use the in-app updater.

### Remove

Stop the local server in **Settings → Server**, then quit Murvix. Stopping the
server ends its running terminals. Removal keeps your settings, projects and
saved session data.

- **Quick installer:** run the command below.

  ```sh
  python3 "${XDG_DATA_HOME:-$HOME/.local/share}/murvix-desktop/current/install.py" uninstall
  ```

- **Debian package:** run `sudo apt remove murvix-desktop`.
- **AppImage:** delete the downloaded file, its matching unpacked copy listed
  above, and any CLI link or Applications entry pointing into that copy. The entry
  is in `${XDG_DATA_HOME:-$HOME/.local/share}/applications/com.murvix.app.desktop`
  Do not delete a copy still used by another running app or server.

## CLI and server only

For a Linux computer or Mac without the desktop app. This installs `murvix` and
`murvix-server`.

### Install script

Set `VERSION` to the version on the
[releases page](https://github.com/teomurgi/Murvix/releases/latest), without the
leading `v`:

```bash
VERSION='<version>'
curl -fsSL "https://github.com/teomurgi/Murvix/releases/download/v$VERSION/install-murvix.sh" \
  | sh -s -- --version "$VERSION"
export PATH="$HOME/.local/bin:$PATH"
```

The script picks the build for your OS and architecture, verifies its checksum,
and installs both commands into `~/.local/bin`. Add the `export` line to your
shell's startup file to keep them on `PATH`.

- **Update:** run it again with the new version and `--replace`.
- **Another folder:** add `--install-dir PATH`.

### Check it

Run `murvix --version` and `murvix-server --version`, then `murvix` to open the
[terminal UI](../features/terminal-ui.md). Updating them never restarts a
running [server](../features/server.md#lifetime); stop it to switch to the new
version.

## Importing from Murvix 1.x

If you have an installed Murvix 1.x configuration, the first launch imports it
unless the current profile already has projects:
supported settings, shortcuts, local projects, terminal tabs, the default shell,
and extensions. Projects and extensions that could not be imported are listed,
and your 1.x data stays where it was. To import again later, open
**Settings → Backup & Restore** and choose **Import installed 1.x**; it adds to
what you already have.

Some 1.x features work differently or are not available. See
[Legacy compatibility](../extensions/get-started.md#legacy-compatibility) for extension migration.

## First steps

1. **Add a project.** Choose **Add Project** at the bottom of the sidebar, or
   press `Cmd+O` on macOS / `Ctrl+Shift+O` on Linux, and pick a folder.
2. **Open a terminal.** `Cmd+T` / `Ctrl+Shift+T` opens a tab in the project.
   `Cmd+N` / `Ctrl+Shift+N` opens one in Home.
3. **Split it.** `Cmd+D` / `Ctrl+Shift+D` splits right;
   `Cmd+Shift+D` / `Ctrl+Shift+Alt+D` splits down.
4. **Quit freely.** Terminals keep running in the background. Reopen Murvix to
   reconnect to them.

## Next

- [How Murvix works](../features/server.md): the server, sessions, and history.
- [Projects](../features/projects.md), [Terminal](../features/terminal.md), and
  [Keyboard shortcuts](keyboard-shortcuts.md).
- [Remote servers](../features/remote-servers.md).
- [Settings](settings.md) and [Troubleshooting](troubleshooting.md).
