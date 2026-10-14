pub mod evaluator;
pub mod parser;
// Enums match condition names to simplify search.
#[allow(clippy::enum_variant_names)]
pub mod variables;

/// Reference of every condition variable: meaning, values and examples.
#[doc = include_str!("../docs/CONDITIONS.md")]
pub mod reference {}
