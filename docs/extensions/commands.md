# Commands

Commands appear in the command palette (`Cmd+Shift+P` on macOS,
`Ctrl+Shift+P` on Linux), can have a shortcut, and run when a
[bar item](bar-items.md) or panel header button is clicked.

```json
"commands": [
  { "id": "ping", "title": "Hello: Ping" },
  {
    "id": "toggle",
    "title": "Hello: Toggle Notes",
    "action": { "kind": "togglePanel", "panel": "notes" },
    "defaultShortcut": "cmd+shift+y"
  }
]
```

| Field | Notes |
| --- | --- |
| `id`, `title` | Required. Palette entries are searchable by title and extension name. |
| `subtitle` | Extra search keywords. |
| `action` | What it does. Defaults to `event`. |
| `defaultShortcut` | Like `cmd+shift+y`. Needs `cmd`, `ctrl`, or `opt`. |

## Actions

| `kind` | Fields | Does | Needs |
| --- | --- | --- | --- |
| `event` | | Sends `command.<id>` to the extension's pages and background script | |
| `openTab` | `tabType` (required), `data` (optional) | Opens one of its [tab types](tabs.md) | `tabs:write` |
| `togglePanel` | `panel` | Shows or hides a [panel](panels.md) | `panels:write` |
| `openPanel` | `panel` | Shows a panel | `panels:write` |
| `openPopover` | `popover` | Opens a [popover](panels.md#popovers) from a bar item. Not listed in the palette | `panels:write` |
| `openModal` | `entry` (required); `width`, `height`, `dismissOnOutsideClick`, `data` (optional) | Opens a [modal web view](dialogs.md#modal-web-views) | `panels:write` |
| `runScript` | `script` | Runs a [script](#scripts) | `commands:run-script` |

## Shortcuts

- A `defaultShortcut` is used unless a built-in shortcut or another extension
  already has those keys.
- Extension shortcuts are listed in **Settings → Keyboard**, where users can
  change or unassign them.
- Add shortcuts at runtime with
  [`murvix.shortcuts.register`](api.md#shortcuts).

## Scripts

A `runScript` command runs a JavaScript file once, without a page. Use it for
quick automation.

```json
{ "id": "count", "title": "Hello: Count Tabs", "action": { "kind": "runScript", "script": "scripts/count.js" } }
```

```js
// scripts/count.js (needs tabs:read and notifications:write)
const tabs = murvix.tabs.list();
murvix.notifications.notify({ title: "Tabs", body: `${tabs.length} open` });
```

- Most calls are synchronous; `execAsync` returns a job handle and
  `modal.openWebview` returns a Promise. Timers and pending host callbacks keep
  the script alive until they finish.
- Scripts can use tabs and panes, files, Git, `exec`, dialogs, and the picker.
  See [availability](api.md#availability).
- The file is read fresh on every run, so edits apply without reloading.
- Errors go to the extension's log.
- Scripts are plain JavaScript evaluated in JavaScriptCore, not Node.js:
  there is no `require`, Node filesystem/process API, or module loader. Bundle
  dependencies into the script; use `murvix` for host access. A script is limited
  to 8 MiB and timers to 256 per context.
