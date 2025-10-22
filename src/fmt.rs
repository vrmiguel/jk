use std::io;

use anyhow::Context as AnyContext;
use jsax::{Event, Parser};
pub struct Formatter<'a> {
    parser: Parser<'a>,
    context: Vec<Context>,
}

struct Context {
    kind: CtxKind,
    // Has this container written any child yet?
    wrote_first: bool,
}

#[derive(Copy, Clone, Debug, PartialEq)]
enum CtxKind {
    Object,
    Array,
}

/// TODO: update to be fairly generic:
/// - Configurable indentation width
/// - Indent with tab or spaces
/// - Don't pretty print (flag to print minified instead)
impl<'a> Formatter<'a> {
    pub fn new(input: &'a str) -> Self {
        Self {
            parser: Parser::new(input),
            context: Vec::with_capacity(4),
        }
    }

    pub fn format_to<W: io::Write>(&mut self, mut writer: W) -> anyhow::Result<()> {
        let mut depth = 0;

        while let Some(event) = self.parser.parse_next()? {
            match event {
                Event::StartObject => {
                    // Handle commas in arrays
                    if let Some(ctx) = self.context.last_mut() {
                        if ctx.kind == CtxKind::Array {
                            if ctx.wrote_first {
                                writer.byte(b',')?;
                            } else {
                                ctx.wrote_first = true;
                            }
                            writer.newline()?;
                            writer.indentation(depth)?;
                            writer.byte(b'{')?;
                        } else {
                            // After a key
                            writer.byte(b'{')?;
                        }
                    } else {
                        // Root level object started
                        writer.byte(b'{')?;
                    }
                    self.context.push(Context {
                        kind: CtxKind::Object,
                        wrote_first: false,
                    });
                    depth += 1;
                }

                Event::EndObject { member_count } => {
                    self.context
                        .pop()
                        .context("Found a closing bracket, but no matching opening bracket")?;
                    depth -= 1;
                    if member_count > 0 {
                        writer.newline()?;
                        writer.indentation(depth)?;
                    }
                    writer.byte(b'}')?;
                }

                Event::StartArray => {
                    if let Some(ctx) = self.context.last_mut() {
                        if ctx.kind == CtxKind::Array {
                            if ctx.wrote_first {
                                writer.byte(b',')?;
                            } else {
                                ctx.wrote_first = true;
                            }
                            writer.newline()?;
                            writer.indentation(depth)?;
                            writer.byte(b'[')?;
                        } else {
                            // After a key
                            writer.byte(b'[')?;
                        }
                    } else {
                        writer.byte(b'[')?;
                    }
                    self.context.push(Context {
                        kind: CtxKind::Array,
                        wrote_first: false,
                    });
                    depth += 1;
                }

                Event::EndArray { len } => {
                    self.context.pop().unwrap();
                    depth -= 1;
                    if len > 0 {
                        writer.newline()?;
                        writer.indentation(depth)?;
                    }
                    writer.byte(b']')?;
                }

                Event::Key(key) => {
                    let ctx = self
                        .context
                        .last_mut()
                        .context("Found a key, no matching object")?;
                    if ctx.wrote_first {
                        writer.byte(b',')?;
                    } else {
                        ctx.wrote_first = true;
                    }
                    writer.newline()?;
                    writer.indentation(depth)?;
                    write!(writer, "\"{key}\": ")?;
                }

                Event::String(s) => match self.context.last_mut() {
                    Some(ctx) if ctx.kind == CtxKind::Array => {
                        if ctx.wrote_first {
                            writer.byte(b',')?;
                        } else {
                            ctx.wrote_first = true;
                        }
                        writer.newline()?;
                        writer.indentation(depth)?;
                        writer.string(s)?;
                    }
                    _ => writer.string(s)?,
                },

                other => match self.context.last_mut() {
                    Some(ctx) if ctx.kind == CtxKind::Array => {
                        if ctx.wrote_first {
                            writer.byte(b',')?;
                        } else {
                            ctx.wrote_first = true;
                        }
                        writer.newline()?;
                        writer.indentation(depth)?;
                        writer.event(other)?;
                    }
                    _ => writer.event(other)?,
                },
            }
        }
        Ok(())
    }
}

trait IoWriteExt {
    fn newline(&mut self) -> io::Result<()>;
    fn byte(&mut self, c: u8) -> io::Result<()>;
    fn string(&mut self, s: &str) -> io::Result<()>;
    fn indentation(&mut self, indent: usize) -> io::Result<()>;
    fn event(&mut self, value: Event<'_>) -> io::Result<()>;
}

impl<W: io::Write> IoWriteExt for W {
    #[inline(always)]
    fn newline(&mut self) -> io::Result<()> {
        self.write_all(b"\n")
    }

    #[inline(always)]
    fn byte(&mut self, byte: u8) -> io::Result<()> {
        self.write_all(&[byte])
    }

    #[inline]
    fn string(&mut self, s: &str) -> io::Result<()> {
        self.byte(b'"')?;
        self.write_all(s.as_bytes())?;
        self.byte(b'"')?;
        Ok(())
    }

    #[inline]
    fn indentation(&mut self, depth: usize) -> io::Result<()> {
        // TODO: make configurable
        let indent = depth * 2;
        const INDENT: &[u8; 64] = &[b' '; 64];

        let full = indent / INDENT.len();
        let rem = indent % INDENT.len();

        for _ in 0..full {
            self.write_all(INDENT)?;
        }
        if rem != 0 {
            self.write_all(&INDENT[..rem])?;
        }
        Ok(())
    }

    #[inline]
    fn event(&mut self, value: Event<'_>) -> io::Result<()> {
        match value {
            Event::String(value) => self.string(value)?,
            other => self.write_all(other.as_str().as_bytes())?,
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::fmt::Formatter;

    fn format_to_string(input: &str) -> String {
        let mut bytes = Vec::new();

        Formatter::new(input).format_to(&mut bytes).unwrap();

        String::from_utf8(bytes).unwrap()
    }

    fn format_to_string_serde(input: &str) -> String {
        use serde_json::ser::PrettyFormatter;
        use serde_json::{self, Deserializer};

        let mut de = Deserializer::from_str(input);
        let mut out = Vec::with_capacity(input.len() + input.len() / 2);
        let formatter = PrettyFormatter::with_indent(b"  ");
        let mut ser = serde_json::Serializer::with_formatter(&mut out, formatter);

        serde_transcode::transcode(&mut de, &mut ser).unwrap();
        String::from_utf8(out).expect("serde_json only emits UTF-8")
    }

    #[test]
    fn sci_not() {
        assert_eq!(
            format_to_string(r#"{"sci": 1e10}"#),
            "{\n  \"sci\": 1e10\n}"
        );
    }

    /// Our formatter should mostly agree with serde_json::to_string_pretty. This is a test that
    /// checks a few inputs against both our formatter and serde_json, ensuring they're the same    
    #[test]
    fn pretty_formatting() {
        let test_cases = [
            // Non-object/array roots
            r#""""#,
            "true",
            "false",
            "null",
            "33",

            // Handles empty
            "[]",
            "{}",
            "[[]]",
            "[{}]",
            r#"{"a": {}}"#,
            r#"{"a": []}"#,
            r#"{"a": {}, "b": []}"#,
            // Objects
            r#"{"a": 2}"#,
            r#"{"a": "b"}"#,
            r#"{"a": "b", "c": "d"}"#,
            r#"{"a": [1,2,3], "c": "d"}"#,
            r#"{"a": [1,2,3], "c": "d", "e3ea":   null}"#,
            r#"{"nested": {"a": [1,2,3], "b": null}}"#,
            r#"{"users": [{"name": "Alice", "age": 30}, {"name": "Bob", "age": 25}]}"#,
            // Arrays
            r#"[null]"#,
            r#"[null, {}, [null]]"#,
            "[1, true, null, {}]",
            r#"[1, "string", false, null, []]"#,
            r#"[{"items": [1, 2, 3]}]"#,
            r#"[{"a": 1}, {"b": 2}, {"c": 3}]"#,

            // Empty strings
            r#"{"": "empty key"}"#,
            r#"{"key": ""}"#,
            // Numbers
            // Note: we don't have scientific notation here because `serde_json` parses them into u64/f64
            r#"{"int": 42, "float": 3.14, "neg": -10}"#,
        ];

        for test_case in test_cases {
            let ours = format_to_string(test_case);
            let theirs = format_to_string_serde(test_case);

            if ours != theirs {
                panic!(
                    "There was a different result in test case '{test_case}'\nExpected: '{theirs}'\nReceived: '{ours}'"
                )
            }
        }
    }
}
