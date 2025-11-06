use std::io::BufWriter;

fn roundtrip(json_str: &str) -> anyhow::Result<()> {
    let mut flattened = Vec::new();
    {
        let writer = BufWriter::new(&mut flattened);
        jk::flatten::flatten(json_str, writer)?;
    }

    let flattened_str = std::str::from_utf8(&flattened)?;
    let unflattened = jk::unflatten::unflatten_to_value(flattened_str)?;

    let original: serde_json::Value = serde_json::from_str(json_str)?;
    let result_json = serde_json::to_value(&unflattened)?;

    assert_eq!(
        original,
        result_json,
        "Roundtrip failed!\nOriginal: {}\nResult: {}",
        serde_json::to_string_pretty(&original).unwrap(),
        serde_json::to_string_pretty(&result_json).unwrap()
    );

    Ok(())
}

#[test]
fn roundtrip_simple_object() {
    let json = r#"{"name": "John", "age": 30}"#;
    roundtrip(json).unwrap();
}

#[test]
fn roundtrip_simple_array() {
    let json = r#"[1, 2, 3, 4, 5]"#;
    roundtrip(json).unwrap();
}

#[test]
fn roundtrip_nested_object() {
    let json = r#"{
        "user": {
            "name": "Alice",
            "age": 25,
            "address": {
                "street": "123 Main St",
                "city": "Boston"
            }
        }
    }"#;
    roundtrip(json).unwrap();
}

#[test]
fn roundtrip_nested_arrays() {
    let json = r#"{
        "hobbies": [
            ["reading", "cycling"],
            ["swimming", "dancing"]
        ]
    }"#;
    roundtrip(json).unwrap();
}

#[test]
fn roundtrip_mixed_nesting() {
    let json = r#"{
        "users": [
            {
                "name": "Alice",
                "tags": ["admin", "user"]
            },
            {
                "name": "Bob",
                "tags": ["user"]
            }
        ]
    }"#;
    roundtrip(json).unwrap();
}

#[test]
fn roundtrip_all_value_types() {
    let json = r#"{
        "string": "hello",
        "number": 42,
        "float": 3.14,
        "bool_true": true,
        "bool_false": false,
        "null_value": null,
        "array": [1, 2, 3],
        "object": {"nested": "value"}
    }"#;
    roundtrip(json).unwrap();
}

#[test]
fn roundtrip_empty_containers() {
    let json = r#"{
        "empty_object": {},
        "empty_array": [],
        "nested_empty": {
            "obj": {},
            "arr": []
        }
    }"#;
    roundtrip(json).unwrap();
}

#[test]
fn roundtrip_deeply_nested() {
    let json = r#"{
        "a": {
            "b": {
                "c": {
                    "d": {
                        "e": "deep"
                    }
                }
            }
        }
    }"#;
    roundtrip(json).unwrap();
}

#[test]
fn roundtrip_deeply_nested_arrays() {
    let json = r#"[[[1, 2], [3, 4]], [[5, 6], [7, 8]]]"#;
    roundtrip(json).unwrap();
}

#[test]
#[ignore = "currently a bug: flatten not escaping special characters"]
fn roundtrip_special_characters_in_strings() {
    let json = r#"{
        "quotes": "He said \"hello\"",
        "backslash": "path\\to\\file",
        "newline": "line1\nline2",
        "tab": "col1\tcol2"
    }"#;
    roundtrip(json).unwrap();
}

#[test]
fn roundtrip_unicode() {
    let json = r#"{
        "emoji": "🎉 🚀",
        "chinese": "你好世界",
        "japanese": "こんにちは",
        "arabic": "مرحبا"
    }"#;
    roundtrip(json).unwrap();
}

#[test]
fn roundtrip_numbers() {
    let json = r#"{
        "integer": 42,
        "negative": -123,
        "float": 3.14159,
        "scientific": 1.23e10,
        "small": 1e-10,
        "zero": 0
    }"#;
    roundtrip(json).unwrap();
}

#[test]
fn roundtrip_sparse_array() {
    let json = r#"[1, null, null, 4, 5]"#;
    roundtrip(json).unwrap();
}

#[test]
fn roundtrip_array_of_objects() {
    let json = r#"[
        {"id": 1, "name": "Alice"},
        {"id": 2, "name": "Bob"},
        {"id": 3, "name": "Charlie"}
    ]"#;
    roundtrip(json).unwrap();
}

#[test]
fn roundtrip_object_with_array_values() {
    let json = r#"{
        "numbers": [1, 2, 3],
        "strings": ["a", "b", "c"],
        "mixed": [1, "two", true, null]
    }"#;
    roundtrip(json).unwrap();
}

#[test]
fn roundtrip_sample_nested_array() {
    let json = include_str!("../samples/nested-array.json");
    roundtrip(json).unwrap();
}

#[test]
fn roundtrip_sample_nested_object() {
    let json = include_str!("../samples/nested-object.json");
    roundtrip(json).unwrap();
}

#[test]
fn roundtrip_sample_data() {
    let json = include_str!("../samples/data.json");
    roundtrip(json).unwrap();
}

#[test]
#[ignore = "currently a bug: flatten not escaping"]
fn roundtrip_sample_twitter() {
    let json = include_str!("../samples/twitter.json");
    roundtrip(json).unwrap();
}

#[test]
fn roundtrip_sample_array() {
    let json = include_str!("../samples/array.json");
    roundtrip(json).unwrap();
}

#[test]
fn roundtrip_sample_null() {
    let json = include_str!("../samples/null.json");
    roundtrip(json).unwrap();
}

#[test]
fn roundtrip_scalar_values() {
    roundtrip(r#""just a string""#).unwrap();
    roundtrip(r#"42"#).unwrap();
    roundtrip(r#"true"#).unwrap();
    roundtrip(r#"false"#).unwrap();
    roundtrip(r#"null"#).unwrap();
    roundtrip(r#"3.14"#).unwrap();
}

#[test]
fn roundtrip_complex_structure() {
    let json = r#"{
        "metadata": {
            "version": "1.0",
            "timestamp": "2025-01-01T00:00:00Z"
        },
        "users": [
            {
                "id": 1,
                "profile": {
                    "name": "Alice",
                    "contacts": {
                        "emails": ["alice@example.com", "alice@work.com"],
                        "phones": ["+1234567890"]
                    }
                },
                "permissions": ["read", "write", "admin"]
            },
            {
                "id": 2,
                "profile": {
                    "name": "Bob",
                    "contacts": {
                        "emails": ["bob@example.com"],
                        "phones": []
                    }
                },
                "permissions": ["read"]
            }
        ],
        "settings": {
            "notifications": {
                "email": true,
                "sms": false,
                "push": true
            },
            "privacy": {
                "public_profile": false,
                "show_email": false
            }
        }
    }"#;
    roundtrip(json).unwrap();
}

#[test]
fn roundtrip_triple_nested_arrays() {
    let json = r#"[[[1]]]"#;
    roundtrip(json).unwrap();
}

#[test]
fn roundtrip_mixed_deeply_nested() {
    let json = r#"{
        "level1": {
            "level2": [
                {
                    "level3": {
                        "level4": [
                            [1, 2, 3]
                        ]
                    }
                }
            ]
        }
    }"#;
    roundtrip(json).unwrap();
}
