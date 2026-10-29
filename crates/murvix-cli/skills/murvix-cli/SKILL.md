---
name: murvix-cli
description: How to drive Murvix 2 from a shell — start terminals, read output, manage projects, worktrees, and workspace folders. Use when asked to list workspace folders, work with extra roots, or drive terminals, sessions, projects, or the desktop browser from a Murvix terminal.
---
# Murvix CLI

Murvix keeps terminals in a background server. The `murvix` command talks to that
server directly; the desktop app doesn't need to be open. Terminals you start
keep running after your command returns, and the user can open them in the
desktop app or the terminal UI from **Existing Terminals**.

Run `murvix --help` and `murvix <command> --help` for every option. Add `--json`
to any management command for machine-readable output.

## When to use it

- Start a long-running process (a dev server, a test watcher, a log tail) in its
  own terminal, then check on it later.
- Wait until that process prints something, or exits, before going on.
- Create a Git worktree for a task, with the user's configured location.

Don't use it for work a plain command can do in your own terminal.

## Inside a Murvix terminal

Murvix sets `MUXY_SESSION_ID` and `MUXY_SERVER_ID` in every terminal. Session
commands then default to your own terminal, and `session create` to your
project, so most commands need no IDs.

Read the live workspace with `murvix project here --json`: the project directory
first, then its added folders, as text and exact byte arrays. This works for any
agent and reflects added or removed folders even in an existing terminal.
Run this command yourself whenever asked to list workspace folders or work with
more than the current directory; do not answer from the shell's current
directory or from `MUXY_WORKSPACE_*` alone. The command identifies your terminal
through its server and session, not cwd, and rejects stale or foreign identities.

`MUXY_WORKSPACE_COUNT` and `MUXY_WORKSPACE_0`, `MUXY_WORKSPACE_1`, and so on are
older terminal-start snapshots; prefer the live command. Files, search, exec,
and Git still target the primary project folder.

To start a new workspace-aware agent, use `murvix agent omp`, `murvix agent claude`,
`murvix agent codex`, `murvix agent copilot`, `murvix agent pi`, or
`murvix agent opencode`. Native arguments follow `--`.
omp, claude, codex and copilot receive current roots through each tool's native
`--add-dir` options; pi has no multi-root flag so it starts in the project
directory; opencode receives the project directory via `run --dir`.
Extra roots reach pi and opencode through this skill: run `murvix project here`.
The launcher never bypasses native permissions. Plain tool commands and
already-running agents are unchanged; use the tool's native folder commands or
launch again after changes.

## Start a terminal and capture its ID

`session create` prints the new session ID. Everything after `--` is typed into
the new terminal, followed by Return. Capture the ID; never guess one.

```bash
WEB=$(murvix session create -- npm run dev)
TESTS=$(murvix session create MyProject --directory ./packages/api -- exec npm test)
```

Outside a Murvix terminal, name the project: an ID, a unique name, or its folder.

The words after `--` are joined with spaces, so quote the whole command to keep
its own quotes, pipes, or `;`:

```bash
murvix session create -- 'grep -rn "TODO list" src | wc -l'
```

## Wait instead of polling

```bash
murvix session wait "$WEB" --text "ready" --timeout-ms 120000   # prints the matching line
murvix session wait "$TESTS" --exit                              # prints how it ended
```

`wait` fails with a nonzero exit if the timeout passes (default 30 s) or, for
`--text`, if the terminal ends without showing the text. It looks at the
visible screen, not scrollback. Don't wait on your own terminal: it is busy
running `murvix`.

`--exit` waits for the terminal's shell to end. Use `exec` for a one-shot command,
as in the test example, so the program replaces the shell and its exit ends the
session. Without `exec`, the shell stays open after the program finishes.

## Read output

```bash
murvix session read-screen "$WEB" --lines 40     # last visible rows, not scrollback
murvix session search "$WEB" "error" --ignore-case
murvix session history "$WEB" --limit 200        # JSON, paged with --before
```

After a terminal ends, add `--saved` to read its last saved output.

## Send input deliberately

`send` types text without pressing Return; `send-keys` presses one key
(Enter, Tab, Escape, Backspace, Ctrl+C, Ctrl+D, Ctrl+Z).

```bash
murvix session send-keys "$WEB" Ctrl+C
murvix session read-screen "$WEB"
murvix session send "$WEB" "npm run dev"
murvix session send-keys "$WEB" Enter
```

You are typing into a live terminal. Read the screen first if you are unsure
what is running.

## End terminals you started

```bash
murvix session list --project MyProject     # id, project-id, status, attached, directory
murvix session end "$WEB" --yes
```

Only end terminals you created. A terminal the user is looking at closes in
their app too.

## Projects and worktrees

```bash
murvix project list
murvix project add ~/code/app --reuse                 # prints the existing project if there is one
murvix project workspace MyProject ~/code/api ~/code/web   # extra roots in scope for agents
murvix project clear-workspace MyProject
murvix worktree create MyProject fix --branch fix/x --base main --hooks
murvix worktree list MyProject
murvix worktree remove login --yes
```

`--hooks` runs the setup or teardown commands from `.murvix/worktree.json`; ask
the user before using it. `worktree remove` refuses uncommitted changes unless
`--force` is given; never pass `--force` without the user's consent.

## Browser

`murvix browser` drives tabs in the desktop app's built-in browser: open a page,
read it, click and type, take screenshots. It talks to the desktop app on this
Mac, so it needs the app (`open` starts it) and never takes `--host`.

```bash
TAB=$(murvix browser open http://localhost:3000)    # prints the new tab's ID
murvix browser wait-for-navigation "$TAB"
murvix browser snapshot "$TAB"                       # interactive elements, each with a selector
murvix browser fill "$TAB" "#email" "me@example.com"
murvix browser click "$TAB" "button[type=submit]"
murvix browser wait "$TAB" --text "Welcome" --timeout-ms 10000
murvix browser read "$TAB"                           # title, URL and page text
```

- Capture the tab ID that `open` prints, or find it with `murvix browser list`;
  never guess one.
- Wait after anything that loads a page (`open`, `navigate`, a submitting
  `click`, `back`): `wait-for-navigation`, `wait-for <selector>`, or `wait`.
- `snapshot` lists the interactive elements, each with a `selector` you can
  pass to `click`, `fill`, `type`, and the other element commands. They print
  `false` when nothing matches.
- `eval` runs JavaScript in the page and prints the result as JSON.
- Screenshots work on background tabs:
  `murvix browser screenshot "$TAB" | base64 -D > page.png`.
- Close only tabs you opened.

## Other computers

Put `--host user@computer` before a server-backed command, such as `project`,
`session`, or `worktree`, to use another computer's server over SSH. Directories
are then absolute paths on that computer.

Folder-opening commands, `browser`, and `install-skills` do not support
`--host`.
`install-skills --dir DIR` always installs into a local directory.
