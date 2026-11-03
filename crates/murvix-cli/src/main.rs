//! Command-line and terminal client for the server on this computer or, with
//! `--host`, on another one; the server runs as a separate executable.
mod agent;
mod args;
mod browser;
mod clipboard;
mod input;
mod manage;
mod mobile;
mod open;
mod projects;
mod scroll;
mod selection;
mod skills;
mod state;
mod target;
mod terminal;
mod tui;
mod ui;
mod worker;
mod workspace;

use args::Command;
use murvix_client::Start;
use std::io::{self, Write};
use std::process::ExitCode;
use target::Target;

#[derive(Debug)]
struct CommandExit(std::process::ExitStatus);

impl std::fmt::Display for CommandExit {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(formatter)
    }
}

impl std::error::Error for CommandExit {}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            if let Some(exit) = error.downcast_ref::<CommandExit>() {
                return ExitCode::from(u8::try_from(exit.0.code().unwrap_or(1)).unwrap_or(1));
            }
            let _ = writeln!(io::stderr(), "murvix: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let (host, command) = args::parse(&std::env::args_os().skip(1).collect::<Vec<_>>())?;
    match command {
        Command::Help => writeln!(io::stdout(), "{}", manage::help::ROOT)?,
        Command::Usage(usage) => writeln!(io::stdout(), "{usage}")?,
        Command::Version => writeln!(io::stdout(), "murvix {}", env!("CARGO_PKG_VERSION"))?,
        Command::BuildInfo => {
            serde_json::to_writer(io::stdout().lock(), &murvix_protocol::BuildInfo::current())?;
        }
        Command::Interactive => tui::run(host, None).map_err(io::Error::other)?,
        Command::Manage(command) => manage::run(*command, host)?,
        Command::Agent(agent, arguments) => agent::run(agent, &arguments)?,
        Command::Workspace(invocation) => workspace::run(invocation)?,
        Command::Open(folder) => open::run(&folder)?,
        Command::Browser(invocation) => browser::run(*invocation)?,
        Command::InstallSkills(directories) => {
            let home = std::env::home_dir();
            let mut stdout = io::stdout().lock();
            for file in skills::install(home.as_deref(), &directories)? {
                writeln!(stdout, "{}", file.display())?;
            }
        }
        Command::Mobile(command) => {
            let _lease = murvix_client::local::bundle::acquire_runtime(
                &murvix_core::executable::current_path()?,
            )?;
            let target = Target::new(host)?;
            let client = target
                .connect(Start::IfNeeded)
                .map_err(|error| target.explain(error))?;
            mobile::run(command, &client).map_err(|error| target.explain(error))?;
        }
        Command::Stdio(start) => {
            let _lease = murvix_client::local::bundle::acquire_runtime(
                &murvix_core::executable::current_path()?,
            )?;
            murvix_client::bridge::serve(
                &murvix_core::dirs::murvix_dir()?.join("server.sock"),
                &murvix_client::local::server_executable()?,
                start,
            )?;
        }
    }
    Ok(())
}
