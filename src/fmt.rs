use std::io;

use anyhow::Context as AnyContext;
use jsax::{Event, Parser};

use crate::borrowed_value::ValueEvents;

// hyperfine --warmup 10 "jk-before-event-source fmt citm_catalog.json" "jk-before-spanned fmt citm_catalog.json" "jk-new-2 fmt citm_catalog.json" "jk fmt citm_catalog.json"

pub trait EventSource {
    fn next_event(&mut self) -> Result<Option<Event<'_>>, jsax::Error>;
}

impl EventSource for ValueEvents<'_> {
    fn next_event(&mut self) -> Result<Option<Event<'_>>, jsax::Error> {
        Ok(self.next())
    }
}

impl EventSource for Parser<'_> {
    #[inline(always)]
    fn next_event(&mut self) -> Result<Option<Event<'_>>, jsax::Error> {
        self.parse_next()
    }
}

struct Writer<W: io::Write> {
    inner: W,
    config: WriterConfig,
}

#[derive(Clone)]
pub struct WriterConfig {
    pub use_color: bool,
    pub indent_width: usize,
}

impl Default for WriterConfig {
    fn default() -> Self {
        Self {
            use_color: false,
            indent_width: 2,
        }
    }
}

impl<W: io::Write> Writer<W> {
    pub fn new(inner: W, config: WriterConfig) -> Self {
        Self { inner, config }
    }

    fn indentation(&mut self, depth: usize) -> io::Result<()> {
        // TODO: make configurable
        let indent = depth * 2;
        const INDENT: &[u8; 64] = &[b' '; 64];

        let full = indent / INDENT.len();
        let rem = indent % INDENT.len();

        for _ in 0..full {
            self.write(INDENT)?;
        }
        if rem != 0 {
            self.write(&INDENT[..rem])?;
        }
        Ok(())
    }

    #[inline(always)]
    fn newline(&mut self) -> io::Result<()> {
        self.inner.write_all(b"\n")
    }

    #[inline(always)]
    fn byte(&mut self, byte: u8) -> io::Result<()> {
        self.inner.write_all(&[byte])
    }

    const RESET: &'static [u8] = b"\x1b[0m";
    const BOLD_WHITE: &'static [u8] = b"\x1b[1;39m";
    const BOLD_BLUE: &'static [u8] = b"\x1b[1;34m";
    const GREEN: &'static [u8] = b"\x1b[0;32m";
    const NORMAL_WHITE: &'static [u8] = b"\x1b[0;39m";

    #[inline]
    fn write_colored(&mut self, color: &'static [u8], content: &[u8]) -> io::Result<()> {
        if self.config.use_color {
            self.inner.write_all(color)?;
            self.inner.write_all(content)?;
            self.inner.write_all(Self::RESET)?;
        } else {
            self.inner.write_all(content)?;
        }
        Ok(())
    }

    #[inline]
    pub fn structural_char(&mut self, byte: u8) -> io::Result<()> {
        self.write_colored(Self::BOLD_WHITE, &[byte])
    }

    #[inline]
    pub fn string_value(&mut self, s: &str) -> io::Result<()> {
        if self.config.use_color {
            self.inner.write_all(Self::GREEN)?;
            self.byte(b'"')?;
            self.inner.write_all(s.as_bytes())?;
            self.byte(b'"')?;
            self.inner.write_all(Self::RESET)?;
        } else {
            self.byte(b'"')?;
            self.inner.write_all(s.as_bytes())?;
            self.byte(b'"')?;
        }
        Ok(())
    }

    #[inline]
    pub fn key(&mut self, key: &str) -> io::Result<()> {
        if self.config.use_color {
            self.inner.write_all(Self::BOLD_BLUE)?;
            self.byte(b'"')?;
            self.inner.write_all(key.as_bytes())?;
            self.byte(b'"')?;
            self.inner.write_all(Self::RESET)?;
            self.structural_char(b':')?;
            self.byte(b' ')?;
        } else {
            self.byte(b'"')?;
            self.inner.write_all(key.as_bytes())?;
            self.write(b"\": ")?;
        }
        Ok(())
    }

    #[inline]
    pub fn number(&mut self, n: &str) -> io::Result<()> {
        self.write_colored(Self::NORMAL_WHITE, n.as_bytes())
    }

    #[inline]
    pub fn boolean(&mut self, b: bool) -> io::Result<()> {
        let s = if b {
            "true".as_bytes()
        } else {
            "false".as_bytes()
        };
        self.write_colored(Self::NORMAL_WHITE, s)
    }

    #[inline]
    pub fn null(&mut self) -> io::Result<()> {
        self.write_colored(Self::NORMAL_WHITE, b"null")
    }

    #[inline]
    pub fn event(&mut self, value: Event<'_>) -> io::Result<()> {
        match value {
            Event::String(s) => self.string_value(s),
            Event::Number(n) => self.number(n),
            Event::Boolean(b) => self.boolean(b),
            Event::Null => self.null(),
            _ => Ok(()),
        }
    }

    #[inline]
    fn write(&mut self, buf: &[u8]) -> io::Result<()> {
        self.inner.write_all(buf)
    }
}

pub struct Formatter<S: EventSource> {
    source: S,
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
impl<S: EventSource> Formatter<S> {
    pub fn new(source: S) -> Self {
        Self {
            source,
            context: Vec::with_capacity(4),
        }
    }

    pub fn format_to<W: io::Write>(
        &mut self,
        output: W,
        config: WriterConfig,
    ) -> anyhow::Result<()> {
        let mut writer = Writer::new(output, config);
        let mut depth = 0;

        while let Some(event) = self.source.next_event()? {
            match event {
                Event::StartObject => {
                    // Handle commas in arrays
                    if let Some(ctx) = self.context.last_mut() {
                        if ctx.kind == CtxKind::Array {
                            if ctx.wrote_first {
                                writer.structural_char(b',')?;
                            } else {
                                ctx.wrote_first = true;
                            }
                            writer.newline()?;
                            writer.indentation(depth)?;
                            writer.structural_char(b'{')?;
                        } else {
                            // After a key
                            writer.structural_char(b'{')?;
                        }
                    } else {
                        // Root level object started
                        writer.structural_char(b'{')?;
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
                    writer.structural_char(b'}')?;
                }

                Event::StartArray => {
                    if let Some(ctx) = self.context.last_mut() {
                        if ctx.kind == CtxKind::Array {
                            if ctx.wrote_first {
                                writer.structural_char(b',')?;
                            } else {
                                ctx.wrote_first = true;
                            }
                            writer.newline()?;
                            writer.indentation(depth)?;
                            writer.structural_char(b'[')?;
                        } else {
                            // After a key
                            writer.structural_char(b'[')?;
                        }
                    } else {
                        writer.structural_char(b'[')?;
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
                    writer.structural_char(b']')?;
                }

                Event::Key(key) => {
                    let ctx = self
                        .context
                        .last_mut()
                        .context("Found a key, no matching object")?;
                    if ctx.wrote_first {
                        writer.structural_char(b',')?;
                    } else {
                        ctx.wrote_first = true;
                    }
                    writer.newline()?;
                    writer.indentation(depth)?;
                    writer.key(key)?;
                }

                Event::String(s) => match self.context.last_mut() {
                    Some(ctx) if ctx.kind == CtxKind::Array => {
                        if ctx.wrote_first {
                            writer.structural_char(b',')?;
                        } else {
                            ctx.wrote_first = true;
                        }
                        writer.newline()?;
                        writer.indentation(depth)?;
                        writer.string_value(s)?;
                    }
                    _ => writer.string_value(s)?,
                },

                other => match self.context.last_mut() {
                    Some(ctx) if ctx.kind == CtxKind::Array => {
                        if ctx.wrote_first {
                            writer.structural_char(b',')?;
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

#[cfg(test)]
mod tests {
    use jsax::Parser;

    use crate::fmt::{Formatter, WriterConfig};

    fn format_to_string(input: &str) -> String {
        let mut bytes = Vec::new();

        Formatter::new(Parser::new(input))
            .format_to(&mut bytes, WriterConfig::default())
            .unwrap();

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

    #[test]
    fn string_escaping() {
        let test_cases = [
            (
                r#"{"key": "hello\"world"}"#,
                "{\n  \"key\": \"hello\\\"world\"\n}",
            ),
            (
                r#"{"path": "C:\\Users\\file.txt"}"#,
                "{\n  \"path\": \"C:\\\\Users\\\\file.txt\"\n}",
            ),
            (
                r#"{"text": "line1\nline2"}"#,
                "{\n  \"text\": \"line1\\nline2\"\n}",
            ),
            (
                r#"{"text": "tab\there"}"#,
                "{\n  \"text\": \"tab\\there\"\n}",
            ),
            (
                r#"{"msg": "She said \"Hi!\"\n"}"#,
                "{\n  \"msg\": \"She said \\\"Hi!\\\"\\n\"\n}",
            ),
            (r#"["quote\"here"]"#, "[\n  \"quote\\\"here\"\n]"),
        ];

        for (input, expected) in test_cases {
            let result = format_to_string(input);
            assert_eq!(
                result, expected,
                "Failed for input: {}\nExpected: {:?}\nGot: {:?}",
                input, expected, result
            );
        }
    }
}
