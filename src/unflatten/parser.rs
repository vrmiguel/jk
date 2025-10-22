use anyhow::anyhow;
use logos::{Lexer, Logos};

use crate::unflatten::{
    lexer::GronToken,
    types::{GronLine, GronValue, Identifier, Index},
};

pub struct Parser<'source> {
    lexer: Lexer<'source, GronToken<'source>>,
    identifiers: Vec<Identifier<'source>>,
}

impl<'source> Parser<'source> {
    pub fn new(source: &'source str) -> Self {
        Self {
            lexer: GronToken::lexer(source),
            identifiers: Vec::new(),
        }
    }

    #[inline]
    /// _Must_ lex a token of the same type as the one provided, or this function errors
    fn must_lex(&mut self, expected: GronToken) -> anyhow::Result<GronToken<'_>> {
        match self.lexer.next() {
            Some(Ok(token))
                if std::mem::discriminant(&token) == std::mem::discriminant(&expected) =>
            {
                Ok(token)
            }
            Some(Ok(token)) => Err(anyhow!("Expected {expected}, found {token} instead")),
            Some(Err(())) => Err(anyhow!("Expected {expected}, but the lexer failed instead")),
            None => Err(anyhow!("Expected {expected}, found EOF instead")),
        }
    }

    fn parse_identifier(&mut self) -> anyhow::Result<bool> {
        let mut current_base: Option<&'source str> = None;
        let mut expect_identifier = true;
        let mut found_any_token = false;

        while let Some(token) = self.lexer.next() {
            let token = token.unwrap();
            found_any_token = true;

            match token {
                GronToken::Identifier(name) if expect_identifier => {
                    // Start of a new path component
                    current_base = Some(name);
                    expect_identifier = false;
                }

                GronToken::BracketOpen if current_base.is_some() => {
                    // Expecting an index (number or string)
                    match self.lexer.next() {
                        Some(Ok(GronToken::Number(num))) => {
                            // Push identifier with numeric index
                            self.identifiers.push(Identifier {
                                base: current_base.take().unwrap(),
                                index: Some(Index::Numeric(num)),
                            });

                            self.must_lex(GronToken::BracketClose)?;
                        }
                        Some(Ok(GronToken::String(s))) => {
                            // Push identifier with string index
                            self.identifiers.push(Identifier {
                                base: current_base.take().unwrap(),
                                index: Some(Index::String(s)),
                            });

                            // Must now find a closing bracket
                            self.must_lex(GronToken::BracketClose)?;
                        }
                        _ => anyhow::bail!("Expected number or string index inside brackets"),
                    }
                }

                GronToken::Dot if current_base.is_some() => {
                    // Push the current identifier (no index) and expect next identifier
                    self.identifiers.push(Identifier {
                        base: current_base.take().unwrap(),
                        index: None,
                    });
                    expect_identifier = true;
                }

                GronToken::Equals => {
                    if let Some(base) = current_base.take() {
                        self.identifiers.push(Identifier { base, index: None });
                    }
                    return Ok(true);
                }

                _ => {
                    anyhow::bail!("Unexpected token while parsing identifier: {}.", token);
                }
            }
        }

        // Reached end of input
        if found_any_token {
            // We parsed some tokens but no '=', that's an error
            anyhow::bail!("Unexpected end of input, expected '='")
        } else {
            // If there were no tokens at all, that's just EOF
            Ok(false)
        }
    }

    fn parse_value(&mut self) -> anyhow::Result<GronValue<'source>> {
        match self.lexer.next() {
            Some(Ok(GronToken::EmptyObject)) => Ok(GronValue::Object),
            Some(Ok(GronToken::EmptyArray)) => Ok(GronValue::Array),
            Some(Ok(GronToken::Bool(b))) => Ok(GronValue::Boolean(b)),
            Some(Ok(GronToken::Null)) => Ok(GronValue::Null),
            Some(Ok(GronToken::Number(num))) => Ok(GronValue::Number(num)),
            Some(Ok(GronToken::String(s))) => Ok(GronValue::String(s)),
            Some(Ok(token)) => anyhow::bail!("Unexpected token while parsing value: {:?}", token),
            Some(Err(_)) => anyhow::bail!("Lexer error while parsing value"),
            None => anyhow::bail!("Unexpected end of input while parsing value"),
        }
    }

    fn parse_semicolon(&mut self) -> anyhow::Result<()> {
        self.must_lex(GronToken::Semicolon).map(|_| ())
    }

    pub fn parse_next_line(&mut self) -> anyhow::Result<Option<GronLine<'source>>> {
        self.identifiers.clear();

        if !self.parse_identifier()? {
            // Return None on EOF
            return Ok(None);
        }

        let value = self.parse_value()?;
        self.parse_semicolon()?;

        Ok(Some(GronLine {
            identifier: std::mem::take(&mut self.identifiers),
            value,
        }))
    }
}

#[cfg(test)]
mod tests {
    use crate::unflatten::{
        parser::Parser,
        types::{GronLine, GronValue, Identifier, Index},
    };

    #[test]
    fn parse_simple() {
        let gron = "json = {};\njson.address = {};\njson.address.street = \"123 Main St\";";
        let mut parser = Parser::new(gron);

        assert_eq!(
            parser.parse_next_line().unwrap(),
            Some(GronLine {
                identifier: vec![Identifier {
                    base: "json",
                    index: None
                }],
                value: GronValue::Object
            })
        );

        assert_eq!(
            parser.parse_next_line().unwrap(),
            Some(GronLine {
                identifier: vec![
                    Identifier {
                        base: "json",
                        index: None
                    },
                    Identifier {
                        base: "address",
                        index: None
                    }
                ],
                value: GronValue::Object
            })
        );

        assert_eq!(
            parser.parse_next_line().unwrap(),
            Some(GronLine {
                identifier: vec![
                    Identifier {
                        base: "json",
                        index: None
                    },
                    Identifier {
                        base: "address",
                        index: None
                    },
                    Identifier {
                        base: "street",
                        index: None
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
                identifier: vec![Identifier {
                    base: "json",
                    index: None
                }],
                value: GronValue::Object
            })
        );

        assert_eq!(
            parser.parse_next_line().unwrap(),
            Some(GronLine {
                identifier: vec![
                    Identifier {
                        base: "json",
                        index: None
                    },
                    Identifier {
                        base: "hobbies",
                        index: Some(Index::Numeric("0"))
                    }
                ],
                value: GronValue::String("reading")
            })
        );

        assert_eq!(
            parser.parse_next_line().unwrap(),
            Some(GronLine {
                identifier: vec![
                    Identifier {
                        base: "json",
                        index: None
                    },
                    Identifier {
                        base: "hobbies",
                        index: Some(Index::Numeric("1"))
                    }
                ],
                value: GronValue::String("cycling")
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
                identifier: vec![Identifier {
                    base: "json",
                    index: None
                }],
                value: GronValue::Object
            })
        );

        assert_eq!(
            parser.parse_next_line().unwrap(),
            Some(GronLine {
                identifier: vec![
                    Identifier {
                        base: "json",
                        index: None
                    },
                    Identifier {
                        base: "hobbies",
                        index: Some(Index::String("hobbies oh my hobbies"))
                    }
                ],
                value: GronValue::String("reading")
            })
        );

        assert_eq!(
            parser.parse_next_line().unwrap(),
            Some(GronLine {
                identifier: vec![
                    Identifier {
                        base: "json",
                        index: None
                    },
                    Identifier {
                        base: "hobbies",
                        index: Some(Index::Numeric("1"))
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

// #[cfg(test)]
// mod tests {
//     use crate::unflatten::{
//         parser::{parse_gron_line, parse_identifier_path, parse_value},
//         types::{GronLine, GronValue, Identifier, Index},
//     };

//     fn parse_collect_identifier_path(mut input: &str) -> Vec<Identifier<'_>> {
//         let it = std::iter::from_fn(move || match parse_identifier_path(input) {
//             Ok((i, o)) => {
//                 input = i;
//                 Some(o)
//             }
//             _ => None,
//         });

//         it.collect()
//     }

//     #[test]
//     fn test_parse_multiple_lines() {
//         let input = "json = {};\njson.address = {};\njson.address.street = \"123 Main St\";";

//         let (rest, line) = parse_gron_line(input).unwrap();
//         assert_eq!(
//             line,
//             GronLine {
//                 identifier: vec![Identifier {
//                     base: "json",
//                     index: None
//                 },],
//                 value: GronValue::Object
//             }
//         );
//         let (rest, line) = parse_gron_line(rest).unwrap();
//         assert_eq!(
//             line,
//             GronLine {
//                 identifier: vec![
//                     Identifier {
//                         base: "json",
//                         index: None
//                     },
//                     Identifier {
//                         base: "address",
//                         index: None
//                     }
//                 ],
//                 value: GronValue::Object
//             }
//         );
//         let (rest, line) = parse_gron_line(rest).unwrap();
//         assert_eq!(
//             line,
//             GronLine {
//                 identifier: vec![
//                     Identifier {
//                         base: "json",
//                         index: None
//                     },
//                     Identifier {
//                         base: "address",
//                         index: None
//                     },
//                     Identifier {
//                         base: "street",
//                         index: None
//                     }
//                 ],
//                 value: GronValue::String("123 Main St")
//             }
//         );
//         assert!(rest.is_empty());
//     }

//     #[test]
//     fn test_parse_gron_line() {
//         assert_eq!(
//             parse_gron_line("json = {};"),
//             Ok((
//                 "",
//                 GronLine {
//                     identifier: vec![Identifier {
//                         base: "json",
//                         index: None
//                     }],
//                     value: GronValue::Object
//                 }
//             ))
//         );

//         assert_eq!(
//             parse_gron_line("json = [];"),
//             Ok((
//                 "",
//                 GronLine {
//                     identifier: vec![Identifier {
//                         base: "json",
//                         index: None
//                     }],
//                     value: GronValue::Array
//                 }
//             ))
//         );

//         assert_eq!(
//             parse_gron_line("json.name = \"Alice\";"),
//             Ok((
//                 "",
//                 GronLine {
//                     identifier: vec![
//                         Identifier {
//                             base: "json",
//                             index: None
//                         },
//                         Identifier {
//                             base: "name",
//                             index: None
//                         },
//                     ],
//                     value: GronValue::String("Alice")
//                 }
//             ))
//         );

//         assert_eq!(
//             parse_gron_line("json.age = 30;"),
//             Ok((
//                 "",
//                 GronLine {
//                     identifier: vec![
//                         Identifier {
//                             base: "json",
//                             index: None
//                         },
//                         Identifier {
//                             base: "age",
//                             index: None
//                         },
//                     ],
//                     value: GronValue::Number("30")
//                 }
//             ))
//         );

//         assert_eq!(
//             parse_gron_line("json.active = true;"),
//             Ok((
//                 "",
//                 GronLine {
//                     identifier: vec![
//                         Identifier {
//                             base: "json",
//                             index: None
//                         },
//                         Identifier {
//                             base: "active",
//                             index: None
//                         },
//                     ],
//                     value: GronValue::Boolean(true)
//                 }
//             ))
//         );

//         assert_eq!(
//             parse_gron_line("json.deleted = false;"),
//             Ok((
//                 "",
//                 GronLine {
//                     identifier: vec![
//                         Identifier {
//                             base: "json",
//                             index: None
//                         },
//                         Identifier {
//                             base: "deleted",
//                             index: None
//                         },
//                     ],
//                     value: GronValue::Boolean(false)
//                 }
//             ))
//         );

//         assert_eq!(
//             parse_gron_line("json.data = null;"),
//             Ok((
//                 "",
//                 GronLine {
//                     identifier: vec![
//                         Identifier {
//                             base: "json",
//                             index: None
//                         },
//                         Identifier {
//                             base: "data",
//                             index: None
//                         },
//                     ],
//                     value: GronValue::Null
//                 }
//             ))
//         );

//         assert_eq!(
//             parse_gron_line("json.items[0] = \"first\";"),
//             Ok((
//                 "",
//                 GronLine {
//                     identifier: vec![
//                         Identifier {
//                             base: "json",
//                             index: None
//                         },
//                         Identifier {
//                             base: "items",
//                             index: Some(Index::Numeric("0"))
//                         },
//                     ],
//                     value: GronValue::String("first")
//                 }
//             ))
//         );

//         assert_eq!(
//             parse_gron_line("json[\"special-key\"] = 42;"),
//             Ok((
//                 "",
//                 GronLine {
//                     identifier: vec![Identifier {
//                         base: "json",
//                         index: Some(Index::String("special-key"))
//                     },],
//                     value: GronValue::Number("42")
//                 }
//             ))
//         );

//         assert_eq!(
//             parse_gron_line("json.value   =   123;"),
//             Ok((
//                 "",
//                 GronLine {
//                     identifier: vec![
//                         Identifier {
//                             base: "json",
//                             index: None
//                         },
//                         Identifier {
//                             base: "value",
//                             index: None
//                         },
//                     ],
//                     value: GronValue::Number("123")
//                 }
//             ))
//         );

//         assert_eq!(
//             parse_gron_line("json.price = 19.99;"),
//             Ok((
//                 "",
//                 GronLine {
//                     identifier: vec![
//                         Identifier {
//                             base: "json",
//                             index: None
//                         },
//                         Identifier {
//                             base: "price",
//                             index: None
//                         },
//                     ],
//                     value: GronValue::Number("19.99")
//                 }
//             ))
//         );

//         assert_eq!(
//             parse_gron_line("json.temperature = -5.5;"),
//             Ok((
//                 "",
//                 GronLine {
//                     identifier: vec![
//                         Identifier {
//                             base: "json",
//                             index: None
//                         },
//                         Identifier {
//                             base: "temperature",
//                             index: None
//                         },
//                     ],
//                     value: GronValue::Number("-5.5")
//                 }
//             ))
//         );

//         assert_eq!(
//             parse_gron_line("json.large = 1.5e10;"),
//             Ok((
//                 "",
//                 GronLine {
//                     identifier: vec![
//                         Identifier {
//                             base: "json",
//                             index: None
//                         },
//                         Identifier {
//                             base: "large",
//                             index: None
//                         },
//                     ],
//                     value: GronValue::Number("1.5e10")
//                 }
//             ))
//         );

//         assert_eq!(
//             parse_gron_line("json.path = \"C:\\\\Users\\\\file.txt\";"),
//             Ok((
//                 "",
//                 GronLine {
//                     identifier: vec![
//                         Identifier {
//                             base: "json",
//                             index: None
//                         },
//                         Identifier {
//                             base: "path",
//                             index: None
//                         },
//                     ],
//                     value: GronValue::String("C:\\\\Users\\\\file.txt")
//                 }
//             ))
//         );

//         assert_eq!(
//             parse_gron_line("json.empty = \"\";"),
//             Ok((
//                 "",
//                 GronLine {
//                     identifier: vec![
//                         Identifier {
//                             base: "json",
//                             index: None
//                         },
//                         Identifier {
//                             base: "empty",
//                             index: None
//                         },
//                     ],
//                     value: GronValue::String("")
//                 }
//             ))
//         );

//         assert_eq!(
//             parse_gron_line("json.address.street = \"123 Main St\";"),
//             Ok((
//                 "",
//                 GronLine {
//                     identifier: vec![
//                         Identifier {
//                             base: "json",
//                             index: None
//                         },
//                         Identifier {
//                             base: "address",
//                             index: None
//                         },
//                         Identifier {
//                             base: "street",
//                             index: None
//                         },
//                     ],
//                     value: GronValue::String("123 Main St")
//                 }
//             ))
//         );
//     }

//     #[test]
//     fn test_parse_identifier() {
//         assert_eq!(
//             parse_collect_identifier_path("json.address[1].test[\"hey\"]"),
//             vec![
//                 Identifier {
//                     base: "json",
//                     index: None
//                 },
//                 Identifier {
//                     base: "address",
//                     index: Some(Index::Numeric("1"))
//                 },
//                 Identifier {
//                     base: "test",
//                     index: Some(Index::String("hey"))
//                 },
//             ]
//         );

//         assert_eq!(
//             parse_collect_identifier_path("json"),
//             vec![Identifier {
//                 base: "json",
//                 index: None
//             },]
//         );

//         assert_eq!(
//             parse_collect_identifier_path("json[0]"),
//             vec![Identifier {
//                 base: "json",
//                 index: Some(Index::Numeric("0"))
//             },]
//         );

//         assert_eq!(
//             parse_collect_identifier_path("json.a.b.c.d"),
//             vec![
//                 Identifier {
//                     base: "json",
//                     index: None
//                 },
//                 Identifier {
//                     base: "a",
//                     index: None
//                 },
//                 Identifier {
//                     base: "b",
//                     index: None
//                 },
//                 Identifier {
//                     base: "c",
//                     index: None
//                 },
//                 Identifier {
//                     base: "d",
//                     index: None
//                 },
//             ]
//         );

//         assert_eq!(
//             parse_collect_identifier_path("json[\"key with spaces\"]"),
//             vec![Identifier {
//                 base: "json",
//                 index: Some(Index::String("key with spaces"))
//             },]
//         );

//         assert_eq!(
//             parse_collect_identifier_path("json[\"key-with-dashes\"]"),
//             vec![Identifier {
//                 base: "json",
//                 index: Some(Index::String("key-with-dashes"))
//             },]
//         );

//         assert_eq!(
//             parse_collect_identifier_path("json[\"key.with.dots\"]"),
//             vec![Identifier {
//                 base: "json",
//                 index: Some(Index::String("key.with.dots"))
//             },]
//         );

//         assert_eq!(
//             parse_collect_identifier_path("json[\"path\\\\to\\\\file\"]"),
//             vec![Identifier {
//                 base: "json",
//                 index: Some(Index::String("path\\\\to\\\\file"))
//             },]
//         );

//         assert_eq!(
//             parse_collect_identifier_path("json[\"key with \\\"quotes\\\"\"]"),
//             vec![Identifier {
//                 base: "json",
//                 index: Some(Index::String("key with \\\"quotes\\\""))
//             },]
//         );

//         assert_eq!(
//             parse_collect_identifier_path("json[\"\"]"),
//             vec![Identifier {
//                 base: "json",
//                 index: Some(Index::String(""))
//             },]
//         );

//         assert_eq!(
//             parse_collect_identifier_path("json.users[0].address[\"street name\"]"),
//             vec![
//                 Identifier {
//                     base: "json",
//                     index: None
//                 },
//                 Identifier {
//                     base: "users",
//                     index: Some(Index::Numeric("0"))
//                 },
//                 Identifier {
//                     base: "address",
//                     index: Some(Index::String("street name"))
//                 },
//             ]
//         );

//         assert_eq!(
//             parse_collect_identifier_path("json.items[999].data[12345]"),
//             vec![
//                 Identifier {
//                     base: "json",
//                     index: None
//                 },
//                 Identifier {
//                     base: "items",
//                     index: Some(Index::Numeric("999"))
//                 },
//                 Identifier {
//                     base: "data",
//                     index: Some(Index::Numeric("12345"))
//                 },
//             ]
//         );
//     }

//     #[test]
//     fn test_parse_value() {
//         assert_eq!(parse_value("null"), Ok(("", GronValue::Null)));
//         assert_eq!(parse_value("true"), Ok(("", GronValue::Boolean(true))));
//         assert_eq!(parse_value("false"), Ok(("", GronValue::Boolean(false))));
//         assert_eq!(parse_value("{}"), Ok(("", GronValue::Object)));
//         assert_eq!(parse_value("[]"), Ok(("", GronValue::Array)));
//         assert_eq!(parse_value("0"), Ok(("", GronValue::Number("0"))));
//         assert_eq!(parse_value("123"), Ok(("", GronValue::Number("123"))));
//         assert_eq!(parse_value("-456"), Ok(("", GronValue::Number("-456"))));
//         assert_eq!(
//             parse_value("123.456"),
//             Ok(("", GronValue::Number("123.456")))
//         );
//         assert_eq!(parse_value("-0.5"), Ok(("", GronValue::Number("-0.5"))));
//         assert_eq!(
//             parse_value("1.23e10"),
//             Ok(("", GronValue::Number("1.23e10")))
//         );
//         assert_eq!(parse_value("1.5E-5"), Ok(("", GronValue::Number("1.5E-5"))));
//         assert_eq!(
//             parse_value("-2.5e+3"),
//             Ok(("", GronValue::Number("-2.5e+3")))
//         );
//         assert_eq!(
//             parse_value("\"hello\""),
//             Ok(("", GronValue::String("hello")))
//         );
//         assert_eq!(parse_value("\"\""), Ok(("", GronValue::String(""))));
//         assert_eq!(
//             parse_value("\"hello world\""),
//             Ok(("", GronValue::String("hello world")))
//         );
//         assert_eq!(
//             parse_value("\"escaped \\\"quotes\\\"\""),
//             Ok(("", GronValue::String("escaped \\\"quotes\\\"")))
//         );
//         assert_eq!(
//             parse_value("\"path\\\\to\\\\file\""),
//             Ok(("", GronValue::String("path\\\\to\\\\file")))
//         );
//         assert_eq!(
//             parse_value("\"line1\\nline2\""),
//             Ok(("", GronValue::String("line1\\nline2")))
//         );
//         assert_eq!(parse_value("   true"), Ok(("", GronValue::Boolean(true))));
//         assert_eq!(parse_value("\t\t123"), Ok(("", GronValue::Number("123"))));
//         assert_eq!(parse_value("true;"), Ok((";", GronValue::Boolean(true))));
//         assert_eq!(
//             parse_value("123 // comment"),
//             Ok((" // comment", GronValue::Number("123")))
//         );
//     }
// }
