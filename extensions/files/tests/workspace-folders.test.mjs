import assert from "node:assert/strict";
import test from "node:test";

import { workspace_folder_entries } from "../src/lib/workspace-folders.js";

test("lists only folders added to the active workspace", () => {
  const entries = workspace_folder_entries([
    { isActive: false, workspaceFolders: ["/projects/other"] },
    { isActive: true, workspaceFolders: ["/shared/design", "/shared/docs/"] },
  ]);

  assert.deepEqual(entries, [
    {
      path: "workspace:0",
      name: "design",
      isDirectory: false,
      isWorkspace: true,
      workspacePath: "/shared/design",
    },
    {
      path: "workspace:1",
      name: "docs",
      isDirectory: false,
      isWorkspace: true,
      workspacePath: "/shared/docs/",
    },
  ]);
});

test("ignores missing or malformed workspace folder lists", () => {
  assert.deepEqual(workspace_folder_entries([]), []);
  assert.deepEqual(workspace_folder_entries([{ isActive: true, workspaceFolders: ["", 42, "/shared/docs"] }]), [
    {
      path: "workspace:0",
      name: "docs",
      isDirectory: false,
      isWorkspace: true,
      workspacePath: "/shared/docs",
    },
  ]);
});
