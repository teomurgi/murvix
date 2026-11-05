//! Local native launches use argv directly; no shell parsing or permission overrides.
//! `Pi`, `Hermes`, and `OpenCode` lack a native multi-root launcher: they start in the
//! primary directory and discover extra roots through the installed Murvix skill.

use std::ffi::OsString;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::process::CommandExt;
use std::process::Command;

use murvix_app_core::agent_launch::Agent;
use murvix_client::Start;

pub(crate) fn run(agent: Agent, arguments: &[OsString]) -> Result<(), Box<dyn std::error::Error>> {
    let _lease =
        murvix_client::local::bundle::acquire_runtime(&murvix_core::executable::current_path()?)?;
    let target = crate::target::Target::new(None)?;
    let client = target
        .connect(Start::Never)
        .map_err(|error| target.explain(error))?;
    let project = (|| {
        client.identify(murvix_protocol::ClientKind::Cli)?;
        crate::manage::current_project(&client)
    })();
    client.disconnect();
    let project = project?;
    if !agent.supports_add_dir() {
        let message = match crate::skills::status(std::env::home_dir().as_deref()) {
            crate::skills::SkillStatus::Current => None,
            crate::skills::SkillStatus::Stale => {
                Some("the installed murvix-cli skill is out of date")
            }
            crate::skills::SkillStatus::Missing => Some("the murvix-cli skill is not installed"),
        };
        if let Some(reason) = message {
            use std::io::Write as _;
            let _ = writeln!(
                std::io::stderr(),
                "murvix: {reason}; run `murvix install-skills` so {} agents discover every workspace folder",
                agent.command()
            );
        }
    }
    let roots = murvix_protocol::workspace_roots(&project);
    // Native global options precede subcommands. Claude's --add-dir is
    // variadic, so its explicit = values follow original options/prompts,
    // but precede a native --, without consuming a positional prompt.
    let boundary = if agent == Agent::Claude {
        arguments
            .iter()
            .position(|word| word == "--")
            .unwrap_or(arguments.len())
    } else {
        0
    };
    let mut command = Command::new(agent.command());
    if agent == Agent::OpenCode {
        // Only `run` accepts --dir; the bare TUI takes a positional project
        // path instead. Extra roots arrive via the installed skill.
        command.arg("run");
        if let Some(directory) = roots.first() {
            command
                .arg("--dir")
                .arg(std::ffi::OsStr::from_bytes(&directory.0));
        }
    } else if agent == Agent::Hermes {
        // Hermes accepts one working directory via its global --in option.
        // Extra roots arrive via the installed skill.
        if let Some(directory) = roots.first() {
            command
                .arg("--in")
                .arg(std::ffi::OsStr::from_bytes(&directory.0));
        }
    }
    command.args(&arguments[..boundary]);
    if agent.supports_add_dir() {
        for root in roots {
            let directory = std::ffi::OsStr::from_bytes(&root.0);
            if matches!(agent, Agent::Claude | Agent::Copilot) {
                let mut option = OsString::from("--add-dir=");
                option.push(directory);
                command.arg(option);
            } else {
                command.arg("--add-dir").arg(directory);
            }
        }
    }
    command.args(&arguments[boundary..]);
    Err(format!("could not launch {}: {}", agent.command(), command.exec()).into())
}
