use murvix_client::Client;
use murvix_protocol::{CatalogPage, OperationId, ProjectIntent, ProjectMutation, ProjectPatch};

use super::{Action, Core, lock};
use crate::state::Result;

impl Core {
    /// Workspace roots belong to the server catalog, never the TUI layout.
    /// Read again before editing so an external client's roots are retained.
    pub(super) fn workspace(
        &mut self,
        action: Action,
        client: &Client,
        catalog: &mut CatalogPage,
    ) -> Result {
        let project = match &action {
            Action::AddWorkspaceFolder { project, .. }
            | Action::RemoveWorkspaceFolder { project, .. }
            | Action::ClearWorkspaceFolders(project) => *project,
            _ => unreachable!("workspace actions are dispatched separately"),
        };
        let latest = client.catalog().map_err(|error| error.to_string())?;
        let mut roots = latest
            .projects
            .into_iter()
            .find(|candidate| candidate.id == project)
            .ok_or("Project is no longer available")?
            .extra_directories;
        match action {
            Action::AddWorkspaceFolder { directory, .. } => {
                if !roots.contains(&directory) {
                    roots.push(directory);
                }
            }
            Action::RemoveWorkspaceFolder { directory, .. } => {
                roots.retain(|root| root != &directory);
            }
            Action::ClearWorkspaceFolders(_) => roots.clear(),
            _ => unreachable!("workspace actions are dispatched separately"),
        }
        // Paths stay byte-faithful and are interpreted only on the server.
        // The server validates primary duplicates, existence, and worktrees.
        client
            .mutate_project(ProjectIntent {
                operation: OperationId::new(),
                mutation: ProjectMutation::Patch {
                    project,
                    patch: ProjectPatch::ExtraDirectories(roots),
                },
            })
            .map_err(|error| error.to_string())?;
        *catalog = client.catalog().map_err(|error| error.to_string())?;
        let selection = lock(&self.shared)
            .state
            .as_ref()
            .ok_or("TUI state is not ready")?
            .selection();
        self.store_mut()?.change(|state| {
            state.select(selection)?;
            state.reconcile(catalog)
        })?;
        lock(&self.shared).sessions_dirty = true;
        self.message("");
        Ok(())
    }
}
