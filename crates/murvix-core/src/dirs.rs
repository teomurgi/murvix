use std::env;
use std::ffi::OsString;
use std::fs;
use std::io;
use std::os::unix::fs::DirBuilderExt;
use std::path::PathBuf;



pub fn muxy_dir() -> io::Result<PathBuf> {
    let linux = cfg!(target_os = "linux");
    let directory = resolve_directory(
        env::var_os("MUXY_DIR"),
        env::var_os("HOME"),
        env::var_os("XDG_STATE_HOME"),
        directory_name(cfg!(debug_assertions), linux),
        linux,
    )?;
    fs::DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(&directory)?;
    Ok(directory)
}

/// Debug builds never share data with releases.
fn directory_name(debug: bool, linux: bool) -> &'static str {
    match (debug, linux) {
        (true, false) => "Murvix Dev",
        (true, true) => "murvix-dev",
        (false, false) => "Murvix",
        (false, true) => "murvix",
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
    fn releases_and_development_builds_do_not_share_data_on_either_platform() -> io::Result<()> {
        for linux in [false, true] {
            for (debug, macos_name, linux_name) in [
                (true, "Murvix Dev", "murvix-dev"),
                (false, "Murvix", "murvix"),
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
                        directory_name(debug, linux),
                        linux
                    )?,
                    PathBuf::from("/home/test").join(expected)
                );
            }
        }
        Ok(())
    }
}
