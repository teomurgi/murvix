# Layout Examples

Save any of these in a project's `.murvix/layouts/` folder and apply it from the
desktop app. Commands run in that project's folder on its server: the examples
using `nvim` or `npm` require those tools and, for npm commands, a
project with the matching scripts. A named terminal with no command starts the
configured shell.

## `side-by-side.yaml`

```yaml
layout: horizontal
panes:
  - tab:
      name: editor
      command: nvim .
  - tab:
      name: shell
```

```
┌──────────────────┬──────────────────┐
│ editor           │ shell            │
│ nvim .           │                  │
└──────────────────┴──────────────────┘
```

## `stacked.yaml`

```yaml
layout: vertical
panes:
  - tab: npm run dev
  - tab:
      name: shell
```

```
┌─────────────────────────────────────┐
│ npm run dev                         │
├─────────────────────────────────────┤
│ shell                               │
└─────────────────────────────────────┘
```

## `quad.yaml`

```yaml
layout: horizontal
panes:
  - layout: vertical
    panes:
      - tab: { name: top-left }
      - tab: { name: bottom-left }
  - layout: vertical
    panes:
      - tab: { name: top-right }
      - tab: { name: bottom-right }
```

```
┌──────────────────┬──────────────────┐
│ top-left         │ top-right        │
├──────────────────┼──────────────────┤
│ bottom-left      │ bottom-right     │
└──────────────────┴──────────────────┘
```

## `dev.yaml`

```yaml
layout: horizontal
panes:
  - tab:
      name: editor
      command: nvim .
  - layout: vertical
    panes:
      - tab:
          name: server
          command:
            - npm install
            - npm run dev
      - tab:
          name: shell
```

```
┌──────────────────┬──────────────────┐
│ editor           │ server           │
│ nvim .           ├──────────────────┤
│                  │ shell            │
└──────────────────┴──────────────────┘
```
