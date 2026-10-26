//! # Skill Activation System
//!
//! How skills decide when to activate during a race. This module holds the
//! trigger system; this page gives an overview of it and of the other modules
//! involved, which hold the details.
//!
//! ## Triggers and Guards
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
//! [`SkillActivationLogic`](skill_activation::SkillActivationLogic): the unit
//! the scheduler works with, holding the trigger, the guards and what to do
//! when they pass. Triggers reference logics by index.
//!
//! ## Where Things Live
//!
//! - [`condition::parser`]: parse conditions into OR branches of AND chains.
//! - [`condition::variables`]: variable types, and whether they can change
//!   during a race.
//! - [`condition::evaluator`]: evaluate comparisons against the race state.
//! - [`random`]: roll random variables (`phase_random`...) into 10m segments.
//! - [`skill_activation`]: compile conditions and preconditions into triggers
//!   and guards.
//! - [`builder`]: build the [`TriggerScheduler`] of a runner's skills.
//! - [`system`]: scheduler data structures (window queues, event lists).
//! - [`unready_runner`]: wit checks and out-of-gate skills, before the race.
//! - [`runner`]: check triggers each frame, evaluate guards, activate skills.
//! - [`solver`]: frame loop, i.e. the order in which everything above is
//!   called.
//!
//! ## Lifecycle
//!
//! 1. **Runner setup** ([`UnreadyRunner`]): skills that pass the wit check are
//!    kept, and the [`TriggerScheduler`] is built for them. Out-of-gate
//!    alternatives are skipped: their condition is evaluated before the race
//!    instead.
//! 2. **Out-of-gate skills**: before the race, their pre-race effects are
//!    applied, and they're scheduled to activate on the first frame the runner
//!    moves.
//! 3. **Runner creation** ([`Runner`]): guards that can't change during a race
//!    are resolved, once every value they read is final.
//! 4. **Every frame** ([`RaceSolver`]): every runner checks her skills
//!    ([`Runner::check_skills`]), then the race state is updated.
//!
//! [`condition::parser`]: crate::condition::parser
//! [`condition::variables`]: crate::condition::variables
//! [`condition::evaluator`]: crate::condition::evaluator
//! [`unready_runner`]: crate::race::unready_runner
//! [`runner`]: crate::race::runner
//! [`solver`]: crate::race::solver
//! [`TriggerScheduler`]: system::TriggerScheduler
//! [`UnreadyRunner`]: crate::race::unready_runner::UnreadyRunner
//! [`Runner`]: crate::race::runner::Runner
//! [`Runner::check_skills`]: crate::race::runner::Runner::check_skills
//! [`RaceSolver`]: crate::race::solver::RaceSolver

pub mod builder;
pub mod random;
pub mod skill_activation;
pub mod system;
