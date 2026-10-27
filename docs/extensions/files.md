# Files

Read and change files in a project. Calls run on the project's server, so they
work the same for local and remote projects. Available on pages and in scripts.

Paths are relative to the project folder. A tab works on its own project;
other page surfaces use the current project. Command scripts retain the project
selected when they started. Pass `{ project }` to select a project/worktree by
ID, case-insensitive name, or path; a top-level project selector resolves to
that project's preferred worktree.

| Call | Needs | Returns |
| --- | --- | --- |
| `files.list(path)` | `files:read` | `[{ name, path, isDirectory, isIgnored }]` |
| `files.read(path)` | `files:read` | `{ path, content, size }` |
| `files.stat(path)` | `files:read` | `{ path, name, isDirectory, size }` |
| `files.write(path, contents)` | `files:write` | `{ path }` |
| `files.mkdir(path)` | `files:write` | `{ path }` |
| `files.rename(path, newName)` | `files:write` | `{ path }` |
| `files.move(paths, into)` | `files:write` | new paths |
| `files.delete(paths)` | `files:write` | |

```js
const entries = await murvix.files.list("");
const readme = await murvix.files.read("README.md");
await murvix.files.write("notes/todo.md", readme.content + "\n- [ ] ship it");
```

- Writes ask the user first; see [Permissions](permissions.md#runtime-prompts).
- `delete` moves files to the Trash.
- Reads and writes are limited to 5 MiB, and listings to 16,384 entries.
- Paths may not escape the workspace root, including through symlinks; `read`
  returns UTF-8 text, not arbitrary binary data. Each call accepts an optional
  final `{ project }` argument (for example, `files.read(path, { project })`).
- Subscribe to `file.changed` (with `files:read`) to hear about changes; see
  [Events](events.md).
