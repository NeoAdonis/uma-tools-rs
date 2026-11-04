//! Integration tests for race simulation
//!
//! These tests verify the public API of the race simulation system.

use crate::common::{load_course, race_params, seeded_config, uma};
use uma_tools::Entrant;
use uma_tools::configuration::{Configuration, GameVersion};
use uma_tools::race::parameters::*;
use uma_tools::race::runner::Runner;
use uma_tools::race::solver::RaceSolver;
use uma_tools::uma::*;

#[test]
fn test_multi_runner_race_simulation() {
    let course = load_course(10101);
    let styles = [
        Style::Runaway,
        Style::Front,
        Style::Front,
        Style::Pace,
        Style::Pace,
        Style::Late,
        Style::Late,
        Style::End,
        Style::End,
    ];

    // Every game version toggles different components during a full race
    let game_versions = [
        GameVersion::Original,
        GameVersion::FirstAnniversary,
        GameVersion::OnePointFiveAnniversary,
        GameVersion::TwoPointFiveAnniversary,
        GameVersion::MechaUmaUpdate,
        GameVersion::FifthAnniversary,
    ];

    for (game_version, seed) in game_versions
        .into_iter()
        .flat_map(|b| (0..5).map(move |s| (b, s)))
    {
        let config = Configuration {
            game_version,
            ..seeded_config(seed)
        };
        let params = RaceParameters {
            gates: styles.len() as u8,
            ..race_params()
        };
        let mut solver = RaceSolver::new(course.clone(), params, config);
        for style in styles {
            solver.add_runner(Entrant::new(uma(style), vec![]));
        }
        let mut solver = solver.ready_runners();
        solver.run();
        assert!(
            solver.state.finish_time.is_some(),
            "{game_version:?}, seed {seed}"
        );
    }
}

#[test]
fn test_same_seed_gives_same_race() {
    let styles = [
        Style::Runaway,
        Style::Front,
        Style::Front,
        Style::Pace,
        Style::Pace,
        Style::Late,
        Style::Late,
        Style::End,
        Style::End,
    ];

    // Final position and time of every runner, in gate order
    let race = |seed: u64| {
        let course = load_course(10101);
        let params = RaceParameters {
            gates: styles.len() as u8,
            ..race_params()
        };
        let config = seeded_config(seed);
        let mut solver = RaceSolver::new(course, params, config);
        for style in styles {
            solver.add_runner(Entrant::new(uma(style), vec![]));
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
    };

    for seed in 0..5 {
        assert_eq!(race(seed), race(seed), "seed {seed}");
    }
}

/// Solvers can be moved to other threads, e.g. to simulate races in parallel.
#[test]
fn test_solvers_are_send() {
    fn assert_send<T: Send>() {}
    assert_send::<RaceSolver<Entrant>>();
    assert_send::<RaceSolver<Runner>>();
}

/// Solvers can be shared between threads, e.g. to analyze results in parallel.
#[test]
fn test_solvers_are_sync() {
    fn assert_sync<T: Sync>() {}
    assert_sync::<RaceSolver<Entrant>>();
    assert_sync::<RaceSolver<Runner>>();
}
