use std::fs;
use std::io;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::{MetadataExt, OpenOptionsExt, symlink};
use std::path::{Path, PathBuf};

mod shell;

pub(crate) use shell::{configure_shell, shell_disclosure};

use crate::model::AppModel;
use gpui::{Context, SharedString};
use murvix_core::capability::{Capability, Feature};
use murvix_ui::tr;

/// The installer links the `murvix` that was installed with the running app.
pub(crate) const CAPABILITY: Capability = Capability::Supported;

impl AppModel {
    pub(crate) fn install_command_line(&mut self, cx: &mut Context<Self>) {
        if !self.require(Feature::CommandLineTool, cx) {
            return;
        }
        let window = self.window;
        cx.spawn(async move |model, cx| {
            let (installed, title, message) = cx
                .background_executor()
                .spawn(async {
                    let result = install();
                    let installed = result.is_ok();
                    let (title, mut message) = installation_message(result);
                    if installed {
                        message.push_str("\n\n");
                        message.push_str(&shell_disclosure());
                    }
                    (installed, title, message)
                })
                .await;
            if installed {
                let _ = model.update(cx, AppModel::remember_cli_enabled);
            }
            let response = window.update(cx, |_, window, cx| {
                if installed {
                    window.prompt(
                        gpui::PromptLevel::Info,
                        &title,
                        Some(&message),
                        &[
                            tr!("Configure Shell").as_ref(),
                            tr!("Use Manual Setup").as_ref(),
                        ],
                        cx,
                    )
                } else {
                    window.prompt(
                        gpui::PromptLevel::Warning,
                        &title,
                        Some(&message),
                        &[tr!("OK").as_ref()],
                        cx,
                    )
                }
            });
            if installed
                && let Ok(answer) = response
                && let Ok(0) = answer.await
            {
                let message = cx
                    .background_executor()
                    .spawn(async {
                        match configure_shell() {
                            Ok(()) => {
                                integration_status().unwrap_or_else(|error| error.to_string())
                            }
                            Err(error) => error.to_string(),
                        }
                    })
                    .await;
                if let Ok(answer) = window.update(cx, |_, window, cx| {
                    window.prompt(
                        gpui::PromptLevel::Info,
                        tr!("Command Line Integration").as_ref(),
                        Some(&message),
                        &[tr!("OK").as_ref()],
                        cx,
                    )
                }) {
                    let _ = answer.await;
                }
            }
        })
        .detach();
    }
}

fn installation_message(result: io::Result<PathBuf>) -> (SharedString, String) {
    match result {
        Ok(path) => {
            let guidance = integration_status().unwrap_or_else(|error| error.to_string());
            (
                tr!("Command Line Tool Installed"),
                tr!("Installed %@.\n\n%@", path.display().to_string(), &guidance).to_string(),
            )
        }
        Err(error) => (
            tr!("Could Not Install Command Line Tool"),
            error.to_string(),
        ),
    }
}

pub(crate) fn install() -> io::Result<PathBuf> {
    let target = validated_tool()?;
    let destination = home()?.join(".local/bin/murvix");
    if target != destination {
        install_link(&target, &destination)?;
    }
    Ok(destination)
}

fn validated_tool() -> io::Result<PathBuf> {
    let target = installed_tool(&murvix_core::executable::current_path()?)?;
    for executable in [&target, &target.with_file_name("murvix-server")] {
        if crate::server::read_build_info(executable)? != murvix_protocol::BuildInfo::current() {
            return Err(io::Error::other(
                tr!(
                    "The installed app changed. Reopen it before installing the command line tool."
                )
                .to_string(),
            ));
        }
    }
    Ok(target)
}

/// A shared guard for every action that persists integration.
pub(crate) fn persistent_integration_allowed() -> io::Result<()> {
    installed_tool(&murvix_core::executable::current_path()?).map(|_| ())
}

pub(crate) fn repair_owned() -> io::Result<()> {
    let target = validated_tool()?;
    let destination = home()?.join(".local/bin/murvix");
    if target != destination {
        install_link_inner(&target, &destination, true)?;
    }
    Ok(())
}

fn home() -> io::Result<PathBuf> {
    std::env::home_dir().ok_or_else(|| io::Error::other("Home directory unavailable"))
}

pub(crate) fn integration_status() -> io::Result<String> {
    let destination = home()?.join(".local/bin/murvix");
    let target = installed_tool(&murvix_core::executable::current_path()?)?;
    let health = match fs::symlink_metadata(&destination) {
        Ok(metadata) if metadata.is_symlink() => {
            let linked = fs::read_link(&destination)?;
            let owned = managed_targets(&destination.with_file_name(".murvix-cli-link.json"))?
                .iter()
                .any(|value| value == linked.as_os_str().as_bytes());
            if !owned {
                format!(
                    "CLI path {} → {} is not receipt-owned; Murvix will not replace or adopt it.",
                    destination.display(),
                    linked.display()
                )
            } else if linked == target && target.is_file() {
                format!(
                    "CLI link: {} → {}.",
                    destination.display(),
                    target.display()
                )
            } else {
                format!(
                    "CLI link: {} → {} (not this installed app; repair only replaces receipt-owned links).",
                    destination.display(),
                    linked.display()
                )
            }
        }
        Ok(_) if destination == target => format!("CLI executable: {}.", destination.display()),
        Ok(_) => format!(
            "CLI path {} is occupied by an unrelated file; it will not be replaced.",
            destination.display()
        ),
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            format!("CLI link: {} is not installed.", destination.display())
        }
        Err(error) => return Err(error),
    };
    Ok(format!("{health}\n{}", shell::status(&destination)))
}

fn managed_targets(receipt: &Path) -> io::Result<Vec<Vec<u8>>> {
    match fs::read(receipt) {
        Ok(bytes) => Ok(serde_json::from_slice(&bytes)?),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(Vec::new()),
        Err(error) => Err(error),
    }
}

/// The `murvix` installed with the app at `executable`: on Linux the app, the
/// tool and the server sit in one directory. An app run from a temporary
/// folder is not installed, and a link into it would not last.
#[cfg(not(target_os = "macos"))]
fn installed_tool(executable: &Path) -> io::Result<PathBuf> {
    #[cfg(target_os = "linux")]
    if let Some(root) = murvix_core::bundle::linux_root(executable) {
        if root.join("current/murvix-app").canonicalize()? != executable {
            return Err(io::Error::other(
                tr!(
                    "The installed app changed. Reopen it before installing the command line tool."
                )
                .to_string(),
            ));
        }
        return Ok(root.join("current/murvix"));
    }
    let app_name = murvix_core::release::APP_NAME;
    let temporary = [Path::new("/tmp"), Path::new("/var/tmp")]
        .into_iter()
        .chain([std::env::temp_dir().as_path()])
        .any(|folder| executable.starts_with(folder));
    if temporary
        || executable
            .file_name()
            .is_none_or(|name| name != "murvix-app")
    {
        return Err(io::Error::other(
            tr!(
                "Install %@ and open it from there before installing its command line tool.",
                app_name
            )
            .to_string(),
        ));
    }
    Ok(executable.with_file_name("murvix"))
}

/// The `murvix` inside the app bundle at `executable`.
#[cfg(target_os = "macos")]
fn installed_tool(executable: &Path) -> io::Result<PathBuf> {
    let app_name = murvix_core::release::APP_NAME;
    let bundle = executable
        .parent()
        .and_then(Path::parent)
        .and_then(Path::parent)
        .filter(|path| path.extension().is_some_and(|extension| extension == "app"))
        .ok_or_else(|| {
            io::Error::other(
                tr!(
                    "Install and open %@.app before installing its command line tool.",
                    app_name
                )
                .to_string(),
            )
        })?;
    // Only the two supported installation directories are persistent roots.
    // An otherwise valid bundle in Downloads, an updater's staging tree or
    // another temporary mount must never become a lasting shell/login target.
    let applications = Path::new("/Applications").canonicalize().ok();
    let user_applications = home()?.join("Applications").canonicalize().ok();
    let installed_parent = bundle.parent().and_then(|path| path.canonicalize().ok());
    let installed = installed_parent.is_some()
        && (installed_parent == applications || installed_parent == user_applications);
    if !installed
        || executable
            .file_name()
            .is_none_or(|name| name != "murvix-app")
        || executable.parent() != Some(bundle.join("Contents/MacOS").as_path())
        || bundle.starts_with("/Volumes")
        || bundle.starts_with("/tmp")
        || bundle.starts_with("/private/tmp")
        || bundle.starts_with("/private/var/folders")
        || bundle.components().any(|part| {
            part.as_os_str() == "AppTranslocation"
                || part
                    .as_os_str()
                    .to_string_lossy()
                    .starts_with(".murvix-beta-update-")
        })
        || bundle
            .file_name()
            .is_some_and(|name| name == "previous.app")
    {
        return Err(io::Error::other(
            tr!(
                "Move %@.app to Applications and reopen it before installing its command line tool.",
                app_name
            )
            .to_string(),
        ));
    }
    Ok(bundle.join("Contents/MacOS/murvix"))
}

fn install_link(target: &Path, destination: &Path) -> io::Result<()> {
    install_link_inner(target, destination, false)
}

fn install_link_inner(target: &Path, destination: &Path, owned_only: bool) -> io::Result<()> {
    if !target.is_absolute() || !target.is_file() {
        return Err(io::Error::other(
            tr!("Bundled murvix executable is missing").to_string(),
        ));
    }
    let parent = destination
        .parent()
        .ok_or_else(|| io::Error::other(tr!("Missing installation directory").to_string()))?;
    if owned_only && !parent.is_dir() {
        return Ok(());
    }
    fs::create_dir_all(parent)?;
    let _lock = murvix_client::local::try_lock(&parent.join(".murvix-cli-install.lock"))?;
    let receipt = parent.join(".murvix-cli-link.json");
    let managed = managed_targets(&receipt)?;
    if owned_only && managed.is_empty() {
        return Ok(());
    }
    let existing = match fs::symlink_metadata(destination) {
        Ok(metadata) if metadata.is_symlink() => {
            let previous = fs::read_link(destination)?;
            if !managed
                .iter()
                .any(|value| value == previous.as_os_str().as_bytes())
            {
                return Err(io::Error::other(
                    tr!(
                        "%@ is an unrelated link; move it before installing Murvix.",
                        destination.display().to_string()
                    )
                    .to_string(),
                ));
            }
            Some((metadata.dev(), metadata.ino(), previous))
        }
        Ok(_) => {
            return Err(io::Error::other(
                tr!(
                    "%@ already exists; move it before installing Murvix.",
                    destination.display().to_string()
                )
                .to_string(),
            ));
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound && owned_only => return Ok(()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => None,
        Err(error) => return Err(error),
    };
    let staging = tempfile::Builder::new()
        .prefix(".murvix-link-")
        .tempdir_in(parent)?;
    let link = staging.path().join("murvix");
    symlink(target, &link)?;
    let mut targets = vec![target.as_os_str().as_bytes().to_vec()];
    if let Some((_, _, previous)) = &existing {
        targets.push(previous.as_os_str().as_bytes().to_vec());
    }
    let record = staging.path().join("receipt");
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&record)?;
    serde_json::to_writer(&mut file, &targets)?;
    file.sync_all()?;
    let unchanged = match (fs::symlink_metadata(destination), &existing) {
        (Err(error), None) => error.kind() == io::ErrorKind::NotFound,
        (Ok(metadata), Some((device, inode, _))) => {
            (metadata.dev(), metadata.ino()) == (*device, *inode)
        }
        _ => false,
    };
    if !unchanged {
        return Err(io::Error::other(
            tr!("The command path changed during installation; try again.").to_string(),
        ));
    }
    fs::rename(&record, receipt)?;
    fs::rename(link, destination)?;
    fs::File::open(parent)?.sync_all()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn installs_repairs_managed_links_and_refuses_unrelated_paths() -> io::Result<()> {
        let root = tempfile::tempdir()?;
        let target = root.path().join("App With Spaces/murvix");
        fs::create_dir(target.parent().ok_or_else(|| io::Error::other("parent"))?)?;
        fs::write(&target, b"executable")?;
        let destination = root.path().join("bin/murvix");
        install_link(&target, &destination)?;
        assert_eq!(fs::read_link(&destination)?, target);
        install_link(&target, &destination)?;
        let next = root.path().join("moved-murvix");
        fs::rename(&target, &next)?;
        install_link(&next, &destination)?;
        assert_eq!(fs::read_link(&destination)?, next);
        fs::remove_file(&destination)?;
        symlink("/usr/bin/false", &destination)?;
        assert!(install_link(&next, &destination).is_err());
        assert_eq!(
            fs::read_link(&destination)?,
            PathBuf::from("/usr/bin/false")
        );
        fs::remove_file(&destination)?;
        fs::write(&destination, b"unrelated")?;
        assert!(install_link(&next, &destination).is_err());
        assert_eq!(fs::read(&destination)?, b"unrelated");
        Ok(())
    }

    #[test]
    fn owned_repair_does_not_create_or_adopt_opted_out_links() -> io::Result<()> {
        let root = tempfile::tempdir()?;
        let target = root.path().join("murvix");
        fs::write(&target, b"executable")?;
        let destination = root.path().join("bin/murvix");
        install_link_inner(&target, &destination, true)?;
        assert!(!destination.exists());
        assert!(!destination.parent().is_some_and(Path::exists));
        install_link(&target, &destination)?;
        let moved = root.path().join("relocated-murvix");
        fs::rename(&target, &moved)?;
        install_link_inner(&moved, &destination, true)?;
        assert_eq!(fs::read_link(&destination)?, moved);
        fs::remove_file(&destination)?;
        install_link_inner(&moved, &destination, true)?;
        assert!(fs::symlink_metadata(&destination).is_err());
        symlink(&moved, &destination)?;
        fs::remove_file(destination.with_file_name(".murvix-cli-link.json"))?;
        install_link_inner(&moved, &destination, true)?;
        assert!(install_link(&moved, &destination).is_err());
        assert_eq!(fs::read_link(&destination)?, moved);
        Ok(())
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn persistent_links_require_an_installed_app_not_a_mount_or_staging_tree() -> io::Result<()> {
        assert_eq!(
            installed_tool(Path::new(
                "/Applications/Murvix.app/Contents/MacOS/murvix-app"
            ))?,
            PathBuf::from("/Applications/Murvix.app/Contents/MacOS/murvix")
        );
        for executable in [
            "/Volumes/Murvix/Murvix.app/Contents/MacOS/murvix-app",
            "/private/var/folders/random/AppTranslocation/Murvix.app/Contents/MacOS/murvix-app",
            "/tmp/Murvix.app/Contents/MacOS/murvix-app",
            "/Applications/.murvix-beta-update-123/Murvix.app/Contents/MacOS/murvix-app",
            "/Applications/previous.app/Contents/MacOS/murvix-app",
            "/Applications/Murvix.app/Contents/MacOS/murvix",
        ] {
            assert!(
                installed_tool(Path::new(executable)).is_err(),
                "{executable}"
            );
        }
        Ok(())
    }

    #[cfg(not(target_os = "macos"))]
    #[test]
    fn the_tool_is_found_beside_an_installed_app_wherever_that_is() -> io::Result<()> {
        for folder in ["/usr/bin", "/opt/Murvix App/bin", "/home/me/.local/bin"] {
            let folder = Path::new(folder);
            assert_eq!(
                installed_tool(&folder.join("murvix-app"))?,
                folder.join("murvix")
            );
        }
        for running in [
            "/tmp/.mount_Murvix12/usr/bin/murvix-app",
            "/var/tmp/build/murvix-app",
            "/usr/bin/murvix",
        ] {
            assert!(installed_tool(Path::new(running)).is_err(), "{running}");
        }
        assert!(installed_tool(&std::env::temp_dir().join("murvix-app")).is_err());
        Ok(())
    }
}
