//! Runtime tests for courses with a run-up, where runners leave the gate before
//! the official start line.

use crate::common::{load_course, negligible_skill, ready_solver, seeded_config, uma};
use uma_tools::race::runner::Runner;
use uma_tools::race::solver::DELTA_TIME;
use uma_tools::uma::*;

/// Santa Anita Park dirt 1600m, with a 50m run-up.
const COURSE_ID: u32 = 11619;

/// Run-up of the test course in meters.
const RUN_UP: f64 = 50.0;

/// Distance from the gate to the goal of the test course in meters.
const LENGTH: f64 = 1650.0;

/// Maximum distance covered in a single frame at a generous speed (30 m/s).
const FRAME_DISTANCE: f64 = 30.0 * DELTA_TIME;

/// Run a single-runner race with the given skill condition; return the runner.
fn run_race(condition: &str, rng_seed: u64) -> Runner {
    let uma = Uma {
        stamina: 1200,
        ..uma(Style::Pace)
    };
    let skills = vec![negligible_skill(condition)];
    let mut solver = ready_solver(COURSE_ID, uma, skills, seeded_config(rng_seed));
    solver.run();
    solver.runners.remove(0)
}

/// Assert that the skill activates exactly once within `[min, max]` meters.
fn assert_activates_within(condition: &str, min: f64, max: f64) {
    for seed in 0..5 {
        let runner = run_race(condition, seed);
        let positions: Vec<f64> = runner.activated_skills.iter().map(|r| r.distance).collect();
        assert_eq!(
            positions.len(),
            1,
            "`{condition}` (seed {seed}) should activate once, got {positions:?}"
        );
        assert!(
            (min..=max).contains(&positions[0]),
            "`{condition}` (seed {seed}) activated at {}, expected [{min}, {max}]",
            positions[0]
        );
    }
}

#[test]
fn test_course_data_includes_run_up() {
    let course = load_course(COURSE_ID);
    assert_eq!(course.distance, 1600);
    assert_eq!(course.run_up, RUN_UP);
    assert_eq!(course.length(), LENGTH);
}

#[test]
fn test_runner_finishes_after_run_up() {
    let runner = run_race("distance_rate>=50", 0);
    assert!(runner.has_finished());
    assert!(
        (LENGTH..LENGTH + FRAME_DISTANCE).contains(&runner.position),
        "finished at {}",
        runner.position
    );
}

#[test]
fn test_remain_distance_counts_down_to_goal() {
    // Floored, so 300m remaining holds from 1349m onwards.
    let start = LENGTH - 301.0;
    assert_activates_within("remain_distance<=300", start, start + FRAME_DISTANCE);
}

#[test]
fn test_distance_rate_excludes_run_up() {
    let halfway = RUN_UP + 800.0;
    assert_activates_within("distance_rate>=50", halfway, halfway + FRAME_DISTANCE);
}

#[test]
fn test_early_phase_starts_after_run_up() {
    assert_activates_within("phase==0", RUN_UP, RUN_UP + FRAME_DISTANCE);
}
