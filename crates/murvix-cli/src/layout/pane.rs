//! `murvix pane`: list, split, focus, close, and drive terminal panes in
//! the desktop app on this computer.

use std::ffi::OsString;
use std::io;

use crate::args::Command;
use crate::manage::help;

use super::{Invocation, Output};

/// Reads the words after `murvix pane`.
pub(crate) fn parse(arguments: &[OsString]) -> io::Result<Command> {
    let words = arguments
        .iter()
        .map(|word| {
            word.to_str()
                .ok_or_else(|| invalid("text arguments must be UTF-8"))
        })
        .collect::<io::Result<Vec<_>>>()?;
    let wants_help = words
        .iter()
        .take_while(|word| **word != "--")
        .any(|word| matches!(*word, "--help" | "-h"));
    if words.is_empty() || wants_help {
        return Ok(Command::Usage(help::PANE));
    }
    invocation(&words).map(|invocation| Command::Layout(Box::new(invocation)))
}

#[allow(
    clippy::too_many_lines,
    reason = "Keep the pane action routing table together"
)]
fn invocation(words: &[&str]) -> io::Result<Invocation> {
    let (action, rest) = words.split_first().ok_or_else(unknown)?;
    match *action {
        "list" => {
            check(rest, "list", 0..=0)?;
            Ok(Invocation::new("pane.list").output(Output::Panes))
        }
        "split" => {
            check(rest, "split [pane] [--direction left|right|up|down]", 0..=3)?;
            let mut pane: Option<&str> = None;
            let mut direction: Option<&str> = None;
            let mut rest = rest.iter().copied();
            while let Some(word) = rest.next() {
                if word == "--direction" {
                    let value = rest
                        .next()
                        .ok_or_else(|| invalid("missing value for --direction"))?;
                    if direction.replace(value).is_some() {
                        return Err(invalid("duplicate option --direction"));
                    }
                } else if word == "--" || pane.replace(word).is_some() {
                    return Err(invalid(
                        "usage: murvix pane split [pane] [--direction left|right|up|down]",
                    ));
                }
            }
            if let Some(direction) = direction
                && !["left", "right", "up", "down"].contains(&direction)
            {
                return Err(invalid("invalid direction; use left, right, up, or down"));
            }
            let mut invocation = Invocation::new("pane.split").output(Output::Text);
            if let Some(pane) = pane {
                invocation = invocation.arg("pane", pane);
            }
            if let Some(direction) = direction {
                invocation = invocation.arg("direction", direction);
            }
            Ok(invocation)
        }
        "focus" => {
            check(rest, "focus [pane]", 0..=1)?;
            let mut invocation = Invocation::new("pane.focus").output(Output::Ok);
            if let Some(pane) = rest.first() {
                invocation = invocation.arg("pane", *pane);
            }
            Ok(invocation)
        }
        "close" => {
            check(rest, "close [pane]", 0..=1)?;
            let mut invocation = Invocation::new("pane.close").output(Output::Ok);
            if let Some(pane) = rest.first() {
                invocation = invocation.arg("pane", *pane);
            }
            Ok(invocation)
        }
        "send" => {
            check(rest, "send [pane] TEXT", 1..=2)?;
            let (pane, text) = if rest.len() == 2 {
                (Some(rest[0]), rest[1])
            } else {
                (None, rest[0])
            };
            let mut invocation = Invocation::new("pane.send").output(Output::Ok);
            if let Some(pane) = pane {
                invocation = invocation.arg("pane", pane);
            }
            Ok(invocation.arg("text", text))
        }
        "send-keys" => {
            check(rest, "send-keys [pane] KEY", 1..=2)?;
            let (pane, key) = if rest.len() == 2 {
                (Some(rest[0]), rest[1])
            } else {
                (None, rest[0])
            };
            let mut invocation = Invocation::new("pane.sendKeys").output(Output::Ok);
            if let Some(pane) = pane {
                invocation = invocation.arg("pane", pane);
            }
            Ok(invocation.arg("key", key))
        }
        "read" => {
            check(rest, "read [pane] [--lines N]", 0..=3)?;
            let mut pane: Option<&str> = None;
            let mut lines: Option<&str> = None;
            let mut rest = rest.iter().copied();
            while let Some(word) = rest.next() {
                if word == "--lines" {
                    let value = rest
                        .next()
                        .ok_or_else(|| invalid("missing value for --lines"))?;
                    if lines.replace(value).is_some() {
                        return Err(invalid("duplicate option --lines"));
                    }
                } else if word == "--" || pane.replace(word).is_some() {
                    return Err(invalid("usage: murvix pane read [pane] [--lines N]"));
                }
            }
            let lines: u64 = match lines {
                None => 50,
                Some(lines) => lines
                    .parse()
                    .map_err(|_| invalid("invalid value for --lines"))?,
            };
            let mut invocation = Invocation::new("pane.readScreen").output(Output::Text);
            if let Some(pane) = pane {
                invocation = invocation.arg("pane", pane);
            }
            Ok(invocation.arg("lines", lines))
        }
        "rename" => {
            check(rest, "rename [pane] TITLE", 1..=2)?;
            let (pane, title) = if rest.len() == 2 {
                (Some(rest[0]), rest[1])
            } else {
                (None, rest[0])
            };
            let mut invocation = Invocation::new("pane.rename").output(Output::Ok);
            if let Some(pane) = pane {
                invocation = invocation.arg("pane", pane);
            }
            Ok(invocation.arg("title", title))
        }
        _ => Err(unknown()),
    }
}

fn check(rest: &[&str], usage: &str, count: std::ops::RangeInclusive<usize>) -> io::Result<()> {
    if count.contains(&rest.len()) {
        Ok(())
    } else {
        Err(invalid(&format!("usage: murvix pane {usage}")))
    }
}

fn unknown() -> io::Error {
    invalid("unknown pane command; run murvix pane --help")
}

fn invalid(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, message)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{Value, json};

    fn invocation(words: &[&str]) -> io::Result<Invocation> {
        match parse(&words.iter().map(OsString::from).collect::<Vec<_>>())? {
            Command::Layout(invocation) => Ok(*invocation),
            other => Err(io::Error::other(format!("{other:?} is not a request"))),
        }
    }

    fn request(words: &[&str]) -> io::Result<(&'static str, Value)> {
        let invocation = invocation(words)?;
        Ok((invocation.verb, Value::Object(invocation.args)))
    }

    #[test]
    fn every_action_sends_its_verb_and_arguments() -> Result<(), Box<dyn std::error::Error>> {
        for (line, verb, args) in [
            ("list", "pane.list", json!({})),
            ("split", "pane.split", json!({})),
            (
                "split --direction down",
                "pane.split",
                json!({"direction": "down"}),
            ),
            ("focus", "pane.focus", json!({})),
            ("close", "pane.close", json!({})),
            ("send hello", "pane.send", json!({"text": "hello"})),
            ("send-keys Enter", "pane.sendKeys", json!({"key": "Enter"})),
            ("read", "pane.readScreen", json!({"lines": 50})),
            ("read --lines 20", "pane.readScreen", json!({"lines": 20})),
            ("rename Work", "pane.rename", json!({"title": "Work"})),
        ] {
            let words: Vec<_> = line.split(' ').collect();
            assert_eq!(request(&words)?, (verb, args), "{line}");
        }
        Ok(())
    }

    #[test]
    fn rejects_unknown_actions_and_bad_arities() {
        for words in [
            vec!["nope"],
            vec!["split", "--direction"],
            vec!["split", "--direction", "sideways"],
            vec!["send"],
            vec!["read", "--lines", "many"],
            vec!["rename"],
        ] {
            assert!(invocation(&words).is_err(), "{words:?}");
        }
    }
}
