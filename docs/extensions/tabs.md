# Tabs

A tab type is a web page that opens as a tab, next to terminal tabs. It can be
split, moved, and pinned like any tab, and is restored on launch.

```json
"tabTypes": [
  { "id": "viewer", "title": "Viewer", "entry": "viewer/index.html", "defaultData": { "mode": "list" } }
]
```

| Field | Notes |
| --- | --- |
| `id` | Unique within the extension. |
| `title` | The tab's default title. |
| `entry` | HTML page in the extension folder. |
| `defaultData` | Data used when the tab is opened without any. |

Open a tab type from a [command](commands.md) with the `openTab` action, or
from code.

## Opening tabs

`tabs.open` needs `tabs:write` and returns the tab ID (which can be an existing
tab's ID when using `singleton`).

```js
// a page from this or another extension
await murvix.tabs.open({
  kind: "extensionWebView",
  extension: { id: "hello", tabType: "viewer", data: { mode: "grid" }, singleton: true },
});

// a terminal in the current project, optionally in a subfolder or running a command
await murvix.tabs.open({ kind: "terminal", directory: "src", command: "npm test" });

// a browser tab; drive it with murvix.browser
await murvix.tabs.open({ kind: "browser", url: "localhost:3000" });
```

- `singleton: true` focuses an existing tab of that type instead of opening
  another.
- Running a command, or opening another extension's tab, asks the user first.
- A terminal's `directory` is checked on the desktop computer and must resolve
  to an existing folder inside the current project's local path. Omit it for
  remote terminals; it is not resolved against the remote server.
- Browser tabs require the built-in browser to be enabled. `murvix.browser`
  uses browser **pane** IDs; obtain them with `browser.list()` rather than
  passing the enclosing tab ID from `tabs.open`.

## Page data

Each tab has its own `data`, saved with the tab.

```js
render(murvix.data);
murvix.onDataChange(render);
murvix.tabs.setTitle("Report");
murvix.tabs.setIcon({ symbol: "doc.text" });   // or { svg: "..." }, or null
```

`setTitle` and `setIcon` work only from a tab, not panels or other page surfaces,
and require `tabs:write`. An empty title resets the title override; `null`
resets the icon. These runtime overrides are not saved across app restarts.

## Theme

Pages get the app's colors as CSS variables, updated live:

```css
body {
  background: var(--murvix-background);
  color: var(--murvix-foreground);
  border-color: var(--murvix-border);
}
```

Variables: `--murvix-background`, `--murvix-foreground`, `--murvix-foreground-muted`,
`--murvix-surface`, `--murvix-surface-solid`, `--murvix-border`, `--murvix-hover`,
`--murvix-accent`, `--murvix-accent-foreground`, `--murvix-accent-soft`,
`--murvix-diff-add`, `--murvix-diff-remove`, `--murvix-diff-hunk`, and
`--murvix-topbar-height`. The same values are in `murvix.theme`, with
`colorScheme` set to `light` or `dark`. `murvix.onThemeChange(fn)` reports
changes.

## File openers

A file opener lets an extension open files that users `Cmd`-click in a
terminal, in one of its tab types.

```json
"fileOpeners": [
  { "id": "markdown", "title": "Markdown preview", "tabType": "viewer", "patterns": ["*.md"] }
]
```

| Field | Notes |
| --- | --- |
| `tabType` | One of the extension's tab types. |
| `patterns` | `*`/`?` globs matched against the full project-relative path, case-insensitive. `*` can cross `/`; this is not a gitignore matcher. Default `["*"]`; an empty array matches everything. |
| `singleton` | Reuse one tab. Default `true`. |

Users pick it in **Settings → General → Open files with**; extensions never
open files unasked. The tab receives `data` like
`{ "filePath": "docs/README.md", "line": 12, "column": 3, "source": "terminal" }`,
with `filePath` relative to the project.

## Pages

- Bundled resources resolve inside the extension's own folder, including
  symlink checks. Navigation to other origins is blocked; blank documents
  (`about:blank`/`about:srcdoc`) are allowed. The native bridge accepts messages
  only from the registered main document.
- Pages stay alive while hidden.
- `murvix.lifecycle.onBeforeClose(fn)` can keep a tab open, for example to save
  first. See [API reference](api.md#page-only).
