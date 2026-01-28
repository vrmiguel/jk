use std::{
    io::{self, BufWriter, Write},
    process::ExitCode,
};

use jk::fold_tree::KeyedJsonElement;
use jsax::Parser;
use syntect::{
    easy::HighlightLines,
    highlighting::{Style, ThemeSet},
    parsing::SyntaxSet,
    util::{LinesWithEndings, as_24_bit_terminal_escaped},
};

use crate::{
    cli::{Command, CommandParseResult, Language},
    utils::should_use_colors,
};

mod cli;
mod source;
mod utils;
/// The interactive TUI JSON viewer
mod viewer;

fn main() -> ExitCode {
    if let Err(err) = run() {
        eprintln!("{err:?}");
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}

fn run() -> anyhow::Result<()> {
    let (command, source) = match cli::parse_command()? {
        CommandParseResult::Help => {
            help_message();
            return Ok(());
        }
        CommandParseResult::Command(command, source) => (command, source),
    };
    match command {
        Command::View => {
            let source = source.load()?;

            let bump = bumpalo::Bump::new();
            let tree = KeyedJsonElement::parse(source.as_str()?, &bump)?;

            viewer::start_viewer(&tree)?;
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
                Language::Rust => jk::schema::generator::rust::generate(&schema),
            };

            if should_use_colors() {
                print_highlighted(&output, format)?;
            } else {
                println!("{}", output);
            }
        }
        Command::Help => {
            unreachable!()
        }
    }

    Ok(())
}

// TODO(vrmiguel): this is probably terrible, gotta figure out how to use syntect better
fn print_highlighted(code: &str, language: Language) -> anyhow::Result<()> {
    let ps = SyntaxSet::load_defaults_newlines();
    let ts = ThemeSet::load_defaults();

    let syntax = match language {
        Language::TypeScript => ps
            .find_syntax_by_extension("js")
            .unwrap_or_else(|| ps.find_syntax_plain_text()),
        Language::Rust => ps
            .find_syntax_by_extension("rs")
            .unwrap_or_else(|| ps.find_syntax_plain_text()),
    };

    let mut h = HighlightLines::new(syntax, &ts.themes["base16-ocean.dark"]);

    for line in LinesWithEndings::from(code) {
        let ranges: Vec<(Style, &str)> = h.highlight_line(line, &ps)?;
        let escaped = as_24_bit_terminal_escaped(&ranges[..], false);
        print!("{}", escaped);
    }

    // Reset colors
    print!("\x1b[0m");
    println!();

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
    println!("                       Formats: typescript (ts), rust (rs)");
    println!("  help                 Show this help message");
    println!();
    println!("Examples:");
    println!("  jk data.json                    # Open in viewer");
    println!("  jk flatten data.json            # Flatten JSON");
    println!("  jk schema typescript data.json  # Generate TypeScript types");
    println!("  jk schema rust data.json        # Generate Rust types");
    println!("  cat data.json | jk fmt          # Format JSON from stdin");
}
