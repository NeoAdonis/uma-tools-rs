//! Import of Umas from data exported by other tools.
//!
//! Each submodule supports one format:
//!
//! - [`umaextractor`]: trained Umas exported by UmaExtractor.
//! - [`umamoe`]: race setups (course, conditions and runners) shared as build
//!   codes by uma.moe.

pub mod umaextractor;
pub mod umamoe;
