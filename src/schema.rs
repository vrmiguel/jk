use std::collections::{BTreeMap, BTreeSet};

/// Given a schema, generate types that would deserialize it
pub mod generator;
/// Parse a JSON and infer its corresponding schema
pub mod infer;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum SchemaType {
    Object(BTreeMap<String, FieldSchema>),
    Array(Box<SchemaType>),
    String,
    Number,
    Boolean,
    Null,
    /// Should be reserved for scenarios where we really can't infer a proper type,
    /// because there's just not enough info for such. E.g. if the JSON is `[]`.
    Unknown,
    Union(BTreeSet<SchemaType>),
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct FieldSchema {
    pub type_: SchemaType,
    pub required: bool,
}
