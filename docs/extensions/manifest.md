# Manifest

The manifest is the `package.json` at the root of the extension. `name` and
`version` sit at the top level; everything Murvix-specific sits under `murvix`.

```json
{
  "$schema": "./manifest.schema.json",
  "name": "hello",
  "version": "0.1.0",
  "scripts": { "build": "vite build && cp package.json dist/" },
  "murvix": {
    "description": "Says hello",
    "permissions": ["notifications:write"],
    "commands": [{ "id": "ping", "title": "Hello: Ping" }]
  }
}
```

The [JSON schema](schema/manifest.schema.json) gives editor completion when
`$schema` sits at the top level. Download it alongside `package.json` for the
relative reference above, or point to the schema in your checkout. In VS Code
it replaces the built-in `package.json` schema for that file. The schema is an
authoring aid, not the loader: it rejects unknown Murvix keys and recommends
nonempty IDs, while the loader ignores unknown keys and treats optional `null`
fields as absent. Resource existence, reference targets, and ID uniqueness
are checked by the loader, not fully expressible in the schema.

## Loading

- Murvix reads `dist/package.json` when it exists, otherwise `package.json`.
- Every path in the manifest (`entry`, `background`, `script`, icons) is
  relative to `dist/` when the folder has one, otherwise to the folder itself.
  Paths can't leave that folder.
- An invalid manifest shows as a load error in **Settings → Extensions**.

## Top-level fields

| Field | Required | Notes |
| --- | --- | --- |
| `name` | yes | The extension ID: 1–128 ASCII letters, digits, `_`, `.`, or `-`, with no leading `.`; `foundation` is reserved. For pages, use only lowercase letters, digits, `.`, and `-`. Installed packages must match their folder name; unpacked folders need not. |
| `version` | yes | Nonempty string for loading. Use semver for marketplace updates. |
| `scripts.build` | no | A build convention, not a runtime requirement. If using `dist/`, copy the manifest and assets there. |

## `murvix` fields

The `murvix` object is required; its fields are optional.

| Field | Type | See |
| --- | --- | --- |
| `description` | string | One line, shown in Settings. |
| `permissions` | string[] | [Permissions](permissions.md) |
| `events` | string[] | App events it may receive. [Events](events.md) |
| `background` | string | Path to a script that runs while the extension is enabled. [Events](events.md#background-scripts) |
| `commands` | object[] | [Commands](commands.md) |
| `tabTypes` | object[] | [Tabs](tabs.md) |
| `fileOpeners` | object[] | [Tabs](tabs.md#file-openers) |
| `panels` | object[] | [Panels](panels.md) |
| `popovers` | object[] | [Panels](panels.md#popovers) |
| `sidebar` | object | [Panels](panels.md#sidebar) |
| `topbarItems` | object[] | [Bar items](bar-items.md) |
| `statusBarItems` | object[] | [Bar items](bar-items.md) |
| `localizations` | object[] | [Localizations](localizations.md) |
| `settings` | object[] | [Settings](#settings) below |
| `marketplace` | object | Listing metadata; ignored by the app loader. [Contributing](contributing.md) |

`homeViews` and `remoteMethods` are parsed compatibility fields, but no home-view
UI or mobile remote-method dispatch is implemented. Their entries still undergo
manifest validation. Declaring them does not make those features available.

Use unique, nonempty IDs within each list. The loader checks uniqueness for
surface/item/settings/localization/remote-method lists, but not command IDs.
Every reference (a command's panel, an item's command) must exist.

## Icons

Panels, the sidebar, and bar items take an `icon`:

```json
{ "icon": "sparkles" }
{ "icon": { "symbol": "sparkles" } }
{ "icon": { "svg": "assets/icon.svg" } }
```

- `symbol` is an [SF Symbol](https://developer.apple.com/sf-symbols/) name.
  Linux has no SF Symbols: it draws the common ones from its own icons and
  the extension icon in place of the rest.
- `svg` is a file of up to 256 KiB. It is tinted like the app's own icons, so
  use `currentColor` or one solid color. It looks the same on macOS and Linux.

## Settings

Declared settings appear on the extension's page in **Settings → Extensions**.

```json
"settings": [
  { "key": "greeting", "title": "Greeting", "type": "string", "defaultValue": "Hello" },
  { "key": "loud", "title": "Shout", "type": "bool" }
]
```

`type` is `string`, `bool`, or `number`. Extensions can't read these values at
runtime yet; use [storage](api.md#storage) for values your code needs.
