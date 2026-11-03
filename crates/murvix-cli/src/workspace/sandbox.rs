//! Linux's writable workspace is the only writable filesystem view.
//!
//! Mount read-only is essential: Landlock does not mediate chmod, chown,
//! xattrs, or timestamps. Landlock additionally closes procfs and UNIX-socket
//! escape routes; seccomp prevents rebuilding a writable mount namespace.
//! There is intentionally no best-effort or external-helper fallback.

mod sys;

use std::error::Error;
use std::ffi::OsString;
use std::io;
use std::os::fd::{AsFd, OwnedFd};
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::Command;

use rustix::fs::{Mode, OFlags, ResolveFlags};
use rustix::mount::{MountPropagationFlags, MoveMountFlags, OpenTreeFlags};
use rustix::thread::{CapabilitiesSecureBits, CapabilitySet, CapabilitySets};

/// Called only by the fresh, single-threaded `workspace sandbox` subprocess.
/// Successful execution replaces this process; every error aborts the launch.
pub(super) fn exec(mountpoint: &Path, command: &[OsString]) -> Result<(), Box<dyn Error>> {
    let (program, arguments) = command
        .split_first()
        .ok_or("workspace sandbox requires a command after --")?;
    sys::prepare_stdio()?;
    require_single_thread()?;
    sys::require_landlock_abi()?;
    let state_paths = state_environment(mountpoint)?;
    let uid = rustix::process::getuid();
    let gid = rustix::process::getgid();
    if uid != rustix::process::geteuid() || gid != rustix::process::getegid() {
        return Err("workspace sandbox must not run as a set-ID executable".into());
    }
    let last_capability: u32 = std::fs::read_to_string("/proc/sys/kernel/cap_last_cap")?
        .trim()
        .parse()?;
    if last_capability >= u64::BITS {
        return Err("kernel capabilities exceed the supported 64-bit capability ABI".into());
    }

    // unshare clones the whole mount tree, including nested mounts. No file
    // descriptor into the original host tree may survive into the command.
    sys::enter_namespaces().map_err(|error| {
        io::Error::other(format!(
            "workspace sandbox requires enabled unprivileged user/mount/IPC namespaces: {error}"
        ))
    })?;
    std::fs::write(
        "/proc/self/uid_map",
        format!("{} {} 1\n", uid.as_raw(), uid.as_raw()),
    )?;
    std::fs::write("/proc/self/setgroups", "deny\n")?;
    std::fs::write(
        "/proc/self/gid_map",
        format!("{} {} 1\n", gid.as_raw(), gid.as_raw()),
    )?;
    rustix::mount::mount_change(
        "/",
        MountPropagationFlags::PRIVATE | MountPropagationFlags::REC,
    )?;

    let root = rustix::fs::open(
        "/",
        OFlags::PATH | OFlags::DIRECTORY | OFlags::CLOEXEC,
        Mode::empty(),
    )?;
    let workspace = open_absolute(&root, mountpoint)?;
    if rustix::fs::fstatfs(&workspace)?.f_type != 0x6573_5546 {
        return Err("workspace sandbox mountpoint is not a FUSE filesystem".into());
    }
    let workspace_stat = rustix::fs::fstat(&workspace)?;
    let state = open_beneath(&workspace, Path::new(".state"))?;
    let state_stat = rustix::fs::fstat(&state)?;
    if state_stat.st_dev != workspace_stat.st_dev {
        return Err("workspace .state must be on the workspace FUSE filesystem".into());
    }

    // Keep one detached, writable FUSE clone while making the ENTIRE attached
    // host tree recursively read-only. Restoring just that clone never clears
    // read-only on an original source path or another inherited host mount.
    let writable = rustix::mount::open_tree(
        &workspace,
        "",
        OpenTreeFlags::OPEN_TREE_CLONE
            | OpenTreeFlags::OPEN_TREE_CLOEXEC
            | OpenTreeFlags::AT_EMPTY_PATH,
    )?;
    sys::protect_workspace_mount(writable.as_fd())?;
    sys::make_host_tree_readonly()?;
    rustix::mount::move_mount(
        &writable,
        "",
        &workspace,
        "",
        MoveMountFlags::MOVE_MOUNT_F_EMPTY_PATH | MoveMountFlags::MOVE_MOUNT_T_EMPTY_PATH,
    )?;
    let attached = open_absolute(&root, mountpoint)?;
    let attached_stat = rustix::fs::fstat(&attached)?;
    if attached_stat.st_dev != workspace_stat.st_dev
        || attached_stat.st_ino != workspace_stat.st_ino
    {
        return Err("workspace mountpoint changed while installing confinement".into());
    }
    // Reopening stdio through the cloned namespace is important even for
    // read-only descriptors: fchmod on an inherited host fd bypasses ro mounts.
    // Anonymous pipes remain usable, but inherited control sockets do not.
    sys::reopen_stdio(root.as_fd())?;
    rustix::process::fchdir(&attached)?;

    let ruleset = sys::landlock_ruleset(attached.as_fd(), state.as_fd())?;
    rustix::thread::set_no_new_privs(true)?;
    sys::enforce_landlock(ruleset.as_fd())?;
    drop_capabilities(last_capability)?;
    sys::install_seccomp()?;
    sys::cloexec_non_stdio()?;

    let mut process = Command::new(program);
    process.args(arguments);
    // Use only the persistent, broker-controlled state view, even when the
    // helper was invoked directly rather than by the normal workspace launcher.
    for (name, path) in state_paths {
        process.env(name, path);
    }
    Err(process.exec().into())
}

fn require_single_thread() -> io::Result<()> {
    let mut tasks = std::fs::read_dir("/proc/self/task")?;
    tasks
        .next()
        .transpose()?
        .ok_or_else(|| io::Error::other("procfs task list is empty"))?;
    if tasks.next().transpose()?.is_some() {
        return Err(io::Error::other(
            "workspace sandbox must start before any runtime or client threads",
        ));
    }
    Ok(())
}

fn state_environment(mountpoint: &Path) -> io::Result<[(&'static str, PathBuf); 9]> {
    if !mountpoint.is_absolute() || mountpoint == Path::new("/") {
        return Err(io::Error::other(
            "workspace mountpoint must be an absolute non-root path",
        ));
    }
    let state = mountpoint.join(".state");
    Ok([
        ("HOME", state.join("home")),
        ("TMPDIR", state.join("tmp")),
        ("TMP", state.join("tmp")),
        ("TEMP", state.join("tmp")),
        ("XDG_CACHE_HOME", state.join("cache")),
        ("XDG_CONFIG_HOME", state.join("config")),
        ("XDG_DATA_HOME", state.join("data")),
        ("XDG_STATE_HOME", state.join("data")),
        ("XDG_RUNTIME_DIR", state.join("runtime")),
    ])
}

fn open_absolute(root: &OwnedFd, path: &Path) -> io::Result<OwnedFd> {
    let relative = path.strip_prefix("/").map_err(io::Error::other)?;
    open_beneath(root, relative)
}

fn open_beneath(parent: &OwnedFd, path: &Path) -> io::Result<OwnedFd> {
    rustix::fs::openat2(
        parent,
        path,
        OFlags::PATH | OFlags::DIRECTORY | OFlags::CLOEXEC,
        Mode::empty(),
        ResolveFlags::BENEATH | ResolveFlags::NO_MAGICLINKS | ResolveFlags::NO_SYMLINKS,
    )
    .map_err(Into::into)
}

fn drop_capabilities(last: u32) -> io::Result<()> {
    rustix::thread::set_capabilities_secure_bits(
        CapabilitiesSecureBits::NO_ROOT
            | CapabilitiesSecureBits::NO_ROOT_LOCKED
            | CapabilitiesSecureBits::NO_SETUID_FIXUP
            | CapabilitiesSecureBits::NO_SETUID_FIXUP_LOCKED
            | CapabilitiesSecureBits::KEEP_CAPS_LOCKED
            | CapabilitiesSecureBits::NO_CAP_AMBIENT_RAISE
            | CapabilitiesSecureBits::NO_CAP_AMBIENT_RAISE_LOCKED,
    )?;
    for capability in 0..=last {
        rustix::thread::remove_capability_from_bounding_set(CapabilitySet::from_bits_retain(
            1_u64 << capability,
        ))?;
    }
    rustix::thread::set_capabilities(
        None,
        CapabilitySets {
            effective: CapabilitySet::empty(),
            permitted: CapabilitySet::empty(),
            inheritable: CapabilitySet::empty(),
        },
    )?;
    Ok(())
}
