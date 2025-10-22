use std::ops::Range;

use logos::{Lexer, Logos};

mod lexer;

use lexer::Token;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("I/O error: {0}")]
    Io(std::io::Error),
    #[error("Unexpected token: {0}")]
    // TODO: add expected?
    Unexpected(String),
    #[error("UTF-8 error: {0}")]
    Utf8(#[from] std::str::Utf8Error),
    #[error("Trailing comma")]
    TrailingComma,
    #[error("Expected comma or ]")]
    ExpectedCommaOrClose,
}

#[derive(Debug)]
pub enum Context {
    /// Currently parsing an object
    Object {
        member_count: usize,
        /// The token we must parse next
        expected_next_token: ObjNextToken,
    },
    /// Currently parsing an array
    Array {
        len: usize,
        /// The token we must parse next
        expected_next_token: ArrayNextToken,
    },
}

/// When parsing an object, this is the next token we're expecting.
#[derive(Debug, PartialEq)]
pub enum ObjNextToken {
    /// After `{`, can see empty object or key
    KeyOrClose,
    /// After key, must see the `:` token
    Colon,
    /// After :, must see a value
    Value,
    /// After value, can see `,` or `}`
    CommaOrClose,
    /// After `,`, must see a key (no trailing commas)
    Key,
}

#[derive(Debug, PartialEq)]
pub enum ArrayNextToken {
    /// After `[`, can see value or `]`
    ValueOrClose,
    /// After value, can see `,` or `]`
    CommaOrClose,
    /// After comma, MUST get a value (no trailing commas)
    Value,
}

#[derive(Debug, PartialEq)]
pub enum Event<'a> {
    StartObject,
    EndObject { member_count: usize },
    StartArray,
    EndArray { len: usize },
    Key(&'a str),
    String(&'a str),
    Number(&'a str),
    Boolean(bool),
    Null,
}

#[cold]
fn unexpected(token: Token<'_>) -> Error {
    Error::Unexpected(token.to_string())
}

/// A JSON streaming parser.
pub struct Parser<'source> {
    /// The last parsed token
    last_token: Token<'source>,
    lexer: Lexer<'source, Token<'source>>,
    context: Vec<Context>,
}

impl<'source> Parser<'source> {
    pub fn new(bytes: &'source str) -> Self {
        Self {
            lexer: Token::lexer(bytes),
            context: vec![],
            last_token: Token::Null,
        }
    }

    #[inline]
    fn consume_colon(&mut self) -> Result<(), Error> {
        match self.context.last_mut() {
            Some(Context::Object {
                expected_next_token,
                ..
            }) if *expected_next_token == ObjNextToken::Colon => {
                *expected_next_token = ObjNextToken::Value;
                Ok(())
            }
            _ => Err(unexpected(Token::Colon)),
        }
    }

    #[inline]
    fn consume_value(&mut self, token: Token) -> Result<(), Error> {
        match self.context.last_mut() {
            Some(Context::Object {
                expected_next_token,
                ..
            }) => match expected_next_token {
                ObjNextToken::Value => {
                    *expected_next_token = ObjNextToken::CommaOrClose;
                    Ok(())
                }
                _ => Err(unexpected(token)),
            },
            Some(Context::Array {
                len,
                expected_next_token,
            }) => match expected_next_token {
                ArrayNextToken::Value | ArrayNextToken::ValueOrClose => {
                    *len += 1;
                    *expected_next_token = ArrayNextToken::CommaOrClose;
                    Ok(())
                }
                ArrayNextToken::CommaOrClose => Err(Error::ExpectedCommaOrClose),
            },
            None => Ok(()),
        }
    }

    #[inline]
    fn consume_comma(&mut self) -> Result<(), Error> {
        match self.context.last_mut() {
            Some(Context::Object {
                expected_next_token,
                ..
            }) => match expected_next_token {
                ObjNextToken::CommaOrClose => {
                    *expected_next_token = ObjNextToken::Key;
                    Ok(())
                }
                _ => Err(unexpected(Token::Comma)),
            },

            //
            Some(Context::Array {
                expected_next_token,
                ..
            }) => match expected_next_token {
                ArrayNextToken::CommaOrClose => {
                    *expected_next_token = ArrayNextToken::Value;
                    Ok(())
                }
                _ => Err(unexpected(Token::Comma)),
            },
            _ => Err(unexpected(Token::Comma)),
        }
    }

    #[allow(unused)]
    /// The range of the last parsed token
    fn span(&self) -> Range<usize> {
        match self.last_token {
            // Logos returns the span including the double quotes, this removes them
            Token::String(_) => {
                let Range { start, end } = self.lexer.span();

                Range {
                    start: start + 1,
                    end: end - 1,
                }
            }
            _ => self.lexer.span(),
        }
    }

    pub fn parse_next(&mut self) -> Result<Option<Event<'_>>, Error> {
        while let Some(token) = self.lexer.next() {
            let token = token.unwrap();
            self.last_token = token;

            match token {
                Token::Colon => {
                    self.consume_colon()?;
                }
                Token::Comma => {
                    self.consume_comma()?;
                }

                Token::Null => {
                    self.consume_value(token)?;
                    return Ok(Some(Event::Null));
                }
                Token::Bool(boolean) => {
                    self.consume_value(token)?;
                    return Ok(Some(Event::Boolean(boolean)));
                }
                Token::BracketOpen => {
                    self.consume_value(token)?;
                    self.context.push(Context::Array {
                        len: 0,
                        expected_next_token: ArrayNextToken::ValueOrClose,
                    });
                    return Ok(Some(Event::StartArray));
                }
                Token::BracketClose => match self.context.pop() {
                    Some(Context::Array {
                        len,
                        expected_next_token,
                    }) => {
                        match expected_next_token {
                            ArrayNextToken::ValueOrClose | ArrayNextToken::CommaOrClose => {
                                // Valid states to close: empty array or after a value
                                return Ok(Some(Event::EndArray { len }));
                            }
                            ArrayNextToken::Value => {
                                // After comma, must see value, not ]
                                return Err(Error::TrailingComma);
                            }
                        }
                    }
                    _ => return Err(unexpected(token)),
                },
                Token::BraceOpen => {
                    self.consume_value(token)?;
                    self.context.push(Context::Object {
                        member_count: 0,
                        expected_next_token: ObjNextToken::KeyOrClose,
                    });
                    return Ok(Some(Event::StartObject));
                }
                Token::BraceClose => match self.context.pop() {
                    Some(Context::Object {
                        member_count,
                        expected_next_token,
                    }) => match expected_next_token {
                        ObjNextToken::KeyOrClose | ObjNextToken::CommaOrClose => {
                            return Ok(Some(Event::EndObject { member_count }));
                        }
                        _ => return Err(Error::TrailingComma),
                    },
                    _ => return Err(unexpected(token)),
                },
                Token::String(val) => match self.context() {
                    Some(Context::Object {
                        expected_next_token,
                        member_count,
                    }) => match expected_next_token {
                        ObjNextToken::Key | ObjNextToken::KeyOrClose => {
                            *expected_next_token = ObjNextToken::Colon;
                            *member_count += 1;
                            return Ok(Some(Event::Key(val)));
                        }
                        ObjNextToken::Value => {
                            self.consume_value(token)?;
                            return Ok(Some(Event::String(val)));
                        }
                        ObjNextToken::Colon | ObjNextToken::CommaOrClose => {
                            return Err(unexpected(token));
                        }
                    },
                    Some(Context::Array { .. }) | None => {
                        self.consume_value(token)?;
                        return Ok(Some(Event::String(val)));
                    }
                },
                Token::Number(num) => {
                    self.consume_value(token)?;
                    return Ok(Some(Event::Number(num)));
                }
            };
        }

        Ok(None)
    }

    fn context(&mut self) -> Option<&mut Context> {
        self.context.last_mut()
    }
}

#[cfg(test)]
mod tests {
    use serde_json::{Value, json};

    use super::*;

    #[test]
    fn boolean_and_null() {
        let mut parser = Parser::new("true");
        assert_eq!(parser.parse_next().unwrap(), Some(Event::Boolean(true)));

        let mut parser = Parser::new("false");
        assert_eq!(parser.parse_next().unwrap(), Some(Event::Boolean(false)));

        let mut parser = Parser::new("null");
        assert_eq!(parser.parse_next().unwrap(), Some(Event::Null));
    }

    #[test]
    fn empty_object() {
        let mut parser = Parser::new("{}");

        assert_eq!(parser.parse_next().unwrap(), Some(Event::StartObject));
        assert_eq!(
            parser.parse_next().unwrap(),
            Some(Event::EndObject { member_count: 0 })
        );
    }

    #[test]
    fn empty_array() {
        let mut parser = Parser::new("[]");

        assert_eq!(parser.parse_next().unwrap(), Some(Event::StartArray));
        assert_eq!(
            parser.parse_next().unwrap(),
            Some(Event::EndArray { len: 0 })
        );
    }

    #[test]
    fn array_empty_object() {
        let mut parser = Parser::new("[{}]");

        assert_eq!(parser.parse_next().unwrap(), Some(Event::StartArray));
        assert_eq!(parser.parse_next().unwrap(), Some(Event::StartObject));
        assert_eq!(
            parser.parse_next().unwrap(),
            Some(Event::EndObject { member_count: 0 })
        );
        assert_eq!(
            parser.parse_next().unwrap(),
            Some(Event::EndArray { len: 1 })
        );
        assert_eq!(parser.parse_next().unwrap(), None);
    }

    #[test]
    fn obj_with_empty_array() {
        let mut parser = Parser::new(r#"{"a":[]}"#);
        assert_eq!(parser.parse_next().unwrap(), Some(Event::StartObject));
        assert_eq!(parser.parse_next().unwrap(), Some(Event::Key("a")));
        assert_eq!(parser.parse_next().unwrap(), Some(Event::StartArray));
        assert_eq!(
            parser.parse_next().unwrap(),
            Some(Event::EndArray { len: 0 })
        );
        assert_eq!(
            parser.parse_next().unwrap(),
            Some(Event::EndObject { member_count: 1 })
        );
        assert_eq!(parser.parse_next().unwrap(), None);

        let json = json!([[], [[[]]], [], [[]]]);
        assert_eq!(parse_to_value(&json.to_string()), json);
    }

    #[test]
    fn array_with_empty_arrays() {
        let mut parser = Parser::new(r#"[[], []]"#);
        assert_eq!(parser.parse_next().unwrap(), Some(Event::StartArray));
        assert_eq!(parser.parse_next().unwrap(), Some(Event::StartArray));
        assert_eq!(
            parser.parse_next().unwrap(),
            Some(Event::EndArray { len: 0 })
        );
        assert_eq!(parser.parse_next().unwrap(), Some(Event::StartArray));
        assert_eq!(
            parser.parse_next().unwrap(),
            Some(Event::EndArray { len: 0 })
        );
        assert_eq!(
            parser.parse_next().unwrap(),
            Some(Event::EndArray { len: 2 })
        );
        assert_eq!(parser.parse_next().unwrap(), None);
    }

    #[test]
    fn unnested_object() {
        let mut parser = Parser::new(r#"{"name": "John", "age": 30}"#);

        assert_eq!(dbg!(parser.parse_next().unwrap()), Some(Event::StartObject));
        assert_eq!(dbg!(parser.parse_next().unwrap()), Some(Event::Key("name")));
        assert_eq!(
            dbg!(parser.parse_next().unwrap()),
            Some(Event::String("John"))
        );
        assert_eq!(dbg!(parser.parse_next().unwrap()), Some(Event::Key("age")));
        assert_eq!(
            dbg!(parser.parse_next().unwrap()),
            Some(Event::Number("30"))
        );
        assert_eq!(
            dbg!(parser.parse_next().unwrap()),
            Some(Event::EndObject { member_count: 2 })
        );
        assert_eq!(parser.parse_next().unwrap(), None);
    }

    #[test]
    fn nested_object() {
        let json = r#"{"person": {"name": "John", "age": 30}}"#;
        let mut parser = Parser::new(json);

        assert_eq!(parser.parse_next().unwrap(), Some(Event::StartObject));
        assert_eq!(
            dbg!(parser.parse_next().unwrap()),
            Some(Event::Key("person"))
        );
        assert_eq!(dbg!(parser.parse_next().unwrap()), Some(Event::StartObject));
        assert_eq!(dbg!(parser.parse_next().unwrap()), Some(Event::Key("name")));
        assert_eq!(
            dbg!(parser.parse_next().unwrap()),
            Some(Event::String("John"))
        );
        assert_eq!(dbg!(parser.parse_next().unwrap()), Some(Event::Key("age")));
        assert_eq!(parser.parse_next().unwrap(), Some(Event::Number("30")));
        assert_eq!(
            parser.parse_next().unwrap(),
            Some(Event::EndObject { member_count: 2 })
        );
        assert_eq!(
            parser.parse_next().unwrap(),
            Some(Event::EndObject { member_count: 1 })
        );
        assert_eq!(parser.parse_next().unwrap(), None);
    }

    #[test]
    fn num_array() {
        let json = r#"[1, 2, 3]"#;
        let mut parser = Parser::new(json);

        assert_eq!(parser.parse_next().unwrap(), Some(Event::StartArray));
        assert_eq!(parser.parse_next().unwrap(), Some(Event::Number("1")));
        assert_eq!(parser.parse_next().unwrap(), Some(Event::Number("2")));
        assert_eq!(parser.parse_next().unwrap(), Some(Event::Number("3")));
        assert_eq!(
            parser.parse_next().unwrap(),
            Some(Event::EndArray { len: 3 })
        );
        assert_eq!(parser.parse_next().unwrap(), None);
    }

    #[test]
    fn string_array() {
        let json = r#"["apple", "banana", "cherry"]"#;
        let mut parser = Parser::new(json);

        assert_eq!(parser.parse_next().unwrap(), Some(Event::StartArray));
        assert_eq!(parser.parse_next().unwrap(), Some(Event::String("apple")));
        assert_eq!(parser.parse_next().unwrap(), Some(Event::String("banana")));
        assert_eq!(parser.parse_next().unwrap(), Some(Event::String("cherry")));
        assert_eq!(
            parser.parse_next().unwrap(),
            Some(Event::EndArray { len: 3 })
        );
        assert_eq!(parser.parse_next().unwrap(), None);
    }

    #[test]
    fn mixed_array() {
        let json = r#"[1, "apple", true, null]"#;
        let mut parser = Parser::new(json);

        assert_eq!(parser.parse_next().unwrap(), Some(Event::StartArray));
        assert_eq!(parser.parse_next().unwrap(), Some(Event::Number("1")));
        assert_eq!(parser.parse_next().unwrap(), Some(Event::String("apple")));
        assert_eq!(parser.parse_next().unwrap(), Some(Event::Boolean(true)));
        assert_eq!(parser.parse_next().unwrap(), Some(Event::Null));
        assert_eq!(
            parser.parse_next().unwrap(),
            Some(Event::EndArray { len: 4 })
        );
        assert_eq!(parser.parse_next().unwrap(), None);
    }

    #[test]
    fn malformed_array() {
        let json = r#"[1,,2]"#;
        let mut parser = Parser::new(json);

        assert_eq!(parser.parse_next().unwrap(), Some(Event::StartArray));
        assert_eq!(parser.parse_next().unwrap(), Some(Event::Number("1")));
        match parser.parse_next().unwrap_err() {
            Error::Unexpected(token) if token == "," => (),
            err => panic!("Expected Unexpected(Token::Comma), got {:?}", err),
        }
    }

    #[test]
    fn array_trailing_comma() {
        let json = r#"[1,2,]"#;
        let mut parser = Parser::new(json);

        assert_eq!(parser.parse_next().unwrap(), Some(Event::StartArray));
        assert_eq!(parser.parse_next().unwrap(), Some(Event::Number("1")));
        assert_eq!(parser.parse_next().unwrap(), Some(Event::Number("2")));
        match parser.parse_next().unwrap_err() {
            Error::TrailingComma => (),
            err => panic!("Expected TrailingComma, got {:?}", err),
        }
    }

    #[test]
    fn obj_trailing_comma() {
        let json = r#"{"name": "John", "age": 30,}"#;
        let mut parser = Parser::new(json);

        assert_eq!(parser.parse_next().unwrap(), Some(Event::StartObject));
        assert_eq!(parser.parse_next().unwrap(), Some(Event::Key("name")));
        assert_eq!(parser.parse_next().unwrap(), Some(Event::String("John")));
        assert_eq!(parser.parse_next().unwrap(), Some(Event::Key("age")));
        assert_eq!(parser.parse_next().unwrap(), Some(Event::Number("30")));
        match parser.parse_next().unwrap_err() {
            Error::TrailingComma => (),
            err => panic!("Expected TrailingComma, got {:?}", err),
        }
    }

    // This is to allow for some more compact test cases, without needing to write out a bunch of `parse_next` calls
    fn parse_to_value(json: &str) -> Value {
        let mut parser = Parser::new(json);

        enum Container {
            Object {
                map: serde_json::Map<String, Value>,
                key: Option<String>,
            },
            Array {
                vec: Vec<Value>,
                key: Option<String>,
            },
        }

        fn add_value(
            stack: &mut Vec<Container>,
            current_key: &mut Option<String>,
            root: &mut Value,
            value: Value,
        ) {
            if stack.is_empty() {
                *root = value;
            } else {
                match stack.last_mut().unwrap() {
                    Container::Object { map, .. } => {
                        map.insert(current_key.take().unwrap(), value);
                    }
                    Container::Array { vec, .. } => {
                        vec.push(value);
                    }
                }
            }
        }

        let mut stack: Vec<Container> = Vec::new();
        let mut current_key: Option<String> = None;
        let mut root = Value::Null;

        while let Some(event) = parser.parse_next().unwrap() {
            match event {
                Event::StartObject => {
                    stack.push(Container::Object {
                        map: serde_json::Map::new(),
                        key: current_key.take(),
                    });
                }
                Event::EndObject { .. } => {
                    if let Some(Container::Object { map, key }) = stack.pop() {
                        current_key = key;
                        add_value(&mut stack, &mut current_key, &mut root, Value::Object(map));
                    }
                }
                Event::StartArray => {
                    stack.push(Container::Array {
                        vec: Vec::new(),
                        key: current_key.take(),
                    });
                }
                Event::EndArray { .. } => {
                    if let Some(Container::Array { vec, key }) = stack.pop() {
                        current_key = key;
                        add_value(&mut stack, &mut current_key, &mut root, Value::Array(vec));
                    }
                }
                Event::Key(k) => {
                    current_key = Some(k.to_string());
                }
                Event::String(s) => {
                    add_value(
                        &mut stack,
                        &mut current_key,
                        &mut root,
                        Value::String(s.to_string()),
                    );
                }
                Event::Number(n) => {
                    add_value(
                        &mut stack,
                        &mut current_key,
                        &mut root,
                        Value::Number(n.parse().unwrap()),
                    );
                }
                Event::Boolean(b) => {
                    add_value(&mut stack, &mut current_key, &mut root, Value::Bool(b));
                }
                Event::Null => {
                    add_value(&mut stack, &mut current_key, &mut root, Value::Null);
                }
            }
        }

        root
    }

    #[test]
    fn deeply_nested() {
        let expected = serde_json::json!({
            "a": {"b": {"c": {"d": {"e": {"f": "deep"}}}}}
        });
        assert_eq!(parse_to_value(&expected.to_string()), expected);
    }

    #[test]
    fn mixed_nested_structures() {
        let expected = serde_json::json!({
            "users": [
                {"name": "Alice", "tags": ["admin", "user"]},
                {"name": "Bob", "tags": []},
                {"name": "Charlie", "tags": ["user"]}
            ],
            "meta": {"count": 3}
        });
        assert_eq!(parse_to_value(&expected.to_string()), expected);
    }

    #[test]
    fn empty_containers() {
        let expected = serde_json::json!({
            "empty_obj": {},
            "empty_arr": [],
            "nested": {"also_empty": {}, "arr": [[], {}]}
        });
        assert_eq!(parse_to_value(&expected.to_string()), expected);
    }

    #[test]
    fn all_value_types() {
        let expected = serde_json::json!({
            "string": "hello",
            "number": 42,
            "float": 3.14,
            "bool_true": true,
            "bool_false": false,
            "null_value": null,
            "negative": -123,
            "array": [1, "two", true, null, {}]
        });
        assert_eq!(parse_to_value(&expected.to_string()), expected);
    }

    #[test]
    fn array_of_arrays() {
        let expected = serde_json::json!([[1, 2, 3], [4, 5, 6], [[7, 8], [9, 10]], []]);
        assert_eq!(parse_to_value(&expected.to_string()), expected);
    }

    #[test]
    fn top_level_primitives() {
        let expected = serde_json::json!("just a string");
        assert_eq!(parse_to_value(&expected.to_string()), expected);

        let expected = serde_json::json!(42);
        assert_eq!(parse_to_value(&expected.to_string()), expected);

        let expected = serde_json::json!(true);
        assert_eq!(parse_to_value(&expected.to_string()), expected);

        let expected = serde_json::json!(false);
        assert_eq!(parse_to_value(&expected.to_string()), expected);

        let expected = serde_json::json!(null);
        assert_eq!(parse_to_value(&expected.to_string()), expected);

        let expected = serde_json::json!([]);
        assert_eq!(parse_to_value(&expected.to_string()), expected);
    }

    #[test]
    fn objects_with_numeric_string_keys() {
        let expected = serde_json::json!({
            "0": "zero",
            "1": "one",
            "123": "one-two-three"
        });
        assert_eq!(parse_to_value(&expected.to_string()), expected);
    }

    #[test]
    fn single_element_containers() {
        let expected = serde_json::json!({
            "single_key": {"nested": [{"deep": "value"}]}
        });
        assert_eq!(parse_to_value(&expected.to_string()), expected);
    }

    #[test]
    fn spanned() {
        let json = r#"[1, "apple", true, null]"#;
        let mut parser = Parser::new(json);

        assert_eq!(parser.parse_next().unwrap(), Some(Event::StartArray));
        assert_eq!(&json[parser.span()], "[");
        assert_eq!(parser.parse_next().unwrap(), Some(Event::Number("1")));
        assert_eq!(&json[parser.span()], "1");
        assert_eq!(parser.parse_next().unwrap(), Some(Event::String("apple")));
        assert_eq!(&json[parser.span()], "apple");
        assert_eq!(parser.parse_next().unwrap(), Some(Event::Boolean(true)));
        assert_eq!(&json[parser.span()], "true");
        assert_eq!(parser.parse_next().unwrap(), Some(Event::Null));
        assert_eq!(&json[parser.span()], "null");
        assert_eq!(
            parser.parse_next().unwrap(),
            Some(Event::EndArray { len: 4 })
        );
        assert_eq!(&json[parser.span()], "]");
        assert_eq!(parser.parse_next().unwrap(), None);
    }

    #[test]
    fn spanned_empty_string() {
        let json = r#"[1, "", "\"", {"a":"c"}]"#;
        let mut parser = Parser::new(json);

        assert_eq!(parser.parse_next().unwrap(), Some(Event::StartArray));
        assert_eq!(&json[parser.span()], "[");
        assert_eq!(parser.parse_next().unwrap(), Some(Event::Number("1")));
        assert_eq!(&json[parser.span()], "1");
        assert_eq!(parser.parse_next().unwrap(), Some(Event::String("")));
        assert_eq!(&json[parser.span()], "");
        assert_eq!(parser.parse_next().unwrap(), Some(Event::String(r#"\""#)));
        assert_eq!(&json[parser.span()], r#"\""#);
        assert_eq!(parser.parse_next().unwrap(), Some(Event::StartObject));
        assert_eq!(&json[parser.span()], "{");
        assert_eq!(parser.parse_next().unwrap(), Some(Event::Key("a")));
        assert_eq!(&json[parser.span()], "a");
        assert_eq!(parser.parse_next().unwrap(), Some(Event::String("c")));
        assert_eq!(&json[parser.span()], "c");
        assert_eq!(
            parser.parse_next().unwrap(),
            Some(Event::EndObject { member_count: 1 })
        );
        assert_eq!(&json[parser.span()], "}");
        assert_eq!(
            parser.parse_next().unwrap(),
            Some(Event::EndArray { len: 4 })
        );
        assert_eq!(&json[parser.span()], "]");
        assert_eq!(parser.parse_next().unwrap(), None);
    }
}
