//! Native workspace filesystem lifecycle; no agent-specific tools or plugins.

#[cfg(target_os = "linux")]
mod fuse;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "linux")]
mod sandbox;

use std::ffi::OsString;
use std::io::{self, BufRead, Read, Write};
#[cfg(target_os = "linux")]
use std::os::unix::fs::DirBuilderExt;
#[cfg(target_os = "linux")]
use std::path::Path;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use murvix_client::Client;
#[cfg(target_os = "linux")]
use murvix_protocol::OperationId;
use murvix_protocol::{ClientKind, ServerIdentity, SessionId, WorkspaceSnapshot};
use serde_json::json;

type Result<T = ()> = std::result::Result<T, Box<dyn std::error::Error>>;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum Invocation {
    Snapshot,
    Metadata,
    Run(Vec<OsString>),
    Sandbox {
        mountpoint: PathBuf,
        command: Vec<OsString>,
    },
}

pub(crate) fn parse(arguments: &[OsString]) -> io::Result<Invocation> {
    match arguments {
        [command] if command == "snapshot" => Ok(Invocation::Snapshot),
        [command, flag] if command == "snapshot" && flag == "--json" => Ok(Invocation::Snapshot),
        [command] if command == "metadata" => Ok(Invocation::Metadata),
        [command, separator, executable, rest @ ..] if command == "run" && separator == "--" => {
            Ok(Invocation::Run(
                std::iter::once(executable.clone())
                    .chain(rest.iter().cloned())
                    .collect(),
            ))
        }
        [command, mountpoint, separator, executable, rest @ ..]
            if command == "sandbox" && separator == "--" =>
        {
            Ok(Invocation::Sandbox {
                mountpoint: mountpoint.into(),
                command: std::iter::once(executable.clone())
                    .chain(rest.iter().cloned())
                    .collect(),
            })
        }
        _ => Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            crate::manage::help::WORKSPACE,
        )),
    }
}

/// A live terminal binding, never an inherited root-array snapshot.
#[derive(Clone, Debug)]
pub(super) struct Binding {
    server: ServerIdentity,
    session: SessionId,
    profile: PathBuf,
    client: Arc<Mutex<Option<Client>>>,
}

impl Binding {
    fn from_environment() -> Result<Self> {
        let server = std::env::var("MUXY_SERVER_ID")
            .map_err(|_| "workspace unavailable: not a Murvix terminal (missing server identity)")?
            .parse()?;
        let session = SessionId::new(
            std::env::var("MUXY_SESSION_ID")
                .map_err(
                    |_| "workspace unavailable: not a Murvix terminal (missing session identity)",
                )?
                .parse()?,
        )
        .ok_or("workspace unavailable: invalid terminal session identity")?;
        Ok(Self {
            server,
            session,
            profile: std::path::absolute(murvix_core::dirs::murvix_dir()?)?,
            client: Arc::new(Mutex::new(None)),
        })
    }

    pub(super) fn snapshot(&self) -> Result<WorkspaceSnapshot> {
        let mut connection = self
            .client
            .lock()
            .map_err(|_| "workspace connection lock poisoned")?;
        if connection
            .as_ref()
            .is_none_or(|client| !client.is_connected())
        {
            let client = Client::connect(&self.profile.join("server.sock"))
                .map_err(|error| format!("workspace unavailable: {error}"))?;
            client.identify(ClientKind::Cli)?;
            *connection = Some(client);
        }
        let client = connection
            .as_ref()
            .ok_or("workspace unavailable: connection missing")?;
        client
            .workspace_snapshot(self.server, self.session)
            .map_err(|error| format!("workspace unavailable: {error}").into())
    }
}

pub(crate) fn run(invocation: Invocation) -> Result {
    // Sandbox startup must remain single-threaded: do not acquire a Client or
    // runtime lease before entering the platform's confinement boundary.
    if let Invocation::Sandbox {
        mountpoint,
        command,
    } = invocation
    {
        #[cfg(target_os = "linux")]
        return sandbox::exec(&mountpoint, &command);
        #[cfg(not(target_os = "linux"))]
        return Err(format!(
            "internal Linux sandbox invocation is unavailable: {} ({})",
            mountpoint.display(),
            command.len()
        )
        .into());
    }
    let _lease =
        murvix_client::local::bundle::acquire_runtime(&murvix_core::executable::current_path()?)?;
    match invocation {
        Invocation::Snapshot => {
            serde_json::to_writer(
                io::stdout().lock(),
                &Binding::from_environment()?.snapshot()?,
            )?;
            Ok(())
        }
        Invocation::Metadata => metadata(),
        Invocation::Run(command) => launch(&command),
        Invocation::Sandbox { .. } => {
            unreachable!("sandbox is dispatched before runtime initialization")
        }
    }
}

/// Local native helpers hold this process's stdin open and request one current
/// snapshot at an operation boundary. Errors never return a saved descriptor.
fn metadata() -> Result {
    let binding = Binding::from_environment()?;
    let mut input = io::stdin().lock();
    let mut output = io::stdout().lock();
    let mut line = Vec::new();
    loop {
        line.clear();
        let count = input.by_ref().take(4097).read_until(b'\n', &mut line)?;
        if count == 0 {
            return Ok(());
        }
        if count > 4096 || line.last() != Some(&b'\n') {
            return Err("workspace metadata request exceeds 4096 bytes or lacks newline".into());
        }
        let response = match serde_json::from_slice::<serde_json::Value>(&line) {
            Ok(request) if request == json!({"op":"snapshot"}) => match binding.snapshot() {
                Ok(snapshot) => json!({"snapshot":snapshot}),
                Err(error) => json!({"error":error.to_string()}),
            },
            _ => json!({"error":"expected {\"op\":\"snapshot\"}"}),
        };
        serde_json::to_writer(&mut output, &response)?;
        output.write_all(b"\n")?;
        output.flush()?;
    }
}

pub(crate) fn launch(command: &[OsString]) -> Result {
    if command.is_empty() {
        return Err("workspace launch requires a command".into());
    }
    let binding = Binding::from_environment()?;
    #[cfg(target_os = "linux")]
    let snapshot = binding.snapshot()?;
    #[cfg(target_os = "linux")]
    {
        let directory = binding
            .profile
            .join("agent-workspaces")
            .join(binding.server.to_string())
            .join(snapshot.project.to_string());
        let state = directory.join("state");
        for child in ["home", "tmp", "cache", "data", "config", "runtime"] {
            private_directory(&state.join(child))?;
        }
        let mountpoint = directory.join(format!("mount-{}", OperationId::new()));
        private_directory(&mountpoint)?;
        let result = launch_linux(binding, &state, &mountpoint, command);
        // Unmount before removing only the empty mountpoint; state survives.
        let removal = std::fs::remove_dir(&mountpoint);
        result?;
        removal?;
        Ok(())
    }
    #[cfg(target_os = "macos")]
    {
        return macos::launch(binding, command);
    }
}

#[cfg(target_os = "linux")]
fn private_directory(path: &Path) -> io::Result<()> {
    std::fs::DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(path)
}

#[cfg(target_os = "linux")]
fn launch_linux(binding: Binding, state: &Path, mountpoint: &Path, command: &[OsString]) -> Result {
    use signal_hook::consts::signal::{SIGHUP, SIGINT, SIGTERM};
    use signal_hook::iterator::Signals;
    use std::process::Command;
    let session = fuse::mount(binding, state.to_path_buf(), mountpoint)?;
    let mut signals = Signals::new([signal_hook::consts::SIGCHLD, SIGINT, SIGTERM, SIGHUP])?;
    let executable = murvix_core::executable::current_path()?;
    let mut child = Command::new(executable)
        .arg("workspace")
        .arg("sandbox")
        .arg(mountpoint)
        .arg("--")
        .args(command)
        .current_dir(mountpoint)
        .env("HOME", mountpoint.join(".state/home"))
        .env("TMPDIR", mountpoint.join(".state/tmp"))
        .env("TMP", mountpoint.join(".state/tmp"))
        .env("TEMP", mountpoint.join(".state/tmp"))
        .env("XDG_CACHE_HOME", mountpoint.join(".state/cache"))
        .env("XDG_DATA_HOME", mountpoint.join(".state/data"))
        .env("XDG_CONFIG_HOME", mountpoint.join(".state/config"))
        .env("XDG_STATE_HOME", mountpoint.join(".state/data"))
        .env("XDG_RUNTIME_DIR", mountpoint.join(".state/runtime"))
        .env("MURVIX_WORKSPACE_ROOT", mountpoint)
        .env("MUXY_WORKSPACE_WRAPPERS", "0")
        .env("MURVIX_PRIMARY_ROOT", mountpoint.join("primary"))
        .spawn()?;
    let status = loop {
        if let Some(status) = child.try_wait()? {
            break status;
        }
        if let Some(signal) = signals.forever().next()
            && signal != signal_hook::consts::SIGCHLD
        {
            let signal =
                rustix::process::Signal::from_named_raw(signal).ok_or("invalid process signal")?;
            if let Some(pid) = rustix::process::Pid::from_raw(child.id().cast_signed()) {
                let _ = rustix::process::kill_process(pid, signal);
            }
        }
    };
    session.umount_and_join()?;
    if status.success() {
        Ok(())
    } else {
        Err(Box::new(crate::CommandExit(status)))
    }
}
