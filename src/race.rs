//! Race simulation: parameters, runners and mechanics.
//!
//! A race is set up and run by a [`RaceSolver`](solver::RaceSolver) in two
//! phases. First, runners are added as [`Entrant`](solver::Entrant)s
//! under the given [`RaceParameters`](parameters::RaceParameters). Once every
//! runner is known, they get their gates and moods, and become
//! [`Runner`](runner::Runner)s, which the solver updates frame by frame until
//! the race ends.
//!
//! The remaining modules each implement a race mechanic (HP, rushing, dueling,
//! last spurt, etc.) or a helper shared by several of them, such as stat
//! modifiers and proximity checks between runners.
//!
//! See [`RaceSolver`](solver::RaceSolver) for examples.

pub(crate) mod all_out_spurt;
pub(crate) mod charge_up;
pub(crate) mod compete_before_spurt;
pub(crate) mod duel;
pub mod events;
pub mod hp;
pub mod last_spurt;
pub mod margin;
pub(crate) mod modifiers;
pub mod parameters;
pub mod pins;
pub mod popularity;
pub(crate) mod proximity;
pub(crate) mod rng;
pub mod runner;
pub(crate) mod rush;
pub(crate) mod secure_lead;
pub(crate) mod skill_scaling;
pub mod solver;
pub(crate) mod spot_struggle;
pub(crate) mod timed_boost;
pub(crate) mod transitions;
pub(crate) mod unready_runner;
pub(crate) mod world_distance;
