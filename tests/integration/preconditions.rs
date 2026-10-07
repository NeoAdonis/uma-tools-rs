//! Tests for precondition build-time and runtime behavior
//!
//! These tests complement the existing precondition tests in skill_activation.rs
//! by focusing on precondition-trigger interaction, edge cases and unlocking skills
//! during a race.

use crate::common::{activation_logics, load_course, make_skill, race_params, seeded_config};
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

    let skill = make_skill(
        "100031",
        "is_last_straight==1",
        "phase>=2&order<=3&bashin_diff_infront<=1&is_overtake==1",
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
        scheduler.precondition_satisfied,
        vec![false],
        "Skill with precondition should start locked"
    );
}

#[test]
fn test_precondition_with_or_condition() {
    // Test: Precondition applies to all OR branches equally
    // Tests: Each OR branch inherits the same precondition

    let skill = make_skill(
        "test_or_precond",
        "distance_rate>=80@hp_per<=50@accumulatetime>=10", // 3 OR branches
        "phase>=2&ground_type==1",                         // Shared precondition
    );

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
    assert_eq!(scheduler.precondition_satisfied, vec![false]);
}

#[test]
fn test_multiple_alternatives_precondition_independence() {
    // Test: Multiple alternatives with different preconditions
    // Tests: Each alternative's precondition is stored independently

    let skill = Skill::new(
        "test_multi_alt_precond".to_string(),
        SkillData {
            rarity: SkillRarity::White,
            alternatives: vec![
                // Has precondition (phase>=2)
                SkillAlternative {
                    condition: "distance_rate>=80".parse().unwrap(),
                    precondition: Some("phase>=2".parse().unwrap()),
                    base_duration: 40000,
                    base_cooldown: DEFAULT_COOLDOWN,
                    effects: vec![SkillEffect {
                        skill_type: SkillType::TargetSpeed,
                        target: SkillTarget::Oneself,
                        modifier: 4000,
                    }],
                },
                // Another precondition (accumulatetime>=10)
                SkillAlternative {
                    condition: "hp_per<=50".parse().unwrap(),
                    precondition: Some("accumulatetime>=10".parse().unwrap()),
                    base_duration: 40000,
                    base_cooldown: DEFAULT_COOLDOWN,
                    effects: vec![SkillEffect {
                        skill_type: SkillType::TargetSpeed,
                        target: SkillTarget::Oneself,
                        modifier: 3000,
                    }],
                },
                // No precondition
                SkillAlternative {
                    condition: "is_last_straight==1".parse().unwrap(),
                    precondition: None,
                    base_duration: 40000,
                    base_cooldown: DEFAULT_COOLDOWN,
                    effects: vec![SkillEffect {
                        skill_type: SkillType::TargetSpeed,
                        target: SkillTarget::Oneself,
                        modifier: 5000,
                    }],
                },
            ],
            popularity: None,
            wisdom_check: true,
        },
    );

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
        scheduler.precondition_satisfied,
        vec![false, false],
        "Precondition states should match alternative requirements"
    );
}

// ============================================================================
// RUNTIME BEHAVIOR
// ============================================================================

/// 2000m medium turf course with 4 corners and no slopes.
const RUNTIME_COURSE: u32 = 10104;

/// Build a skill with a single alternative that always passes its wisdom check.
fn guaranteed_skill(id: &str, condition: &str, precondition: &str, effect: SkillType) -> Skill {
    Skill::new(
        id.to_string(),
        SkillData {
            rarity: SkillRarity::White,
            alternatives: vec![SkillAlternative {
                condition: condition.parse().unwrap(),
                precondition: (!precondition.is_empty()).then(|| precondition.parse().unwrap()),
                base_duration: 30000,
                base_cooldown: DEFAULT_COOLDOWN,
                effects: vec![SkillEffect {
                    skill_type: effect,
                    target: SkillTarget::Oneself,
                    modifier: 350,
                }],
            }],
            popularity: None,
            wisdom_check: false,
        },
    )
}

/// Build a target speed skill that always passes its wisdom check.
fn speed_skill(id: &str, condition: &str, precondition: &str) -> Skill {
    guaranteed_skill(id, condition, precondition, SkillType::TargetSpeed)
}

/// Get a single ready runner with the given skills.
fn ready_runner(skills: Vec<Skill>) -> (RaceSolver<Runner>, usize) {
    let uma = Uma {
        speed: 1200,
        stamina: 1000,
        power: 1000,
        guts: 800,
        wiz: 1000,
        style: Style::Front,
        distance_aptitude: Aptitude::A,
        track_aptitude: Aptitude::A,
        style_aptitude: Aptitude::A,
    };
    let mut solver = RaceSolver::new(
        load_course(RUNTIME_COURSE),
        race_params(),
        &seeded_config(42),
    );
    solver.add_runner(uma, Motivation::Normal, skills);
    (solver.ready_runners(), 0)
}

/// Run a race with a single runner and return where each skill activated.
fn activation_distances(skills: Vec<Skill>) -> Vec<(String, f64)> {
    let (mut solver, runner) = ready_runner(skills);
    solver.run();
    solver.runners[runner]
        .activated_skills
        .iter()
        .map(|record| (record.skill_id.clone(), record.distance))
        .collect()
}

/// Where a skill activated, if it did.
fn activation_distance(activations: &[(String, f64)], id: &str) -> Option<f64> {
    activations
        .iter()
        .find(|(skill_id, _)| skill_id == id)
        .map(|(_, distance)| *distance)
}

#[test]
fn test_precondition_unlocks_skill_once_satisfied() {
    // The condition holds from 10% on, but the skill must wait for Late phase
    let activations = activation_distances(vec![speed_skill(
        "late",
        "distance_rate>=10&order<=18",
        "phase>=2",
    )]);

    let late_start = load_course(RUNTIME_COURSE).section_length() * 16.0;
    let distance = activation_distance(&activations, "late").expect("Skill should activate");
    assert!(
        (late_start..late_start + 30.0).contains(&distance),
        "Should activate right after Late phase starts ({late_start}m), got {distance}m"
    );
}

#[test]
fn test_unsatisfied_precondition_blocks_skill() {
    // Early phase and the final corner never overlap
    let activations = activation_distances(vec![speed_skill(
        "blocked",
        "distance_rate>=10&order<=18",
        "phase==0&is_finalcorner==1",
    )]);

    assert_eq!(activation_distance(&activations, "blocked"), None);
}

#[test]
fn test_heal_precondition_unlocks_on_heal_activation() {
    // Skill 110771 (Joy to the World) waits for a heal in the first half of
    // Mid phase (17%-42% of the race)
    let precondition = "phase_firsthalf==1&is_activate_heal_skill==1";
    let joy = || speed_skill("joy", "distance_rate>=45&order<=18", precondition);
    let heal = guaranteed_skill("heal", "distance_rate>=20", "", SkillType::Recovery);

    let activations = activation_distances(vec![heal, joy()]);
    assert!(activation_distance(&activations, "heal").is_some());
    let distance = activation_distance(&activations, "joy").expect("Heal should unlock skill");
    assert!(
        (900.0..930.0).contains(&distance),
        "Should activate at 45% (900m), got {distance}m"
    );

    // Without a heal, the skill stays locked
    let activations = activation_distances(vec![joy()]);
    assert_eq!(activation_distance(&activations, "joy"), None);
}

#[test]
fn test_last_straight_onetime_precondition() {
    // Skill 100911 has `is_last_straight_onetime==1` in its precondition, which
    // can only be checked when entering the last straight
    let activations = activation_distances(vec![speed_skill(
        "straight",
        "is_last_straight==1&order<=18",
        "phase>=2&is_last_straight_onetime==1",
    )]);

    let straight_start = load_course(RUNTIME_COURSE).straights.last().unwrap().start;
    let distance = activation_distance(&activations, "straight").expect("Skill should activate");
    assert!(
        (straight_start..straight_start + 30.0).contains(&distance),
        "Should activate when entering the last straight ({straight_start}m), got {distance}m"
    );
}

#[test]
fn test_static_preconditions_resolved_at_race_start() {
    // Course 10104 is a medium distance (distance_type==3) turf course
    let (solver, runner) = ready_runner(vec![
        speed_skill("medium", "distance_rate>=10&order<=18", "distance_type==3"),
        speed_skill("short", "distance_rate>=10&order<=18", "distance_type==1"),
        speed_skill(
            "mixed",
            "distance_rate>=10&order<=18",
            "ground_type==1&order<=18",
        ),
    ]);
    let scheduler = &solver.runners[runner].trigger_scheduler;

    // True static comparisons are removed; unsatisfiable windows are dropped
    let windows: Vec<_> = scheduler
        .precondition_windows
        .iter()
        .map(|window| &scheduler.skill_logics[window.logic_idx])
        .collect();
    assert_eq!(windows.len(), 2);
    assert_eq!(windows[0].skill_index, 0);
    assert!(windows[0].guards.is_empty());
    assert_eq!(windows[1].skill_index, 2);
    assert_eq!(windows[1].guards.len(), 1, "Only order<=18 should remain");

    let activations = activation_distances(vec![
        speed_skill("medium", "distance_rate>=10&order<=18", "distance_type==3"),
        speed_skill("short", "distance_rate>=10&order<=18", "distance_type==1"),
    ]);
    assert!(activation_distance(&activations, "medium").is_some());
    assert_eq!(activation_distance(&activations, "short"), None);
}
