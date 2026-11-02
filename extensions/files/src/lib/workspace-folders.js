function basename(path) {
  const clean = path.replace(/\/+$/, "");
  return clean.slice(clean.lastIndexOf("/") + 1);
}

export function workspace_folder_entries(projects) {
  const active = Array.isArray(projects) ? projects.find((project) => project?.isActive) : null;
  if (!Array.isArray(active?.workspaceFolders)) return [];
  return active.workspaceFolders
    .filter((path) => typeof path === "string" && path)
    .map((path, index) => ({
      path: `workspace:${index}`,
      name: basename(path) || path,
      isDirectory: false,
      isWorkspace: true,
      workspacePath: path,
    }));
}
