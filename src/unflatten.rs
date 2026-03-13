use std::io::{BufWriter, Write, stdout};

use anyhow::Context;

use crate::borrowed_value::{Value, ValueEvents};
use crate::fmt::Formatter;
use crate::unflatten::{
    parser::Parser,
    types::{GronLine, GronValue, Index},
};

mod parser;
mod types;

pub fn unflatten(input: &str, use_colors: bool) -> anyhow::Result<()> {
    let value = unflatten_to_value(input)?;
    let mut writer = BufWriter::new(stdout().lock());

    if use_colors {
        Formatter::new_colored(ValueEvents::new(&value)).format_to(&mut writer)?;
    } else {
        Formatter::new_plain(ValueEvents::new(&value)).format_to(&mut writer)?;
    }

    writer.flush().with_context(|| "failed to flush output")?;

    Ok(())
}

pub fn unflatten_to_value<'a>(input: &'a str) -> anyhow::Result<Value<'a>> {
    let mut parser = Parser::new(input);
    let first_line = parser
        .parse_next_line()?
        .with_context(|| "The supplied file was empty!")?;

    let root_name = first_line
        .identifier
        .first()
        .with_context(|| "No identifier parsed")?
        .base;

    let mut root = match first_line.value {
        GronValue::Object => Value::object(),
        GronValue::Array => Value::array(),
        _other => return Ok(first_line.value.to_value()),
    };

    while let Some(GronLine { identifier, value }) = parser.parse_next_line()? {
        let components_amount = identifier.len();
        let components = identifier.iter().enumerate();

        let mut entry = &mut root;

        for (idx, component) in components {
            let is_last = idx == components_amount - 1;
            let is_root = component.base == root_name;

            // Handle an index at the root (e.g. json[0])
            if is_root {
                if component.indices.is_empty() {
                    continue;
                }

                let indices_to_navigate = if is_last {
                    &component.indices[..component.indices.len() - 1]
                } else {
                    &component.indices[..]
                };

                for index in indices_to_navigate {
                    entry = apply_single_index(entry, index)?;
                }

                if is_last {
                    let last_index = component
                        .indices
                        .last()
                        .with_context(|| "Expected at least one index")?;
                    set_at_last_index(entry, last_index, value.to_value())?;
                }
                continue;
            }

            // Last component of the path and no other index to apply means it's time to assign to the parsed value
            if is_last && component.indices.is_empty() {
                entry
                    .as_object_mut()
                    .with_context(|| "Expected object")?
                    .insert(component.base, value.to_value());
                break;
            }

            // if we're in an object, we need to navigate to current component's base
            entry = entry
                .as_object_mut()
                .with_context(|| "Expected object")?
                .get_mut(component.base)
                .with_context(|| format!("Path not found: {}", component.base))?;

            // Determine how many indices to navigate through
            let indices_to_navigate = if is_last {
                // For the last component, navigate through all but the last index
                &component.indices[..component.indices.len() - 1]
            } else {
                // For intermediate components, navigate through all indices
                &component.indices[..]
            };

            for index in indices_to_navigate {
                entry = apply_single_index(entry, index)?;
            }

            // Last component: set the value at the final index
            if is_last {
                let last_index = component
                    .indices
                    .last()
                    .with_context(|| "Expected at least one index")?;
                set_at_last_index(entry, last_index, value.to_value())?;
            }
        }
    }

    Ok(root)
}

/// Inserts `value` at the `last_index` pos
fn set_at_last_index<'a>(
    entry: &mut Value<'a>,
    last_index: &Index<'a>,
    value: Value<'a>,
) -> anyhow::Result<()> {
    match *last_index {
        Index::Numeric(idx) => {
            let arr = entry.as_array_mut().with_context(|| "Expected array")?;
            while arr.len() <= idx {
                arr.push(Value::Null);
            }
            arr[idx] = value;
        }
        Index::String(key) => {
            entry
                .as_object_mut()
                .with_context(|| "Expected object")?
                .insert(key, value);
        }
    }
    Ok(())
}

/// Follow a single index in order to navigate deeper into the tree.
fn apply_single_index<'data, 'borrow>(
    entry: &'borrow mut Value<'data>,
    index: &Index<'data>,
) -> anyhow::Result<&'borrow mut Value<'data>> {
    match *index {
        Index::Numeric(idx) => entry
            .as_array_mut()
            .with_context(|| "Expected array")?
            .get_mut(idx)
            .with_context(|| format!("Array index out of bounds: {idx}")),
        Index::String(key) => entry
            .as_object_mut()
            .with_context(|| "Expected object")?
            .get_mut(key)
            .with_context(|| format!("Object key not found: {key}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_unflatten_nested_arrays() {
        let input = r#"json = {};
json.hobbies = [];
json.hobbies[0] = [];
json.hobbies[0][0] = "reading";
json.hobbies[0][1] = "cycling";
json.hobbies[1] = [];
json.hobbies[1][0] = "swimming";
json.hobbies[1][1] = "dancing";"#;

        let result = unflatten_to_value(input);
        assert!(result.is_ok(), "Failed to unflatten: {:?}", result.err());

        let value = result.unwrap();
        let mut output = Vec::new();
        Formatter::new_plain(ValueEvents::new(&value))
            .format_to(&mut output)
            .unwrap();
        let json_str = String::from_utf8(output).unwrap();
        assert!(json_str.contains("hobbies"));
    }
}
