# Layout Schema

A layout is a tree. Each leaf is one terminal (`tab`). Each branch splits its
`panes` side by side or stacked. The whole tree opens as one tab, titled after
its first terminal. Legacy `tabs` lists can add extra top-level tabs.

## A single terminal

```yaml
tab:
  name: editor
  command: nvim .
```

A bare string is a command:

```yaml
tab: htop
```

## Splits

```yaml
layout: horizontal
panes:
  - tab:
      name: editor
      command: nvim .
  - layout: vertical
    panes:
      - tab: npm run dev
      - tab:
          name: shell
```

Splits can nest up to 32 levels. Panes in a split share its space evenly.

## Fields

| Field | Description |
| --- | --- |
| `layout` | `horizontal` (side by side) or `vertical` (stacked). Defaults to `horizontal`. |
| `panes` | A non-empty list of child nodes. Makes the node a split. |
| `tab` | The terminal of a leaf: a non-empty command string, or an object with optional `name` and `command`. Use `tab: {}` for a plain shell. |
| `tab.name` | Optional title. Defaults to the command's first word, or `Terminal`. |
| `tab.command` | Optional command, or a list of commands joined with `&&`. Runs in the project folder. |

```yaml
tab:
  name: setup
  command:
    - npm install
    - npm run dev
```

Names and commands are trimmed. An empty `tab.command` string, a command set to
`null`, an empty command list, and lists containing only blank strings start a
plain shell. A bare `tab: ""` or `tab: null` is invalid. Every command-list entry
must be a string; blank entries are skipped before joining the rest with ` && `.
The resulting text is sent to the new terminal's shell, so shell syntax and
installed tools are those of the project's server. Titles are sanitized for
display without changing the command text.

Use `horizontal` and `vertical` for portability. The current parser recognizes
`vertical` without regard to case and treats other or omitted `layout` values
as horizontal. It ignores unknown fields. When fields are mixed, `panes` takes
precedence over `tab`, and `tab` over legacy `tabs`; use only one form per node
to avoid silently ignored settings.

## Legacy `tabs`

Older layouts with a non-empty `tabs` list still load. The first entry fills
that leaf; the others open as separate top-level tabs, even when the leaf is
nested in a split. Those extra tabs are appended in tree traversal order after
the main split tab. Use `tab` in new layouts.

```yaml
tabs:
  - name: editor
    command: nvim .
  - name: shell
```

## JSON

The same schema works in `.json` files:

```json
{
  "layout": "horizontal",
  "panes": [
    { "tab": { "name": "editor", "command": "nvim ." } },
    { "tab": "npm run dev" }
  ]
}
```

## Limits

A layout file can be up to 256 KiB, nest up to 32 levels, and hold up to 64
terminals, including extra terminals from legacy `tabs` lists. YAML parsing also
has event, scalar-size, and alias-expansion budgets, so a pathological file can
be rejected before those layout limits.
