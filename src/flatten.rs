use serde_json::Value;
use std::fmt::Write;

// TODO: implement other formats. Only `gron` is supported so far.
#[allow(dead_code)]
pub enum FlattenFormat {
    Gron,
    Pointer,
}

/// Displays a flattened version of a JSON object
pub fn flatten(value: serde_json::Value) {
    let mut path = String::with_capacity(256);
    path.push_str("json");
    flatten_with_path(&mut path, &value);
}

fn flatten_with_path(path: &mut String, value: &Value) {
    match value {
        Value::Object(map) => {
            println!("{} = {{}};", path);

            for (key, val) in map {
                let len = path.len();
                path.push('.');
                path.push_str(key);

                flatten_with_path(path, val);

                path.truncate(len);
            }
        }
        Value::Array(arr) => {
            println!("{} = [];", path);

            for (idx, val) in arr.iter().enumerate() {
                let len = path.len();
                write!(path, "[{}]", idx).unwrap();

                flatten_with_path(path, val);

                path.truncate(len);
            }
        }
        Value::String(s) => {
            // TODO: replace with some Display impl that perofrms the escaping
            let escaped = s.replace('\\', "\\\\").replace('"', "\\\"");
            println!("{} = \"{}\";", path, escaped);
        }
        Value::Number(n) => {
            println!("{} = {};", path, n);
        }
        Value::Bool(b) => {
            println!("{} = {};", path, b);
        }
        Value::Null => {
            println!("{} = null;", path);
        }
    }
}
