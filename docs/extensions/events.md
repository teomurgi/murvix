# Events

Pages and background scripts can subscribe to app events. List every
event you use in `murvix.events`; some also need a permission.

```json
"events": ["tab.focused", "agent.status"],
"permissions": ["agents:read"]
```

```js
const unsubscribe = murvix.events.subscribe("agent.status", (event) => {
  console.log(event.providerID, event.status);
});
```

Every **app-event** payload value is a string; unavailable context keys are
omitted. Extension-local messages can contain arbitrary JSON.

## App events

| Event | Payload | Needs |
| --- | --- | --- |
| `tab.created`, `tab.closed`, `tab.updated` | Tab context, below | |
| `tab.focused` | `areaID`, `tabID` | |
| `pane.created`, `pane.closed` | Tab context plus `paneID` (terminal panes) | |
| `pane.focused` | `projectID`, `worktreeID`, `areaID`, `tabID` | |
| `project.switched` | `projectID` | |
| `worktree.switched` | `projectID`, `worktreeID` | |
| `projects.changed` | none | `projects:read` |
| `worktree.headChanged` | `projectID`, `worktreeID`, `branch`, `path` | `worktrees:read` |
| `agent.status` | `projectID`, `worktreeID`, `paneID`, `providerID`, `status` (`working`, `waiting`, `idle`) | `agents:read` |
| `file.changed` | `path`, `projectPath` | `files:read` |
| `notification.posted` | `paneID`, `projectID`, `worktreeID`, `worktreePath`, `tabID`, `source`, `title`, `body` | |
| `panel.opened`, `panel.closed` | `extensionID`, `panelID` | |
| `popover.opened`, `popover.closed` | `extensionID`, `popoverID` | |
| `modal.opened`, `modal.closed` | `extensionID`, `modalID` | |

**Tab context:** `tabID`, `projectID`, `worktreeID`, `areaID`, `title`,
`projectPath`, and `kind` (`terminal`, `extensionWebView`, or `browser`).
Terminal tabs add `paneID` and, when known, `cwd`. Extension tabs add `extensionID`,
`tabTypeID`, `tabInstanceID`, and non-null `data` as a JSON string. Browser tabs
add `paneID` and `url`. Pane creation/closure events are emitted for terminal
panes only, even when a tab also contains web views.

- `projectID` is the top-level project. `worktreeID` is the project or
  worktree the tab belongs to.
- `file.changed` covers the current project and projects with the extension's
  tabs open.
- `agent.status` reports the busiest agent per worktree; `providerID` is
  `claude`, `codex`, `opencode`, `cursor`, `copilot`, `droid`, `pi`, `grok`,
  `kiro`, `xal`, `antigravity`, `omp`, or `other`.
- `pane.focused` reports moving between split panes within the same tab, not
  ordinary tab/project switches; its payload does not include a pane ID.
- `worktree.headChanged` currently reports changes to a previously known branch
  name, not every new commit on the same branch.
- Terminal title changes reach `tab.updated` after settling for 500 ms.

## Command events

A command with the `event` action sends `command.<id>` to the extension with
`{ extension: "<extension name>", command: "<command id>" }`. Runtime shortcuts
send the same payload. No event declaration is needed for manifest commands or
currently registered runtime shortcuts.

## Extension messages

An extension's pages send `extension.*` messages to its background script, and
the background script broadcasts them to its pages. Pages do not send directly
to other pages, and the sender does not receive an automatic echo. No manifest
declaration is needed; messages never cross extension boundaries.

```js
// page
await murvix.events.emit("extension.refresh", { reason: "button" });

// background
murvix.events.subscribe("extension.refresh", (payload) => { /* ... */ });
```

A page's message fails if its background script is not running. Payloads are
JSON, up to 64 KiB. Names must be `extension.` followed by at least one of
`A-Z`, `a-z`, `0-9`, `.`, `_`, `:`, or `-`, at most 200 characters in total.

## Background scripts

Set `murvix.background` to a script path to run code while the extension is
enabled. It starts with the extension, restarts when the extension updates,
and stops when it is disabled or reloaded. Use it to react to events, keep
state, or update [bar items](bar-items.md). Most extensions don't need one.

```js
// background.js
// Manifest needs events: ["tab.focused"] and
// permissions: ["agents:read", "panels:write"] for this example.
murvix.events.subscribe("tab.focused", () => {
  murvix.statusbar.set({ id: "count", text: String(murvix.agents.list().length) });
});
```

Background scripts run in-process on their own thread. Errors and `console`
output go to the extension's log. Disabling an extension retires its host calls
and requests that its script loop stop; it does not forcibly interrupt
JavaScript already executing. Do not treat this runtime as a process sandbox.
