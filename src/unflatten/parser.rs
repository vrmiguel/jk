use smallvec::SmallVec;

use crate::unflatten::types::{GronLine, GronValue, Identifier, Index};

pub struct Parser<'a> {
    input: &'a str,
    bytes: &'a [u8],
    pos: usize,
    identifiers: Vec<Identifier<'a>>,
}

impl<'a> Parser<'a> {
    pub fn new(input: &'a str) -> Self {
        Self {
            input,
            bytes: input.as_bytes(),
            pos: 0,
            identifiers: Vec::with_capacity(4),
        }
    }

    #[inline(always)]
    fn peek(&self) -> Option<u8> {
        self.bytes.get(self.pos).copied()
    }

    #[inline(always)]
    fn advance(&mut self) -> Option<u8> {
        let b = self.peek()?;
        self.pos += 1;
        Some(b)
    }

    #[inline(always)]
    fn skip_whitespace(&mut self) {
        while self.pos < self.bytes.len() {
            match self.bytes[self.pos] {
                b' ' | b'\t' | b'\r' | b'\n' | 0x0C => self.pos += 1,
                _ => return,
            }
        }
    }

    #[inline(always)]
    fn starts_with(&self, prefix: &[u8]) -> bool {
        self.bytes[self.pos..].starts_with(prefix)
    }

    #[inline]
    fn expect(&mut self, expected: u8) -> anyhow::Result<()> {
        match self.advance() {
            Some(b) if b == expected => Ok(()),
            Some(b) => anyhow::bail!("Expected '{}', found '{}'", expected as char, b as char),
            None => anyhow::bail!("Expected '{}', found EOF", expected as char),
        }
    }

    #[inline]
    fn parse_identifier(&mut self) -> anyhow::Result<&'a str> {
        let start = self.pos;
        match self.peek() {
            Some(b) if b.is_ascii_alphabetic() || b == b'_' => self.pos += 1,
            Some(b) => anyhow::bail!("Expected identifier, found '{}'", b as char),
            None => anyhow::bail!("Expected identifier, found EOF"),
        }
        while let Some(b) = self.peek() {
            if b.is_ascii_alphanumeric() || b == b'_' {
                self.pos += 1;
            } else {
                break;
            }
        }
        Ok(&self.input[start..self.pos])
    }

    #[inline]
    fn parse_string(&mut self) -> anyhow::Result<&'a str> {
        let start = self.pos;
        loop {
            match self.advance() {
                Some(b'"') => return Ok(&self.input[start..self.pos - 1]),
                Some(b'\\') => {
                    // Skip the escaped character
                    let _ = self.advance();
                }
                Some(_) => {}
                None => anyhow::bail!("Unterminated string"),
            }
        }
    }

    #[inline]
    fn parse_number(&mut self) -> &'a str {
        let start = self.pos;
        // Takes everything that can be part of a JSON number
        while let Some(b) = self.peek() {
            match b {
                b'0'..=b'9' | b'.' | b'-' | b'+' | b'e' | b'E' => self.pos += 1,
                _ => break,
            }
        }
        &self.input[start..self.pos]
    }

    #[inline]
    fn parse_index(&mut self) -> anyhow::Result<usize> {
        let mut value: usize = 0;
        let mut has_digits = false;
        while let Some(b) = self.peek() {
            if b.is_ascii_digit() {
                value = value * 10 + (b - b'0') as usize;
                self.pos += 1;
                has_digits = true;
            } else {
                break;
            }
        }
        anyhow::ensure!(has_digits, "Expected numeric index");
        Ok(value)
    }

    /// Parse the path portion of a gron line.
    /// e.g. `json.reports[0].name` would be parsed as:
    ///
    /// - `Identifier { base: "json", indices: [] }`
    /// - `Identifier { base: "reports", indices: [Numeric(0)] }`
    /// - `Identifier { base: "name", indices: [] }`
    fn parse_path(&mut self) -> anyhow::Result<()> {
        let mut base = self.parse_identifier()?;
        let mut indices: SmallVec<[Index<'a>; 2]> = SmallVec::new();

        loop {
            match self.peek() {
                Some(b'.') => {
                    self.identifiers.push(Identifier {
                        base,
                        indices: std::mem::replace(&mut indices, SmallVec::new()),
                    });
                    self.pos += 1;
                    base = self.parse_identifier()?;
                }
                Some(b'[') => {
                    self.pos += 1;
                    match self.peek() {
                        Some(b'"') => {
                            self.pos += 1;
                            let s = self.parse_string()?;
                            indices.push(Index::String(s));
                        }
                        Some(b) if b.is_ascii_digit() => {
                            let idx = self.parse_index()?;
                            indices.push(Index::Numeric(idx));
                        }
                        Some(b) => anyhow::bail!(
                            "Expected number or string inside brackets, found '{}'",
                            b as char
                        ),
                        None => anyhow::bail!("Unexpected EOF inside brackets"),
                    }
                    self.expect(b']')?;
                }
                _ => break,
            }
        }

        self.identifiers.push(Identifier { base, indices });
        Ok(())
    }

    /// Gets the value after `=`, in a line like 'json[0].timestamp = 1572299227'
    fn parse_value(&mut self) -> anyhow::Result<GronValue<'a>> {
        self.skip_whitespace();

        match self.peek() {
            Some(b'{') => {
                self.pos += 1;
                self.expect(b'}')?;
                Ok(GronValue::Object)
            }
            Some(b'[') => {
                self.pos += 1;
                self.expect(b']')?;
                Ok(GronValue::Array)
            }
            Some(b'"') => {
                self.pos += 1;
                Ok(GronValue::String(self.parse_string()?))
            }
            Some(b't') if self.starts_with(b"true") => {
                self.pos += 4;
                Ok(GronValue::Boolean(true))
            }
            Some(b'f') if self.starts_with(b"false") => {
                self.pos += 5;
                Ok(GronValue::Boolean(false))
            }
            Some(b'n') if self.starts_with(b"null") => {
                self.pos += 4;
                Ok(GronValue::Null)
            }
            Some(b) if b == b'-' || b.is_ascii_digit() => {
                Ok(GronValue::Number(self.parse_number()))
            }
            Some(b) => anyhow::bail!("Unexpected '{}' while parsing value", b as char),
            None => anyhow::bail!("Unexpected EOF while parsing value"),
        }
    }

    /// Parse the next gron line, or return `None` at EOF.
    pub fn parse_next_line(&mut self) -> anyhow::Result<Option<GronLine<'a, '_>>> {
        self.identifiers.clear();
        self.skip_whitespace();

        if self.pos >= self.bytes.len() {
            return Ok(None);
        }

        self.parse_path()?;

        self.skip_whitespace();
        self.expect(b'=')?;

        let value = self.parse_value()?;

        self.skip_whitespace();
        self.expect(b';')?;

        Ok(Some(GronLine {
            identifier: &self.identifiers,
            value,
        }))
    }
}

#[cfg(test)]
mod tests {
    use smallvec::smallvec;

    use super::Parser;
    use crate::unflatten::types::{GronLine, GronValue, Identifier, Index};

    #[test]
    fn parse_simple() {
        let gron = "json = {};\njson.address = {};\njson.address.street = \"123 Main St\";";
        let mut parser = Parser::new(gron);

        assert_eq!(
            parser.parse_next_line().unwrap(),
            Some(GronLine {
                identifier: &[Identifier {
                    base: "json",
                    indices: smallvec![]
                }],
                value: GronValue::Object
            })
        );

        assert_eq!(
            parser.parse_next_line().unwrap(),
            Some(GronLine {
                identifier: &[
                    Identifier {
                        base: "json",
                        indices: smallvec![]
                    },
                    Identifier {
                        base: "address",
                        indices: smallvec![]
                    }
                ],
                value: GronValue::Object
            })
        );

        assert_eq!(
            parser.parse_next_line().unwrap(),
            Some(GronLine {
                identifier: &[
                    Identifier {
                        base: "json",
                        indices: smallvec![]
                    },
                    Identifier {
                        base: "address",
                        indices: smallvec![]
                    },
                    Identifier {
                        base: "street",
                        indices: smallvec![]
                    }
                ],
                value: GronValue::String("123 Main St")
            })
        );

        assert_eq!(parser.parse_next_line().unwrap(), None);
    }

    #[test]
    fn parse_numeric_index() {
        let gron = "json = {};\njson.hobbies[0] = \"reading\";json.hobbies[1] = \"cycling\";";
        let mut parser = Parser::new(gron);

        assert_eq!(
            parser.parse_next_line().unwrap(),
            Some(GronLine {
                identifier: &[Identifier {
                    base: "json",
                    indices: smallvec![]
                }],
                value: GronValue::Object
            })
        );

        assert_eq!(
            parser.parse_next_line().unwrap(),
            Some(GronLine {
                identifier: &[
                    Identifier {
                        base: "json",
                        indices: smallvec![]
                    },
                    Identifier {
                        base: "hobbies",
                        indices: smallvec![Index::Numeric(0)]
                    }
                ],
                value: GronValue::String("reading")
            })
        );

        assert_eq!(
            parser.parse_next_line().unwrap(),
            Some(GronLine {
                identifier: &[
                    Identifier {
                        base: "json",
                        indices: smallvec![]
                    },
                    Identifier {
                        base: "hobbies",
                        indices: smallvec![Index::Numeric(1)]
                    }
                ],
                value: GronValue::String("cycling")
            })
        );
    }

    #[test]
    fn parse_nested_numeric_index() {
        let gron = "json = {};\njson.hobbies = [];\njson.hobbies[0] = [];\njson.hobbies[0][0] = \"reading\";\njson.hobbies[0][1] = \"cycling\";\njson.hobbies[1] = [];\njson.hobbies[1][0] = \"swimming\";\njson.hobbies[1][1] = \"dancing\";";
        let mut parser = Parser::new(gron);

        assert_eq!(
            parser.parse_next_line().unwrap(),
            Some(GronLine {
                identifier: &[Identifier {
                    base: "json",
                    indices: smallvec![]
                }],
                value: GronValue::Object
            })
        );

        assert_eq!(
            parser.parse_next_line().unwrap(),
            Some(GronLine {
                identifier: &[
                    Identifier {
                        base: "json",
                        indices: smallvec![]
                    },
                    Identifier {
                        base: "hobbies",
                        indices: smallvec![]
                    }
                ],
                value: GronValue::Array
            })
        );

        assert_eq!(
            parser.parse_next_line().unwrap(),
            Some(GronLine {
                identifier: &[
                    Identifier {
                        base: "json",
                        indices: smallvec![]
                    },
                    Identifier {
                        base: "hobbies",
                        indices: smallvec![Index::Numeric(0)]
                    }
                ],
                value: GronValue::Array
            })
        );

        assert_eq!(
            parser.parse_next_line().unwrap(),
            Some(GronLine {
                identifier: &[
                    Identifier {
                        base: "json",
                        indices: smallvec![]
                    },
                    Identifier {
                        base: "hobbies",
                        indices: smallvec![Index::Numeric(0), Index::Numeric(0)]
                    }
                ],
                value: GronValue::String("reading")
            })
        );

        assert_eq!(
            parser.parse_next_line().unwrap(),
            Some(GronLine {
                identifier: &[
                    Identifier {
                        base: "json",
                        indices: smallvec![]
                    },
                    Identifier {
                        base: "hobbies",
                        indices: smallvec![Index::Numeric(0), Index::Numeric(1)]
                    }
                ],
                value: GronValue::String("cycling")
            })
        );

        assert_eq!(
            parser.parse_next_line().unwrap(),
            Some(GronLine {
                identifier: &[
                    Identifier {
                        base: "json",
                        indices: smallvec![]
                    },
                    Identifier {
                        base: "hobbies",
                        indices: smallvec![Index::Numeric(1)]
                    }
                ],
                value: GronValue::Array
            })
        );

        assert_eq!(
            parser.parse_next_line().unwrap(),
            Some(GronLine {
                identifier: &[
                    Identifier {
                        base: "json",
                        indices: smallvec![]
                    },
                    Identifier {
                        base: "hobbies",
                        indices: smallvec![Index::Numeric(1), Index::Numeric(0)]
                    }
                ],
                value: GronValue::String("swimming")
            })
        );

        assert_eq!(
            parser.parse_next_line().unwrap(),
            Some(GronLine {
                identifier: &[
                    Identifier {
                        base: "json",
                        indices: smallvec![]
                    },
                    Identifier {
                        base: "hobbies",
                        indices: smallvec![Index::Numeric(1), Index::Numeric(1)]
                    }
                ],
                value: GronValue::String("dancing")
            })
        );
    }

    #[test]
    fn parse_string_index() {
        let gron = "json = {};\njson.hobbies[\"hobbies oh my hobbies\"] = \"reading\";json.hobbies[1] = \"cycling\";";
        let mut parser = Parser::new(gron);

        assert_eq!(
            parser.parse_next_line().unwrap(),
            Some(GronLine {
                identifier: &[Identifier {
                    base: "json",
                    indices: smallvec![]
                }],
                value: GronValue::Object
            })
        );

        assert_eq!(
            parser.parse_next_line().unwrap(),
            Some(GronLine {
                identifier: &[
                    Identifier {
                        base: "json",
                        indices: smallvec![]
                    },
                    Identifier {
                        base: "hobbies",
                        indices: smallvec![Index::String("hobbies oh my hobbies")]
                    }
                ],
                value: GronValue::String("reading")
            })
        );

        assert_eq!(
            parser.parse_next_line().unwrap(),
            Some(GronLine {
                identifier: &[
                    Identifier {
                        base: "json",
                        indices: smallvec![]
                    },
                    Identifier {
                        base: "hobbies",
                        indices: smallvec![Index::Numeric(1)]
                    }
                ],
                value: GronValue::String("cycling")
            })
        );
    }

    #[test]
    fn parses_til_eof() {
        let gron = "json = {};\njson.address = {};\njson.address.street = \"123 Main St\";json.address.zip = \"10001\";json.age = 30;json.city = \"New York\";json.hobbies = [];json.hobbies[0] = \"reading\";json.hobbies[1] = \"cycling\";json.name = \"John\";";
        let mut parser = Parser::new(gron);

        for i in 0..10 {
            let result = parser.parse_next_line();
            assert!(result.is_ok(), "Line {} failed: {:?}", i, result);
            assert!(result.unwrap().is_some(), "Line {} was None", i);
        }

        // Should return None at EOF
        assert_eq!(parser.parse_next_line().unwrap(), None);
        assert_eq!(parser.parse_next_line().unwrap(), None);
    }
}
