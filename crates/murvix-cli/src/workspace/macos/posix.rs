//! Darwin operations never follow a symlink while traversing a selected root.
#![allow(unsafe_code)]

use std::ffi::{CString, OsStr, OsString};
use std::fs::File;
use std::io;
use std::os::fd::{AsRawFd, FromRawFd};
use std::os::unix::ffi::{OsStrExt, OsStringExt};
use std::os::unix::fs::MetadataExt;
use std::path::{Component, Path, PathBuf};

fn string(value: &OsStr) -> io::Result<CString> {
    CString::new(value.as_bytes()).map_err(|_| io::Error::from_raw_os_error(libc::EINVAL))
}

pub(super) fn component(name: &[u8]) -> io::Result<&OsStr> {
    if name.is_empty() || name == b"." || name == b".." || name.contains(&0) || name.contains(&b'/')
    {
        return Err(io::Error::from_raw_os_error(libc::EINVAL));
    }
    Ok(OsStr::from_bytes(name))
}

fn descriptor(fd: i32) -> io::Result<File> {
    if fd < 0 {
        Err(io::Error::last_os_error())
    } else {
        // SAFETY: a successful open returns a new descriptor owned by this call.
        Ok(unsafe { File::from_raw_fd(fd) })
    }
}

fn result(value: i32) -> io::Result<()> {
    if value < 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}

pub(super) fn root(path: &Path) -> io::Result<File> {
    let path = string(path.as_os_str())?;
    // Root selection itself is authoritative; intermediate components below it
    // are resolved only with openat and O_NOFOLLOW.
    descriptor(unsafe {
        libc::open(
            path.as_ptr(),
            libc::O_RDONLY | libc::O_DIRECTORY | libc::O_CLOEXEC,
        )
    })
}

pub(super) fn at(directory: &File, name: &OsStr, flags: i32, mode: u32) -> io::Result<File> {
    let name = string(name)?;
    descriptor(unsafe {
        libc::openat(
            directory.as_raw_fd(),
            name.as_ptr(),
            flags | libc::O_CLOEXEC | libc::O_NOFOLLOW | libc::O_NONBLOCK,
            mode as libc::mode_t,
        )
    })
}

pub(super) fn beneath(root: &File, relative: &Path, flags: i32) -> io::Result<File> {
    let mut directory = root.try_clone()?;
    let parts: Vec<_> = relative.components().collect();
    for (index, part) in parts.iter().enumerate() {
        let Component::Normal(name) = part else {
            return Err(io::Error::from_raw_os_error(libc::EACCES));
        };
        directory = at(
            &directory,
            name,
            if index + 1 == parts.len() {
                flags
            } else {
                libc::O_RDONLY | libc::O_DIRECTORY
            },
            0,
        )?;
    }
    Ok(directory)
}

pub(super) fn same(a: &File, b: &File) -> io::Result<bool> {
    let a = a.metadata()?;
    let b = b.metadata()?;
    Ok(a.dev() == b.dev() && a.ino() == b.ino())
}

pub(super) fn path(file: &File) -> io::Result<PathBuf> {
    let mut bytes = [0_u8; libc::PATH_MAX as usize];
    result(unsafe { libc::fcntl(file.as_raw_fd(), libc::F_GETPATH, bytes.as_mut_ptr()) })?;
    let length = bytes
        .iter()
        .position(|byte| *byte == 0)
        .ok_or_else(|| io::Error::from_raw_os_error(libc::ENAMETOOLONG))?;
    Ok(PathBuf::from(OsString::from_vec(bytes[..length].to_vec())))
}

pub(super) fn names(directory: &File) -> io::Result<Vec<Vec<u8>>> {
    use std::os::fd::IntoRawFd;
    // Reopen '.' rather than dup: fdopendir/readdir must not share seek offsets
    // with another concurrent directory enumeration.
    let fd = at(
        directory,
        OsStr::new("."),
        libc::O_RDONLY | libc::O_DIRECTORY,
        0,
    )?
    .into_raw_fd();
    let stream = unsafe { libc::fdopendir(fd) };
    if stream.is_null() {
        let error = io::Error::last_os_error();
        unsafe {
            libc::close(fd);
        }
        return Err(error);
    }
    struct Directory(*mut libc::DIR);
    impl Drop for Directory {
        fn drop(&mut self) {
            unsafe {
                libc::closedir(self.0);
            }
        }
    }
    let stream = Directory(stream);
    let mut names = Vec::new();
    loop {
        unsafe {
            *libc::__error() = 0;
        }
        let entry = unsafe { libc::readdir(stream.0) };
        if entry.is_null() {
            let error = io::Error::last_os_error();
            if error.raw_os_error() != Some(0) {
                return Err(error);
            }
            break;
        }
        let entry = unsafe { &*entry };
        let bytes = unsafe {
            std::slice::from_raw_parts(
                entry.d_name.as_ptr().cast::<u8>(),
                usize::from(entry.d_namlen),
            )
        };
        if bytes != b"." && bytes != b".." {
            names.push(bytes.to_vec());
        }
    }
    names.sort();
    Ok(names)
}

pub(super) fn mkdir(directory: &File, name: &OsStr, mode: u32) -> io::Result<()> {
    let name = string(name)?;
    result(unsafe { libc::mkdirat(directory.as_raw_fd(), name.as_ptr(), mode as libc::mode_t) })
}

pub(super) fn remove(directory: &File, name: &OsStr, is_directory: bool) -> io::Result<()> {
    let name = string(name)?;
    result(unsafe {
        libc::unlinkat(
            directory.as_raw_fd(),
            name.as_ptr(),
            if is_directory { libc::AT_REMOVEDIR } else { 0 },
        )
    })
}

pub(super) fn rename(from: &File, name: &OsStr, to: &File, target: &OsStr) -> io::Result<()> {
    let name = string(name)?;
    let target = string(target)?;
    result(unsafe {
        libc::renameat(
            from.as_raw_fd(),
            name.as_ptr(),
            to.as_raw_fd(),
            target.as_ptr(),
        )
    })
}

pub(super) fn symlink(directory: &File, name: &OsStr, target: &[u8]) -> io::Result<()> {
    let name = string(name)?;
    let target = CString::new(target).map_err(|_| io::Error::from_raw_os_error(libc::EINVAL))?;
    result(unsafe { libc::symlinkat(target.as_ptr(), directory.as_raw_fd(), name.as_ptr()) })
}

pub(super) fn readlink(directory: &File, name: &OsStr) -> io::Result<Vec<u8>> {
    let name = string(name)?;
    let mut bytes = vec![0_u8; libc::PATH_MAX as usize];
    let count = unsafe {
        libc::readlinkat(
            directory.as_raw_fd(),
            name.as_ptr(),
            bytes.as_mut_ptr().cast(),
            bytes.len(),
        )
    };
    if count < 0 {
        return Err(io::Error::last_os_error());
    }
    if count as usize == bytes.len() {
        return Err(io::Error::from_raw_os_error(libc::ENAMETOOLONG));
    }
    bytes.truncate(count as usize);
    Ok(bytes)
}

pub(super) fn chmod(file: &File, mode: u32) -> io::Result<()> {
    result(unsafe { libc::fchmod(file.as_raw_fd(), (mode & 0o7777) as libc::mode_t) })
}

pub(super) fn chown(file: &File, uid: Option<u32>, gid: Option<u32>) -> io::Result<()> {
    result(unsafe {
        libc::fchown(
            file.as_raw_fd(),
            uid.unwrap_or(u32::MAX),
            gid.unwrap_or(u32::MAX),
        )
    })
}

pub(super) fn times(file: &File, times: [libc::timespec; 2]) -> io::Result<()> {
    result(unsafe { libc::futimens(file.as_raw_fd(), times.as_ptr()) })
}

pub(super) fn statfs(file: &File) -> io::Result<libc::statfs> {
    let mut result_stat = std::mem::MaybeUninit::uninit();
    result(unsafe { libc::fstatfs(file.as_raw_fd(), result_stat.as_mut_ptr()) })?;
    Ok(unsafe { result_stat.assume_init() })
}

const CREATE_VERIFIER: &[u8] = b"com.murvix.workspace.createverf\0";
pub(super) fn set_verifier(file: &File, verifier: &[u8; 8]) -> io::Result<()> {
    result(unsafe {
        libc::fsetxattr(
            file.as_raw_fd(),
            CREATE_VERIFIER.as_ptr().cast(),
            verifier.as_ptr().cast(),
            8,
            0,
            0,
        )
    })
}
pub(super) fn verifier(file: &File) -> io::Result<[u8; 8]> {
    let mut bytes = [0; 8];
    let count = unsafe {
        libc::fgetxattr(
            file.as_raw_fd(),
            CREATE_VERIFIER.as_ptr().cast(),
            bytes.as_mut_ptr().cast(),
            8,
            0,
            0,
        )
    };
    if count != 8 {
        return Err(if count < 0 {
            io::Error::last_os_error()
        } else {
            io::Error::from_raw_os_error(libc::EINVAL)
        });
    }
    Ok(bytes)
}
