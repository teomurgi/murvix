use std::env;
use std::ffi::{OsStr, OsString};
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};

use murvix_protocol::ErrorCode;
use murvix_terminal::pty::{Pty, PtySize, SpawnRequest};

use crate::error::ServerError;
use crate::settings::ServerSettings;

#[cfg(target_os = "macos")]
const FALLBACK_SHELL: &str = "/bin/zsh";
#[cfg(target_os = "linux")]
const FALLBACK_SHELL: &str = "/bin/sh";
const LOGIN_FLAG: &str = "-l";
const TERMINAL_ENV: [(&str, &str); 3] = [
    ("TERM", "xterm-256color"),
    ("COLORTERM", "truecolor"),
    ("TERM_PROGRAM", "muxy"),
];

pub(crate) fn spawn_shell(
    settings: &ServerSettings,
    integration: Option<&crate::ShellIntegration>,
    directory: &Path,
    size: PtySize,
    identity: (murvix_protocol::ServerIdentity, murvix_protocol::SessionId),
    workspace: &[murvix_protocol::ServerPath],
) -> Result<Pty, ServerError> {
    if !directory.is_dir() {
        return Err(ServerError::new(
            ErrorCode::BadPath,
            format!("{} is not a directory", directory.display()),
        ));
    }
    let mut request = SpawnRequest {
        program: resolve_shell(settings),
        args: vec![OsString::from(LOGIN_FLAG)],
        cwd: directory.to_path_buf(),
        env: environment(identity, workspace),
        size,
    };
    request.env.retain(|(name, _)| name != "SHELL");
    request.env.push((
        OsString::from("SHELL"),
        request.program.clone().into_os_string(),
    ));
    if let Some(integration) = integration {
        integration.configure(&mut request, settings.shell_integration);
    }
    Pty::spawn(request).map_err(ServerError::spawn_failed)
}

fn resolve_shell(settings: &ServerSettings) -> PathBuf {
    select_shell(
        settings.default_shell.clone(),
        env::var_os("SHELL"),
        cfg!(target_os = "linux"),
    )
}

fn select_shell(configured: Option<PathBuf>, inherited: Option<OsString>, linux: bool) -> PathBuf {
    use std::os::unix::fs::PermissionsExt;
    configured
        .or_else(|| {
            inherited
                .filter(|shell| !shell.is_empty())
                .map(PathBuf::from)
                .filter(|shell| {
                    !linux
                        || (shell.is_absolute()
                            && shell.metadata().is_ok_and(|metadata| {
                                metadata.is_file() && metadata.permissions().mode() & 0o111 != 0
                            }))
                })
        })
        .unwrap_or_else(|| PathBuf::from(if linux { "/bin/sh" } else { FALLBACK_SHELL }))
}

fn environment(
    identity: (murvix_protocol::ServerIdentity, murvix_protocol::SessionId),
    workspace: &[murvix_protocol::ServerPath],
) -> Vec<(OsString, OsString)> {
    environment_from(env::vars_os().collect(), identity, workspace)
}

fn environment_from(
    mut env: Vec<(OsString, OsString)>,
    identity: (murvix_protocol::ServerIdentity, murvix_protocol::SessionId),
    workspace: &[murvix_protocol::ServerPath],
) -> Vec<(OsString, OsString)> {
    env.retain(|(name, _)| {
        name != "MUXY_SERVER_ID"
            && name != "MUXY_SESSION_ID"
            && !name.as_encoded_bytes().starts_with(b"MUXY_WORKSPACE_")
    });
    env.push(("MUXY_SERVER_ID".into(), identity.0.to_string().into()));
    env.push((
        "MUXY_SESSION_ID".into(),
        identity.1.get().to_string().into(),
    ));
    env.push((
        "MUXY_WORKSPACE_COUNT".into(),
        workspace.len().to_string().into(),
    ));
    for (index, root) in workspace.iter().enumerate() {
        env.push((
            OsString::from(format!("MUXY_WORKSPACE_{index}")),
            OsStr::from_bytes(&root.0).to_os_string(),
        ));
    }
    env.extend(
        TERMINAL_ENV
            .iter()
            .map(|(key, value)| (OsString::from(key), OsString::from(value))),
    );
    env
}

#[cfg(test)]
mod tests {
    use super::{OsString, environment_from};
    use murvix_protocol::{ServerIdentity, ServerPath, SessionId};
    use std::os::unix::ffi::OsStringExt;

    fn get(env: &[(OsString, OsString)], name: &str) -> Option<OsString> {
        env.iter()
            .find(|(key, _)| key == name)
            .map(|(_, value)| value.clone())
    }
    #[test]
    fn workspace_env_is_indexed_byte_exact_and_clears_stale_entries() {
        let identity = (
            ServerIdentity::new(),
            SessionId::new(7).expect("nonzero session"),
        );
        // Non-UTF8, newline- and colon-bearing roots: a joined encoding would
        // corrupt or split these; indexed entries carry them byte-exact.
        let roots = vec![
            ServerPath(b"/srv/app".to_vec()),
            ServerPath(b"/srv/\xff:we\nird".to_vec()),
        ];
        // Stale inherited entries larger than the new set must not leak.
        let base = vec![
            (OsString::from("MUXY_WORKSPACE_COUNT"), OsString::from("9")),
            (OsString::from("MUXY_WORKSPACE_0"), OsString::from("stale")),
            (OsString::from("MUXY_WORKSPACE_7"), OsString::from("stale")),
            (OsString::from("MUXY_SERVER_ID"), OsString::from("old")),
            (OsString::from("PATH"), OsString::from("/bin")),
        ];
        let env = environment_from(base, identity, &roots);
        assert_eq!(get(&env, "MUXY_WORKSPACE_COUNT"), Some("2".into()));
        assert_eq!(
            get(&env, "MUXY_WORKSPACE_0"),
            Some(OsString::from_vec(b"/srv/app".to_vec()))
        );
        assert_eq!(
            get(&env, "MUXY_WORKSPACE_1"),
            Some(OsString::from_vec(b"/srv/\xff:we\nird".to_vec()))
        );
        assert!(get(&env, "MUXY_WORKSPACE_2").is_none());
        assert!(get(&env, "MUXY_WORKSPACE_7").is_none());
        // PATH passes through; stale server ID is replaced.
        assert_eq!(get(&env, "PATH"), Some("/bin".into()));
        assert_ne!(get(&env, "MUXY_SERVER_ID"), Some("old".into()));
        // Reader contract: increment from 0 while the name exists.
        let mut read = Vec::new();
        let mut index = 0;
        while let Some(value) = env
            .iter()
            .find(|(key, _)| key.as_encoded_bytes() == format!("MUXY_WORKSPACE_{index}").as_bytes())
            .map(|(_, value)| value.clone())
        {
            read.push(value);
            index += 1;
        }
        assert_eq!(read.len(), 2);
        assert_eq!(read[1].as_encoded_bytes(), b"/srv/\xff:we\nird");
    }
}
