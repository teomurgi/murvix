# Troubleshooting

Common problems and fixes. If yours isn't here, please
[open an issue](https://github.com/teomurgi/Murvix/issues).

## Logs

| What | Where |
| --- | --- |
| Server | `server.log` in the [profile folder](../features/server.md#files) |
| Desktop app | Start it with `MUXY_DIAGNOSTICS=1` to write `app-diagnostics-<pid>.log` to the profile folder |
| Extensions | **Settings → Extensions →** the extension **→ Reveal log** |
| CLI and terminal UI | Printed in the terminal |

`murvix --build-info` and `murvix server status` show the versions in use.

## The app doesn't open

A malformed `settings.toml` (including unknown keys) can prevent startup. Start
the app from a terminal to see the error. On macOS:

```bash
"/Applications/Murvix.app/Contents/MacOS/murvix-app"
```

On a quick-installer Linux installation, run
`"${XDG_DATA_HOME:-$HOME/.local/share}/murvix-desktop/current/murvix-app"`.
For a Debian package, run `murvix-app`; for an AppImage, launch the downloaded
AppImage from the terminal.

Fix the line it names, or move `settings.toml` out of the profile folder to
start with defaults.

## The server won't start

If `murvix` reports `request timed out`, run the server in the foreground to see
the error, often a typo in `server.toml`:

```bash
"/Applications/Murvix.app/Contents/MacOS/murvix-server"   # or murvix-server, if installed on its own
```

## `murvix` is not found, or is the wrong version

On macOS, open **Settings → Installation & Updates** to inspect the CLI link,
login-shell resolution, and installed/running versions. Install or repair the
bundled link if necessary. First move the app into `/Applications` or
`~/Applications` and reopen it: DMG, Downloads, and App Translocation launches
cannot create persistent CLI, shell, or login integration.

The bundled CLI link and standalone installers use `~/.local/bin`; Debian
packages use `/usr/bin`. Optional macOS shell setup appends `~/.local/bin` in a
consented managed block for zsh, bash, or fish. It preserves unrelated startup
configuration and existing command precedence. Open a **new shell** afterward.
For custom or symlinked configurations, use
[manual PATH setup](../features/murvix-cli.md#install) instead.

Run `command -v murvix` and `murvix --version` to identify the command being used.
Homebrew, a standalone installation, an alias, or a function can shadow the
bundled CLI; Murvix reports this without overriding it. Use
`~/.local/bin/murvix` directly or adjust your own configuration deliberately.
Automatic relocation repair handles only receipt-owned Murvix links and leaves
unrelated files untouched. CLI and shell opt-outs are remembered.

## Updates and running sessions

On macOS, **Settings → Installation & Updates** distinguishes the installed
bundle, running app, running local server, and pending update versions. A newer
bundle on disk does not mean the processes have restarted. Automatic downloads
are **off by default**; enable them explicitly or use **Check for Updates…**.

Closing a window or quitting Murvix leaves its on-demand apphost server and
terminal sessions running. **Start Murvix at login** opens the app and is
independent of server persistence. If login activation says `requiresApproval`,
use **Open Login Items Settings** to approve it. If you revoke approval there,
Murvix does not silently re-enable it; review the native status and change it
yourself. **Do not start at login** unregisters the app.

A safe server restart requires **zero live sessions**, not merely no visible
tabs or no busy commands. Idle shells, detached sessions, and sessions held by
another desktop/CLI client still count. Inspect `murvix session list` and finish
or explicitly end those sessions before restarting. A compatible app update can
replace the app and CLI while preserving the existing server. An incompatible
update stays staged with the old installation active until all sessions end,
unless you explicitly confirm ending them. Server restarts do not hand off live
terminal processes.

**Destructive action:** stopping or restarting with force ends every live
terminal on that server, including detached sessions and those used by other
clients. Save your work first; use `murvix server stop --force` only when you
intend that result.

If the installed bundle or its parent directory is not writable, use manual
Finder recovery instead of repeatedly retrying in-app installation: finish
sessions, stop the server, quit Murvix, download the matching DMG, and replace
the app in `/Applications` (with Finder authorization if needed) or install it
in `~/Applications`. Reopen that installed copy, not the mounted DMG, and review
CLI health. Retained previous/staged bundles are not proof that the update is
running. Murvix keeps retired bundles while running processes require them and
cleans them after they become unused; do not delete those bundles while in use.
An unwritable installation directory does not require root for normal CLI/server
startup: Murvix uses a private per-user coordination lock when it cannot create
the installation lock. This does not grant permission to replace the app.

## A remote server won't connect

- **Host key not trusted:** run `ssh <host>` once and accept the key.
- **Login refused:** check that `ssh <host>` works with your key or agent. Only
  the desktop app supports passwords.
- **Murvix isn't installed:** install it from the **Remote** section, or run the
  installer there. See [Remote servers](../features/remote-servers.md).
- **Incompatible version:** install the same version on both computers. If the
  version is right but the old server is still running, run
  `murvix server stop --force` on that computer, then reconnect. This ends its terminals.
- **Terminals end when SSH disconnects (Linux):** run `loginctl enable-linger`.

## No agent notifications

- On macOS, allow notifications in **System Settings → Notifications → Murvix**.
  On Linux, check that your desktop's `org.freedesktop.Notifications` service is
  running and notifications are not muted.
- Murvix doesn't notify for the pane you're looking at, or for detached
  terminals.
- Recognition is best effort. `murvix activity list` shows what the server sees.

## Git or AI actions are disabled

- Install and sign in to a supported [AI tool](../features/git.md#ai-tools) in
  a terminal, then try again.
- Pull request features need `gh` installed and signed in on the project's
  server: `gh auth login`.
- AI tools run on the computer running the desktop app. For a remote project,
  the same folder path must exist locally.
- Actions are disabled on a clean tree, a detached HEAD, during conflicts, or
  while another action runs.

## Keys don't work as expected

- On macOS, Option acts as Alt in terminals. Choose **Neither** under
  **Settings → Terminal → Option as Alt** to type special characters.
- Key bindings in **Settings → Terminal → Key bindings** win over app
  shortcuts while a terminal is focused.

## Super+C interrupts the shell instead of copying (Omarchy)

Omarchy's universal copy binding sends `Ctrl+C` to windows it does not consider
terminals. Add this rule to `~/.config/hypr/hyprland.lua`, then run
`hyprctl reload`:

```lua
o.window("com\\.murvix\\.app", { tag = "+terminal" })
```

This tells Omarchy to send `Ctrl+Shift+C` to Murvix, which is the Linux copy
shortcut.

## Start fresh

1. Stop the server, which ends every terminal: `murvix server stop --force`.
2. Quit Murvix.
3. Move the [profile folder](../features/server.md#files) somewhere else.

The release profile is `~/Library/Application Support/Murvix` on macOS, or
`${XDG_STATE_HOME:-$HOME/.local/state}/murvix` on Linux. `MUXY_DIR` overrides it;
development builds use `Murvix Dev` / `murvix-dev`. Keep the moved folder until
you are sure you no longer need its settings or saved sessions.

The next launch may import legacy `settings.json` data if it finds an installed
1.x configuration at `~/Library/Application Support/Murvix`. On macOS, moving
the release profile already moves that location. On Linux, move that separate
legacy folder too if it exists and you want to avoid importing it.
`~/.config/ghostty` is not touched.

## Reporting a bug

Include your macOS or Linux version, the output of `murvix --build-info`, steps
to reproduce, and the relevant lines of `server.log`.
