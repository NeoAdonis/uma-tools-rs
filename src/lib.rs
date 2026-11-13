//! Simulation of Umamusume: Pretty Derby races, aimed at running many races to
//! analyze performance metrics and strategies.
//!
//! The main modules are:
//!
//! - [`course`]: course data (distance, corners, straights, slopes, etc.).
//! - [`uma`]: Umamusume stats, aptitudes, running styles and moods.
//! - [`skill`]: skill data, effects and conditions.
//! - [`race`]: race parameters, mechanics and the [`RaceSolver`] that
//!   simulates them.
//! - [`condition`] and [`trigger`]: parsing and evaluation of skill
//!   conditions, and the system that activates skills during a race.
//! - [`configuration`]: simulation behavior, game version and language.
//! - [`import`]: import of Umas from data exported by other tools.
//! - [`names`]: character, skill and track names in every supported language.
//!
//! Commonly used types are re-exported at the crate root.
//!
//! # Examples
//!
//! Simulate a race with a single runner, using the course and skill data in
//! the repository:
//!
//! ```
//! use uma_tools::{
//!     Configuration, Course, RaceParametersData, RaceSolver, SkillData,
//!     UmaData,
//! };
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
//! let parameters = RaceParametersData::from_json(
//!     r#"{
//!         "trackCondition": "Firm",
//!         "weather": "Sunny",
//!         "season": "Spring",
//!         "time": "Noon",
//!         "grade": "G1",
//!         "gates": 18,
//!         "courseId": "10913"
//!     }"#,
//! )?;
//! let uma = UmaData::from_json(
//!     r#"{
//!         "speed": 1500, "stamina": 800, "power": 1200, "guts": 700,
//!         "wisdom": 1200, "strategy": "Oikomi", "distanceAptitude": "S",
//!         "surfaceAptitude": "A", "strategyAptitude": "A",
//!         "motivation": "Great", "skills": [200642]
//!     }"#,
//! )?;
//! let course = Course::from_json_files(
//!     "data/course_data.json",
//!     "data/jp/rails",
//!     &parameters.course_id,
//! )?;
//! let all_skills = SkillData::from_json_file("data/jp/skill_data.json")?;
//! let entrant = uma.to_entrant(&all_skills)?;
//!
//! let config = Configuration {
//!     seed: Some(42),
//!     ..Configuration::default()
//! };
//! let mut solver = RaceSolver::new(course, parameters.race, config);
//! solver.add_runner(entrant);
//! let mut solver = solver.ready_runners();
//! solver.run();
//!
//! let runner = &solver.runners()[0];
//! assert!(runner.finish_time.is_some());
//! println!("Activated skills: {:?}", runner.activated_skills);
//! # Ok(())
//! # }
//! ```

#![warn(missing_debug_implementations)]

pub mod condition;
pub mod configuration;
pub mod course;
pub mod import;
pub mod names;
pub mod race;
pub mod skill;
pub mod trigger;
pub mod uma;
pub mod units;

#[cfg(test)]
mod test_utils;

// Re-export the types needed to load data, build a race, run it and read its
// results.
pub use condition::parser::ParseError;
pub use configuration::Configuration;
pub use course::Course;
pub use race::parameters::{RaceParameters, RaceParametersData};
pub use race::pins::Pins;
pub use race::runner::Runner;
pub use race::solver::{Entrant, RaceSolver, RunnerId};
pub use skill::{Skill, SkillData, SkillDataError};
pub use uma::{
    Aptitude, BaseStyle, EffectiveStyle, Mood, ResolveSkillsError, Stats, Uma, UmaData, UmaSkill,
};
