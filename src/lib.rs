/// Similar to `serde_json::Value`, but keeps only references to source data
pub mod borrowed_value;
/// Prints a flattened version of the loaded JSON
pub mod flatten;
/// A JSON formatter
pub mod fmt;
/// A foldable (sum-)tree
pub mod fold_tree;
/// Reverts `jk flatten` to its original form
pub mod unflatten;
/// Code for inferring a schema from JSON, and generating types to deserialize
/// it in different languages
pub mod schema;

pub use borrowed_value::{Value, ValueEvents, parse_value};
pub use flatten::flatten;
pub use fmt::Formatter;
pub use jsax::{Event, Parser};
