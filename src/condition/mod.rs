pub mod evaluator;
pub mod parser;
#[allow(clippy::enum_variant_names)] // Enums match condition names to simplify search
pub mod variables;

#[cfg(test)]
mod tests;
