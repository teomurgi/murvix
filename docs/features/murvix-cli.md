# Murvix CLI

The `murvix` command talks to the [server](server.md) from a shell. It manages
projects, terminal sessions, worktrees, server settings, and phone access. No
desktop app is needed, so it works on Linux and over SSH too.

Commands work with what the server owns: projects and sessions. Tabs and panes
belong to each app, so there are no commands for them. Browser tabs
are the exception: they live in the desktop app, and `murvix browser` talks to
the app on this computer. The [terminal UI](terminal-ui.md) is an app of its
own, with its own layout.

## Install

- **With the desktop app:** choose **Install Command Line Tool…** from the app
  menu. It links `~/.local/bin/murvix` to the copy inside the app.
- **On its own:** see [Getting started](../user-guide/getting-started.md#cli-and-server-only).

Make sure `~/.local/bin` is on your `PATH`.

## Basics

```bash
murvix                 # open the terminal UI
murvix .               # open this folder in the desktop app or the terminal UI
murvix project list    # list projects
murvix --help          # all commands; murvix <command> --help for details
```

- Commands start the server they use if it isn't running, including a remote
  one with `--host`. `murvix server status` and `murvix server stop` never start
  it.
- Management commands take `--json` for machine-readable output, except
  `murvix mobile`.
- Ending or discarding a session and deleting a project or worktree need
  `--yes`.
- A project can be named by its ID, its name if unique, or its folder.
- `--host <destination>` runs the command against the server on another
  computer. It must come first: `murvix --host devbox project list`. See
  [Remote servers](remote-servers.md).

## Commands

| Command | What it does |
| --- | --- |
| `murvix` | Opens the [terminal UI](terminal-ui.md). |
| `murvix <folder>` | Opens the folder as a project in the desktop app. On Linux, over SSH, or without the desktop app, opens it in the terminal UI. |
| `murvix server start\|status\|stop` | Starts, inspects, or stops the server. `stop` refuses while terminals run; add `--force` to end them. |
| `murvix project list\|add\|rename\|set-color\|set-icon\|workspace\|clear-workspace\|delete` | Manages projects. Deleting ends its terminals but keeps files on disk. `workspace` sets the extra roots every agent in its terminals sees, alongside the project directory. JSON lists carry `workspace`/`extra_directories` text plus `workspace_bytes`/`extra_directory_bytes` byte arrays. |
| `murvix settings get\|set` | Reads or changes [server settings](server.md#settings). |
| `murvix activity list\|ack` | Reads or clears [AI agent](ai-agents.md) activity. |
| `murvix browser open\|list\|read\|click\|…` | Drives browser tabs in the desktop app on this computer. See [Browser control](#browser-control). |
| `murvix exec <project> -- <program> [args]` | Runs a program in the project folder, without a shell. |
| `murvix git`, `murvix files` | Run raw server Git and file actions as JSON. Advanced; the JSON shape follows the protocol and may change. |
| `murvix mobile` | Turns on phone access and pairs phones. See [Mobile](mobile.md). |
| `murvix install-skills` | Installs the Murvix CLI skill for the AI agents found in your home folder. `--dir <path>` adds another skills folder. |
| `murvix stdio` | Joins stdin and stdout to the server. Used by `--host` over SSH; you don't run it yourself. |

## Sessions from a script

```bash
id=$(murvix session create my-app)
murvix session send "$id" "npm test"
murvix session send-keys "$id" Enter
murvix session read-screen "$id" --lines 20
murvix session end "$id" --yes
```

- `send` types text without pressing Return. `send-keys` sends `Enter`, `Tab`,
  `Escape`, `Backspace`, `Ctrl+C`, `Ctrl+D`, or `Ctrl+Z`.
- `read-screen` returns the last visible rows. `history` and `search` page
  through scrollback as JSON.
- `--saved` reads a session's last saved screen, even after it ended.
- `end` stops the program and keeps its saved output. `discard` also deletes
  the output.
- Sessions created here outlive the command and appear in every app under
  **Existing Terminals**.
- Inside a Murvix terminal, session commands default to that terminal, and
  `create` to its project.
- `create` can take a `--directory` and a command to run after `--`.
- `wait --text <text>` waits for text on the screen, and `wait --exit` for the
  shell to exit. Both give up after 30 seconds by default.

## Browser control

```bash
tab=$(murvix browser open localhost:3000)
murvix browser wait-for-navigation "$tab"
murvix browser fill "$tab" "#email" "me@example.com"
murvix browser click "$tab" "button[type=submit]"
murvix browser wait "$tab" --text "Welcome"
murvix browser read "$tab"
murvix browser screenshot "$tab" | base64 -D > page.png
murvix browser close "$tab"
```

- Browser commands need the desktop app and don't take `--host`. `open` starts
  the app if it isn't running.
- `open` prints the new tab's ID. Pass it to the other commands.
- `snapshot` lists a page's interactive elements, each with a selector for
  `click`, `fill`, or `type`.
- `murvix browser --help` lists every command.

## Exit codes

`0` on success. On any error, `murvix` prints `murvix: <message>` to stderr and exits
with `1`, including when a program run with `murvix exec` fails.

## Profiles

`MUXY_DIR` points the CLI, and the server it starts, at another profile folder.
Use it to run a separate, throwaway server. See [Server](server.md#files).
