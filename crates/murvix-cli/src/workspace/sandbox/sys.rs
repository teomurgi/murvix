//! Audited Linux UAPI boundary. Unsafe is confined to synchronous syscalls,
//! borrowing this single-threaded helper's stdio, and installing the immutable
//! seccomp program. All kernel buffers remain live for their complete calls;
//! returned owned descriptors are adopted exactly once.
#![allow(unsafe_code)]

use std::io;
use std::mem::size_of;
use std::os::fd::{AsFd, AsRawFd, BorrowedFd, FromRawFd, IntoRawFd, OwnedFd};
use std::ptr;

use rustix::fs::{Mode, OFlags, ResolveFlags};

const READONLY: u64 = 1;
const NOSUID: u64 = 2;
const NODEV: u64 = 4;
const AT_RECURSIVE: u32 = 0x8000;
const PIPEFS_MAGIC: i64 = 0x5049_5045;

const WRITE_FILE: u64 = 1 << 1;
const MAKE_CHAR: u64 = 1 << 6;
const MAKE_SOCK: u64 = 1 << 9;
const MAKE_BLOCK: u64 = 1 << 11;
const IOCTL_DEV: u64 = 1 << 15;
const RESOLVE_UNIX: u64 = 1 << 16;
// All ABI-9 filesystem rights except the three ambient read/execute rights.
const HANDLED_FS: u64 = ((1 << 17) - 1) & !((1 << 0) | (1 << 2) | (1 << 3));
const WORKSPACE_WRITES: u64 =
    HANDLED_FS & !(MAKE_CHAR | MAKE_BLOCK | MAKE_SOCK | IOCTL_DEV | RESOLVE_UNIX);

#[repr(C)]
struct MountAttr {
    attr_set: u64,
    attr_clr: u64,
    propagation: u64,
    userns_fd: u64,
}

#[repr(C)]
struct RulesetAttr {
    handled_access_fs: u64,
    handled_access_net: u64,
    scoped: u64,
}

#[repr(C, packed)]
struct PathBeneathAttr {
    allowed_access: u64,
    parent_fd: i32,
}

pub(super) fn prepare_stdio() -> io::Result<()> {
    // Reserve missing standard slots before opening any long-lived sandbox
    // descriptors, so an O_PATH root/ruleset fd can never become agent stdio.
    for number in 0..=2 {
        // SAFETY: F_GETFD accepts arbitrary fd numbers and retains no pointer.
        if unsafe { libc::fcntl(number, libc::F_GETFD) } >= 0 {
            continue;
        }
        let error = io::Error::last_os_error();
        if error.raw_os_error() != Some(libc::EBADF) {
            return Err(error);
        }
        let null = rustix::fs::open("/dev/null", OFlags::RDWR | OFlags::CLOEXEC, Mode::empty())?;
        if null.as_raw_fd() == number {
            rustix::io::fcntl_setfd(&null, rustix::io::FdFlags::empty())?;
            let _ = null.into_raw_fd(); // The standard slot owns it until exec.
        } else {
            // SAFETY: The owned null fd is live and number is a stdio slot.
            check(unsafe { libc::dup3(null.as_raw_fd(), number, 0) }.into())?;
        }
    }
    Ok(())
}

pub(super) fn require_landlock_abi() -> io::Result<()> {
    // SAFETY: VERSION queries take a null pointer and no attribute buffer.
    let abi = unsafe {
        libc::syscall(
            libc::SYS_landlock_create_ruleset,
            ptr::null::<u8>(),
            0_usize,
            1_u32,
        )
    };
    if abi < 0 {
        return Err(io::Error::other(format!(
            "workspace sandbox requires enabled Landlock ABI 9 or later (upstream Linux 7.1+): {}",
            io::Error::last_os_error()
        )));
    }
    if abi < 9 {
        return Err(io::Error::other(format!(
            "workspace sandbox requires Landlock ABI 9 for host UNIX-socket isolation; kernel provides ABI {abi}"
        )));
    }
    Ok(())
}

pub(super) fn enter_namespaces() -> io::Result<()> {
    // SAFETY: Caller verified that this fresh exec helper has exactly one
    // thread. No other thread can observe a changed fs/files/namespace table.
    unsafe {
        rustix::thread::unshare_unsafe(
            rustix::thread::UnshareFlags::NEWUSER
                | rustix::thread::UnshareFlags::NEWNS
                | rustix::thread::UnshareFlags::NEWIPC,
        )
    }
    .map_err(Into::into)
}

pub(super) fn protect_workspace_mount(mount: BorrowedFd<'_>) -> io::Result<()> {
    mount_attributes(
        mount.as_raw_fd(),
        c"",
        libc::AT_EMPTY_PATH as u32,
        NOSUID | NODEV,
        0,
    )
}

pub(super) fn make_host_tree_readonly() -> io::Result<()> {
    mount_attributes(libc::AT_FDCWD, c"/", AT_RECURSIVE, READONLY | NOSUID, 0)
}

fn mount_attributes(
    fd: i32,
    path: &std::ffi::CStr,
    flags: u32,
    set: u64,
    clear: u64,
) -> io::Result<()> {
    let attr = MountAttr {
        attr_set: set,
        attr_clr: clear,
        propagation: 0,
        userns_fd: 0,
    };
    // SAFETY: Both C path and initialized, correctly sized UAPI struct are live
    // throughout this synchronous call. No pointer is retained by the kernel.
    check(unsafe {
        libc::syscall(
            libc::SYS_mount_setattr,
            fd,
            path.as_ptr(),
            flags,
            &raw const attr,
            size_of::<MountAttr>(),
        )
    })
}

/// Replace inherited host file descriptions, not just their pathname views.
/// This prevents metadata operations on a pre-namespace fd from reaching a
/// still-writable host mount. Only anonymous pipes may keep their old fds.
pub(super) fn reopen_stdio(root: BorrowedFd<'_>) -> io::Result<()> {
    for number in 0..=2 {
        // SAFETY: We only borrow each process-owned stdio slot until dup3 below;
        // no reference survives that replacement and this helper has one thread.
        let original = unsafe { BorrowedFd::borrow_raw(number) };
        let stat = match rustix::fs::fstat(original) {
            Ok(stat) => stat,
            Err(rustix::io::Errno::BADF) => continue,
            Err(error) => return Err(error.into()),
        };
        let filesystem = rustix::fs::fstatfs(original)?;
        if filesystem.f_type as i64 == PIPEFS_MAGIC {
            continue;
        }
        let kind = stat.st_mode & libc::S_IFMT;
        if kind != libc::S_IFREG && kind != libc::S_IFCHR && kind != libc::S_IFIFO {
            return Err(io::Error::other(format!(
                "workspace sandbox refuses inherited control/socket descriptor {number}"
            )));
        }
        if kind == libc::S_IFCHR && !is_null_or_tty(original, stat.st_rdev) {
            return Err(io::Error::other(format!(
                "workspace sandbox stdio {number} must be a terminal, /dev/null, a pipe, or a regular file"
            )));
        }
        let target = std::fs::read_link(format!("/proc/self/fd/{number}"))?;
        let relative = target.strip_prefix("/").map_err(|_| {
            io::Error::other("workspace sandbox cannot safely reopen non-path stdio")
        })?;
        let flags = rustix::fs::fcntl_getfl(original)?;
        if flags.contains(OFlags::PATH) {
            return Err(io::Error::other(
                "workspace sandbox stdio must not be an O_PATH descriptor",
            ));
        }
        let descriptor_flags = rustix::io::fcntl_getfd(original)?;
        let reopened = rustix::fs::openat2(
            root,
            relative,
            flags | OFlags::CLOEXEC | OFlags::NOCTTY | OFlags::NONBLOCK,
            Mode::empty(),
            ResolveFlags::BENEATH | ResolveFlags::NO_MAGICLINKS,
        )?;
        let replacement = rustix::fs::fstat(&reopened)?;
        if replacement.st_dev != stat.st_dev
            || replacement.st_ino != stat.st_ino
            || replacement.st_rdev != stat.st_rdev
            || replacement.st_mode & libc::S_IFMT != kind
        {
            return Err(io::Error::other(
                "workspace sandbox stdio changed while reopening it",
            ));
        }
        rustix::fs::fcntl_setfl(&reopened, flags)?;
        // SAFETY: dup3 replaces the known stdio slot with our owned descriptor.
        // The borrowed original is not used after this call. reopened is still
        // owned separately and closed by RAII (or CLOEXEC if the number is low).
        check(unsafe { libc::dup3(reopened.as_raw_fd(), number, 0) }.into())?;
        // SAFETY: This borrows the replacement now occupying the same slot.
        let replacement = unsafe { BorrowedFd::borrow_raw(number) };
        rustix::io::fcntl_setfd(replacement, descriptor_flags)?;
    }
    Ok(())
}

fn is_null_or_tty(fd: BorrowedFd<'_>, device: libc::dev_t) -> bool {
    // /dev/ptmx is not a retained terminal: reopening it creates another pty.
    if device == libc::makedev(5, 2) {
        return false;
    }
    device == libc::makedev(1, 3)
        // SAFETY: isatty only inspects this live, borrowed descriptor.
        || unsafe { libc::isatty(fd.as_raw_fd()) } == 1
}

pub(super) fn landlock_ruleset(
    workspace: BorrowedFd<'_>,
    state: BorrowedFd<'_>,
) -> io::Result<OwnedFd> {
    let attr = RulesetAttr {
        handled_access_fs: HANDLED_FS,
        handled_access_net: 0,
        scoped: (1 << 0) | (1 << 1), // abstract UNIX sockets and signals (ABI 6)
    };
    // SAFETY: attr is the ABI-6+ 24-byte prefix, fully initialized and live.
    let raw = unsafe {
        libc::syscall(
            libc::SYS_landlock_create_ruleset,
            &raw const attr,
            size_of::<RulesetAttr>(),
            0_u32,
        )
    };
    if raw < 0 {
        return Err(io::Error::last_os_error());
    }
    let raw = i32::try_from(raw).map_err(io::Error::other)?;
    // SAFETY: A successful create_ruleset returns a new CLOEXEC descriptor.
    let ruleset = unsafe { OwnedFd::from_raw_fd(raw) };
    allow_path(ruleset.as_fd(), workspace, WORKSPACE_WRITES)?;
    allow_path(ruleset.as_fd(), state, MAKE_SOCK | RESOLVE_UNIX)?;
    let null = rustix::fs::open("/dev/null", OFlags::PATH | OFlags::CLOEXEC, Mode::empty())?;
    let null_stat = rustix::fs::fstat(&null)?;
    if null_stat.st_mode & libc::S_IFMT != libc::S_IFCHR || null_stat.st_rdev != libc::makedev(1, 3)
    {
        return Err(io::Error::other(
            "/dev/null is not the expected null device",
        ));
    }
    allow_path(ruleset.as_fd(), null.as_fd(), WRITE_FILE)?;
    match rustix::fs::open("/dev/tty", OFlags::PATH | OFlags::CLOEXEC, Mode::empty()) {
        Ok(tty) => {
            let stat = rustix::fs::fstat(&tty)?;
            if stat.st_mode & libc::S_IFMT != libc::S_IFCHR || stat.st_rdev != libc::makedev(5, 0) {
                return Err(io::Error::other(
                    "/dev/tty is not the expected controlling-terminal device",
                ));
            }
            allow_path(ruleset.as_fd(), tty.as_fd(), WRITE_FILE | IOCTL_DEV)?;
        }
        Err(rustix::io::Errno::NOENT) => {}
        Err(error) => return Err(error.into()),
    }
    for number in 0..=2 {
        // SAFETY: stdio slots are borrowed only within this iteration.
        let fd = unsafe { BorrowedFd::borrow_raw(number) };
        // SAFETY: isatty only inspects a descriptor; a closed slot returns 0.
        if unsafe { libc::isatty(number) } == 1 {
            allow_path(ruleset.as_fd(), fd, WRITE_FILE | IOCTL_DEV)?;
        }
    }
    Ok(ruleset)
}

fn allow_path(ruleset: BorrowedFd<'_>, path: BorrowedFd<'_>, access: u64) -> io::Result<()> {
    let attr = PathBeneathAttr {
        allowed_access: access,
        parent_fd: path.as_raw_fd(),
    };
    // SAFETY: This packed struct exactly matches the 12-byte Linux UAPI. The
    // syscall copies it synchronously without retaining either borrowed fd.
    check(unsafe {
        libc::syscall(
            libc::SYS_landlock_add_rule,
            ruleset.as_raw_fd(),
            1_u32,
            &raw const attr,
            0_u32,
        )
    })
}

pub(super) fn enforce_landlock(ruleset: BorrowedFd<'_>) -> io::Result<()> {
    // SAFETY: Valid ruleset fd; caller has already enabled no_new_privs.
    check(unsafe { libc::syscall(libc::SYS_landlock_restrict_self, ruleset.as_raw_fd(), 0_u32) })
}

pub(super) fn cloexec_non_stdio() -> io::Result<()> {
    // SAFETY: CLOEXEC changes descriptor flags, not ownership. This single-
    // threaded helper cannot race fd creation and no inherited fd >= 3 reaches
    // arbitrary executable code, including broker sockets and original host fds.
    check(unsafe { libc::syscall(libc::SYS_close_range, 3_u32, u32::MAX, 1_u32 << 2) })
}

pub(super) fn install_seccomp() -> io::Result<()> {
    #[cfg(target_arch = "x86_64")]
    const ARCH: u32 = 0xc000_003e;
    #[cfg(target_arch = "aarch64")]
    const ARCH: u32 = 0xc000_00b7;
    #[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
    return Err(io::Error::other(
        "workspace sandbox seccomp supports Linux x86-64 and AArch64 only",
    ));

    #[cfg(any(target_arch = "x86_64", target_arch = "aarch64"))]
    {
        const LOAD: u16 = 0x20; // BPF_LD | BPF_W | BPF_ABS
        const EQUAL: u16 = 0x15; // BPF_JMP | BPF_JEQ | BPF_K
        const SET: u16 = 0x45; // BPF_JMP | BPF_JSET | BPF_K
        const RET: u16 = 0x06;
        const ALLOW: u32 = 0x7fff_0000;
        const KILL: u32 = 0x8000_0000;
        const DENY: u32 = 0x0005_0000 | libc::EPERM as u32;
        const NOSYS: u32 = 0x0005_0000 | libc::ENOSYS as u32;
        let statement = |code, k| libc::sock_filter {
            code,
            jt: 0,
            jf: 0,
            k,
        };
        let jump = |code, k, jt, jf| libc::sock_filter { code, jt, jf, k };
        let denied = denied_syscalls();
        let mut filter = Vec::with_capacity(denied.len() * 2 + 32);
        filter.extend([
            statement(LOAD, 4), // struct seccomp_data.arch
            jump(EQUAL, ARCH, 1, 0),
            statement(RET, KILL),
            statement(LOAD, 0), // struct seccomp_data.nr
        ]);
        // x32 shares AUDIT_ARCH_X86_64 but uses a different syscall table.
        #[cfg(target_arch = "x86_64")]
        filter.extend([jump(SET, 0x4000_0000, 0, 1), statement(RET, KILL)]);
        for syscall in denied {
            filter.push(jump(
                EQUAL,
                u32::try_from(syscall).map_err(io::Error::other)?,
                0,
                1,
            ));
            filter.push(statement(RET, DENY));
        }
        // clone3 arguments live behind a mutable userspace pointer. Returning
        // ENOSYS makes libc use clone, whose namespace flags we can safely read.
        // io_uring is disabled to prevent alternate submission paths bypassing
        // the syscall argument checks (notably device ioctls).
        for syscall in [libc::SYS_clone3, libc::SYS_io_uring_setup] {
            filter.push(jump(
                EQUAL,
                u32::try_from(syscall).map_err(io::Error::other)?,
                0,
                1,
            ));
            filter.push(statement(RET, NOSYS));
        }
        let namespaces = u32::try_from(
            libc::CLONE_NEWUSER
                | libc::CLONE_NEWNS
                | libc::CLONE_NEWIPC
                | libc::CLONE_NEWNET
                | libc::CLONE_NEWPID
                | libc::CLONE_NEWUTS
                | libc::CLONE_NEWCGROUP,
        )
        .map_err(io::Error::other)?;
        filter.extend([
            jump(
                EQUAL,
                u32::try_from(libc::SYS_clone).map_err(io::Error::other)?,
                0,
                4,
            ),
            statement(LOAD, 16), // clone arg0, low 32 bits (supported LE ABIs)
            jump(SET, namespaces, 0, 1),
            statement(RET, DENY),
            statement(RET, ALLOW),
            jump(
                EQUAL,
                u32::try_from(libc::SYS_ioctl).map_err(io::Error::other)?,
                0,
                5,
            ),
            statement(LOAD, 24),       // ioctl arg1, low 32 bits
            jump(EQUAL, 0x5412, 2, 0), // TIOCSTI: do not inject host tty input
            jump(EQUAL, 0x541c, 1, 0), // TIOCLINUX: virtual-console control
            statement(RET, ALLOW),
            statement(RET, DENY),
            statement(RET, ALLOW),
        ]);
        let program = libc::sock_fprog {
            len: u16::try_from(filter.len()).map_err(io::Error::other)?,
            filter: filter.as_mut_ptr(),
        };
        // SAFETY: A live, initialized cBPF program is synchronously copied by
        // seccomp. no_new_privs is already set; the filter is inherited and
        // cannot subsequently be removed by this process or any descendant.
        check(unsafe { libc::syscall(libc::SYS_seccomp, 1_u32, 0_u32, &raw const program) })
    }
}

/// Mount-namespace escape and introspection syscalls, denied with `EPERM`.
fn denied_syscalls() -> [libc::c_long; 25] {
    [
        libc::SYS_mount,
        libc::SYS_umount2,
        libc::SYS_pivot_root,
        libc::SYS_open_tree,
        libc::SYS_move_mount,
        libc::SYS_mount_setattr,
        libc::SYS_fsopen,
        libc::SYS_fsconfig,
        libc::SYS_fsmount,
        libc::SYS_fspick,
        libc::SYS_setns,
        libc::SYS_unshare,
        libc::SYS_ptrace,
        libc::SYS_process_vm_readv,
        libc::SYS_process_vm_writev,
        libc::SYS_pidfd_getfd,
        libc::SYS_open_by_handle_at,
        libc::SYS_name_to_handle_at,
        libc::SYS_bpf,
        libc::SYS_perf_event_open,
        libc::SYS_keyctl,
        libc::SYS_add_key,
        libc::SYS_request_key,
        libc::SYS_io_uring_enter,
        libc::SYS_io_uring_register,
    ]
}

fn check(result: libc::c_long) -> io::Result<()> {
    if result < 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}
