use std::collections::BTreeSet;

use crate::schema::SchemaType;
use crate::schema::generator::{Language, generate_with_language};

pub struct TypeScript;

impl Language for TypeScript {
    fn can_inline_unions(&self) -> bool {
        true
    }

    fn name_union(&self, _parent_type_name: &str, _variants: &BTreeSet<SchemaType>) -> String {
        panic!("TypeScript can inline unions, name_union should not be called")
    }

    fn sanitize_field_name(&self, name: &str) -> String {
        if name.is_empty() {
            return "\"\"".to_string();
        }

        let mut chars = name.chars();
        let first = chars.next().unwrap();
        let is_valid_start = first.is_alphabetic() || first == '_' || first == '$';
        let is_valid_rest = chars.all(|c| c.is_alphanumeric() || c == '_' || c == '$');

        let is_valid_identifier = is_valid_start && is_valid_rest;

        if is_valid_identifier {
            name.to_string()
        } else {
            format!("\"{}\"", name)
        }
    }

    fn primitive_type(&self, schema: &SchemaType) -> Option<&str> {
        match schema {
            SchemaType::String => Some("string"),
            SchemaType::Number => Some("number"),
            SchemaType::Boolean => Some("boolean"),
            SchemaType::Null => Some("null"),
            SchemaType::Unknown => Some("unknown"),
            _ => None,
        }
    }

    fn array_type(&self, inner: &str) -> String {
        format!("{}[]", inner)
    }

    fn union_type(&self, variants: &[String]) -> String {
        format!("({})", variants.join(" | "))
    }

    fn object_type_declaration(
        &self,
        name: &str,
        fields: &std::collections::BTreeMap<String, crate::schema::FieldSchema>,
    ) -> String {
        if fields.is_empty() {
            return format!("export type {} = {{}};", name);
        }

        let mut field_defs = Vec::new();
        for (field_name, field_schema) in fields {
            let sanitized_name = self.sanitize_field_name(field_name);
            let optional = if field_schema.required { "" } else { "?" };
            let type_ref = crate::schema::generator::generate_type_ref(
                &field_schema.type_,
                name,
                Some(field_name),
                self,
            );
            field_defs.push(format!("  {}{}: {};", sanitized_name, optional, type_ref));
        }

        format!("export type {} = {{\n{}\n}};", name, field_defs.join("\n"))
    }

    fn type_alias_declaration(&self, name: &str, type_ref: &str) -> String {
        format!("export type {} = {};", name, type_ref)
    }

    fn union_type_declaration(
        &self,
        _name: &str,
        _variants: &std::collections::BTreeSet<crate::schema::SchemaType>,
    ) -> String {
        panic!("TypeScript can inline unions, union_type_declaration should not be called")
    }
}

pub fn generate(schema: &SchemaType) -> String {
    generate_with_name(schema, "Root")
}

pub fn generate_with_name(schema: &SchemaType, root_name: &str) -> String {
    generate_with_language(schema, root_name, &TypeScript)
}

#[cfg(test)]
mod tests {
    use heck::AsPascalCase;
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
        let ts = TypeScript;
        assert_eq!(ts.sanitize_field_name("name"), "name");
        assert_eq!(ts.sanitize_field_name("user_id"), "user_id");
        assert_eq!(ts.sanitize_field_name("_private"), "_private");
        assert_eq!(ts.sanitize_field_name("$jquery"), "$jquery");

        // keywords are valid as property names in TS
        assert_eq!(ts.sanitize_field_name("class"), "class");
        assert_eq!(ts.sanitize_field_name("for"), "for");
        assert_eq!(ts.sanitize_field_name("type"), "type");
        assert_eq!(ts.sanitize_field_name("instanceof"), "instanceof");

        // Need quotes
        assert_eq!(ts.sanitize_field_name("first-name"), "\"first-name\"");
        assert_eq!(ts.sanitize_field_name("my field"), "\"my field\"");
        assert_eq!(ts.sanitize_field_name("123abc"), "\"123abc\"");
        assert_eq!(ts.sanitize_field_name("5g"), "\"5g\"");
        assert_eq!(ts.sanitize_field_name(""), "\"\"");
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
              class: string;
              for: number;
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

    #[test]
    fn test_merge_same_field_name_different_structures() {
        use crate::schema::infer::infer_schema;

        let json = r#"{
            "a": {
                "credit_card": {
                    "number": 5502
                }
            },
            "b": {
                "credit_card": {
                    "number": 5503,
                    "owes": true
                }
            }
        }"#;

        let schema = infer_schema(json).unwrap();
        let result = generate(&schema);

        let expected = indoc! {r#"
            export type CreditCard = {
              number: number;
              owes?: boolean;
            };

            export type B = {
              credit_card: CreditCard;
            };

            export type A = {
              credit_card: CreditCard;
            };

            export type Root = {
              a: A;
              b: B;
            };"#
        };

        assert_eq!(result, expected);
    }
}
