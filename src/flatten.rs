use std::{
    fmt::{self, Display, Write},
    io::{self, BufWriter, Write as IoWrite, stdout},
};

use jsax::Event;
use serde_json::Value;

// TODO: implement other formats. Only `gron` is supported so far.
#[allow(dead_code)]
pub enum FlattenFormat {
    Gron,
    Pointer,
}

/// Applies JSON string escaping
struct Escaped<'a>(&'a str);

impl Display for Escaped<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for ch in self.0.chars() {
            match ch {
                '\\' => f.write_str("\\\\")?,
                '"' => f.write_str("\\\"")?,
                _ => f.write_char(ch)?,
            }
        }
        Ok(())
    }
}

/// A struct whose Display impl prints out the flattened version of this [Value].
// TODO: the next step here, in terms of performance, is to implement this using a SAX/streaming parser
pub struct Flattened<'a> {
    val: &'a serde_json::Value,
}

impl Display for Flattened<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut path = String::with_capacity(256);
        path.push_str("json");

        fn flatten_rec(
            value: &Value,
            path: &mut String,
            f: &mut fmt::Formatter<'_>,
        ) -> fmt::Result {
            match value {
                Value::Object(map) => {
                    writeln!(f, "{path} = {{}};")?;

                    for (key, val) in map {
                        let len = path.len();
                        path.push('.');
                        path.push_str(key);

                        flatten_rec(val, path, f)?;

                        path.truncate(len);
                    }
                }
                Value::Array(arr) => {
                    writeln!(f, "{path} = [];")?;

                    for (idx, val) in arr.iter().enumerate() {
                        let len = path.len();
                        write!(path, "[{}]", idx).unwrap();

                        flatten_rec(val, path, f)?;

                        path.truncate(len);
                    }
                }
                Value::String(s) => {
                    writeln!(f, "{path} = \"{}\";", Escaped(s))?;
                }
                Value::Number(n) => {
                    writeln!(f, "{path} = {n};")?;
                }
                Value::Bool(b) => {
                    writeln!(f, "{path} = {b};")?;
                }
                Value::Null => {
                    writeln!(f, "{path} = null;")?;
                }
            }
            Ok(())
        }

        flatten_rec(&self.val, &mut path, f)
    }
}

pub fn flatten(
    mut parser: jsax::Parser<io::Error, impl Iterator<Item = Result<u8, io::Error>>>,
) -> anyhow::Result<()> {
    let stdout = stdout();
    let mut writer = BufWriter::new(stdout.lock());

    let mut path = String::from("json");

    // Stack of (base_path_length, array_index_if_applicable)
    let mut stack: Vec<(usize, Option<usize>)> = Vec::new();

    while let Some(event) = parser.parse_next()? {
        match event {
            Event::StartObject => {
                writeln!(writer, "{path} = {{}};")?;

                stack.push((path.len(), None));
            }
            Event::StartArray => {
                writeln!(writer, "{path} = [];")?;

                let base_len = path.len();
                stack.push((base_len, Some(0)));

                write!(path, "[0]").unwrap();
            }
            Event::Key(key) => {
                if let Some(&(base_len, _)) = stack.last() {
                    path.truncate(base_len);
                }

                path.push('.');
                path.push_str(key);
            }
            Event::String(s) => {
                writeln!(writer, "{path} = \"{s}\";")?;

                if let Some((base_len, array_idx)) = stack.last_mut() {
                    if let Some(idx) = array_idx {
                        *idx += 1;
                        path.truncate(*base_len);
                        write!(path, "[{}]", idx).unwrap();
                    } else {
                        path.truncate(*base_len);
                    }
                }
            }
            Event::Number(n) => {
                writeln!(writer, "{path} = {n};")?;
                if let Some((base_len, array_idx)) = stack.last_mut() {
                    if let Some(idx) = array_idx {
                        *idx += 1;
                        path.truncate(*base_len);
                        write!(path, "[{}]", idx).unwrap();
                    } else {
                        path.truncate(*base_len);
                    }
                }
            }
            Event::Boolean(b) => {
                writeln!(writer, "{path} = {b};")?;
                if let Some((base_len, array_idx)) = stack.last_mut() {
                    if let Some(idx) = array_idx {
                        *idx += 1;
                        path.truncate(*base_len);
                        write!(path, "[{}]", idx).unwrap();
                    } else {
                        path.truncate(*base_len);
                    }
                }
            }
            Event::Null => {
                writeln!(writer, "{path} = null;")?;
                if let Some((base_len, array_idx)) = stack.last_mut() {
                    if let Some(idx) = array_idx {
                        *idx += 1;
                        path.truncate(*base_len);
                        write!(path, "[{}]", idx).unwrap();
                    } else {
                        path.truncate(*base_len);
                    }
                }
            }
            Event::EndObject { .. } | Event::EndArray { .. } => {
                stack.pop();

                if let Some((base_len, array_idx)) = stack.last_mut() {
                    if let Some(idx) = array_idx {
                        *idx += 1;
                        path.truncate(*base_len);
                        write!(path, "[{}]", idx).unwrap();
                    } else {
                        path.truncate(*base_len);
                    }
                }
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flatten_simple_object() {
        let json = serde_json::json!({
            "a": "b",
            "c": "d",
        });
        let flattened = Flattened { val: &json };
        assert_eq!(
            flattened.to_string(),
            "json = {};\njson.a = \"b\";\njson.c = \"d\";\n"
        );
    }

    #[test]
    fn flatten_array() {
        let json = serde_json::json!([1, 2, 3]);
        let flattened = Flattened { val: &json };
        assert_eq!(
            flattened.to_string(),
            "json = [];\njson[0] = 1;\njson[1] = 2;\njson[2] = 3;\n"
        );

        let json = serde_json::json!([
            {
                "name": "John Doe",
                "age": 30,
                "hobbies": [
                    "reading",
                    "cycling"
                ]
            },
            {
                "name": "Jane Doe",
                "age": 25,
                "hobbies": [
                    "swimming",
                    "dancing"
                ]
            }
        ]);
        let flattened = Flattened { val: &json };
        assert_eq!(
            flattened.to_string(),
            "json = [];\njson[0] = {};\njson[0].age = 30;\njson[0].hobbies = [];\njson[0].hobbies[0] = \"reading\";\njson[0].hobbies[1] = \"cycling\";\njson[0].name = \"John Doe\";\njson[1] = {};\njson[1].age = 25;\njson[1].hobbies = [];\njson[1].hobbies[0] = \"swimming\";\njson[1].hobbies[1] = \"dancing\";\njson[1].name = \"Jane Doe\";\n"
        );
    }
}
