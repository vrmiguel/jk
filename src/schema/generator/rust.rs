// TODOs:
// - Integer numbers (currently always f64)
// - Allow customizing what traits are derived

use std::collections::BTreeSet;

use crate::schema::generator::Language;
use crate::schema::{FieldSchema, SchemaType};

pub struct Rust;

impl Language for Rust {
    fn can_inline_unions(&self) -> bool {
        false
    }

    fn name_union(&self, parent_type_name: &str, variants: &BTreeSet<SchemaType>) -> String {
        let mut type_names = Vec::new();

        for variant in variants {
            let name = match variant {
                SchemaType::String => "String",
                SchemaType::Number => "F64",
                SchemaType::Boolean => "Bool",
                SchemaType::Null => "Null",
                SchemaType::Unknown => "Value",
                SchemaType::Object(_) => "Object",
                SchemaType::Array(_) => "Array",
                SchemaType::Union(_) => "Union",
            };
            type_names.push(name);
        }

        if type_names.len() <= 3 {
            type_names.join("Or")
        } else {
            format!("{}Value", parent_type_name)
        }
    }

    fn sanitize_field_name(&self, name: &str) -> String {
        if name.is_empty() {
            return "empty".to_string();
        }

        const KEYWORDS: &[&str] = &[
            "as", "break", "const", "continue", "crate", "else", "enum", "extern", "false", "fn",
            "for", "if", "impl", "in", "let", "loop", "match", "mod", "move", "mut", "pub", "ref",
            "return", "self", "Self", "static", "struct", "super", "trait", "true", "type",
            "unsafe", "use", "where", "while", "async", "await", "dyn",
        ];

        let mut chars = name.chars();
        let first = chars.next().unwrap();
        let is_valid_start = first.is_alphabetic() || first == '_';
        let is_valid_rest = chars.all(|c| c.is_alphanumeric() || c == '_');

        let is_valid_identifier = is_valid_start && is_valid_rest;

        if !is_valid_identifier {
            let sanitized: String = name
                .chars()
                .map(|c| {
                    if c.is_alphanumeric() || c == '_' {
                        c
                    } else {
                        '_'
                    }
                })
                .collect();

            let result = if sanitized.chars().next().unwrap().is_numeric() {
                format!("_{}", sanitized)
            } else {
                sanitized
            };

            return result;
        }

        if KEYWORDS.contains(&name) {
            format!("r#{}", name)
        } else {
            name.to_string()
        }
    }

    fn primitive_type(&self, schema: &SchemaType) -> Option<&str> {
        match schema {
            SchemaType::String => Some("String"),
            SchemaType::Number => Some("f64"),
            SchemaType::Boolean => Some("bool"),
            SchemaType::Null => Some("()"),
            SchemaType::Unknown => Some("serde_json::Value"),
            _ => None,
        }
    }

    fn array_type(&self, inner: &str) -> String {
        format!("Vec<{}>", inner)
    }

    fn union_type(&self, _variants: &[String]) -> String {
        panic!("Rust cannot inline unions, union_type should not be called")
    }

    fn object_type_declaration(
        &self,
        name: &str,
        fields: &std::collections::BTreeMap<String, FieldSchema>,
    ) -> String {
        if fields.is_empty() {
            return format!(
                "#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]\npub struct {} {{}}",
                name
            );
        }

        let mut field_defs = Vec::new();
        for (field_name, field_schema) in fields {
            let sanitized_name = self.sanitize_field_name(field_name);
            let type_ref = crate::schema::generator::generate_type_ref(
                &field_schema.type_,
                name,
                Some(field_name),
                self,
            );

            let field_type = if field_schema.required {
                type_ref
            } else {
                format!("Option<{}>", type_ref)
            };

            let rename_attr = if sanitized_name != *field_name {
                format!("    #[serde(rename = \"{}\")]\n", field_name)
            } else {
                String::new()
            };

            field_defs.push(format!(
                "{}    pub {}: {},",
                rename_attr, sanitized_name, field_type
            ));
        }

        format!(
            "#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]\npub struct {} {{\n{}\n}}",
            name,
            field_defs.join("\n")
        )
    }

    fn type_alias_declaration(&self, name: &str, type_ref: &str) -> String {
        format!("pub type {} = {};", name, type_ref)
    }

    fn union_type_declaration(&self, name: &str, variants: &BTreeSet<SchemaType>) -> String {
        let mut variant_defs = Vec::new();

        for variant in variants {
            let type_ref = crate::schema::generator::generate_type_ref(variant, name, None, self);

            let variant_name = match variant {
                SchemaType::String => "String",
                SchemaType::Number => "Number",
                SchemaType::Boolean => "Boolean",
                SchemaType::Null => "Null",
                SchemaType::Unknown => "Value",
                SchemaType::Object(_) => "Object",
                SchemaType::Array(_) => "Array",
                SchemaType::Union(_) => "Union",
            };

            variant_defs.push(format!("    {}({}),", variant_name, type_ref));
        }

        format!(
            "#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]\n#[serde(untagged)]\npub enum {} {{\n{}\n}}",
            name,
            variant_defs.join("\n")
        )
    }
}

pub fn generate(schema: &SchemaType) -> String {
    generate_with_name(schema, "Root")
}

pub fn generate_with_name(schema: &SchemaType, root_name: &str) -> String {
    crate::schema::generator::generate_with_language(schema, root_name, &Rust)
}

#[cfg(test)]
mod tests {
    use indoc::indoc;

    use super::*;

    #[test]
    fn test_simple_object() {
        use crate::schema::infer::infer_schema;

        let json = r#"{"name": "Alice", "age": 30}"#;
        let schema = infer_schema(json).unwrap();

        let result = generate(&schema);
        let expected = indoc! {r#"
            #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
            pub struct Root {
                pub age: f64,
                pub name: String,
            }"#
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
            #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
            pub struct Root {
                pub optional_field: Option<f64>,
                pub required_field: String,
            }"#
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
            #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
            pub struct User {
                pub active: bool,
                pub name: String,
            }

            #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
            pub struct Root {
                pub user: User,
            }"#
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
            #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
            pub struct Root {
                pub scores: Vec<f64>,
            }"#
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
        println!("{}", result);
        // Union types should generate an enum
        assert!(result.contains("StringOrF64"));
        assert!(result.contains("pub value: StringOrF64"));
    }

    #[test]
    fn test_keyword_field() {
        use crate::schema::infer::infer_schema;

        let json = r#"{"type": "MyType", "for": 123}"#;
        let schema = infer_schema(json).unwrap();
        let result = generate(&schema);

        assert!(result.contains("r#for: f64"));
        assert!(result.contains("r#type: String"));
    }

    #[test]
    fn test_sanitize() {
        let rust = Rust;
        assert_eq!(rust.sanitize_field_name("name"), "name");
        assert_eq!(rust.sanitize_field_name("user_id"), "user_id");
        assert_eq!(rust.sanitize_field_name("_private"), "_private");
        assert_eq!(rust.sanitize_field_name("type"), "r#type");
        assert_eq!(rust.sanitize_field_name("for"), "r#for");
        assert_eq!(rust.sanitize_field_name("match"), "r#match");
        assert_eq!(rust.sanitize_field_name("first-name"), "first_name");
        assert_eq!(rust.sanitize_field_name("my field"), "my_field");
        assert_eq!(rust.sanitize_field_name("123abc"), "_123abc");
        assert_eq!(rust.sanitize_field_name("5g"), "_5g");
    }
}
