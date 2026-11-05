//! `tab.*` and `pane.*` requests from `murvix tab` and `murvix pane`:
//! terminal layout in this app, over the same socket `murvix browser` uses.
//! Like browser tabs, layout lives in this app, not the server, so these
//! commands are local-only and need the desktop app running.

use std::collections::BTreeSet;

use gpui::{Context, Task};
use murvix_app_core::{Direction, PaneId, TabId};
use serde_json::{Map, Value, json};

use super::AppModel;

type Reply = Task<Result<Value, String>>;

fn done(value: Value) -> Reply {
    Task::ready(Ok(value))
}

fn failed(error: impl Into<String>) -> Reply {
    Task::ready(Err(error.into()))
}

fn result(result: Result<Value, String>) -> Reply {
    match result {
        Ok(value) => done(value),
        Err(error) => failed(error),
    }
}

impl AppModel {
    /// Carries out one `tab.<verb>` or `pane.<verb>` request.
    pub(crate) fn layout_call(
        &mut self,
        verb: &str,
        args: &Map<String, Value>,
        cx: &mut Context<Self>,
    ) -> Reply {
        let args = Value::Object(args.clone());
        match verb {
            "tab.list" => result(self.tabs_call("tabs.list", &args, cx)),
            "tab.switch" => {
                let identifier = args
                    .get("identifier")
                    .and_then(Value::as_str)
                    .unwrap_or_default();
                result(self.tabs_call("tabs.switch", &json!({"identifier": identifier}), cx))
            }
            "tab.new" => result(self.tabs_call("tabs.new", &args, cx)),
            "tab.next" => result(self.tabs_call("tabs.next", &args, cx)),
            "tab.previous" => result(self.tabs_call("tabs.previous", &args, cx)),
            "tab.open" => result(self.tabs_call("tabs.open", &args, cx)),
            "tab.close" => {
                let tab = match self.layout_tab(args.get("identifier").and_then(Value::as_str)) {
                    Ok(tab) => tab,
                    Err(error) => return failed(error),
                };
                self.close_tab(tab, cx);
                done(Value::Null)
            }
            "pane.list" => result(self.panes_call("panes.list", &args, cx)),
            "pane.split" => self.layout_split_pane(&args, cx),
            "pane.focus" => {
                let pane = match self
                    .layout_terminal_pane(args.get("pane").and_then(Value::as_str), true)
                {
                    Ok(pane) => pane,
                    Err(error) => return failed(error),
                };
                self.focus_pane(pane, cx);
                done(Value::Null)
            }
            "pane.close" => {
                let pane = match self
                    .layout_terminal_pane(args.get("pane").and_then(Value::as_str), false)
                {
                    Ok(pane) => pane,
                    Err(error) => return failed(error),
                };
                self.close_pane(pane, cx);
                done(Value::Null)
            }
            "pane.send" | "pane.sendKeys" | "pane.readScreen" | "pane.rename" => {
                let verb = match verb {
                    "pane.send" => "panes.send",
                    "pane.sendKeys" => "panes.sendKeys",
                    "pane.readScreen" => "panes.readScreen",
                    _ => "panes.rename",
                };
                // Extension pane verbs take `paneID`; the CLI sends `pane`,
                // defaulting to the active pane.
                let mut args = args;
                match self.layout_terminal_pane(args.get("pane").and_then(Value::as_str), false) {
                    Ok(pane) => args["paneID"] = json!(pane.to_string()),
                    Err(error) => return failed(error),
                }
                result(self.panes_call(verb, &args, cx))
            }
            _ => failed(format!("unknown command {verb}")),
        }
    }

    fn layout_split_pane(&mut self, args: &Value, cx: &mut Context<Self>) -> Reply {
        let source = match self.layout_terminal_pane(args.get("pane").and_then(Value::as_str), true)
        {
            Ok(pane) => pane,
            Err(error) => return failed(error),
        };
        let edge = match args.get("direction").and_then(Value::as_str) {
            Some("left") => Direction::Left,
            Some("up") => Direction::Up,
            Some("down") => Direction::Down,
            None | Some("right") => Direction::Right,
            Some(other) => {
                return failed(format!(
                    "unknown direction '{other}'; use left, right, up, or down"
                ));
            }
        };
        if self.active_pane() != Some(source) {
            self.focus_pane(source, cx);
        }
        let before: BTreeSet<PaneId> = self
            .state
            .projects()
            .iter()
            .flat_map(|project| &project.tabs)
            .flat_map(|tab| &tab.panes)
            .map(|pane| pane.id)
            .collect();
        self.split_pane(edge, cx);
        self.state
            .projects()
            .iter()
            .flat_map(|project| &project.tabs)
            .flat_map(|tab| &tab.panes)
            .map(|pane| pane.id)
            .find(|pane| !before.contains(pane))
            .map_or_else(
                || failed("could not split pane"),
                |pane| done(json!(pane.to_string())),
            )
    }

    /// Resolves a tab identifier: index, ID, or empty for the active tab.
    fn layout_tab(&self, identifier: Option<&str>) -> Result<TabId, String> {
        let project = self.state.current_project();
        let identifier = identifier.unwrap_or("").trim();
        if identifier.is_empty() {
            return self.active_tab().ok_or_else(|| "no active tab".to_owned());
        }
        if let Ok(index) = identifier.parse::<usize>()
            && let Some(tab) = project.tabs.get(index)
        {
            return Ok(tab.id);
        }
        project
            .tabs
            .iter()
            .find(|tab| {
                tab.id.to_string() == identifier
                    || tab
                        .panes
                        .iter()
                        .any(|pane| pane.id.to_string() == identifier)
            })
            .map(|tab| tab.id)
            .ok_or_else(|| format!("tab not found {identifier}"))
    }

    /// Resolves a terminal pane identifier, or the active pane when omitted.
    ///
    /// Split and focus only work for panes visible in the current layout.
    fn layout_terminal_pane(
        &self,
        identifier: Option<&str>,
        visible: bool,
    ) -> Result<PaneId, String> {
        let identifier = identifier.unwrap_or("").trim();
        let pane = if identifier.is_empty() {
            self.active_pane()
                .ok_or_else(|| "no active pane".to_owned())?
        } else {
            identifier
                .parse()
                .map_err(|_| "invalid pane ID".to_owned())?
        };
        let terminal = self
            .state
            .projects()
            .iter()
            .flat_map(|project| &project.tabs)
            .flat_map(|tab| &tab.panes)
            .any(|candidate| {
                candidate.id == pane
                    && matches!(
                        candidate.content,
                        murvix_app_core::PaneContent::Terminal { .. }
                    )
            });
        if !terminal {
            return Err(format!("terminal pane not found {pane}"));
        }
        if visible && !self.visible_panes().contains(&pane) {
            return Err(format!("terminal pane is not visible {pane}"));
        }
        Ok(pane)
    }
}
