use std::{
    io::{self, BufWriter},
    process::ExitCode,
};

use jsax::Parser;
use lexopt::Arg;

use crate::{node::Node, source::Source, utils::is_stdin_readable};

/// A version of serde_json::Value that tracks which parts of it are collapsed/expanded
mod node;
mod source;
mod utils;
/// The interactive TUI JSON viewer
mod viewer;

#[derive(Debug)]
enum Command {
    View,
    Flatten,
    Unflatten,
    Fmt,
    Help,
}

fn main() -> ExitCode {
    if let Err(err) = run() {
        eprintln!("{err:?}");
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}

fn run() -> anyhow::Result<()> {
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
            Arg::Value(value) if value == "fmt" && command.is_none() => {
                command = Some(Command::Fmt);
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
            let source = source.load()?;
            let json = serde_json::from_slice(source.as_bytes()).unwrap();
            let root = Node::from_value(json);
            viewer::start_viewer(root)?;
        }
        Command::Flatten => {
            let source = source.load()?;

            let stdout = io::stdout();
            let writer = BufWriter::new(stdout.lock());
            jk::flatten::flatten(source.as_str()?, writer)?;
        }
        Command::Unflatten => {
            let source = source.load()?;
            jk::unflatten::unflatten(source.as_str()?)?;
        }
        Command::Fmt => {
            let source = source.load()?;

            let stdout = io::stdout();
            let writer = BufWriter::new(stdout.lock());
            jk::fmt::Formatter::new(Parser::new(source.as_str()?)).format_to(writer)?;
            // TODO: print a final newline if output is not being piped
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
