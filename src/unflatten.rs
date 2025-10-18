use std::ops::Not;

use anyhow::Context;
use serde_json::{Map, Number, Value};

use crate::unflatten::{
    parser::parse_gron_line,
    types::{GronLine, GronValue, Index},
};

mod parser;
mod types;

type Result<T = ()> = std::result::Result<T, Error>;

#[derive(Debug)]
pub struct Error(String);

impl From<nom::Err<nom::error::Error<&str>>> for Error {
    fn from(value: nom::Err<nom::error::Error<&str>>) -> Self {
        Error(value.to_string())
    }
}

impl From<anyhow::Error> for Error {
    fn from(value: anyhow::Error) -> Self {
        Error(value.to_string())
    }
}

pub fn unflatten<'a>(mut input: &'a str) -> Result {
    let mut root;
    let (rest, first_line) = parse_gron_line(input)?;
    input = rest;
    let root_name = first_line
        .identifier
        .first()
        .with_context(|| "No identifier parsed")?
        .base;

    match first_line.value {
        GronValue::Object => root = Value::Object(Map::new()),
        GronValue::Array => root = Value::Array(Vec::new()),
        GronValue::String(val) => {
            println!("\"{val}\"");
            return Ok(());
        }
        GronValue::Number(num) => {
            println!("{num}");
            return Ok(());
        }
        GronValue::Boolean(boolean) => {
            println!("{boolean}");
            return Ok(());
        }
        GronValue::Null => {
            println!("null");
            return Ok(());
        }
    }

    while input.is_empty().not() {
        let (rest, GronLine { identifier, value }) = parse_gron_line(input)?;
        input = rest;

        // TODO: validate root name
        let components_amount = identifier.len();
        let components = identifier.into_iter().enumerate();

        let mut entry = &mut root;

        for (idx, component) in components {
            if idx == components_amount - 1 {
                match component.index {
                    Some(index) => {
                        if component.base != root_name {
                            entry = entry
                                .as_object_mut()
                                .unwrap()
                                .get_mut(component.base)
                                .unwrap();
                        }

                        match index {
                            Index::Numeric(idx) => {
                                let idx: usize = idx.parse().unwrap();
                                // TODO: validate idx against length of array?
                                let arr = entry.as_array_mut().unwrap();
                                while arr.len() <= idx {
                                    arr.push(Value::Null); // Fill gaps with null
                                }
                                arr[idx] = value.to_serde();
                            }
                            Index::String(key) => {
                                entry
                                    .as_object_mut()
                                    .unwrap()
                                    .insert(key.to_string(), value.to_serde());
                            }
                        }
                    }
                    None => {
                        let obj = entry.as_object_mut().unwrap();
                        obj.insert(component.base.to_string(), value.to_serde());
                    }
                }
            } else {
                if component.base != root_name {
                    entry = navigate_to(entry, component.base, component.index)?;
                }
            }
        }
    }

    let res = serde_json::to_string_pretty(&root).unwrap();
    println!("{res}");

    Ok(())
}

fn navigate_to<'a>(
    entry: &'a mut Value,
    base: &str,
    index: Option<Index<'_>>,
) -> Result<&'a mut Value> {
    // First navigate to base (object key lookup)
    let entry = entry
        .as_object_mut()
        .with_context(|| "Expected object")?
        .get_mut(base)
        .with_context(|| format!("Path not found: {}", base))?;

    apply_index(entry, index)
}

fn apply_index<'a>(entry: &'a mut Value, index: Option<Index<'_>>) -> Result<&'a mut Value> {
    match index {
        Some(Index::Numeric(idx)) => {
            let idx: usize = idx
                .parse()
                .with_context(|| format!("Invalid array index: {}", idx))?;
            entry
                .as_array_mut()
                .with_context(|| "Expected array")?
                .get_mut(idx)
                .with_context(|| format!("Array index out of bounds: {}", idx))
                .map_err(Into::into)
        }
        Some(Index::String(key)) => entry
            .as_object_mut()
            .with_context(|| "Expected object")?
            .get_mut(key)
            .with_context(|| format!("Object key not found: {}", key))
            .map_err(Into::into),
        None => Ok(entry),
    }
}
