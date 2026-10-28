//! Local native launches use argv directly; no shell parsing or permission overrides.

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
    command.args(&arguments[..boundary]);
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
    command.args(&arguments[boundary..]);
    Err(format!("could not launch {}: {}", agent.command(), command.exec()).into())
}
