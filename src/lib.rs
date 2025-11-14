/// Similar to `serde_json::Value`, but keeps only references to source data
mod borrowed_value;
/// Prints a flattened version of the loaded JSON
pub mod flatten;
/// A JSON formatter
pub mod fmt;
/// Reverts `jk flatten` to its original form
pub mod unflatten;
/// Foldable tree structure for efficient JSON viewing
pub mod fold_tree;

pub use borrowed_value::{Value, ValueEvents};
pub use flatten::flatten;
pub use fmt::Formatter;
pub use jsax::{Event, Parser};
