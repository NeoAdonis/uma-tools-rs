//! Race simulation: parameters, runners and mechanics.
//!
//! A race is set up and run by a [`RaceSolver`](solver::RaceSolver) in two
//! phases. First, runners are added as
//! [`UnreadyRunner`](unready_runner::UnreadyRunner)s under the given
//! [`RaceParameters`](parameters::RaceParameters). Once every runner is known,
//! they become [`Runner`](runner::Runner)s, which the solver updates frame by
//! frame until the race ends.
//!
//! The remaining modules each implement a race mechanic (HP, rushing, dueling,
//! last spurt, etc.) or a helper shared by several of them, such as stat
//! modifiers and proximity checks between runners.

pub mod all_out_spurt;
pub mod charge_up;
pub mod compete_before_spurt;
pub mod duel;
pub mod hp;
pub mod last_spurt;
pub mod margin;
pub mod modifiers;
pub mod parameters;
pub mod popularity;
pub mod proximity;
pub mod runner;
pub mod rush;
pub mod secure_lead;
pub mod skill_scaling;
pub mod solver;
pub mod spot_struggle;
pub mod timed_boost;
pub mod transitions;
pub mod unready_runner;
pub mod world_distance;
