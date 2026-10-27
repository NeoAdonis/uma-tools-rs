//! Tests for precondition build-time and runtime behavior
//!
//! These tests complement the existing precondition tests in
//! skill_activation.rs by focusing on precondition-trigger interaction, edge
//! cases and unlocking skills during a race.

use crate::common::{
    activation_logics, alternative, alternative_with_precondition, load_course, ready_solver,
    seeded_config, skill, skill_data, uma,
};
use rand::SeedableRng;
use rand_pcg::Pcg64Mcg;
use uma_tools::race::runner::Runner;
use uma_tools::race::solver::RaceSolver;
use uma_tools::skill::*;
use uma_tools::trigger::builder::build_trigger_scheduler;
use uma_tools::trigger::skill_activation::*;
use uma_tools::uma::*;

// ============================================================================
// PRECONDITION-TRIGGER INTERACTION (BUILD-TIME)
// ============================================================================

#[test]
fn test_precondition_with_window_trigger() {
    // Skill 100031: Complex precondition with spatial window trigger
    // Precondition: phase>=2&order<=3&bashin_diff_infront<=1&is_overtake==1
    // Condition: is_last_straight==1
    // Tests: Complex precondition properly separated from spatial window trigger

    let skill = Skill::new(
        100031,
        skill_data(vec![alternative_with_precondition(
            "phase>=2&order<=3&bashin_diff_infront<=1&is_overtake==1",
            "is_last_straight==1",
            SkillType::TargetSpeed,
            0.4,
        )]),
    );

    let course = load_course(10104); // 2000m course
    let mut rng = Pcg64Mcg::seed_from_u64(0);
    let scheduler = build_trigger_scheduler(&[skill], &course, &mut rng);

    // Should create 1 skill logic
    let logics = activation_logics(&scheduler);
    assert_eq!(logics.len(), 1);
    let logic = logics[0];

    // Verify precondition exists and is separate from condition
    assert_eq!(
        logic.precondition,
        Some(0),
        "Precondition should be compiled and stored separately"
    );

    // Verify trigger is a position window (not affected by precondition)
    assert!(
        matches!(
            logic.trigger,
            TriggerCondition::Window {
                variable: WindowVariable::Position,
                ..
            }
        ),
        "Condition should create position window trigger"
    );

    // Only the condition's comparison is a guard
    assert_eq!(logic.guards.len(), 1);

    // Verify precondition_satisfied array is initialized correctly
    assert_eq!(
        scheduler.precondition_satisfied(),
        vec![false],
        "Skill with precondition should start locked"
    );
}

#[test]
fn test_precondition_with_or_condition() {
    // Test: Precondition applies to all OR branches equally
    // Tests: Each OR branch inherits the same precondition

    let alternative = alternative_with_precondition(
        "phase>=2&ground_type==1",                         // Shared precondition
        "distance_rate>=80@hp_per<=50@accumulatetime>=10", // 3 OR branches
        SkillType::TargetSpeed,
        0.4,
    );
    let skill = Skill::new(0, skill_data(vec![alternative]));

    let course = load_course(10104);
    let mut rng = Pcg64Mcg::seed_from_u64(0);
    let scheduler = build_trigger_scheduler(&[skill], &course, &mut rng);

    // Should create 3 branches (OR condition)
    let logics = activation_logics(&scheduler);
    assert_eq!(
        logics.len(),
        3,
        "OR condition with 3 branches should create 3 skill logics"
    );

    // Verify all branches share the same precondition
    for (idx, logic) in logics.iter().enumerate() {
        assert_eq!(
            logic.precondition,
            Some(0),
            "Branch {} should have precondition",
            idx
        );
    }

    // A single precondition, locked at the start
    assert_eq!(scheduler.precondition_satisfied(), vec![false]);
}

#[test]
fn test_multiple_alternatives_precondition_independence() {
    // Test: Multiple alternatives with different preconditions
    // Tests: Each alternative's precondition is stored independently

    let alternatives = vec![
        // Has precondition (phase>=2)
        alternative_with_precondition("phase>=2", "distance_rate>=80", SkillType::TargetSpeed, 0.4),
        // Another precondition (accumulatetime>=10)
        alternative_with_precondition(
            "accumulatetime>=10",
            "hp_per<=50",
            SkillType::TargetSpeed,
            0.3,
        ),
        // No precondition
        alternative("is_last_straight==1", SkillType::TargetSpeed, 0.5),
    ];
    let skill = Skill::new(0, skill_data(alternatives));

    let course = load_course(10104);
    let mut rng = Pcg64Mcg::seed_from_u64(0);
    let scheduler = build_trigger_scheduler(&[skill], &course, &mut rng);

    // Should have 3 skill logics (one per alternative)
    let logics = activation_logics(&scheduler);
    assert_eq!(logics.len(), 3);

    // Alternative 1: has precondition (phase>=2)
    assert_eq!(
        logics[0].precondition,
        Some(0),
        "First alternative should have precondition"
    );

    // Alternative 2: has different precondition (accumulatetime>=10)
    assert_eq!(
        logics[1].precondition,
        Some(1),
        "Second alternative should have its own precondition"
    );

    // Alternative 3: no precondition
    assert!(
        logics[2].precondition.is_none(),
        "Third alternative should not have precondition"
    );

    // Both preconditions start locked
    assert_eq!(
        scheduler.precondition_satisfied(),
        vec![false, false],
        "Precondition states should match alternative requirements"
    );
}

// ============================================================================
// RUNTIME BEHAVIOR
// ============================================================================

/// 2000m medium turf course with 4 corners and no slopes.
const RUNTIME_COURSE: u32 = 10104;

/// Always available target speed skill, locked until `precondition` is
/// satisfied.
fn speed_skill(id: i32, condition: &str, precondition: &str) -> Skill {
    let alternative =
        alternative_with_precondition(precondition, condition, SkillType::TargetSpeed, 0.035);
    Skill::new(id, skill_data(vec![alternative]))
}

/// Ready race with a single runner with the given skills.
fn ready_runner(skills: Vec<Skill>) -> RaceSolver<Runner> {
    ready_solver(RUNTIME_COURSE, uma(Style::Front), skills, seeded_config(42))
}

/// Run a race with a single runner and return where each skill activated.
fn activation_distances(skills: Vec<Skill>) -> Vec<(i32, f64)> {
    let mut solver = ready_runner(skills);
    solver.run();
    solver.runners[0]
        .activated_skills
        .iter()
        .map(|record| (record.skill_id, record.distance))
        .collect()
}

/// Where a skill activated, if it did.
fn activation_distance(activations: &[(i32, f64)], id: i32) -> Option<f64> {
    activations
        .iter()
        .find(|&&(skill_id, _)| skill_id == id)
        .map(|(_, distance)| *distance)
}

#[test]
fn test_precondition_unlocks_skill_once_satisfied() {
    // The condition holds from 10% on, but the skill must wait for Late phase
    let activations = activation_distances(vec![speed_skill(
        4,
        "distance_rate>=10&order<=18",
        "phase>=2",
    )]);

    let late_start = load_course(RUNTIME_COURSE).section_length() * 16.0;
    let distance = activation_distance(&activations, 4).expect("Skill should activate");
    assert!(
        (late_start..late_start + 30.0).contains(&distance),
        "Should activate right after Late phase starts ({late_start}m), got {distance}m"
    );
}

#[test]
fn test_unsatisfied_precondition_blocks_skill() {
    // Early phase and the final corner never overlap
    let activations = activation_distances(vec![speed_skill(
        1,
        "distance_rate>=10&order<=18",
        "phase==0&is_finalcorner==1",
    )]);

    assert_eq!(activation_distance(&activations, 1), None);
}

#[test]
fn test_heal_precondition_unlocks_on_heal_activation() {
    // Skill 110771 (Joy to the World) waits for a heal in the first half of
    // Mid phase (17%-42% of the race)
    let precondition = "phase_firsthalf==1&is_activate_heal_skill==1";
    let joy = || speed_skill(3, "distance_rate>=45&order<=18", precondition);
    let heal = skill(2, "distance_rate>=20", SkillType::Recovery, 0.035);

    let activations = activation_distances(vec![heal, joy()]);
    assert!(activation_distance(&activations, 2).is_some());
    let distance = activation_distance(&activations, 3).expect("Heal should unlock skill");
    assert!(
        (900.0..930.0).contains(&distance),
        "Should activate at 45% (900m), got {distance}m"
    );

    // Without a heal, the skill stays locked
    let activations = activation_distances(vec![joy()]);
    assert_eq!(activation_distance(&activations, 3), None);
}

#[test]
fn test_last_straight_onetime_precondition() {
    // Skill 100911 has `is_last_straight_onetime==1` in its precondition, which
    // can only be checked when entering the last straight
    let activations = activation_distances(vec![speed_skill(
        8,
        "is_last_straight==1&order<=18",
        "phase>=2&is_last_straight_onetime==1",
    )]);

    let straight_start = load_course(RUNTIME_COURSE).straights.last().unwrap().start;
    let distance = activation_distance(&activations, 8).expect("Skill should activate");
    assert!(
        (straight_start..straight_start + 30.0).contains(&distance),
        "Should activate when entering the last straight ({straight_start}m), got {distance}m"
    );
}

#[test]
fn test_static_preconditions_resolved_at_race_start() {
    // Course 10104 is a medium distance (distance_type==3) turf course
    let solver = ready_runner(vec![
        speed_skill(5, "distance_rate>=10&order<=18", "distance_type==3"),
        speed_skill(7, "distance_rate>=10&order<=18", "distance_type==1"),
        speed_skill(6, "distance_rate>=10&order<=18", "ground_type==1&order<=18"),
    ]);
    let scheduler = &solver.runners[0].trigger_scheduler;

    // True static comparisons are removed; unsatisfiable windows are dropped
    let windows: Vec<_> = scheduler
        .precondition_windows()
        .iter()
        .map(|window| &scheduler.skill_logics()[window.logic_idx])
        .collect();
    assert_eq!(windows.len(), 2);
    assert_eq!(windows[0].skill_index, 0);
    assert!(windows[0].guards.is_empty());
    assert_eq!(windows[1].skill_index, 2);
    assert_eq!(windows[1].guards.len(), 1, "Only order<=18 should remain");

    let activations = activation_distances(vec![
        speed_skill(5, "distance_rate>=10&order<=18", "distance_type==3"),
        speed_skill(7, "distance_rate>=10&order<=18", "distance_type==1"),
    ]);
    assert!(activation_distance(&activations, 5).is_some());
    assert_eq!(activation_distance(&activations, 7), None);
}
