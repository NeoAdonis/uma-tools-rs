//! Integration tests for race simulation
//!
//! These tests verify the public API of the race simulation system.

use crate::common::{load_course, race_params, ready_solver, runaway_skill, seeded_config, uma};
use uma_tools::Entrant;
use uma_tools::configuration::{Configuration, GameVersion};
use uma_tools::race::parameters::*;
use uma_tools::race::runner::Runner;
use uma_tools::race::solver::RaceSolver;
use uma_tools::uma::*;

/// Final position and time of every runner of a 9-runner race, in gate order.
fn race(game_version: GameVersion, seed: u64) -> (Option<f64>, Vec<(u8, f64)>) {
    // The first runner is a Runaway: a Front runner with the Runaway skill.
    let styles = [
        BaseStyle::Front,
        BaseStyle::Front,
        BaseStyle::Front,
        BaseStyle::Pace,
        BaseStyle::Pace,
        BaseStyle::Late,
        BaseStyle::Late,
        BaseStyle::End,
        BaseStyle::End,
    ];
    let params = RaceParameters {
        gates: styles.len() as u8,
        ..race_params()
    };
    let config = Configuration {
        game_version,
        ..seeded_config(seed)
    };
    let mut solver = RaceSolver::new(load_course(10101), params, config);
    for (i, style) in styles.into_iter().enumerate() {
        let skills = if i == 0 {
            vec![runaway_skill()]
        } else {
            vec![]
        };
        solver.add_runner(Entrant::new(uma(style), skills));
    }
    let mut solver = solver.ready_runners();
    solver.run();

    let mut runners: Vec<_> = solver
        .runners()
        .iter()
        .map(|runner| (runner.gate_roll, runner.position))
        .collect();
    runners.sort_by_key(|(gate, _)| *gate);
    (solver.state.finish_time, runners)
}

#[test]
fn test_front_runner_with_runaway_skill_races_as_runaway() {
    let solver = ready_solver(
        10101,
        uma(BaseStyle::Front),
        vec![runaway_skill()],
        seeded_config(0),
    );
    assert_eq!(solver.runners()[0].style, EffectiveStyle::Runaway);
}

#[test]
fn test_races_finish_and_are_reproducible() {
    // Every game version toggles different components during a full race.
    let game_versions = [
        GameVersion::Original,
        GameVersion::FirstAnniversary,
        GameVersion::OnePointFiveAnniversary,
        GameVersion::TwoPointFiveAnniversary,
        GameVersion::MechaUmaUpdate,
        GameVersion::FifthAnniversary,
    ];
    for game_version in game_versions {
        for seed in 0..5 {
            let result = race(game_version, seed);
            assert!(result.0.is_some(), "{game_version:?}, seed {seed}");
            assert_eq!(
                result,
                race(game_version, seed),
                "{game_version:?}, seed {seed}"
            );
        }
    }
}

/// Solvers can be moved to and shared between threads, e.g. to simulate races
/// or analyze results in parallel.
#[test]
fn test_solvers_are_send_and_sync() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<RaceSolver<Entrant>>();
    assert_send_sync::<RaceSolver<Runner>>();
}
