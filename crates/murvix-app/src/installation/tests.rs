use super::State;

#[test]
fn explicit_opt_out_survives_relaunch_and_later_preference_changes() -> std::io::Result<()> {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("installation.json");
    let mut state = State::load(&path);
    state.onboarding.completed = true;
    state.preferences.cli_enabled = false;
    state.preferences.shell = false;
    state.preferences.automatic_downloads = false;
    state.save(&path)?;
    let mut relaunched = State::load(&path);
    assert!(relaunched.onboarding.completed);
    assert!(!relaunched.preferences.cli_enabled);
    assert!(!relaunched.preferences.shell);
    relaunched.preferences.automatic_downloads = true;
    relaunched.save(&path)?;
    let relaunched = State::load(&path);
    assert!(relaunched.onboarding.completed);
    assert!(!relaunched.preferences.cli_enabled);
    assert!(!relaunched.preferences.shell);
    assert!(relaunched.preferences.automatic_downloads);
    Ok(())
}

#[test]
fn damaged_preferences_do_not_authorize_repairs_or_downloads() -> std::io::Result<()> {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("installation.json");
    std::fs::write(&path, br#"{"completed":true,"automatic_downloads":true,"#)?;
    let state = State::load(&path);
    assert!(!state.onboarding.completed);
    assert!(!state.preferences.automatic_downloads);
    assert!(state.problem.is_some());
    Ok(())
}
