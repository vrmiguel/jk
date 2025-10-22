use std::fmt::Display;

use logos::Logos;

#[derive(Debug, Logos, Clone, Copy, PartialEq)]
#[logos(skip r"[ \t\r\n\f]+")]
pub enum GronToken<'source> {
    #[token(".")]
    Dot,

    #[token("[")]
    BracketOpen,

    #[token("]")]
    BracketClose,

    #[token("=")]
    Equals,

    #[token(";")]
    Semicolon,

    #[token("{}")]
    EmptyObject,

    #[token("[]")]
    EmptyArray,

    #[token("true", |_| true)]
    #[token("false", |_| false)]
    Bool(bool),

    #[token("null")]
    Null,

    #[regex(r"-?(?:0|[1-9]\d*)(?:\.\d+)?(?:[eE][+-]?\d+)?", |lex| lex.slice())]
    Number(&'source str),

    #[regex(r#""([^"\\\x00-\x1F]|\\(["\\bnfrt/]|u[a-fA-F0-9]{4}))*""#, |lex| trim_quotes(lex.slice()))]
    String(&'source str),

    #[regex(r"[a-zA-Z_][a-zA-Z0-9_]*", |lex| lex.slice())]
    Identifier(&'source str),
}

#[inline]
fn trim_quotes(input: &str) -> &str {
    let len = input.len();

    debug_assert_eq!(input.as_bytes()[0], b'"');
    debug_assert_eq!(input.as_bytes()[len - 1], b'"');

    &input[1..len - 1]
}

impl Display for GronToken<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GronToken::Dot => f.write_str("a dot ('.')"),
            GronToken::BracketOpen => f.write_str("an opening bracket ('[')"),
            GronToken::BracketClose => f.write_str("a closing bracket (']')"),
            GronToken::Equals => f.write_str("an equals sign ('=')"),
            GronToken::Semicolon => f.write_str("a semicolon (';')"),
            GronToken::EmptyObject => f.write_str("an empty object ('{}')"),
            GronToken::EmptyArray => f.write_str("an empty array ('[]')"),
            GronToken::Bool(b) => write!(f, "a boolean ('{}')", b),
            GronToken::Null => f.write_str("null"),
            GronToken::Number(n) => write!(f, "a number ('{}')", n),
            GronToken::String(s) => write!(f, "a string (\"{}\")", s),
            GronToken::Identifier(id) => write!(f, "an identifier ('{}')", id),
        }
    }
}
