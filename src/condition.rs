//! Skill conditions: parsing, variables and evaluation.
//!
//! - [`parser`]: parses condition strings into OR branches of comparisons.
//! - [`variables`]: defines and classifies the variables used in conditions.
//! - [`evaluator`]: evaluates guard comparisons against the race state.
//! - [`variable_reference`]: describes every condition variable.
//!
//! See the [`trigger`](crate::trigger) module for an overview of how
//! conditions decide when skills activate.
//!
//! # Examples
//!
//! Find the skills whose conditions use a given variable, e.g. every skill
//! that activates at a random point of a corner:
//!
//! ```
//! use uma_tools::{Skill, SkillData};
//! use uma_tools::condition::variables::VarType;
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
//! let all_skills = SkillData::from_json_file("data/jp/skill_data.json")?;
//! let var = VarType::parse("all_corner_random").ok_or("unknown variable")?;
//! let uses_var = |skill: &Skill| {
//!     skill
//!         .alternatives
//!         .iter()
//!         .flat_map(|alternative| &alternative.condition.branches)
//!         .flatten()
//!         .any(|comparison| comparison.var == var)
//! };
//! let corner_skills: Vec<i32> = all_skills
//!     .skills()
//!     .filter(|skill| uses_var(skill))
//!     .map(|skill| skill.id)
//!     .collect();
//! // Professor of Curvature.
//! assert!(corner_skills.contains(&200331));
//! # Ok(())
//! # }
//! ```

pub mod evaluator;
pub mod parser;
// Enums match condition names to simplify search.
#[allow(clippy::enum_variant_names)]
pub mod variables;

/// Reference of every condition variable: meaning, values and examples.
#[doc = include_str!("../docs/CONDITIONS.md")]
pub mod variable_reference {}
