//! Live NFSv3 broker over [`Binding`] snapshots. Every operation fetches
//! current membership first; removed roots fail instead of using cached paths.
//! The primary root is writable, added roots are read-only, and `.state`
//! backs the isolated agent home. No agent-specific tools or plugins.

use std::collections::HashMap;
use std::ffi::OsStr;
use std::fs::File;
use std::io;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};

use nfs3_server::vfs::{
    DirEntryPlus, FileHandleU64, NextResult, NfsFileSystem, NfsReadFileSystem, ReadDirIterator,
    ReadDirPlusIterator, VFSCapabilities,
};
use nfs3_types::nfs3::{
    FSSTAT3resok, PATHCONF3resok, createverf3, fattr3, fileid3, filename3, ftype3, mode3, nfspath3,
    nfsstat3, nfstime3, post_op_attr, sattr3, set_atime, set_gid3, set_mode3, set_mtime, set_size3,
    set_uid3, specdata3, stable_how,
};
use nfs3_types::rpc::auth_unix;
use nfs3_types::xdr_codec::Opaque;

use super::super::Binding;
use super::posix;

type Result<T, E = nfsstat3> = std::result::Result<T, E>;

/// Reserved handle values. Table entries start above these.
const ROOT: u64 = 1;
const MANIFEST: u64 = 2;
const FIRST_DYNAMIC: u64 = 3;
/// The manifest is rendered per open; bound each serialized copy and the
/// total held across concurrent opens like the Linux frontend.
const MAX_MANIFEST_BYTES: usize = 256 * 1024;
const MAX_MANIFEST_MEMORY: usize = 4 * 1024 * 1024;
/// Cap live handle mappings; eviction turns old handles stale explicitly.
const MAX_HANDLES: usize = 65_536;

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
enum RootId {
    State,
    Manifest,
    Catalog {
        key: String,
        path: Vec<u8>,
        primary: bool,
    },
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
struct Location {
    root: RootId,
    relative: PathBuf,
}

struct Table {
    next: u64,
    by_id: HashMap<u64, Location>,
    by_location: HashMap<Location, u64>,
    manifest_bytes: usize,
}

impl Table {
    fn insert(&mut self, location: Location) -> u64 {
        if let Some(id) = self.by_location.get(&location).copied() {
            return id;
        }
        if self.by_id.len() >= MAX_HANDLES {
            self.by_id.clear();
            self.by_location.clear();
        }
        let id = self.next;
        self.next = self.next.checked_add(1).unwrap_or(FIRST_DYNAMIC);
        self.by_id.insert(id, location.clone());
        self.by_location.insert(location, id);
        id
    }
}

pub struct Filesystem {
    binding: Binding,
    state: PathBuf,
    table: MutexTable,
}

type MutexTable = std::sync::Mutex<Table>;

impl Filesystem {
    pub(super) fn new(
        binding: Binding,
        state: PathBuf,
        _mountpoint: PathBuf,
    ) -> io::Result<std::sync::Arc<Self>> {
        posix::root(&state)?;
        Ok(std::sync::Arc::new(Self {
            binding,
            state,
            table: std::sync::Mutex::new(Table {
                next: FIRST_DYNAMIC,
                by_id: HashMap::new(),
                by_location: HashMap::new(),
                manifest_bytes: 0,
            }),
        }))
    }

    fn snapshot(&self) -> Result<murvix_protocol::WorkspaceSnapshot> {
        self.binding.snapshot().map_err(|_| nfsstat3::NFS3ERR_IO)
    }

    fn table(&self) -> std::sync::MutexGuard<'_, Table> {
        // The broker thread owns this table; a poisoned lock cannot serve.
        self.table.lock().unwrap_or_else(|error| error.into_inner())
    }

    fn root_path(
        &self,
        snapshot: &murvix_protocol::WorkspaceSnapshot,
        root: &RootId,
    ) -> Result<PathBuf> {
        match root {
            RootId::State => Ok(self.state.clone()),
            RootId::Manifest => Err(nfsstat3::NFS3ERR_ROFS),
            RootId::Catalog { key, path, primary } => {
                let entry = snapshot
                    .roots
                    .iter()
                    .find(|entry| &entry.key == key)
                    .ok_or(nfsstat3::NFS3ERR_STALE)?;
                if entry.path.0 != *path || entry.primary != *primary {
                    return Err(nfsstat3::NFS3ERR_STALE);
                }
                if entry.error.is_some() {
                    return Err(nfsstat3::NFS3ERR_NOENT);
                }
                Ok(PathBuf::from(OsStr::from_bytes(&entry.path.0)))
            }
        }
    }

    fn open_root(
        &self,
        snapshot: &murvix_protocol::WorkspaceSnapshot,
        root: &RootId,
    ) -> Result<File> {
        posix::root(&self.root_path(snapshot, root)?).map_err(|_| nfsstat3::NFS3ERR_IO)
    }

    fn resolve(&self, snapshot: &murvix_protocol::WorkspaceSnapshot, id: u64) -> Result<Location> {
        if id == ROOT || id == MANIFEST {
            return Err(nfsstat3::NFS3ERR_BADHANDLE);
        }
        let table = self.table();
        let location = table
            .by_id
            .get(&id)
            .cloned()
            .ok_or(nfsstat3::NFS3ERR_STALE)?;
        if matches!(location.root, RootId::Manifest) {
            return Ok(location);
        }
        // Membership is revalidated against the current snapshot on every use.
        self.root_path(snapshot, &location.root)?;
        Ok(location)
    }

    fn child(
        &self,
        snapshot: &murvix_protocol::WorkspaceSnapshot,
        dirid: &FileHandleU64,
        name: &[u8],
    ) -> Result<Location> {
        let name = posix::component(name).map_err(|_| nfsstat3::NFS3ERR_INVAL)?;
        if dirid.as_u64() == ROOT {
            if name == "workspace.json" {
                return Ok(Location {
                    root: RootId::Manifest,
                    relative: PathBuf::new(),
                });
            }
            if name == ".state" {
                return Ok(Location {
                    root: RootId::State,
                    relative: PathBuf::new(),
                });
            }
            let catalog = snapshot
                .roots
                .iter()
                .find(|root| {
                    (root.primary && name == "primary")
                        || (!root.primary && name == OsStr::new(&root.key))
                })
                .ok_or(nfsstat3::NFS3ERR_NOENT)?;
            return Ok(Location {
                root: RootId::Catalog {
                    key: catalog.key.clone(),
                    path: catalog.path.0.clone(),
                    primary: catalog.primary,
                },
                relative: PathBuf::new(),
            });
        }
        if dirid.as_u64() == MANIFEST {
            return Err(nfsstat3::NFS3ERR_NOTDIR);
        }
        let parent = self.resolve(snapshot, dirid.as_u64())?;
        if matches!(parent.root, RootId::Manifest) {
            return Err(nfsstat3::NFS3ERR_NOTDIR);
        }
        let mut relative = parent.relative.clone();
        relative.push(name);
        Ok(Location {
            root: parent.root,
            relative,
        })
    }

    fn manifest_bytes(&self, snapshot: &murvix_protocol::WorkspaceSnapshot) -> Result<Vec<u8>> {
        let mut roots = Vec::with_capacity(snapshot.roots.len());
        for root in &snapshot.roots {
            let path = String::from_utf8(root.path.0.clone()).ok();
            roots.push(serde_json::json!({
                "directory": if root.primary { "primary".to_owned() } else { root.key.clone() },
                "key": root.key,
                "primary": root.primary,
                "path_bytes": root.path.0,
                "path": path,
                "error": root.error,
            }));
        }
        let bytes = serde_json::to_vec(&serde_json::json!({
            "server": snapshot.server.to_string(),
            "session": snapshot.session.get(),
            "project": snapshot.project.to_string(),
            "revision": snapshot.revision,
            "roots": roots,
        }))
        .map_err(|_| nfsstat3::NFS3ERR_IO)?;
        if bytes.len() > MAX_MANIFEST_BYTES {
            return Err(nfsstat3::NFS3ERR_FBIG);
        }
        Ok(bytes)
    }

    fn location_attr(
        &self,
        snapshot: &murvix_protocol::WorkspaceSnapshot,
        location: &Location,
    ) -> Result<fattr3> {
        if matches!(location.root, RootId::Manifest) {
            return Err(nfsstat3::NFS3ERR_ROFS);
        }
        let root = self.open_root(snapshot, &location.root)?;
        let file = if location.relative.as_os_str().is_empty() {
            root
        } else {
            posix::beneath(
                &root,
                &location.relative,
                libc::O_RDONLY | libc::O_DIRECTORY,
            )
            .or_else(|_| posix::beneath(&root, &location.relative, libc::O_RDONLY))
            .map_err(io_to_nfs)?
        };
        self.file_attr(&file)
    }

    fn file_attr(&self, file: &File) -> Result<fattr3> {
        let metadata = file.metadata().map_err(|_| nfsstat3::NFS3ERR_IO)?;
        let kind = metadata.file_type();
        let type_ = if kind.is_dir() {
            ftype3::NF3DIR
        } else if kind.is_symlink() {
            ftype3::NF3LNK
        } else if kind.is_file() {
            ftype3::NF3REG
        } else {
            return Err(nfsstat3::NFS3ERR_INVAL);
        };
        Ok(fattr3 {
            type_,
            mode: metadata.mode() & 0o7777,
            nlink: u32::try_from(metadata.nlink()).unwrap_or(u32::MAX),
            uid: metadata.uid(),
            gid: metadata.gid(),
            size: metadata.len(),
            used: metadata.blocks() * 512,
            rdev: specdata3 {
                specdata1: 0,
                specdata2: 0,
            },
            fsid: metadata.dev(),
            fileid: metadata.ino(),
            atime: to_nfs_time(metadata.atime(), metadata.atime_nsec()),
            mtime: to_nfs_time(metadata.mtime(), metadata.mtime_nsec()),
            ctime: to_nfs_time(metadata.ctime(), metadata.ctime_nsec()),
        })
    }

    fn writable(
        &self,
        snapshot: &murvix_protocol::WorkspaceSnapshot,
        location: &Location,
    ) -> Result<File> {
        if matches!(location.root, RootId::Catalog { primary: false, .. }) {
            return Err(nfsstat3::NFS3ERR_ROFS);
        }
        if matches!(location.root, RootId::Manifest) {
            return Err(nfsstat3::NFS3ERR_ROFS);
        }
        let root = self.open_root(snapshot, &location.root)?;
        let actual = actual_path(&root, &location.relative)?;
        for extra in snapshot.roots.iter().filter(|entry| !entry.primary) {
            let protected = actual_path(
                &posix::root(&PathBuf::from(OsStr::from_bytes(&extra.path.0)))
                    .map_err(|_| nfsstat3::NFS3ERR_IO)?,
                Path::new(""),
            )?;
            if actual.starts_with(&protected) {
                return Err(nfsstat3::NFS3ERR_ROFS);
            }
        }
        Ok(root)
    }
}

fn io_to_nfs(error: io::Error) -> nfsstat3 {
    match error.raw_os_error() {
        Some(code) if code == libc::ENOENT => nfsstat3::NFS3ERR_NOENT,
        Some(code) if code == libc::EACCES || code == libc::EPERM => nfsstat3::NFS3ERR_ACCES,
        Some(code) if code == libc::EEXIST => nfsstat3::NFS3ERR_EXIST,
        Some(code) if code == libc::ENOTDIR => nfsstat3::NFS3ERR_NOTDIR,
        Some(code) if code == libc::EISDIR => nfsstat3::NFS3ERR_ISDIR,
        Some(code) if code == libc::ENOTEMPTY => nfsstat3::NFS3ERR_NOTEMPTY,
        Some(code) if code == libc::ENAMETOOLONG => nfsstat3::NFS3ERR_NAMETOOLONG,
        Some(code) if code == libc::EXDEV => nfsstat3::NFS3ERR_XDEV,
        Some(code) if code == libc::EROFS => nfsstat3::NFS3ERR_ROFS,
        Some(code) if code == libc::EFBIG => nfsstat3::NFS3ERR_FBIG,
        Some(code) if code == libc::ENOSPC => nfsstat3::NFS3ERR_NOSPC,
        Some(code) if code == libc::EDQUOT => nfsstat3::NFS3ERR_DQUOT,
        Some(code) if code == libc::EINVAL => nfsstat3::NFS3ERR_INVAL,
        _ => nfsstat3::NFS3ERR_IO,
    }
}

fn to_nfs_time(seconds: i64, nanos: i64) -> nfstime3 {
    nfstime3 {
        seconds: u32::try_from(seconds.max(0)).unwrap_or(u32::MAX),
        nseconds: u32::try_from(nanos.clamp(0, u32::MAX as i64)).unwrap_or(u32::MAX),
    }
}

/// Nearest existing ancestor with its on-disk path, without scanning trees.
fn actual_path(root: &File, relative: &Path) -> Result<PathBuf> {
    let mut ancestor = relative.to_path_buf();
    let mut suffix = Vec::new();
    loop {
        match posix::beneath(root, &ancestor, libc::O_RDONLY | libc::O_DIRECTORY) {
            Ok(directory) => {
                let mut actual = posix::path(&directory).map_err(|_| nfsstat3::NFS3ERR_IO)?;
                for part in suffix.iter().rev() {
                    actual.push(part);
                }
                return Ok(actual);
            }
            Err(error) if error.raw_os_error() == Some(libc::ENOENT) => {
                let name = ancestor
                    .file_name()
                    .ok_or(nfsstat3::NFS3ERR_ACCES)?
                    .to_owned();
                suffix.push(name);
                if !ancestor.pop() {
                    return Err(nfsstat3::NFS3ERR_ACCES);
                }
            }
            Err(error) if error.raw_os_error() == Some(libc::ENOTDIR) => {
                let file = posix::beneath(root, &ancestor, libc::O_RDONLY).map_err(io_to_nfs)?;
                let mut actual = posix::path(&file).map_err(|_| nfsstat3::NFS3ERR_IO)?;
                for part in suffix.iter().rev() {
                    actual.push(part);
                }
                return Ok(actual);
            }
            Err(error) => return Err(io_to_nfs(error)),
        }
    }
}

fn into_filename(bytes: Vec<u8>) -> filename3<'static> {
    filename3(Opaque::owned(bytes))
}

struct PlusEntries {
    entries: Vec<DirEntryPlus<FileHandleU64>>,
    index: usize,
}

impl ReadDirPlusIterator<FileHandleU64> for PlusEntries {
    async fn next(&mut self) -> NextResult<DirEntryPlus<FileHandleU64>> {
        if self.index < self.entries.len() {
            let entry = self.entries[self.index].clone();
            self.index += 1;
            NextResult::Ok(entry)
        } else {
            NextResult::Eof
        }
    }
}

struct SimpleEntries {
    entries: Vec<(fileid3, Vec<u8>)>,
    index: usize,
}

impl ReadDirIterator for SimpleEntries {
    async fn next(&mut self) -> NextResult<nfs3_server::vfs::DirEntry> {
        use nfs3_server::vfs::DirEntry;
        if self.index < self.entries.len() {
            let (fileid, name) = self.entries[self.index].clone();
            self.index += 1;
            NextResult::Ok(DirEntry {
                fileid,
                name: into_filename(name),
                cookie: self.index as u64,
            })
        } else {
            NextResult::Eof
        }
    }
}

fn virtual_dir_attr(fileid: u64) -> fattr3 {
    fattr3 {
        type_: ftype3::NF3DIR,
        mode: 0o555,
        nlink: 2,
        uid: 0,
        gid: 0,
        size: 0,
        used: 0,
        rdev: specdata3 {
            specdata1: 0,
            specdata2: 0,
        },
        fsid: 0,
        fileid,
        atime: nfstime3 {
            seconds: 0,
            nseconds: 0,
        },
        mtime: nfstime3 {
            seconds: 0,
            nseconds: 0,
        },
        ctime: nfstime3 {
            seconds: 0,
            nseconds: 0,
        },
    }
}

fn virtual_file_attr(fileid: u64, size: u64) -> fattr3 {
    fattr3 {
        type_: ftype3::NF3REG,
        mode: 0o444,
        nlink: 1,
        uid: 0,
        gid: 0,
        size,
        used: size,
        rdev: specdata3 {
            specdata1: 0,
            specdata2: 0,
        },
        fsid: 0,
        fileid,
        atime: nfstime3 {
            seconds: 0,
            nseconds: 0,
        },
        mtime: nfstime3 {
            seconds: 0,
            nseconds: 0,
        },
        ctime: nfstime3 {
            seconds: 0,
            nseconds: 0,
        },
    }
}

impl NfsReadFileSystem for Filesystem {
    type Handle = FileHandleU64;

    fn root_dir(&self) -> Self::Handle {
        FileHandleU64::new(ROOT)
    }

    fn authorize(&self, auth: Option<&auth_unix>) -> bool {
        let _ = auth;
        true
    }

    async fn lookup(
        &self,
        dirid: &Self::Handle,
        filename: &filename3<'_>,
    ) -> Result<Self::Handle, nfsstat3> {
        let snapshot = self.snapshot()?;
        if dirid.as_u64() == ROOT && filename.0.as_ref() == b"workspace.json" {
            return Ok(FileHandleU64::new(MANIFEST));
        }
        let location = self.child(&snapshot, dirid, filename.0.as_ref())?;
        if matches!(location.root, RootId::Manifest) {
            return Ok(FileHandleU64::new(MANIFEST));
        }
        let root = self.open_root(&snapshot, &location.root)?;
        if location.relative.as_os_str().is_empty() {
            let id = self.table().insert(location);
            return Ok(FileHandleU64::new(id));
        }
        posix::beneath(&root, &location.relative, libc::O_RDONLY)
            .or_else(|_| {
                posix::beneath(
                    &root,
                    &location.relative,
                    libc::O_RDONLY | libc::O_DIRECTORY,
                )
            })
            .map_err(io_to_nfs)?;
        let id = self.table().insert(location);
        Ok(FileHandleU64::new(id))
    }

    async fn getattr(&self, id: &Self::Handle) -> Result<fattr3, nfsstat3> {
        let snapshot = self.snapshot()?;
        if id.as_u64() == ROOT {
            return Ok(virtual_dir_attr(ROOT));
        }
        if id.as_u64() == MANIFEST {
            // The size is re-rendered per call; the read path below serves the
            // same current revision, so no stale length can persist.
            let size = self.manifest_bytes(&snapshot)?.len() as u64;
            let mut table = self.table();
            table.manifest_bytes = table.manifest_bytes.saturating_add(size as usize);
            return Ok(virtual_file_attr(MANIFEST, size));
        }
        let location = self.resolve(&snapshot, id.as_u64())?;
        if matches!(location.root, RootId::Manifest) {
            let size = self.manifest_bytes(&snapshot)?.len() as u64;
            return Ok(virtual_file_attr(MANIFEST, size));
        }
        self.location_attr(&snapshot, &location)
    }

    async fn read(
        &self,
        id: &Self::Handle,
        offset: u64,
        count: u32,
    ) -> Result<(Vec<u8>, bool), nfsstat3> {
        use std::os::unix::fs::FileExt;
        let snapshot = self.snapshot()?;
        if id.as_u64() == MANIFEST {
            let bytes = self.manifest_bytes(&snapshot)?;
            let start = usize::try_from(offset)
                .unwrap_or(usize::MAX)
                .min(bytes.len());
            let end = start.saturating_add(count as usize).min(bytes.len());
            return Ok((bytes[start..end].to_vec(), end == bytes.len()));
        }
        if id.as_u64() == ROOT {
            return Err(nfsstat3::NFS3ERR_ISDIR);
        }
        let location = self.resolve(&snapshot, id.as_u64())?;
        if matches!(location.root, RootId::Manifest) {
            let bytes = self.manifest_bytes(&snapshot)?;
            let start = usize::try_from(offset)
                .unwrap_or(usize::MAX)
                .min(bytes.len());
            let end = start.saturating_add(count as usize).min(bytes.len());
            return Ok((bytes[start..end].to_vec(), end == bytes.len()));
        }
        let root = self.open_root(&snapshot, &location.root)?;
        let file = posix::beneath(&root, &location.relative, libc::O_RDONLY).map_err(io_to_nfs)?;
        let size = file.metadata().map_err(|_| nfsstat3::NFS3ERR_IO)?.len();
        let start = offset.min(size);
        let length = (count as u64).min(size.saturating_sub(start)) as usize;
        let mut bytes = vec![0; length];
        file.read_at(&mut bytes, start)
            .map_err(|_| nfsstat3::NFS3ERR_IO)?;
        Ok((bytes, start + length as u64 >= size))
    }

    async fn readdirplus(
        &self,
        dirid: &Self::Handle,
        cookie: u64,
    ) -> Result<impl ReadDirPlusIterator<Self::Handle>, nfsstat3> {
        let snapshot = self.snapshot()?;
        let mut names: Vec<(Vec<u8>, Option<Location>)> = Vec::new();
        if dirid.as_u64() == ROOT {
            names.push((b"workspace.json".to_vec(), None));
            names.push((b".state".to_vec(), None));
            for root in &snapshot.roots {
                let name = if root.primary {
                    b"primary".to_vec()
                } else {
                    root.key.as_bytes().to_vec()
                };
                let location = Location {
                    root: RootId::Catalog {
                        key: root.key.clone(),
                        path: root.path.0.clone(),
                        primary: root.primary,
                    },
                    relative: PathBuf::new(),
                };
                names.push((name, Some(location)));
            }
        } else if dirid.as_u64() == MANIFEST {
            return Err(nfsstat3::NFS3ERR_NOTDIR);
        } else {
            let location = self.resolve(&snapshot, dirid.as_u64())?;
            if matches!(location.root, RootId::Manifest) {
                return Err(nfsstat3::NFS3ERR_NOTDIR);
            }
            let root = self.open_root(&snapshot, &location.root)?;
            let directory = if location.relative.as_os_str().is_empty() {
                root
            } else {
                posix::beneath(
                    &root,
                    &location.relative,
                    libc::O_RDONLY | libc::O_DIRECTORY,
                )
                .map_err(io_to_nfs)?
            };
            for name in posix::names(&directory).map_err(io_to_nfs)? {
                let mut relative = location.relative.clone();
                relative.push(OsStr::from_bytes(&name));
                names.push((
                    name,
                    Some(Location {
                        root: location.root.clone(),
                        relative,
                    }),
                ));
            }
        }
        names.sort_by(|a, b| a.0.cmp(&b.0));
        names.dedup_by(|a, b| a.0 == b.0);
        let mut entries = Vec::with_capacity(names.len());
        for (index, (name, location)) in names.into_iter().enumerate() {
            let entry_cookie = index as u64 + 1;
            if entry_cookie <= cookie {
                continue;
            }
            let (handle, attr) = match location {
                None if name == b"workspace.json" => {
                    let size = self.manifest_bytes(&snapshot)?.len() as u64;
                    (
                        FileHandleU64::new(MANIFEST),
                        virtual_file_attr(MANIFEST, size),
                    )
                }
                None => {
                    let location = Location {
                        root: RootId::State,
                        relative: PathBuf::new(),
                    };
                    let attr = self.location_attr(&snapshot, &location)?;
                    let id = self.table().insert(location);
                    (FileHandleU64::new(id), attr)
                }
                Some(location) => {
                    let attr = self.location_attr(&snapshot, &location)?;
                    let id = self.table().insert(location);
                    (FileHandleU64::new(id), attr)
                }
            };
            entries.push(DirEntryPlus {
                fileid: attr.fileid,
                name: into_filename(name),
                cookie: entry_cookie,
                name_attributes: Some(attr),
                name_handle: Some(handle),
            });
        }
        Ok(PlusEntries { entries, index: 0 })
    }

    async fn readlink(&self, id: &Self::Handle) -> Result<nfspath3<'_>, nfsstat3> {
        let snapshot = self.snapshot()?;
        if id.as_u64() == ROOT || id.as_u64() == MANIFEST {
            return Err(nfsstat3::NFS3ERR_INVAL);
        }
        let location = self.resolve(&snapshot, id.as_u64())?;
        if matches!(location.root, RootId::Manifest) {
            return Err(nfsstat3::NFS3ERR_INVAL);
        }
        let root = self.open_root(&snapshot, &location.root)?;
        let (parent, name) = split(&location.relative)?;
        let directory = posix::beneath(&root, &parent, libc::O_RDONLY | libc::O_DIRECTORY)
            .map_err(io_to_nfs)?;
        let target = posix::readlink(&directory, name).map_err(io_to_nfs)?;
        Ok(nfspath3(Opaque::owned(target)))
    }

    async fn fsstat(&self, root_fileid: &Self::Handle) -> Result<FSSTAT3resok, nfsstat3> {
        let snapshot = self.snapshot()?;
        let location = if root_fileid.as_u64() == ROOT || root_fileid.as_u64() == MANIFEST {
            Location {
                root: RootId::State,
                relative: PathBuf::new(),
            }
        } else {
            let location = self.resolve(&snapshot, root_fileid.as_u64())?;
            if matches!(location.root, RootId::Manifest) {
                Location {
                    root: RootId::State,
                    relative: PathBuf::new(),
                }
            } else {
                location
            }
        };
        let root = self.open_root(&snapshot, &location.root)?;
        let file = if location.relative.as_os_str().is_empty() {
            root
        } else {
            posix::beneath(
                &root,
                &location.relative,
                libc::O_RDONLY | libc::O_DIRECTORY,
            )
            .map_err(io_to_nfs)?
        };
        let stat = posix::statfs(&file).map_err(|_| nfsstat3::NFS3ERR_IO)?;
        let block = u64::try_from(stat.f_bsize.max(1)).unwrap_or(1);
        Ok(FSSTAT3resok {
            obj_attributes: post_op_attr::None,
            tbytes: stat.f_blocks * block,
            fbytes: stat.f_bavail * block,
            abytes: stat.f_bavail * block,
            tfiles: stat.f_files,
            ffiles: stat.f_ffree,
            afiles: stat.f_ffree,
            invarsec: 0,
        })
    }

    async fn pathconf(&self, _object: &Self::Handle) -> Result<PATHCONF3resok, nfsstat3> {
        // No authoritative local source is plumbed here; report unsupported
        // rather than fabricate pathname constraints.
        Err(nfsstat3::NFS3ERR_NOTSUPP)
    }
}

fn split(relative: &Path) -> Result<(PathBuf, &OsStr)> {
    let name = relative.file_name().ok_or(nfsstat3::NFS3ERR_INVAL)?;
    let parent = relative.parent().map(Path::to_path_buf).unwrap_or_default();
    Ok((parent, name))
}

impl NfsFileSystem for Filesystem {
    fn capabilities(&self) -> VFSCapabilities {
        VFSCapabilities::ReadWrite
    }

    async fn setattr(&self, id: &Self::Handle, setattr: sattr3) -> Result<fattr3, nfsstat3> {
        let snapshot = self.snapshot()?;
        if id.as_u64() == ROOT || id.as_u64() == MANIFEST {
            return Err(nfsstat3::NFS3ERR_ROFS);
        }
        let location = self.resolve(&snapshot, id.as_u64())?;
        if matches!(location.root, RootId::Manifest) {
            return Err(nfsstat3::NFS3ERR_ROFS);
        }
        let root = self.writable(&snapshot, &location)?;
        let file = posix::beneath(&root, &location.relative, libc::O_RDONLY | libc::O_NOFOLLOW)
            .map_err(io_to_nfs)?;
        if let set_mode3::Some(mode) = setattr.mode {
            posix::chmod(&file, mode).map_err(io_to_nfs)?;
        }
        match (setattr.uid, setattr.gid) {
            (set_uid3::None, set_gid3::None) => {}
            (uid, gid) => {
                let uid = match uid {
                    set_uid3::Some(uid) => Some(uid),
                    _ => None,
                };
                let gid = match gid {
                    set_gid3::Some(gid) => Some(gid),
                    _ => None,
                };
                posix::chown(&file, uid, gid).map_err(io_to_nfs)?;
            }
        }
        if let set_size3::Some(size) = setattr.size {
            truncate(&file, size)?;
        }
        let atime = convert_atime(setattr.atime)?;
        let mtime = convert_mtime(setattr.mtime)?;
        if atime.is_some() || mtime.is_some() {
            let current = file.metadata().map_err(|_| nfsstat3::NFS3ERR_IO)?;
            let times = [
                atime.unwrap_or(to_libc_time(current.atime(), current.atime_nsec())),
                mtime.unwrap_or(to_libc_time(current.mtime(), current.mtime_nsec())),
            ];
            posix::times(&file, times).map_err(io_to_nfs)?;
        }
        let snapshot = self.snapshot()?;
        let location = self.resolve(&snapshot, id.as_u64())?;
        self.location_attr(&snapshot, &location)
    }

    async fn write(
        &self,
        id: &Self::Handle,
        offset: u64,
        data: &[u8],
        _stable: stable_how,
    ) -> Result<(fattr3, stable_how), nfsstat3> {
        use std::os::unix::fs::FileExt;
        let snapshot = self.snapshot()?;
        if id.as_u64() == ROOT || id.as_u64() == MANIFEST {
            return Err(nfsstat3::NFS3ERR_ROFS);
        }
        let location = self.resolve(&snapshot, id.as_u64())?;
        if matches!(location.root, RootId::Manifest) {
            return Err(nfsstat3::NFS3ERR_ROFS);
        }
        let root = self.writable(&snapshot, &location)?;
        let file = posix::beneath(&root, &location.relative, libc::O_WRONLY | libc::O_NOFOLLOW)
            .map_err(io_to_nfs)?;
        file.write_at(data, offset)
            .map_err(|_| nfsstat3::NFS3ERR_IO)?;
        // The mount uses sync semantics; always stabilize before replying.
        file.sync_data().map_err(|_| nfsstat3::NFS3ERR_IO)?;
        let snapshot = self.snapshot()?;
        let location = self.resolve(&snapshot, id.as_u64())?;
        let attr = self.location_attr(&snapshot, &location)?;
        Ok((attr, stable_how::FILE_SYNC))
    }

    async fn create(
        &self,
        dirid: &Self::Handle,
        filename: &filename3<'_>,
        attr: sattr3,
    ) -> Result<(Self::Handle, fattr3), nfsstat3> {
        let snapshot = self.snapshot()?;
        let location = self.child(&snapshot, dirid, filename.0.as_ref())?;
        if matches!(location.root, RootId::Manifest) {
            return Err(nfsstat3::NFS3ERR_ROFS);
        }
        let root = self.writable(&snapshot, &location)?;
        let (parent, name) = split(&location.relative)?;
        let directory = posix::beneath(&root, &parent, libc::O_RDONLY | libc::O_DIRECTORY)
            .map_err(io_to_nfs)?;
        let file = posix::at(
            &directory,
            name,
            libc::O_WRONLY | libc::O_CREAT | libc::O_EXCL,
            0o666,
        )
        .map_err(|error| {
            if error.raw_os_error() == Some(libc::EEXIST) {
                nfsstat3::NFS3ERR_EXIST
            } else {
                io_to_nfs(error)
            }
        })?;
        if let set_mode3::Some(mode) = attr.mode {
            posix::chmod(&file, mode & 0o7777).map_err(io_to_nfs)?;
        }
        drop(file);
        let snapshot = self.snapshot()?;
        let created = self.child(&snapshot, dirid, filename.0.as_ref())?;
        let attr = self.location_attr(&snapshot, &created)?;
        let id = self.table().insert(created);
        Ok((FileHandleU64::new(id), attr))
    }

    async fn create_exclusive(
        &self,
        dirid: &Self::Handle,
        filename: &filename3<'_>,
        createverf: createverf3,
    ) -> Result<Self::Handle, nfsstat3> {
        let snapshot = self.snapshot()?;
        let location = self.child(&snapshot, dirid, filename.0.as_ref())?;
        if matches!(location.root, RootId::Manifest) {
            return Err(nfsstat3::NFS3ERR_ROFS);
        }
        let root = self.writable(&snapshot, &location)?;
        let (parent, name) = split(&location.relative)?;
        let directory = posix::beneath(&root, &parent, libc::O_RDONLY | libc::O_DIRECTORY)
            .map_err(io_to_nfs)?;
        match posix::beneath(&directory, Path::new(name), libc::O_RDONLY) {
            Ok(existing) => {
                if posix::verifier(&existing).map_err(io_to_nfs)? == createverf.0 {
                    let id = self.table().insert(location);
                    return Ok(FileHandleU64::new(id));
                }
                Err(nfsstat3::NFS3ERR_EXIST)
            }
            Err(error) if error.raw_os_error() == Some(libc::ENOENT) => {
                let file = posix::at(
                    &directory,
                    name,
                    libc::O_WRONLY | libc::O_CREAT | libc::O_EXCL,
                    0o666,
                )
                .map_err(io_to_nfs)?;
                posix::set_verifier(&file, &createverf.0).map_err(io_to_nfs)?;
                let id = self.table().insert(location);
                Ok(FileHandleU64::new(id))
            }
            Err(error) => Err(io_to_nfs(error)),
        }
    }

    async fn mkdir(
        &self,
        dirid: &Self::Handle,
        dirname: &filename3<'_>,
    ) -> Result<(Self::Handle, fattr3), nfsstat3> {
        let snapshot = self.snapshot()?;
        let location = self.child(&snapshot, dirid, dirname.0.as_ref())?;
        if matches!(location.root, RootId::Manifest) {
            return Err(nfsstat3::NFS3ERR_ROFS);
        }
        let root = self.writable(&snapshot, &location)?;
        let (parent, name) = split(&location.relative)?;
        let directory = posix::beneath(&root, &parent, libc::O_RDONLY | libc::O_DIRECTORY)
            .map_err(io_to_nfs)?;
        posix::mkdir(&directory, name, 0o777).map_err(io_to_nfs)?;
        let snapshot = self.snapshot()?;
        let created = self.child(&snapshot, dirid, dirname.0.as_ref())?;
        let attr = self.location_attr(&snapshot, &created)?;
        let id = self.table().insert(created);
        Ok((FileHandleU64::new(id), attr))
    }

    async fn remove(&self, dirid: &Self::Handle, filename: &filename3<'_>) -> Result<(), nfsstat3> {
        let snapshot = self.snapshot()?;
        let location = self.child(&snapshot, dirid, filename.0.as_ref())?;
        if matches!(location.root, RootId::Manifest) {
            return Err(nfsstat3::NFS3ERR_ROFS);
        }
        let root = self.writable(&snapshot, &location)?;
        let (parent, name) = split(&location.relative)?;
        let directory = posix::beneath(&root, &parent, libc::O_RDONLY | libc::O_DIRECTORY)
            .map_err(io_to_nfs)?;
        // Probe type without following a final-component symlink.
        let is_directory = posix::beneath(
            &directory,
            Path::new(name),
            libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW,
        )
        .map(|_| true)
        .unwrap_or(false);
        posix::remove(&directory, name, is_directory).map_err(io_to_nfs)?;
        self.table().by_location.remove(&location);
        Ok(())
    }

    async fn rename<'a>(
        &self,
        from_dirid: &Self::Handle,
        from_filename: &filename3<'a>,
        to_dirid: &Self::Handle,
        to_filename: &filename3<'a>,
    ) -> Result<(), nfsstat3> {
        let snapshot = self.snapshot()?;
        let source = self.child(&snapshot, from_dirid, from_filename.0.as_ref())?;
        let destination = self.child(&snapshot, to_dirid, to_filename.0.as_ref())?;
        if matches!(source.root, RootId::Manifest) || matches!(destination.root, RootId::Manifest) {
            return Err(nfsstat3::NFS3ERR_ROFS);
        }
        if source.root != destination.root {
            return Err(nfsstat3::NFS3ERR_XDEV);
        }
        let root = self.writable(&snapshot, &source)?;
        self.writable(&snapshot, &destination)?;
        let (from_parent, from_name) = split(&source.relative)?;
        let (to_parent, to_name) = split(&destination.relative)?;
        let from_directory =
            posix::beneath(&root, &from_parent, libc::O_RDONLY | libc::O_DIRECTORY)
                .map_err(io_to_nfs)?;
        let to_directory = posix::beneath(&root, &to_parent, libc::O_RDONLY | libc::O_DIRECTORY)
            .map_err(io_to_nfs)?;
        posix::rename(&from_directory, from_name, &to_directory, to_name).map_err(io_to_nfs)?;
        let mut table = self.table();
        table.by_location.remove(&source);
        table.by_location.remove(&destination);
        Ok(())
    }

    async fn symlink<'a>(
        &self,
        dirid: &Self::Handle,
        linkname: &filename3<'a>,
        symlink: &nfspath3<'a>,
        _attr: &sattr3,
    ) -> Result<(Self::Handle, fattr3), nfsstat3> {
        let snapshot = self.snapshot()?;
        let location = self.child(&snapshot, dirid, linkname.0.as_ref())?;
        if matches!(location.root, RootId::Manifest) {
            return Err(nfsstat3::NFS3ERR_ROFS);
        }
        let root = self.writable(&snapshot, &location)?;
        let (parent, name) = split(&location.relative)?;
        let directory = posix::beneath(&root, &parent, libc::O_RDONLY | libc::O_DIRECTORY)
            .map_err(io_to_nfs)?;
        // Absolute targets are rejected: they would escape the workspace view.
        if symlink.0.as_ref().starts_with(b"/") {
            return Err(nfsstat3::NFS3ERR_ACCES);
        }
        posix::symlink(&directory, name, symlink.0.as_ref()).map_err(io_to_nfs)?;
        let snapshot = self.snapshot()?;
        let created = self.child(&snapshot, dirid, linkname.0.as_ref())?;
        let attr = self.location_attr(&snapshot, &created)?;
        let id = self.table().insert(created);
        Ok((FileHandleU64::new(id), attr))
    }

    async fn commit(&self, id: &Self::Handle, _offset: u64, _count: u32) -> Result<(), nfsstat3> {
        let snapshot = self.snapshot()?;
        if id.as_u64() == ROOT || id.as_u64() == MANIFEST {
            return Err(nfsstat3::NFS3ERR_ROFS);
        }
        let location = self.resolve(&snapshot, id.as_u64())?;
        if matches!(location.root, RootId::Manifest) {
            return Err(nfsstat3::NFS3ERR_ROFS);
        }
        let root = self.writable(&snapshot, &location)?;
        let file = posix::beneath(&root, &location.relative, libc::O_RDONLY).map_err(io_to_nfs)?;
        file.sync_all().map_err(|_| nfsstat3::NFS3ERR_IO)?;
        Ok(())
    }
}

fn truncate(file: &File, size: u64) -> Result<()> {
    use std::os::fd::AsRawFd;
    let size = size.min(i64::MAX as u64) as libc::off_t;
    if unsafe { libc::ftruncate(file.as_raw_fd(), size) } < 0 {
        return Err(io_to_nfs(io::Error::last_os_error()));
    }
    Ok(())
}

fn convert_atime(value: set_atime) -> Result<Option<libc::timespec>> {
    Ok(match value {
        set_atime::DONT_CHANGE => None,
        set_atime::SET_TO_SERVER_TIME => Some(libc::timespec {
            tv_sec: 0,
            tv_nsec: libc::UTIME_NOW,
        }),
        set_atime::SET_TO_CLIENT_TIME(time) => Some(libc::timespec {
            tv_sec: time.seconds as libc::time_t,
            tv_nsec: time.nseconds as _,
        }),
    })
}

fn convert_mtime(value: set_mtime) -> Result<Option<libc::timespec>> {
    Ok(match value {
        set_mtime::DONT_CHANGE => None,
        set_mtime::SET_TO_SERVER_TIME => Some(libc::timespec {
            tv_sec: 0,
            tv_nsec: libc::UTIME_NOW,
        }),
        set_mtime::SET_TO_CLIENT_TIME(time) => Some(libc::timespec {
            tv_sec: time.seconds as libc::time_t,
            tv_nsec: time.nseconds as _,
        }),
    })
}

fn to_libc_time(seconds: i64, nanos: i64) -> libc::timespec {
    libc::timespec {
        tv_sec: seconds as libc::time_t,
        tv_nsec: nanos as _,
    }
}
