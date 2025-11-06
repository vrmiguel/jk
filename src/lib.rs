mod borrowed_value;
/// Prints a flattened version of the loaded JSON
pub mod flatten;
/// A JSON formatter
mod fmt;
/// Reverts `jk flatten` to its original form
pub mod unflatten;

pub use fmt::Formatter;
