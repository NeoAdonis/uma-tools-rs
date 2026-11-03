//! Import of Umas from data exported by other tools.
//!
//! Each submodule supports one format:
//!
//! - [`umadump`]: trained Umas exported by umadump.
//! - [`umamoe`]: race setups (course, conditions and runners) shared as build
//!   codes by uma.moe.

pub mod umadump;
pub mod umamoe;
