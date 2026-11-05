use std::cell::RefCell;

use super::*;
use murvix_app_core::{PaneId, browser::BrowserPane};
use serde_json::{Value, json};

/// Sends a `tab.*` or `pane.*` request as the CLI does and waits for the reply.
fn call(
    view: &Entity<AppModel>,
    cx: &mut VisualTestContext,
    verb: &str,
    args: &Value,
) -> std::result::Result<Value, String> {
    let args = args.as_object().cloned().unwrap_or_default();
    let reply = view.update(cx, |model, cx| model.layout_call(verb, &args, cx));
    let result = Rc::new(RefCell::new(None));
    let slot = Rc::clone(&result);
    cx.update(|_, cx| {
        cx.spawn(async move |_| {
            *slot.borrow_mut() = Some(reply.await);
        })
        .detach();
    });
    cx.run_until_parked();
    result.borrow_mut().take().expect("reply")
}

fn pane_ids(model: &AppModel) -> Vec<PaneId> {
    model
        .state
        .projects()
        .iter()
        .flat_map(|project| &project.tabs)
        .flat_map(|tab| &tab.panes)
        .map(|pane| pane.id)
        .collect()
}

#[gpui::test]
fn layout_commands_reject_unknown_panes_without_changing_the_active_layout(
    cx: &mut TestAppContext,
) {
    let state = AppState::bootstrap().expect("state");
    let (boot, _requests) = stub_boot(state);
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    let unknown = PaneId::new();
    let before = view.read_with(cx, |model, _| pane_ids(model));

    for (verb, args) in [
        ("pane.focus", json!({"pane": unknown})),
        ("pane.close", json!({"pane": unknown})),
        ("pane.split", json!({"pane": unknown})),
    ] {
        assert_eq!(
            call(&view, cx, verb, &args),
            Err(format!("terminal pane not found {unknown}")),
            "{verb}"
        );
    }

    assert_eq!(view.read_with(cx, |model, _| pane_ids(model)), before);
}

#[gpui::test]
fn layout_commands_reject_non_terminal_panes(cx: &mut TestAppContext) {
    let mut state = AppState::bootstrap().expect("state");
    let (_, browser) = state
        .open_browser(state.home().id, BrowserPane::default(), false)
        .expect("browser tab");
    let (boot, _requests) = stub_boot(state);
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));

    for (verb, args) in [
        ("pane.focus", json!({"pane": browser})),
        ("pane.close", json!({"pane": browser})),
        ("pane.split", json!({"pane": browser})),
        ("pane.send", json!({"pane": browser, "text": "echo no"})),
        ("pane.sendKeys", json!({"pane": browser, "key": "Enter"})),
        ("pane.readScreen", json!({"pane": browser, "lines": 1})),
        ("pane.rename", json!({"pane": browser, "title": "no"})),
    ] {
        assert_eq!(
            call(&view, cx, verb, &args),
            Err(format!("terminal pane not found {browser}")),
            "{verb}"
        );
    }
    assert!(view.read_with(cx, |model, _| model.state.browser(browser).is_some()));
}
