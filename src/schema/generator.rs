use std::borrow::Cow;
use std::collections::{BTreeMap, BTreeSet};

use heck::AsPascalCase;
use indexmap::IndexMap;
use singularize::singularize;

use crate::schema::{FieldSchema, SchemaType};

pub mod rust;
pub mod typescript;

pub trait Language {
    /// Can this language inline union types?
    ///
    /// e.g. in TS we can do `x: string | number`, but in Rust that'd need to be a separate enum
    fn can_inline_unions(&self) -> bool;

    /// Generate a name for a union type (only called if `can_inline_unions()` returns false)
    fn name_union(&self, parent_type_name: &str, variants: &BTreeSet<SchemaType>) -> String;

    /// Sanitize a field name to be a valid identifier in this language
    fn sanitize_field_name(&self, name: &str) -> String;

    /// How to represent a given primitive type in this lang
    fn primitive_type(&self, schema: &SchemaType) -> Option<&str>;

    /// How to format an array type for the given type
    fn array_type(&self, inner: &str) -> String;

    /// How to format a union type given the variant type strings (inline format)
    fn union_type(&self, variants: &[String]) -> String;

    /// Generate a full type declaration for an object with fields
    fn object_type_declaration(&self, name: &str, fields: &BTreeMap<String, FieldSchema>)
    -> String;

    /// Generate a type alias declaration
    fn type_alias_declaration(&self, name: &str, type_ref: &str) -> String;
}

/// What should we call this inner type?
///
/// - parent_name is the name of the parent type
/// - field_name is the inferred name of the field that this type is the value of
/// - is_array_item is true if this type is the value of an array item
///
/// Examples:
/// - {"users": [{...}]} -> parent_name="Root", field_name="users", is_array_item=true -> "User"
/// - {"user": {...}} -> "User"
/// - [{...}] -> "RootItem"
/// - {...} -> "Root"
pub fn get_target_type_name(
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

/// Collects all "inner" objects (and unions, if language can't inline them) that need to become their own types.
/// Allows duplicates - merging happens later in `generate_with_language`.
fn collect_types<'a, L: Language>(
    schema: &'a SchemaType,
    root_name: &str,
    lang: &L,
) -> Vec<(String, &'a SchemaType)> {
    fn collect_types_rec<'a, L: Language>(
        schema: &'a SchemaType,
        parent_type_name: &str,
        field_name: Option<&str>,
        lang: &L,
        types: &mut Vec<(String, &'a SchemaType)>,
    ) {
        match schema {
            SchemaType::Object(fields) => {
                let current_type_name = if let Some(fname) = field_name {
                    let name = get_target_type_name(parent_type_name, Some(fname), false);
                    types.push((name.clone(), schema));
                    name
                } else {
                    // Root object - not added to types list
                    parent_type_name.to_string()
                };

                for (fname, field_schema) in fields {
                    collect_types_rec(
                        &field_schema.type_,
                        &current_type_name,
                        Some(fname),
                        lang,
                        types,
                    );
                }
            }
            SchemaType::Array(inner) => {
                if let SchemaType::Object(obj_fields) = &**inner {
                    let item_name = get_target_type_name(parent_type_name, field_name, true);
                    types.push((item_name.clone(), inner));

                    for (fname, field_schema) in obj_fields {
                        collect_types_rec(
                            &field_schema.type_,
                            &item_name,
                            Some(fname),
                            lang,
                            types,
                        );
                    }
                } else {
                    collect_types_rec(inner, parent_type_name, field_name, lang, types);
                }
            }
            SchemaType::Union(variants) => {
                // If this language can't inline unions, we'll have to add the unions as new
                // types to be named separately
                if !lang.can_inline_unions() {
                    let union_name = lang.name_union(parent_type_name, variants);
                    types.push((union_name.clone(), schema));
                }

                // Still recurse into variants to collect nested objects
                for variant in variants {
                    collect_types_rec(variant, parent_type_name, field_name, lang, types);
                }
            }
            _ => {}
        }
    }

    let mut collected = Vec::new();
    collect_types_rec(schema, root_name, None, lang, &mut collected);
    collected
}

/// How do we write the given type in this language?
pub fn generate_type_ref<L: Language>(
    schema: &SchemaType,
    parent_type_name: &str,
    field_name: Option<&str>,
    lang: &L,
) -> String {
    // Check if it's a primitive type first
    if let Some(prim) = lang.primitive_type(schema) {
        return prim.to_string();
    }

    match schema {
        SchemaType::Object(_) => get_target_type_name(parent_type_name, field_name, false),
        SchemaType::Array(inner) => {
            let inner_type = match &**inner {
                SchemaType::Object(_) => get_target_type_name(parent_type_name, field_name, true),
                _ => generate_type_ref(inner, parent_type_name, field_name, lang),
            };
            lang.array_type(&inner_type)
        }
        SchemaType::Union(variants) => {
            if lang.can_inline_unions() {
                // Inline the union
                let type_refs: Vec<String> = variants
                    .iter()
                    .map(|v| generate_type_ref(v, parent_type_name, field_name, lang))
                    .collect();
                lang.union_type(&type_refs)
            } else {
                // Reference the named union type
                lang.name_union(parent_type_name, variants)
            }
        }
        _ => unreachable!("All other cases should be handled by primitive_type"),
    }
}

fn generate_single_type<L: Language>(name: &str, schema: &SchemaType, lang: &L) -> String {
    match schema {
        SchemaType::Object(fields) => lang.object_type_declaration(name, fields),
        _ => {
            let type_ref = generate_type_ref(schema, name, None, lang);
            lang.type_alias_declaration(name, &type_ref)
        }
    }
}

pub fn generate_with_language<L: Language>(
    schema: &SchemaType,
    root_name: &str,
    lang: &L,
) -> String {
    let collected = collect_types(schema, root_name, lang);

    // If there are any duplicates in the types collected, merge them
    let mut types: IndexMap<String, Cow<'_, SchemaType>> = IndexMap::new();
    for (name, schema_ref) in collected {
        match types.shift_remove(&name) {
            Some(existing) => {
                let merged = ((*existing).clone()).merge(schema_ref.clone());
                types.insert(name, Cow::Owned(merged));
            }
            None => {
                types.insert(name, Cow::Borrowed(schema_ref));
            }
        }
    }

    let mut result: Vec<String> = types
        .iter()
        .rev() // Nested types first
        .map(|(name, schema)| generate_single_type(name, schema, lang))
        .collect();

    result.push(generate_single_type(root_name, schema, lang));

    result.join("\n\n")
}
