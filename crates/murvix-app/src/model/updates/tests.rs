use super::*;
use crate::model::tests::stub_boot;
use gpui::TestAppContext;
use murvix_app_core::AppState;

#[gpui::test]
fn newer_release_replaces_downloaded_update_and_preserves_schedule(cx: &mut TestAppContext) {
    let (boot, requests) = stub_boot(AppState::bootstrap().expect("state"));
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    view.update(cx, |model, cx| {
        model.installation.preferences.automatic_downloads = true;
        model.servers.local.connection = ConnectionState::Ready;
        model.updates.server = Some(murvix_protocol::ServerInfo::current());
        model.updates.ready = Some(PreparedUpdate::fixture().expect("cached update"));
        model.updates.scheduled = true;
        model.updates.retry_preparation = Some(model.servers.local.generation);
        model.updates.mode = Some(UpdateMode::EndSessions);
        model.updates.download = AppUpdatePhase::Checking;
        let release = model
            .receive_update_release(Ok(Some(Release::fixture("2.0.1"))), cx)
            .expect("newer release");
        assert_eq!(release.version, "2.0.1");
        assert!(model.updates.download == AppUpdatePhase::Downloading);
        model.begin_update(cx);
        assert!(model.quitting == Quitting::Idle);
        model.receive_server_update(
            Ok(ServerUpdate {
                server: murvix_protocol::ServerInfo::current(),
                sessions: 0,
                replaced: false,
            }),
            cx,
        );
        assert!(model.quitting == Quitting::Idle);
        let mut latest = PreparedUpdate::fixture().expect("latest update");
        latest.version = release.version;
        model.finish_update_check(Ok(Some(latest)), cx);
        assert!(!model.updates.checking());
        assert!(model.updates.scheduled);
        assert!(model.updates.retry_preparation.is_none());
        assert!(model.updates.mode.is_none());
        assert_eq!(
            model.updates.ready.as_ref().expect("update").version,
            "2.0.1"
        );
        let record: serde_json::Value = serde_json::from_slice(
            &std::fs::read(model.path.with_file_name("pending-update.json")).expect("record"),
        )
        .expect("json");
        assert_eq!(record["version"], "2.0.1");
        assert_eq!(record["scheduled"], true);
    });
    assert!(
        !requests
            .try_iter()
            .any(|(_, work)| matches!(work, Work::Flush | Work::PrepareUpdate { .. }))
    );
}

#[gpui::test]
fn revoking_download_consent_during_release_check_preserves_staged_work(cx: &mut TestAppContext) {
    let (boot, _requests) = stub_boot(AppState::bootstrap().expect("state"));
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    view.update(cx, |model, cx| {
        model.updates.ready = Some(PreparedUpdate::fixture().expect("cached update"));
        model.updates.scheduled = true;
        model.updates.download = AppUpdatePhase::Checking;
        model.installation.preferences.automatic_downloads = false;
        assert!(
            model
                .receive_update_release(Ok(Some(Release::fixture("2.0.1"))), cx)
                .is_none()
        );
        assert!(!model.updates.checking());
        assert!(model.updates.scheduled);
        assert_eq!(
            model.updates.ready.as_ref().expect("cached update").version,
            "2.0.0"
        );
        // A manual request joining a background check is explicit authorization,
        // even after the persisted automatic preference was switched off.
        model.updates.manual = true;
        model.updates.download = AppUpdatePhase::Checking;
        assert!(
            model
                .receive_update_release(Ok(Some(Release::fixture("2.0.1"))), cx)
                .is_some()
        );
        assert!(model.updates.download == AppUpdatePhase::Downloading);
    });
}
