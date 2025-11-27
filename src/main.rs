use std::{
    io::{self, BufWriter, Write},
    process::ExitCode,
};

use jsax::Parser;
use lexopt::Arg;

use crate::{
    source::Source,
    utils::{is_stdin_readable, should_use_colors},
};

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
    Schema(Language),
    Help,
}

#[derive(Debug)]
enum Language {
    TypeScript,
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
            Arg::Value(value) if value == "schema" && command.is_none() => {
                // Next argument should be the format (typescript, rust, etc.)
                let format_arg = parser.value()?;
                let format_str = format_arg.to_str().ok_or_else(|| {
                    anyhow::anyhow!("Invalid format specified for schema command")
                })?;

                let format = match format_str {
                    "typescript" | "ts" => Language::TypeScript,
                    _ => {
                        return Err(anyhow::anyhow!(
                            "Unknown schema format '{}'. Supported: typescript, ts",
                            format_str
                        ));
                    }
                };

                command = Some(Command::Schema(format));
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
            let json = jk::borrowed_value::parse_value(source.as_str()?).unwrap();
            viewer::start_viewer(&json)?;
        }
        Command::Flatten => {
            let source = source.load()?;

            let stdout = io::stdout();
            let writer = BufWriter::new(stdout.lock());
            jk::flatten::flatten(source.as_str()?, writer)?;
        }
        Command::Unflatten => {
            let source = source.load()?;
            jk::unflatten::unflatten(source.as_str()?, should_use_colors())?;
        }
        Command::Fmt => {
            let source = source.load()?;

            let use_colors = should_use_colors();

            let stdout = io::stdout();
            let mut writer = BufWriter::new(stdout.lock());
            if use_colors {
                jk::fmt::Formatter::new_colored(Parser::new(source.as_str()?))
                    .format_to(&mut writer)?;
            } else {
                jk::fmt::Formatter::new_plain(Parser::new(source.as_str()?))
                    .format_to(&mut writer)?;
            }
            if use_colors {
                writer.write_all(b"\n")?;
            }
            writer.flush()?;
        }
        Command::Schema(format) => {
            let source = source.load()?;
            let schema = jk::schema::infer::infer_schema(source.as_str()?)?;

            let output = match format {
                Language::TypeScript => jk::schema::generator::typescript::generate(&schema),
            };

            println!("{}", output);
        }
        Command::Help => {
            unreachable!()
        }
    }

    Ok(())
}

fn help_message() {
    println!("Usage: jk [command] [path]");
    println!();
    println!("Commands:");
    println!("  [none]               Open JSON in interactive viewer (default)");
    println!("  flatten              Flatten JSON to dot-notation format");
    println!("  unflatten            Convert flattened format back to JSON");
    println!("  fmt                  Format/pretty-print JSON");
    println!("  schema <format>      Generate types from JSON schema");
    println!("                       Formats: typescript (ts)");
    println!("  help                 Show this help message");
    println!();
    println!("Examples:");
    println!("  jk data.json                    # Open in viewer");
    println!("  jk flatten data.json            # Flatten JSON");
    println!("  jk schema typescript data.json  # Generate TypeScript types");
    println!("  cat data.json | jk fmt          # Format JSON from stdin");
}
