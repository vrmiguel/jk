use std::fmt::{Display, Write};

use logos::Logos;

#[derive(Debug, Logos, Clone, Copy)]
#[logos(skip r"[ \t\r\n\f]+")]
pub enum Token<'source> {
    #[token("false", |_| false)]
    #[token("true", |_| true)]
    Bool(bool),

    #[token("{")]
    BraceOpen,

    #[token("}")]
    BraceClose,

    /// "["
    #[token("[")]
    BracketOpen,

    /// "]"
    #[token("]")]
    BracketClose,

    #[token(":")]
    Colon,

    #[token(",")]
    Comma,

    #[token("null")]
    Null,

    #[regex(r"-?(?:0|[1-9]\d*)(?:\.\d+)?(?:[eE][+-]?\d+)?", |lex| lex.slice())]
    Number(&'source str),

    #[regex(r#""([^"\\\x00-\x1F]|\\(["\\bnfrt/]|u[a-fA-F0-9]{4}))*""#, |lex| trim_quotes(lex.slice()))]
    String(&'source str),
}

#[inline]
fn trim_quotes(input: &str) -> &str {
    let len = input.len();

    debug_assert_eq!(input.as_bytes()[0], b'"');
    debug_assert_eq!(input.as_bytes()[len - 1], b'"');

    &input[1..len - 1]
}

impl Display for Token<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Token::Bool(b) => write!(f, "{b}"),
            Token::BraceOpen => f.write_char('{'),
            Token::BraceClose => f.write_char('}'),
            Token::BracketOpen => f.write_char('['),
            Token::BracketClose => f.write_char(']'),
            Token::Colon => f.write_char(':'),
            Token::Comma => f.write_char(','),
            Token::Null => f.write_str("null"),
            Token::Number(num) => f.write_str(num),
            Token::String(val) => f.write_str(val),
        }
    }
}
