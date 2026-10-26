use std::collections::BTreeSet;
use std::error::Error;
use std::fs::{self, File};
use std::io::Read;
use std::os::unix::ffi::OsStrExt;
use std::path::PathBuf;

use murvix_app_core::{
    AppError, AppState, PaneId, ProjectId, ServerId, TabId, WindowBounds, store,
};
use murvix_protocol::SessionId;
use serde_json::{Value, json};

type TestResult = Result<(), Box<dyn Error>>;

struct Fixture(PathBuf);

impl Fixture {
    fn new() -> Result<Self, Box<dyn Error>> {
        let directory = std::env::temp_dir().join(format!("murvix-app-core-{}", ProjectId::new()));
        fs::create_dir(&directory)?;
        Ok(Self(directory))
    }

    fn path(&self) -> PathBuf {
        self.0.join("state.json")
    }

    fn temporary(&self) -> PathBuf {
        self.0.join("state.json.tmp")
    }

    fn write(&self, value: &Value) -> TestResult {
        fs::write(self.path(), serde_json::to_vec(value)?)?;
        Ok(())
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn populated() -> Result<AppState, Box<dyn Error>> {
    let mut state = AppState::bootstrap()?;
    let home = state.home().id;
    let first = state.open_terminal_tab(home)?;
    state.open_terminal_tab(home)?;
    state.select_tab(home, first)?;
    let pane = state.home().tabs[0].panes[0].id;
    state.set_pane_title(pane, "日本語 — editor")?;
    state.set_pane_session(pane, SessionId::new(u64::MAX))?;
    state.set_window_bounds(Some(WindowBounds {
        x: -1400.0,
        y: 40.0,
        width: 1200.0,
        height: 900.0,
    }))?;
    Ok(state)
}

#[test]
fn removed_tab_groups_load_as_ordinary_tabs_without_losing_panes() -> TestResult {
    let fixture = Fixture::new()?;
    let mut state = populated()?;
    let pane = state.home().tabs[0].panes[0].id;
    state.split_pane(pane, murvix_app_core::Direction::Down)?;
    let home = state.home().id;
    let first = state.home().tabs[0].id;
    let second = state.home().tabs[1].id;
    state.select_tab(home, second)?;
    let mut value = serde_json::to_value(&state)?;
    value["window"]["tab_layouts"] = json!({
        home.to_string(): {
            "Split": {
                "axis": "Horizontal",
                "ratio": 0.6,
                "first": {"Group": {"id": TabId::new(), "tabs": [first], "active": first}},
                "second": {"Group": {"id": TabId::new(), "tabs": [second], "active": second}}
            }
        }
    });
    fixture.write(&value)?;
    let restored = store::load(fixture.path())?;
    assert_eq!(restored, state);
    store::save(fixture.path(), &restored)?;
    let saved: Value = serde_json::from_slice(&fs::read(fixture.path())?)?;
    assert!(saved["window"].get("tab_layouts").is_none());
    assert_eq!(store::load(fixture.path())?, state);
    Ok(())
}
#[test]
fn non_utf8_workspace_roots_survive_a_save_and_reload() -> TestResult {
    use std::os::unix::ffi::OsStringExt;
    let fixture = Fixture::new()?;
    let primary = fixture.0.join("primary");
    let extra = fixture.0.join("extra");
    fs::create_dir_all(&primary)?;
    fs::create_dir_all(&extra)?;
    let mut state = AppState::bootstrap()?;
    let id = state.add_project(ServerId::local(), primary.clone())?;
    state.set_project_extra_directories(id, vec![extra.clone()])?;
    // A non-UTF8 root must serialize as bytes, not fail or lossy text.
    let mut value = serde_json::to_value(&state)?;
    let projects = value["projects"].as_array_mut().ok_or("projects")?;
    let entry = projects
        .iter_mut()
        .find(|project| project["id"] == id.to_string())
        .ok_or("project")?;
    entry["extra_directories"] =
        json!([[47, 115, 114, 118, 47, 255, 58, 119, 101, 10, 105, 114, 100]]);
    fixture.write(&value)?;
    let restored = store::load(fixture.path())?;
    assert_eq!(
        restored.project(id).ok_or("project")?.extra_directories,
        vec![PathBuf::from(std::ffi::OsString::from_vec(
            b"/srv/\xff:we\nird".to_vec()
        ))]
    );
    // Text arrays from older backups still read.
    let mut text = serde_json::to_value(&state)?;
    text["projects"]
        .as_array_mut()
        .ok_or("projects")?
        .iter_mut()
        .find(|project| project["id"] == id.to_string())
        .ok_or("project")?["extra_directories"] = json!([extra.to_string_lossy()]);
    fixture.write(&text)?;
    let restored = store::load(fixture.path())?;
    assert_eq!(
        restored.project(id).ok_or("project")?.extra_directories,
        vec![extra.clone()]
    );
    // And the live state (UTF-8 extras) round-trips byte-exact.
    store::save(fixture.path(), &state)?;
    let saved: Value = serde_json::from_slice(&fs::read(fixture.path())?)?;
    let saved_extras = saved["projects"]
        .as_array()
        .ok_or("projects")?
        .iter()
        .find(|project| project["id"] == id.to_string())
        .ok_or("project")?["extra_directories"]
        .clone();
    assert_eq!(saved_extras, json!([extra.as_os_str().as_bytes().to_vec()]));
    assert_eq!(store::load(fixture.path())?, state);
    Ok(())
}

#[test]
fn removed_workspaces_load_as_a_plain_project_list() -> TestResult {
    let fixture = Fixture::new()?;
    let state = populated()?;
    let mut value = serde_json::to_value(&state)?;
    value["window"]["workspace"] = json!("c10c95a4df8c47afae2878ddf1a2a4e7");
    value["workspaces"] = json!([{
        "id": "c10c95a4df8c47afae2878ddf1a2a4e7",
        "name": "Work",
        "projects": [state.home().id.to_string()],
    }]);
    fixture.write(&value)?;
    let restored = store::load(fixture.path())?;
    assert_eq!(restored, state);
    store::save(fixture.path(), &restored)?;
    let saved: Value = serde_json::from_slice(&fs::read(fixture.path())?)?;
    assert!(saved.get("workspaces").is_none());
    assert!(saved["window"].get("workspace").is_none());
    assert_eq!(store::load(fixture.path())?, state);
    Ok(())
}

#[test]
fn save_replaces_file_atomically_and_reuses_abandoned_temporary_file() -> TestResult {
    let fixture = Fixture::new()?;
    let mut state = populated()?;
    store::save(fixture.path(), &state)?;
    let old_bytes = fs::read_to_string(fixture.path())?;
    let mut old_file = File::open(fixture.path())?;
    let home = state.home().id;
    state.open_terminal_tab(home)?;
    fs::write(fixture.temporary(), "interrupted write")?;
    store::save(fixture.path(), &state)?;
    let mut still_old = String::new();
    old_file.read_to_string(&mut still_old)?;
    assert_eq!(still_old, old_bytes);
    assert_ne!(fs::read_to_string(fixture.path())?, old_bytes);
    assert_eq!(store::load(fixture.path())?, state);
    assert!(!fixture.temporary().exists());
    Ok(())
}

#[test]
fn corrupt_json_is_an_error_naming_the_path_and_is_not_overwritten() -> TestResult {
    let fixture = Fixture::new()?;
    fs::write(fixture.path(), b"{ broken json")?;
    let error = store::load(fixture.path())
        .err()
        .ok_or("corruption accepted")?;
    assert!(matches!(error, AppError::Json { .. }));
    assert!(
        error
            .to_string()
            .contains(&fixture.path().display().to_string())
    );
    assert!(error.source().is_some());
    assert_eq!(fs::read(fixture.path())?, b"{ broken json");
    Ok(())
}

#[test]
fn malformed_domain_state_is_rejected_with_the_path() -> TestResult {
    let fixture = Fixture::new()?;
    let valid = serde_json::to_value(populated()?)?;
    let mut invalid_states = Vec::new();
    for version in [0, 4] {
        let mut value = valid.clone();
        value["version"] = json!(version);
        invalid_states.push(value);
    }
    for panes in [
        json!([]),
        json!(vec![valid["projects"][0]["tabs"][0]["panes"][0].clone(); 2]),
    ] {
        let mut value = valid.clone();
        value["projects"][0]["tabs"][0]["panes"] = panes;
        invalid_states.push(value);
    }
    for (pointer, replacement) in [
        ("/window/active_pane", json!(PaneId::new())),
        (
            "/projects/0/tabs/0/id",
            valid["projects"][0]["tabs"][1]["id"].clone(),
        ),
        (
            "/projects/0/tabs/0/panes/0/content",
            json!({"type": "unknown"}),
        ),
        ("/projects/0/tabs/0/panes/0/content/session", json!(0)),
        ("/projects/0/id", json!("invalid")),
        ("/projects/0/color", json!("blue")),
        ("/window/bounds/width", json!(-1)),
    ] {
        let mut value = valid.clone();
        *value.pointer_mut(pointer).ok_or("missing fixture field")? = replacement;
        invalid_states.push(value);
    }
    let mut duplicate_pane = valid.clone();
    let pane = duplicate_pane["projects"][0]["tabs"][0]["panes"][0]["id"].clone();
    duplicate_pane["projects"][0]["tabs"][1]["active_pane"] = pane.clone();
    duplicate_pane["projects"][0]["tabs"][1]["panes"][0]["id"] = pane;
    invalid_states.push(duplicate_pane);
    for value in invalid_states {
        fixture.write(&value)?;
        let error = store::load(fixture.path())
            .err()
            .ok_or_else(|| format!("invalid state accepted: {value}"))?;
        assert!(
            error
                .to_string()
                .contains(&fixture.path().display().to_string())
        );
        assert_eq!(
            serde_json::from_slice::<Value>(&fs::read(fixture.path())?)?,
            value
        );
    }
    Ok(())
}

#[test]
fn overlapping_save_preserves_the_active_writer_and_previous_state() -> TestResult {
    let fixture = Fixture::new()?;
    let state = populated()?;
    store::save(fixture.path(), &state)?;
    let old_bytes = fs::read(fixture.path())?;
    let active_writer = File::create(fixture.temporary())?;
    active_writer.lock()?;
    fs::write(fixture.temporary(), "active write")?;
    let error = store::save(fixture.path(), &state)
        .err()
        .ok_or("overlapping save was accepted")?;
    assert!(
        matches!(error, AppError::Io { source, .. } if source.kind() == std::io::ErrorKind::WouldBlock)
    );
    assert_eq!(fs::read(fixture.temporary())?, b"active write");
    assert_eq!(fs::read(fixture.path())?, old_bytes);
    drop(active_writer);
    store::save(fixture.path(), &state)?;
    assert_eq!(store::load(fixture.path())?, state);
    Ok(())
}

#[test]
fn older_files_keep_everything_and_move_server_state_to_the_local_entry() -> TestResult {
    let fixture = Fixture::new()?;
    for version in [1, 2] {
        let mut legacy: Value =
            serde_json::from_str(include_str!("fixtures/desktop-state-v2.json"))?;
        legacy["version"] = json!(version);
        fixture.write(&legacy)?;
        let state = store::load(fixture.path())?;
        assert_eq!(state.version(), 3);
        store::save(fixture.path(), &state)?;
        let saved: Value = serde_json::from_slice(&fs::read(fixture.path())?)?;

        let keys: BTreeSet<_> = saved
            .as_object()
            .ok_or("state object")?
            .keys()
            .map(String::as_str)
            .collect();
        assert_eq!(
            keys,
            BTreeSet::from([
                "projects",
                "quick_terminal",
                "servers",
                "starting_directories",
                "startup_commands",
                "version",
                "window",
            ])
        );
        assert_eq!(saved["version"], 3);
        let servers = saved["servers"].as_object().ok_or("servers")?;
        assert_eq!(servers.len(), 1);
        let local = &servers[&ServerId::local().to_string()];
        for (old, new) in [
            ("catalog_server", "identity"),
            ("catalog_revision", "catalog_revision"),
            ("project_intents", "project_intents"),
            ("pending_cancellations", "pending_cancellations"),
            ("pending_discards", "pending_discards"),
            ("close_operations", "close_operations"),
        ] {
            assert!(!legacy[old].is_null() && legacy[old] != json!([]) && legacy[old] != json!({}));
            assert_eq!(local[new], legacy[old], "{old}");
        }
        let mut expected = legacy.clone();
        let home = std::env::home_dir().ok_or("home directory")?;
        expected["projects"][0]["directory"] = json!(home.as_os_str().as_bytes());
        // Workspaces were removed; their saved fields are ignored.
        expected["window"]
            .as_object_mut()
            .ok_or("window")?
            .remove("workspace");
        for key in [
            "projects",
            "quick_terminal",
            "starting_directories",
            "startup_commands",
            "window",
        ] {
            assert_eq!(saved[key], expected[key], "{key}");
        }

        let reloaded = store::load(fixture.path())?;
        assert_eq!(reloaded, state);
        store::save(fixture.path(), &reloaded)?;
        assert_eq!(
            serde_json::from_slice::<Value>(&fs::read(fixture.path())?)?,
            saved
        );
    }
    Ok(())
}
