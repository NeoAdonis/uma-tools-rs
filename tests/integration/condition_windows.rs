//! Runtime tests for conditions combining several comparisons: every comparison
//! must hold when the skill activates, wherever it is checked.

use crate::common::{load_course, race_params, seeded_config};
use uma_tools::race::solver::RaceSolver;
use uma_tools::skill::*;
use uma_tools::uma::*;

/// Build a skill with the given condition, without wisdom check.
fn skill(condition: &str) -> Skill {
    Skill::new(
        "999999".to_string(),
        SkillData {
            rarity: SkillRarity::White,
            alternatives: vec![SkillAlternative {
                base_duration: 1.0,
                base_cooldown: DEFAULT_COOLDOWN,
                condition: condition.parse().unwrap(),
                effects: vec![SkillEffect {
                    skill_type: SkillType::TargetSpeed,
                    target: SkillTarget::Oneself,
                    modifier: 0.0001,
                    value_usage: ValueUsage::Direct,
                }],
                precondition: None,
            }],
            popularity: None,
            wisdom_check: false,
        },
    )
}

/// Run single-runner races (5 seeds) and return where the skill activated.
fn activation_positions(course_id: u32, condition: &str) -> Vec<Vec<f64>> {
    (0..5)
        .map(|seed| {
            let uma = Uma {
                speed: 1200,
                stamina: 1200,
                power: 1000,
                guts: 800,
                wiz: 1000,
                style: Style::Pace,
                distance_aptitude: Aptitude::A,
                track_aptitude: Aptitude::A,
                style_aptitude: Aptitude::A,
            };
            let mut solver =
                RaceSolver::new(load_course(course_id), race_params(), &seeded_config(seed));
            solver.add_runner(uma, Motivation::Normal, vec![skill(condition)]);
            let mut solver = solver.ready_runners();
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
