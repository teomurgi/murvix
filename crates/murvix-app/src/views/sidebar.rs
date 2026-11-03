pub(super) mod folders;
mod resize;
pub(super) mod tips;
mod vibrancy;
pub(super) mod worktrees;

pub(crate) use resize::SidebarResize;
pub(super) use resize::handle as resize_handle;

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use gpui::prelude::FluentBuilder;
use gpui::{
    AnyElement, AppContext, Bounds, Context, DragMoveEvent, Empty, FontWeight, InteractiveElement,
    IntoElement, ParentElement, Pixels, Point, SharedString, StatefulInteractiveElement, Styled,
    StyledImage, Window, div, px,
};
use murvix_app_core::{
    Project, ProjectId, ProjectStatus, ServerId,
    settings::{AppLayout, ProjectOrder, SidebarCollapsedStyle},
};

use super::{
    menu::{Command, Item},
    tab_sidebar,
};
use murvix_ui::components::{IconGlyph, SymbolGlyph};
use murvix_ui::icon::Icon;
use murvix_ui::l10n::tr_key;
use murvix_ui::theme::{contrasting_foreground, parse_hex};
use murvix_ui::tr;

use crate::model::AppModel;

pub(crate) fn register_commands(
    registry: &mut murvix_ui::command_palette::Registry<super::command_palette::Handler>,
    model: &AppModel,
    cx: &Context<AppModel>,
) {
    use super::command_palette::{Handler, action};
    use murvix_core::shortcuts::ShortcutId;
    use murvix_ui::command_palette::{Command, Registry};

    registry.register(action(
        model,
        ShortcutId::AddProject,
        tr_key!("Open Project…"),
        super::workspace::AddProject,
    ));
    let model = cx.weak_entity();
    registry.register(
        Command::list("switch_project", tr!("Switch Project…"), move |cx| {
            let mut projects = Registry::default();
            let Some(model) = model.upgrade() else {
                return projects;
            };
            let model = model.read(cx);
            for project in model
                .state
                .projects()
                .iter()
                .filter(|project| model.project_listed(project))
            {
                let id = project.id;
                let handler: Handler = Rc::new(move |model, _, cx| model.select_project(id, cx));
                let title = project
                    .parent_id
                    .and_then(|parent| {
                        model
                            .state
                            .projects()
                            .iter()
                            .find(|project| project.id == parent)
                    })
                    .map_or_else(
                        || model.project_title(project),
                        |parent| format!("{} / {}", parent.name, project.name),
                    );
                projects.register(
                    Command::new(id.to_string(), title, handler)
                        .keywords(project.directory.to_string_lossy())
                        .disabled(project.status() == ProjectStatus::Missing),
                );
            }
            projects
        })
        .keywords("Switch Project"),
    );
}

pub(crate) fn sidebar(model: &AppModel, window: &Window, cx: &mut Context<AppModel>) -> AnyElement {
    let m = model.metrics;
    if model.extension_sidebar_active() {
        return div()
            .debug_selector(|| "workspace-sidebar".into())
            .flex()
            .flex_col()
            .flex_none()
            .w(px(model.sidebar_width()))
            .h_full()
            .min_h(px(0.0))
            .bg(model.sidebar_background())
            .map(|sidebar| super::titlebar::top_corners(sidebar, window, true, false))
            .child(div().h(m.title_bar_height()).flex_none())
            .children(model.webviews.sidebar.as_ref().map(|sidebar| {
                div()
                    .flex()
                    .flex_1()
                    .min_h(px(0.0))
                    .child(sidebar.surface.view.clone())
            }))
            .into_any_element();
    }
    let header = header(model, cx);
    let contents = match model.appearance.layout {
        AppLayout::ProjectFocused => project_list(model, cx),
        AppLayout::TabFocused | AppLayout::AgentsFocused => {
            tab_sidebar::contents(model, window, cx)
        }
    };
    div()
        .debug_selector(|| "workspace-sidebar".into())
        .flex()
        .flex_col()
        .flex_none()
        .w(px(model.sidebar_width()))
        .h_full()
        .min_h(px(0.0))
        .child(
            div()
                .flex()
                .flex_col()
                .flex_1()
                .min_h(px(0.0))
                .bg(model.sidebar_background())
                .map(|sidebar| super::titlebar::top_corners(sidebar, window, true, false))
                .child(div().h(m.title_bar_height()).flex_none())
                .child(header)
                .child(contents)
                .children(tips::footer(model, cx))
                .children(super::agents_panel::collapsed_button(model, cx)),
        )
        .children(super::agents_panel::docked(model, window, cx))
        .child(
            div()
                .flex_none()
                .bg(model.sidebar_background())
                .child(super::remote_servers::section(model, cx)),
        )
        .into_any_element()
}

impl AppModel {
    /// Add Project opens the picker on this computer, or, with remote
    /// devices, a menu to choose where the project is.
    pub(crate) fn choose_project_source(
        &mut self,
        position: Point<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.settings.servers.is_empty() {
            self.open_project_picker(window, cx);
            return;
        }
        let devices = self
            .settings
            .servers
            .iter()
            .map(|entry| Item::action(entry.name.clone(), Command::AddRemoteProject(entry.id)))
            .chain(std::iter::once(
                Item::action(tr!("Manage Remote Servers…"), Command::ManageRemoteDevices)
                    .separated(),
            ))
            .collect();
        self.open_menu(
            vec![
                Item::action(tr!("Local"), Command::AddProject),
                Item::submenu(tr!("Remote"), devices),
            ],
            position,
            window,
            cx,
        );
    }

    /// The name shown for a project: another computer's Home goes by its
    /// server's name.
    pub(crate) fn project_title(&self, project: &Project) -> String {
        match self.server_name(project.server_id) {
            Some(name) if project.home => name.to_owned(),
            _ => project.name.clone(),
        }
    }

    /// Where another computer's project is, as `box · ~/code/app`.
    pub(crate) fn remote_location(&self, project: &Project) -> Option<String> {
        let name = self.server_name(project.server_id)?;
        Some(format!("{name} · {}", self.project_path_label(project)))
    }

    /// The project's folder, with `~` for its own computer's Home.
    pub(crate) fn project_path_label(&self, project: &Project) -> String {
        if project.server_id.is_local() {
            return super::project_picker::display_path(&project.directory);
        }
        let directory = project.directory.to_string_lossy();
        match self.remote_home(project.server_id) {
            Some(home) if project.directory == home => "~".to_owned(),
            Some(home) => project.directory.strip_prefix(home).map_or_else(
                |_| directory.into_owned(),
                |relative| format!("~/{}", relative.display()),
            ),
            None => directory.into_owned(),
        }
    }

    pub(crate) fn cached_sidebar(&self) -> gpui::AnyView {
        gpui::AnyView::from(self.sidebar_view.clone()).cached(
            div()
                .w(px(self.sidebar_width()))
                .h_full()
                .min_h(px(0.0))
                .flex_none()
                .style()
                .clone(),
        )
    }

    pub(crate) fn sidebar_width(&self) -> f32 {
        if self.appearance.sidebar_expanded {
            let width = self
                .appearance
                .sidebar_expanded_width
                .filter(|width| width.is_finite())
                .map_or_else(
                    || self.metrics.sidebar_expanded_width(),
                    |width| self.metrics.scaled(width),
                );
            f32::from(self.clamp_expanded_sidebar_width(width))
        } else if (self.appearance.layout == AppLayout::ProjectFocused
            || self.extension_sidebar_active())
            && self.appearance.sidebar_collapsed_style == SidebarCollapsedStyle::Icons
        {
            f32::from(self.metrics.sidebar_collapsed_width())
        } else {
            0.0
        }
    }

    fn clamp_expanded_sidebar_width(&self, width: Pixels) -> Pixels {
        let width = width.clamp(
            self.metrics.sidebar_expanded_min_width(),
            self.metrics.sidebar_expanded_max_width(),
        );
        #[cfg(target_os = "linux")]
        let width = width.min(self.sidebar_available_width);
        width
    }

    #[cfg(target_os = "linux")]
    pub(crate) fn fit_sidebar_to_window(
        &mut self,
        window: &Window,
        actions_width: f32,
        cx: &mut Context<Self>,
    ) {
        let available = (super::titlebar::content_size(window).width
            - px(actions_width)
            - self.metrics.control_medium()
            - self.metrics.spacing2()
            - px(56.0))
        .max(self.metrics.sidebar_expanded_min_width());
        if self.sidebar_available_width != available {
            self.sidebar_available_width = available;
            self.sidebar_view.update(cx, |_, cx| cx.notify());
        }
    }

    pub(crate) fn sidebar_filter_label(&self) -> SharedString {
        if self.appearance.sidebar_focus {
            return tr!("Focused Project");
        }
        if self.state.remote_servers_selected() {
            return tr!("Remote Servers");
        }
        tr!("All Projects")
    }

    /// Top-level projects the sidebar filter lists, in visual section order.
    pub(crate) fn listed_parents(&self) -> Vec<&Project> {
        self.sidebar_sections()
            .into_iter()
            .flat_map(|section| section.projects)
            .collect()
    }

    fn sorted_listed_parents(&self) -> Vec<&Project> {
        let mut parents: Vec<_> = self
            .state
            .projects()
            .iter()
            .filter(|project| {
                project.parent_id.is_none()
                    && self.state.is_listed(project)
                    && self.project_listed(project)
            })
            .collect();
        self.sort_projects(&mut parents);
        parents
    }

    pub(super) fn sidebar_sections(&self) -> Vec<ProjectSection<'_>> {
        let active = self.state.current_project();
        let focused = active.parent_id.unwrap_or(active.id);
        let mut remaining: Vec<_> = self
            .sorted_listed_parents()
            .into_iter()
            .filter(|project| !self.appearance.sidebar_focus || project.id == focused)
            .collect();
        let mut sections = Vec::new();
        let mut pinned = Vec::new();
        for id in &self.appearance.pinned_projects {
            if let Some(index) = remaining.iter().position(|project| project.id == *id) {
                pinned.push(remaining.remove(index));
            }
        }
        if !pinned.is_empty() {
            sections.push(ProjectSection {
                id: ProjectSectionId::Pinned,
                projects: pinned,
            });
        }
        let mut servers = Vec::new();
        for project in &remaining {
            if !servers.contains(&project.server_id) {
                servers.push(project.server_id);
            }
        }
        servers.sort_by_key(|server| self.server_section_order(*server));
        sections.extend(servers.into_iter().map(|server| {
            ProjectSection {
                id: ProjectSectionId::Server(server),
                projects: remaining
                    .iter()
                    .copied()
                    .filter(|project| project.server_id == server)
                    .collect(),
            }
        }));
        sections
    }

    fn server_section_order(&self, server: ServerId) -> (bool, usize) {
        let configured = self
            .appearance
            .sidebar_server_order
            .iter()
            .position(|candidate| *candidate == server);
        let default = if server.is_local() {
            0
        } else {
            self.settings
                .servers
                .iter()
                .position(|entry| entry.id == server)
                .map_or(usize::MAX, |index| index + 1)
        };
        (configured.is_none(), configured.unwrap_or(default))
    }

    fn project_section(&self, project: ProjectId) -> Option<ProjectSectionId> {
        let project = self.state.project(project)?;
        if project.parent_id.is_some() || project.home {
            return None;
        }
        Some(if self.appearance.pinned_projects.contains(&project.id) {
            ProjectSectionId::Pinned
        } else {
            ProjectSectionId::Server(project.server_id)
        })
    }

    pub(crate) fn project_pinned(&self, project: ProjectId) -> bool {
        self.project_section(project) == Some(ProjectSectionId::Pinned)
    }

    pub(crate) fn toggle_project_pin(&mut self, project: ProjectId, cx: &mut Context<Self>) {
        let Some(section) = self.project_section(project) else {
            return;
        };
        if section == ProjectSectionId::Pinned {
            self.appearance.pinned_projects.retain(|id| *id != project);
        } else {
            self.appearance.pinned_projects.push(project);
        }
        self.save_appearance(cx);
        cx.notify();
    }

    fn move_project_in_section(&mut self, from: ProjectId, to: ProjectId, cx: &mut Context<Self>) {
        let (Some(from_section), Some(to_section)) =
            (self.project_section(from), self.project_section(to))
        else {
            return;
        };
        if from_section != to_section {
            return;
        }
        if from_section == ProjectSectionId::Pinned {
            self.appearance.pinned_projects.retain(|id| *id != from);
            let Some(index) = self
                .appearance
                .pinned_projects
                .iter()
                .position(|id| *id == to)
            else {
                return;
            };
            self.appearance.pinned_projects.insert(index, from);
            self.save_appearance(cx);
            cx.notify();
        } else {
            self.move_project(from, to, cx);
        }
    }

    fn move_server_section(&mut self, from: ServerId, to: ServerId, cx: &mut Context<Self>) {
        if from == to {
            return;
        }
        let mut order: Vec<_> = self
            .sidebar_sections()
            .into_iter()
            .filter_map(|section| match section.id {
                ProjectSectionId::Pinned => None,
                ProjectSectionId::Server(server) => Some(server),
            })
            .collect();
        let Some(from_index) = order.iter().position(|server| *server == from) else {
            return;
        };
        order.remove(from_index);
        let Some(to_index) = order.iter().position(|server| *server == to) else {
            return;
        };
        order.insert(to_index, from);
        self.appearance.sidebar_server_order = order;
        self.save_appearance(cx);
        cx.notify();
    }

    pub(crate) fn sort_projects(&self, projects: &mut [&Project]) {
        match self.appearance.sidebar_project_order {
            ProjectOrder::Manual => {}
            ProjectOrder::Name | ProjectOrder::NameDescending => {
                projects.sort_by(|left, right| {
                    right.home.cmp(&left.home).then_with(|| {
                        let order =
                            crate::picker::path_service::natural_compare(&left.name, &right.name);
                        if self.appearance.sidebar_project_order == ProjectOrder::NameDescending {
                            order.reverse()
                        } else {
                            order
                        }
                    })
                });
            }
            ProjectOrder::RecentlyActive => {
                let recent: std::collections::HashMap<_, _> = self
                    .appearance
                    .project_recent
                    .iter()
                    .enumerate()
                    .map(|(index, id)| (*id, index))
                    .collect();
                projects.sort_by_key(|project| {
                    (
                        !project.home,
                        recent.get(&project.id).copied().unwrap_or(usize::MAX),
                    )
                });
            }
            ProjectOrder::DateAdded => {
                projects.sort_by_key(|project| (!project.home, project.created_at));
            }
        }
    }

    pub(crate) fn sidebar_projects(&self) -> Vec<&Project> {
        let active = self.state.current_project();
        let focused = active.parent_id.unwrap_or(active.id);
        self.listed_parents()
            .into_iter()
            .filter(|project| !self.appearance.sidebar_focus || project.id == focused)
            .flat_map(|parent| {
                let children = if self.appearance.layout != AppLayout::ProjectFocused
                    && self.worktrees_visible(parent.id)
                {
                    self.worktree_children(parent.id)
                } else {
                    Vec::new()
                };
                std::iter::once(parent).chain(children)
            })
            .collect()
    }
}

fn header(model: &AppModel, cx: &mut Context<AppModel>) -> AnyElement {
    if !model.appearance.sidebar_expanded {
        return div().into_any_element();
    }
    let m = model.metrics;
    let theme = &model.theme;
    div()
        .flex()
        .items_center()
        .gap(m.spacing2())
        .px(m.spacing3())
        .pt(m.spacing2())
        .child(
            div()
                .id("sidebar-project-filter")
                .debug_selector(|| "sidebar-project-filter".into())
                .flex()
                .flex_1()
                .min_w(px(0.0))
                .items_center()
                .justify_between()
                .px(m.spacing4())
                .h(m.control_medium())
                .rounded(m.radius_md())
                .bg(theme.surface)
                .cursor_pointer()
                .hover(|style| style.bg(theme.hover))
                .text_size(m.font_caption())
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(theme.fg_muted)
                .on_click(cx.listener(|model, event: &gpui::ClickEvent, window, cx| {
                    model.open_menu(filter_items(model), event.position(), window, cx);
                }))
                .child(
                    div()
                        .flex_1()
                        .min_w(px(0.0))
                        .truncate()
                        .child(model.sidebar_filter_label()),
                )
                .child(IconGlyph::new(
                    Icon::ChevronDown,
                    m.font_caption(),
                    theme.fg_muted,
                )),
        )
        .child(
            div()
                .id("sidebar-project-sort")
                .debug_selector(|| "sidebar-project-sort".into())
                .flex()
                .items_center()
                .justify_center()
                .size(m.control_medium())
                .rounded(m.radius_md())
                .bg(theme.surface)
                .cursor_pointer()
                .hover(|style| style.bg(theme.hover))
                .on_click(cx.listener(|model, event: &gpui::ClickEvent, window, cx| {
                    model.open_menu(
                        [
                            (tr!("Manual Order"), ProjectOrder::Manual),
                            (tr!("Name (A–Z)"), ProjectOrder::Name),
                            (tr!("Name (Z–A)"), ProjectOrder::NameDescending),
                            (tr!("Recently Active"), ProjectOrder::RecentlyActive),
                            (tr!("Date Added"), ProjectOrder::DateAdded),
                        ]
                        .into_iter()
                        .map(|(title, order)| {
                            Item::action(title, Command::SortProjects(order))
                                .checked_if(model.appearance.sidebar_project_order == order)
                        })
                        .collect(),
                        event.position(),
                        window,
                        cx,
                    );
                }))
                .child(IconGlyph::new(
                    Icon::ArrowUpDown,
                    m.font_body(),
                    theme.fg_muted,
                )),
        )
        .child(layout_selector(model, cx))
        .into_any_element()
}

fn filter_items(model: &AppModel) -> Vec<Item> {
    let focused = model.appearance.sidebar_focus;
    let remote = model.state.remote_servers_selected();
    let mut items = vec![
        Item::action(tr!("All Projects"), Command::SelectAllProjects)
            .checked_if(!focused && !remote),
    ];
    if model.remote_servers_offered() {
        items.push(
            Item::action(tr!("Remote Servers"), Command::SelectRemoteServers)
                .checked_if(!focused && remote),
        );
    }
    items.push(
        Item::action(tr!("Focus Current Project"), Command::FocusProject(true))
            .checked_if(focused)
            .separated(),
    );
    items
}

fn layout_selector(model: &AppModel, cx: &mut Context<AppModel>) -> AnyElement {
    let m = model.metrics;
    let theme = &model.theme;
    div()
        .id("layout-menu")
        .debug_selector(|| "layout-menu".into())
        .group("layout-menu")
        .flex()
        .flex_none()
        .items_center()
        .justify_center()
        .size(m.control_medium())
        .rounded(m.radius_md())
        .bg(theme.surface)
        .hover(|style| style.bg(theme.hover))
        .cursor_pointer()
        .on_mouse_down(gpui::MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .on_click(cx.listener(|model, event: &gpui::ClickEvent, window, cx| {
            cx.stop_propagation();
            model.open_menu(
                vec![
                    Item::action(
                        tr!("Project Focused"),
                        Command::Layout(AppLayout::ProjectFocused),
                    )
                    .checked_if(model.appearance.layout == AppLayout::ProjectFocused),
                    Item::action(tr!("Tab Focused"), Command::Layout(AppLayout::TabFocused))
                        .checked_if(model.appearance.layout == AppLayout::TabFocused),
                    Item::action(
                        tr!("Agents Focused"),
                        Command::Layout(AppLayout::AgentsFocused),
                    )
                    .checked_if(model.appearance.layout == AppLayout::AgentsFocused),
                    Item::action(
                        if model.appearance.agents_panel_visible {
                            tr!("Hide agents panel")
                        } else {
                            tr!("Show agents panel")
                        },
                        Command::AgentsPanel(!model.appearance.agents_panel_visible),
                    )
                    .separated(),
                ],
                event.position(),
                window,
                cx,
            );
        }))
        .child(
            IconGlyph::new(Icon::Grid, m.font_body(), theme.fg_muted)
                .hover_in_group("layout-menu", theme.fg),
        )
        .into_any_element()
}

#[derive(Clone, Copy, Eq, PartialEq)]
pub(super) enum ProjectSectionId {
    Pinned,
    Server(ServerId),
}

pub(super) struct ProjectSection<'a> {
    pub(super) id: ProjectSectionId,
    pub(super) projects: Vec<&'a Project>,
}

enum SidebarDrag {
    Project(DraggedProject),
    Section(DraggedSection),
}

struct DraggedProject {
    id: ProjectId,
    last_target: Cell<Option<ProjectId>>,
}

impl DraggedProject {
    fn move_to(&self, target: Option<ProjectId>, model: &mut AppModel, cx: &mut Context<AppModel>) {
        if model.overlay.is_some()
            || model.close_prompt.is_some()
            || model.appearance.sidebar_project_order != ProjectOrder::Manual
        {
            return;
        }
        let target = target.filter(|target| *target != self.id);
        if self.last_target.replace(target) != target
            && let Some(target) = target
        {
            model.move_project_in_section(self.id, target, cx);
        }
    }
}

struct DraggedSection {
    id: ServerId,
    last_target: Cell<Option<ServerId>>,
}

impl DraggedSection {
    fn move_to(&self, target: Option<ServerId>, model: &mut AppModel, cx: &mut Context<AppModel>) {
        if model.overlay.is_some() || model.close_prompt.is_some() {
            return;
        }
        let target = target.filter(|target| *target != self.id);
        if self.last_target.replace(target) != target
            && let Some(target) = target
        {
            model.move_server_section(self.id, target, cx);
        }
    }
}

impl SidebarDrag {
    fn move_to(
        &self,
        target: Option<SidebarDropTarget>,
        model: &mut AppModel,
        cx: &mut Context<AppModel>,
    ) {
        match self {
            Self::Project(drag) => drag.move_to(
                target.and_then(|target| match target {
                    SidebarDropTarget::Project(id) => Some(id),
                    SidebarDropTarget::Section(_) => None,
                }),
                model,
                cx,
            ),
            Self::Section(drag) => drag.move_to(
                target.and_then(|target| match target {
                    SidebarDropTarget::Project(_) => None,
                    SidebarDropTarget::Section(id) => Some(id),
                }),
                model,
                cx,
            ),
        }
    }
}

#[derive(Clone, Copy)]
enum SidebarDropTarget {
    Project(ProjectId),
    Section(ServerId),
}

#[derive(Default)]
struct ProjectRows {
    projects: Vec<(ProjectId, Bounds<Pixels>)>,
    sections: Vec<(ServerId, Bounds<Pixels>)>,
}

impl ProjectRows {
    fn at(&self, position: Point<Pixels>) -> Option<SidebarDropTarget> {
        self.projects
            .iter()
            .find_map(|(id, bounds)| {
                bounds
                    .contains(&position)
                    .then_some(SidebarDropTarget::Project(*id))
            })
            .or_else(|| {
                self.sections.iter().find_map(|(id, bounds)| {
                    bounds
                        .contains(&position)
                        .then_some(SidebarDropTarget::Section(*id))
                })
            })
    }
}

pub(super) fn section_title(section: ProjectSectionId, model: &AppModel) -> String {
    match section {
        ProjectSectionId::Pinned => tr!("Pinned").to_string(),
        ProjectSectionId::Server(server) => model
            .server_name(server)
            .map_or_else(|| tr!("Unknown machine").to_string(), str::to_owned),
    }
}

fn section_header(section: ProjectSectionId, model: &AppModel) -> AnyElement {
    let m = model.metrics;
    let theme = &model.theme;
    let (id, drag) = match section {
        ProjectSectionId::Pinned => ("pinned".to_owned(), None),
        ProjectSectionId::Server(server) => (
            server.to_string(),
            Some(SidebarDrag::Section(DraggedSection {
                id: server,
                last_target: Cell::new(None),
            })),
        ),
    };
    let title = section_title(section, model);
    div()
        .id(SharedString::from(format!("project-section-{id}")))
        .debug_selector(move || format!("project-section-{id}"))
        .h(m.control_medium())
        .px(m.spacing2())
        .flex()
        .flex_none()
        .items_center()
        .text_size(m.font_caption())
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(theme.fg_muted)
        .when_some(drag, |header, drag| {
            header
                .cursor_pointer()
                .on_drag(drag, |_, _, _, cx| cx.new(|_| Empty))
        })
        .child(title)
        .into_any_element()
}

#[allow(
    clippy::too_many_lines,
    reason = "Declarative project-section rendering and drag interaction"
)]
fn project_list(model: &AppModel, cx: &mut Context<AppModel>) -> AnyElement {
    let m = model.metrics;
    let wide = model.appearance.sidebar_expanded;
    let targets = Rc::new(RefCell::new(ProjectRows::default()));
    let measured_sections = targets.clone();
    let moving = targets.clone();
    let dropping = targets.clone();
    let sections = model.sidebar_sections();
    let shortcuts = model.navigation_projects();
    let section_servers: Vec<_> = sections
        .iter()
        .map(|section| match section.id {
            ProjectSectionId::Pinned => None,
            ProjectSectionId::Server(server) => Some(server),
        })
        .collect();
    let mut index = 0;
    let rows: Vec<_> = sections
        .iter()
        .map(|section| {
            let ids: Vec<_> = section
                .projects
                .iter()
                .filter(|project| {
                    !project.home
                        && project.status() == ProjectStatus::Available
                        && project.parent_id.is_none()
                })
                .map(|project| project.id)
                .collect();
            let measured = targets.clone();
            let groups: Vec<_> = section
                .projects
                .iter()
                .map(|project| {
                    let shortcut = shortcuts.iter().take(9).position(|id| *id == project.id);
                    let group = worktrees::group(project, index, shortcut, model, cx);
                    index += 1;
                    group
                })
                .collect();
            div()
                .flex()
                .flex_col()
                .gap(m.spacing1())
                .child(section_header(section.id, model))
                .children(groups)
                .on_children_prepainted(move |bounds, _, _| {
                    let mut rows = measured.borrow_mut();
                    rows.projects.retain(|(id, _)| !ids.contains(id));
                    rows.projects.extend(
                        ids.iter()
                            .zip(bounds.iter().skip(1))
                            .map(|(id, bounds)| (*id, *bounds)),
                    );
                })
        })
        .collect();
    div()
        .id("projects-scroll")
        .debug_selector(|| "projects-scroll".into())
        .flex_1()
        .min_h(px(0.0))
        .overflow_y_scroll()
        .on_drag_move(cx.listener(
            move |model, event: &DragMoveEvent<SidebarDrag>, window, cx| {
                if event.event.pressed_button != Some(gpui::MouseButton::Left) {
                    cx.stop_active_drag(window);
                    return;
                }
                let target = event
                    .bounds
                    .contains(&event.event.position)
                    .then(|| moving.borrow().at(event.event.position))
                    .flatten();
                if let Some(drag) = event.dragged_item().downcast_ref::<SidebarDrag>() {
                    drag.move_to(target, model, cx);
                }
            },
        ))
        .on_drop(cx.listener(move |model, drag: &SidebarDrag, window, cx| {
            drag.move_to(dropping.borrow().at(window.mouse_position()), model, cx);
        }))
        .child(
            div()
                .flex()
                .flex_col()
                .px(if wide { m.spacing3() } else { m.spacing4() })
                .gap(m.spacing3())
                .pt(if wide { m.spacing5() } else { m.spacing2() })
                .pb(m.spacing3())
                .when(!wide, Styled::items_center)
                .children(rows)
                .when(!model.appearance.sidebar_focus, |list| {
                    list.child(add_project_button(model, cx))
                })
                .on_children_prepainted(move |bounds, _, _| {
                    measured_sections.borrow_mut().sections = section_servers
                        .iter()
                        .zip(bounds)
                        .filter_map(|(server, bounds)| server.map(|server| (server, bounds)))
                        .collect();
                }),
        )
        .into_any_element()
}

#[allow(
    clippy::too_many_lines,
    reason = "Declarative project row and interaction layout"
)]
fn project_row(
    project: &Project,
    index: usize,
    shortcut: Option<usize>,
    model: &AppModel,
    cx: &mut Context<AppModel>,
) -> AnyElement {
    let m = model.metrics;
    let theme = &model.theme;
    let wide = model.appearance.sidebar_expanded;
    let id = project.id;
    let missing = project.status() == ProjectStatus::Missing;
    let active = model.state.current_project().id == id
        || model.state.current_project().parent_id == Some(id);
    let has_worktrees = model.has_worktrees(project) && !missing;
    let group = SharedString::from(format!("project-{id}"));
    let tile = shortcut
        .and_then(|index| {
            model.shortcut_hints.label(
                murvix_core::shortcuts::ShortcutId::PROJECTS[index],
                &model.settings.keymap,
            )
        })
        .map_or_else(
            || project_tile(project, model, group.clone()),
            |label| {
                div()
                    .debug_selector(move || format!("project-shortcut-{id}"))
                    .child(super::shortcut_hints::badge(label, m.icon_xxl(), model))
                    .into_any_element()
            },
        );
    let activity = super::tab_activity::project_status(id, model);
    let server = model.server_name(project.server_id);
    let drag = DraggedProject {
        id,
        last_target: Cell::new(None),
    };
    div()
        .id(group.clone())
        .debug_selector(|| format!("project-row-{index}"))
        .group(group)
        .relative()
        .flex()
        .flex_none()
        .items_center()
        .when(wide && project.parent_id.is_some(), |row| {
            row.ml(m.spacing5())
        })
        .when(wide, |row| {
            row.p(m.spacing2())
                .h(m.icon_xxl() + m.spacing2() * 2.0)
                .gap(m.spacing4())
                .rounded(m.radius_lg())
                .when(active, |row| row.bg(theme.surface))
                .hover(|style| style.bg(if active { theme.surface } else { theme.hover }))
        })
        .when(!wide, |row| row.justify_center().size(m.scaled(34.0)))
        .when(missing, |row| row.opacity(0.5))
        .when(!missing, |row| {
            row.cursor_pointer()
                .on_click(cx.listener(move |model, _, window, cx| {
                    if active && has_worktrees && wide {
                        model.toggle_worktree_list(id, cx);
                    } else if active && has_worktrees {
                        model.expanded_worktrees.insert(id);
                        model.toggle_sidebar(window, cx);
                    } else {
                        model.select_project(model.preferred_worktree(id), cx);
                        model.focus_active(window, cx);
                    }
                }))
        })
        .on_mouse_down(
            gpui::MouseButton::Right,
            cx.listener(move |model, event: &gpui::MouseDownEvent, window, cx| {
                if let Some(project) = model.state.project(id) {
                    model.open_menu(model.project_menu(project), event.position, window, cx);
                }
            }),
        )
        .when(
            !project.home
                && project.parent_id.is_none()
                && !missing
                && model.appearance.sidebar_project_order == ProjectOrder::Manual,
            |row| row.on_drag(SidebarDrag::Project(drag), |_, _, _, cx| cx.new(|_| Empty)),
        )
        .child(tile)
        .when(wide, |row| {
            row.child(
                div()
                    .flex_1()
                    .min_w(px(0.0))
                    .flex()
                    .flex_col()
                    .gap(m.scaled(1.0))
                    .text_color(theme.fg)
                    .child(
                        div()
                            .truncate()
                            .text_size(m.font_emphasis())
                            .font_weight(FontWeight::MEDIUM)
                            .child(model.project_title(project)),
                    )
                    .when(has_worktrees, |label| {
                        let selected = model.state.project(model.preferred_worktree(id));
                        let primary = tr!("primary");
                        let worktree = selected
                            .filter(|p| p.parent_id.is_some())
                            .map_or(primary.as_ref(), |p| p.name.as_str());
                        label.child(
                            div()
                                .debug_selector(move || format!("project-worktree-label-{id}"))
                                .truncate()
                                .text_size(m.font_footnote())
                                .font_family(murvix_ui::font::monospace())
                                .font_weight(FontWeight::NORMAL)
                                .child(server.map_or_else(
                                    || worktree.to_owned(),
                                    |server| format!("{server} · {worktree}"),
                                )),
                        )
                    })
                    .when_some(
                        model
                            .remote_location(project)
                            .filter(|_| !has_worktrees && !project.is_remote_home()),
                        |label, location| {
                            label.child(
                                div()
                                    .debug_selector(move || format!("project-server-label-{id}"))
                                    .truncate()
                                    .text_size(m.font_footnote())
                                    .font_weight(FontWeight::NORMAL)
                                    .text_color(theme.fg_muted)
                                    .child(location),
                            )
                        },
                    ),
            )
        })
        .when(wide, |row| row.children(remote_marker(project, model)))
        .when(activity != super::tab_activity::Status::None, |row| {
            row.child(
                div()
                    .flex()
                    .flex_none()
                    .items_center()
                    .justify_center()
                    .min_w(m.scaled(18.0))
                    .h(m.scaled(18.0))
                    .when(!wide, |badge| {
                        badge.absolute().right(m.scaled(-3.0)).top(m.scaled(-3.0))
                    })
                    .child(super::tab_activity::status_glyph(
                        format!("project-activity-{id}"),
                        activity,
                        m.icon_sm(),
                        model,
                    )),
            )
        })
        .when(wide && has_worktrees, |row| {
            row.child(worktrees::disclosure(project, model, cx))
        })
        .when(!wide && active, |row| {
            row.child(
                div()
                    .absolute()
                    .inset_0()
                    .rounded(m.scaled(9.0))
                    .border(m.scaled(1.5))
                    .border_color(theme.accent),
            )
        })
        .when(missing, |row| {
            row.child(div().absolute().top_0().right_0().child(SymbolGlyph::new(
                "exclamationmark.triangle.fill",
                m.font_caption(),
                theme.warning,
            )))
        })
        .into_any_element()
}

/// Marks another computer's project, naming it in a tooltip.
pub(super) fn remote_marker(project: &Project, model: &AppModel) -> Option<AnyElement> {
    if project.is_remote_home() {
        return None;
    }
    let name = model.server_name(project.server_id)?;
    let id = project.id;
    let m = model.metrics;
    let theme = model.theme.clone();
    let tooltip = tr!("Remote project on %@", name);
    Some(
        div()
            .id(SharedString::from(format!("project-remote-{id}")))
            .debug_selector(move || format!("project-remote-{id}"))
            .flex()
            .flex_none()
            .items_center()
            .justify_center()
            .size(m.scaled(18.0))
            .tooltip(move |_, cx| {
                cx.new(|_| {
                    murvix_ui::components::Tooltip::new(
                        tooltip.clone(),
                        theme.raised(),
                        theme.fg,
                        theme.border,
                        theme.bg,
                    )
                })
                .into()
            })
            .child(IconGlyph::new(
                Icon::Network,
                m.font_caption(),
                model.theme.fg_muted,
            ))
            .into_any_element(),
    )
}

pub(super) fn add_project_button(model: &AppModel, cx: &mut Context<AppModel>) -> AnyElement {
    let m = model.metrics;
    let theme = &model.theme;
    let wide = model.appearance.sidebar_expanded;
    div()
        .id("add-project")
        .flex()
        .flex_none()
        .items_center()
        .cursor_pointer()
        .when(wide, |row| {
            row.p(m.spacing2())
                .gap(m.spacing4())
                .rounded(m.radius_lg())
                .hover(|style| style.bg(theme.hover))
        })
        .when(!wide, |row| row.justify_center().size(m.scaled(34.0)))
        .on_click(cx.listener(|model, event: &gpui::ClickEvent, window, cx| {
            model.choose_project_source(event.position(), window, cx);
        }))
        .child(
            div()
                .flex()
                .flex_none()
                .items_center()
                .justify_center()
                .size(m.icon_xxl())
                .rounded(m.radius_md())
                .bg(theme.surface)
                .child(IconGlyph::new(
                    Icon::Plus,
                    m.font_emphasis(),
                    theme.fg_muted,
                )),
        )
        .when(wide, |row| {
            row.child(
                div()
                    .text_size(m.font_body())
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(theme.fg_muted)
                    .child(tr!("Add Project")),
            )
        })
        .into_any_element()
}

fn project_tile(project: &Project, model: &AppModel, group: SharedString) -> AnyElement {
    let m = model.metrics;
    let color = if project.home && project.server_id.is_local() {
        model.theme.accent.to_rgb()
    } else {
        parse_hex(project.color.as_str()).unwrap_or(gpui::rgb(0x80_80_80))
    };
    let foreground = contrasting_foreground(color);
    let glyph = if let Some((_, logo)) = model.project_logos.get(&project.id) {
        gpui::img(logo.clone())
            .size(m.icon_xxl())
            .rounded(m.radius_md())
            .object_fit(gpui::ObjectFit::Cover)
            .into_any_element()
    } else if let Some(icon) = project
        .icon
        .as_ref()
        .filter(|icon| murvix_ui::symbols::PROJECT_ICONS || !icon.starts_with("sf:"))
    {
        if let Some(symbol) = icon.strip_prefix("sf:") {
            SymbolGlyph::new(symbol.to_owned(), m.font_title_large(), foreground.into())
                .into_any_element()
        } else {
            div()
                .text_size(m.font_title_large())
                .child(icon.clone())
                .into_any_element()
        }
    } else if project.home && project.server_id.is_local() {
        SymbolGlyph::new("house.fill", m.font_title_large(), foreground.into()).into_any_element()
    } else {
        div()
            .text_size(m.font_emphasis())
            .font_weight(FontWeight::BOLD)
            .text_color(foreground)
            .child(murvix_app_core::initial(&model.project_title(project)).to_uppercase())
            .into_any_element()
    };
    div()
        .flex()
        .flex_none()
        .items_center()
        .justify_center()
        .size(m.icon_xxl())
        .rounded(m.radius_md())
        .when(!model.project_logos.contains_key(&project.id), |tile| {
            tile.bg(color)
        })
        .overflow_hidden()
        .group_hover(group, |style| style.opacity(0.85))
        .child(glyph)
        .into_any_element()
}
