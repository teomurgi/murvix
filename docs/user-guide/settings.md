# Settings

Open Settings with `Cmd+,` on macOS or `Ctrl+,` on Linux. Use the search box to
find a setting. App preferences work while the server is offline; server-backed
settings require a connection to the local server.

## Sections

| Section | What's in it |
| --- | --- |
| General | Closing behavior, the app that opens files, window size, worktree locations |
| Quick Terminal | Global shortcut, size, transparency |
| Composer | Dictation, clearing, image submission, font |
| Appearance | Language, light and dark themes, sidebar, status bar, tips |
| Keyboard | App shortcuts. See [Keyboard shortcuts](keyboard-shortcuts.md) |
| Commands | Your own commands, each run in a new tab, with optional shortcuts |
| Terminal | Fonts, spacing, transparency, vibrancy, colors, cursor, padding, selection, scrolling, Option keys, key bindings |
| Browser | The [built-in browser](../features/browser.md): terminal links, search engine, home page, profiles, import |
| Server | This computer's [server](../features/server.md#settings): shell, history, shell integration, stop and restart |
| Installation & Updates | macOS CLI and shell integration, app-at-login status, installed/running versions, and update controls |
| Mobile | [Mobile access](../features/mobile.md), TLS pairing, and paired devices |
| Extensions | Installed extensions and the marketplace |
| AI | Tools and prompts for [Git](../features/git.md) actions |
| Backup & Restore | Export, restore, and import from 1.x |

## Installation & Updates (macOS)

This section brings together CLI health, shell resolution, native login-item
status, and update status. It distinguishes the **installed bundle version**,
**running app version**, **running local server version**, and a **pending update**
when present: replacing the bundle does not replace processes already running.
Use **Refresh integration status** after moving the app or changing your shell.

- **Install or repair bundled CLI link** manages `~/.local/bin/murvix`. Only
  Murvix-owned links are repaired automatically after relocation; unrelated
  commands are preserved. First-run CLI and shell opt-outs survive relaunch.
- **Configure shell PATH…** discloses the exact files and managed block before
  asking permission. zsh, bash, and fish are supported; custom or symlinked
  configurations can use manual setup instead. The block appends the directory
  only when missing, without changing existing command precedence. Open a new
  shell after applying it; remove the marked block to undo it.
- **Start Murvix at login** registers the main app through macOS `SMAppService`.
  **Do not start at login** unregisters it. **Open Login Items Settings** lets
  you approve a pending request or review a revoked permission. Murvix reports
  macOS's actual status and does not silently restore revoked approval.
- **Automatically download updates** is **off by default**. Turn it on explicitly
  to allow background downloads; **Check for Updates…** remains available for a
  manual check. Downloads and installation are separate from restarting the app
  and server.

Run persistent setup only from `/Applications` or `~/Applications`, not a DMG
or translocated copy. Start at login opens the app; it is not required to keep
the on-demand apphost server alive after closing a window or quitting Murvix.
For session-safe restarts and manual bundle replacement, see
[Troubleshooting](troubleshooting.md#updates-and-running-sessions).

## Files

Settings are plain files in Murvix's
[profile folder](../features/server.md#files). **Edit in…** at the top of a
section opens its file.

| File | Holds |
| --- | --- |
| `settings.toml` | App preferences, shortcuts (`[keymap]`), commands, remote servers |
| `terminal.toml` | Terminal preferences and key bindings |
| `themes/` | Your own themes |
| `server.toml` | Server settings |
| `installation.json` | This profile's setup completion, CLI/shell choices, and automatic-download consent; login approval remains in macOS |

- Restart the app after editing `settings.toml` by hand. If the file has an
  error or an unknown key, the app won't open; see
  [Troubleshooting](troubleshooting.md#the-app-doesnt-open).
- Hand edits to `terminal.toml` apply when Murvix or Settings becomes active. If
  the file has an error, Settings shows it and keeps the last good values.

## Terminal configuration

Use **Settings → Terminal** for:

- Font family, fallback fonts, bold and italic fonts, size, line and character
  spacing, ligatures and other font features, fonts for Unicode ranges, thicker
  strokes, and bright colors for bold text.
- Background transparency and vibrancy sliders, optionally including colored cells.
  Vibrancy uses the native macOS window material, including its blur and desktop
  tinting. Increasing vibrancy reveals more of that material by reducing the
  terminal's solid color overlay; it also works with transparency set to zero.
  Reduce Transparency and Increase Contrast disable this material.
- Cursor shape, blinking, opacity, and thickness. Thickness accepts an adjustment in
  physical pixels or a percentage: `1` adds one pixel and `100%` doubles it.
  It applies to bar, underline, and outlined cursors. Terminal programs can
  override cursor shape and blinking.
- Separate sliders for left, right, top, and bottom padding, with balanced
  unused space and a padding color.
- Background, text, cursor, selection, and the 16 ANSI colors. Each replaces
  the theme's color; leave it empty to use the theme.
- Copy on select, clearing selection when typing or copying, mouse reporting,
  separate trackpad and mouse wheel speeds, and scrolling to live output.
- Whether both, neither, left, or right Option keys act as Alt.
- The working directory for new panes.
- Key bindings the terminal handles while focused: record a key, then pick an
  action such as sending text. They win over app shortcuts. You can also turn
  off the built-in terminal bindings.

Numeric controls use sliders, on/off options use switches, and fixed choices use
dropdowns. Sliders preview changes while dragging and save when released or when
Settings closes. Spacing and thickness offer a pixels/percent dropdown; changing
units keeps the numeric adjustment within the supported range.

**Reset terminal settings**, at the bottom of the section, returns everything
on it, including key bindings, to its default after you confirm.

Themes remain in **Settings → Appearance**. Shell and history limits remain
in **Settings → Server**. Quick Terminal opacity combines with terminal opacity.

On the first launch without `terminal.toml`, Murvix imports the supported values
from its old `ghostty.conf`, including included files and custom terminal key
bindings. The original files stay intact. Later edits to those old files have
no effect. Any unsupported legacy options appear under **Imported settings**
until you dismiss them.
Older backups and 1.x imports are converted to native preferences too. Legacy blur
and glass presets become native background vibrancy.

## Themes

Pick the app's bundled light and dark themes in **Settings → Appearance**, or
press `Cmd+Shift+K` on macOS / `Ctrl+Alt+K` on Linux. Murvix follows the system
appearance.

Custom themes in the profile's `themes/` folder show up too. The chosen theme
colors the app and its terminals.

## Backup and restore

- **Export…** saves a `.murvix` file with your settings, `terminal.toml`, themes,
  server settings, extension preferences, remote server list, and local
  projects with their layouts.
- **Choose file…** under **Restore backup** restores on the next launch. Murvix
  keeps a recovery copy in the profile's `Backups/` folder. Existing projects
  are kept; matching projects get the saved layouts. Restart the server from
  Settings to apply restored server settings.
- Backups never include running terminals, paired phones, extension packages,
  remote projects, or project files.

## Import from Murvix 1.x

**Import installed 1.x** reads Murvix 1.x's settings from this computer's
`~/Library/Application Support/Murvix` folder. **Import
projects** reads only its projects and worktrees. **Choose file…** takes a 1.x
`.murvix` backup or `settings.json`. Murvix imports supported settings, shortcuts,
custom commands, local projects, worktrees, and terminal tabs, and lists what it
skipped. Projects and tabs you already have are kept. **Import installed 1.x**
also brings the default shell and extensions.

## Extensions

**Settings → Extensions → Browse** installs extensions from the marketplace.
New extensions start disabled; review their permissions and turn them on.
Updates are manual: use **Update all**, or update one from its page. Building
your own? See [Extensions](../extensions/overview.md).

Language packs are extensions too. Pick a language in **Settings → Appearance →
App language**, or choose **Browse language extensions…**.

## Linux dictation

Dictation uses local tools on Linux. Install `parec` and `whisper-cli`, then set
**Settings → Composer → Offline voice model** to a multilingual GGML model.
Leaving it blank uses
`${XDG_DATA_HOME:-$HOME/.local/share}/murvix/voice/ggml-base.bin`.
The same `murvix/voice` folder must contain `ggml-silero-v6.2.0.bin` for speech
detection. Audio stays on the computer and is transcribed after recording ends.
