use std::{
    fs::File,
    io::{BufReader, Read},
    time::Instant,
};

use anyhow::Context;
use lexopt::Arg;

use crate::{node::Node, utils::is_stdin_readable};

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
mod utils;
/// The interactive TUI JSON viewer
mod viewer;

fn main() -> anyhow::Result<()> {
    let stdin = std::io::stdin();
    let piped_input = is_stdin_readable();
    let mut parser = lexopt::Parser::from_env();

    let mut command = None;
    let mut path = None;

    // TODO: if `piped_input`, conflict if `path` is provided? just ignore the piped input?
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

    // This is a bit unsightly, but the idea is to allow the `path` argument to not be required if the input is piped.
    // Also, if no specific command is provided, the default is to view the JSON interactively
    let (command, path) = match (command, path) {
        (Some(Command::Help), _) => {
            help_message();
            return Ok(());
        }
        (None, None) if !piped_input => {
            help_message();
            return Ok(());
        }
        (Some(command), Some(path)) => (command, path.into()),
        (None, Some(path)) => (Command::View, Some(path.into())),
        (Some(command), None) if !piped_input => {
            return Err(lexopt::Error::MissingValue {
                option: Some(format!("{:?}", command)),
            }
            .into());
        }
        (Some(command), None) => (command, None),
        (None, None) => (Command::View, None),
    };

    let json = if let Some(path) = path {
        let now = Instant::now();
        let file = File::open(&path)
            .with_context(|| format!("failed to open file: {}", path.display()))?;
        let mut reader = BufReader::new(file);
        let json = serde_json::from_reader(&mut reader).unwrap();
        let elapsed = now.elapsed();
        eprintln!("Time taken to load JSON: {:?}ms", elapsed.as_millis());
        json
    } else {
        debug_assert!(piped_input);
        let mut buf = Vec::with_capacity(1024);
        stdin.lock().read_to_end(&mut buf).unwrap();
        serde_json::from_slice(&buf).unwrap()
    };

    match command {
        Command::View => {
            let root = Node::from_value(json);
            viewer::start_viewer(root)?;
        }
        Command::Flatten => {
            flatten::flatten(json)?;
        }
        Command::Unflatten => {}
        Command::Help => {
            unreachable!()
        }
    }

    Ok(())
}

fn help_message() {
    // TODO: flesh this out
    println!("Usage: jk [command] [path]");
}
