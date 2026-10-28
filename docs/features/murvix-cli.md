# Murvix CLI

The `murvix` command talks to the [server](server.md) from a shell. It manages
projects, terminal sessions, worktrees, server settings, and paired-device access.
No desktop app is needed, so it works on Linux and over SSH too.

Commands work with what the server owns: projects and sessions. Tabs and panes
belong to each app, so there are no commands for them. Browser tabs
are the exception: they live in the desktop app, and `murvix browser` talks to
the app on this computer. The [terminal UI](terminal-ui.md) is an app of its
own, with its own layout.

## Install

- **With the macOS desktop app:** accept CLI setup on first launch, or use
  **Settings → Installation & Updates → Install or repair bundled CLI link**
  (**Install Command Line Tool…** in the app menu is also available). Run the
  app from `/Applications` or `~/Applications`, not the DMG. The link at
  `~/.local/bin/murvix` points inside the app; an ownership receipt lets Murvix
  repair its own link after relocation without replacing unrelated files.
- **With the Linux desktop app:** choose **Install Command Line Tool…** from
  the app menu to link the bundled CLI into `~/.local/bin`.
- **On its own:** see [Getting started](../user-guide/getting-started.md#cli-and-server-only).

On macOS, optional **Configure shell PATH…** shows the files and managed block
before requesting consent. It updates only the marked block, preserving the
rest of your configuration:

| Shell | Startup files |
| --- | --- |
| zsh | `.zshrc` under `$ZDOTDIR`, or your home folder if unset |
| bash | `~/.bashrc` plus the first existing `~/.bash_profile`, `~/.bash_login`, or `~/.profile` (creates `.bash_profile` if none exists) |
| fish | `fish/config.fish` under `$XDG_CONFIG_HOME`, or `~/.config` if unset |

The block appends `~/.local/bin` **only if absent**. It does not move it earlier
on PATH or override a Homebrew/standalone command, alias, or function. If another
command shadows Murvix, setup reports that rather than taking over precedence.
Run `command -v murvix` and `murvix --version` in a **new shell** to check;
already-open terminals do not receive the changed environment.

For custom startup files, unsupported shells, or symlinked configuration files,
choose manual setup instead. Append the directory in your own configuration:

```sh
# zsh or bash
export PATH="${PATH:+$PATH:}$HOME/.local/bin"
```

```fish
if not contains -- "$HOME/.local/bin" $PATH
    set -gx PATH $PATH "$HOME/.local/bin"
end
```

Or invoke `~/.local/bin/murvix` directly without changing PATH. Skipping CLI or
shell setup is remembered; it is not silently enabled on the next launch.
Remove only the `# >>> Murvix managed PATH >>>` through
`# <<< Murvix managed PATH <<<` block to undo managed shell setup. Adjust other
installations and aliases yourself only if you want to change which command wins.

## Basics

```bash
murvix                 # open the terminal UI
murvix .               # open this folder in the desktop app or the terminal UI
murvix project list    # list projects
murvix --help          # all commands; murvix <command> --help for details
```

- Server-backed commands start the server they use if it isn't running, including
  a remote one with `--host`. `murvix server status` and `murvix server stop`
  never start it. Help, version information, and skill installation need no server.
- Management commands accept `--json`; `mobile` does not. Settings, server
  start/status, Git, files, and session history/search already return JSON.
- Ending or discarding a session and deleting a project or worktree need
  `--yes`.
- A project can be named by its ID, its name if unique, or its folder.
- `--host <destination>` selects the server on another computer. It must come
  first: `murvix --host devbox project list`. It supports the terminal UI,
  management commands, and mobile-access management, not folder-opening,
  browser, or skill-installation commands. See [Remote servers](remote-servers.md).

## Commands

| Command | What it does |
| --- | --- |
| `murvix` | Opens the [terminal UI](terminal-ui.md). |
| `murvix <folder>` | Opens the folder as a project in the desktop app on macOS or Linux. Over SSH, without a desktop session, or if the app cannot launch, opens it in the terminal UI. Use `./NAME` for a folder named like a command. |
| `murvix server start\|status\|stop` | Starts, inspects, or stops the server. `stop` refuses while terminals run; add `--force` to end them. |
| `murvix project list\|add\|rename\|set-color\|set-icon\|workspace\|clear-workspace\|here\|delete` | Manages projects. Deleting ends its terminals and deletes child projects but keeps files on disk. `workspace` sets the extra roots advertised to agents alongside the project directory; `here` prints the calling terminal's roots. JSON lists carry `workspace`/`extra_directories` text plus `workspace_bytes`/`extra_directory_bytes` byte arrays. |
| `murvix session list\|create\|send\|send-keys\|read-screen\|history\|search\|wait\|end\|discard` | Creates and controls terminal sessions; see below. |
| `murvix worktree list\|create\|checkout-pr\|register\|remove` | Manages Git worktree projects. Setup/teardown hooks run only with `--hooks`; removal needs `--yes` and refuses uncommitted changes without `--force`. |
| `murvix settings get\|set` | Reads or changes [server settings](server.md#settings). |
| `murvix activity list\|ack` | Reads or clears [AI agent](ai-agents.md) activity. |
| `murvix browser open\|list\|read\|click\|…` | Drives browser tabs in the desktop app on this computer. See [Browser control](#browser-control). |
| `murvix exec <project> -- <program> [args]` | Runs a program in the project folder, without a shell. |
| `murvix git`, `murvix files` | Run raw server Git and file actions as JSON. Advanced; the JSON shape follows the protocol and may change. |
| `murvix mobile` | Manages the server's network listener, pairing codes, and paired devices. Phone apps are not shipped in this repository; see [Mobile](mobile.md). |
| `murvix install-skills` | Installs the Murvix CLI skill for the AI agents found in your home folder. `--dir <path>` adds another skills folder. |
| `murvix stdio` | Joins stdin and stdout to the server. Used by `--host` over SSH; you don't run it yourself. |

`project add` creates a distinct project even if the directory is already
registered; use `--reuse` to return the existing project instead, or `--create`
to create a missing directory. `worktree create` creates a branch from `HEAD`
by default; use `--base <ref>` or `--existing` to select another starting point.

`exec` defaults to a 30-second timeout; set `--timeout-ms` up to `300000`.
For shell syntax, explicitly run a shell:
`murvix exec Home -- /bin/sh -c 'printf "%s\n" hello'`.

## Sessions from a script

```bash
id=$(murvix session create Home)
murvix session send "$id" "printf 'Hello from %s\n' Murvix"
murvix session send-keys "$id" Enter
murvix session wait "$id" --text "Hello from Murvix"
murvix session read-screen "$id"
murvix session end "$id" --yes
```

- `send` types text without pressing Return. `send-keys` sends `Enter`, `Tab`,
  `Escape`, `Backspace`, `Ctrl+C`, `Ctrl+D`, or `Ctrl+Z`.
- `read-screen` returns the last visible rows (50 by default), including blank
  rows at the bottom of the screen; it does not trim to the last output.
  `history` and `search` page through scrollback as JSON.
- `--saved` reads a session's last saved screen, even after it ended.
- `end` stops the program and keeps its saved output. `discard` also deletes
  the output.
- Sessions created here outlive the command and appear in every app under
  **Existing Terminals**.
- Inside a Murvix terminal, session commands default to that terminal, and
  `create` to its project.
- `create` can take `--directory`, `--cols`, `--rows`, and a command after `--`.
  It starts a shell and types that command followed by Return; it is not a
  direct process launch like `exec`. Arguments are joined with spaces without
  shell quoting, so pass a whole quoted command when its shell syntax matters:
  `murvix session create Home -- 'exec /bin/sh -c "printf hello; sleep 2"'`.
- `wait --text <text>` waits for text on the screen, and `wait --exit` for the
  shell to exit. Both give up after 30 seconds by default; `--timeout-ms`
  accepts up to `3600000`. Use `exec` in the command typed into the session if
  `--exit` should wait for that program rather than its enclosing shell.
- Use `session list --all` to include saved/ended sessions. JSON session IDs and
  history/search cursors are strings to preserve all 64 bits; pass the page's
  `next` value as `--before` to continue.

## Browser control

For a local web app already serving a login form with `#email` and a submit
button (replace the URL and selectors for your page):

```bash
tab=$(murvix browser open http://localhost:3000)
murvix browser wait "$tab" --selector "#email"
murvix browser fill "$tab" "#email" "me@example.com"
murvix browser click "$tab" "button[type=submit]"
murvix browser wait "$tab" --text "Welcome"
murvix browser read "$tab"
murvix browser screenshot "$tab" | base64 --decode > page.png  # Linux; use base64 -D on macOS
murvix browser close "$tab"
```

- Browser commands need the desktop app and don't take `--host`. `open` tries
  to start the app if it isn't running, except when the CLI itself is over SSH.
- `open` prints the new tab's ID. Pass it to the other commands.
- `snapshot` lists a page's interactive elements, each with a selector for
  `click`, `fill`, or `type`.
- `murvix browser --help` lists every command.

## Exit codes

`0` on success. On any error, `murvix` prints `murvix: <message>` to stderr and exits
with `1`, including when a program run with `murvix exec` fails.

## Profiles

`MUXY_DIR` points the CLI, and the local server it starts, at another profile
folder. Use it to run a separate, throwaway local server. With `--host`, it
selects local TUI layout storage, not the remote server's profile. See
[Server](server.md#files).
