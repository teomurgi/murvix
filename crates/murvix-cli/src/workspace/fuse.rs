//! Live Linux native workspace filesystem.
//!
//! Broker operations fetch live membership before touching the host. Regular
//! opens use direct I/O; shared mmap is explicitly negotiated for `SQLite` WAL.
//! Mappings use the kernel page cache and, like established socket connections,
//! are captured open state rather than revocable namespace capabilities.
//! Future opens remain live-authorized; no backing fd is passed to the kernel.
//! Unix sockets bind to this mount's kernel inode, never a proxied host socket.
//! Device/FIFO open and creation, and extended attributes, are unsupported.
//! Multiply linked non-directory inodes reject mutations without an alias scan.
mod host;
mod manifest;

use std::collections::HashMap;
use std::error::Error;
use std::ffi::OsStr;
use std::fs::File;
use std::mem::MaybeUninit;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::FileExt;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, SystemTime};

use super::Binding;
use fuser::{
    AccessFlags, BsdFileFlags, Errno, FileAttr, FileHandle, FileType, Filesystem, FopenFlags,
    Generation, INodeNo, InitFlags, KernelConfig, LockOwner, OpenFlags, RenameFlags, ReplyAttr,
    ReplyCreate, ReplyData, ReplyDirectory, ReplyEmpty, ReplyEntry, ReplyOpen, ReplyStatfs,
    ReplyWrite, ReplyXattr, Request, TimeOrNow, WriteFlags,
};
use host::{Result, errno};
use murvix_protocol::{WorkspaceRoot, WorkspaceSnapshot};
use rustix::fs::{self, AtFlags, Mode, OFlags};

const TTL: Duration = Duration::ZERO;
const ROOT: INodeNo = INodeNo(1);
const MANIFEST: INodeNo = INodeNo(2);
const MAX_MANIFEST_BYTES: usize = 256 * 1024;
const MAX_MANIFEST_MEMORY: usize = 4 * 1024 * 1024;

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
enum RootId {
    State,
    Catalog {
        key: String,
        path: Vec<u8>,
        primary: bool,
    },
}

impl RootId {
    fn name(&self) -> &OsStr {
        match self {
            Self::State => OsStr::new(".state"),
            Self::Catalog { primary: true, .. } => OsStr::new("primary"),
            Self::Catalog { key, .. } => OsStr::new(key),
        }
    }

    fn catalog(root: &WorkspaceRoot) -> Self {
        Self::Catalog {
            key: root.key.clone(),
            path: root.path.0.clone(),
            primary: root.primary,
        }
    }
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
struct Location {
    root: RootId,
    relative: PathBuf,
}

struct Node {
    location: Location,
    identity: Option<(u64, u64)>,
    lookups: u64,
    opens: u64,
}

struct Handle {
    inode: u64,
    file: Option<File>,
    manifest: Option<Vec<u8>>,
    writable: bool,
    directory: bool,
    revision: String,
}

#[derive(Default)]
struct Table {
    next_inode: u64,
    next_handle: u64,
    nodes: HashMap<u64, Node>,
    paths: HashMap<Location, u64>,
    handles: HashMap<u64, Handle>,
    manifest_bytes: usize,
}

impl Table {
    fn node(&self, ino: INodeNo) -> Result<&Node> {
        self.nodes.get(&ino.0).ok_or(Errno::ESTALE)
    }

    fn insert(&mut self, location: Location, identity: Option<(u64, u64)>) -> Result<INodeNo> {
        if let Some(ino) = self.paths.get(&location).copied()
            && let Some(node) = self.nodes.get_mut(&ino)
            && (node.identity == identity
                || (node.identity.is_none() && location.relative.as_os_str().is_empty()))
        {
            node.identity = identity;
            node.lookups = node.lookups.checked_add(1).ok_or(Errno::EOVERFLOW)?;
            return Ok(INodeNo(ino));
        }
        self.next_inode = self
            .next_inode
            .max(MANIFEST.0)
            .checked_add(1)
            .ok_or(Errno::EOVERFLOW)?;
        let ino = self.next_inode;
        self.paths.insert(location.clone(), ino);
        self.nodes.insert(
            ino,
            Node {
                location,
                identity,
                lookups: 1,
                opens: 0,
            },
        );
        Ok(INodeNo(ino))
    }

    fn collect(&mut self, ino: u64) {
        if self
            .nodes
            .get(&ino)
            .is_some_and(|node| node.lookups == 0 && node.opens == 0)
            && let Some(node) = self.nodes.remove(&ino)
            && self.paths.get(&node.location) == Some(&ino)
        {
            self.paths.remove(&node.location);
        }
    }

    fn open(
        &mut self,
        ino: INodeNo,
        file: Option<File>,
        writable: bool,
        directory: bool,
        revision: &str,
    ) -> Result<FileHandle> {
        self.next_handle = self.next_handle.checked_add(1).ok_or(Errno::EOVERFLOW)?;
        if ino != ROOT && ino != MANIFEST {
            let node = self.nodes.get_mut(&ino.0).ok_or(Errno::ESTALE)?;
            node.opens = node.opens.checked_add(1).ok_or(Errno::EOVERFLOW)?;
        }
        let handle = self.next_handle;
        self.handles.insert(
            handle,
            Handle {
                inode: ino.0,
                file,
                manifest: None,
                writable,
                directory,
                revision: revision.to_owned(),
            },
        );
        Ok(FileHandle(handle))
    }

    fn close(&mut self, ino: INodeNo, fh: FileHandle, directory: bool) -> Result<()> {
        let handle = self.handles.get(&fh.0).ok_or(Errno::EBADF)?;
        if handle.inode != ino.0 || handle.directory != directory {
            return Err(Errno::EBADF);
        }
        self.manifest_bytes = self
            .manifest_bytes
            .saturating_sub(handle.manifest.as_ref().map_or(0, Vec::len));
        self.handles.remove(&fh.0);
        if let Some(node) = self.nodes.get_mut(&ino.0) {
            node.opens = node.opens.saturating_sub(1);
        }
        self.collect(ino.0);
        Ok(())
    }

    fn handle(&self, ino: INodeNo, fh: FileHandle, directory: bool) -> Result<&Handle> {
        let handle = self.handles.get(&fh.0).ok_or(Errno::EBADF)?;
        if handle.inode != ino.0 || handle.directory != directory {
            return Err(Errno::EBADF);
        }
        Ok(handle)
    }

    fn moved(&mut self, source: &Location, destination: &Location) {
        // Only referenced/open nodes exist here; this never scans the host tree.
        self.paths.retain(|location, _| {
            location.root != source.root || !location.relative.starts_with(&source.relative)
        });
        for (ino, node) in &mut self.nodes {
            if node.location.root == source.root
                && let Ok(suffix) = node.location.relative.strip_prefix(&source.relative)
            {
                node.location = Location {
                    root: destination.root.clone(),
                    relative: destination.relative.join(suffix),
                };
                self.paths.insert(node.location.clone(), *ino);
            }
        }
    }
}

struct Workspace {
    binding: Binding,
    state: PathBuf,
    mountpoint: PathBuf,
    table: Mutex<Table>,
}

pub(super) fn mount(
    binding: Binding,
    state: PathBuf,
    mountpoint: &Path,
) -> std::result::Result<fuser::BackgroundSession, Box<dyn Error>> {
    binding.snapshot()?;
    host::open_root(&state).map_err(|error| std::io::Error::from_raw_os_error(error.code()))?;
    let workspace = Workspace {
        binding,
        state,
        mountpoint: mountpoint.to_owned(),
        table: Mutex::new(Table::default()),
    };
    let mut config = fuser::Config::default();
    config.mount_options = vec![
        fuser::MountOption::FSName("murvix-workspace".into()),
        fuser::MountOption::NoDev,
        fuser::MountOption::NoSuid,
    ];
    Ok(fuser::spawn_mount(workspace, mountpoint, &config)?)
}

impl Workspace {
    fn operation<T>(
        &self,
        action: impl FnOnce(&WorkspaceSnapshot, &mut Table) -> Result<T>,
    ) -> Result<T> {
        let snapshot = self.binding.snapshot().map_err(|_| Errno::EIO)?;
        let mut table = self.table.lock().map_err(|_| Errno::EIO)?;
        action(&snapshot, &mut table)
    }

    fn manifest(snapshot: &WorkspaceSnapshot) -> Result<Vec<u8>> {
        let bytes = manifest::render(snapshot).map_err(|_| Errno::EIO)?;
        if bytes.len() > MAX_MANIFEST_BYTES {
            return Err(Errno::EOVERFLOW);
        }
        Ok(bytes)
    }

    fn manifest_attr(snapshot: &WorkspaceSnapshot, size: Option<usize>) -> Result<FileAttr> {
        let mut attr = host::virtual_attr(MANIFEST, false)?;
        attr.kind = FileType::RegularFile;
        attr.perm = 0o444;
        attr.nlink = 1;
        attr.size = match size {
            Some(size) => size,
            None => Self::manifest(snapshot)?.len(),
        } as u64;
        Ok(attr)
    }

    fn child(
        &self,
        snapshot: &WorkspaceSnapshot,
        table: &Table,
        parent: INodeNo,
        name: &OsStr,
    ) -> Result<Location> {
        host::component(name)?;
        if parent == ROOT && name == "workspace.json" {
            return Err(Errno::EROFS);
        }
        if parent == ROOT {
            let root = if name == ".state" {
                RootId::State
            } else {
                let root = snapshot
                    .roots
                    .iter()
                    .find(|root| {
                        if root.primary {
                            name == "primary"
                        } else {
                            name == OsStr::new(&root.key)
                        }
                    })
                    .ok_or(Errno::ENOENT)?;
                RootId::catalog(root)
            };
            return Ok(Location {
                root,
                relative: PathBuf::new(),
            });
        }
        let node = table.node(parent)?;
        self.object(snapshot, node)?;
        let mut location = node.location.clone();
        location.relative.push(name);
        Ok(location)
    }

    fn object(&self, snapshot: &WorkspaceSnapshot, node: &Node) -> Result<(File, File)> {
        let root = host::current_root(snapshot, &node.location, &self.state)?;
        let object = host::target(&root, &node.location)?;
        if node.identity.is_some_and(|expected| {
            host::stat(&object)
                .map(|value| host::identity(&value) != expected)
                .unwrap_or(true)
        }) {
            return Err(Errno::ESTALE);
        }
        Ok((root, object))
    }

    fn attributes(
        &self,
        snapshot: &WorkspaceSnapshot,
        table: &Table,
        ino: INodeNo,
        fh: Option<FileHandle>,
    ) -> Result<FileAttr> {
        if ino == ROOT {
            return host::virtual_attr(ino, false);
        }
        if ino == MANIFEST {
            let size = if let Some(fh) = fh {
                let handle = table.handle(ino, fh, false)?;
                if handle.revision != snapshot.revision {
                    return Err(Errno::ESTALE);
                }
                Some(handle.manifest.as_ref().ok_or(Errno::EBADF)?.len())
            } else {
                None
            };
            return Self::manifest_attr(snapshot, size);
        }
        let node = table.node(ino)?;
        let result = if let Some(fh) = fh {
            let handle = table.handles.get(&fh.0).ok_or(Errno::EBADF)?;
            if handle.inode != ino.0 {
                return Err(Errno::EBADF);
            }
            let root = host::current_root(snapshot, &node.location, &self.state)?;
            let file = handle.file.as_ref().ok_or(Errno::EBADF)?;
            host::validate_handle(&root, file)?;
            host::stat(file)
        } else {
            self.object(snapshot, node)
                .and_then(|(_, object)| host::stat(&object))
        };
        match result {
            Ok(value) => host::attr(ino, &value),
            Err(error)
                if node.location.relative.as_os_str().is_empty() && error != Errno::ESTALE =>
            {
                host::virtual_attr(ino, false)
            }
            Err(error) => Err(error),
        }
    }

    fn entry(
        &self,
        snapshot: &WorkspaceSnapshot,
        table: &mut Table,
        location: Location,
    ) -> Result<FileAttr> {
        let value = host::current_root(snapshot, &location, &self.state)
            .and_then(|root| host::target(&root, &location))
            .and_then(|object| host::stat(&object));
        match value {
            Ok(value) => {
                let ino = table.insert(location, Some(host::identity(&value)))?;
                host::attr(ino, &value)
            }
            Err(error) if location.relative.as_os_str().is_empty() && error != Errno::ESTALE => {
                let ino = table.insert(location, None)?;
                host::virtual_attr(ino, false)
            }
            Err(error) => Err(error),
        }
    }

    fn checked_handle<'a>(
        &self,
        snapshot: &WorkspaceSnapshot,
        table: &'a Table,
        ino: INodeNo,
        fh: FileHandle,
        directory: bool,
        write: bool,
    ) -> Result<&'a File> {
        let handle = table.handle(ino, fh, directory)?;
        if ino == MANIFEST {
            return Err(Errno::EROFS);
        }
        let node = table.node(ino)?;
        let root = host::current_root(snapshot, &node.location, &self.state)?;
        let file = handle.file.as_ref().ok_or(Errno::EBADF)?;
        host::validate_handle(&root, file)?;
        if write {
            if !handle.writable && !directory {
                return Err(Errno::EBADF);
            }
            host::writable(snapshot, &node.location, &root, file, false)?;
        }
        Ok(file)
    }

    fn remove(
        &self,
        snapshot: &WorkspaceSnapshot,
        table: &Table,
        parent: INodeNo,
        name: &OsStr,
        directory: bool,
    ) -> Result<()> {
        let location = self.child(snapshot, table, parent, name)?;
        let root = host::current_root(snapshot, &location, &self.state)?;
        let (parent, name) = host::parent(&root, &location)?;
        host::writable_name(snapshot, &location, &root, &parent, &name, directory)?;
        fs::unlinkat(
            &parent,
            &name,
            if directory {
                AtFlags::REMOVEDIR
            } else {
                AtFlags::empty()
            },
        )
        .map_err(errno)
    }

    fn sync(
        &self,
        snapshot: &WorkspaceSnapshot,
        table: &Table,
        ino: INodeNo,
        fh: FileHandle,
        directory: bool,
        datasync: bool,
    ) -> Result<()> {
        if ino == MANIFEST {
            let handle = table.handle(ino, fh, false)?;
            return if handle.revision == snapshot.revision {
                Ok(())
            } else {
                Err(Errno::ESTALE)
            };
        }
        if ino == ROOT {
            table.handle(ino, fh, true)?;
            return Ok(());
        }
        let file = self.checked_handle(snapshot, table, ino, fh, directory, false)?;
        if datasync {
            file.sync_data()
        } else {
            file.sync_all()
        }
        .map_err(host::io_error)
    }
}

impl Filesystem for Workspace {
    fn init(&mut self, _req: &Request, config: &mut KernelConfig) -> std::io::Result<()> {
        // SQLite WAL and native agent runtimes need shared mappings. Refuse a
        // kernel that cannot provide them instead of launching a broken HOME.
        config
            .add_capabilities(InitFlags::FUSE_DIRECT_IO_ALLOW_MMAP)
            .map_err(|_| {
                std::io::Error::new(
                    std::io::ErrorKind::Unsupported,
                    "The workspace requires Linux FUSE shared-mmap support for direct-I/O files",
                )
            })
    }

    fn lookup(&self, _req: &Request, parent: INodeNo, name: &OsStr, reply: ReplyEntry) {
        match self.operation(|snapshot, table| {
            if parent == ROOT && name == "workspace.json" {
                return Self::manifest_attr(snapshot, None);
            }
            let location = self.child(snapshot, table, parent, name)?;
            self.entry(snapshot, table, location)
        }) {
            Ok(attr) => reply.entry(&TTL, &attr, Generation(0)),
            Err(error) => reply.error(error),
        }
    }

    fn forget(&self, _req: &Request, ino: INodeNo, nlookup: u64) {
        // Reference accounting is not a namespace operation and must still run
        // during server outages, otherwise failed refreshes would leak nodes.
        if let Ok(mut table) = self.table.lock() {
            if let Some(node) = table.nodes.get_mut(&ino.0) {
                node.lookups = node.lookups.saturating_sub(nlookup);
            }
            table.collect(ino.0);
        }
    }

    fn getattr(&self, _req: &Request, ino: INodeNo, fh: Option<FileHandle>, reply: ReplyAttr) {
        match self.operation(|snapshot, table| self.attributes(snapshot, table, ino, fh)) {
            Ok(attr) => reply.attr(&TTL, &attr),
            Err(error) => reply.error(error),
        }
    }

    fn open(&self, _req: &Request, ino: INodeNo, flags: OpenFlags, reply: ReplyOpen) {
        match self.operation(|snapshot, table| {
            if ino == MANIFEST {
                if flags.0 & libc::O_ACCMODE != libc::O_RDONLY || flags.0 & libc::O_TRUNC != 0 {
                    return Err(Errno::EROFS);
                }
                let bytes = Self::manifest(snapshot)?;
                if table.manifest_bytes.saturating_add(bytes.len()) > MAX_MANIFEST_MEMORY {
                    return Err(Errno::EMFILE);
                }
                let fh = table.open(ino, None, false, false, &snapshot.revision)?;
                table.handles.get_mut(&fh.0).ok_or(Errno::EIO)?.manifest = Some(bytes);
                table.manifest_bytes += table
                    .handles
                    .get(&fh.0)
                    .and_then(|handle| handle.manifest.as_ref())
                    .map_or(0, Vec::len);
                return Ok(fh);
            }
            let node = table.node(ino)?;
            let (root, object) = self.object(snapshot, node)?;
            if host::kind(host::stat(&object)?.st_mode)? != FileType::RegularFile {
                return Err(Errno::EOPNOTSUPP);
            }
            let write = flags.0 & libc::O_ACCMODE != libc::O_RDONLY || flags.0 & libc::O_TRUNC != 0;
            if write {
                host::writable(snapshot, &node.location, &root, &object, false)?;
            }
            let flags = OFlags::from_bits_retain(u32::try_from(flags.0).unwrap_or(0))
                - OFlags::CREATE
                - OFlags::EXCL;
            let file = host::reopen(&object, flags | OFlags::NONBLOCK)?;
            table.open(ino, Some(file), write, false, &snapshot.revision)
        }) {
            Ok(fh) => reply.opened(fh, FopenFlags::FOPEN_DIRECT_IO),
            Err(error) => reply.error(error),
        }
    }

    fn read(
        &self,
        _req: &Request,
        ino: INodeNo,
        fh: FileHandle,
        offset: u64,
        size: u32,
        _flags: OpenFlags,
        _owner: Option<LockOwner>,
        reply: ReplyData,
    ) {
        let mut reply = Some(reply);
        let result = self.operation(|snapshot, table| {
            let mut storage;
            let bytes = if ino == MANIFEST {
                let handle = table.handle(ino, fh, false)?;
                if handle.revision != snapshot.revision {
                    return Err(Errno::ESTALE);
                }
                let bytes = handle.manifest.as_ref().ok_or(Errno::EBADF)?;
                let start = usize::try_from(offset)
                    .unwrap_or(usize::MAX)
                    .min(bytes.len());
                &bytes[start..start.saturating_add(size as usize).min(bytes.len())]
            } else {
                let file = self.checked_handle(snapshot, table, ino, fh, false, false)?;
                storage = vec![0; size as usize];
                let count = file.read_at(&mut storage, offset).map_err(host::io_error)?;
                &storage[..count]
            };
            reply.take().ok_or(Errno::EIO)?.data(bytes);
            Ok(())
        });
        if let Err(error) = result
            && let Some(reply) = reply
        {
            reply.error(error);
        }
    }

    fn write(
        &self,
        _req: &Request,
        ino: INodeNo,
        fh: FileHandle,
        offset: u64,
        data: &[u8],
        _write_flags: WriteFlags,
        _flags: OpenFlags,
        _owner: Option<LockOwner>,
        reply: ReplyWrite,
    ) {
        match self.operation(|snapshot, table| {
            let file = self.checked_handle(snapshot, table, ino, fh, false, true)?;
            file.write_at(data, offset).map_err(host::io_error)
        }) {
            Ok(count) => reply.written(u32::try_from(count).unwrap_or(u32::MAX)),
            Err(error) => reply.error(error),
        }
    }

    fn flush(
        &self,
        _req: &Request,
        ino: INodeNo,
        fh: FileHandle,
        _owner: LockOwner,
        reply: ReplyEmpty,
    ) {
        match self.operation(|snapshot, table| {
            if ino == MANIFEST {
                let handle = table.handle(ino, fh, false)?;
                return if handle.revision == snapshot.revision {
                    Ok(())
                } else {
                    Err(Errno::ESTALE)
                };
            }
            self.checked_handle(snapshot, table, ino, fh, false, false)
                .map(|_| ())
        }) {
            Ok(()) => reply.ok(),
            Err(error) => reply.error(error),
        }
    }

    fn release(
        &self,
        _req: &Request,
        ino: INodeNo,
        fh: FileHandle,
        _flags: OpenFlags,
        _owner: Option<LockOwner>,
        _flush: bool,
        reply: ReplyEmpty,
    ) {
        let result = self
            .table
            .lock()
            .map_err(|_| Errno::EIO)
            .and_then(|mut table| table.close(ino, fh, false));
        match result {
            Ok(()) => reply.ok(),
            Err(error) => reply.error(error),
        }
    }

    fn fsync(
        &self,
        _req: &Request,
        ino: INodeNo,
        fh: FileHandle,
        datasync: bool,
        reply: ReplyEmpty,
    ) {
        match self.operation(|snapshot, table| self.sync(snapshot, table, ino, fh, false, datasync))
        {
            Ok(()) => reply.ok(),
            Err(error) => reply.error(error),
        }
    }

    fn opendir(&self, _req: &Request, ino: INodeNo, _flags: OpenFlags, reply: ReplyOpen) {
        match self.operation(|snapshot, table| {
            if ino == MANIFEST {
                return Err(Errno::ENOTDIR);
            }
            let file = if ino == ROOT {
                None
            } else {
                let node = table.node(ino)?;
                let (_, object) = self.object(snapshot, node)?;
                Some(host::reopen(&object, OFlags::RDONLY | OFlags::DIRECTORY)?)
            };
            table.open(ino, file, false, true, &snapshot.revision)
        }) {
            Ok(fh) => reply.opened(fh, FopenFlags::empty()),
            Err(error) => reply.error(error),
        }
    }

    fn readdir(
        &self,
        _req: &Request,
        ino: INodeNo,
        fh: FileHandle,
        offset: u64,
        mut reply: ReplyDirectory,
    ) {
        let result = self.operation(|snapshot, table| {
            let handle = table.handle(ino, fh, true)?;
            if handle.revision != snapshot.revision {
                return Err(Errno::ESTALE);
            }
            if ino == ROOT {
                let entries = [
                    OsStr::new("."),
                    OsStr::new(".."),
                    OsStr::new("workspace.json"),
                    OsStr::new(".state"),
                ]
                .into_iter()
                .chain(snapshot.roots.iter().map(|root| {
                    if root.primary {
                        OsStr::new("primary")
                    } else {
                        OsStr::new(&root.key)
                    }
                }));
                for (index, name) in entries
                    .enumerate()
                    .skip(offset.try_into().map_err(|_| Errno::EINVAL)?)
                {
                    let (inode, kind) = match index {
                        0 | 1 => (ROOT, FileType::Directory),
                        2 => (MANIFEST, FileType::RegularFile),
                        _ => (INodeNo(0), FileType::Directory),
                    };
                    if reply.add(inode, index as u64 + 1, kind, name) {
                        break;
                    }
                }
                return Ok(());
            }
            let file = self.checked_handle(snapshot, table, ino, fh, true, false)?;
            fs::seek(file, fs::SeekFrom::Start(offset)).map_err(errno)?;
            let mut buffer = [MaybeUninit::uninit(); 8192];
            let mut directory = fs::RawDir::new(file, &mut buffer);
            while let Some(entry) = directory.next() {
                let entry = entry.map_err(errno)?;
                let name = OsStr::from_bytes(entry.file_name().to_bytes());
                let kind = match entry.file_type() {
                    fs::FileType::Unknown => {
                        let stat =
                            fs::statat(file, name, AtFlags::SYMLINK_NOFOLLOW).map_err(errno)?;
                        host::kind(stat.st_mode)?
                    }
                    kind => host::kind(kind.as_raw_mode())?,
                };
                if reply.add(
                    if name == "." { ino } else { INodeNo(0) },
                    entry.next_entry_cookie(),
                    kind,
                    name,
                ) {
                    break;
                }
            }
            Ok(())
        });
        match result {
            Ok(()) => reply.ok(),
            Err(error) => reply.error(error),
        }
    }

    fn releasedir(
        &self,
        _req: &Request,
        ino: INodeNo,
        fh: FileHandle,
        _flags: OpenFlags,
        reply: ReplyEmpty,
    ) {
        let result = self
            .table
            .lock()
            .map_err(|_| Errno::EIO)
            .and_then(|mut table| table.close(ino, fh, true));
        match result {
            Ok(()) => reply.ok(),
            Err(error) => reply.error(error),
        }
    }

    fn fsyncdir(
        &self,
        _req: &Request,
        ino: INodeNo,
        fh: FileHandle,
        datasync: bool,
        reply: ReplyEmpty,
    ) {
        match self.operation(|snapshot, table| self.sync(snapshot, table, ino, fh, true, datasync))
        {
            Ok(()) => reply.ok(),
            Err(error) => reply.error(error),
        }
    }

    fn create(
        &self,
        _req: &Request,
        parent: INodeNo,
        name: &OsStr,
        mode: u32,
        umask: u32,
        flags: i32,
        reply: ReplyCreate,
    ) {
        match self.operation(|snapshot, table| {
            let location = self.child(snapshot, table, parent, name)?;
            let root = host::current_root(snapshot, &location, &self.state)?;
            let (directory, name) = host::parent(&root, &location)?;
            host::writable_name(snapshot, &location, &root, &directory, &name, false)?;
            let flags = OFlags::from_bits_retain(u32::try_from(flags).unwrap_or(0))
                | OFlags::CREATE
                | OFlags::NOFOLLOW
                | OFlags::NONBLOCK;
            let file = match host::beneath(
                &directory,
                Path::new(&name),
                flags | OFlags::EXCL,
                Mode::from_bits_truncate((mode & !umask) & 0o7777),
            ) {
                Ok(file) => file,
                Err(error) if error == Errno::EEXIST && !flags.contains(OFlags::EXCL) => {
                    let object = host::target(&root, &location)?;
                    if host::kind(host::stat(&object)?.st_mode)? != FileType::RegularFile {
                        return Err(Errno::EOPNOTSUPP);
                    }
                    host::writable(snapshot, &location, &root, &object, false)?;
                    host::reopen(&object, flags)?
                }
                Err(error) => return Err(error),
            };
            let stat = host::stat(&file)?;
            if host::kind(stat.st_mode)? != FileType::RegularFile {
                return Err(Errno::EOPNOTSUPP);
            }
            let ino = table.insert(location, Some(host::identity(&stat)))?;
            let fh = table.open(
                ino,
                Some(file),
                flags.intersects(OFlags::WRONLY | OFlags::RDWR),
                false,
                &snapshot.revision,
            )?;
            Ok((host::attr(ino, &stat)?, fh))
        }) {
            Ok((attr, fh)) => {
                reply.created(&TTL, &attr, Generation(0), fh, FopenFlags::FOPEN_DIRECT_IO);
            }
            Err(error) => reply.error(error),
        }
    }

    fn mkdir(
        &self,
        _req: &Request,
        parent: INodeNo,
        name: &OsStr,
        mode: u32,
        umask: u32,
        reply: ReplyEntry,
    ) {
        match self.operation(|snapshot, table| {
            let location = self.child(snapshot, table, parent, name)?;
            let root = host::current_root(snapshot, &location, &self.state)?;
            let (directory, name) = host::parent(&root, &location)?;
            host::writable_name(snapshot, &location, &root, &directory, &name, false)?;
            fs::mkdirat(
                &directory,
                &name,
                Mode::from_bits_truncate((mode & !umask) & 0o7777),
            )
            .map_err(errno)?;
            self.entry(snapshot, table, location)
        }) {
            Ok(attr) => reply.entry(&TTL, &attr, Generation(0)),
            Err(error) => reply.error(error),
        }
    }

    fn unlink(&self, _req: &Request, parent: INodeNo, name: &OsStr, reply: ReplyEmpty) {
        match self.operation(|snapshot, table| self.remove(snapshot, table, parent, name, false)) {
            Ok(()) => reply.ok(),
            Err(error) => reply.error(error),
        }
    }

    fn rmdir(&self, _req: &Request, parent: INodeNo, name: &OsStr, reply: ReplyEmpty) {
        match self.operation(|snapshot, table| self.remove(snapshot, table, parent, name, true)) {
            Ok(()) => reply.ok(),
            Err(error) => reply.error(error),
        }
    }

    fn rename(
        &self,
        _req: &Request,
        parent: INodeNo,
        name: &OsStr,
        newparent: INodeNo,
        newname: &OsStr,
        flags: RenameFlags,
        reply: ReplyEmpty,
    ) {
        match self.operation(|snapshot, table| {
            let source = self.child(snapshot, table, parent, name)?;
            let destination = self.child(snapshot, table, newparent, newname)?;
            if source.root != destination.root {
                return Err(Errno::EXDEV);
            }
            let root = host::current_root(snapshot, &source, &self.state)?;
            let (old_dir, old_name) = host::parent(&root, &source)?;
            let (target_dir, target_name) = host::parent(&root, &destination)?;
            host::writable_name(snapshot, &source, &root, &old_dir, &old_name, true)?;
            host::writable_name(
                snapshot,
                &destination,
                &root,
                &target_dir,
                &target_name,
                true,
            )?;
            let flags = fs::RenameFlags::from_bits(flags.bits()).ok_or(Errno::EINVAL)?;
            fs::renameat_with(&old_dir, &old_name, &target_dir, &target_name, flags)
                .map_err(errno)?;
            if flags.contains(fs::RenameFlags::EXCHANGE) {
                // Preserve both exchanged node identities without path collisions.
                let mut exchanged = Vec::new();
                for (ino, node) in &table.nodes {
                    if node.location.root == destination.root
                        && node.location.relative.starts_with(&destination.relative)
                    {
                        exchanged.push((
                            *ino,
                            source.relative.join(
                                node.location
                                    .relative
                                    .strip_prefix(&destination.relative)
                                    .map_err(|_| Errno::EIO)?,
                            ),
                        ));
                    }
                }
                table.moved(&source, &destination);
                for (ino, relative) in exchanged {
                    if let Some(node) = table.nodes.get_mut(&ino) {
                        node.location.relative = relative;
                    }
                }
                table.paths.clear();
                for (ino, node) in &table.nodes {
                    table.paths.insert(node.location.clone(), *ino);
                }
            } else {
                table.moved(&source, &destination);
            }
            Ok(())
        }) {
            Ok(()) => reply.ok(),
            Err(error) => reply.error(error),
        }
    }

    fn symlink(
        &self,
        _req: &Request,
        parent: INodeNo,
        name: &OsStr,
        target: &Path,
        reply: ReplyEntry,
    ) {
        match self.operation(|snapshot, table| {
            let location = self.child(snapshot, table, parent, name)?;
            let root = host::current_root(snapshot, &location, &self.state)?;
            let (directory, name) = host::parent(&root, &location)?;
            host::writable_name(snapshot, &location, &root, &directory, &name, false)?;
            fs::symlinkat(target, &directory, &name).map_err(errno)?;
            self.entry(snapshot, table, location)
        }) {
            Ok(attr) => reply.entry(&TTL, &attr, Generation(0)),
            Err(error) => reply.error(error),
        }
    }

    fn readlink(&self, _req: &Request, ino: INodeNo, reply: ReplyData) {
        match self.operation(|snapshot, table| {
            let node = table.node(ino)?;
            let (root, _) = self.object(snapshot, node)?;
            let logical = host::root_path(snapshot, &node.location.root, &self.state)?;
            host::safe_link(
                &root,
                &node.location,
                logical,
                &self.mountpoint,
                node.location.root.name(),
            )
        }) {
            Ok(bytes) => reply.data(&bytes),
            Err(error) => reply.error(error),
        }
    }

    fn link(
        &self,
        _req: &Request,
        ino: INodeNo,
        newparent: INodeNo,
        newname: &OsStr,
        reply: ReplyEntry,
    ) {
        match self.operation(|snapshot, table| {
            let node = table.node(ino)?;
            let source = node.location.clone();
            let destination = self.child(snapshot, table, newparent, newname)?;
            if source.root != destination.root {
                return Err(Errno::EXDEV);
            }
            let (root, object) = self.object(snapshot, node)?;
            host::writable(snapshot, &source, &root, &object, false)?;
            let (old_dir, old_name) = host::parent(&root, &source)?;
            let (target_dir, target_name) = host::parent(&root, &destination)?;
            host::writable_name(
                snapshot,
                &destination,
                &root,
                &target_dir,
                &target_name,
                false,
            )?;
            fs::linkat(
                &old_dir,
                &old_name,
                &target_dir,
                &target_name,
                AtFlags::empty(),
            )
            .map_err(errno)?;
            self.entry(snapshot, table, destination)
        }) {
            Ok(attr) => reply.entry(&TTL, &attr, Generation(0)),
            Err(error) => reply.error(error),
        }
    }

    fn setattr(
        &self,
        _req: &Request,
        ino: INodeNo,
        mode: Option<u32>,
        uid: Option<u32>,
        gid: Option<u32>,
        size: Option<u64>,
        atime: Option<TimeOrNow>,
        mtime: Option<TimeOrNow>,
        _ctime: Option<SystemTime>,
        fh: Option<FileHandle>,
        _crtime: Option<SystemTime>,
        _chgtime: Option<SystemTime>,
        _bkuptime: Option<SystemTime>,
        flags: Option<BsdFileFlags>,
        reply: ReplyAttr,
    ) {
        match self.operation(|snapshot, table| {
            if ino == ROOT || ino == MANIFEST {
                return Err(Errno::EROFS);
            }
            if flags.is_some_and(|flags| !flags.is_empty()) {
                return Err(Errno::EOPNOTSUPP);
            }
            let node = table.node(ino)?;
            let root = host::current_root(snapshot, &node.location, &self.state)?;
            let pinned;
            let object = if let Some(fh) = fh {
                let handle = table.handles.get(&fh.0).ok_or(Errno::EBADF)?;
                if handle.inode != ino.0 {
                    return Err(Errno::EBADF);
                }
                handle.file.as_ref().ok_or(Errno::EBADF)?
            } else {
                pinned = self.object(snapshot, node)?.1;
                &pinned
            };
            host::writable(snapshot, &node.location, &root, object, false)?;
            if let Some(size) = size {
                if host::kind(host::stat(object)?.st_mode)? != FileType::RegularFile {
                    return Err(Errno::EINVAL);
                }
                if let Some(fh) = fh {
                    self.checked_handle(snapshot, table, ino, fh, false, true)?
                        .set_len(size)
                        .map_err(host::io_error)?;
                } else {
                    let file = host::reopen(object, OFlags::WRONLY | OFlags::NONBLOCK)?;
                    file.set_len(size).map_err(host::io_error)?;
                }
            }
            if let Some(mode) = mode {
                host::chmod(object, mode)?;
            }
            if uid.is_some() || gid.is_some() {
                if uid == Some(u32::MAX) || gid == Some(u32::MAX) {
                    return Err(Errno::EINVAL);
                }
                fs::chownat(
                    object,
                    "",
                    uid.map(fs::Uid::from_raw),
                    gid.map(fs::Gid::from_raw),
                    AtFlags::EMPTY_PATH | AtFlags::SYMLINK_NOFOLLOW,
                )
                .map_err(errno)?;
            }
            if atime.is_some() || mtime.is_some() {
                fs::utimensat(
                    object,
                    "",
                    &host::timestamps(atime, mtime)?,
                    AtFlags::EMPTY_PATH | AtFlags::SYMLINK_NOFOLLOW,
                )
                .map_err(errno)?;
            }
            host::attr(ino, &host::stat(object)?)
        }) {
            Ok(attr) => reply.attr(&TTL, &attr),
            Err(error) => reply.error(error),
        }
    }

    fn mknod(
        &self,
        _req: &Request,
        parent: INodeNo,
        name: &OsStr,
        mode: u32,
        umask: u32,
        _rdev: u32,
        reply: ReplyEntry,
    ) {
        match self.operation(|snapshot, table| {
            let location = self.child(snapshot, table, parent, name)?;
            let root = host::current_root(snapshot, &location, &self.state)?;
            let (directory, name) = host::parent(&root, &location)?;
            host::writable_name(snapshot, &location, &root, &directory, &name, false)?;
            match mode & libc::S_IFMT {
                libc::S_IFREG => {
                    host::beneath(
                        &directory,
                        Path::new(&name),
                        OFlags::WRONLY | OFlags::CREATE | OFlags::EXCL | OFlags::NOFOLLOW,
                        Mode::from_bits_truncate((mode & !umask) & 0o7777),
                    )?;
                }
                libc::S_IFSOCK => {
                    // The kernel associates bind/connect with the FUSE inode.
                    // This host inode persists the pathname only; it has no
                    // listening socket and grants no connection to host IPC.
                    fs::mknodat(
                        &directory,
                        &name,
                        fs::FileType::Socket,
                        Mode::from_bits_truncate((mode & !umask) & 0o7777),
                        0,
                    )
                    .map_err(errno)?;
                }
                _ => return Err(Errno::EOPNOTSUPP),
            }
            self.entry(snapshot, table, location)
        }) {
            Ok(attr) => reply.entry(&TTL, &attr, Generation(0)),
            Err(error) => reply.error(error),
        }
    }

    fn statfs(&self, _req: &Request, ino: INodeNo, reply: ReplyStatfs) {
        match self.operation(|snapshot, table| {
            let root = if ino == ROOT || ino == MANIFEST {
                let location = Location {
                    root: RootId::catalog(
                        snapshot
                            .roots
                            .iter()
                            .find(|root| root.primary)
                            .ok_or(Errno::ENOENT)?,
                    ),
                    relative: PathBuf::new(),
                };
                host::current_root(snapshot, &location, &self.state)?
            } else {
                self.object(snapshot, table.node(ino)?)?.0
            };
            fs::fstatvfs(root).map_err(errno)
        }) {
            Ok(stat) => reply.statfs(
                stat.f_blocks,
                stat.f_bfree,
                stat.f_bavail,
                stat.f_files,
                stat.f_ffree,
                u32::try_from(stat.f_bsize).unwrap_or(u32::MAX),
                u32::try_from(stat.f_namemax).unwrap_or(u32::MAX),
                u32::try_from(stat.f_frsize).unwrap_or(u32::MAX),
            ),
            Err(error) => reply.error(error),
        }
    }

    fn access(&self, _req: &Request, ino: INodeNo, mask: AccessFlags, reply: ReplyEmpty) {
        match self.operation(|snapshot, table| {
            if ino == ROOT || ino == MANIFEST {
                return if mask.bits() & libc::W_OK != 0 {
                    Err(Errno::EROFS)
                } else {
                    Ok(())
                };
            }
            let node = table.node(ino)?;
            let (root, object) = self.object(snapshot, node)?;
            if mask.bits() & libc::W_OK != 0 {
                host::writable(snapshot, &node.location, &root, &object, false)?;
            }
            if node.location.relative.as_os_str().is_empty() {
                return Ok(());
            }
            // The broker descriptor is O_PATH and cannot back accessat, even
            // with EMPTY_PATH. Check the same node through its containing root.
            host::access(
                &root,
                &node.location.relative,
                u32::try_from(mask.bits()).unwrap_or(0) & !(libc::W_OK as u32),
            )?;
            Ok(())
        }) {
            Ok(()) => reply.ok(),
            Err(error) => reply.error(error),
        }
    }
    fn getxattr(&self, _req: &Request, ino: INodeNo, _name: &OsStr, _size: u32, reply: ReplyXattr) {
        match self
            .operation(|snapshot, table| self.attributes(snapshot, table, ino, None).map(|_| ()))
        {
            Ok(()) => reply.error(Errno::EOPNOTSUPP),
            Err(error) => reply.error(error),
        }
    }

    fn listxattr(&self, _req: &Request, ino: INodeNo, _size: u32, reply: ReplyXattr) {
        match self
            .operation(|snapshot, table| self.attributes(snapshot, table, ino, None).map(|_| ()))
        {
            Ok(()) => reply.error(Errno::EOPNOTSUPP),
            Err(error) => reply.error(error),
        }
    }

    fn setxattr(
        &self,
        _req: &Request,
        ino: INodeNo,
        _name: &OsStr,
        _value: &[u8],
        _flags: i32,
        _position: u32,
        reply: ReplyEmpty,
    ) {
        match self.operation(|snapshot, table| {
            if ino == ROOT || ino == MANIFEST {
                return Err(Errno::EROFS);
            }
            let node = table.node(ino)?;
            let (root, object) = self.object(snapshot, node)?;
            host::writable(snapshot, &node.location, &root, &object, false)?;
            Err::<(), _>(Errno::EOPNOTSUPP)
        }) {
            Ok(()) => reply.ok(),
            Err(error) => reply.error(error),
        }
    }

    fn removexattr(&self, req: &Request, ino: INodeNo, name: &OsStr, reply: ReplyEmpty) {
        self.setxattr(req, ino, name, &[], 0, 0, reply);
    }
}
