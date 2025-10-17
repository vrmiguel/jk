use std::{
    fmt::{self, Display, Write},
    io::{BufWriter, Write as IoWrite, stdout},
};

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

/// Displays a flattened version of a JSON object
pub fn flatten(value: serde_json::Value) -> anyhow::Result<()> {
    let stdout = stdout();
    let mut writer = BufWriter::new(stdout.lock());
    write!(writer, "{}", Flattened { val: &value })?;

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
