# Permissions

An extension lists what it needs in `murvix.permissions`. Calls without the
matching permission fail, and unknown permission names stop the extension from
loading. Ask for the minimum.

```json
"permissions": ["tabs:read", "git:read", "notifications:write"]
```

## Permissions

| Permission | Allows |
| --- | --- |
| `tabs:read` | `tabs.list` |
| `tabs:write` | Open, switch, and create tabs; set a tab's title and icon; `openTab` commands |
| `panes:read` | `panes.list`, `panes.readScreen` |
| `panes:write` | `panes.send`, `panes.sendKeys`, `panes.close`, `panes.rename` |
| `projects:read` | List projects; the `projects.changed` event |
| `projects:write` | Add/create, switch, rename, recolor, change icons, clear logos, and reorder projects |
| `projects:delete` | `projects.delete` |
| `worktrees:read` | `worktrees.list`; the `worktree.headChanged` event |
| `worktrees:write` | `worktrees.switchTo`, `worktrees.refresh` |
| `agents:read` | `agents.list`; the `agent.status` event |
| `files:read` | `files.list`, `files.read`, `files.stat`; the `file.changed` event |
| `files:write` | `files.write`, `mkdir`, `rename`, `move`, `delete` |
| `git:read` | Status, diffs, log, branches, worktrees, pull request info |
| `git:write` | Every other `git.*` call, including worktree switching |
| `gh:read` | `gh.user` |
| `storage:read` / `storage:write` | Read / change the extension's storage |
| `notifications:write` | `notifications.notify`, `toast` |
| `panels:write` | Panels, popovers, modal web views, and setting bar items; panel, popover, and modal commands |
| `commands:run-script` | `runScript` commands |
| `commands:exec` | `exec`, `execAsync` |
| `shortcuts:register` | `shortcuts.register`, `shortcuts.unregister` |
| `browser:read` | List/read browser pages, inspect DOM, screenshots, cookies and web storage, and wait for selectors/navigation |
| `browser:write` | Open/navigate/close browser panes, DOM actions, JavaScript evaluation, function-based waits, and changing cookies/web storage |
| `remote:serve` | Parsed compatibility permission; mobile remote-method dispatch is not implemented |

`http.fetch`, dialogs, the picker, event subscription/emission, and `lifecycle`
need no permission. Some app-event subscriptions additionally require a read
permission, as listed above. Browser DOM automation, cookies, and web storage
are exposed to command scripts, not extension pages or background scripts.

## Runtime prompts

Some calls also request runtime consent. Without a matching saved rule, they
ask the user even when the manifest declares the permission:

| Asks before | Remembered per |
| --- | --- |
| Running a program (`exec`, `execAsync`) | Program, or shell command |
| Typing or pressing keys in a terminal | Extension |
| Reading terminal output | Extension |
| Opening a terminal tab that runs a command | Command |
| Opening another extension's tab | Calling extension (all foreign targets) |
| Changing the repository (`git:write` calls) | Operation |
| Changing files (`files:write` calls) | Operation |
| A web request (`http.fetch`) | Host |
| Deleting a project | Project |

The user can **Allow** once, **Allow & Remember**, **Cancel**, **Deny &
Remember**, or block that kind of action for the extension. An unanswered
prompt is denied after 60 seconds, and at most five prompts can wait at once.

Saved choices are listed under **Permission rules** on the extension's page in
**Settings → Extensions**, with **Reveal audit log** for a history of
decisions. Uninstalling clears them.

Permissions are not a process sandbox. In particular, `commands:exec` permits
programs with the server user's filesystem/network access, and browser
permissions can expose signed-in pages and profile data without the
`http.fetch` host prompt. Only enable extensions you trust.

Remembering an `exec` program grants that program regardless of its arguments,
working directory, or environment; shell commands are remembered by their full
shell string. Review these rules with that scope in mind.

An update that adds declared permissions leaves the extension disabled for
review. An enabled extension can remain enabled when the update requests only
permissions it already had; storage, settings, and remembered rules are preserved.
