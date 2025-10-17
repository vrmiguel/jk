use std::{
    fs::File,
    io::{BufReader, IsTerminal, Read},
    ops::Not,
    path::PathBuf,
    time::Instant,
};

use crate::node::Node;
use anyhow::Context;
use lexopt::Arg;

#[derive(Debug)]
enum Command {
    View,
    Flatten,
    Unflatten,
    Help,
}

/// Prints a flattened version of the loaded JSON
mod flatten;
/// A version of serde_json::Value that tracks which parts of it are collapsed/expanded
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
                option: Some(format!("{:?}", command)),
            }
            .into());
        }
    };

    let json = if piped_input {
        let mut buf = Vec::with_capacity(1024);
        stdin.lock().read_to_end(&mut buf).unwrap();
        let utf8_error = std::str::from_utf8(&buf).unwrap();
        eprintln!("{utf8_error}");
        serde_json::from_slice(&buf).unwrap()
    } else {
        let now = Instant::now();
        let file = File::open(&path)
            .with_context(|| format!("failed to open file: {}", path.display()))?;
        let mut reader = BufReader::new(file);
        let json = serde_json::from_reader(&mut reader).unwrap();
        let elapsed = now.elapsed();
        eprintln!("Time taken to load JSON: {:?}ms", elapsed.as_millis());
        json
    };

    match command {
        Command::View => {
            let root = Node::from_value(json);
            viewer::start_viewer(root)?;
        }
        Command::Flatten => {
            flatten::flatten(json);
        }
        Command::Unflatten => {}
        Command::Help => {
            unreachable!()
        }
    }

    Ok(())
}
