use std::io;

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
                        // Root level
                        writer.byte(b'{')?;
                    }
                    self.context.push(Context {
                        kind: CtxKind::Object,
                        wrote_first: false,
                    });
                    depth += 1;
                }

                Event::EndObject { member_count } => {
                    self.context.pop().unwrap();
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

                Event::EndArray { .. } => {
                    self.context.pop().unwrap();
                    depth -= 1;
                    if self.context.last().map_or(false, |c| c.wrote_first) || depth == 0 {
                        writer.newline()?;
                        writer.indentation(depth)?;
                        writer.byte(b']')?;
                    } else {
                        writer.byte(b']')?;
                    }
                }

                Event::Key(key) => {
                    let ctx = self.context.last_mut().unwrap();
                    if ctx.wrote_first {
                        writer.byte(b',')?;
                    } else {
                        ctx.wrote_first = true;
                    }
                    writer.newline()?;
                    writer.indentation(depth)?;
                    write!(writer, "\"{key}\": ")?;
                }

                Event::String(s) => {
                    let ctx = self.context.last_mut().unwrap();
                    if ctx.kind == CtxKind::Object {
                        // Print this inline with the key
                        write!(writer, "\"{s}\"")?;
                    } else {
                        // Array element
                        if ctx.wrote_first {
                            writer.byte(b',')?;
                        } else {
                            ctx.wrote_first = true;
                        }
                        writer.newline()?;
                        writer.indentation(depth)?;
                        write!(writer, "\"{s}\"")?;
                    }
                }

                other => {
                    let ctx = self.context.last_mut().unwrap();

                    if ctx.kind == CtxKind::Object {
                        writer.event(other)?;
                    } else {
                        if ctx.wrote_first {
                            writer.byte(b',')?;
                        } else {
                            ctx.wrote_first = true;
                        }

                        writer.newline()?;
                        writer.indentation(depth)?;
                        writer.event(other)?;
                    }
                }
            }
        }

        writer.newline()?;
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
    #[inline]
    fn newline(&mut self) -> io::Result<()> {
        self.write_all(b"\n")
    }

    #[inline]
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
