use super::*;
use murvix_app_core::settings::{AppLayout, ProjectOrder, Settings};

fn fixture() -> (AppState, [ProjectId; 4]) {
    let mut state = AppState::bootstrap().expect("state");
    let ids = ["repo10", "repo2b", "Repo1", "repo2a"].map(|name| {
        let id = state
            .add_project(ServerId::local(), std::env::temp_dir())
            .expect("project");
        state.rename_project(id, name).expect("name");
        id
    });
    let mut stored = serde_json::to_value(&state).expect("state JSON");
    for (index, project) in stored["projects"]
        .as_array_mut()
        .expect("projects")
        .iter_mut()
        .enumerate()
    {
        project["created_at"] =
            serde_json::json!({"secs_since_epoch": index, "nanos_since_epoch": 0});
    }
    let mut state: AppState = serde_json::from_value(stored).expect("dated state");
    state.move_project(ids[0], 4).expect("manual order");
    state.select_project(state.home().id).expect("home");
    (state, ids)
}

fn parent_ids(model: &AppModel) -> Vec<ProjectId> {
    model
        .listed_parents()
        .iter()
        .map(|project| project.id)
        .collect()
}

fn click(selector: &'static str, cx: &mut VisualTestContext) {
    cx.update(|window, _| window.refresh());
    cx.run_until_parked();
    let bounds = cx.debug_bounds(selector).expect(selector);
    cx.simulate_click(bounds.center(), Modifiers::none());
    cx.run_until_parked();
}

#[gpui::test]
fn project_sort_menu_updates_every_layout_navigation_and_new_tab_picker(cx: &mut TestAppContext) {
    let (state, [ten, two_b, one, two_a]) = fixture();
    let home = state.home().id;
    let manual = [home, two_b, one, two_a, ten];
    let (mut boot, _requests) = stub_boot(state);
    boot.settings.appearance.sidebar_expanded = true;
    boot.settings.appearance.worktree_recent = vec![two_a, two_b];
    cx.update(|cx| crate::views::workspace::bind_keys(&boot.settings.keymap, cx));
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    cx.simulate_resize(size(px(1200.0), px(800.0)));
    for layout in [
        AppLayout::ProjectFocused,
        AppLayout::TabFocused,
        AppLayout::AgentsFocused,
    ] {
        view.update(cx, |model, cx| model.set_layout(layout, cx));
        for (selector, order, expected) in [
            (
                "menu-item-1",
                ProjectOrder::Name,
                [home, one, two_a, two_b, ten],
            ),
            (
                "menu-item-2",
                ProjectOrder::NameDescending,
                [home, ten, two_b, two_a, one],
            ),
            (
                "menu-item-3",
                ProjectOrder::RecentlyActive,
                [home, two_a, two_b, one, ten],
            ),
            (
                "menu-item-4",
                ProjectOrder::DateAdded,
                [home, ten, two_b, one, two_a],
            ),
            ("menu-item-0", ProjectOrder::Manual, manual),
        ] {
            click("sidebar-project-sort", cx);
            view.read_with(cx, |model, _| {
                let rows = &model.menu_outline()[0];
                for (row, title) in rows.iter().zip([
                    "Manual Order",
                    "Name (A–Z)",
                    "Name (Z–A)",
                    "Recently Active",
                    "Date Added",
                ]) {
                    assert!(row.ends_with(title));
                }
                assert_eq!(rows.len(), 5);
                assert_eq!(rows.iter().filter(|row| row.contains('✓')).count(), 1);
            });
            click(selector, cx);
            view.read_with(cx, |model, _| {
                assert_eq!(model.appearance.sidebar_project_order, order);
                assert_eq!(parent_ids(model), expected);
                assert_eq!(model.navigation_projects(), expected);
                assert_eq!(
                    model
                        .state
                        .projects()
                        .iter()
                        .map(|project| project.id)
                        .collect::<Vec<_>>(),
                    manual
                );
                assert_eq!(
                    Settings::load(&model.path.with_file_name("settings.toml"))
                        .expect("saved settings")
                        .appearance
                        .sidebar_project_order,
                    order
                );
            });
            view.update_in(cx, |model, window, cx| {
                model.toggle_new_tab_list(window, cx);
            });
            cx.run_until_parked();
            let tops: Vec<_> = expected
                .iter()
                .map(|id| {
                    cx.debug_bounds(format!("picker-row-{id}").leak())
                        .expect("project row")
                        .top()
                })
                .collect();
            assert!(tops.windows(2).all(|pair| pair[0] < pair[1]));
            view.update(cx, AppModel::dismiss_overlay);
        }
    }
}

#[gpui::test]
fn project_sort_recent_activity_follows_worktrees_and_keeps_server_groups(cx: &mut TestAppContext) {
    let mut state = AppState::bootstrap().expect("state");
    let local_home = state.home().id;
    let first_server = ServerId::new();
    let second_server = ServerId::new();
    let [first_home, ten, two, child, second_home, other] =
        std::array::from_fn(|_| ProjectId::new());
    state
        .apply_catalog(
            second_server,
            &servers::page(3, second_home, &[(other, "Alpha")]),
        )
        .expect("second server");
    let mut first_page = servers::page(2, first_home, &[(ten, "repo10"), (two, "Repo2")]);
    let mut worktree = servers::descriptor(child, false, "feature");
    worktree.parent_id = Some(ten);
    worktree.kind = Some(murvix_app_core::ProjectKind::Worktree);
    first_page.projects.push(worktree);
    state
        .apply_catalog(first_server, &first_page)
        .expect("first server");
    state.move_project(ten, 4).expect("manual order");
    let (boot, _requests, _remotes) = remote_boot(
        state,
        vec![
            servers::entry(first_server, "First"),
            servers::entry(second_server, "Second"),
        ],
    );
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    view.update(cx, |model, cx| {
        acknowledge_catalog(model, cx);
        model.appearance.sidebar_project_order = ProjectOrder::Name;
        assert_eq!(parent_ids(model), [local_home, two, ten, other]);
        model.appearance.sidebar_project_order = ProjectOrder::RecentlyActive;
        for id in [two, other, child] {
            model.select_project(id, cx);
            model.sync_tab_sidebar(cx);
        }
        assert_eq!(parent_ids(model), [local_home, ten, two, other]);
        model.appearance.layout = AppLayout::TabFocused;
        assert_eq!(
            model
                .sidebar_projects()
                .iter()
                .map(|project| project.id)
                .collect::<Vec<_>>(),
            [local_home, ten, child, two, other]
        );
        model.state.select_remote_servers();
        assert_eq!(parent_ids(model), [first_home, second_home]);
        model.state.select_all_projects();
        for id in [ten, two, child, local_home] {
            model.select_project(id, cx);
            model.sync_tab_sidebar(cx);
        }
        assert_eq!(parent_ids(model), [local_home, ten, two, other]);
        assert!(
            model
                .appearance
                .worktree_recent
                .starts_with(&[local_home, child, two, ten])
        );
        first_page.projects.retain(|project| project.id != child);
        first_page.revision += 1;
        model.receive_catalog(first_server, Ok(first_page), cx);
        assert!(model.state.project(child).is_none());
        assert_eq!(parent_ids(model), [local_home, ten, two, other]);
        model.appearance = Settings::load(&model.path.with_file_name("settings.toml"))
            .expect("saved parent activity")
            .appearance;
        assert_eq!(parent_ids(model), [local_home, ten, two, other]);
    });
}

#[gpui::test]
fn project_sort_date_added_keeps_undated_projects_stable_ahead_of_new_projects(
    cx: &mut TestAppContext,
) {
    let (state, [ten, two_b, one, two_a]) = fixture();
    let mut stored = serde_json::to_value(&state).expect("state JSON");
    for project in stored["projects"].as_array_mut().expect("projects") {
        if [two_a.to_string(), one.to_string()]
            .iter()
            .any(|id| project["id"].as_str() == Some(id))
        {
            project
                .as_object_mut()
                .expect("project")
                .remove("created_at");
        }
    }
    let (boot, _requests) = stub_boot(serde_json::from_value(stored).expect("legacy state"));
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    view.update(cx, |model, _| {
        model.appearance.sidebar_project_order = ProjectOrder::DateAdded;
        assert_eq!(
            parent_ids(model),
            [model.state.home().id, one, two_a, ten, two_b]
        );
    });
}
