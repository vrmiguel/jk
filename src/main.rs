use std::{
    fs::File,
    io::{self, BufReader, Read},
    path::PathBuf,
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

/// Where to load data from
enum Source {
    Stdin,
    File(PathBuf),
}

enum SourceIterator {
    Stdin(io::Bytes<io::StdinLock<'static>>),
    File(io::Bytes<BufReader<File>>),
}

impl Iterator for SourceIterator {
    type Item = Result<u8, io::Error>;

    fn next(&mut self) -> Option<Self::Item> {
        match self {
            SourceIterator::Stdin(iter) => iter.next(),
            SourceIterator::File(iter) => iter.next(),
        }
    }
}

impl Source {
    fn load(self) -> anyhow::Result<Vec<u8>> {
        match self {
            Source::Stdin => {
                let mut buf = Vec::with_capacity(1024);
                io::stdin().lock().read_to_end(&mut buf).unwrap();
                Ok(buf)
            }
            Source::File(path) => {
                let buf = std::fs::read(&path)
                    .with_context(|| format!("failed to read file: {}", path.display()))?;
                Ok(buf)
            }
        }
    }

    fn iterator(self) -> anyhow::Result<SourceIterator> {
        match self {
            Source::Stdin => Ok(SourceIterator::Stdin(io::stdin().lock().bytes())),
            Source::File(path) => {
                let file = File::open(&path)
                    .with_context(|| format!("failed to open file: {}", path.display()))?;
                let reader = BufReader::new(file);
                
                Ok(SourceIterator::File(reader.bytes()))
            }
        }
    }
}

mod borrowed_value;
/// Prints a flattened version of the loaded JSON
mod flatten;
/// A version of serde_json::Value that tracks which parts of it are collapsed/expanded
mod node;
/// Reverts `jk flatten` to its original form
mod unflatten;
mod utils;
/// The interactive TUI JSON viewer
mod viewer;

fn main() -> anyhow::Result<()> {
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
    let (command, source) = match (command, path) {
        (Some(Command::Help), _) => {
            help_message();
            return Ok(());
        }
        (None, None) if !piped_input => {
            help_message();
            return Ok(());
        }
        (Some(command), Some(path)) => (command, Source::File(path.into())),
        (None, Some(path)) => (Command::View, Source::File(path.into())),
        (Some(command), None) if !piped_input => {
            return Err(lexopt::Error::MissingValue {
                option: Some(format!("{:?}", command)),
            }
            .into());
        }
        (Some(command), None) => (command, Source::Stdin),
        (None, None) => (Command::View, Source::Stdin),
    };

    match command {
        Command::View => {
            let buf = source.load()?;
            let json = serde_json::from_slice(&buf).unwrap();
            let root = Node::from_value(json);
            viewer::start_viewer(root)?;
        }
        Command::Flatten => {
            let iter = source.iterator()?;
            let parser = jsax::Parser::new(iter);

            flatten::flatten(parser)?;
        }
        Command::Unflatten => {
            let buf = source.load()?;
            let utf8 = std::str::from_utf8(&buf).unwrap();
            unflatten::unflatten(utf8).unwrap();
        }
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
