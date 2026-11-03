# AI Agents

Murvix recognizes AI coding agents running in its terminals and shows whether
each one is working, waiting for you, or done. It needs no setup, hooks, or
plugins, and works while no app is open.

## Supported agents

Antigravity, Claude Code, Codex, Copilot, Cursor, Droid, Grok, Kiro, Oh My Pi,
OpenCode, Pi, and Xal.

The server spots them from the running program and what's on screen.
Recognition is best effort.

Oh My Pi detection reads its OSC terminal-title run state: a spinner while
working, `!` when waiting, and `>` when idle. It also recognizes optional OSC
progress reports, `⎋` working messages, and the older `Working… [esc]` spinner.
In `omp --print` sessions, a final non-empty
`Working...` line signals work; once output follows it, that progress line
no longer keeps the session busy. Detection uses terminal escape sequences
and screen text, so disabling or customizing these signals can affect recognition.

For foreground `omp` and `oh-my-pi` processes, automatic terminal titles omit
the leading `π ` prefix. The provider icon remains visible, and custom tab
titles are unchanged.

## States

| State | Meaning |
| --- | --- |
| Working | The agent is busy. |
| Waiting | The agent needs your input. |
| Idle | The agent is done or not doing anything. |

Tabs, worktrees, and projects show the agent's icon and state. When several
agents run, "waiting" wins.

## Workspace

A project is a workspace: its directory plus the extra roots set with
`murvix project workspace`, or right-click → Workspace → Add Folder… in the
desktop app. Adding a folder does not make it a physical child of the project.
Files, search, exec, and Git still target the project folder.

### Start an agent in the live workspace filesystem

Right-click an available local project or worktree → **Start Agent** → choose
the installed tool. Murvix opens a new terminal in that project inside the
live workspace filesystem. Inside an existing Murvix terminal, use the shared
launcher:

| Agent | Command |
| --- | --- |
| Claude Code | `murvix agent claude` |
| Codex | `murvix agent codex` |
| Copilot | `murvix agent copilot` |
| Hermes | `murvix agent hermes` |
| Oh My Pi | `murvix agent omp` |
| Pi | `murvix agent pi` |
| OpenCode | `murvix agent opencode` |

Pass native arguments after `--`, for example
`murvix agent codex -- resume SESSION_ID`. The launcher reads the live project
from the calling terminal's server and session, not its current directory or
old environment variables.
Any other program can use the same boundary explicitly:
`murvix workspace run -- PROGRAM [ARGS...]`. Bare `command <tool>` and
`MUXY_WORKSPACE_WRAPPERS=0` bypass confinement; that bypass is explicit.
Install and authenticate each tool separately; the isolated agent home is
persistent per project, so authenticate there once.

The workspace directory contains `primary` (writable), one keyed directory per
added root (read-only), a read-only `workspace.json` membership manifest, and
a private `.state` directory backing the isolated agent home and temporary
files. Adding a folder changes the next directory listing without restarting
the agent; removing one makes its paths fail subsequent operations (an
already-started operation may finish). Original host paths are not writable
inside the managed process: added roots are read-only both through the mount
and through their original paths. Filesystem visibility does not replace
native approval: tools keep their own permission prompts.

Linux requires FUSE and supported kernel confinement (unprivileged user and
mount namespaces plus Landlock ABI 9 or later; no fallback is attempted).
macOS requires version 26 or later and the native filesystem route with
administrative mount approval. Plain external processes are unchanged; launch
again through `murvix agent` to enter the managed filesystem.

For membership details without entering the filesystem, `murvix project here`
and `murvix workspace snapshot --json` report the calling terminal's live
roots from its server and session identity. A terminal from another server or
an ended session is rejected without falling back to stale roots. On another
computer, run the launcher inside that computer's Murvix terminal;
`murvix --host … agent …` and `murvix --host … workspace …` are not supported.

## Notifications

The desktop app sends a native notification when an agent needs your attention
or finishes working, when the operating system's notification service allows it.

- No notification is shown for the pane you are looking at, or for detached
  terminals.
- Looking at the terminal clears the alert in every app.
- Clicking a notification jumps to its pane.
- The server coordinates notification delivery between apps to avoid duplicates.
- On macOS, turn notifications off in **System Settings → Notifications →
  Murvix**. On Linux, delivery needs the session D-Bus and a service implementing
  `org.freedesktop.Notifications`; manage permissions in your desktop's settings.

Alerts live in memory on the server, at most one per terminal. Restarting the
server clears them.

## Agents panel

`Cmd+Opt+A` on macOS or `Ctrl+Shift+Alt+A` on Linux shows the agents panel,
focuses it, and hides it once it has focus. Use the arrow keys and Return to
open a row.

- It sits at the bottom of the expanded sidebar; drag its top edge to make it
  taller or shorter. With the sidebar collapsed it
  opens as a popup that closes when you click elsewhere. Once you have shown
  the panel, the collapsed sidebar keeps a button for it until you hide it.
- **Move to Right** docks it in the right panel, even when the left sidebar is
  collapsed. Drag its left edge to resize it; Murvix remembers the side and width.
  It shares this area with extension panels and the panel-mode composer: opening
  one replaces the other. Use `Cmd+Opt+A` to bring Agents back. **Move to Left
  Sidebar** returns it to the sidebar.
- **Detach** floats it over the window. Drag its header to move it and its
  edges or corner to resize it; Murvix remembers where. **Reattach to Sidebar** or
  **Reattach to Right Panel** docks it on its last side. The other side's move
  button is also available while floating.
- **Settings → Appearance → Agents panel rows** lists each agent, grouped as
  needs input, working, completed, and idle, where Return opens its pane; or
  each project and worktree running agents, with counts, where Return opens
  the worktree.

## Saved agent sessions

Click the **Agent Sessions** history icon on the right of the titlebar, or use
**Toggle Agent Sessions** in the command palette or View menu, to show or hide
the right panel. It has no default shortcut; assign one in Settings → Keyboard.
This is separate from the running-agents panel.

- Sessions belong to the selected local project directory. Worktrees have their
  own history; remote projects never display this computer's sessions.
- Search matches session titles, IDs, and provider names. The filter button on
  the right of search opens a multi-select menu with matching counts,
  **Select all**, and **Clear**. Provider choices, panel visibility, and width
  are remembered.
- Sessions are grouped by recency, with the provider and age under each title.
  Use Up/Down while searching to select a session, then Return to resume it.
  Tab opens provider filtering. Escape clears the search; Escape again returns
  focus to the terminal.
- Click a session to resume it in a new project terminal inside the live
  workspace filesystem, using the harness's native resume command. All
  workspace-capable providers resume through the same launcher; other
  supported providers keep ordinary resume. The harness must be installed and
  available on the terminal's PATH. Murvix does not install or authenticate
  it; the isolated agent home requires one authentication per project.
- Drag the left edge to resize. The panel shares the right-panel area with
  Agents, extension panels, and the panel-mode composer; opening one replaces
  the other.
- History refreshes on opening the panel or switching projects. Large histories
  load automatically in bounded batches across providers; keep browsing while
  loading completes. Use **Refresh sessions** to pick up later changes.

Discovery only reads local JSONL files; it runs in the background, caches
metadata rather than conversations, and never installs hooks or launches a
harness. Files are not modified by discovery. Resuming hands control to the
harness, which may update its session normally.

| Provider | Session files | Resume command |
| --- | --- | --- |
| Claude Code | `~/.claude/projects/*/*.jsonl` | `claude --resume ID` |
| Codex | `~/.codex/sessions/YYYY/MM/DD/*.jsonl` | `codex resume ID` |
| Copilot | `~/.copilot/session-state/*/events.jsonl` | `copilot --resume ID` |
| Droid | `~/.factory/sessions/*/*.jsonl` | `droid --resume ID` |
| Pi | `~/.pi/agent/sessions/*/*.jsonl` | `pi --session ID` |
| Oh My Pi | `~/.omp/agent/sessions/*/*.jsonl` | `omp --resume ID` |
| Xal | `~/.xal/sessions/*/*.jsonl` | `xal resume ID` |

`CLAUDE_CONFIG_DIR` and `CODEX_HOME` are respected when set to absolute paths.
OpenCode, Cursor, Grok, Kiro, and Antigravity appear as **Not supported** in the
filter menu: their database-only or unverified storage formats are not read.
Running-agent detection for these providers is unaffected.

Session discovery is best effort: malformed or unfinished records, records
without a project directory, and nested subagent artifacts are skipped. Reads
are bounded to the start and end of transcripts; unusually large metadata,
older formats, custom storage locations, and titles in separate databases may
not be found. Sessions are sorted by transcript modification time.

## Agents Focused sidebar

Choose **Agents Focused** from the sidebar layout menu to list only the tabs
that run an agent.

## From a shell and extensions

- `murvix activity list --json` prints agent status and unread alerts as JSON.
  `murvix activity ack <event-id>` clears alerts.
- Extensions get the `agent.status` event and `murvix.agents.list()`. See
  [Events](../extensions/events.md).
