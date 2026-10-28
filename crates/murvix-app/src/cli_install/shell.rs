use std::fs;
use std::io::{self, Read, Write};
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant};

const BEGIN: &str = "# >>> Murvix managed PATH >>>";
const END: &str = "# <<< Murvix managed PATH <<<";
const MARKER: &str = "__MURVIX_SHELL_V1__";
const LIMIT: usize = 64 * 1024;
const CONFIG_LIMIT: u64 = 1024 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Shell {
    Zsh,
    Bash,
    Fish,
}

struct Environment {
    shell: Shell,
    path: String,
    resolved: String,
    kind: String,
    zdotdir: String,
    xdg: String,
}

struct Plan {
    files: Vec<PathBuf>,
    block: String,
    manual: &'static str,
}

impl Shell {
    fn from_path(path: &Path) -> io::Result<Self> {
        match path.file_name().and_then(|name| name.to_str()) {
            Some("zsh") => Ok(Self::Zsh),
            Some("bash") => Ok(Self::Bash),
            Some("fish") => Ok(Self::Fish),
            _ => Err(io::Error::other(format!(
                "Shell {} needs manual setup. Add ~/.local/bin at the END of your shell's PATH; do not replace existing commands.",
                path.display()
            ))),
        }
    }

    fn script(self) -> &'static str {
        match self {
            Self::Zsh | Self::Bash => concat!(
                "printf '__MURVIX_SHELL_V1__\\0'; ",
                "printf '%s\\0' \"$PATH\" \"$(command -v murvix 2>/dev/null)\" '' \"${ZDOTDIR-}\" \"${XDG_CONFIG_HOME-}\""
            ),
            Self::Fish => concat!(
                "set -l murvix_result (command -s murvix 2>/dev/null); ",
                "set -l murvix_kind (type -t murvix 2>/dev/null); ",
                "printf '__MURVIX_SHELL_V1__\\0'; ",
                "printf '%s\\0' (string join : $PATH) \"$murvix_result\" \"$murvix_kind\" \"$ZDOTDIR\" \"$XDG_CONFIG_HOME\""
            ),
        }
    }
}

/// Shell startup files are executable user code. Keep the probe off the UI
/// thread, bound both output and runtime, and kill the entire probe group.
fn capture(mut command: Command) -> io::Result<Vec<u8>> {
    let mut child = command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .process_group(0)
        .spawn()?;
    let group = child.id();
    let mut pipe = child
        .stdout
        .take()
        .ok_or_else(|| io::Error::other("Missing shell output"))?;
    let (sender, receiver) = mpsc::sync_channel(4);
    std::thread::spawn(move || {
        let mut buffer = [0; 4096];
        loop {
            match pipe.read(&mut buffer) {
                Ok(0) => break,
                Ok(size) => {
                    if sender.send(Ok(buffer[..size].to_vec())).is_err() {
                        return;
                    }
                }
                Err(error) => {
                    let _ = sender.send(Err(error));
                    return;
                }
            }
        }
    });
    let deadline = Instant::now() + Duration::from_secs(3);
    let mut bytes = Vec::new();
    let result = loop {
        if Instant::now() >= deadline {
            break Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "The login shell did not finish within three seconds. Use manual PATH setup or fix its startup files.",
            ));
        }
        match receiver.recv_timeout(Duration::from_millis(20)) {
            Ok(Ok(chunk)) => {
                if bytes.len() + chunk.len() > LIMIT {
                    break Err(io::Error::other(
                        "Shell startup output exceeded 64 KiB; use manual PATH setup.",
                    ));
                }
                bytes.extend_from_slice(&chunk);
            }
            Ok(Err(error)) => break Err(error),
            Err(mpsc::RecvTimeoutError::Timeout) => (),
            Err(mpsc::RecvTimeoutError::Disconnected) => match child.try_wait() {
                Ok(Some(status)) if status.success() => break Ok(bytes),
                Ok(Some(status)) => {
                    break Err(io::Error::other(format!(
                        "Shell inspection failed: {status}"
                    )));
                }
                Ok(None) => std::thread::sleep(Duration::from_millis(20)),
                Err(error) => break Err(error),
            },
        }
    };
    // Match the existing provider process-group cancellation convention.
    let _ = Command::new("/bin/kill")
        .args(["-KILL", "--", &format!("-{group}")])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
    let _ = child.kill();
    let _ = child.wait();
    result
}

fn login_shell() -> io::Result<PathBuf> {
    // Finder's inherited SHELL can be stale after chsh. Ask the account
    // directory first, falling back to SHELL on systems without that facility.
    let mut id = Command::new("/usr/bin/id");
    id.arg("-un");
    if let Ok(user) = capture(id) {
        let user = String::from_utf8_lossy(&user);
        let user = user.trim();
        #[cfg(target_os = "macos")]
        {
            let mut query = Command::new("/usr/bin/dscl");
            query.args([".", "-read", &format!("/Users/{user}"), "UserShell"]);
            if let Ok(value) = capture(query) {
                let value = String::from_utf8_lossy(&value);
                if let Some((_, shell)) = value.split_once("UserShell:") {
                    let path = PathBuf::from(shell.trim());
                    if path.is_absolute() {
                        return Ok(path);
                    }
                }
            }
        }
        #[cfg(not(target_os = "macos"))]
        {
            let mut query = Command::new("/usr/bin/getent");
            query.args(["passwd", user]);
            if let Ok(value) = capture(query) {
                let value = String::from_utf8_lossy(&value);
                if let Some(shell) = value.trim().split(':').nth(6) {
                    let path = PathBuf::from(shell);
                    if path.is_absolute() {
                        return Ok(path);
                    }
                }
            }
        }
    }
    std::env::var_os("SHELL").map(PathBuf::from).filter(|path| path.is_absolute())
        .ok_or_else(|| io::Error::other("Cannot determine your login shell. Manually append ~/.local/bin to your shell's PATH."))
}

fn inspect() -> io::Result<Environment> {
    let executable = login_shell()?;
    let shell = Shell::from_path(&executable)?;
    let mut command = Command::new(executable);
    command.args(["-l", "-i", "-c", shell.script()]);
    parse(shell, &capture(command)?)
}

fn parse(shell: Shell, bytes: &[u8]) -> io::Result<Environment> {
    let text = std::str::from_utf8(bytes)
        .map_err(|_| io::Error::other("Shell environment is not UTF-8; use manual PATH setup."))?;
    let (_, output) = text.rsplit_once(&format!("{MARKER}\0")).ok_or_else(|| {
        io::Error::other("Shell startup prevented PATH inspection; use manual PATH setup.")
    })?;
    let fields: Vec<_> = output.split_terminator('\0').take(5).collect();
    if fields.len() != 5
        || output
            .as_bytes()
            .iter()
            .filter(|byte| **byte == 0)
            .take(5)
            .count()
            < 5
    {
        return Err(io::Error::other(
            "Shell environment inspection was incomplete; use manual PATH setup.",
        ));
    }
    Ok(Environment {
        shell,
        path: fields[0].into(),
        resolved: fields[1].into(),
        kind: fields[2].into(),
        zdotdir: fields[3].into(),
        xdg: fields[4].into(),
    })
}

fn directory(value: &str, fallback: PathBuf) -> io::Result<PathBuf> {
    if value.is_empty() {
        return Ok(fallback);
    }
    let path = PathBuf::from(value);
    if path.is_absolute() {
        Ok(path)
    } else {
        Err(io::Error::other(
            "The shell's configuration directory is relative. Use manual PATH setup rather than changing an ambiguous file.",
        ))
    }
}

fn plan(home: &Path, environment: &Environment) -> io::Result<Plan> {
    let posix = concat!(
        "case \":${PATH-}:\" in\n",
        "    *:\"$HOME/.local/bin\":*) ;;\n",
        "    *) export PATH=\"${PATH:+$PATH:}$HOME/.local/bin\" ;;\n",
        "esac"
    );
    let (files, body, manual) = match environment.shell {
        Shell::Zsh => (
            vec![directory(&environment.zdotdir, home.to_path_buf())?.join(".zshrc")],
            posix,
            "export PATH=\"${PATH:+$PATH:}$HOME/.local/bin\"",
        ),
        Shell::Bash => {
            let login = [".bash_profile", ".bash_login", ".profile"]
                .into_iter()
                .map(|name| home.join(name))
                .find(|path| fs::symlink_metadata(path).is_ok())
                .unwrap_or_else(|| home.join(".bash_profile"));
            (
                vec![home.join(".bashrc"), login],
                posix,
                "export PATH=\"${PATH:+$PATH:}$HOME/.local/bin\"",
            )
        }
        Shell::Fish => (
            vec![directory(&environment.xdg, home.join(".config"))?.join("fish/config.fish")],
            "if not contains -- \"$HOME/.local/bin\" $PATH\n    set -gx PATH $PATH \"$HOME/.local/bin\"\nend",
            "set -gx PATH $PATH \"$HOME/.local/bin\"",
        ),
    };
    Ok(Plan {
        files,
        block: format!("{BEGIN}\n{body}\n{END}\n"),
        manual,
    })
}

pub(crate) fn shell_disclosure() -> String {
    match inspect().and_then(|environment| plan(&super::home()?, &environment)) {
        Ok(plan) => format!(
            "Optional shell setup will add or update only this managed block in:\n{}\n\n{}\nIt appends ~/.local/bin only when absent, preserving existing command precedence. Existing external terminals need a new shell.\n\nManual option (no files changed): add this to your custom shell configuration:\n{}\nOr invoke ~/.local/bin/murvix directly. Remove the marked block to undo shell setup.",
            plan.files
                .iter()
                .map(|path| path.display().to_string())
                .collect::<Vec<_>>()
                .join("\n"),
            plan.block,
            plan.manual,
        ),
        Err(error) => format!(
            "Automatic shell setup is unavailable: {error}\nNo shell files will be changed. For custom setups, append ~/.local/bin to PATH in your shell configuration, or invoke ~/.local/bin/murvix directly. Open a new shell afterward."
        ),
    }
}

pub(crate) fn status(destination: &Path) -> String {
    match inspect() {
        Ok(environment) => resolution_status(destination, &environment),
        Err(error) => format!(
            "Could not inspect the user's login shell: {error}\nUse ~/.local/bin/murvix directly, or append ~/.local/bin to PATH in your custom shell configuration. Existing external terminals need a new shell."
        ),
    }
}

fn resolution_status(destination: &Path, environment: &Environment) -> String {
    let on_path = destination.parent().is_some_and(|parent| {
        std::env::split_paths(&environment.path).any(|entry| entry == parent)
    });
    let same_command = Path::new(&environment.resolved) == destination
        || (Path::new(&environment.resolved).is_absolute()
            && fs::canonicalize(&environment.resolved)
                .ok()
                .zip(fs::canonicalize(destination).ok())
                .is_some_and(|(actual, expected)| actual == expected));
    let shadowed = !environment.resolved.is_empty() && !same_command
        || (!environment.kind.is_empty() && environment.kind != "file");
    let health = if shadowed {
        format!(
            "Another command shadows Murvix in your login shell: {} ({}). Existing Homebrew/standalone commands, aliases and functions are not replaced. Adjust your own shell configuration if you want this app's CLI; otherwise run {} directly.",
            environment.resolved,
            environment.kind,
            destination.display()
        )
    } else if same_command {
        "The bundled murvix command resolves correctly in a newly opened login shell.".into()
    } else if on_path {
        "~/.local/bin is on your login shell's PATH, but murvix does not resolve. Repair the CLI link and check shell startup configuration.".into()
    } else {
        "~/.local/bin is not on your login shell's PATH. Choose the disclosed shell setup, or manually append it in your custom shell configuration.".into()
    };
    format!(
        "{health}\nExisting external terminals may need a new shell; setup never changes their environment."
    )
}

pub(crate) fn configure_shell() -> io::Result<()> {
    super::persistent_integration_allowed()?;
    let home = super::home()?;
    let environment = inspect()?;
    let plan = plan(&home, &environment)?;
    let _lock = murvix_client::local::try_lock(&home.join(".murvix-shell-install.lock"))?;
    // Validate every affected file before changing any of them (notably bash's
    // separate login and interactive startup files).
    let changes = plan
        .files
        .iter()
        .map(|path| prepare(path, &plan.block))
        .collect::<io::Result<Vec<_>>>()?;
    for change in changes {
        change.apply()?;
    }
    Ok(())
}

struct Change {
    path: PathBuf,
    contents: String,
    original: Option<(u64, u64, Vec<u8>, fs::Permissions)>,
}

fn prepare(path: &Path, block: &str) -> io::Result<Change> {
    let original = match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.is_file() && !metadata.is_symlink() => {
            if metadata.len() > CONFIG_LIMIT {
                return Err(io::Error::other(
                    "Shell configuration exceeds 1 MiB; use manual setup.",
                ));
            }
            let mut bytes = Vec::new();
            fs::File::open(path)?
                .take(CONFIG_LIMIT + 1)
                .read_to_end(&mut bytes)?;
            if bytes.len() as u64 > CONFIG_LIMIT {
                return Err(io::Error::other(
                    "Shell configuration is too large; use manual setup.",
                ));
            }
            Some((
                metadata.dev(),
                metadata.ino(),
                bytes,
                metadata.permissions(),
            ))
        }
        Ok(_) => {
            return Err(io::Error::other(format!(
                "{} is a symlink or non-regular file; Murvix will not replace it. Use manual shell setup.",
                path.display()
            )));
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => None,
        Err(error) => return Err(error),
    };
    let previous = original
        .as_ref()
        .map_or(&[][..], |(_, _, bytes, _)| bytes.as_slice());
    let previous = std::str::from_utf8(previous)
        .map_err(|_| io::Error::other("Shell configuration is not UTF-8; use manual setup."))?;
    Ok(Change {
        path: path.to_path_buf(),
        contents: managed_contents(previous, block)?,
        original,
    })
}

impl Change {
    fn apply(self) -> io::Result<()> {
        if self
            .original
            .as_ref()
            .is_some_and(|(_, _, bytes, _)| bytes.as_slice() == self.contents.as_bytes())
        {
            return Ok(());
        }
        let parent = self
            .path
            .parent()
            .ok_or_else(|| io::Error::other("Missing shell configuration directory"))?;
        fs::create_dir_all(parent)?;
        let mut temporary = tempfile::NamedTempFile::new_in(parent)?;
        temporary
            .as_file()
            .set_permissions(self.original.as_ref().map_or_else(
                || fs::Permissions::from_mode(0o600),
                |(_, _, _, permissions)| permissions.clone(),
            ))?;
        temporary.write_all(self.contents.as_bytes())?;
        temporary.as_file().sync_all()?;
        let unchanged = match (fs::symlink_metadata(&self.path), &self.original) {
            (Ok(metadata), Some((device, inode, bytes, _))) => {
                metadata.is_file()
                    && !metadata.is_symlink()
                    && (metadata.dev(), metadata.ino()) == (*device, *inode)
                    && metadata.len() == bytes.len() as u64
                    && fs::read(&self.path)? == *bytes
            }
            (Err(error), None) => error.kind() == io::ErrorKind::NotFound,
            _ => false,
        };
        if !unchanged {
            return Err(io::Error::other(
                "Shell configuration changed during setup; retry after saving it.",
            ));
        }
        temporary.persist(&self.path).map_err(|error| error.error)?;
        fs::File::open(parent)?.sync_all()
    }
}

fn managed_contents(previous: &str, block: &str) -> io::Result<String> {
    let mut begin = None;
    let mut end = None;
    let mut offset = 0;
    for line in previous.split_inclusive('\n') {
        let value = line.trim_end_matches(['\r', '\n']);
        if value == BEGIN && begin.replace(offset).is_some() {
            return Err(io::Error::other(
                "Duplicate Murvix shell blocks; use manual setup.",
            ));
        }
        if value == END && end.replace(offset + line.len()).is_some() {
            return Err(io::Error::other(
                "Duplicate Murvix shell blocks; use manual setup.",
            ));
        }
        offset += line.len();
    }
    match (begin, end) {
        (None, None) => Ok(format!(
            "{previous}{}{block}",
            if previous.is_empty() || previous.ends_with('\n') {
                ""
            } else {
                "\n"
            }
        )),
        (Some(start), Some(finish)) if start < finish => Ok(format!(
            "{}{block}{}",
            &previous[..start],
            &previous[finish..]
        )),
        _ => Err(io::Error::other(
            "The Murvix shell block is incomplete; fix its markers or use manual setup.",
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn environment(shell: Shell) -> Environment {
        Environment {
            shell,
            path: String::new(),
            resolved: String::new(),
            kind: String::new(),
            zdotdir: String::new(),
            xdg: String::new(),
        }
    }

    #[test]
    fn managed_blocks_are_idempotent_and_preserve_unrelated_configuration() -> io::Result<()> {
        for shell in [Shell::Zsh, Shell::Bash, Shell::Fish] {
            let plan = plan(Path::new("/home/person"), &environment(shell))?;
            let once = managed_contents("# custom\nexport EDITOR=vi", &plan.block)?;
            assert!(once.starts_with("# custom\nexport EDITOR=vi\n"));
            assert_eq!(managed_contents(&once, &plan.block)?, once);
            assert_eq!(once.matches(BEGIN).count(), 1);
            let with_tail = format!("{once}# keep this too\n");
            assert_eq!(managed_contents(&with_tail, &plan.block)?, with_tail);
        }
        assert!(managed_contents(BEGIN, "replacement").is_err());
        assert!(managed_contents(&format!("{END}\n{BEGIN}\n"), "replacement").is_err());
        Ok(())
    }

    #[test]
    fn shell_specific_locations_respect_actual_startup_environment() -> io::Result<()> {
        let home = tempfile::tempdir()?;
        let mut zsh = environment(Shell::Zsh);
        zsh.zdotdir = "/custom/zsh".into();
        assert_eq!(
            plan(home.path(), &zsh)?.files,
            vec![PathBuf::from("/custom/zsh/.zshrc")]
        );
        let mut fish = environment(Shell::Fish);
        fish.xdg = "/custom/config".into();
        assert_eq!(
            plan(home.path(), &fish)?.files,
            vec![PathBuf::from("/custom/config/fish/config.fish")]
        );
        fs::write(home.path().join(".profile"), "# existing\n")?;
        assert_eq!(
            plan(home.path(), &environment(Shell::Bash))?.files,
            vec![home.path().join(".bashrc"), home.path().join(".profile")]
        );
        Ok(())
    }

    #[test]
    fn shell_changes_are_isolated_and_refuse_dotfile_symlinks() -> io::Result<()> {
        let home = tempfile::tempdir()?;
        let file = home.path().join(".zshrc");
        let block = plan(home.path(), &environment(Shell::Zsh))?.block;
        fs::write(&file, "# existing\n")?;
        prepare(&file, &block)?.apply()?;
        let once = fs::read(&file)?;
        prepare(&file, &block)?.apply()?;
        assert_eq!(fs::read(&file)?, once);
        fs::remove_file(&file)?;
        std::os::unix::fs::symlink(home.path().join("other"), &file)?;
        assert!(prepare(&file, &block).is_err());
        Ok(())
    }

    #[test]
    fn existing_commands_are_reported_as_shadowing_without_precedence_changes() -> io::Result<()> {
        let home = tempfile::tempdir()?;
        let destination = home.path().join(".local/bin/murvix");
        let mut shell = environment(Shell::Zsh);
        shell.path = "/opt/homebrew/bin:/usr/bin".into();
        shell.resolved = "/opt/homebrew/bin/murvix".into();
        assert!(resolution_status(&destination, &shell).contains("shadows"));
        shell.resolved = "murvix is a function".into();
        assert!(resolution_status(&destination, &shell).contains("shadows"));
        shell.resolved = destination.to_string_lossy().into_owned();
        assert!(resolution_status(&destination, &shell).contains("resolves correctly"));
        let block = plan(home.path(), &shell)?.block;
        assert!(block.contains("${PATH:+$PATH:}$HOME/.local/bin"));
        Ok(())
    }

    #[test]
    fn probe_parser_ignores_startup_noise() -> io::Result<()> {
        let output = b"welcome\n__MURVIX_SHELL_V1__\0/usr/bin\0/opt/homebrew/bin/murvix\0\0/custom/zsh\0/custom/config\0goodbye";
        let parsed = parse(Shell::Zsh, output)?;
        assert_eq!(parsed.path, "/usr/bin");
        assert_eq!(parsed.zdotdir, "/custom/zsh");
        assert_eq!(parsed.xdg, "/custom/config");
        assert!(parse(Shell::Zsh, b"__MURVIX_SHELL_V1__\0/usr/bin\0").is_err());
        Ok(())
    }

    #[test]
    fn shell_probes_bound_output_without_reading_user_startup_files() {
        let mut command = Command::new("/bin/sh");
        command.args([
            "-c",
            "while :; do printf '0123456789012345678901234567890123456789'; done",
        ]);
        let error = capture(command).expect_err("oversized probe must fail");
        assert!(error.to_string().contains("64 KiB"));
    }
}
