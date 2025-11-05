use std::io::Write as IoWrite;

use jsax::{Event, Parser};
// TODO: implement other formats. Only `gron` is supported so far.
#[allow(dead_code)]
pub enum FlattenFormat {
    Gron,
    Pointer,
}

enum ContextFrame {
    /// Currently inside object: tracks where to truncate path for next key
    Object { base_path_len: usize },
    /// Currently inside array: tracks path truncation point and current index
    Array { base_path_len: usize, index: usize },
}

#[inline]
fn push_array_index(path: &mut String, idx: usize) {
    path.push('[');

    if idx < 10 {
        path.push((b'0' + idx as u8) as char);
        path.push(']');
        return;
    }

    let mut buf = itoa::Buffer::new();
    path.push_str(buf.format(idx));
    path.push(']');
}

pub fn flatten<W: IoWrite>(input: &str, mut writer: W) -> anyhow::Result<()> {
    let mut parser = Parser::new(input);

    let mut path = String::with_capacity(128);
    path.push_str("json");

    let mut stack = Vec::with_capacity(4);

    #[inline]
    fn update_path_after_value(stack: &mut Vec<ContextFrame>, path: &mut String) {
        match stack.last_mut() {
            Some(ContextFrame::Array {
                base_path_len,
                index,
            }) => {
                // Increment to next array index
                *index += 1;
                // Truncate path back to array base (e.g., "json.arr[5]" → "json.arr")
                path.truncate(*base_path_len);
                // Append new index (e.g., "json.arr" → "json.arr[6]")
                push_array_index(path, *index);
            }
            Some(ContextFrame::Object { base_path_len }) => {
                // Truncate back to object base, ready for next key
                // (e.g., "json.obj.key1" → "json.obj")
                path.truncate(*base_path_len);
            }
            None => {}
        }
    }

    while let Some(event) = parser.parse_next()? {
        match event {
            Event::StartObject => {
                writer.write_empty_object(&path)?;
                stack.push(ContextFrame::Object {
                    base_path_len: path.len(),
                });
            }
            Event::StartArray => {
                writer.write_empty_array(&path)?;
                let base_len = path.len();
                stack.push(ContextFrame::Array {
                    base_path_len: base_len,
                    index: 0,
                });
                path.push_str("[0]");
            }
            Event::Key(key) => {
                if let Some(frame) = stack.last() {
                    let base_len = match frame {
                        ContextFrame::Object { base_path_len } => *base_path_len,
                        ContextFrame::Array { base_path_len, .. } => *base_path_len,
                    };
                    path.truncate(base_len);
                }
                path.push('.');
                path.push_str(key);
            }
            Event::String(s) => {
                writer.write_string_value(&path, s)?;
                update_path_after_value(&mut stack, &mut path);
            }
            Event::Number(n) => {
                writer.write_raw_value(&path, n)?;
                update_path_after_value(&mut stack, &mut path);
            }
            Event::Boolean(b) => {
                writer.write_raw_value(&path, if b { "true" } else { "false" })?;
                update_path_after_value(&mut stack, &mut path);
            }
            Event::Null => {
                writer.write_raw_value(&path, "null")?;
                update_path_after_value(&mut stack, &mut path);
            }
            Event::EndObject { .. } | Event::EndArray { .. } => {
                stack.pop();
                update_path_after_value(&mut stack, &mut path);
            }
        }
    }

    Ok(())
}

trait GronWriteExt {
    /// Writes `{path} = {};`
    fn write_empty_object(&mut self, path: &str) -> std::io::Result<()>;

    /// Writes `{path} = [];`
    fn write_empty_array(&mut self, path: &str) -> std::io::Result<()>;

    /// Writes `{path} = "{value}";`
    fn write_string_value(&mut self, path: &str, value: &str) -> std::io::Result<()>;

    /// Write: `{path} = {value};` where `value` is a primitive value
    fn write_raw_value(&mut self, path: &str, value: &str) -> std::io::Result<()>;
}

impl<W: IoWrite> GronWriteExt for W {
    #[inline(always)]
    fn write_empty_object(&mut self, path: &str) -> std::io::Result<()> {
        self.write_all(path.as_bytes())?;
        self.write_all(b" = {};\n")
    }

    #[inline(always)]
    fn write_empty_array(&mut self, path: &str) -> std::io::Result<()> {
        self.write_all(path.as_bytes())?;
        self.write_all(b" = [];\n")
    }

    #[inline(always)]
    fn write_string_value(&mut self, path: &str, value: &str) -> std::io::Result<()> {
        self.write_all(path.as_bytes())?;
        self.write_all(b" = \"")?;
        self.write_all(value.as_bytes())?;
        self.write_all(b"\";\n")
    }

    #[inline(always)]
    fn write_raw_value(&mut self, path: &str, value: &str) -> std::io::Result<()> {
        self.write_all(path.as_bytes())?;
        self.write_all(b" = ")?;
        self.write_all(value.as_bytes())?;
        self.write_all(b";\n")
    }
}

#[cfg(test)]
mod old_impl {
    use std::fmt::{self, Display, Write};

    use serde_json::Value;

    /// This was our initial implementation of `jk flatten`. It's worth to keep it around since it's very likely correct,
    /// so it serves as a way of testing our faster-but-more-complicated implementation
    ///
    /// For reference, how both implementations compare:
    ///
    /// ```no_run
    /// % hyperfine 'jk flatten large.json' 'jk-initial flatten large.json'
    /// Benchmark 1: jk flatten large.json
    /// Time (mean ± σ):      1.276 s ±  0.148 s    [User: 1.152 s, System: 0.064 s]
    /// Range (min … max):    1.200 s …  1.672 s    10 runs
    ///
    /// Warning: The first benchmarking run for this command was significantly slower than the rest (1.672 s). This could be caused by (filesystem) caches that were not filled until after the first run. You should consider using the '--warmup' option to fill those caches before the actual benchmark. Alternatively, use the '--prepare' option to clear the caches before each timing run.
    ///
    /// Benchmark 2: jk-initial flatten large.json
    /// Time (mean ± σ):      5.748 s ±  0.519 s    [User: 3.676 s, System: 1.399 s]
    /// Range (min … max):    4.925 s …  6.279 s    10 runs
    ///
    /// Summary
    /// jk flatten large.json ran
    /// 4.50 ± 0.66 times faster than jk-initial flatten large.json
    /// ```
    ///
    /// In terms of memory, the new streaming flatten takes around 1MB of RAM, the older reached over 2GB. The test file in question was a 320MB JSON.
    ///
    /// Refs:
    /// jk: f98b24f8b6035f0e79f2e84988f21bd0a4212320
    /// jk-old: 7c990ce00ff384adf5ace3e219acde8c7877246a
    ///
    pub struct Flattened<'a> {
        pub val: &'a serde_json::Value,
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
}

#[cfg(test)]
mod tests_old_impl {
    use crate::flatten::old_impl::Flattened;

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

#[cfg(test)]
mod tests {
    use std::io::BufWriter;

    use super::*;

    fn flatten_to_string(input: &str) -> String {
        let mut output = Vec::new();
        let writer = BufWriter::new(&mut output);
        flatten(input, writer).unwrap();
        String::from_utf8(output).unwrap()
    }

    #[test]
    fn flatten_simple_object() {
        let json = r#"{"a": "b", "c": "d"}"#;
        let result = flatten_to_string(json);
        assert_eq!(result, "json = {};\njson.a = \"b\";\njson.c = \"d\";\n");
    }

    #[test]
    fn flatten_simple_array() {
        let json = r#"[1, 2, 3]"#;
        let result = flatten_to_string(json);
        assert_eq!(
            result,
            "json = [];\njson[0] = 1;\njson[1] = 2;\njson[2] = 3;\n"
        );
    }

    #[test]
    fn flatten_nested_object() {
        let json = r#"{"user": {"name": "John", "age": 30}}"#;
        let result = flatten_to_string(json);
        assert_eq!(
            result,
            "json = {};\njson.user = {};\njson.user.name = \"John\";\njson.user.age = 30;\n"
        );
    }

    #[test]
    fn flatten_object_with_array() {
        let json = r#"{"name": "John", "hobbies": ["reading", "coding"]}"#;
        let result = flatten_to_string(json);
        assert_eq!(
            result,
            "json = {};\njson.name = \"John\";\njson.hobbies = [];\njson.hobbies[0] = \"reading\";\njson.hobbies[1] = \"coding\";\n"
        );
    }

    #[test]
    fn flatten_array_of_objects() {
        let json = r#"[{"name": "John", "age": 30}, {"name": "Jane", "age": 25}]"#;
        let result = flatten_to_string(json);
        assert_eq!(
            result,
            "json = [];\njson[0] = {};\njson[0].name = \"John\";\njson[0].age = 30;\njson[1] = {};\njson[1].name = \"Jane\";\njson[1].age = 25;\n"
        );
    }

    #[test]
    fn flatten_complex_nested_structure() {
        let json = r#"[
            {
                "name": "John Doe",
                "age": 30,
                "hobbies": ["reading", "cycling"]
            },
            {
                "name": "Jane Doe",
                "age": 25,
                "hobbies": ["swimming", "dancing"]
            }
        ]"#;
        let result = flatten_to_string(json);
        assert_eq!(
            result,
            "json = [];\njson[0] = {};\njson[0].name = \"John Doe\";\njson[0].age = 30;\njson[0].hobbies = [];\njson[0].hobbies[0] = \"reading\";\njson[0].hobbies[1] = \"cycling\";\njson[1] = {};\njson[1].name = \"Jane Doe\";\njson[1].age = 25;\njson[1].hobbies = [];\njson[1].hobbies[0] = \"swimming\";\njson[1].hobbies[1] = \"dancing\";\n"
        );
    }

    #[test]
    fn flatten_all_value_types() {
        let json = r#"{"string": "hello", "number": 42, "bool": true, "null": null}"#;
        let result = flatten_to_string(json);
        assert_eq!(
            result,
            "json = {};\njson.string = \"hello\";\njson.number = 42;\njson.bool = true;\njson.null = null;\n"
        );
    }

    #[test]
    fn flatten_empty_object() {
        let json = r#"{}"#;
        let result = flatten_to_string(json);
        assert_eq!(result, "json = {};\n");
    }

    #[test]
    fn flatten_empty_array() {
        let json = r#"[]"#;
        let result = flatten_to_string(json);
        assert_eq!(result, "json = [];\n");
    }

    #[test]
    fn flatten_nested_arrays() {
        let json = r#"[[1, 2], [3, 4]]"#;
        let result = flatten_to_string(json);
        assert_eq!(
            result,
            "json = [];\njson[0] = [];\njson[0][0] = 1;\njson[0][1] = 2;\njson[1] = [];\njson[1][0] = 3;\njson[1][1] = 4;\n"
        );
    }

    #[test]
    fn flatten_deeply_nested() {
        let json = r#"{"a": {"b": {"c": {"d": "deep"}}}}"#;
        let result = flatten_to_string(json);
        assert_eq!(
            result,
            "json = {};\njson.a = {};\njson.a.b = {};\njson.a.b.c = {};\njson.a.b.c.d = \"deep\";\n"
        );
    }

    #[test]
    fn flatten_string_with_quotes() {
        let json = r#"{"key": "string with \"quotes\""}"#;
        let result = flatten_to_string(json);
        assert_eq!(
            result,
            "json = {};\njson.key = \"string with \\\"quotes\\\"\";\n"
        );
    }

    #[test]
    fn flatten_string_with_backslash() {
        let json = r#"{"key": "path\\to\\file"}"#;
        let result = flatten_to_string(json);
        assert_eq!(result, "json = {};\njson.key = \"path\\\\to\\\\file\";\n");
    }

    #[test]
    fn flatten_string_with_escapes() {
        let json = r#"{"key": "line\nbreak\ttab\rcarriage"}"#;
        let result = flatten_to_string(json);
        assert_eq!(
            result,
            "json = {};\njson.key = \"line\\nbreak\\ttab\\rcarriage\";\n"
        );
    }

    #[test]
    fn flatten_empty_string() {
        let json = r#"{"key": "", "": "empty_key"}"#;
        let result = flatten_to_string(json);
        assert_eq!(
            result,
            "json = {};\njson.key = \"\";\njson. = \"empty_key\";\n"
        );
    }

    #[test]
    fn flatten_keys_with_special_chars() {
        let json = r#"{"key.with.dots": "value1", "key with spaces": "value2", "key-with-dashes": "value3"}"#;
        let result = flatten_to_string(json);
        assert_eq!(
            result,
            "json = {};\njson.key.with.dots = \"value1\";\njson.key with spaces = \"value2\";\njson.key-with-dashes = \"value3\";\n"
        );
    }

    #[test]
    fn flatten_various_number_formats() {
        let json =
            r#"{"float": 3.14, "negative": -42, "scientific": 1.23e-10, "zero": 0, "large": 1e10}"#;
        let result = flatten_to_string(json);
        assert_eq!(
            result,
            "json = {};\njson.float = 3.14;\njson.negative = -42;\njson.scientific = 1.23e-10;\njson.zero = 0;\njson.large = 1e10;\n"
        );
    }

    #[test]
    fn flatten_mixed_type_array() {
        let json = r#"[1, "string", true, false, null, {"obj": "here"}, [1, 2]]"#;
        let result = flatten_to_string(json);
        assert_eq!(
            result,
            "json = [];\njson[0] = 1;\njson[1] = \"string\";\njson[2] = true;\njson[3] = false;\njson[4] = null;\njson[5] = {};\njson[5].obj = \"here\";\njson[6] = [];\njson[6][0] = 1;\njson[6][1] = 2;\n"
        );
    }

    #[test]
    fn flatten_unicode_strings() {
        let json = r#"{"emoji": "🎉", "chinese": "你好", "accents": "café"}"#;
        let result = flatten_to_string(json);
        assert_eq!(
            result,
            "json = {};\njson.emoji = \"🎉\";\njson.chinese = \"你好\";\njson.accents = \"café\";\n"
        );
    }

    #[test]
    fn flatten_whitespace_variations() {
        let json = r#"  {  "a"  :  "b"  ,  "c"  :  "d"  }  "#;
        let result = flatten_to_string(json);
        assert_eq!(result, "json = {};\njson.a = \"b\";\njson.c = \"d\";\n");

        let json = r#"{"a":"b","c":"d"}"#;
        let result = flatten_to_string(json);
        assert_eq!(result, "json = {};\njson.a = \"b\";\njson.c = \"d\";\n");
    }

    #[test]
    fn flatten_boolean_variations() {
        let json = r#"{"t": true, "f": false}"#;
        let result = flatten_to_string(json);
        assert_eq!(result, "json = {};\njson.t = true;\njson.f = false;\n");
    }

    #[test]
    fn flatten_array_with_nulls() {
        let json = r#"[null, null, "not null", null]"#;
        let result = flatten_to_string(json);
        assert_eq!(
            result,
            "json = [];\njson[0] = null;\njson[1] = null;\njson[2] = \"not null\";\njson[3] = null;\n"
        );
    }

    #[test]
    fn flatten_deeply_nested_arrays() {
        let json = r#"[[[1, 2], [3, 4]], [[5, 6], [7, 8]]]"#;
        let result = flatten_to_string(json);
        assert_eq!(
            result,
            "json = [];\njson[0] = [];\njson[0][0] = [];\njson[0][0][0] = 1;\njson[0][0][1] = 2;\njson[0][1] = [];\njson[0][1][0] = 3;\njson[0][1][1] = 4;\njson[1] = [];\njson[1][0] = [];\njson[1][0][0] = 5;\njson[1][0][1] = 6;\njson[1][1] = [];\njson[1][1][0] = 7;\njson[1][1][1] = 8;\n"
        );
    }

    #[test]
    fn flatten_single_value() {
        let json = r#""just a string""#;
        let result = flatten_to_string(json);
        assert_eq!(result, "json = \"just a string\";\n");

        let json = r#"42"#;
        let result = flatten_to_string(json);
        assert_eq!(result, "json = 42;\n");

        let json = r#"true"#;
        let result = flatten_to_string(json);
        assert_eq!(result, "json = true;\n");

        let json = r#"null"#;
        let result = flatten_to_string(json);
        assert_eq!(result, "json = null;\n");
    }
}
