use std::io::{BufWriter, Write, stdout};

use anyhow::Context;

use crate::borrowed_value::Value;
use crate::unflatten::{
    parser::Parser,
    types::{GronLine, GronValue, Index},
};

mod lexer;
mod parser;
mod types;

pub fn unflatten(input: &str) -> anyhow::Result<()> {
    let value = unflatten_to_value(input)?;
    let mut writer = BufWriter::new(stdout().lock());
    serde_json::to_writer_pretty(&mut writer, &value)
        .with_context(|| "failed to print out JSON")?;
    writer.flush().with_context(|| "failed to flush output")?;

    Ok(())
}

pub fn unflatten_to_value<'a>(input: &'a str) -> anyhow::Result<Value<'a>> {
    let mut parser = Parser::new(input);
    let mut root;
    let first_line = parser
        .parse_next_line()?
        .with_context(|| "The supplied file was empty!")?;

    let root_name = first_line
        .identifier
        .first()
        .with_context(|| "No identifier parsed")?
        .base;

    match first_line.value {
        GronValue::Object => root = Value::object(),
        GronValue::Array => root = Value::array(),
        _other => {
            // For scalar root values, just return them directly
            return Ok(first_line.value.to_value());
        }
    }

    while let Some(GronLine { identifier, value }) = parser.parse_next_line()? {
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
                                arr[idx] = value.to_value();
                            }
                            Index::String(key) => {
                                entry.as_object_mut().unwrap().insert(key, value.to_value());
                            }
                        }
                    }
                    None => {
                        let obj = entry.as_object_mut().unwrap();
                        obj.insert(component.base, value.to_value());
                    }
                }
            } else {
                if component.base != root_name {
                    entry = navigate_to(entry, component.base, component.index)?;
                }
            }
        }
    }

    Ok(root)
}

fn navigate_to<'data, 'borrow>(
    entry: &'borrow mut Value<'data>,
    base: &str,
    index: Option<Index<'_>>,
) -> anyhow::Result<&'borrow mut Value<'data>> {
    // First navigate to base (object key lookup)
    let entry = entry
        .as_object_mut()
        .with_context(|| "Expected object")?
        .get_mut(base)
        .with_context(|| format!("Path not found: {}", base))?;

    apply_index(entry, index)
}

fn apply_index<'data, 'borrow>(
    entry: &'borrow mut Value<'data>,
    index: Option<Index<'_>>,
) -> anyhow::Result<&'borrow mut Value<'data>> {
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
        }
        Some(Index::String(key)) => entry
            .as_object_mut()
            .with_context(|| "Expected object")?
            .get_mut(key)
            .with_context(|| format!("Object key not found: {}", key)),
        None => Ok(entry),
    }
}
