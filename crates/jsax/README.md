# jsax 🎷

A ["SAX"](https://en.wikipedia.org/wiki/Simple_API_for_XML)-style parser for JSON.

## Overview

Most JSON parsers (esp. in the Rust ecosystem) build a complete DOM tree in memory (like `serde_json::Value`).
`jsax` differs in the sense that it merely emits events as it processes the source text. This makes `jsax` ideal for:

- Processing large JSON files with bounded memory usage: it doesn't matter much if you're parsing 1MB or 5GB
- Selective parsing (skip irrelevant data without allocation)
- Performance-critical applications

## Events

```rust
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
```

## Example

```rust
use jsax::{Parser, Event};

let json = r#"{"name": "Alice", "age": 30}"#;
let mut parser = Parser::new(json);

while let Some(event) = parser.parse_next()? {
    match event {
        Event::Key(key) => println!("Key: {}", key),
        Event::String(s) => println!("String: {}", s),
        Event::Number(n) => println!("Number: {}", n),
        _ => {}
    }
}
```

## Important Notes

### No unescaping

String values are returned **exactly as they appear** in the source JSON, including escape sequences.
This is great for performance, the catch is that you'll have to handle unescaping yourself, _if_ you actually need to.

### Numbers

Numbers are returned as string slices, _not_ parsed as u64/f64/etc.

```rust
// Input: {"value": 3.14e10}
Event::Number("3.14e10")  // If required, you need to parse this yourself
```

This allows you to choose your preferred number representation (f64, Decimal, BigInt, etc.) or defer parsing.

## Don't use jsax when:

- You need a simple deserialize-to-struct workflow (that's what `serde_json` and `simd_json` are for)
- You need random access to JSON data (use something like `serde_json::Value`)
- You want automatic string unescaping and number parsing

## Inspiration

`jsax` broadly implements the event structure defined by [rapidjson](https://rapidjson.org/md_doc_stream.html).

Unlike rapidjson, however, `jsax` does not employ SIMD at all at the moment.