use std::env;
use std::ffi::OsString;
use std::fs;
use std::io;
use std::os::unix::fs::DirBuilderExt;
use std::path::PathBuf;

use crate::release::{Channel, Version};

pub fn muxy_dir() -> io::Result<PathBuf> {
    let linux = cfg!(target_os = "linux");
    let directory = resolve_directory(
        env::var_os("MUXY_DIR"),
        env::var_os("HOME"),
        env::var_os("XDG_STATE_HOME"),
        directory_name(env!("CARGO_PKG_VERSION"), cfg!(debug_assertions), linux),
        linux,
    )?;
    fs::DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(&directory)?;
    Ok(directory)
}

/// Releases of each channel and local builds never share data.
fn directory_name(version: &str, debug: bool, linux: bool) -> &'static str {
    let channel = Version::parse(version)
        .filter(|_| !debug)
        .map(Version::channel);
    match (channel, linux) {
        (None, false) => "Murvix Dev",
        (None, true) => "murvix-dev",
        (Some(Channel::Beta), false) => "Murvix Beta",
        (Some(Channel::Beta), true) => "murvix-beta",
        (Some(Channel::Stable), false) => "Murvix",
        (Some(Channel::Stable), true) => "murvix",
    }
}

fn resolve_directory(
    override_path: Option<OsString>,
    home: Option<OsString>,
    state_home: Option<OsString>,
    name: &str,
    linux: bool,
) -> io::Result<PathBuf> {
    if let Some(path) = override_path {
        return if path.is_empty() {
            Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "MUXY_DIR must not be empty",
            ))
        } else {
            Ok(PathBuf::from(path))
        };
    }
    if linux && let Some(state_home) = state_home {
        let state_home = PathBuf::from(state_home);
        if !state_home.is_absolute() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "XDG_STATE_HOME must be absolute",
            ));
        }
        return Ok(state_home.join(name));
    }
    let home = home
        .filter(|home| !home.is_empty())
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "HOME is not set"))?;
    Ok(PathBuf::from(home)
        .join(if linux {
            ".local/state"
        } else {
            "Library/Application Support"
        })
        .join(name))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn channels_and_development_builds_do_not_share_data_on_either_platform() -> io::Result<()> {
        for linux in [false, true] {
            for (version, debug, macos_name, linux_name) in [
                ("2.0.0-beta-0", true, "Murvix Dev", "murvix-dev"),
                ("2.0.0-beta-0", false, "Murvix Dev", "murvix-dev"),
                ("2.0.0-beta.1001", true, "Murvix Dev", "murvix-dev"),
                ("2.0.0-beta.1001", false, "Murvix Beta", "murvix-beta"),
                ("2.0.0", true, "Murvix Dev", "murvix-dev"),
                ("2.0.0", false, "Murvix", "murvix"),
            ] {
                let expected = if linux {
                    format!(".local/state/{linux_name}")
                } else {
                    format!("Library/Application Support/{macos_name}")
                };
                assert_eq!(
                    resolve_directory(
                        None,
                        Some("/home/test".into()),
                        None,
                        directory_name(version, debug, linux),
                        linux
                    )?,
                    PathBuf::from("/home/test").join(expected)
                );
            }
        }
        Ok(())
    }
}
