use gpui::{AnyElement, Context, IntoElement};
use murvix_ui::l10n::tr_key;
use murvix_ui::{controls, tr};

use super::{Category, SettingsEvent, SettingsView};
use crate::installation::Action;

pub(super) fn rows(pane: &SettingsView, cx: &mut Context<SettingsView>) -> Vec<AnyElement> {
    if !pane.matches(Category::Installation, tr_key!("Installation & Updates")) {
        return Vec::new();
    }
    let state = &pane.snapshot.installation;
    let mut rows = vec![
        pane.note(&state.health, false).into_any_element(),
        pane.note(&state.shell_disclosure, false).into_any_element(),
        pane.note(&state.login_status, false).into_any_element(),
        pane.note(&pane.snapshot.installation_update, false).into_any_element(),
        pane.note(&tr!("Closing the window or quitting Murvix preserves the on-demand server. Start at login opens the app; it is independent of terminal session persistence."), false).into_any_element(),
    ];
    if let Some(problem) = &pane.snapshot.installation_problem {
        rows.push(pane.note(problem, true).into_any_element());
    }
    for (id, label, action) in [
        (
            "installation-cli",
            tr_key!("Install or repair bundled CLI link"),
            Action::RepairCli,
        ),
        (
            "installation-shell",
            tr_key!("Configure shell PATH…"),
            Action::ConfigureShell,
        ),
        (
            "installation-login-enable",
            tr_key!("Start Murvix at login"),
            Action::Login(true),
        ),
        (
            "installation-login-disable",
            tr_key!("Do not start at login"),
            Action::Login(false),
        ),
        (
            "installation-login-settings",
            tr_key!("Open Login Items Settings"),
            Action::LoginSettings,
        ),
        (
            "installation-check",
            tr_key!("Check for Updates…"),
            Action::CheckUpdates,
        ),
        (
            "installation-refresh",
            tr_key!("Refresh integration status"),
            Action::Refresh,
        ),
    ] {
        rows.push(
            pane.row(
                id,
                label,
                controls::button(
                    pane.style(),
                    id,
                    &murvix_ui::l10n::translate(label),
                    !state.busy,
                    cx.listener(move |_, _, _, cx| cx.emit(SettingsEvent::Installation(action))),
                )
                .into_any_element(),
            ),
        );
    }
    rows.push(pane.row(
        "installation-downloads",
        tr_key!("Automatically download updates"),
        controls::toggle(
            pane.style(),
            "installation-downloads",
            state.preferences.automatic_downloads,
            cx.listener(move |pane: &mut SettingsView, _, _, cx| {
                cx.emit(SettingsEvent::Installation(Action::AutomaticDownloads(
                    !pane.snapshot.installation.preferences.automatic_downloads,
                )));
            }),
        ),
    ));
    rows
}
