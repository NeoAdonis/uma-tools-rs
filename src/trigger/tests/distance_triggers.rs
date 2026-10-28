//! Runtime tests for distance-based triggers (random points, remaining
//! distance, and ranges made of monotonic comparisons).

use crate::course::Phase;
use crate::race::rng::RngSeed;
use crate::race::runner::Runner;
use crate::race::solver::DELTA_TIME;
use crate::test_utils::{load_course, negligible_skill, ready_solver, seeded_config, uma};
use crate::trigger::skill_activation::TriggerCondition;
use crate::trigger::system::TriggerScheduler;
use crate::uma::*;

/// Length of the test course (10104) in meters.
const COURSE_DISTANCE: f64 = 2000.0;

/// Run a single-runner race with the given skill condition; return the runner.
fn run_race(condition: &str, rng_seed: u64) -> Runner {
    let uma = Uma {
        stamina: 1200,
        ..uma(Style::Pace)
    };
    let skills = vec![negligible_skill(condition)];
    let mut solver = ready_solver(10104, uma, skills, seeded_config(rng_seed));
    solver.run();
    solver.runners.remove(0)
}

/// Run a single-runner race with the given skill condition and return the
/// positions at which the skill activated.
fn activation_positions(condition: &str, rng_seed: u64) -> Vec<f64> {
    run_race(condition, rng_seed)
        .activated_skills
        .iter()
        .map(|record| record.distance)
        .collect()
}

/// Assert that the skill activates exactly once within `[min, max]` meters.
fn assert_activates_within(condition: &str, min: f64, max: f64) {
    for seed in 0..5 {
        let positions = activation_positions(condition, seed);
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

/// Maximum distance covered in a single frame at a generous speed (30 m/s).
const FRAME_DISTANCE: f64 = 30.0 * DELTA_TIME;

#[test]
fn test_random_point_activates_within_phase() {
    let (start, end) = Phase::Mid.boundaries(&load_course(10104));
    assert_activates_within("phase_random==1", start, end + FRAME_DISTANCE);
}

#[test]
fn test_remain_distance_activates_near_goal() {
    // Remaining distance is floored: 300.5m remaining counts as 300m
    let threshold = COURSE_DISTANCE - 301.0;
    assert_activates_within(
        "remain_distance<=300",
        threshold,
        threshold + FRAME_DISTANCE,
    );
}

#[test]
fn test_reversed_remain_distance_activates_early() {
    // remain_distance>=350 holds from the start until 1650m
    assert_activates_within("remain_distance>=350", 0.0, COURSE_DISTANCE - 350.0);
}

#[test]
fn test_remain_distance_range_activates_within_range() {
    // Skill 100771
    // 399 <= remain_distance <= 401, floored (so up to 402m remaining)
    let min = COURSE_DISTANCE - 402.0;
    let max = COURSE_DISTANCE - 399.0;
    assert_activates_within("remain_distance<=401&remain_distance>=399", min, max);
}

#[test]
fn test_distance_windows_are_positions_in_order() {
    use crate::units::furlongs_to_meters;
    let skills = [
        negligible_skill("remain_distance<=300"), // 1700m (1699m, as values are floored)
        negligible_skill("distance_rate>=50"),    // 1000m
        negligible_skill("furlong>=3"),           // 3 furlongs
    ];
    let rng_seed = RngSeed::new(0);
    let scheduler = TriggerScheduler::from_skills(&skills, &load_course(10104), rng_seed);

    // Windows are queued by their start
    let order: Vec<(usize, f64)> = scheduler
        .position_windows
        .iter()
        .map(|window| (window.logic_idx, window.min_threshold))
        .collect();
    assert_eq!(
        order,
        vec![
            (2, furlongs_to_meters(3.0)),
            (1, 1000.0),
            (0, COURSE_DISTANCE - 301.0)
        ]
    );
}

#[test]
fn test_reversed_remain_distance_window_range() {
    let rng_seed = RngSeed::new(0);
    let scheduler = TriggerScheduler::from_skills(
        &[negligible_skill("remain_distance>=350")],
        &load_course(10104),
        rng_seed,
    );

    assert_eq!(scheduler.position_windows.len(), 1);
    let window = scheduler.position_windows.iter().next().unwrap();
    assert_eq!(window.min_threshold, 0.0);
    assert_eq!(window.max_threshold, COURSE_DISTANCE - 350.0);
}

#[test]
fn test_remain_distance_pair_window_range() {
    let rng_seed = RngSeed::new(0);
    let scheduler = TriggerScheduler::from_skills(
        &[negligible_skill(
            "remain_distance<=401&remain_distance>=399",
        )],
        &load_course(10104),
        rng_seed,
    );

    // A single window; both comparisons stay as guards
    assert_eq!(scheduler.skill_logics[0].guards.len(), 2);
    assert_eq!(scheduler.position_windows.len(), 1);
    let window = scheduler.position_windows.iter().next().unwrap();
    assert_eq!(window.min_threshold, COURSE_DISTANCE - 402.0);
    assert_eq!(window.max_threshold, COURSE_DISTANCE - 399.0);
}

#[test]
fn test_all_corner_random_activates_at_later_segment() {
    // Up to 4 segments are picked on corners, but the skill can only activate on
    // the fourth corner (1475-1734m). Earlier segments fail the guard, so it
    // activates at the first segment on the fourth corner, if there's any.
    let condition = "all_corner_random==1&corner==4";
    let mut activated = 0;
    for seed in 0..20 {
        let positions = activation_positions(condition, seed);
        assert!(positions.len() <= 1, "seed {seed}: {positions:?}");
        if let Some(&position) = positions.first() {
            assert!(
                (1475.0..=1734.0).contains(&position),
                "seed {seed}: activated at {position}"
            );
            activated += 1;
        }
    }

    // With a single segment, it would only activate a quarter of the time
    assert!(activated >= 12, "activated in {activated} of 20 races");
}

#[test]
fn test_random_segment_activates_within_segment() {
    for seed in 0..5 {
        let runner = run_race("phase_random==1", seed);

        // The rolled segment is 10m long
        let TriggerCondition::Segments { ranges } =
            &runner.trigger_scheduler.skill_logics[0].trigger
        else {
            panic!("seed {seed}: expected segments");
        };
        assert_eq!(ranges.len(), 1, "seed {seed}");
        let (start, end) = ranges[0];
        assert_eq!(end - start, 10.0, "seed {seed}");

        // Activates in the first frame inside the segment
        let positions: Vec<f64> = runner.activated_skills.iter().map(|r| r.distance).collect();
        assert_eq!(positions.len(), 1, "seed {seed}");
        assert!(
            (start..=start + FRAME_DISTANCE).contains(&positions[0]),
            "seed {seed}: activated at {}, segment [{start}, {end}]",
            positions[0]
        );
    }
}
