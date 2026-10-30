# files

Murvix extension scaffolded from a starter kit. This is an npm + Vite project.

## Layout

- `package.json` — npm manifest. Identity (`name`, `version`) is at the
  top level; all Murvix fields live under the `murvix` key. A `build` script
  (Vite) is required.
- `vite.config.js` — builds to `dist/`, the directory Murvix installs.
- `panel/` + `src/` — vanilla JavaScript source for the files panel and
  editor tab.

Add a `"background"` script (e.g. `background.js`) under the `murvix` key
only if the extension needs to receive pushed workspace events or run
shell commands in the background. Murvix runs it as a long-lived process
that subscribes to events with `murvix.events.subscribe` and runs commands
with `murvix.exec`. Command, topbar, status bar, tab, and runScript
extensions need no background script.

## Building & editing

Install deps with `npm install`, then `npm run build` to produce
`dist/`. After rebuilding, click "Reload" in the Murvix Extensions modal to
pick up the changes. (`npm run dev` runs Vite's dev server for fast
iteration.)

## Skill

Coding agents in this directory should consult the `murvix-extension`
skill in `.claude/skills/` or `.agents/skills/` before generating
manifest or runtime changes.
