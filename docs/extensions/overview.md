# Extensions Overview

> **New here?** Start with [Get started](get-started.md).

An extension is a web project that adds to the desktop app: tabs, panels,
popovers, a sidebar, top bar and status bar items, palette commands, shortcuts,
background logic, and app languages. You build it with any web tooling and Murvix
loads the build output.

The loader retains several legacy manifest fields, but this is not a blanket
compatibility guarantee. Check [legacy compatibility](get-started.md#legacy-compatibility)
and the API availability table when migrating an extension.

## Where code runs

| Where | What it is | API |
| --- | --- | --- |
| **Pages** | Tabs, panels, popovers, the sidebar, and modal web views. Native WebKit views that stay alive while hidden. | `window.murvix`, async (Promises) |
| **Scripts** | A `runScript` command. Runs once in JavaScriptCore on its own thread. | `murvix`, synchronous |
| **Background** | The optional `background` script. Runs while the extension is enabled. Receives events. | `murvix`, synchronous |

Not every API exists everywhere. Pages use native WebKit on macOS and
WebKitGTK on Linux; scripts use the corresponding JavaScriptCore runtime.
Linux requires those runtime libraries. See the
[API reference](api.md#availability) and [Linux constraints](../tech/constraints.md#experimental-linux-desktop).

File, Git, and `exec` work runs on the project's server, so it works the same
for local and remote projects. Web requests run in the desktop app.

## Pages

| Page | What's in it |
| --- | --- |
| [Get started](get-started.md) | Build, load, and debug an extension |
| [Manifest](manifest.md) | `package.json` fields |
| [Permissions](permissions.md) | Permissions and runtime prompts |
| [API reference](api.md) | Every `murvix` namespace and where it is available |
| [Events](events.md) | App events and extension messages |
| [Tabs](tabs.md) | Tab types, file openers, page data, and theme |
| [Panels](panels.md) | Panels, popovers, and the sidebar |
| [Bar items](bar-items.md) | Top bar and status bar items |
| [Commands](commands.md) | Palette commands, shortcuts, and scripts |
| [Dialogs](dialogs.md) | Dialogs, pickers, and modal web views |
| [Files](files.md) | Reading and writing project files |
| [Git](git.md) | Repository, branches, pull requests, and worktrees |
| [Localizations](localizations.md) | Language packs |
| [Contributing](contributing.md) | Package sharing and current marketplace limitations |

## Where extensions live

Installed extensions live in the `extensions/<name>/` folder of Murvix's
[profile folder](../features/server.md#files). Each is the build output, with
`package.json` at its root.

Manage them in **Settings → Extensions**: **Installed** lists them with their
permissions, settings, and logs; **Browse** installs from the marketplace.
Extensions start disabled after installing or loading.

## Security

- **Declared permissions.** Most API calls need a
  [permission](permissions.md) in the manifest. The `http.fetch` bridge, dialogs,
  and event subscription need none, though some app events need read permissions.
- **Runtime prompts.** Running programs, typing into terminals, writing files or
  Git, web requests, and deleting projects ask the user first.
- **Declared events.** An extension only receives the app events it
  lists in its manifest.
- **Extension page resources.** Bundled resources resolve inside the extension's
  own folder, including symlink checks. Page navigation is restricted to its
  own extension origin and blank documents. This is not a process or network
  sandbox: `exec` runs programs with the server user's access, and browser
  automation can access signed-in browser profiles.
- **Checked installs.** Marketplace downloads must come from `https://murvix.app`
  on port 443 and match the listing's size, SHA-256, name, version, and declared
  permissions. These checks establish consistency with the listing, not a
  guarantee that extension code is safe.
