//! Integration tests for race simulation
//!
//! These tests verify the public API of the race simulation system.

use crate::common::{load_course, race_params, seeded_config, uma};
use uma_tools::configuration::{Configuration, GameVersion};
use uma_tools::race::parameters::*;
use uma_tools::race::solver::RaceSolver;
use uma_tools::uma::*;

#[test]
fn test_basic_race_simulation() {
    // Create a basic Uma
    let _uma = Uma {
        speed: 1200,
        stamina: 800,
        power: 1000,
        guts: 600,
        wiz: 1000,
        style: Style::End,
        distance_aptitude: Aptitude::A,
        track_aptitude: Aptitude::A,
        style_aptitude: Aptitude::A,
        fan_count: None,
    };

    // Create race parameters
    let _params = RaceParameters {
        track_condition: TrackCondition::Good,
        weather: Weather::Sunny,
        season: Season::Spring,
        time: Time::Noon,
        grade: Grade::G1,
        gates: 18,
    };

    // Create a test course (this would normally be loaded from JSON)
    // For now, we'll skip the full course loading and just verify the API works

    // This test verifies that the public API is accessible and compiles correctly
}

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
        let mut solver = RaceSolver::new(course.clone(), params, &config);
        for style in styles {
            solver.add_runner(uma(style), Motivation::Normal, vec![]);
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
        let mut solver = RaceSolver::new(course, params, &config);
        for style in styles {
            solver.add_runner(uma(style), Motivation::Normal, vec![]);
        }
        let mut solver = solver.ready_runners();
        solver.run();

        let mut runners: Vec<_> = solver
            .runners
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
