//! `murvix tab`: list, open, switch, and close terminal tabs in the
//! desktop app on this computer.

use std::ffi::OsString;
use std::io;

use crate::args::Command;
use crate::manage::help;

use super::{Invocation, Output};

/// Reads the words after `murvix tab`.
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
        return Ok(Command::Usage(help::TAB));
    }
    invocation(&words).map(|invocation| Command::Layout(Box::new(invocation)))
}

fn invocation(words: &[&str]) -> io::Result<Invocation> {
    let (action, rest) = words.split_first().ok_or_else(unknown)?;
    match *action {
        "list" => {
            check(rest, "list", 0..=0)?;
            Ok(Invocation::new("tab.list").output(Output::Tabs))
        }
        "new" => {
            check(rest, "new", 0..=0)?;
            Ok(Invocation::new("tab.new").output(Output::Text))
        }
        "open" => {
            let given = Given::parse(
                rest,
                "open [--directory DIR] [--command COMMAND]",
                &["--directory", "--command"],
            )?;
            let mut invocation = Invocation::new("tab.open")
                .arg("kind", "terminal")
                .output(Output::Text);
            if let Some(directory) = given.value("--directory") {
                invocation = invocation.arg("directory", directory);
            }
            if let Some(command) = given.value("--command") {
                invocation = invocation.arg("command", command);
            }
            Ok(invocation)
        }
        "switch" => {
            check(rest, "switch <tab>", 1..=1)?;
            Ok(Invocation::new("tab.switch")
                .arg("identifier", rest[0])
                .output(Output::Ok))
        }
        "next" => {
            check(rest, "next", 0..=0)?;
            Ok(Invocation::new("tab.next").output(Output::Ok))
        }
        "previous" => {
            check(rest, "previous", 0..=0)?;
            Ok(Invocation::new("tab.previous").output(Output::Ok))
        }
        "close" => {
            check(rest, "close [tab]", 0..=1)?;
            let mut invocation = Invocation::new("tab.close").output(Output::Ok);
            if let Some(identifier) = rest.first() {
                invocation = invocation.arg("identifier", *identifier);
            }
            Ok(invocation)
        }
        _ => Err(unknown()),
    }
}

fn check(rest: &[&str], usage: &str, count: std::ops::RangeInclusive<usize>) -> io::Result<()> {
    if count.contains(&rest.len()) {
        Ok(())
    } else {
        Err(invalid(&format!("usage: murvix tab {usage}")))
    }
}

struct Given<'a> {
    values: std::collections::BTreeMap<&'static str, &'a str>,
}

impl<'a> Given<'a> {
    fn parse(rest: &[&'a str], usage: &str, values: &[&'static str]) -> io::Result<Self> {
        let mut given = Self {
            values: std::collections::BTreeMap::new(),
        };
        let mut rest = rest.iter().copied();
        while let Some(word) = rest.next() {
            if word == "--" {
                return Err(invalid(&format!("usage: murvix tab {usage}")));
            }
            if let Some(option) = values.iter().copied().find(|option| *option == word) {
                let value = rest
                    .next()
                    .ok_or_else(|| invalid(&format!("missing value for {option}")))?;
                if given.values.insert(option, value).is_some() {
                    return Err(invalid(&format!("duplicate option {option}")));
                }
            } else {
                return Err(invalid(&format!("usage: murvix tab {usage}")));
            }
        }
        Ok(given)
    }
    fn value(&self, option: &str) -> Option<&'a str> {
        self.values.get(option).copied()
    }
}

fn unknown() -> io::Error {
    invalid("unknown tab command; run murvix tab --help")
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
            ("list", "tab.list", json!({})),
            ("new", "tab.new", json!({})),
            ("open", "tab.open", json!({"kind": "terminal"})),
            (
                "open --directory ./api --command npm-test",
                "tab.open",
                json!({"kind": "terminal", "directory": "./api", "command": "npm-test"}),
            ),
            ("switch 1", "tab.switch", json!({"identifier": "1"})),
            ("next", "tab.next", json!({})),
            ("previous", "tab.previous", json!({})),
            ("close", "tab.close", json!({})),
            ("close 2", "tab.close", json!({"identifier": "2"})),
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
            vec!["list", "extra"],
            vec!["switch"],
            vec!["switch", "1", "2"],
            vec!["close", "1", "2"],
            vec!["open", "extra"],
            vec!["open", "--directory"],
        ] {
            assert!(invocation(&words).is_err(), "{words:?}");
        }
    }
}
