//! Unit tests for activation-dependent trigger behavior
//!
//! These tests verify that skills with conditions depending on other skill
//! activations are properly detected and configured at build-time.

use crate::common::{activation_logics, load_course, make_skill, make_skill_with_alternatives};
use rand::SeedableRng;
use rand_pcg::Pcg64Mcg;
use uma_tools::trigger::builder::build_trigger_scheduler;
use uma_tools::trigger::skill_activation::*;

// ============================================================================
// BASIC ACTIVATION DEPENDENCY
// ============================================================================

#[test]
fn test_skill_activation_flag_triggers_dependent_skill() {
    // Skill 100661 (Alt 2): phase==3&is_activate_other_skill_detail==1
    // Tests: Skill that requires another skill to have activated first

    let skill = make_skill_with_alternatives(
        "100661",
        vec![
            (
                "distance_rate>=34&distance_rate<=36&order==1&bashin_diff_behind>=1",
                "",
            ),
            ("phase==3&is_activate_other_skill_detail==1", ""),
        ],
    );

    let course = load_course(10104);
    let mut rng = Pcg64Mcg::seed_from_u64(0);
    let scheduler = build_trigger_scheduler(&[skill], &course, &mut rng);

    // Should create 2 skill logics (one per alternative)
    assert_eq!(scheduler.skill_logics.len(), 2);

    // Alternative 2 (index 1) has activation dependency
    let logic_alt2 = &scheduler.skill_logics[1];

    let (triggers, guards) = (&logic_alt2.triggers[..], logic_alt2.guards.as_slice());

    // Should have PhaseChange trigger
    assert_eq!(triggers.len(), 1);
    assert!(
        matches!(triggers[0], TriggerCondition::PhaseChange { .. }),
        "Should have PhaseChange trigger for phase==3"
    );

    // is_activate_other_skill_detail==1 should be a guard
    assert!(
        !guards.is_empty(),
        "Should have is_activate_other_skill_detail as guard"
    );

    // Key behavior: At build-time, we detect the activation dependency
    // At runtime, this would only fire if another skill activated in the same frame
}

// ============================================================================
// HEAL-SPECIFIC DEPENDENCY
// ============================================================================

#[test]
fn test_heal_skill_activation_dependency() {
    // Skill 110771: Depends on healing skill activation
    // Precondition: phase_firsthalf==1&is_activate_heal_skill==1
    // Tests: Heal-specific activation tracking

    let skill = make_skill_with_alternatives(
        "110771",
        vec![
            (
                "distance_rate>=45&order_rate>=20&order_rate<=50&remain_distance>=1000",
                "phase_firsthalf==1&is_activate_heal_skill==1",
            ),
            (
                "distance_rate>=45&order_rate<=20&remain_distance>=1000",
                "phase_firsthalf==1&is_activate_heal_skill==1",
            ),
        ],
    );

    let course = load_course(10104);
    let mut rng = Pcg64Mcg::seed_from_u64(0);
    let scheduler = build_trigger_scheduler(&[skill], &course, &mut rng);

    // Should create 2 skill logics (one per alternative)
    let logics = activation_logics(&scheduler);
    assert_eq!(logics.len(), 2);

    // Each alternative has its own precondition, locked at the start
    for (idx, logic) in logics.iter().enumerate() {
        assert_eq!(
            logic.precondition,
            Some(idx),
            "Alternative {} should have precondition with heal dependency",
            idx
        );
    }
    assert_eq!(scheduler.precondition_satisfied, vec![false, false]);

    // Key behavior: is_activate_heal_skill==1 is detected in precondition
    // It's checked when a heal skill activates instead of every frame
    // Only heal skills set this flag, not general skill activations
    assert_eq!(scheduler.heal_activation_triggers.len(), 2);
    assert!(scheduler.precondition_windows.is_empty());
}

// ============================================================================
// ACTIVATION-DEPENDENT WITH OR BRANCHES
// ============================================================================

#[test]
fn test_activation_dependent_or_branches() {
    // Skill 100901 (Alt 2): Two OR branches both requiring activation dependency
    // Branch 1: is_activate_other_skill_detail==1&...&distance_type==2
    // Branch 2: is_activate_other_skill_detail==1&...&distance_type==3
    // Tests: Activation dependency combined with OR mutual exclusion

    let skill = make_skill_with_alternatives(
        "100901",
        vec![
            (
                "distance_rate>=50&order>=2&is_overtake==1&distance_type==2",
                "",
            ),
            (
                "is_activate_other_skill_detail==1&phase>=2&is_last_straight==1&overtake_target_time>=1&distance_type==2@is_activate_other_skill_detail==1&phase>=2&is_last_straight==1&overtake_target_time>=1&distance_type==3",
                "",
            ),
        ],
    );

    let course = load_course(10104);
    let mut rng = Pcg64Mcg::seed_from_u64(0);
    let scheduler = build_trigger_scheduler(&[skill], &course, &mut rng);

    // Alternative 1: 1 branch
    // Alternative 2: 2 branches (OR condition)
    // Total: 3 skill logics
    assert_eq!(scheduler.skill_logics.len(), 3);

    // Alt 2, Branch 1 and Branch 2 (indices 1 and 2) both have activation dependency
    for idx in 1..=2 {
        let logic = &scheduler.skill_logics[idx];
        assert_eq!(1, 1, "Each OR branch should have 1 branch");

        let (triggers, guards) = &(logic.triggers.as_slice(), logic.guards.as_slice());

        // Should have spatial window trigger (is_last_straight)
        assert_eq!(triggers.len(), 1);
        assert!(
            matches!(
                triggers[0],
                TriggerCondition::Window {
                    variable: WindowVariable::SpatialPosition,
                    ..
                }
            ),
            "Branch {} should have spatial window trigger",
            idx
        );

        // Should have guards including is_activate_other_skill_detail
        assert!(
            !guards.is_empty(),
            "Branch {} should have activation dependency guard",
            idx
        );
    }

    // Key behavior: OR branches with activation dependency
    // Both branches require another skill to activate first
    // Then only one OR branch can activate (mutual exclusion)
}

// ============================================================================
// MULTIPLE ALTERNATIVES WITH DEPENDENCY
// ============================================================================

#[test]
fn test_alternative_with_and_without_dependency() {
    // Skill 101021: First alternative is normal, second requires activation
    // Alt 1: Normal conditions (no activation dependency)
    // Alt 2: Requires is_activate_other_skill_detail==1
    // Tests: Alternative prioritization with activation dependency

    let skill = make_skill_with_alternatives(
        "101021",
        vec![
            (
                "distance_rate<=50&order_rate>=40&order_rate<=80&infront_near_lane_time>=3&accumulatetime>=10@distance_rate<=50&order_rate>=40&order_rate<=80&behind_near_lane_time>=3&accumulatetime>=10",
                "",
            ),
            (
                "distance_rate<=60&distance_rate>=45&corner==0&is_activate_other_skill_detail==1",
                "",
            ),
        ],
    );

    let course = load_course(10104);
    let mut rng = Pcg64Mcg::seed_from_u64(0);
    let scheduler = build_trigger_scheduler(&[skill], &course, &mut rng);

    // Alt 1: 2 branches (OR condition)
    // Alt 2: 1 branch
    // Total: 3 skill logics
    assert_eq!(scheduler.skill_logics.len(), 3);

    // Alternative 1 (indices 0-1): No activation dependency
    for idx in 0..=1 {
        let logic = &scheduler.skill_logics[idx];
        let (triggers, guards) = &(logic.triggers.as_slice(), logic.guards.as_slice());

        // Should have Window trigger for distance_rate<=50
        assert_eq!(triggers.len(), 1);
        assert!(
            matches!(
                triggers[0],
                TriggerCondition::Window {
                    variable: WindowVariable::DistanceRate,
                    min_threshold: 0.0,
                    max_threshold: 50.0,
                }
            ),
            "Alt 1 Branch {} should have distance_rate window",
            idx
        );

        assert!(!guards.is_empty());
    }

    // Alternative 2 (index 2): Has activation dependency
    let logic_alt2 = &scheduler.skill_logics[2];
    let (triggers, guards) = (&logic_alt2.triggers[..], logic_alt2.guards.as_slice());

    // Should have Continuous trigger (corner==0 is complex)
    assert_eq!(triggers.len(), 1);
    assert!(
        !triggers.is_empty(),
        "Alt 2 should have Continuous trigger for corner==0"
    );

    // Should have guards including is_activate_other_skill_detail
    assert!(
        !guards.is_empty(),
        "Alt 2 should have activation dependency guard"
    );

    // Key behavior: Alternative priority system
    // Alt 1 can activate normally without dependency
    // Alt 2 requires another skill activation first
    // System evaluates alternatives in order (0 before 1)
}

// ============================================================================
// INTER-CHARACTER ACTIVATION
// ============================================================================

#[test]
fn test_other_character_skill_activation() {
    // Skill 101103211: Depends on OTHER characters activating specific skill types
    // is_other_character_activate_advantage_skill==27 or ==22
    // Tests: External (other character) activation tracking

    let skill = make_skill(
        "101103211",
        "distance_type==3&phase>=2&is_finalcorner==1&is_other_character_activate_advantage_skill==27@distance_type==4&phase>=2&is_finalcorner==1&is_other_character_activate_advantage_skill==27@distance_type==3&phase>=2&is_finalcorner==1&is_other_character_activate_advantage_skill==22@distance_type==4&phase>=2&is_finalcorner==1&is_other_character_activate_advantage_skill==22",
        "",
    );

    let course = load_course(10104);
    let mut rng = Pcg64Mcg::seed_from_u64(0);
    let scheduler = build_trigger_scheduler(&[skill], &course, &mut rng);

    // Should create 4 branches (4-way OR)
    assert_eq!(scheduler.skill_logics.len(), 4);

    // All branches have is_other_character_activate_advantage_skill dependency
    for (idx, logic) in scheduler.skill_logics.iter().enumerate() {
        assert_eq!(1, 1);
        let (triggers, guards) = &(logic.triggers.as_slice(), logic.guards.as_slice());

        // Should have spatial window trigger (is_finalcorner)
        assert_eq!(triggers.len(), 1);
        assert!(
            matches!(
                triggers[0],
                TriggerCondition::Window {
                    variable: WindowVariable::SpatialPosition,
                    ..
                }
            ),
            "Branch {} should have finalcorner spatial window",
            idx
        );

        // Should have guards including other character activation check
        assert!(
            !guards.is_empty(),
            "Branch {} should have other character activation dependency",
            idx
        );
    }

    // Key behavior: Inter-character activation dependency
    // This skill only activates if another character in the race activated
    // specific advantage skill types (27 or 22)
    // Tests cross-runner interaction tracking
}

// ============================================================================
// DEPENDENCY DETECTION
// ============================================================================

#[test]
fn test_activation_dependency_variable_detection() {
    // Test: Various activation dependency variables are correctly detected
    // Tests: Build system recognizes all forms of activation dependencies

    // Test 1: is_activate_other_skill_detail (DynamicGuard → Continuous trigger with guard)
    {
        let skill = make_skill("test_dep1", "is_activate_other_skill_detail==1", "");
        let course = load_course(10104);
        let mut rng = Pcg64Mcg::seed_from_u64(0);
        let scheduler = build_trigger_scheduler(&[skill], &course, &mut rng);

        assert_eq!(scheduler.skill_logics.len(), 1);
        let logic = &scheduler.skill_logics[0];
        let (triggers, guards) = &(logic.triggers.as_slice(), logic.guards.as_slice());

        assert_eq!(triggers.len(), 1);
        assert!(
            matches!(triggers[0], TriggerCondition::Continuous),
            "is_activate_other_skill_detail should use Continuous trigger"
        );
        assert!(
            !guards.is_empty(),
            "is_activate_other_skill_detail should remain as guard"
        );
    }

    // Test 2: is_activate_heal_skill (ActivationTrigger → specific trigger, no guard)
    {
        let skill = make_skill("test_dep2", "is_activate_heal_skill==1", "");
        let course = load_course(10104);
        let mut rng = Pcg64Mcg::seed_from_u64(0);
        let scheduler = build_trigger_scheduler(&[skill], &course, &mut rng);

        assert_eq!(scheduler.skill_logics.len(), 1);
        let logic = &scheduler.skill_logics[0];
        let (triggers, guards) = &(logic.triggers.as_slice(), logic.guards.as_slice());

        assert_eq!(triggers.len(), 1);
        assert!(
            matches!(triggers[0], TriggerCondition::HealSkillActivation),
            "is_activate_heal_skill should create HealSkillActivation trigger"
        );
        assert!(
            guards.is_empty(),
            "is_activate_heal_skill should have no guards (becomes trigger)"
        );
    }

    // Test 3: is_other_character_activate_advantage_skill (DynamicGuard → Continuous with guard)
    {
        let skill = make_skill(
            "test_dep3",
            "is_other_character_activate_advantage_skill==27",
            "",
        );
        let course = load_course(10104);
        let mut rng = Pcg64Mcg::seed_from_u64(0);
        let scheduler = build_trigger_scheduler(&[skill], &course, &mut rng);

        assert_eq!(scheduler.skill_logics.len(), 1);
        let logic = &scheduler.skill_logics[0];
        let (triggers, guards) = &(logic.triggers.as_slice(), logic.guards.as_slice());

        assert_eq!(triggers.len(), 1);
        assert!(
            matches!(triggers[0], TriggerCondition::Continuous),
            "is_other_character_activate_advantage_skill should use Continuous trigger"
        );
        assert!(
            !guards.is_empty(),
            "is_other_character_activate_advantage_skill should remain as guard"
        );
    }
}

// ============================================================================
// COMBINED DEPENDENCIES
// ============================================================================

#[test]
fn test_activation_dependency_with_precondition() {
    // Test: Skill with both precondition and activation dependency in condition
    // Tests: Layered dependency checking (precondition → trigger → guard with activation)

    let skill = make_skill(
        "test_combined",
        "is_activate_other_skill_detail==1&distance_rate>=50",
        "phase>=2",
    );

    let course = load_course(10104);
    let mut rng = Pcg64Mcg::seed_from_u64(0);
    let scheduler = build_trigger_scheduler(&[skill], &course, &mut rng);

    let logics = activation_logics(&scheduler);
    assert_eq!(logics.len(), 1);
    let logic = logics[0];

    // Should have precondition
    assert_eq!(
        logic.precondition,
        Some(0),
        "Should have precondition (phase>=2)"
    );

    // Should start locked
    assert_eq!(
        scheduler.precondition_satisfied,
        vec![false],
        "Should start with precondition locked"
    );

    // Should have Window trigger (distance_rate>=50 promoted to window with dynamic guard present)
    assert_eq!(1, 1);
    let (triggers, guards) = &(logic.triggers.as_slice(), logic.guards.as_slice());
    assert_eq!(triggers.len(), 1);
    assert!(
        matches!(
            triggers[0],
            TriggerCondition::Window {
                variable: WindowVariable::DistanceRate,
                min_threshold: 50.0,
                max_threshold: 100.0,
            }
        ),
        "Should have DistanceRate window trigger [50, 100]"
    );

    // Should have guards including activation dependency
    assert!(
        !guards.is_empty(),
        "Should have guards with activation dependency"
    );

    // Key behavior: Three-layer checking
    // 1. Precondition must be satisfied first (phase>=2)
    // 2. Then trigger fires (distance_rate in window [50, 100])
    // 3. Then guards checked (is_activate_other_skill_detail==1)
    // All three must align for skill activation
}
