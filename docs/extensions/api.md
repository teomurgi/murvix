# API Reference

Murvix injects a `murvix` object into extension code. On pages, most calls return
a Promise. In scripts and background scripts, most calls are synchronous.
Exceptions are noted where they apply, such as `execAsync`, which returns a
handle whose `result` you await.

```js
const tabs = await murvix.tabs.list();   // page
const tabs = murvix.tabs.list();         // script or background
```

Most calls need a [permission](permissions.md). Errors throw (or reject) with
a message.

## Availability

| Namespace | Pages | Scripts | Background |
| --- | :-: | :-: | :-: |
| `notifications`, `dialog`, `storage`, `shortcuts`, `topbar`, `statusbar`, `agents`, `gh`, `git`, `exec`, `modal` | ✓ | ✓ | ✓ |
| `tabs.open` | ✓ | ✓ | ✓ |
| other `tabs`, `panes`, `projects`, `worktrees`, `files`, `toast`, `execAsync`, basic `browser` calls | ✓ | ✓ | |
| `events` | ✓ | | ✓ |
| `tabs.setTitle`, `tabs.setIcon`, `panels`, `popover`, `http`, `lifecycle`, `data`, `theme` | ✓ | | |
| `browser` DOM automation, screenshots, cookies, and web storage | | ✓ | |

Basic browser calls are `open`, `navigate`, `list`, `read`, and `close`.
The API is implemented on macOS and Linux; Linux needs the corresponding
WebKitGTK/JavaScriptCore runtime libraries. Missing runtime support is reported
by the app, rather than making every extension contribution available.

Scripts and background scripts also have `setTimeout`, `setInterval`, and
their `clear` functions.

## Projects, tabs, and panes

| Call | Returns |
| --- | --- |
| `projects.list()` | `[{ id, name, path, isActive, sortOrder, iconColor, icon, logo, worktreesEnabled }]` |
| `projects.switchTo(identifier)` | |
| `projects.add(path)`, `create(path, { name, createIfMissing })` | Project ID |
| `projects.rename(id, name)`, `setColor(id, color)`, `setIcon(id, icon)`, `setLogo(id, null)`, `reorder(ids)` | `setLogo` currently only clears an existing logo |
| `projects.delete(id)` | |
| `worktrees.list(project?)` | `[{ id, name, path, branch, isActive }]` |
| `worktrees.switchTo(id, project?)`, `refresh(project?)` | |
| `tabs.list()` | `[{ index, id, kind, title, isActive }]` for the current project; indices are 0-based |
| `tabs.switchTo(indexOrIdOrTitle)`, `next()`, `previous()` | |
| `tabs.new()` | New tab ID, or `null` |
| `tabs.open(request)` | The new tab's ID. See [Tabs](tabs.md#opening-tabs) |
| `panes.list()` | `[{ id, title, workingDirectory, isFocused }]` across projects |
| `panes.send(id, text)`, `sendKeys(id, key)` | |
| `panes.readScreen(id, lines = 50)` | The last rows of the screen as text, up to 500 |
| `panes.close(id)`, `rename(id, title)` | |
| `agents.list()` | Agent status per worktree, like the `agent.status` event |

`send` and `sendKeys` only reach panes that are on screen. For a hidden pane,
`readScreen` returns the text it last showed.

Project selectors accept an ID, case-insensitive name, or path.
`projects.add` and `create` operate on folders on the desktop computer, not a
remote server. `reorder` requires every non-Home top-level project exactly once.
`panes.sendKeys` supports `escape`/`esc`, `enter`/`return`, `tab`, `ctrl+c`,
`ctrl+d`, `ctrl+z` (also `ctrl-c`, `ctrl-d`, `ctrl-z`), and `backspace`.

## Running programs

```js
const status = await murvix.exec(["git", "status", "--short"], { cwd: "src" });
const count = await murvix.exec({ shell: "ls | wc -l" });
// { stdout, stderr, exitCode, timedOut, truncated }
```

- Runs on the project's server, in the project folder, with your login `PATH`
  and `MUXY_EXTENSION_ID` set.
- Options: `cwd`, `env`, `stdin`, `timeoutMs` (default 30 s, up to 300 s).
- Output is capped at 4 MiB per stream.
- `execAsync(...)` returns `{ id, result, cancel }` right away, so a page or
  script can cancel a long run.
- `cwd` may be relative to the project or absolute. Executed programs are not
  confined to the project or to the `files`/`http` APIs' restrictions; grant
  execution only to trusted extensions.
- Cancellation rejects `result` with `error.cancelled === true` and
  `error.code === "cancelled"`.

## Storage

A small key-value store per extension, kept after uninstall.

```js
await murvix.storage.set("count", 3);
await murvix.storage.get("count");   // 3, or null
await murvix.storage.delete("count");
await murvix.storage.keys();         // sorted keys
```

Values are JSON, up to 1 MiB each and 5 MiB in total.

## Web requests

`http.fetch(url, { method, headers, body, timeoutMs })` resolves to
`{ status, headers, body, truncated }`. Pages only. It runs in the desktop app
and requests consent for the initial URL's host. The supported methods are
`GET` (default), `POST`, `PUT`, `PATCH`, `DELETE`, and `HEAD`; the body is text.
Loopback, link-local, private addresses, `.local`/`.localhost` names, and
unresolvable hosts are refused. DNS resolution and redirects are checked too.
Public redirects can go to a different host without another consent prompt.
Response bodies are capped at 10 MiB (invalid UTF-8 becomes an empty string);
timeouts default to 30 s and are clamped to 1 ms–120 s. `Host`,
`Content-Length`, and `Connection` request headers are ignored.

## Notifications

- `notifications.notify({ title, body })` posts a notification and the
  `notification.posted` event.
- `toast({ title, body })` does the same from pages and scripts.

## Shortcuts

`shortcuts.register({ id, combo })` binds a key while the extension runs.
Pressing it sends the `command.<id>` event to the extension. Commands in the
manifest use `defaultShortcut` instead. `combo` looks like `cmd+shift+e` or
`opt+left` and needs `cmd`, `ctrl`, or `opt`. `unregister(id)` and `list()`
manage them.
`register` returns `{ ok: true }` or `{ ok: false, conflict }`; it does not
override a conflicting shortcut. `list()` returns `[{ id, combo, source }]`,
where `source` is `manifest` or `runtime` and an unassigned combo is `""`.

## Browser tabs

Pages and command scripts can use `murvix.browser`:

| Call | Returns / behavior |
| --- | --- |
| `open(url?, { split }?)` | Browser pane ID |
| `navigate(tabId, url)` | Starts navigation |
| `list()` | `[{ tabId, title, url, profile, active }]` across projects |
| `read(tabId)` | `{ title, url, text }`, with text capped at 1,000,000 characters |
| `close(tabId)` | Closes the browser pane |

These `tabId` values identify browser **panes**, not the IDs returned by
`tabs.open`. Use `browser.list()` to obtain them. Browser reads need
`browser:read`; opening, navigation, and closing need `browser:write`.
When the built-in browser is disabled, `list()` returns `[]` and other browser
calls fail. Browser calls operate on the desktop browser, not a remote server.

Command scripts additionally expose `eval`, `click`, `type`, `fill`, `press`,
`select`, `hover`, `scrollIntoView`, `setChecked`, `is`, `getValue`,
`getCount`, `find`, `snapshot`, `getText`, `getHTML`, `getAttribute`,
`waitFor`, `wait`, `reload`, `back`, `forward`, `waitForNavigation`,
`screenshot`, `storage.{get,set,clear}`, and `cookies.{get,set,delete,clear}`.
The first argument is always the browser pane ID. For example:

```js
// command script, not an extension page; needs browser:read and browser:write
const tab = murvix.browser.open("https://example.com");
murvix.browser.waitFor(tab, "h1", { timeoutMs: 5000 });
const heading = murvix.browser.getText(tab, "h1");
```

These APIs can read signed-in pages, cookies, and web storage or run JavaScript
in a browser page; the `http.fetch` private-network guard and host prompts do
not apply to browser automation.

## GitHub

`gh.user()` returns the signed-in GitHub user from the `gh` CLI on the desktop
computer, even for remote projects. It is cached for five minutes.

## Page only

| Member | What it is |
| --- | --- |
| `extensionID` | The extension's name (everywhere) |
| `tabInstanceID`, `panelID` | This view's ID |
| `data`, `onDataChange(fn)` | The view's data. See [Tabs](tabs.md#page-data) |
| `theme`, `onThemeChange(fn)` | Theme colors. See [Tabs](tabs.md#theme) |
| `focused`, `onFocus(fn)` | Whether the view has focus |
| `lifecycle.onBeforeClose(fn)` | Receives `{ surface, instanceID }`. Return `true` or `{ prevent: true }`, synchronously or through a Promise, to keep the view open. Only failure to acknowledge the callback times out after 5 s; once acknowledged, an async callback has no completion timeout |
| `lifecycle.close()` | Closes this view directly, without invoking its before-close callback; no effect for the sidebar |

More: [Events](events.md), [Tabs](tabs.md), [Panels](panels.md),
[Bar items](bar-items.md), [Dialogs](dialogs.md), [Files](files.md),
[Git](git.md).
