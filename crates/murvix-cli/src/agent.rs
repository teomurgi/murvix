//! Named entry points share the same agent-neutral filesystem launcher.

use std::ffi::OsString;

use murvix_app_core::agent_launch::Agent;

pub(crate) fn run(agent: Agent, arguments: &[OsString]) -> Result<(), Box<dyn std::error::Error>> {
    let mut command = Vec::with_capacity(arguments.len() + 1);
    command.push(OsString::from(agent.command()));
    command.extend_from_slice(arguments);
    crate::workspace::run(crate::workspace::Invocation::Run(command))
}
