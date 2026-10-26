//! Skill conditions: parsing, variables and evaluation.
//!
//! - [`parser`]: parses condition strings into OR branches of comparisons.
//! - [`variables`]: defines and classifies the variables used in conditions.
//! - [`evaluator`]: rewrites guard comparisons for a runner, and evaluates them
//!   against the race state.
//! - [`variable_reference`]: describes every condition variable.
//!
//! See the [`trigger`](crate::trigger) module for an overview of how
//! conditions decide when skills activate.

pub mod evaluator;
pub mod parser;
// Enums match condition names to simplify search.
#[allow(clippy::enum_variant_names)]
pub mod variables;

/// Reference of every condition variable: meaning, values and examples.
#[doc = include_str!("../docs/CONDITIONS.md")]
pub mod variable_reference {}
