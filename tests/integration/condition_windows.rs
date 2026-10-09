//! Runtime tests for conditions combining several comparisons: every comparison
//! must hold when the skill activates, wherever it is checked.

use crate::common::{negligible_skill, ready_solver, seeded_config, uma};
use uma_tools::uma::*;

/// Run single-runner races (5 seeds) and return where the skill activated.
fn activation_positions(course_id: u32, condition: &str) -> Vec<Vec<f64>> {
    (0..5)
        .map(|seed| {
            let uma = Uma {
                stamina: 1200,
                ..uma(Style::Pace)
            };
            let skills = vec![negligible_skill(condition)];
            let mut solver = ready_solver(course_id, uma, skills, &seeded_config(seed));
            solver.run();
            solver.runners[0]
                .activated_skills
                .iter()
                .map(|record| record.distance)
                .collect()
        })
        .collect()
}

/// Assert that the skill activates exactly once within `[min, max)` meters.
fn assert_activates_within(course_id: u32, condition: &str, min: f64, max: f64) {
    for (seed, positions) in activation_positions(course_id, condition)
        .into_iter()
        .enumerate()
    {
        assert_eq!(
            positions.len(),
            1,
            "`{condition}` (seed {seed}) should activate once, got {positions:?}"
        );
        assert!(
            (min..max).contains(&positions[0]),
            "`{condition}` (seed {seed}) activated at {}, expected [{min}, {max})",
            positions[0]
        );
    }
}

#[test]
fn test_phase_equality_is_checked_during_the_phase() {
    // Course 10104 (2000m): 60% is 1200m, Mid phase ends at 1333m. Previously
    // only checked when entering Mid phase (333m), so it never activated
    assert_activates_within(
        10104,
        "phase==1&distance_rate>=60",
        1200.0,
        2000.0 / 24.0 * 16.0,
    );
}

#[test]
fn test_slope_is_required() {
    // Course 10913: first uphill at 275-400m. Previously the slope comparison was
    // dropped, so it activated right after the start
    assert_activates_within(10913, "slope==1&order<=18", 275.0, 400.0);
}

#[test]
fn test_all_trigger_comparisons_are_required() {
    // Course 10104: final corner at 1475-1734m, Late phase at 1333-1667m.
    // Previously it activated at the start of Late phase, or on the final corner
    // regardless of the phase
    assert_activates_within(
        10104,
        "phase==2&is_finalcorner==1",
        1475.0,
        2000.0 / 24.0 * 20.0,
    );
}
