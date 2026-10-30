let cachedRoot;
let resolved = false;

export async function worktree_root() {
  if (resolved) return cachedRoot;
  try {
    const worktrees = await murvix.worktrees.list();
    const active =
      worktrees.find((w) => w.isActive) ?? worktrees.find((w) => w.isPrimary) ?? worktrees[0];
    cachedRoot = active?.path;
  } catch {
    cachedRoot = undefined;
  }
  resolved = true;
  return cachedRoot;
}

function invalidate() {
  resolved = false;
  cachedRoot = undefined;
}

murvix.events.subscribe("project.switched", invalidate);
murvix.events.subscribe("worktree.switched", invalidate);
