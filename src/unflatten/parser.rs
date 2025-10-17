use nom::{
    IResult, Parser,
    branch::alt,
    bytes::{complete::tag, escaped, is_not},
    character::{
        complete::{alpha1, char, digit1},
        multispace0, one_of,
    },
    combinator::{opt, success},
    error::ParseError,
    number::complete::recognize_float,
    sequence::{delimited, preceded},
};

use crate::unflatten::types::{GronLine, GronValue, Identifier, Index};

pub fn parse_gron_line<'a>(input: &'a str) -> IResult<&'a str, GronLine<'a>> {
    let (rest, identifier) = parse_identifier(input)?;
    let (rest, _) = ws(tag("=")).parse(rest)?;
    let (rest, value) = parse_value(rest)?;
    let (rest, _) = ws(tag(";")).parse(rest)?;

    Ok((rest, GronLine { identifier, value }))
}

fn parse_identifier<'a, 'b>(mut rest: &'a str) -> IResult<&'a str, Vec<Identifier<'a>>> {
    let mut components = Vec::with_capacity(4);

    while let Ok((new_rest, identifier)) = parse_identifier_path(rest) {
        rest = new_rest;
        components.push(identifier);
    }

    Ok((rest, components))
}

// Parses a string, assuming it has already been correctly escaped.
// I need to double check if this is correct.
fn escaped_string<'a>(input: &'a str) -> IResult<&'a str, &'a str> {
    delimited(
        char('"'),
        alt((
            escaped(is_not("\\\""), '\\', one_of("\"\\bfnrtu/")),
            // Accept empty string
            success(""),
        )),
        char('"'),
    )
    .parse(input)
}

fn parse_value(input: &str) -> IResult<&str, GronValue<'_>> {
    let value = alt((
        tag("null").map(|_| GronValue::Null),
        tag("true").map(|_| GronValue::Boolean(true)),
        tag("false").map(|_| GronValue::Boolean(false)),
        tag("{}").map(|_| GronValue::Object),
        tag("[]").map(|_| GronValue::Array),
        recognize_float.map(|n| GronValue::Number(n)),
        escaped_string.map(|s| GronValue::String(s)),
    ));

    preceded(multispace0(), value).parse(input)
}

/// Parses the next single, dot separated, path of a Gron 'identifier'.
///
/// Examples:
/// 1. "json.hobbies[0]" -> base: "json", index: None
/// 2. ".hobbies[0]" -> base: ".hobbies", index: Some("0"")
/// 2. Identifier { base: "hobbies", index: Some(Index::Array("0")) }
fn parse_identifier_path<'a>(input: &'a str) -> IResult<&'a str, Identifier<'a>> {
    let numeric_index = delimited(char('['), digit1, char(']')).map(|idx| Index::Numeric(idx));
    let string_index =
        delimited(char('['), escaped_string, char(']')).map(|idx| Index::String(idx));

    let (rest, _dot) = opt(char('.')).parse(input)?;
    let (rest, base) = alpha1(rest)?;
    let (rest, index) = opt(alt((numeric_index, string_index))).parse(rest)?;

    Ok((rest, Identifier { base, index }))
}

fn ws<'a, O, E: ParseError<&'a str>, F>(inner: F) -> impl Parser<&'a str, Output = O, Error = E>
where
    F: Parser<&'a str, Output = O, Error = E>,
{
    delimited(multispace0(), inner, multispace0())
}

#[cfg(test)]
mod tests {
    use crate::unflatten::{
        parser::{parse_identifier_path, parse_value},
        types::{GronValue, Identifier, Index},
    };

    fn parse_collect_identifier_path(mut input: &str) -> Vec<Identifier<'_>> {
        let it = std::iter::from_fn(move || match parse_identifier_path(input) {
            Ok((i, o)) => {
                input = i;
                Some(o)
            }
            _ => None,
        });

        it.collect()
    }

    #[test]
    fn test_parse_identifier() {
        // Original test
        assert_eq!(
            parse_collect_identifier_path("json.address[1].test[\"hey\"]"),
            vec![
                Identifier {
                    base: "json",
                    index: None
                },
                Identifier {
                    base: "address",
                    index: Some(Index::Numeric("1"))
                },
                Identifier {
                    base: "test",
                    index: Some(Index::String("hey"))
                },
            ]
        );

        // Simple root identifier
        assert_eq!(
            parse_collect_identifier_path("json"),
            vec![Identifier {
                base: "json",
                index: None
            },]
        );

        // Root array access
        assert_eq!(
            parse_collect_identifier_path("json[0]"),
            vec![Identifier {
                base: "json",
                index: Some(Index::Numeric("0"))
            },]
        );

        // Deep nesting with only properties
        assert_eq!(
            parse_collect_identifier_path("json.a.b.c.d"),
            vec![
                Identifier {
                    base: "json",
                    index: None
                },
                Identifier {
                    base: "a",
                    index: None
                },
                Identifier {
                    base: "b",
                    index: None
                },
                Identifier {
                    base: "c",
                    index: None
                },
                Identifier {
                    base: "d",
                    index: None
                },
            ]
        );

        // String keys with special characters
        assert_eq!(
            parse_collect_identifier_path("json[\"key with spaces\"]"),
            vec![Identifier {
                base: "json",
                index: Some(Index::String("key with spaces"))
            },]
        );

        assert_eq!(
            parse_collect_identifier_path("json[\"key-with-dashes\"]"),
            vec![Identifier {
                base: "json",
                index: Some(Index::String("key-with-dashes"))
            },]
        );

        assert_eq!(
            parse_collect_identifier_path("json[\"key.with.dots\"]"),
            vec![Identifier {
                base: "json",
                index: Some(Index::String("key.with.dots"))
            },]
        );

        assert_eq!(
            parse_collect_identifier_path("json[\"path\\\\to\\\\file\"]"),
            vec![Identifier {
                base: "json",
                index: Some(Index::String("path\\\\to\\\\file"))
            },]
        );

        assert_eq!(
            parse_collect_identifier_path("json[\"key with \\\"quotes\\\"\"]"),
            vec![Identifier {
                base: "json",
                index: Some(Index::String("key with \\\"quotes\\\""))
            },]
        );

        assert_eq!(
            parse_collect_identifier_path("json[\"\"]"),
            vec![Identifier {
                base: "json",
                index: Some(Index::String(""))
            },]
        );

        assert_eq!(
            parse_collect_identifier_path("json.users[0].address[\"street name\"]"),
            vec![
                Identifier {
                    base: "json",
                    index: None
                },
                Identifier {
                    base: "users",
                    index: Some(Index::Numeric("0"))
                },
                Identifier {
                    base: "address",
                    index: Some(Index::String("street name"))
                },
            ]
        );

        assert_eq!(
            parse_collect_identifier_path("json.items[999].data[12345]"),
            vec![
                Identifier {
                    base: "json",
                    index: None
                },
                Identifier {
                    base: "items",
                    index: Some(Index::Numeric("999"))
                },
                Identifier {
                    base: "data",
                    index: Some(Index::Numeric("12345"))
                },
            ]
        );
    }

    #[test]
    fn test_parse_value() {
        // Null
        assert_eq!(parse_value("null"), Ok(("", GronValue::Null)));

        // Booleans
        assert_eq!(parse_value("true"), Ok(("", GronValue::Boolean(true))));
        assert_eq!(parse_value("false"), Ok(("", GronValue::Boolean(false))));

        // Objects and Arrays
        assert_eq!(parse_value("{}"), Ok(("", GronValue::Object)));
        assert_eq!(parse_value("[]"), Ok(("", GronValue::Array)));

        // Numbers - integers
        assert_eq!(parse_value("0"), Ok(("", GronValue::Number("0"))));
        assert_eq!(parse_value("123"), Ok(("", GronValue::Number("123"))));
        assert_eq!(parse_value("-456"), Ok(("", GronValue::Number("-456"))));

        // Numbers - decimals
        assert_eq!(
            parse_value("123.456"),
            Ok(("", GronValue::Number("123.456")))
        );
        assert_eq!(parse_value("-0.5"), Ok(("", GronValue::Number("-0.5"))));

        // Numbers - scientific notation
        assert_eq!(
            parse_value("1.23e10"),
            Ok(("", GronValue::Number("1.23e10")))
        );
        assert_eq!(parse_value("1.5E-5"), Ok(("", GronValue::Number("1.5E-5"))));
        assert_eq!(
            parse_value("-2.5e+3"),
            Ok(("", GronValue::Number("-2.5e+3")))
        );

        // Strings - simple
        assert_eq!(
            parse_value("\"hello\""),
            Ok(("", GronValue::String("hello")))
        );
        assert_eq!(parse_value("\"\""), Ok(("", GronValue::String(""))));

        // Strings - with spaces
        assert_eq!(
            parse_value("\"hello world\""),
            Ok(("", GronValue::String("hello world")))
        );

        // Strings - with escapes
        assert_eq!(
            parse_value("\"escaped \\\"quotes\\\"\""),
            Ok(("", GronValue::String("escaped \\\"quotes\\\"")))
        );
        assert_eq!(
            parse_value("\"path\\\\to\\\\file\""),
            Ok(("", GronValue::String("path\\\\to\\\\file")))
        );
        assert_eq!(
            parse_value("\"line1\\nline2\""),
            Ok(("", GronValue::String("line1\\nline2")))
        );

        // With leading whitespace (multispace0 should handle it)
        assert_eq!(parse_value("   true"), Ok(("", GronValue::Boolean(true))));
        assert_eq!(parse_value("\t\t123"), Ok(("", GronValue::Number("123"))));

        // With trailing content (shouldn't be consumed)
        assert_eq!(parse_value("true;"), Ok((";", GronValue::Boolean(true))));
        assert_eq!(
            parse_value("123 // comment"),
            Ok((" // comment", GronValue::Number("123")))
        );
    }
}

// json = {}
// json.address = {}
// json.address.street = "123 Main St"
// json.address.zip = "10001"
// json.age = 30
// json.city = "New York"
// json.hobbies = []
// json.hobbies[0] = "reading"
// json.hobbies[1] = "cycling"
// json.name = "John"
