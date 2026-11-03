//! Descriptor-relative Linux operations. No backing descriptor is given to the kernel.
use std::ffi::{OsStr, OsString};
use std::fs::File;
use std::io;
use std::os::fd::AsRawFd;
use std::os::unix::ffi::{OsStrExt, OsStringExt};
use std::path::{Component, Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use fuser::{Errno, FileAttr, FileType, INodeNo, TimeOrNow};
use murvix_protocol::WorkspaceSnapshot;
use rustix::fs::{self, AtFlags, Mode, OFlags, ResolveFlags, Stat, Timespec, Timestamps};

use super::{Location, RootId};

pub(super) type Result<T, E = Errno> = std::result::Result<T, E>;

pub(super) fn errno(error: rustix::io::Errno) -> Errno {
    Errno::from_i32(error.raw_os_error())
}

pub(super) fn open_root(path: &Path) -> Result<File> {
    fs::openat2(
        fs::CWD,
        path,
        OFlags::PATH | OFlags::DIRECTORY | OFlags::CLOEXEC,
        Mode::empty(),
        ResolveFlags::NO_MAGICLINKS,
    )
    .map(File::from)
    .map_err(errno)
}

pub(super) fn beneath(root: &File, path: &Path, flags: OFlags, mode: Mode) -> Result<File> {
    fs::openat2(
        root,
        if path.as_os_str().is_empty() {
            Path::new(".")
        } else {
            path
        },
        flags | OFlags::CLOEXEC,
        mode,
        ResolveFlags::BENEATH | ResolveFlags::NO_MAGICLINKS,
    )
    .map(File::from)
    .map_err(errno)
}

pub(super) fn reopen(file: &File, flags: OFlags) -> Result<File> {
    // The only intentional magic-link open: the descriptor is owned and pinned
    // here, so this upgrades that exact O_PATH inode, never a guessed host path.
    // In particular O_TRUNC cannot race a replacement device or another inode.
    fs::open(
        format!("/proc/self/fd/{}", file.as_raw_fd()),
        (flags - OFlags::NOFOLLOW - OFlags::CREATE - OFlags::EXCL) | OFlags::CLOEXEC,
        Mode::empty(),
    )
    .map(File::from)
    .map_err(errno)
}

pub(super) fn chmod(file: &File, mode: u32) -> Result<()> {
    if kind(stat(file)?.st_mode)? == FileType::Symlink {
        return Err(Errno::EOPNOTSUPP);
    }
    // chmod follows this broker-owned descriptor alias, not the original
    // mutable pathname. Unlike opening the inode, this needs no read access.
    fs::chmod(
        format!("/proc/self/fd/{}", file.as_raw_fd()),
        Mode::from_bits_truncate(mode),
    )
    .map_err(errno)
}

pub(super) fn stat(file: &File) -> Result<Stat> {
    fs::fstat(file).map_err(errno)
}

pub(super) fn identity(stat: &Stat) -> (u64, u64) {
    (stat.st_dev, stat.st_ino)
}

pub(super) fn fd_path(file: &File) -> Result<PathBuf> {
    // This is a name query on an already pinned descriptor, never a pathname
    // authorization followed by an unconfined open of that pathname.
    std::fs::read_link(format!("/proc/self/fd/{}", file.as_raw_fd())).map_err(Errno::from)
}

pub(super) fn root_path<'a>(
    snapshot: &'a WorkspaceSnapshot,
    root: &'a RootId,
    state: &'a Path,
) -> Result<&'a Path> {
    match root {
        RootId::State => Ok(state),
        RootId::Catalog { key, path, primary } => {
            let entry = snapshot
                .roots
                .iter()
                .find(|entry| &entry.key == key)
                .ok_or(Errno::ESTALE)?;
            if entry.path.0 != *path || entry.primary != *primary {
                return Err(Errno::ESTALE);
            }
            if entry.error.is_some() {
                return Err(Errno::ENOENT);
            }
            Ok(Path::new(OsStr::from_bytes(&entry.path.0)))
        }
    }
}

pub(super) fn current_root(
    snapshot: &WorkspaceSnapshot,
    location: &Location,
    state: &Path,
) -> Result<File> {
    open_root(root_path(snapshot, &location.root, state)?)
}

pub(super) fn target(root: &File, location: &Location) -> Result<File> {
    beneath(
        root,
        &location.relative,
        OFlags::PATH | OFlags::NOFOLLOW,
        Mode::empty(),
    )
}

pub(super) fn parent(root: &File, location: &Location) -> Result<(File, OsString)> {
    let name = location
        .relative
        .file_name()
        .ok_or(Errno::EBUSY)?
        .to_owned();
    let directory = beneath(
        root,
        location.relative.parent().unwrap_or(Path::new("")),
        OFlags::PATH | OFlags::DIRECTORY,
        Mode::empty(),
    )?;
    Ok((directory, name))
}

pub(super) fn component(name: &OsStr) -> Result<()> {
    let bytes = name.as_bytes();
    if bytes.is_empty()
        || bytes == b"."
        || bytes == b".."
        || bytes.contains(&b'/')
        || bytes.contains(&0)
    {
        Err(Errno::EINVAL)
    } else {
        Ok(())
    }
}

fn normalized(path: &Path) -> Result<PathBuf> {
    let mut result = PathBuf::new();
    for component in path.components() {
        match component {
            Component::RootDir => result.push("/"),
            Component::CurDir => {}
            Component::Normal(name) => result.push(name),
            Component::ParentDir => {
                if !result.pop() {
                    return Err(Errno::EACCES);
                }
            }
            Component::Prefix(_) => return Err(Errno::EINVAL),
        }
    }
    Ok(result)
}

fn actual_extra(path: &Path) -> Result<PathBuf> {
    // Missing extra roots still reserve their location. Resolve the nearest
    // existing ancestor with a descriptor, without scanning any directory.
    let mut ancestor = path;
    let mut suffix = Vec::new();
    loop {
        match open_root(ancestor) {
            Ok(directory) => {
                let mut actual = fd_path(&directory)?;
                for name in suffix.iter().rev() {
                    actual.push(name);
                }
                return normalized(&actual);
            }
            Err(error) if error == Errno::ENOENT => {
                suffix.push(ancestor.file_name().ok_or(Errno::EACCES)?.to_owned());
                ancestor = ancestor.parent().ok_or(Errno::EACCES)?;
            }
            Err(error) => return Err(error),
        }
    }
}

pub(super) fn writable(
    snapshot: &WorkspaceSnapshot,
    location: &Location,
    root: &File,
    object: &File,
    descendants: bool,
) -> Result<()> {
    if matches!(location.root, RootId::Catalog { primary: false, .. }) {
        return Err(Errno::EROFS);
    }
    let value = stat(object)?;
    if value.st_nlink > 1 && !fs::FileType::from_raw_mode(value.st_mode).is_dir() {
        // An alternate name may lie below a protected extra root. Discovering
        // every hard-link alias would require a recursive inode index.
        return Err(Errno::EROFS);
    }
    let actual = fd_path(object)?;
    let base = fd_path(root)?;
    if !actual.starts_with(&base) {
        return Err(Errno::ESTALE);
    }
    for extra in snapshot.roots.iter().filter(|entry| !entry.primary) {
        let path = Path::new(OsStr::from_bytes(&extra.path.0));
        let protected = actual_extra(path)?;
        if actual.starts_with(&protected) || (descendants && protected.starts_with(&actual)) {
            return Err(Errno::EROFS);
        }
    }
    Ok(())
}

pub(super) fn writable_name(
    snapshot: &WorkspaceSnapshot,
    location: &Location,
    root: &File,
    directory: &File,
    name: &OsStr,
    descendants: bool,
) -> Result<()> {
    writable(snapshot, location, root, directory, false)?;
    match fs::statat(directory, name, AtFlags::SYMLINK_NOFOLLOW) {
        Ok(value) if value.st_nlink > 1 && !fs::FileType::from_raw_mode(value.st_mode).is_dir() => {
            return Err(Errno::EROFS);
        }
        Ok(_) | Err(rustix::io::Errno::NOENT) => {}
        Err(error) => return Err(errno(error)),
    }
    let actual = fd_path(directory)?.join(name);
    for extra in snapshot.roots.iter().filter(|entry| !entry.primary) {
        let protected = actual_extra(Path::new(OsStr::from_bytes(&extra.path.0)))?;
        if actual.starts_with(&protected) || (descendants && protected.starts_with(&actual)) {
            return Err(Errno::EROFS);
        }
    }
    Ok(())
}

pub(super) fn access(root: &File, relative: &Path, mask: u32) -> Result<()> {
    // The caller holds an O_PATH broker descriptor, which cannot back
    // accessat even with EMPTY_PATH. Check the same node through its
    // containing root with symlink-following semantics matching execve.
    fs::accessat(
        root,
        relative,
        fs::Access::from_bits_retain(mask),
        AtFlags::EACCESS,
    )
    .map_err(errno)
}

pub(super) fn validate_handle(root: &File, object: &File) -> Result<()> {
    if !fd_path(object)?.starts_with(fd_path(root)?) {
        return Err(Errno::ESTALE);
    }
    Ok(())
}

pub(super) fn kind(mode: u32) -> Result<FileType> {
    match mode & libc::S_IFMT {
        libc::S_IFREG => Ok(FileType::RegularFile),
        libc::S_IFDIR => Ok(FileType::Directory),
        libc::S_IFLNK => Ok(FileType::Symlink),
        libc::S_IFIFO => Ok(FileType::NamedPipe),
        libc::S_IFSOCK => Ok(FileType::Socket),
        libc::S_IFCHR => Ok(FileType::CharDevice),
        libc::S_IFBLK => Ok(FileType::BlockDevice),
        _ => Err(Errno::EOPNOTSUPP),
    }
}

fn time(seconds: i64, nanos: i64) -> SystemTime {
    let duration = Duration::new(seconds.unsigned_abs(), 0);
    let base = if seconds < 0 {
        UNIX_EPOCH.checked_sub(duration)
    } else {
        UNIX_EPOCH.checked_add(duration)
    };
    base.and_then(|base| {
        base.checked_add(Duration::from_nanos(
            u64::try_from(nanos.max(0)).unwrap_or(u64::MAX),
        ))
    })
    .unwrap_or(UNIX_EPOCH)
}

pub(super) fn attr(ino: INodeNo, value: &Stat) -> Result<FileAttr> {
    Ok(FileAttr {
        ino,
        size: u64::try_from(value.st_size.max(0)).unwrap_or(u64::MAX),
        blocks: u64::try_from(value.st_blocks.max(0)).unwrap_or(u64::MAX),
        atime: time(
            value.st_atime,
            i64::try_from(value.st_atime_nsec).unwrap_or(i64::MAX),
        ),
        mtime: time(
            value.st_mtime,
            i64::try_from(value.st_mtime_nsec).unwrap_or(i64::MAX),
        ),
        ctime: time(
            value.st_ctime,
            i64::try_from(value.st_ctime_nsec).unwrap_or(i64::MAX),
        ),
        crtime: UNIX_EPOCH,
        kind: kind(value.st_mode)?,
        perm: u16::try_from(value.st_mode & 0o7777).unwrap_or(0o777),
        nlink: u32::try_from(value.st_nlink).unwrap_or(u32::MAX),
        uid: value.st_uid,
        gid: value.st_gid,
        rdev: u32::try_from(value.st_rdev).unwrap_or(0),
        blksize: u32::try_from(value.st_blksize).unwrap_or(0),
        flags: 0,
    })
}

pub(super) fn virtual_attr(ino: INodeNo, writable: bool) -> Result<FileAttr> {
    let metadata = fs::statat(fs::CWD, ".", AtFlags::empty()).map_err(errno)?;
    let mut result = attr(ino, &metadata)?;
    result.kind = FileType::Directory;
    result.perm = if writable { 0o700 } else { 0o555 };
    result.size = 0;
    result.blocks = 0;
    result.nlink = 2;
    Ok(result)
}

pub(super) fn timestamps(atime: Option<TimeOrNow>, mtime: Option<TimeOrNow>) -> Result<Timestamps> {
    fn convert(value: Option<TimeOrNow>) -> Result<Timespec> {
        Ok(match value {
            None => Timespec {
                tv_sec: 0,
                tv_nsec: fs::UTIME_OMIT,
            },
            Some(TimeOrNow::Now) => Timespec {
                tv_sec: 0,
                tv_nsec: fs::UTIME_NOW,
            },
            Some(TimeOrNow::SpecificTime(value)) => {
                let nanos = match value.duration_since(UNIX_EPOCH) {
                    Ok(duration) => i128::try_from(duration.as_nanos()).unwrap_or(i128::MAX),
                    Err(error) => -i128::try_from(error.duration().as_nanos()).unwrap_or(i128::MAX),
                };
                Timespec {
                    tv_sec: nanos
                        .div_euclid(1_000_000_000)
                        .try_into()
                        .map_err(|_| Errno::EOVERFLOW)?,
                    tv_nsec: nanos.rem_euclid(1_000_000_000).try_into().map_err(|_| {
                        // A nanosecond remainder always fits, but stay explicit.
                        Errno::EOVERFLOW
                    })?,
                }
            }
        })
    }
    Ok(Timestamps {
        last_access: convert(atime)?,
        last_modification: convert(mtime)?,
    })
}

pub(super) fn safe_link(
    root: &File,
    location: &Location,
    logical: &Path,
    mountpoint: &Path,
    name: &OsStr,
) -> Result<Vec<u8>> {
    let (directory, link_name) = parent(root, location)?;
    let target = fs::readlinkat(&directory, &link_name, Vec::new()).map_err(errno)?;
    let target = PathBuf::from(OsString::from_vec(target.into_bytes()));
    let parent = location.relative.parent().unwrap_or(Path::new(""));
    let destination = if target.is_absolute() {
        let actual = fd_path(root)?;
        target
            .strip_prefix(&actual)
            .or_else(|_| target.strip_prefix(logical))
            .or_else(|_| target.strip_prefix(mountpoint.join(name)))
            .map(Path::to_path_buf)
            .map_err(|_| Errno::EACCES)?
    } else {
        parent.join(&target)
    };
    let destination = normalized(&destination)?;
    if destination.is_absolute() {
        return Err(Errno::EACCES);
    }
    if !target.is_absolute() {
        return Ok(target.into_os_string().into_vec());
    }
    // Translate permitted absolute host links into a relative namespace link.
    // Every intermediate link is independently subjected to the same check.
    let mut output = PathBuf::new();
    for _ in parent.components() {
        output.push("..");
    }
    output.push(if destination.as_os_str().is_empty() {
        Path::new(".")
    } else {
        &destination
    });
    Ok(output.into_os_string().into_vec())
}

pub(super) fn io_error(error: io::Error) -> Errno {
    Errno::from(error)
}
