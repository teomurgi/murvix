use gpui::{
    AnyElement, AppContext, Context, FontWeight, InteractiveElement, IntoElement, ParentElement,
    SharedString, StatefulInteractiveElement, Styled, div, px,
};
use murvix_app_core::{Project, ProjectId, ProjectStatus};
use murvix_ui::{
    components::{IconButton, IconGlyph, Tooltip},
    icon::Icon,
    tr,
};

use crate::model::AppModel;

/// Keeps the workspace boundary separate from the project's worktrees and tabs.
pub(in crate::views) fn card(
    project: &Project,
    header: AnyElement,
    model: &AppModel,
    cx: &mut Context<AppModel>,
) -> AnyElement {
    let Some(folders) = section(project, model, cx) else {
        return header;
    };
    let id = project.id;
    div()
        .debug_selector(move || format!("workspace-card-{id}"))
        .flex()
        .flex_col()
        .rounded(model.metrics.radius_xl())
        .border_1()
        .border_color(model.theme.fg_muted)
        .child(header)
        .child(folders)
        .into_any_element()
}

/// Extra agent roots belong to the project, independently of its worktree list.
fn section(project: &Project, model: &AppModel, cx: &mut Context<AppModel>) -> Option<AnyElement> {
    if !model.appearance.sidebar_expanded
        || project.home
        || project.parent_id.is_some()
        || project.status() != ProjectStatus::Available
        || (model.state.current_project().id != project.id
            && model.state.current_project().parent_id != Some(project.id))
    {
        return None;
    }
    let local = project.server_id.is_local();
    if !local && project.extra_directories.is_empty() {
        return None;
    }
    let id = project.id;
    let m = model.metrics;
    let mut section = div()
        .debug_selector(move || format!("workspace-folders-{id}"))
        .flex()
        .flex_col()
        .gap(m.scaled(1.0))
        .pt(m.spacing1())
        .pb(m.spacing2());
    for (index, folder) in project.extra_directories.iter().enumerate() {
        let theme = model.theme.clone();
        let path = folder.to_string_lossy().into_owned();
        let name = folder
            .file_name()
            .map_or_else(|| path.clone(), |name| name.to_string_lossy().into_owned());
        let folder = folder.clone();
        section = section.child(
            div()
                .id(SharedString::from(format!("workspace-folder-{id}-{index}")))
                .debug_selector(move || format!("workspace-folder-{id}-{index}"))
                .flex()
                .flex_none()
                .items_center()
                .gap(m.spacing4())
                .px(m.spacing2())
                .h(m.control_medium())
                .text_size(m.font_footnote())
                .text_color(theme.fg_muted)
                .tooltip(move |_, cx| {
                    cx.new(|_| {
                        Tooltip::new(
                            path.clone(),
                            theme.raised(),
                            theme.fg,
                            theme.border,
                            theme.bg,
                        )
                    })
                    .into()
                })
                .child(
                    div()
                        .flex()
                        .flex_none()
                        .items_center()
                        .justify_center()
                        .w(m.icon_xxl())
                        .child(IconGlyph::new(
                            Icon::Folder,
                            m.icon_sm(),
                            model.theme.fg_muted,
                        )),
                )
                .child(div().flex_1().min_w(px(0.0)).truncate().child(name))
                .child(
                    div()
                        .debug_selector(move || format!("remove-workspace-folder-{id}-{index}"))
                        .child(
                            IconButton::new(
                                SharedString::from(format!("remove-workspace-folder-{id}-{index}")),
                                Icon::Trash,
                                m.icon_xs(),
                                m.control_small(),
                                model.theme.fg_muted,
                                model.theme.danger,
                            )
                            .tooltip(
                                tr!("Remove Folder"),
                                model.theme.raised(),
                                model.theme.fg,
                                model.theme.border,
                                model.theme.bg,
                            )
                            .on_click(cx.listener(
                                move |model, _, _, cx| {
                                    cx.stop_propagation();
                                    model.remove_workspace_folder(id, &folder, cx);
                                },
                            )),
                        ),
                ),
        );
    }
    if local {
        section = section.child(add_folder(id, model, cx));
    }
    Some(section.into_any_element())
}

fn add_folder(id: ProjectId, model: &AppModel, cx: &mut Context<AppModel>) -> AnyElement {
    let m = model.metrics;
    let group = SharedString::from(format!("add-workspace-folder-{id}"));
    div()
        .id(group.clone())
        .debug_selector(move || format!("add-workspace-folder-{id}"))
        .group(group.clone())
        .flex()
        .flex_none()
        .items_center()
        .gap(m.spacing4())
        .px(m.spacing2())
        .h(m.control_large())
        .rounded(m.radius_md())
        .text_size(m.font_footnote())
        .font_weight(FontWeight::MEDIUM)
        .text_color(model.theme.fg_muted)
        .hover(|style| style.bg(model.theme.hover).text_color(model.theme.accent))
        .cursor_pointer()
        .on_click(cx.listener(move |model, _, window, cx| {
            model.open_workspace_folder_picker(id, window, cx);
        }))
        .child(
            div()
                .flex()
                .flex_none()
                .items_center()
                .justify_center()
                .w(m.icon_xxl())
                .child(
                    IconGlyph::new(Icon::Plus, m.font_caption(), model.theme.fg_muted)
                        .hover_in_group(group, model.theme.accent),
                ),
        )
        .child(
            div()
                .flex_1()
                .min_w(px(0.0))
                .truncate()
                .child(tr!("Add folder to workspace…")),
        )
        .into_any_element()
}
