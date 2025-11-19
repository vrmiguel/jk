use std::collections::{BTreeMap, BTreeSet};

use crate::schema::{FieldSchema, SchemaType};

/// A container (object or array) that is currently being built
enum PartialType {
    Object {
        fields: BTreeMap<String, FieldSchema>,
        current_key: Option<String>,
    },
    Array {
        /// None if empty array
        elements: Option<SchemaType>,
    },
}

impl SchemaType {
    /// Merge schema `other` into `self`.
    ///
    /// The function takes ownership of `self` and mutates it, but `self` is then
    /// returned after it's done.
    pub fn merge(self, other: SchemaType) -> Self {
        match (self, other) {
            // If both are the same primitive types, nothing to do
            (s @ SchemaType::String, SchemaType::String)
            | (s @ SchemaType::Number, SchemaType::Number)
            | (s @ SchemaType::Boolean, SchemaType::Boolean)
            | (s @ SchemaType::Null, SchemaType::Null)
            | (s @ SchemaType::Unknown, SchemaType::Unknown) => s,

            // Both are objects: must be merged into one
            (SchemaType::Object(mut self_map), SchemaType::Object(other_map)) => {
                // If there are any fields in `self` that are not in `other`: we must mark those as optional
                for (key, self_field) in self_map.iter_mut() {
                    if !other_map.contains_key(key) {
                        self_field.required = false;
                    }
                }

                // Now similarly, we check if there's anything in `other` that is not in `self``
                for (key, other_field) in other_map {
                    // Is this field in `other` found here in `self`?
                    if let Some(mut self_field) = self_map.remove(&key) {
                        self_field.type_ = self_field.type_.merge(other_field.type_);

                        self_field.required = self_field.required && other_field.required;
                        self_map.insert(key, self_field);
                    } else {
                        // found only in other, mark as optional
                        self_map.insert(
                            key,
                            FieldSchema {
                                type_: other_field.type_,
                                required: false,
                            },
                        );
                    }
                }

                SchemaType::Object(self_map)
            }

            (SchemaType::Array(self_inner), SchemaType::Array(other_inner)) => {
                let merged = (*self_inner).merge(*other_inner);
                SchemaType::Array(Box::new(merged))
            }

            // Both are unions: just join both sets
            (SchemaType::Union(mut self_set), SchemaType::Union(other_set)) => {
                self_set.extend(other_set);
                SchemaType::Union(self_set)
            }

            (SchemaType::Union(mut set), other) | (other, SchemaType::Union(mut set))
                if !matches!(other, SchemaType::Union(_)) =>
            {
                set.insert(other);
                SchemaType::Union(set)
            }

            (self_type, other_type) => {
                let mut set = BTreeSet::new();
                set.insert(self_type);
                set.insert(other_type);
                SchemaType::Union(set)
            }
        }
    }

    /// Merge multiple types into a single type
    /// This is useful for arrays that contain different types
    pub fn merge_into_union(types: impl IntoIterator<Item = SchemaType>) -> SchemaType {
        let mut types_iter = types.into_iter();

        if let Some(first) = types_iter.next() {
            let mut result = first;
            for next_type in types_iter {
                result = result.merge(next_type);
            }
            result
        } else {
            // Empty array
            SchemaType::Unknown
        }
    }
}

pub fn infer_schema(text: &str) -> Result<SchemaType, jsax::Error> {
    let mut parser = jsax::Parser::new(text);
    let mut stack: Vec<PartialType> = Vec::new();

    while let Some(event) = parser.parse_next()? {
        match event {
            jsax::Event::StartObject => {
                stack.push(PartialType::Object {
                    fields: BTreeMap::new(),
                    current_key: None,
                });
            }

            jsax::Event::EndObject { .. } => {
                let frame = stack.pop().expect("stack underflow on EndObject");
                let completed = match frame {
                    PartialType::Object { fields, .. } => SchemaType::Object(fields),
                    _ => panic!("expected Object on EndObject"),
                };

                if stack.is_empty() {
                    return Ok(completed);
                }

                add_schema_to_parent(&mut stack, completed);
            }

            jsax::Event::StartArray => {
                stack.push(PartialType::Array { elements: None });
            }

            jsax::Event::EndArray { .. } => {
                let frame = stack.pop().expect("stack underflow on EndArray");
                let array_schema = match frame {
                    PartialType::Array { elements } => {
                        SchemaType::Array(Box::new(elements.unwrap_or(SchemaType::Unknown)))
                    }
                    _ => panic!("expected Array on EndArray"),
                };

                if stack.is_empty() {
                    return Ok(array_schema);
                }

                add_schema_to_parent(&mut stack, array_schema);
            }

            jsax::Event::Key(key) => {
                if let Some(PartialType::Object { current_key, .. }) = stack.last_mut() {
                    *current_key = Some(key.to_string());
                }
            }

            jsax::Event::Null => {
                let schema = SchemaType::Null;
                if stack.is_empty() {
                    return Ok(schema);
                }
                add_schema_to_parent(&mut stack, schema);
            }

            jsax::Event::Boolean(_) => {
                let schema = SchemaType::Boolean;
                if stack.is_empty() {
                    return Ok(schema);
                }
                add_schema_to_parent(&mut stack, schema);
            }

            jsax::Event::Number(_) => {
                let schema = SchemaType::Number;
                if stack.is_empty() {
                    return Ok(schema);
                }
                add_schema_to_parent(&mut stack, schema);
            }

            jsax::Event::String(_) => {
                let schema = SchemaType::String;
                if stack.is_empty() {
                    return Ok(schema);
                }
                add_schema_to_parent(&mut stack, schema);
            }
        }
    }

    Err(jsax::Error::Unexpected(
        "empty or incomplete JSON".to_string(),
    ))
}

fn add_schema_to_parent(stack: &mut [PartialType], schema: SchemaType) {
    let parent = stack.last_mut().expect("parent container missing");

    match parent {
        PartialType::Object {
            fields,
            current_key,
        } => {
            let key = current_key.take().expect("key missing for object entry");
            fields.insert(
                key,
                FieldSchema {
                    type_: schema,
                    required: true,
                },
            );
        }
        PartialType::Array { elements } => {
            *elements = Some(match elements.take() {
                None => schema,
                Some(existing) => existing.merge(schema),
            });
        }
    }
}

pub fn infer_schema_from_many(texts: &[&str]) -> Result<SchemaType, jsax::Error> {
    let mut schemas = Vec::new();

    for text in texts {
        schemas.push(infer_schema(text)?);
    }

    if schemas.is_empty() {
        return Err(jsax::Error::Unexpected("no documents provided".to_string()));
    }

    Ok(SchemaType::merge_into_union(schemas))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_object() {
        let json = r#"{"name": "Alice", "age": 30}"#;
        let schema = infer_schema(json).unwrap();

        let expected = SchemaType::Object(BTreeMap::from([
            (
                "age".to_string(),
                FieldSchema {
                    type_: SchemaType::Number,
                    required: true,
                },
            ),
            (
                "name".to_string(),
                FieldSchema {
                    type_: SchemaType::String,
                    required: true,
                },
            ),
        ]));

        assert_eq!(schema, expected);
    }

    #[test]
    fn test_nested_object() {
        let json = r#"{"user": {"name": "Bob", "active": true}}"#;
        let schema = infer_schema(json).unwrap();

        let expected = SchemaType::Object(BTreeMap::from([(
            "user".to_string(),
            FieldSchema {
                type_: SchemaType::Object(BTreeMap::from([
                    (
                        "active".to_string(),
                        FieldSchema {
                            type_: SchemaType::Boolean,
                            required: true,
                        },
                    ),
                    (
                        "name".to_string(),
                        FieldSchema {
                            type_: SchemaType::String,
                            required: true,
                        },
                    ),
                ])),
                required: true,
            },
        )]));

        assert_eq!(schema, expected);
    }

    #[test]
    fn test_array_with_homogeneous_types() {
        let json = r#"{"scores": [1, 2, 3]}"#;
        let schema = infer_schema(json).unwrap();

        let expected = SchemaType::Object(BTreeMap::from([(
            "scores".to_string(),
            FieldSchema {
                type_: SchemaType::Array(Box::new(SchemaType::Number)),
                required: true,
            },
        )]));

        assert_eq!(schema, expected);
    }

    #[test]
    fn test_array_mixed_types() {
        let json = r#"{"items": [1, "two", 3]}"#;
        let schema = infer_schema(json).unwrap();

        let expected = SchemaType::Object(BTreeMap::from([(
            "items".to_string(),
            FieldSchema {
                type_: SchemaType::Array(Box::new(SchemaType::Union(BTreeSet::from([
                    SchemaType::Number,
                    SchemaType::String,
                ])))),
                required: true,
            },
        )]));

        assert_eq!(schema, expected);
    }

    #[test]
    fn test_empty_array() {
        let json = r#"{"items": []}"#;
        let schema = infer_schema(json).unwrap();

        let expected = SchemaType::Object(BTreeMap::from([(
            "items".to_string(),
            FieldSchema {
                type_: SchemaType::Array(Box::new(SchemaType::Unknown)),
                required: true,
            },
        )]));

        assert_eq!(schema, expected);
    }

    #[test]
    fn test_optional_fields_from_multiple_documents() {
        let json1 = r#"{"id": 1, "name": "Alice"}"#;
        let json2 = r#"{"id": 2, "email": "bob@example.com"}"#;

        let schema = infer_schema_from_many(&[json1, json2]).unwrap();

        let expected = SchemaType::Object(BTreeMap::from([
            (
                "email".to_string(),
                FieldSchema {
                    type_: SchemaType::String,
                    required: false,
                },
            ),
            (
                "id".to_string(),
                FieldSchema {
                    type_: SchemaType::Number,
                    required: true,
                },
            ),
            (
                "name".to_string(),
                FieldSchema {
                    type_: SchemaType::String,
                    required: false,
                },
            ),
        ]));

        assert_eq!(schema, expected);
    }

    #[test]
    fn test_primitive_types() {
        assert_eq!(infer_schema("null").unwrap(), SchemaType::Null);
        assert_eq!(infer_schema("true").unwrap(), SchemaType::Boolean);
        assert_eq!(infer_schema("false").unwrap(), SchemaType::Boolean);
        assert_eq!(infer_schema("42").unwrap(), SchemaType::Number);
        assert_eq!(infer_schema("3.14").unwrap(), SchemaType::Number);
        assert_eq!(infer_schema(r#""hello""#).unwrap(), SchemaType::String);
    }

    #[test]
    fn test_null_in_object_field() {
        let json1 = r#"{"value": "test"}"#;
        let json2 = r#"{"value": null}"#;

        let schema = infer_schema_from_many(&[json1, json2]).unwrap();

        let expected = SchemaType::Object(BTreeMap::from([(
            "value".to_string(),
            FieldSchema {
                type_: SchemaType::Union(BTreeSet::from([SchemaType::Null, SchemaType::String])),
                required: true,
            },
        )]));

        assert_eq!(schema, expected);
    }

    #[test]
    fn test_array_of_objects() {
        let json = r#"[{"name": "Alice", "age": 30}, {"name": "Bob", "age": 25}]"#;
        let schema = infer_schema(json).unwrap();

        let expected = SchemaType::Array(Box::new(SchemaType::Object(BTreeMap::from([
            (
                "age".to_string(),
                FieldSchema {
                    type_: SchemaType::Number,
                    required: true,
                },
            ),
            (
                "name".to_string(),
                FieldSchema {
                    type_: SchemaType::String,
                    required: true,
                },
            ),
        ]))));

        assert_eq!(schema, expected);
    }

    #[test]
    fn test_array_of_objects_with_optional_fields() {
        let json = r#"[{"id": 1, "name": "Alice"}, {"id": 2, "email": "bob@example.com"}]"#;
        let schema = infer_schema(json).unwrap();

        let expected = SchemaType::Array(Box::new(SchemaType::Object(BTreeMap::from([
            (
                "email".to_string(),
                FieldSchema {
                    type_: SchemaType::String,
                    required: false,
                },
            ),
            (
                "id".to_string(),
                FieldSchema {
                    type_: SchemaType::Number,
                    required: true,
                },
            ),
            (
                "name".to_string(),
                FieldSchema {
                    type_: SchemaType::String,
                    required: false,
                },
            ),
        ]))));

        assert_eq!(schema, expected);
    }

    #[test]
    fn test_nested() {
        let json = r#"{"a": {"b": {"c": {"d": "deep"}}}}"#;
        let schema = infer_schema(json).unwrap();

        let expected = SchemaType::Object(BTreeMap::from([(
            "a".to_string(),
            FieldSchema {
                type_: SchemaType::Object(BTreeMap::from([(
                    "b".to_string(),
                    FieldSchema {
                        type_: SchemaType::Object(BTreeMap::from([(
                            "c".to_string(),
                            FieldSchema {
                                type_: SchemaType::Object(BTreeMap::from([(
                                    "d".to_string(),
                                    FieldSchema {
                                        type_: SchemaType::String,
                                        required: true,
                                    },
                                )])),
                                required: true,
                            },
                        )])),
                        required: true,
                    },
                )])),
                required: true,
            },
        )]));

        assert_eq!(schema, expected);
    }

    #[test]
    fn test_nested_array() {
        let json = r#"{"matrix": [[1, 2], [3, 4], [5, 6]]}"#;
        let schema = infer_schema(json).unwrap();

        let expected = SchemaType::Object(BTreeMap::from([(
            "matrix".to_string(),
            FieldSchema {
                type_: SchemaType::Array(Box::new(SchemaType::Array(Box::new(SchemaType::Number)))),
                required: true,
            },
        )]));

        assert_eq!(schema, expected);
    }

    #[test]
    fn test_merge_two_values_same_structure() {
        let json1 = r#"{"name": "Alice", "age": 30}"#;
        let json2 = r#"{"name": "Bob", "age": 25}"#;

        let schema = infer_schema_from_many(&[json1, json2]).unwrap();

        let expected = SchemaType::Object(BTreeMap::from([
            (
                "age".to_string(),
                FieldSchema {
                    type_: SchemaType::Number,
                    required: true,
                },
            ),
            (
                "name".to_string(),
                FieldSchema {
                    type_: SchemaType::String,
                    required: true,
                },
            ),
        ]));

        assert_eq!(schema, expected);
    }

    #[test]
    fn test_merge_two_values_different_structure() {
        let json1 = r#"{"value": 42}"#;
        let json2 = r#"{"value": "text"}"#;

        let schema = infer_schema_from_many(&[json1, json2]).unwrap();

        let expected = SchemaType::Object(BTreeMap::from([(
            "value".to_string(),
            FieldSchema {
                type_: SchemaType::Union(BTreeSet::from([SchemaType::Number, SchemaType::String])),
                required: true,
            },
        )]));

        assert_eq!(schema, expected);
    }

    #[test]
    fn infer_for_twitter_json() {
        let twitter_json = include_str!("../../samples/twitter.json");
        infer_schema(twitter_json).unwrap(); 
        // Some way of asserting the gigantic resulting schema. So far, this test only really asserts that twitter.json doesn't cause it to panic :v
    }
}
