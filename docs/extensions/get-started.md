# Get started

This page builds a working extension in a few minutes: a panel you toggle from
the top bar or a shortcut.

## 1. Create the files

An extension is a folder with a `package.json`. Murvix loads the folder's `dist/`
build output when it has one, otherwise the folder itself, so a plain HTML page
needs no build step.

```
hello/
  package.json
  panel.html
```

`package.json`:

```json
{
  "name": "hello",
  "version": "0.1.0",
  "murvix": {
    "description": "Says hello",
    "permissions": ["panels:write", "notifications:write"],
    "panels": [{ "id": "hello", "title": "Hello", "entry": "panel.html" }],
    "commands": [
      {
        "id": "toggle",
        "title": "Hello: Toggle Panel",
        "action": { "kind": "togglePanel", "panel": "hello" },
        "defaultShortcut": "ctrl+opt+h"
      }
    ],
    "topbarItems": [
      { "id": "hello", "icon": "sparkles", "command": "toggle", "tooltip": "Hello" }
    ]
  }
}
```

`panel.html`:

```html
<!doctype html>
<body style="background: var(--murvix-background); color: var(--murvix-foreground)">
  <button id="hi">Say hello</button>
  <script>
    document.getElementById("hi").onclick = () =>
      murvix.notifications.notify({ title: "Hello", body: "From my extension" });
  </script>
</body>
```

The extension `name` should use lowercase letters, digits, `.`, and `-`. Its
pages don't load otherwise.

## 2. Load it

1. Open **Settings → Extensions** and choose **Load Unpacked…**.
2. Pick the `hello` folder.
3. Open its entry and turn it on. Extensions start disabled.
4. Click the sparkles icon in the top bar, or press `Ctrl+Opt+H`.

The extension shows an **Unpacked** badge. After editing, choose **Reload**.
**Unload folder** removes it from Murvix and keeps your files.

## 3. Use a build tool

For anything bigger, use any web tooling, such as Vite. Point every `entry` at
paths inside your build output, and copy `package.json` into `dist/` as part of
the build. The app treats `dist/` as the resource root whenever it exists.

```json
"scripts": {
  "build": "vite build && cp package.json dist/"
}
```

## Debugging

- `console.log`, `console.warn`, and `console.error` from pages and scripts go
  to the extension's log, along with uncaught errors.
- The extension's page in **Settings → Extensions** shows recent log lines,
  load errors, and **Reveal log**.
- The log file is `logs/output.log` inside the resource root (`dist/logs/output.log`
  for an extension loaded from build output).

## Next

- Declare what it does in the [manifest](manifest.md).
- Ask for the minimum [permissions](permissions.md).
- Add [tabs](tabs.md), [panels](panels.md), [bar items](bar-items.md), and
  [commands](commands.md).
- Work with [files](files.md), [Git](git.md), and [events](events.md).
- [Prepare a shareable package](contributing.md).

## Legacy compatibility

Check the current [manifest](manifest.md) and [API availability](api.md#availability)
rather than assuming an older extension works unchanged.

- **Install folder.** Extensions live in the current profile folder. The
  legacy `~/.config/murvix/extensions` location is not the active install root;
  importing legacy app data can copy those extensions.
- **No scaffold.** There is no **Create** button or starter kit. Use
  **Load Unpacked…**.
- **Page names.** Pages only load for extension names made of `a-z`, `0-9`,
  `.`, and `-`, although the manifest loader accepts a wider identifier set.
- **Background scripts run in-process,** in JavaScriptCore on their own thread.
- **Not implemented:** mobile remote-method dispatch, home views, reading
  extension settings at runtime, and the `worktree.offline` event.
- **Bar items** appear in the top bar and status bar. There is no icon rail.
- The API supports `openPanel`, `visible` on panel header buttons,
  `execAsync` on pages and command scripts, and `modal.opened`/`modal.closed`.
