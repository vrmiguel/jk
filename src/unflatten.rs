use serde_json::{Map, Value};

use crate::unflatten::types::GronValue;

mod parser;
mod types;

pub fn unflatten<'a>(input: &'a str) {
    let mut root = Value::Null;

    let root_value = GronValue::Array;

    match root_value {
        GronValue::Array => {
            root = Value::Array(Vec::new());
        }
        GronValue::Object => {
            root = Value::Object(Map::new());
        }
        GronValue::Number(val) | GronValue::String(val) => {
            println!("\"{val}\"");
            return;
        }
        GronValue::Boolean(val) => {
            println!("{val}");
            return;
        }
        GronValue::Null => {
            println!("null");
            return;
        }
    }
}
