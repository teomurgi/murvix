//! Workspace-aware native agent entry points shared by desktop and CLI.

use std::ffi::{OsStr, OsString};
use std::io;
use std::path::Path;

use murvix_protocol::AgentProvider;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Agent {
    Claude,
    Codex,
    Copilot,
    OhMyPi,
}

pub const AGENTS: [Agent; 4] = [Agent::Claude, Agent::Codex, Agent::Copilot, Agent::OhMyPi];

impl Agent {
    pub fn from_command(command: &str) -> Option<Self> {
        Some(match command {
            "claude" => Self::Claude,
            "codex" => Self::Codex,
            "copilot" => Self::Copilot,
            "omp" => Self::OhMyPi,
            _ => return None,
        })
    }

    pub fn from_provider(provider: AgentProvider) -> Option<Self> {
        Some(match provider {
            AgentProvider::Claude => Self::Claude,
            AgentProvider::Codex => Self::Codex,
            AgentProvider::Copilot => Self::Copilot,
            AgentProvider::OhMyPi => Self::OhMyPi,
            _ => return None,
        })
    }

    pub fn command(self) -> &'static str {
        match self {
            Self::Claude => "claude",
            Self::Codex => "codex",
            Self::Copilot => "copilot",
            Self::OhMyPi => "omp",
        }
    }

    pub fn provider(self) -> AgentProvider {
        match self {
            Self::Claude => AgentProvider::Claude,
            Self::Codex => AgentProvider::Codex,
            Self::Copilot => AgentProvider::Copilot,
            Self::OhMyPi => AgentProvider::OhMyPi,
        }
    }
}

/// Quotes a launcher command, not a snapshot of project paths. The CLI resolves
/// the terminal's current workspace immediately before executing the native tool.
/// Refuses non-UTF-8 command text instead of relying on shell-specific escapes.
pub fn launch_command(cli: &Path, agent: Agent, arguments: &[OsString]) -> io::Result<String> {
    let mut command = String::new();
    for component in std::iter::once(cli.as_os_str())
        .chain([
            OsStr::new("agent"),
            OsStr::new(agent.command()),
            OsStr::new("--"),
        ])
        .chain(arguments.iter().map(OsString::as_os_str))
    {
        let text = component.to_str().ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "cannot type a non-UTF-8 agent command into a terminal",
            )
        })?;
        if !command.is_empty() {
            command.push(' ');
        }
        command.push('\'');
        for character in text.chars() {
            if character == '\'' {
                command.push_str("'\\''");
            } else {
                command.push(character);
            }
        }
        command.push('\'');
    }
    Ok(command)
}
