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
//!
//! Commonly used types are re-exported at the crate root.
//!
//! # Examples
//!
//! Simulate a race with a single runner, using the course and skill data in
//! the repository:
//!
//! ```
//! use uma_tools::configuration::Configuration;
//! use uma_tools::{Course, RaceParametersData, RaceSolver, Skill, UmaData};
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
//! let course =
//!     Course::from_json_file("data/course_data.json", &parameters.course_id)?;
//! let all_skills = Skill::load_all_from_json_file("data/skill_data.json")?;
//! let skills = uma.resolve_skills(&all_skills)?;
//!
//! let config = Configuration {
//!     seed: Some(42),
//!     ..Configuration::default()
//! };
//! let mut solver = RaceSolver::new(course, parameters.race, config);
//! solver.add_runner(uma.uma, uma.mood, skills);
//! let mut solver = solver.ready_runners();
//! solver.run();
//!
//! let runner = &solver.runners[0];
//! assert!(runner.finish_time.is_some());
//! println!("Activated skills: {:?}", runner.activated_skills);
//! # Ok(())
//! # }
//! ```

#![warn(missing_debug_implementations)]

pub mod condition;
pub mod configuration;
pub mod course;
pub mod race;
pub mod skill;
pub mod trigger;
pub mod uma;
pub mod units;

#[cfg(test)]
mod test_utils;

// Re-export commonly used types for convenience.
pub use condition::*;
pub use course::*;
pub use race::parameters::*;
pub use race::solver::*;
pub use skill::*;
pub use uma::*;
