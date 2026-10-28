use crate::file_lock::FileLock;
use std::fs::{self, File, OpenOptions};
use std::io;
use std::os::unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

#[derive(Debug)]
pub struct BundleLease {
    _file: File,
}

pub fn lock_replacement(directory: &Path) -> io::Result<FileLock> {
    #[cfg(target_os = "macos")]
    {
        lock_replacement_with_runtime(directory, lease_directory)
    }
    #[cfg(not(target_os = "macos"))]
    {
        crate::file_lock::try_lock(&directory.join(".murvix-update.lock"))
    }
}

#[cfg(target_os = "macos")]
fn lock_replacement_with_runtime(
    directory: &Path,
    runtime: impl FnOnce() -> io::Result<PathBuf>,
) -> io::Result<FileLock> {
    let path = directory.join(".murvix-update.lock");
    match crate::file_lock::try_lock(&path) {
        Err(error)
            if matches!(
                error.kind(),
                io::ErrorKind::PermissionDenied | io::ErrorKind::ReadOnlyFilesystem
            ) =>
        {
            match crate::file_lock::try_lock_existing(&path) {
                Err(missing) if missing.kind() == io::ErrorKind::NotFound => {}
                result => return result,
            }
            // Keep the existing lock when accessible, including contention with
            // an older client. Read-only installs coordinate this user's clients
            // in the same private runtime directory as their bundle leases.
            let runtime = runtime()?;
            fs::DirBuilder::new()
                .recursive(true)
                .mode(0o700)
                .create(&runtime)?;
            crate::file_lock::try_lock(
                &runtime.join(format!("replacement-{}.lock", identity(directory)?)),
            )
        }
        result => result,
    }
}

/// The app bundle whose `Contents/MacOS` holds `executable`.
pub fn containing(executable: &Path) -> Option<&Path> {
    executable
        .parent()
        .and_then(Path::parent)
        .and_then(Path::parent)
        .filter(|path| path.extension().is_some_and(|extension| extension == "app"))
}

pub fn acquire_runtime(
    executable: &Path,
    expected_metadata: &[u8],
) -> io::Result<Option<BundleLease>> {
    #[cfg(target_os = "linux")]
    if let Some(root) = linux_root(executable) {
        let directory = executable
            .parent()
            .ok_or_else(|| io::Error::other("Missing executable directory"))?;
        let _replacement = wait_for_replacement(root)?;
        if crate::executable::build_metadata(executable)? != expected_metadata {
            return Err(io::Error::other(
                "The installed Murvix changed during startup. Run the command again.",
            ));
        }
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .open(directory.join(".lease"))?;
        file.lock_shared()?;
        return Ok(Some(BundleLease { _file: file }));
    }
    let Some(bundle) = containing(executable) else {
        return Ok(None);
    };
    let identity = identity(bundle)?;
    let deadline = Instant::now() + Duration::from_secs(3);
    let _replacement = loop {
        match lock_replacement(
            bundle
                .parent()
                .ok_or_else(|| io::Error::other("Missing application directory"))?,
        ) {
            Err(error)
                if error.kind() == io::ErrorKind::WouldBlock && Instant::now() < deadline =>
            {
                std::thread::sleep(Duration::from_millis(25));
            }
            result => break result?,
        }
    };
    if self::identity(bundle)? != identity
        || crate::executable::build_metadata(executable)? != expected_metadata
    {
        return Err(io::Error::other(
            "The installed Murvix changed during startup. Run the command again.",
        ));
    }
    let file = lease_file(&lease_directory()?, &identity)?;
    file.lock_shared()?;
    Ok(Some(BundleLease { _file: file }))
}

/// The root of a versioned, user-installed Linux desktop, including its matching helpers.
#[cfg(target_os = "linux")]
pub fn linux_root(executable: &Path) -> Option<&Path> {
    let version = executable.parent()?;
    let versions = version.parent()?;
    let root = versions.parent()?;
    (versions.file_name()? == "versions"
        && crate::release::parse(version.file_name()?.to_str()?).is_some()
        && root.join("installation.json").is_file())
    .then_some(root)
}

#[cfg(target_os = "linux")]
fn wait_for_replacement(root: &Path) -> io::Result<FileLock> {
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        match lock_replacement(root) {
            Err(error)
                if error.kind() == io::ErrorKind::WouldBlock && Instant::now() < deadline =>
            {
                std::thread::sleep(Duration::from_millis(25));
            }
            result => return result,
        }
    }
}

pub fn lock_unused(bundle: &Path) -> io::Result<Option<File>> {
    let file = lease_file(&lease_directory()?, &identity(bundle)?)?;
    match file.try_lock() {
        Ok(()) => Ok(Some(file)),
        Err(fs::TryLockError::WouldBlock) => Ok(None),
        Err(fs::TryLockError::Error(error)) => Err(error),
    }
}

fn identity(bundle: &Path) -> io::Result<String> {
    let metadata = fs::metadata(bundle)?;
    Ok(format!("{:x}-{:x}", metadata.dev(), metadata.ino()))
}

fn lease_directory() -> io::Result<PathBuf> {
    let home =
        std::env::home_dir().ok_or_else(|| io::Error::other("Home directory unavailable"))?;
    Ok(home.join("Library/Application Support/Murvix Runtime/leases"))
}

fn lease_file(directory: &Path, identity: &str) -> io::Result<File> {
    fs::DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(directory)?;
    OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .mode(0o600)
        .open(directory.join(format!("{identity}.lock")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(target_os = "macos")]
    #[test]
    fn readonly_installations_keep_an_exclusive_startup_lock_without_bypassing_contention()
    -> io::Result<()> {
        use std::os::unix::fs::PermissionsExt;

        let root = tempfile::tempdir()?;
        let installation = root.path().join("Applications");
        let runtime = root.path().join("runtime");
        fs::create_dir(&installation)?;
        fs::set_permissions(&installation, fs::Permissions::from_mode(0o555))?;
        let owner = lock_replacement_with_runtime(&installation, || Ok(runtime.clone()))?;
        assert!(
            lock_replacement_with_runtime(&installation.join("."), || Ok(runtime.clone()))
                .is_err_and(|error| error.kind() == io::ErrorKind::WouldBlock)
        );
        drop(owner);
        drop(lock_replacement_with_runtime(&installation, || {
            Ok(runtime.clone())
        })?);
        fs::set_permissions(&installation, fs::Permissions::from_mode(0o755))?;
        let legacy_path = installation.join(".murvix-update.lock");
        let legacy = crate::file_lock::try_lock(&legacy_path)?;
        assert!(
            lock_replacement_with_runtime(&installation, || Ok(runtime.clone()))
                .is_err_and(|error| error.kind() == io::ErrorKind::WouldBlock)
        );
        fs::set_permissions(&legacy_path, fs::Permissions::from_mode(0o444))?;
        assert!(
            lock_replacement_with_runtime(&installation, || Ok(runtime.clone()))
                .is_err_and(|error| error.kind() == io::ErrorKind::WouldBlock)
        );
        drop(legacy);
        let owner = lock_replacement_with_runtime(&installation, || Ok(runtime.clone()))?;
        assert!(
            lock_replacement_with_runtime(&installation, || Ok(runtime))
                .is_err_and(|error| error.kind() == io::ErrorKind::WouldBlock)
        );
        drop(owner);
        Ok(())
    }

    #[test]
    fn only_executables_inside_an_app_bundle_have_one() {
        assert_eq!(
            containing(Path::new(
                "/Applications/Murvix.app/Contents/MacOS/murvix-app"
            )),
            Some(Path::new("/Applications/Murvix.app"))
        );
        assert_eq!(containing(Path::new("/repo/target/debug/murvix-app")), None);
    }

    #[test]
    fn leases_follow_bundle_identity_through_replacement_without_holding_installation_lock()
    -> io::Result<()> {
        let root = tempfile::tempdir()?;
        let current = root.path().join("Murvix Beta.app");
        let retired = root.path().join("previous.app");
        fs::create_dir(&current)?;
        let lease = lease_file(root.path(), &identity(&current)?)?;
        lease.lock_shared()?;
        let _installation = lock_replacement(root.path())?;
        fs::rename(&current, &retired)?;
        fs::create_dir(&current)?;
        let old = lease_file(root.path(), &identity(&retired)?)?;
        assert!(matches!(old.try_lock(), Err(fs::TryLockError::WouldBlock)));
        let new = lease_file(root.path(), &identity(&current)?)?;
        new.try_lock()?;
        drop(lease);
        old.try_lock()?;
        Ok(())
    }
}
