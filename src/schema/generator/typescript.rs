// TODO: needs some sort of structural equality checking
// TODO: it's very possible that this code can be generalized to generate
// muiltiple languages, through some sort of trait. What's a complicating factor here is
// that we need to handle the fact that different languages deal with `Union`s differently.

use heck::AsPascalCase;
use indexmap::IndexMap;
use singularize::singularize;

use crate::schema::SchemaType;

// ts reserved keywords
const RESERVED: &[&str] = &[
    "break",
    "case",
    "catch",
    "class",
    "const",
    "continue",
    "debugger",
    "default",
    "delete",
    "do",
    "else",
    "enum",
    "export",
    "extends",
    "false",
    "finally",
    "for",
    "function",
    "if",
    "import",
    "in",
    "instanceof",
    "new",
    "null",
    "return",
    "super",
    "switch",
    "this",
    "throw",
    "true",
    "try",
    "typeof",
    "var",
    "void",
    "while",
    "with",
    "as",
    "implements",
    "interface",
    "let",
    "package",
    "private",
    "protected",
    "public",
    "static",
    "yield",
    "any",
    "boolean",
    "constructor",
    "declare",
    "get",
    "module",
    "require",
    "number",
    "set",
    "string",
    "symbol",
    "type",
    "from",
    "of",
];

fn sanitize_field_name(name: &str) -> String {
    if name.is_empty() {
        return "\"\"".to_string();
    }

    let mut chars = name.chars();
    let first = chars.next().unwrap();
    let is_valid_start = first.is_alphabetic() || first == '_';
    let is_valid_rest = chars.all(|c| c.is_alphanumeric() || c == '_');

    let is_valid_identifier = is_valid_start && is_valid_rest;

    if is_valid_identifier && !RESERVED.contains(&name) {
        name.to_string()
    } else {
        format!("\"{}\"", name)
    }
}

/// What should we call this inner type?
///
/// - parent_name is the name of the parent type
/// - field_name is the inferred name of the field that this type is the value o
/// - is_array_item is true if this type is the value of an array item
///
/// Examples:
/// - {"users": [{...}]} -> parent_name="Root", field_name="users", is_array_item=true -> "User"
/// - {"user": {...}} -> "User"
/// - [{...}] -> "RootItem"
/// - {...} -> "Root"
fn get_target_type_name(
    parent_name: &str,
    field_name: Option<&str>,
    is_array_item: bool,
) -> String {
    match (field_name, is_array_item) {
        // Array of objects with a field name: {"users": [{...}]} -> "User"
        (Some(fname), true) => AsPascalCase(&singularize(fname)).to_string(),
        // Nested object field: {"user": {...}} -> "User"
        (Some(fname), false) => AsPascalCase(fname).to_string(),
        // Root-level array of objects: [{...}] -> "RootItem"
        (None, true) => format!("{}Item", parent_name),
        // Root-level object
        (None, false) => parent_name.to_string(),
    }
}

/// Collects all "inner" objects that need to become their own types
fn collect_types<'a>(
    schema: &'a SchemaType,
    parent_type_name: &str,
    field_name: Option<&str>,
    types: &mut IndexMap<String, &'a SchemaType>,
) {
    match schema {
        SchemaType::Object(fields) => {
            let current_type_name = if let Some(fname) = field_name {
                let name = get_target_type_name(parent_type_name, Some(fname), false);
                if !types.contains_key(&name) {
                    types.insert(name.clone(), schema);
                }
                name
            } else {
                // If we're here, the current type is the root object, and not added to `types`
                parent_type_name.to_string()
            };

            for (fname, field_schema) in fields {
                collect_types(&field_schema.type_, &current_type_name, Some(fname), types);
            }
        }
        SchemaType::Array(inner) => {
            if let SchemaType::Object(obj_fields) = &**inner {
                let item_name = get_target_type_name(parent_type_name, field_name, true);

                if !types.contains_key(&item_name) {
                    types.insert(item_name.clone(), inner);
                    for (fname, field_schema) in obj_fields {
                        collect_types(&field_schema.type_, &item_name, Some(fname), types);
                    }
                }
            } else {
                collect_types(inner, parent_type_name, field_name, types);
            }
        }
        SchemaType::Union(variants) => {
            for variant in variants {
                collect_types(variant, parent_type_name, field_name, types);
            }
        }
        _ => {}
    }
}

/// How do we write the given type in TypeScript?
fn generate_type_ref(
    schema: &SchemaType,
    parent_type_name: &str,
    field_name: Option<&str>,
) -> String {
    match schema {
        SchemaType::String => "string".to_string(),
        SchemaType::Number => "number".to_string(),
        SchemaType::Boolean => "boolean".to_string(),
        SchemaType::Null => "null".to_string(),
        SchemaType::Unknown => "unknown".to_string(),
        SchemaType::Object(_) => get_target_type_name(parent_type_name, field_name, false),
        SchemaType::Array(inner) => match &**inner {
            SchemaType::Object(_) => {
                let item_name = get_target_type_name(parent_type_name, field_name, true);
                format!("{}[]", item_name)
            }
            _ => format!(
                "{}[]",
                generate_type_ref(inner, parent_type_name, field_name)
            ),
        },
        SchemaType::Union(variants) => {
            let type_refs: Vec<String> = variants
                .iter()
                .map(|v| generate_type_ref(v, parent_type_name, field_name))
                .collect();
            format!("({})", type_refs.join(" | "))
        }
    }
}

fn generate_single_type(name: &str, schema: &SchemaType) -> String {
    match schema {
        SchemaType::Object(fields) => {
            if fields.is_empty() {
                return format!("export type {} = {{}};", name);
            }

            let mut field_defs = Vec::new();
            for (field_name, field_schema) in fields {
                let sanitized_name = sanitize_field_name(field_name);
                let optional = if field_schema.required { "" } else { "?" };
                let type_ref = generate_type_ref(&field_schema.type_, name, Some(field_name));
                field_defs.push(format!("  {}{}: {};", sanitized_name, optional, type_ref));
            }

            format!("export type {} = {{\n{}\n}};", name, field_defs.join("\n"))
        }
        _ => {
            let type_ref = generate_type_ref(schema, name, None);
            format!("export type {} = {};", name, type_ref)
        }
    }
}

pub fn generate(schema: &SchemaType) -> String {
    generate_with_name(schema, "Root")
}

pub fn generate_with_name(schema: &SchemaType, root_name: &str) -> String {
    let mut types = IndexMap::new();

    collect_types(schema, root_name, None, &mut types);

    let mut result: Vec<String> = types
        .iter()
        // Rev to display nested types first
        .rev()
        .map(|(name, schema)| generate_single_type(name, schema))
        .collect();

    result.push(generate_single_type(root_name, schema));

    result.join("\n\n")
}

#[cfg(test)]
mod tests {
    use indoc::indoc;

    use super::*;

    #[test]
    fn test_pascal_case() {
        assert_eq!(AsPascalCase("user_name").to_string(), "UserName");
        assert_eq!(AsPascalCase("first-name").to_string(), "FirstName");
        assert_eq!(AsPascalCase("firstName").to_string(), "FirstName");
        assert_eq!(AsPascalCase("API_KEY").to_string(), "ApiKey");
        assert_eq!(AsPascalCase("user").to_string(), "User");
        assert_eq!(AsPascalCase("_private").to_string(), "Private");
    }

    #[test]
    fn test_sanitize() {
        assert_eq!(sanitize_field_name("name"), "name");
        assert_eq!(sanitize_field_name("user_id"), "user_id");
        assert_eq!(sanitize_field_name("_private"), "_private");
        assert_eq!(sanitize_field_name("class"), "\"class\"");
        assert_eq!(sanitize_field_name("for"), "\"for\"");
        assert_eq!(sanitize_field_name("first-name"), "\"first-name\"");
        assert_eq!(sanitize_field_name("my field"), "\"my field\"");
        assert_eq!(sanitize_field_name("123abc"), "\"123abc\"");
        assert_eq!(sanitize_field_name("5g"), "\"5g\"");
        assert_eq!(sanitize_field_name(""), "\"\"");
    }

    #[test]
    fn test_simple_object() {
        use crate::schema::infer::infer_schema;

        let json = r#"{"name": "Alice", "age": 30}"#;
        let schema = infer_schema(json).unwrap();

        let result = generate(&schema);
        let expected = indoc! {r#"
            export type Root = {
              age: number;
              name: string;
            };"#
        };
        assert_eq!(result, expected);
    }

    #[test]
    fn test_optional_fields() {
        use crate::schema::infer::infer_schema_from_many;

        let json1 = r#"{"required_field": "test"}"#;
        let json2 = r#"{"required_field": "test", "optional_field": 42}"#;
        let schema = infer_schema_from_many(&[json1, json2]).unwrap();

        let result = generate(&schema);
        let expected = indoc! {r#"
            export type Root = {
              optional_field?: number;
              required_field: string;
            };"#
        };
        assert_eq!(result, expected);
    }

    #[test]
    fn test_nested_object() {
        use crate::schema::infer::infer_schema;

        let json = r#"{"user": {"name": "Alice", "active": true}}"#;
        let schema = infer_schema(json).unwrap();

        let result = generate(&schema);
        let expected = indoc! {r#"
            export type User = {
              active: boolean;
              name: string;
            };

            export type Root = {
              user: User;
            };"#
        };
        assert_eq!(result, expected);
    }

    #[test]
    fn test_array_of_primitives() {
        use crate::schema::infer::infer_schema;

        let json = r#"{"scores": [1, 2, 3, 4, 5]}"#;
        let schema = infer_schema(json).unwrap();

        let result = generate(&schema);
        let expected = indoc! {r#"
            export type Root = {
              scores: number[];
            };"#
        };
        assert_eq!(result, expected);
    }

    #[test]
    fn test_array_of_objects() {
        use crate::schema::infer::infer_schema;

        let json = r#"[{"id": 1, "name": "Alice"}, {"id": 2, "name": "Bob"}]"#;
        let schema = infer_schema(json).unwrap();

        let result = generate(&schema);
        let expected = indoc! {r#"
            export type RootItem = {
              id: number;
              name: string;
            };

            export type Root = RootItem[];"#
        };
        assert_eq!(result, expected);
    }

    #[test]
    fn test_union_types() {
        use crate::schema::infer::infer_schema_from_many;

        let json1 = r#"{"value": 42}"#;
        let json2 = r#"{"value": "text"}"#;
        let schema = infer_schema_from_many(&[json1, json2]).unwrap();

        let result = generate(&schema);
        // String comes before Number in enum ordering
        let expected = indoc! {r#"
            export type Root = {
              value: (string | number);
            };"#
        };
        assert_eq!(result, expected);
    }

    #[test]
    fn test_null_type() {
        use crate::schema::infer::infer_schema_from_many;

        let json1 = r#"{"nullable_field": "value"}"#;
        let json2 = r#"{"nullable_field": null}"#;
        let schema = infer_schema_from_many(&[json1, json2]).unwrap();

        let result = generate(&schema);
        let expected = indoc! {r#"
            export type Root = {
              nullable_field: (string | null);
            };"#
        };
        assert_eq!(result, expected);
    }

    #[test]
    fn test_unknown_type() {
        use crate::schema::infer::infer_schema;

        let json = r#"{"unknown_field": []}"#;
        let schema = infer_schema(json).unwrap();

        let result = generate(&schema);
        let expected = indoc! {r#"
            export type Root = {
              unknown_field: unknown[];
            };"#
        };
        assert_eq!(result, expected);
    }

    #[test]
    fn test_deeply_nested() {
        use crate::schema::infer::infer_schema;

        let json = r#"{"a": {"b": {"c": "deep"}}}"#;
        let schema = infer_schema(json).unwrap();

        let result = generate(&schema);
        let expected = indoc! {r#"
            export type B = {
              c: string;
            };

            export type A = {
              b: B;
            };

            export type Root = {
              a: A;
            };"#
        };
        assert_eq!(result, expected);
    }

    #[test]
    fn test_reserved_keyword_field() {
        use crate::schema::infer::infer_schema;

        let json = r#"{"class": "MyClass", "for": 123}"#;
        let schema = infer_schema(json).unwrap();

        let result = generate(&schema);
        let expected = indoc! {r#"
            export type Root = {
              "class": string;
              "for": number;
            };"#
        };
        assert_eq!(result, expected);
    }

    #[test]
    fn test_empty_object() {
        use crate::schema::infer::infer_schema;

        let json = r#"{}"#;
        let schema = infer_schema(json).unwrap();

        let result = generate(&schema);
        assert_eq!(result, "export type Root = {};");
    }

    #[test]
    fn test_primitive_at_root() {
        use crate::schema::infer::infer_schema;

        let json = r#""hello""#;
        let schema = infer_schema(json).unwrap();
        let result = generate(&schema);
        assert_eq!(result, "export type Root = string;");

        let json = r#"42"#;
        let schema = infer_schema(json).unwrap();
        let result = generate(&schema);
        assert_eq!(result, "export type Root = number;");
    }

    #[test]
    fn test_array_at_root() {
        use crate::schema::infer::infer_schema;

        let json = r#"["hello", "world"]"#;
        let schema = infer_schema(json).unwrap();
        let result = generate(&schema);
        assert_eq!(result, "export type Root = string[];");
    }

    #[test]
    fn test_union_at_root() {
        use crate::schema::infer::infer_schema_from_many;

        let json1 = r#"42"#;
        let json2 = r#""text""#;
        let schema = infer_schema_from_many(&[json1, json2]).unwrap();
        let result = generate(&schema);
        assert_eq!(result, "export type Root = (string | number);");
    }

    #[test]
    fn test_nested_arrays() {
        use crate::schema::infer::infer_schema;

        let json = r#"{"matrix": [[1, 2], [3, 4], [5, 6]]}"#;
        let schema = infer_schema(json).unwrap();

        let result = generate(&schema);
        let expected = indoc! {r#"
            export type Root = {
              matrix: number[][];
            };"#
        };
        assert_eq!(result, expected);
    }

    #[test]
    fn test_custom_root_name() {
        use crate::schema::infer::infer_schema;

        let json = r#"{"name": "Alice"}"#;
        let schema = infer_schema(json).unwrap();

        let result = generate_with_name(&schema, "User");
        let expected = indoc! {r#"
            export type User = {
              name: string;
            };"#
        };
        assert_eq!(result, expected);
    }

    #[test]
    fn test_real_world_nested_json() {
        use crate::schema::infer::infer_schema;

        let json = r#"{
  "name": "John Doe",
  "age": 30,
  "city": "New York",
  "hobbies": ["reading", "cycling"],
  "address": {
    "street": "123 Main St",
    "zip": "10001"
  }
}"#;

        let schema = infer_schema(json).unwrap();
        let result = generate(&schema);

        let expected = indoc! {r#"
            export type Address = {
              street: string;
              zip: string;
            };

            export type Root = {
              address: Address;
              age: number;
              city: string;
              hobbies: string[];
              name: string;
            };"#
        };
        assert_eq!(result, expected);
    }
}
