use std::ffi::OsString;
use std::fs;
use std::os::unix::ffi::{OsStrExt, OsStringExt};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

use murvix_app_core::{AppState, ServerId};
use murvix_client::Client;
use murvix_protocol::{
    OperationId, ProjectDescriptor, ProjectId, ProjectIntent, ProjectMutation, ServerPath,
};

use super::fixture::{Fixture, Result, Tui};

fn server_path(path: &Path) -> ServerPath {
    ServerPath(path.as_os_str().as_bytes().to_vec())
}

fn roots(client: &Client, project: ProjectId) -> Result<Vec<ServerPath>> {
    Ok(client
        .catalog()?
        .projects
        .into_iter()
        .find(|candidate| candidate.id == project)
        .ok_or("workspace project")?
        .extra_directories)
}

fn folder(fixture: &Fixture, name: &str) -> Result<PathBuf> {
    let path = fixture.directory.path().join(name);
    fs::create_dir(&path)?;
    Ok(path)
}

fn project(fixture: &Fixture, tui: &mut Tui<'_>) -> Result<(Client, ProjectId, PathBuf)> {
    let client = fixture.client()?;
    let directory = folder(fixture, "primary")?;
    let mut descriptor =
        ProjectDescriptor::new(server_path(&directory), std::iter::empty::<&str>());
    descriptor.name = "Workspace regression".into();
    let project = descriptor.id;
    client.mutate_project(ProjectIntent {
        operation: OperationId::new(),
        mutation: ProjectMutation::Create(descriptor),
    })?;
    let (column, row) = tui.locate("Workspace regression")?;
    tui.click(column, row)?;
    tui.wait(|tui| {
        Ok(fixture.state()?["active"] == project.to_string()
            && tui.active_tab()?["panes"].as_object().is_some_and(|panes| {
                !panes.is_empty() && panes.values().all(|pane| !pane["session"].is_null())
            }))
    })?;
    tui.ready()?;
    Ok((client, project, directory))
}

fn add(tui: &mut Tui<'_>, path: &Path) -> Result {
    tui.write(b"\x02a")?;
    tui.output("Add folder to workspace")?;
    tui.write(path.as_os_str().as_bytes())?;
    tui.write(b"\r")?;
    tui.wait(|tui| {
        Ok(!tui
            .text()?
            .iter()
            .any(|row| row.contains("Enter an absolute path on the server.")))
    })
}

fn wait_roots(
    tui: &mut Tui<'_>,
    client: &Client,
    project: ProjectId,
    expected: &[ServerPath],
) -> Result {
    tui.wait(|_| Ok(roots(client, project)? == expected))
}

/// The sidebar card's folder rows, so manager input never races a stale frame.
fn wait_sidebar_folders(tui: &mut Tui<'_>, count: usize) -> Result {
    tui.wait(|tui| Ok(tui.text()?.iter().filter(|row| row.contains('▸')).count() == count))
}

/// Opens the manager, runs `keys`, and waits for `expected`. Retries once: a
/// loaded runner can process dialog keys against a settling frame, swallowing
/// the selection without an error.
fn manage(
    tui: &mut Tui<'_>,
    client: &Client,
    project: ProjectId,
    keys: &[u8],
    expected: &[ServerPath],
) -> Result {
    let mut attempts = 0;
    loop {
        wait_sidebar_folders(tui, roots(client, project)?.len())?;
        tui.write(b"\x02W")?;
        tui.output("Workspace folders")?;
        tui.write(keys)?;
        if wait_roots(tui, client, project, expected).is_ok() {
            return Ok(());
        }
        attempts += 1;
        if attempts > 1 {
            return wait_roots(tui, client, project, expected);
        }
        tui.write(b"\x1b")?;
    }
}

fn desktop_edit(
    desktop: &mut AppState,
    client: &Client,
    project: ProjectId,
    paths: Vec<PathBuf>,
) -> Result {
    desktop.set_project_extra_directories(project, paths)?;
    let intent = desktop
        .project_intents(ServerId::local())
        .first()
        .cloned()
        .ok_or("desktop workspace intent")?;
    client.mutate_project(intent.clone())?;
    desktop.complete_project_intent(ServerId::local(), intent.operation)?;
    desktop.apply_catalog(ServerId::local(), &client.catalog()?)?;
    Ok(())
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "One cross-client scenario verifies mutations, validation, and durable server ownership"
)]
fn tui_workspace_mutations_share_desktop_and_cli_catalog_and_survive_server_restart() -> Result {
    let fixture = Fixture::new()?;
    let mut tui = Tui::start(&fixture, &[])?;
    tui.ready()?;
    let (client, project, primary) = project(&fixture, &mut tui)?;
    let initial_layout = fixture.state()?;
    let first = folder(&fixture, "from-tui")?;
    let desktop_folder = folder(&fixture, "from-desktop")?;
    let cli_folder = folder(&fixture, "from-cli")?;
    let persisted = folder(&fixture, "persisted-root")?;

    add(&mut tui, &first)?;
    wait_roots(&mut tui, &client, project, &[server_path(&first)])?;
    tui.output("from-tui")?;
    let revision = client.catalog()?.revision;
    add(&mut tui, &first)?;
    // Even a duplicate is an acknowledged server mutation. Wait for its
    // catalog refresh, rather than passing against the pre-action roots.
    tui.wait(|_| {
        let state: serde_json::Value =
            serde_json::from_slice(&fs::read(fixture.directory.path().join("tui-state.json"))?)?;
        Ok(state["catalog_revision"]
            .as_u64()
            .is_some_and(|next| next > revision))
    })?;
    assert!(
        !tui.text()?
            .iter()
            .any(|row| row.contains("workspace folder does not exist"))
    );
    wait_roots(&mut tui, &client, project, &[server_path(&first)])?;

    let mut desktop = AppState::bootstrap()?;
    desktop.apply_catalog(ServerId::local(), &client.catalog()?)?;
    assert_eq!(
        desktop
            .project(project)
            .ok_or("desktop project")?
            .extra_directories,
        vec![first.clone()]
    );
    desktop_edit(
        &mut desktop,
        &client,
        project,
        vec![first.clone(), desktop_folder.clone()],
    )?;
    tui.output("from-desktop")?;

    let output = Command::new(super::support::binary())
        .args(["project", "workspace", &project.to_string()])
        .args([&first, &desktop_folder, &cli_folder])
        .envs(fixture.environment())
        .current_dir(fixture.directory.path())
        .output()?;
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    tui.output("from-cli")?;
    add(&mut tui, &persisted)?;
    wait_roots(
        &mut tui,
        &client,
        project,
        &[
            server_path(&first),
            server_path(&desktop_folder),
            server_path(&cli_folder),
            server_path(&persisted),
        ],
    )?;
    desktop.apply_catalog(ServerId::local(), &client.catalog()?)?;
    assert_eq!(
        desktop
            .project(project)
            .ok_or("desktop project")?
            .extra_directories,
        vec![
            first.clone(),
            desktop_folder.clone(),
            cli_folder.clone(),
            persisted.clone()
        ]
    );

    // Primary is read-only item zero; remove the second extra.
    manage(
        &mut tui,
        &client,
        project,
        b"jj\r",
        &[
            server_path(&first),
            server_path(&cli_folder),
            server_path(&persisted),
        ],
    )?;
    tui.write(b"\x02W")?;
    tui.output("Workspace folders")?;
    tui.write(b"C")?;
    tui.output("Clear workspace folders?")?;
    tui.write(b"y")?;
    wait_roots(&mut tui, &client, project, &[])?;

    add(&mut tui, &fixture.directory.path().join("missing-folder"))?;
    tui.output("workspace folder does not exist")?;
    assert!(roots(&client, project)?.is_empty());
    add(&mut tui, &primary)?;
    tui.output("workspace folder does not exist")?;
    assert!(roots(&client, project)?.is_empty());
    add(&mut tui, &persisted)?;
    wait_roots(&mut tui, &client, project, &[server_path(&persisted)])?;
    tui.output("persisted-root")?;
    assert_eq!(
        fixture.state()?,
        initial_layout,
        "folder edits must not mutate the terminal layout"
    );
    assert!(!serde_json::to_string(&fixture.state()?)?.contains("extra_directories"));
    tui.detach()?;
    client.stop_server()?;
    let socket = fixture.directory.path().join("server.sock");
    let deadline = Instant::now() + Duration::from_secs(5);
    while socket.exists() && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(10));
    }
    assert!(!socket.exists(), "server did not stop");
    let mut restarted = Tui::start(&fixture, &[])?;
    restarted.output("persisted-root")?;
    let client = fixture.client()?;
    assert_eq!(roots(&client, project)?, vec![server_path(&persisted)]);
    let mut desktop = AppState::bootstrap()?;
    desktop.apply_catalog(ServerId::local(), &client.catalog()?)?;
    assert_eq!(
        desktop
            .project(project)
            .ok_or("desktop project")?
            .extra_directories,
        vec![persisted]
    );
    restarted.detach()?;
    Ok(())
}

#[test]
fn tui_removes_the_selected_non_utf8_root_without_lossy_path_aliasing() -> Result {
    let fixture = Fixture::new()?;
    let mut tui = Tui::start(&fixture, &[])?;
    tui.ready()?;
    let (client, project, _) = project(&fixture, &mut tui)?;
    // APFS rejects non-UTF-8 names; control bytes are distinct on every
    // filesystem yet render identically once the UI sanitizes them.
    let first = fixture.directory.path().join(if cfg!(target_os = "macos") {
        OsString::from("same-\u{1}A")
    } else {
        OsString::from_vec(b"same-\xff".to_vec())
    });
    let second = fixture.directory.path().join(if cfg!(target_os = "macos") {
        OsString::from("same-\u{2}A")
    } else {
        OsString::from_vec(b"same-\xfe".to_vec())
    });
    fs::create_dir(&first)?;
    fs::create_dir(&second)?;
    assert_eq!(first.to_string_lossy(), second.to_string_lossy());
    let mut desktop = AppState::bootstrap()?;
    desktop.apply_catalog(ServerId::local(), &client.catalog()?)?;
    desktop_edit(
        &mut desktop,
        &client,
        project,
        vec![first.clone(), second.clone()],
    )?;
    tui.output(if cfg!(target_os = "macos") {
        "same-A"
    } else {
        "same-�"
    })?;
    wait_sidebar_folders(&mut tui, 2)?;
    tui.write(b"\x02W")?;
    tui.output("Workspace folders")?;
    tui.write(b"jj\r")?;
    wait_roots(&mut tui, &client, project, &[server_path(&first)])?;
    desktop.apply_catalog(ServerId::local(), &client.catalog()?)?;
    assert_eq!(
        desktop
            .project(project)
            .ok_or("desktop project")?
            .extra_directories,
        vec![first]
    );
    tui.write(b"\x02W")?;
    tui.output("Workspace folders")?;
    tui.write(b"j\r")?;
    wait_roots(&mut tui, &client, project, &[])?;
    tui.detach()?;
    Ok(())
}
