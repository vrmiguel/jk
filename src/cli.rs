use lexopt::Arg;

use crate::{source::Source, utils::is_stdin_readable};

#[derive(Debug)]
pub enum Command {
    View,
    Flatten,
    Unflatten,
    Fmt,
    Schema(Language),
    // TODO: this could be removed?
    Help,
}

#[derive(Debug)]
pub enum Language {
    TypeScript,
}

pub enum CommandParseResult {
    Help,
    Command(Command, Source),
}

pub fn parse_command() -> anyhow::Result<CommandParseResult> {
    let piped_input = is_stdin_readable();
    let parser = lexopt::Parser::from_env();

    parse_command_pure(piped_input, parser)
}

fn parse_command_pure(
    piped_input: bool,
    mut parser: lexopt::Parser,
) -> anyhow::Result<CommandParseResult> {
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
            return Ok(CommandParseResult::Help);
        }
        (None, None) if !piped_input => {
            return Ok(CommandParseResult::Help);
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

    Ok(CommandParseResult::Command(command, source))
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    #[test]
    fn test_flatten_file() {
        let parser = lexopt::Parser::from_args(&["flatten", "data.json"]);
        let result = parse_command_pure(false, parser).unwrap();

        match result {
            CommandParseResult::Command(Command::Flatten, Source::File(path)) => {
                assert_eq!(path, PathBuf::from("data.json"));
            }
            _ => panic!("Expected Flatten command with file source"),
        }
    }

    #[test]
    fn test_flatten_stdin() {
        let parser = lexopt::Parser::from_args(&["flatten"]);
        let result = parse_command_pure(true, parser).unwrap();

        match result {
            CommandParseResult::Command(Command::Flatten, Source::Stdin) => {}
            _ => panic!("Expected Flatten command with stdin source"),
        }
    }

    #[test]
    fn test_unflatten_file() {
        let parser = lexopt::Parser::from_args(&["unflatten", "data.gron"]);
        let result = parse_command_pure(false, parser).unwrap();

        match result {
            CommandParseResult::Command(Command::Unflatten, Source::File(path)) => {
                assert_eq!(path, PathBuf::from("data.gron"));
            }
            _ => panic!("Expected Unflatten command with file source"),
        }
    }

    #[test]
    fn test_unflatten_stdin() {
        let parser = lexopt::Parser::from_args(&["unflatten"]);
        let result = parse_command_pure(true, parser).unwrap();

        match result {
            CommandParseResult::Command(Command::Unflatten, Source::Stdin) => {}
            _ => panic!("Expected Unflatten command with stdin source"),
        }
    }

    #[test]
    fn test_fmt_file() {
        let parser = lexopt::Parser::from_args(&["fmt", "data.json"]);
        let result = parse_command_pure(false, parser).unwrap();

        match result {
            CommandParseResult::Command(Command::Fmt, Source::File(path)) => {
                assert_eq!(path, PathBuf::from("data.json"));
            }
            _ => panic!("Expected Fmt command with file source"),
        }
    }

    #[test]
    fn test_fmt_stdin() {
        let parser = lexopt::Parser::from_args(&["fmt"]);
        let result = parse_command_pure(true, parser).unwrap();

        match result {
            CommandParseResult::Command(Command::Fmt, Source::Stdin) => {}
            _ => panic!("Expected Fmt command with stdin source"),
        }
    }

    #[test]
    fn test_schema_file_ts() {
        let parser = lexopt::Parser::from_args(&["schema", "ts", "data.json"]);
        let result = parse_command_pure(false, parser).unwrap();

        match result {
            CommandParseResult::Command(
                Command::Schema(Language::TypeScript),
                Source::File(path),
            ) => {
                assert_eq!(path, PathBuf::from("data.json"));
            }
            _ => panic!("Expected Schema TypeScript command with file source"),
        }
    }

    #[test]
    fn test_schema_file_typescript() {
        let parser = lexopt::Parser::from_args(&["schema", "typescript", "data.json"]);
        let result = parse_command_pure(false, parser).unwrap();

        match result {
            CommandParseResult::Command(
                Command::Schema(Language::TypeScript),
                Source::File(path),
            ) => {
                assert_eq!(path, PathBuf::from("data.json"));
            }
            _ => panic!("Expected Schema TypeScript command with file source"),
        }
    }

    #[test]
    fn test_schema_stdin_ts() {
        let parser = lexopt::Parser::from_args(&["schema", "ts"]);
        let result = parse_command_pure(true, parser).unwrap();

        match result {
            CommandParseResult::Command(Command::Schema(Language::TypeScript), Source::Stdin) => {}
            _ => panic!("Expected Schema TypeScript command with stdin source"),
        }
    }

    #[test]
    fn test_view_file() {
        let parser = lexopt::Parser::from_args(&["data.json"]);
        let result = parse_command_pure(false, parser).unwrap();

        match result {
            CommandParseResult::Command(Command::View, Source::File(path)) => {
                assert_eq!(path, PathBuf::from("data.json"));
            }
            _ => panic!("Expected View command with file source"),
        }
    }

    #[test]
    fn test_view_stdin() {
        let parser = lexopt::Parser::from_args(&[] as &[&str]);
        let result = parse_command_pure(true, parser).unwrap();

        match result {
            CommandParseResult::Command(Command::View, Source::Stdin) => {}
            _ => panic!("Expected View command with stdin source"),
        }
    }

    #[test]
    fn test_help_command() {
        let parser = lexopt::Parser::from_args(&["help"]);
        let result = parse_command_pure(false, parser).unwrap();

        match result {
            CommandParseResult::Help => {}
            _ => panic!("Expected Help result"),
        }
    }

    #[test]
    fn test_no_args_no_stdin() {
        let parser = lexopt::Parser::from_args(&[] as &[&str]);
        let result = parse_command_pure(false, parser).unwrap();

        match result {
            CommandParseResult::Help => {}
            _ => panic!("Expected Help result when no args and no stdin"),
        }
    }

    #[test]
    fn test_schema_missing_format() {
        let parser = lexopt::Parser::from_args(&["schema"]);
        let result = parse_command_pure(true, parser);

        assert!(result.is_err());
    }

    #[test]
    fn test_schema_unknown_format() {
        let parser = lexopt::Parser::from_args(&["schema", "rust", "data.json"]);
        let result = parse_command_pure(false, parser);

        assert!(result.is_err());
    }

    #[test]
    fn test_flatten_missing_file() {
        let parser = lexopt::Parser::from_args(&["flatten"]);
        let result = parse_command_pure(false, parser);

        assert!(result.is_err());
    }
}
