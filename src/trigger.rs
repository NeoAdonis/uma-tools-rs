//! Trigger system that determines skill activation times during a race.
//!
//! This page gives an overview of it and of the other modules involved, with
//! the details held in their respective modules.
//!
//! # Triggers and Guards
//!
//! Skill data gives each skill one or more **alternatives**, each with a
//! condition and an optional precondition. Naively, every condition would be
//! evaluated every frame for every skill. Instead, each condition is compiled
//! into:
//!
//! - **Guards**: comparisons that must all be true **at that moment** for the
//!   skill to activate (e.g. `order<=3`, `hp_per>=20`). They alone decide
//!   *whether* a skill activates.
//! - **A trigger**: decides **when** the guards are worth evaluating (e.g.
//!   "from 60% of the race on", "while on the final corner", "while inside a
//!   rolled random segment", "when a heal skill activates").
//!
//! Guards are only evaluated when a trigger fires, so most skills cost nothing
//! on most frames. Triggers are only an optimization: a trigger never replaces
//! a guard, so it only needs to fire *at least* whenever the guards could all
//! be true. A trigger that's too wide costs extra checks; it can never make a
//! skill activate when it shouldn't.
//!
//! Each OR branch of an alternative's condition becomes a
//! [`SkillActivationLogic`]: the unit the scheduler works with, holding the
//! trigger, the guards and what to do when they pass. Triggers reference logics
//! by index.
//!
//! # Where Things Live
//!
//! - [`condition::parser`]: parse conditions into OR branches of AND chains.
//! - [`condition::variables`]: variable types, and whether they can change
//!   during a race.
//! - [`condition::evaluator`]: evaluate comparisons against the race state.
//! - `random`: roll random variables (`phase_random`...) into 10m segments.
//! - `skill_activation`: compile conditions and preconditions into triggers
//!   and guards.
//! - `builder`: build the `TriggerScheduler` of a runner's skills.
//! - `system`: scheduler data structures (window queues, event lists).
//! - [`unready_runner`]: wit checks and out-of-gate skills, before the race.
//! - [`runner`]: check triggers each frame, evaluate guards, activate skills.
//! - [`solver`]: frame loop, i.e. the order in which everything above is
//!   called.
//!
//! # Lifecycle
//!
//! 1. **Runner setup** ([`UnreadyRunner`]): skills that pass the wit check are
//!    kept, and the `TriggerScheduler` is built for them. Out-of-gate
//!    alternatives are skipped: their condition is evaluated before the race
//!    instead. Skills whose activations are pinned only get their conditions
//!    checked, to report whether they agree with the pins.
//! 2. **Out-of-gate skills**: before the race, their pre-race effects are
//!    applied, and they're scheduled to activate on the first frame the runner
//!    moves.
//! 3. **Runner creation** ([`Runner`]): guards that can't change during a race
//!    are resolved, once every value they read is final.
//! 4. **Every frame** ([`RaceSolver`]): every runner checks her skills
//!    (`Runner::check_skills`), then the race state is updated.
//!
//! # Examples
//!
//! Find where a skill tends to activate over many races, from the activation
//! records of each runner. Professor of Curvature activates at a random point
//! of any corner:
//!
//! ```
//! use uma_tools::course::geometry::Segment;
//! use uma_tools::{Configuration, RaceSolver};
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
//! # use uma_tools::{Course, RaceParametersData, SkillData, UmaData};
//! # let parameters = RaceParametersData::from_json(
//! #     r#"{
//! #         "trackCondition": "Firm", "weather": "Sunny", "season": "Spring",
//! #         "time": "Noon", "grade": "G1", "gates": 18, "courseId": "10913"
//! #     }"#,
//! # )?;
//! # let uma = UmaData::from_json(
//! #     r#"{
//! #         "speed": 1500, "stamina": 800, "power": 1200, "guts": 700,
//! #         "wisdom": 1200, "strategy": "Oikomi", "distanceAptitude": "S",
//! #         "surfaceAptitude": "A", "strategyAptitude": "A",
//! #         "motivation": "Great", "skills": [200331]
//! #     }"#,
//! # )?;
//! # let (courses, rails) = ("data/course_data.json", "data/jp/rails");
//! # let course =
//! #     Course::from_json_files(courses, rails, &parameters.course_id)?;
//! # let all_skills = SkillData::from_json_file("data/jp/skill_data.json")?;
//! # let entrant = uma.to_entrant(&all_skills)?;
//! let mut positions = Vec::new();
//! for seed in 0..20 {
//!     let config = Configuration {
//!         seed: Some(seed),
//!         ..Configuration::default()
//!     };
//!     let course = course.clone();
//!     let mut solver = RaceSolver::new(course, parameters.race, config);
//!     solver.add_runner(entrant.clone());
//!     let mut solver = solver.ready_runners();
//!     solver.run();
//!     let activations = &solver.runners()[0].activated_skills();
//!     positions.extend(
//!         activations
//!             .iter()
//!             .filter(|activation| activation.skill_id == 200331)
//!             .map(|activation| activation.distance),
//!     );
//! }
//!
//! let final_corner = course.corners.last().ok_or("no corners")?;
//! let on_final_corner =
//!     positions.iter().filter(|&&d| final_corner.contains(d));
//! println!(
//!     "{} of {} activations on the final corner",
//!     on_final_corner.count(),
//!     positions.len()
//! );
//! # Ok(())
//! # }
//! ```
//!
//! [`condition::parser`]: crate::condition::parser
//! [`condition::variables`]: crate::condition::variables
//! [`condition::evaluator`]: crate::condition::evaluator
//! [`unready_runner`]: crate::race::unready_runner
//! [`runner`]: crate::race::runner
//! [`solver`]: crate::race::solver
//! [`UnreadyRunner`]: crate::race::unready_runner::UnreadyRunner
//! [`Runner`]: crate::race::runner::Runner
//! [`RaceSolver`]: crate::race::solver::RaceSolver

pub(crate) mod builder;
pub(crate) mod random;
pub(crate) mod skill_activation;
pub(crate) mod system;

pub use skill_activation::{SkillActivationLogic, SkillActivationRecord};

#[cfg(test)]
mod tests;
