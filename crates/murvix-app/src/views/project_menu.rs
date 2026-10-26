use gpui::{AnyWindowHandle, AsyncApp, Context};
use murvix_app_core::{Project, ProjectId, ProjectStatus};
use murvix_ui::dialog::ConfirmationResponse;
use murvix_ui::l10n::tr_key;
use murvix_ui::tr;

use super::menu::{Command, Item, color_items};
use super::project_editor::Field;
use crate::model::{AppModel, git::Repository};

/// The menu of a top-level project. `worktrees` is `None` when its folder is not
/// a Git repository, and otherwise whether its worktrees are shown.
pub(crate) fn items(project: &Project, worktrees: Option<bool>) -> Vec<Item> {
    let id = project.id;
    if project.status() != ProjectStatus::Available {
        return removal(project);
    }
    let mut items = vec![
        Item::action(tr!("New Terminal Tab"), Command::NewProjectTab(id)),
        Item::action(tr!("Existing Terminals…"), Command::ExistingSessions(id)),
        Item::action(tr!("Apply Layout…"), Command::ProjectLayouts(id)),
    ];
    let icon = Item::submenu(tr!("Icon"), icon_items(project));
    // Another computer's Home goes by its server's name.
    if project.is_remote_home() {
        items.push(icon.separated());
    } else {
        items.extend([
            Item::action(tr!("Rename…"), Command::EditProject(id, Field::Name)).separated(),
            icon,
        ]);
    }
    items.push(Item::submenu(
        tr!("Color"),
        color_items(Some(project.color.as_str()), |index| {
            Command::ProjectColor(id, index)
        }),
    ));
    if !project.home
        && let Some(visible) = worktrees
    {
        items.push(Item::submenu(tr!("Worktrees"), worktree_toggle_items(id, visible)).separated());
    }
    if !project.home && project.parent_id.is_none() && project.server_id.is_local() {
        items.push(Item::submenu(tr!("Workspace"), workspace_items(project)).separated());
    }
    items.extend(path_items(project));
    items.extend(removal(project).into_iter().map(Item::separated));
    items
}

/// A folder on another computer can't be revealed here, but its path can be copied.
fn path_items(project: &Project) -> Vec<Item> {
    let copy = Item::action(tr!("Copy Path"), Command::CopyPath(project.id));
    if project.server_id.is_local() {
        vec![
            Item::action(
                crate::opener::reveal_label(),
                Command::RevealPath(project.id),
            )
            .separated(),
            copy,
        ]
    } else {
        vec![copy.separated()]
    }
}

fn removal(project: &Project) -> Vec<Item> {
    if project.home {
        Vec::new()
    } else {
        vec![Item::action(
            tr!("Remove Project…"),
            Command::RemoveProject(project.id),
        )]
    }
}

fn icon_items(project: &Project) -> Vec<Item> {
    let id = project.id;
    let mut items = Vec::new();
    if murvix_ui::symbols::PROJECT_ICONS {
        items.push(Item::action(
            tr!("Choose Icon…"),
            Command::EditProject(id, Field::Icon),
        ));
    }
    items.push(Item::action(tr!("Set Logo…"), Command::ProjectLogo(id)));
    if project.logo.is_some() {
        items.push(Item::action(
            tr!("Remove Logo"),
            Command::RemoveProjectLogo(id),
        ));
    }
    items
}

fn worktree_toggle_items(id: ProjectId, visible: bool) -> Vec<Item> {
    let mut items =
        vec![Item::action(tr!("Show Worktrees"), Command::Worktrees(id)).checked_if(visible)];
    if visible {
        items.push(Item::action(tr!("New Worktree…"), Command::NewWorktree(id)).separated());
    }
    items
}

/// The folders an agent sees in `project` besides its own, each removable,
/// plus adding another and clearing them all.
fn workspace_items(project: &Project) -> Vec<Item> {
    let id = project.id;
    let mut items = vec![Item::action(
        tr!("Add Folder…"),
        Command::AddWorkspaceFolder(id),
    )];
    if project.extra_directories.is_empty() {
        return items;
    }
    let mut roots = project.workspace();
    roots.remove(0);
    roots.sort();
    for root in roots {
        items.push(Item::action(
            root.to_string_lossy().into_owned(),
            Command::RemoveWorkspaceFolder(id, Box::new(root)),
        ));
    }
    items
        .push(Item::action(tr!("Clear Workspace"), Command::ClearWorkspaceFolders(id)).separated());
    items
}

pub(crate) fn worktree_items(project: &Project, primary: bool) -> Vec<Item> {
    let id = project.id;
    if project.status() != ProjectStatus::Available {
        return if primary {
            Vec::new()
        } else {
            vec![Item::action(
                tr!("Remove Worktree…"),
                Command::RemoveProject(id),
            )]
        };
    }
    let mut items = vec![
        Item::action(tr!("New Terminal Tab"), Command::NewProjectTab(id)),
        Item::action(tr!("Existing Terminals…"), Command::ExistingSessions(id)),
        Item::action(tr!("Apply Layout…"), Command::ProjectLayouts(id)),
    ];
    if !primary {
        items.push(
            Item::action(
                tr!("Rename Worktree…"),
                Command::EditProject(id, Field::Name),
            )
            .separated(),
        );
    }
    items.extend(path_items(project));
    if !primary {
        items.push(
            Item::action(
                tr!("Remove Worktree and Files…"),
                Command::RemoveWorktree(id),
            )
            .separated(),
        );
    }
    items
}

impl AppModel {
    /// The sidebar menu of `project`. Worktree projects get the worktree menu.
    pub(crate) fn project_menu(&self, project: &Project) -> Vec<Item> {
        if project.parent_id.is_some() {
            return worktree_items(project, false);
        }
        let repository = self
            .state
            .projects()
            .iter()
            .any(|child| child.parent_id == Some(project.id))
            || !self
                .git
                .projects
                .get(&project.id)
                .is_some_and(Repository::not_a_repository);
        items(
            project,
            repository.then(|| self.worktrees_visible(project.id)),
        )
    }

    pub(crate) fn confirm_remove_project(&mut self, project: ProjectId, cx: &mut Context<Self>) {
        let Some(record) = self.state.project(project).filter(|project| !project.home) else {
            return;
        };
        let gone = record.parent_id.is_some() && record.status() != ProjectStatus::Available;
        let message = if gone {
            tr!(
                "Remove “%@” from every client? Its folder is already gone. All of its terminal sessions will end, including those displayed in other clients, and their saved output will be discarded.",
                &record.name
            )
        } else {
            tr!(
                "Remove “%@” from every client? All of its terminal sessions will end, including those displayed in other clients, and their saved output will be discarded. The folder and its files will stay on disk.",
                &record.name
            )
        }
        .to_string();
        self.confirm(
            if gone {
                tr_key!("Remove Worktree?")
            } else {
                tr_key!("Remove Project?")
            },
            message,
            tr_key!("Remove"),
            cx,
            move |model, cx| {
                if gone {
                    model.leave_worktree(project, cx);
                }
                model.remove_project_confirmed(project, cx);
            },
        );
    }

    /// `title` and `action` are English keys, shown translated; `message` is
    /// shown as is.
    pub(crate) fn confirm(
        &mut self,
        title: &'static str,
        message: String,
        action: &'static str,
        cx: &mut Context<Self>,
        confirmed: impl FnOnce(&mut Self, &mut Context<Self>) + 'static,
    ) {
        if self.close_prompt.is_some() {
            return;
        }
        self.dismiss_overlay(cx);
        let window = self.window;
        self.close_prompt = Some(cx.spawn(async move |model, cx| {
            let response = prompt(window, title, message, action, cx).await;
            let _ = model.update(cx, |model, cx| {
                model.finish_confirmation(cx);
                model.focus_requested = true;
                match response {
                    Ok(ConfirmationResponse::Confirmed { .. }) => confirmed(model, cx),
                    Ok(ConfirmationResponse::Cancelled) => {}
                    Err(error) => {
                        model.fail(
                            tr!("Could not show confirmation: %@", error).to_string(),
                            cx,
                        );
                    }
                }
                cx.notify();
            });
        }));
    }
}

#[cfg(not(test))]
async fn prompt(
    window: AnyWindowHandle,
    title: &'static str,
    message: String,
    action: &'static str,
    cx: &mut AsyncApp,
) -> Result<ConfirmationResponse, String> {
    let (sender, receiver) = async_channel::bounded(1);
    let _dialog = window
        .update(cx, |_, window, cx| {
            murvix_ui::dialog::confirm(
                window,
                cx,
                &murvix_ui::l10n::translate(title),
                &message,
                &murvix_ui::l10n::translate(action),
                None,
                move |response| {
                    let _ = sender.try_send(response);
                },
            )
        })
        .map_err(|error| error.to_string())?
        .map_err(|error| error.to_string())?;
    Ok(receiver.recv().await.unwrap_or_default())
}

#[cfg(test)]
async fn prompt(
    window: AnyWindowHandle,
    title: &'static str,
    message: String,
    action: &'static str,
    cx: &mut AsyncApp,
) -> Result<ConfirmationResponse, String> {
    let answer = window
        .update(cx, |_, window, cx| {
            window.prompt(
                gpui::PromptLevel::Warning,
                title,
                Some(&message),
                &[action, "Cancel"],
                cx,
            )
        })
        .map_err(|error| error.to_string())?;
    Ok(if answer.await == Ok(0) {
        ConfirmationResponse::Confirmed {
            dont_ask_again: false,
        }
    } else {
        ConfirmationResponse::Cancelled
    })
}
