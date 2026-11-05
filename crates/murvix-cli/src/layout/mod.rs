//! `murvix tab` and `murvix pane`: terminal layout in the desktop app on
//! this computer. Each command is one request to the app over its socket,
//! like `murvix browser`. Layout lives in the app, not the server, so these
//! commands never take `--host` and need the app running.

pub(crate) mod pane;
pub(crate) mod tab;

use std::error::Error;
use std::io::{self, Write};
use std::time::Duration;

use murvix_app_core::browser::api::{self, CallError, Request};
use serde_json::{Map, Value};

/// How long the app has to answer, unless `MURVIX_CLI_TIMEOUT` says otherwise.
const REPLY_TIME: Duration = Duration::from_secs(10);
const INVALID_REPLY: &str = "invalid reply from Murvix";

/// A layout command as the request it sends to the desktop app.
#[derive(Debug, Eq, PartialEq)]
pub(crate) struct Invocation {
    pub(crate) verb: &'static str,
    pub(crate) args: Map<String, Value>,
    pub(crate) output: Output,
}

/// How the app's reply is printed.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Output {
    /// `ok`, once the app has done it.
    Ok,
    Json,
    /// A string as is, such as a new tab or pane ID.
    Text,
    /// One tab-separated line per tab.
    Tabs,
    /// One tab-separated line per pane.
    Panes,
}

impl Invocation {
    fn new(verb: &'static str) -> Self {
        Self {
            verb,
            args: Map::new(),
            output: Output::Json,
        }
    }
    fn arg(mut self, key: &str, value: impl Into<Value>) -> Self {
        self.args.insert(key.to_owned(), value.into());
        self
    }
    fn output(self, output: Output) -> Self {
        Self { output, ..self }
    }
}

pub(crate) fn run(invocation: Invocation) -> Result<(), Box<dyn Error>> {
    let socket = api::socket_path(&murvix_core::dirs::murvix_dir()?);
    let request = Request::new(invocation.verb, invocation.args);
    let reply = api::call(&socket, &request, reply_time()?).map_err(|error| match error {
        CallError::Failed(error) => failed(&error),
        error => Box::<dyn Error>::from(error),
    })?;
    print(invocation.output, reply)
}

fn reply_time() -> Result<Duration, Box<dyn Error>> {
    let Some(seconds) = std::env::var_os("MURVIX_CLI_TIMEOUT").filter(|value| !value.is_empty())
    else {
        return Ok(REPLY_TIME);
    };
    seconds
        .to_str()
        .and_then(|seconds| seconds.parse().ok())
        .filter(|seconds| *seconds > 0)
        .map(Duration::from_secs)
        .ok_or_else(|| "MURVIX_CLI_TIMEOUT must be a whole number of seconds".into())
}
fn print(output: Output, reply: Value) -> Result<(), Box<dyn Error>> {
    let mut stdout = io::stdout().lock();
    match output {
        Output::Ok => writeln!(stdout, "ok")?,
        Output::Json => writeln!(stdout, "{reply}")?,
        Output::Text => match reply {
            Value::String(text) => writeln!(stdout, "{text}")?,
            other => writeln!(stdout, "{other}")?,
        },
        Output::Tabs => {
            for tab in reply.as_array().ok_or(INVALID_REPLY)? {
                let row: Vec<_> = ["index", "id", "kind", "title", "isActive"]
                    .iter()
                    .map(|key| cell(&tab[key]))
                    .collect();
                writeln!(stdout, "{}", row.join("\t"))?;
            }
        }
        Output::Panes => {
            for pane in reply.as_array().ok_or(INVALID_REPLY)? {
                let row: Vec<_> = ["id", "title", "workingDirectory", "isFocused"]
                    .iter()
                    .map(|key| cell(&pane[key]))
                    .collect();
                writeln!(stdout, "{}", row.join("\t"))?;
            }
        }
    }
    Ok(())
}

/// A list field kept on its line: tabs and line breaks become spaces.
fn cell(value: &Value) -> String {
    match value {
        Value::String(text) => text.replace(['\t', '\n', '\r'], " "),
        Value::Null => String::new(),
        other => other.to_string(),
    }
}
/// The CLI error for an app-side failure.
pub(crate) fn failed(error: &str) -> Box<dyn Error> {
    format!("the Murvix desktop app reported: {error}").into()
}
