# Dialogs

Native dialogs, a searchable picker, and modal web views. All work from pages,
scripts, and background scripts. On pages they return Promises. In scripts,
dialogs block until answered, but the picker and modal web views don't; see
their sections.

## Dialogs

```js
const choice = await murvix.dialog.confirm({
  title: "Delete branch?",
  message: "This can't be undone.",
  buttons: ["Delete", "Cancel"],
  default: "Delete",
  cancel: "Cancel",
});   // the clicked button's label, or null for the cancel button

await murvix.dialog.alert({ title: "Done", message: "All tests passed" });
const name = await murvix.dialog.prompt({ title: "Branch name", placeholder: "feature-x" });
const folder = await murvix.dialog.pickFolder({ title: "Choose a folder" });
```

- `confirm` takes up to three buttons. `prompt` and `pickFolder` resolve to
  `null` when cancelled.
- One dialog per extension at a time. Text is limited to 2,000 characters.
- Folder selection returns a path on the desktop computer, not the project's
  remote server.

## Picker

A command-palette style list to choose from.

```js
const item = await murvix.modal.open({
  placeholder: "Pick a branch",
  items: [{ id: "main", title: "main", subtitle: "default" }, { id: "dev", title: "dev" }],
});   // the chosen item, or null
```

- `items` can be an array, or a function that receives `emit` to stream items
  in batches.
- `onQuery(query, emit)` reloads items as the user types, for searches against
  a large or remote source.
- `emptyLabel` and `noMatchLabel` set the empty-state text.
- In scripts and background scripts, `modal.open` returns a request ID right
  away, not the selected item; pass `onSelect(item)` to get the choice.
- Up to 100,000 items.

## Modal web views

A centered page for a custom form or flow. Needs `panels:write`.

```js
const result = await murvix.modal.openWebview({
  entry: "dialogs/new-item.html",
  width: 480,
  height: 320,
  data: { kind: "note" },
});
```

Inside the modal page:

```js
await murvix.modal.submitWebview({ title: "My note" });   // resolves openWebview with this
await murvix.modal.closeWebview();                         // resolves it with null
```

The size defaults to 480 × 320 and is kept within 120–900 × 120–760. Results
over 256 KiB resolve as `null`. Clicking outside requests a close unless
`dismissOnOutsideClick` is `false`; an `onBeforeClose` handler can prevent that
close. `submitWebview` completes directly, without a before-close callback.
A command can open a modal directly with the `openModal` action.
