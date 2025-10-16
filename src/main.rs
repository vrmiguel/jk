use std::{
    fmt::Display,
    fs::File,
    io::{BufReader, IsTerminal, Read},
    ops::Not,
    path::PathBuf,
};

use crate::node::Node;
use anyhow::Context;
use lexopt::Arg;

enum Command {
    View,
    Flatten,
    Unflatten,
    Help,
}

impl Display for Command {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Command::View => write!(f, "view"),
            Command::Flatten => write!(f, "flatten"),
            Command::Unflatten => write!(f, "unflatten"),
            Command::Help => write!(f, "help"),
        }
    }
}

mod node;
/// The interactive TUI JSON viewer
mod viewer;

fn main() -> anyhow::Result<()> {
    let stdin = std::io::stdin();
    let piped_input = stdin.is_terminal().not();
    let mut parser = lexopt::Parser::from_env();

    let mut command = None;
    let mut path = None;

    while let Some(arg) = parser.next()? {
        match arg {
            Arg::Value(value) if value == "flatten" && command.is_none() => {
                command = Some(Command::Flatten);
            }
            Arg::Value(value) if value == "unflatten" && command.is_none() => {
                command = Some(Command::Unflatten);
            }
            Arg::Value(value) if value == "help" && command.is_none() => {
                command = Some(Command::Help);
            }
            Arg::Value(value) if path.is_none() => {
                path = Some(value);
            }
            _ => return Err(arg.unexpected().into()),
        }
    }

    let (command, path): (Command, PathBuf) = match (command, path) {
        (None, None) | (Some(Command::Help), _) => {
            println!("Usage: jk [command] [path]");
            return Ok(());
        }
        (Some(command), Some(path)) => (command, path.into()),
        (None, Some(path)) => (Command::View, path.into()),
        (Some(command), None) => {
            return Err(lexopt::Error::MissingValue {
                option: Some(command.to_string()),
            }
            .into());
        }
    };

    let json = if piped_input {
        let mut buf = Vec::with_capacity(1024);
        stdin.lock().read_to_end(&mut buf).unwrap();
        serde_json::from_slice(&buf).unwrap()
    } else {
        let file = File::open(&path)
            .with_context(|| format!("failed to open file: {}", path.display()))?;
        let mut reader = BufReader::new(file);
        serde_json::from_reader(&mut reader).unwrap()
    };

    let root = Node::from_value(json);

    match command {
        Command::View => {
            viewer::start_viewer(root)?;
        }
        Command::Flatten => {
            todo!()
        }
        Command::Unflatten => {
            todo!()
        }
        Command::Help => {
            unreachable!()
        }
    }

    Ok(())
}
