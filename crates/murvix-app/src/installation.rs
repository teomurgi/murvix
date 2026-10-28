#[cfg(test)]
mod tests;

use std::io::{self, Write};
use std::path::Path;

use gpui::{
    Context, InteractiveElement, IntoElement, ParentElement, StatefulInteractiveElement, Styled,
    div, px,
};
use murvix_ui::l10n::{tr_key, translate};
use murvix_ui::{controls, tr};

use crate::model::AppModel;

#[derive(Clone)]
pub(crate) struct Preferences {
    pub(crate) automatic_downloads: bool,
    pub(crate) cli_enabled: bool,
    pub(crate) shell: bool,
}

#[derive(Clone)]
pub(crate) struct Onboarding {
    pub(crate) visible: bool,
    pub(crate) login: bool,
    completed: bool,
}

#[derive(Clone)]
pub(crate) struct State {
    pub(crate) preferences: Preferences,
    pub(crate) onboarding: Onboarding,
    pub(crate) busy: bool,
    pub(crate) health: String,
    pub(crate) login_status: String,
    pub(crate) shell_disclosure: String,
    pub(crate) installed_version: Option<String>,
    problem: Option<String>,
}

impl State {
    pub(crate) fn load(path: &Path) -> Self {
        let mut state = Self {
            preferences: Preferences {
                automatic_downloads: false,
                cli_enabled: true,
                shell: true,
            },
            onboarding: Onboarding {
                visible: false,
                login: false,
                completed: false,
            },
            busy: false,
            health: String::new(),
            login_status: String::new(),
            shell_disclosure: String::new(),
            installed_version: None,
            problem: None,
        };
        match std::fs::read(path) {
            Ok(bytes) => match serde_json::from_slice::<serde_json::Value>(&bytes) {
                Ok(value) => {
                    state.onboarding.completed = value["completed"].as_bool().unwrap_or(false);
                    state.preferences.cli_enabled = value["cli_enabled"].as_bool().unwrap_or(false);
                    state.preferences.shell = value["shell"].as_bool().unwrap_or(false);
                    state.preferences.automatic_downloads =
                        value["automatic_downloads"].as_bool().unwrap_or(false);
                }
                Err(error) => {
                    state.problem =
                        Some(format!("Could not read installation preferences: {error}"));
                }
            },
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => {
                state.problem = Some(format!("Could not read installation preferences: {error}"));
            }
        }
        // Development binaries and tests are not installations. Bundled launches,
        // including DMG/translocated launches, still show move-and-reopen guidance.
        state.onboarding.visible = cfg!(all(target_os = "macos", not(test)))
            && !state.onboarding.completed
            && murvix_core::executable::current_path().is_ok_and(|path| {
                path.ancestors()
                    .any(|part| part.extension().is_some_and(|ext| ext == "app"))
            });
        state
    }

    fn save(&self, path: &Path) -> io::Result<()> {
        let parent = path
            .parent()
            .ok_or_else(|| io::Error::other("Missing profile directory"))?;
        std::fs::create_dir_all(parent)?;
        let mut file = tempfile::NamedTempFile::new_in(parent)?;
        serde_json::to_writer_pretty(
            &mut file,
            &serde_json::json!({
                "completed": self.onboarding.completed,
                "cli_enabled": self.preferences.cli_enabled,
                "shell": self.preferences.shell,
                "automatic_downloads": self.preferences.automatic_downloads,
            }),
        )?;
        file.write_all(b"\n")?;
        file.as_file().sync_all()?;
        file.persist(path).map_err(|error| error.error)?;
        Ok(())
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) enum Action {
    Refresh,
    RepairCli,
    ConfigureShell,
    Login(bool),
    LoginSettings,
    AutomaticDownloads(bool),
    CheckUpdates,
}

impl AppModel {
    pub(crate) fn remember_cli_enabled(&mut self, cx: &mut Context<Self>) {
        let mut next = self.installation.clone();
        next.preferences.cli_enabled = true;
        match next.save(&self.configuration_path("installation.json")) {
            Ok(()) => self.installation.preferences.cli_enabled = true,
            Err(error) => self.fail(error.to_string(), cx),
        }
        self.sync_preferences(cx);
    }

    pub(crate) fn start_installation(&mut self, cx: &mut Context<Self>) {
        if cfg!(test) {
            return;
        }
        if self.installation.onboarding.completed && self.installation.preferences.cli_enabled {
            self.installation_action_inner(Action::RepairCli, true, cx);
        } else {
            self.installation_action(Action::Refresh, cx);
        }
    }

    pub(crate) fn installation_action(&mut self, action: Action, cx: &mut Context<Self>) {
        if matches!(action, Action::ConfigureShell) {
            let window = self.window;
            let disclosure = self.installation.shell_disclosure.clone();
            cx.spawn(async move |model, cx| {
                let response = window.update(cx, |_, window, cx| {
                    window.prompt(
                        gpui::PromptLevel::Info,
                        &tr!("Configure shell PATH?"),
                        Some(&disclosure),
                        &[tr!("Configure").as_ref(), tr!("Keep manual setup").as_ref()],
                        cx,
                    )
                });
                if let Ok(response) = response
                    && response.await.is_ok_and(|answer| answer == 0)
                {
                    let _ = model.update(cx, |model, cx| {
                        model.installation_action_inner(action, false, cx);
                    });
                }
            })
            .detach();
        } else {
            self.installation_action_inner(action, false, cx);
        }
    }

    fn installation_action_inner(
        &mut self,
        action: Action,
        owned_only: bool,
        cx: &mut Context<Self>,
    ) {
        if self.installation.busy {
            return;
        }
        if let Action::AutomaticDownloads(enabled) = action {
            let mut next = self.installation.clone();
            next.preferences.automatic_downloads = enabled;
            match next.save(&self.configuration_path("installation.json")) {
                Ok(()) => self.installation = next,
                Err(error) => self.installation.problem = Some(error.to_string()),
            }
            self.sync_preferences(cx);
            cx.notify();
            return;
        }
        if matches!(action, Action::CheckUpdates) {
            self.check_for_updates(true, cx);
            return;
        }
        self.installation.busy = true;
        let path = self.configuration_path("installation.json");
        let mut next = self.installation.clone();
        // SMAppService registration runs on the UI thread. Never re-register
        // at launch: native approval/revocation remains authoritative.
        let native = match action {
            Action::Login(enabled) => crate::login::set_enabled(enabled),
            Action::LoginSettings => crate::login::open_settings(),
            _ => Ok(()),
        };
        let login_status = crate::login::status();
        cx.spawn(async move |model, cx| {
            let result =
                cx.background_executor()
                    .spawn(async move {
                        let result = native.and_then(|()| match action {
                            Action::RepairCli if owned_only => crate::cli_install::repair_owned()
                                .map_err(|error| error.to_string()),
                            Action::RepairCli => crate::cli_install::install()
                                .map(|_| {
                                    next.preferences.cli_enabled = true;
                                })
                                .map_err(|error| error.to_string()),
                            Action::ConfigureShell => crate::cli_install::configure_shell()
                                .map(|()| {
                                    next.preferences.shell = true;
                                })
                                .map_err(|error| error.to_string()),
                            _ => Ok(()),
                        });
                        if result.is_ok()
                            && matches!(action, Action::RepairCli | Action::ConfigureShell)
                            && !owned_only
                        {
                            next.save(&path).map_err(|error| error.to_string())?;
                        }
                        next.health = crate::cli_install::integration_status()
                            .unwrap_or_else(|error| error.to_string());
                        next.shell_disclosure = crate::cli_install::shell_disclosure();
                        next.login_status = login_status;
                        next.installed_version = crate::updater::installed_app_version().ok();
                        next.problem = result.err();
                        next.busy = false;
                        Ok::<_, String>(next)
                    })
                    .await;
            let _ = model.update(cx, |model, cx| {
                match result {
                    Ok(state) => model.installation = state,
                    Err(error) => {
                        model.installation.busy = false;
                        model.installation.problem = Some(error);
                    }
                }
                model.sync_preferences(cx);
                cx.notify();
            });
        })
        .detach();
        self.sync_preferences(cx);
        cx.notify();
    }

    pub(crate) fn finish_installation_setup(&mut self, cx: &mut Context<Self>) {
        if self.installation.busy {
            return;
        }
        if let Err(error) = crate::cli_install::persistent_integration_allowed() {
            self.installation.onboarding.visible = false;
            self.fail(error.to_string(), cx);
            cx.notify();
            return;
        }
        self.installation.busy = true;
        let mut next = self.installation.clone();
        let path = self.configuration_path("installation.json");
        let login_result = if next.onboarding.login {
            crate::login::set_enabled(true)
        } else {
            Ok(())
        };
        cx.spawn(async move |model, cx| {
            let next = cx
                .background_executor()
                .spawn(async move {
                    let mut errors = Vec::new();
                    if let Err(error) = login_result {
                        errors.push(error);
                    }
                    if next.preferences.cli_enabled {
                        match crate::cli_install::install() {
                            Ok(_) if next.preferences.shell => {
                                if let Err(error) = crate::cli_install::configure_shell() {
                                    errors.push(error.to_string());
                                }
                            }
                            Ok(_) => {}
                            Err(error) => errors.push(error.to_string()),
                        }
                    }
                    next.onboarding.completed = true;
                    next.onboarding.visible = false;
                    next.busy = false;
                    if let Err(error) = next.save(&path) {
                        errors.push(error.to_string());
                    }
                    next.problem = (!errors.is_empty()).then(|| errors.join("\n\n"));
                    next
                })
                .await;
            let _ = model.update(cx, |model, cx| {
                let problem = next.problem.clone();
                model.installation = next;
                if let Some(problem) = problem {
                    model.fail(problem, cx);
                }
                model.installation_action(Action::Refresh, cx);
                model.check_for_updates(false, cx);
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    pub(crate) fn installation_problem(&self) -> Option<&str> {
        self.installation.problem.as_deref()
    }
}

pub(crate) fn setup(model: &AppModel, cx: &mut Context<AppModel>) -> Option<gpui::AnyElement> {
    if !model.installation.onboarding.visible {
        return None;
    }
    let style = controls::Style {
        theme: &model.theme,
        metrics: &model.metrics,
    };
    let state = &model.installation;
    let mut panel = div().id("installation-setup").flex().flex_col().gap(px(16.0)).w(px(620.0)).max_w_full().max_h_full().overflow_y_scroll().p(px(28.0))
        .rounded(px(12.0)).bg(model.theme.raised()).border_1().border_color(model.theme.border)
        .child(div().text_size(px(22.0)).child(tr!("Set up Murvix")))
        .child(tr!("One app bundle contains the desktop, command line tool, and server. No administrator access is needed."));
    for (id, label, checked) in [
        (
            "setup-cli",
            tr_key!("Enable the murvix command — recommended"),
            state.preferences.cli_enabled,
        ),
        (
            "setup-shell",
            tr_key!("Configure my shell PATH (or turn off for manual setup)"),
            state.preferences.shell,
        ),
        (
            "setup-login",
            tr_key!("Start Murvix at login"),
            state.onboarding.login,
        ),
        (
            "setup-downloads",
            tr_key!("Automatically download updates"),
            state.preferences.automatic_downloads,
        ),
    ] {
        panel = panel.child(
            div()
                .flex()
                .justify_between()
                .gap(px(12.0))
                .child(translate(label))
                .child(controls::toggle(
                    style,
                    id,
                    checked,
                    cx.listener(move |model, _, _, cx| {
                        if model.installation.busy {
                            return;
                        }
                        match id {
                            "setup-cli" => {
                                model.installation.preferences.cli_enabled =
                                    !model.installation.preferences.cli_enabled;
                            }
                            "setup-shell" => {
                                model.installation.preferences.shell =
                                    !model.installation.preferences.shell;
                            }
                            "setup-login" => {
                                model.installation.onboarding.login =
                                    !model.installation.onboarding.login;
                            }
                            _ => {
                                model.installation.preferences.automatic_downloads =
                                    !model.installation.preferences.automatic_downloads;
                            }
                        }
                        cx.notify();
                    }),
                )),
        );
    }
    panel = panel
        .child(
            div()
                .text_size(px(12.0))
                .child(state.shell_disclosure.clone()),
        )
        .child(div().text_size(px(12.0)).child(state.health.clone()))
        .child(controls::button(
            style,
            "setup-continue",
            &tr!("Continue"),
            !state.busy,
            cx.listener(|model, _, _, cx| model.finish_installation_setup(cx)),
        ));
    Some(
        div()
            .absolute()
            .inset_0()
            .occlude()
            .flex()
            .items_center()
            .justify_center()
            .bg(model.theme.bg)
            .text_color(model.theme.fg)
            .child(panel)
            .into_any_element(),
    )
}
