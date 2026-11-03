//! Native NFSv3 over a private Unix socket. Only mount/unmount use sudo;
//! the broker and the confined command always retain the invoking user's uid.
#![allow(unsafe_code)]

mod backend;
mod posix;

use std::error::Error;
use std::ffi::OsString;
use std::io;
use std::os::unix::fs::{DirBuilderExt, PermissionsExt};
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Arc, mpsc};
use std::thread;
use std::time::Duration;

use super::Binding;
use signal_hook::consts::signal::{SIGCHLD, SIGHUP, SIGINT, SIGTERM};
use signal_hook::iterator::Signals;

type Result<T = ()> = std::result::Result<T, Box<dyn Error>>;

// Default-deny process/file rules follow the native Seatbelt primitives used by
// Chromium and anthropic-experimental/sandbox-runtime's macos-sandbox-utils.ts.
// Deliberately do NOT copy its broad Mach, POSIX IPC, or Unix-socket allowances:
// an unconstrained local service could perform writes on the agent's behalf.
const SANDBOX: &str = r#"
(version 1)
(deny default)
(allow process-exec process-fork)
(allow process-info* (target same-sandbox))
(allow signal (target same-sandbox))
(allow sysctl-read)
(allow file-read*)
(allow file-write* (subpath (param "PRIMARY")) (subpath (param "STATE")))
(allow file-read-data file-write-data file-ioctl
    (require-all (vnode-type CHARACTER-DEVICE)
        (require-any (literal "/dev/null") (literal "/dev/tty")
                     (regex #"^/dev/ttys[0-9]+$"))))
(allow file-ioctl (literal "/dev/random") (literal "/dev/urandom"))
(allow network-outbound (remote ip "*:*"))
(allow network-bind (local ip "*:*"))
(allow system-socket (require-all (socket-domain AF_SYSTEM) (socket-protocol 2)))
; Name resolution only. No launchservices, AppleEvents, privileged task ports,
; shared-memory IPC, or generic Mach/XPC service access.
(allow mach-lookup (global-name "com.apple.system.opendirectoryd.libinfo")
                   (global-name "com.apple.system.opendirectoryd.membership")
                   (global-name "com.apple.mDNSResponder"))
(deny file-read* file-write* (subpath (param "PROFILE")) (subpath (param "CONTROL")))
(deny network-outbound (remote unix-socket (subpath (param "CONTROL"))))
"#;

pub(super) fn launch(binding: Binding, command: &[OsString]) -> Result {
    let (program, arguments) = command
        .split_first()
        .ok_or("workspace launch requires a command")?;
    // Never accidentally turn the ordinary-user broker into a root service.
    if unsafe { libc::geteuid() } == 0
        || unsafe { libc::getuid() != libc::geteuid() || libc::getgid() != libc::getegid() }
    {
        return Err(
            "run workspace as an ordinary user; only the native mount receives sudo approval"
                .into(),
        );
    }
    safe_stdio()?;
    let snapshot = binding.snapshot()?;
    let profile = binding.profile.canonicalize()?;
    let state = profile
        .join("agent-workspaces")
        .join(binding.server.to_string())
        .join(snapshot.project.to_string())
        .join("state");
    for child in ["home", "tmp", "cache", "data", "config", "runtime"] {
        private_directory(&state.join(child))?;
    }
    // No catalog/user pathname enters mount_nfs's comma-separated option parser.
    let directory = PathBuf::from(format!(
        "/private/tmp/murvix-{}",
        murvix_protocol::OperationId::new()
    ));
    std::fs::DirBuilder::new().mode(0o700).create(&directory)?;
    let control = directory.join("control");
    let mountpoint = directory.join("workspace");
    let socket = control.join("nfs.sock");
    let mut broker = None;
    let mut mounted = false;
    let result = (|| -> Result {
        private_directory(&control)?;
        private_directory(&mountpoint)?;
        let filesystem = backend::Filesystem::new(binding, state, mountpoint.clone())?;
        broker = Some(Broker::start(&socket, filesystem)?);
        let mut signals = Signals::new([SIGCHLD, SIGINT, SIGTERM, SIGHUP])?;
        // Apple's mount_nfs/getaddresslist explicitly interprets an empty host
        // as AF_LOCAL, using local_nfs_port/local_mount_port. See:
        // github.com/apple-oss-distributions/NFS/blob/main/mount_nfs/mount_nfs.c
        let options = format!(
            "vers=3,proto=ticotsord,port={},mountport={},noac,nonegnamecache,nordirplus,locallocks,sync,nosuid,nodev",
            socket.display(),
            socket.display()
        );
        let status = Command::new("/usr/bin/sudo")
            .args(["--", "/sbin/mount_nfs", "-o"])
            .arg(options)
            .arg(":/workspace")
            .arg(&mountpoint)
            .status()?;
        if !status.success() {
            return Err(format!("native Unix-socket NFS mount failed: {status}").into());
        }
        mounted = true;
        let mut child = Command::new("/usr/bin/sandbox-exec");
        child.arg("-p").arg(SANDBOX);
        for (name, path) in [
            ("PROFILE", profile.clone()),
            ("CONTROL", control.clone()),
            ("PRIMARY", mountpoint.join("primary")),
            ("STATE", mountpoint.join(".state")),
        ] {
            let mut parameter = OsString::from(format!("{name}="));
            parameter.push(path);
            child.arg("-D").arg(parameter);
        }
        child
            .arg(program)
            .args(arguments)
            .current_dir(&mountpoint)
            .process_group(0);
        for (name, relative) in [
            ("HOME", "home"),
            ("TMPDIR", "tmp"),
            ("TMP", "tmp"),
            ("TEMP", "tmp"),
            ("XDG_CACHE_HOME", "cache"),
            ("XDG_DATA_HOME", "data"),
            ("XDG_CONFIG_HOME", "config"),
            ("XDG_STATE_HOME", "data"),
            ("XDG_RUNTIME_DIR", "runtime"),
        ] {
            child.env(name, mountpoint.join(".state").join(relative));
        }
        child
            .env("MURVIX_WORKSPACE_ROOT", &mountpoint)
            .env("MURVIX_PRIMARY_ROOT", mountpoint.join("primary"));
        let mut child = child.spawn()?;
        let pid = i32::try_from(child.id())?;
        let outcome = loop {
            match child.try_wait() {
                Ok(Some(status)) => {
                    break if status.success() {
                        Ok(())
                    } else {
                        Err(format!("workspace command exited with {status}").into())
                    };
                }
                Err(error) => break Err(error.into()),
                Ok(None) => {}
            }
            if let Some(error) = broker
                .as_ref()
                .and_then(|broker| broker.failure.try_recv().ok())
            {
                break Err(error.into());
            }
            for signal in signals.pending().filter(|signal| *signal != SIGCHLD) {
                unsafe {
                    libc::kill(-pid, signal);
                }
            }
            thread::sleep(Duration::from_millis(20));
        };
        // Descendants must not keep the filesystem alive after the command ends.
        unsafe {
            libc::kill(-pid, SIGTERM);
        }
        if child.try_wait()?.is_none() {
            child.kill()?;
            child.wait()?;
        }
        unsafe {
            libc::kill(-pid, libc::SIGKILL);
        }
        outcome
    })();
    // Keep the broker serving until the kernel has released the mount. A failed
    // ordinary unmount gets one explicit forced unmount, never a directory view.
    let unmount: Result = if mounted {
        match unmount(&mountpoint, false) {
            Ok(()) => Ok(()),
            Err(first) => unmount(&mountpoint, true)
                .map_err(|second| format!("{first}; forced unmount: {second}").into()),
        }
    } else {
        Ok(())
    };
    if let Err(error) = unmount {
        // Do not remove a live mountpoint or stop its server underneath it.
        // Process exit still terminates the broker; report the privileged cleanup
        // path explicitly instead of concealing an administrative failure.
        return Err(format!(
            "workspace cleanup failed at {}: {} (command: {})",
            mountpoint.display(),
            error,
            result
                .err()
                .map_or_else(|| "success".to_owned(), |e| e.to_string())
        )
        .into());
    }
    let stopped = broker.map_or(Ok(()), Broker::stop);
    let cleanup = (|| -> Result {
        if socket.exists() {
            std::fs::remove_file(&socket)?;
        }
        if control.exists() {
            std::fs::remove_dir(&control)?;
        }
        if mountpoint.exists() {
            std::fs::remove_dir(&mountpoint)?;
        }
        std::fs::remove_dir(&directory)?;
        Ok(())
    })();
    // Cleanup failures are never swallowed, including when the command failed.
    let failures: Vec<String> = [result, stopped, cleanup]
        .into_iter()
        .filter_map(|r| r.err().map(|e| e.to_string()))
        .collect();
    if failures.is_empty() {
        Ok(())
    } else {
        Err(failures.join("; ").into())
    }
}

fn private_directory(path: &Path) -> io::Result<()> {
    std::fs::DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(path)?;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o700))
}

fn unmount(mountpoint: &Path, force: bool) -> Result {
    let mut command = Command::new("/usr/bin/sudo");
    command.args(["--", "/sbin/umount"]);
    if force {
        command.arg("-f");
    }
    let status = command.arg(mountpoint).status()?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("unmount failed: {status}").into())
    }
}

fn safe_stdio() -> Result {
    // An inherited regular-file or control-socket descriptor is a capability
    // outside pathname confinement. Accept only anonymous pipes and safe tty or
    // null devices; callers can capture output through a pipe, not a host file.
    for fd in 0..=2 {
        let mut stat = std::mem::MaybeUninit::<libc::stat>::uninit();
        if unsafe { libc::fstat(fd, stat.as_mut_ptr()) } != 0 {
            return Err(io::Error::last_os_error().into());
        }
        let stat = unsafe { stat.assume_init() };
        let null = std::fs::metadata("/dev/null")?;
        use std::os::unix::fs::MetadataExt;
        let safe = stat.st_mode & libc::S_IFMT == libc::S_IFIFO
            || (stat.st_mode & libc::S_IFMT == libc::S_IFCHR
                && (unsafe { libc::isatty(fd) } == 1
                    || stat.st_rdev == null.rdev() as libc::dev_t));
        if !safe {
            return Err(format!("workspace stdio fd {fd} must be a pipe, terminal, or /dev/null (not a host file/socket)").into());
        }
    }
    Ok(())
}

struct Broker {
    shutdown: tokio::sync::oneshot::Sender<()>,
    thread: thread::JoinHandle<std::result::Result<(), String>>,
    failure: mpsc::Receiver<String>,
}

impl Broker {
    fn start(socket: &Path, filesystem: Arc<backend::Filesystem>) -> Result<Self> {
        let listener = std::os::unix::net::UnixListener::bind(socket)?;
        std::fs::set_permissions(socket, std::fs::Permissions::from_mode(0o600))?;
        listener.set_nonblocking(true)?;
        let (shutdown, mut receiver) = tokio::sync::oneshot::channel();
        let (ready_tx, ready_rx) = mpsc::sync_channel(1);
        let (failure_tx, failure) = mpsc::channel();
        let thread = thread::Builder::new().name("workspace-nfs".into()).spawn(move || {
            let result = (|| -> std::result::Result<(), String> {
                let runtime = tokio::runtime::Builder::new_current_thread().enable_all().build().map_err(|e| e.to_string())?;
                runtime.block_on(async move {
                    let listener = tokio::net::UnixListener::from_std(listener).map_err(|e| e.to_string())?;
                    ready_tx.send(()).map_err(|e| e.to_string())?;
                    let mut connections = tokio::task::JoinSet::new();
                    loop {
                        tokio::select! {
                            _ = &mut receiver => break,
                            accepted = listener.accept() => {
                                let (stream, _) = accepted.map_err(|e| e.to_string())?;
                                let filesystem = filesystem.clone();
                                connections.spawn(async move {
                                    nfs3_server::transport::Connection::new("/workspace", filesystem)
                                        .serve(stream).await.map_err(|e| e.to_string())
                                });
                            }
                            completed = connections.join_next(), if !connections.is_empty() => {
                                match completed {
                                    Some(Ok(Ok(()))) => {},
                                    Some(Ok(Err(error))) => return Err(error),
                                    Some(Err(error)) => return Err(error.to_string()),
                                    None => {},
                                }
                            }
                        }
                    }
                    connections.abort_all();
                    while connections.join_next().await.is_some() {}
                    Ok(())
                })
            })();
            if let Err(error) = &result { let _ = failure_tx.send(error.clone()); }
            result
        })?;
        if let Err(error) = ready_rx.recv() {
            let details = thread.join().map_err(|_| "workspace broker panicked")?;
            return Err(details.err().unwrap_or_else(|| error.to_string()).into());
        }
        Ok(Self {
            shutdown,
            thread,
            failure,
        })
    }

    fn stop(self) -> Result {
        let _ = self.shutdown.send(());
        self.thread
            .join()
            .map_err(|_| "workspace broker panicked")?
            .map_err(Into::into)
    }
}
