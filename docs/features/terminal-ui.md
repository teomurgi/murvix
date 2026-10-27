# Terminal UI

Run `murvix` with no arguments to open Murvix inside any terminal. It shows the
same projects and terminals as the desktop app, with its own tabs and splits.
It works on macOS and Linux, locally or over SSH.

```bash
murvix                   # this computer's server
murvix --host devbox     # the server on another computer, over SSH
```

## Keys

Layout commands start with the prefix `Ctrl-B`, like tmux; ordinary keys go to
the focused terminal. Press `Ctrl-B ?` for help.

| After `Ctrl-B` | Action |
| --- | --- |
| `c` | New tab in the project folder |
| `n` / `p` | Next / previous tab |
| `1`–`9`, `0` | Go to tabs 1–9, or tab 10 |
| `%` / `v` | Split side by side |
| `"` / `-` | Split top and bottom |
| Arrows / `h`, `j`, `k`, `l` | Move focus between panes |
| `Ctrl` or `Alt` + Arrows / `h`, `j`, `k`, `l` | Resize the split |
| `r` | Enter resize mode; use arrows or `h`, `j`, `k`, `l`, then `Esc`, `Enter`, `q`, or `r` to leave |
| `o` / `Tab`; `Shift-Tab` | Next pane; previous pane |
| `z` | Zoom the pane |
| `x` | Close the pane |
| `s` / `g` | Switch project |
| `(` / `)` | Previous / next project |
| `b` | Toggle the sidebar |
| `m` | Toggle mouse support |
| `[` | Enter scrollback |
| `w` | Open an existing terminal of this project |
| `d` | Detach and quit; terminals keep running |
| `Ctrl-B` | Send `Ctrl-B` to the terminal |

## Behavior

- The first run opens one shell in Home. After that, it restores your layout.
- TUI instances on the **same computer and profile**, connected to the same
  server, share a saved layout. Layout mutations reload the shared file before
  saving; this is not live layout synchronization between different computers
  or an automatic mirror of every other window.
- Quitting or detaching does not end a terminal. Closing its last pane ends it
  **and discards its saved output**, unless another connected client still
  references it. Closing asks for confirmation if a foreground program may be
  running.
- With `--host`, the layout is kept locally at
  `servers/<server-id>/tui-state.json`, separately from the local server's layout.
- A tab holds up to 16 panes, a project up to 64 tabs, and one TUI layout up to
  256 panes in total.
- When a terminal exits, the TUI removes its panes. If no other connected
  client references that session, this also discards its saved output; do not
  rely on an exited TUI pane remaining available as a saved transcript.
- The sidebar shows AI agent activity.

## Mouse and scrollback

Click to focus a pane or pick a project or tab, and drag a divider to resize.
Drag across text to select it; it is copied on release. Double-click selects a
word and triple-click a line. Right-click opens a menu. When a program uses the
mouse, hold `Shift` to select.

`Ctrl-B [` enters scrollback; `Ctrl-B PageUp` also scrolls back a page.
Use arrows or `j`/`k` for rows, `PageUp`/`PageDown` for pages, and `Home`/`g`
for the oldest loaded history. `Ctrl-U`/`Ctrl-D` move half a page; `Space` or
`Ctrl-F` moves a page toward the live screen. `q`, `Esc`, `End`, or `G` returns
to the live screen. `y` or `Enter` copies a mouse selection and returns live.

The mouse wheel scrolls history unless mouse tracking or alternate-screen
scrolling sends it to the terminal program; hold `Shift` to scroll history
instead. Clipboard copying uses a system clipboard tool when available, or
the host terminal's OSC 52 clipboard support; the host may require permission.

## Not supported

The terminal UI has no custom key bindings or
[project layouts](../layouts/overview.md), and doesn't show images in the
terminal. Desktop and terminal UI layouts are kept separately.
