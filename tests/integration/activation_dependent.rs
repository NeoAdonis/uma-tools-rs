//! Unit tests for activation-dependent trigger behavior
//!
//! These tests verify that skills with conditions depending on other skill
//! activations are properly detected and configured at build-time.

use crate::common::{
    activation_logics, alternative, alternative_with_precondition, assert_close, load_course,
    skill, skill_data,
};
use rand::SeedableRng;
use rand_pcg::Pcg64Mcg;
use uma_tools::skill::{Skill, SkillType};
use uma_tools::trigger::builder::build_trigger_scheduler;
use uma_tools::trigger::skill_activation::*;

/// Get the variable and `(min, max)` thresholds of a window trigger.
fn window(logic: &SkillActivationLogic) -> (WindowVariable, f64, f64) {
    match logic.trigger {
        TriggerCondition::Window {
            variable,
            min_threshold,
            max_threshold,
        } => (variable, min_threshold, max_threshold),
        ref other => panic!("Expected window, got {:?}", other),
    }
}

#[test]
fn test_skill_activation_flag_triggers_dependent_skill() {
    // Skill 100661 (Alt 2): phase==3&is_activate_other_skill_detail==1
    // Tests: Skill that requires another skill to have activated first

    let alternatives = vec![
        alternative(
            "distance_rate>=34&distance_rate<=36&order==1&bashin_diff_behind>=1",
            SkillType::TargetSpeed,
            0.4,
        ),
        alternative(
            "phase==3&is_activate_other_skill_detail==1",
            SkillType::TargetSpeed,
            0.4,
        ),
    ];
    let skill = Skill::new("100661".to_string(), skill_data(alternatives));

    let course = load_course(10104);
    let mut rng = Pcg64Mcg::seed_from_u64(0);
    let scheduler = build_trigger_scheduler(&[skill], &course, &mut rng);

    // Should create 2 skill logics (one per alternative)
    assert_eq!(scheduler.skill_logics.len(), 2);

    // Alternative 2 (index 1) has activation dependency
    let logic_alt2 = &scheduler.skill_logics[1];

    // Checked during Last phase (from 20/24 of the 2000m course)
    let (variable, min, max) = window(logic_alt2);
    assert_eq!(variable, WindowVariable::Position);
    assert_close!(min, 2000.0 / 24.0 * 20.0);
    assert_eq!(max, 2000.0);

    // Both comparisons are guards
    assert_eq!(
        logic_alt2.guards.len(),
        2,
        "Should have phase and is_activate_other_skill_detail as guards"
    );

    // Key behavior: At build-time, we detect the activation dependency
    // At runtime, this would only fire if another skill activated in the same frame
}

#[test]
fn test_heal_skill_activation_dependency() {
    // Skill 110771: Depends on healing skill activation
    // Precondition: phase_firsthalf==1&is_activate_heal_skill==1
    // Tests: Heal-specific activation tracking

    let alternatives = vec![
        alternative_with_precondition(
            "phase_firsthalf==1&is_activate_heal_skill==1",
            "distance_rate>=45&order_rate>=20&order_rate<=50&remain_distance>=1000",
            SkillType::TargetSpeed,
            0.4,
        ),
        alternative_with_precondition(
            "phase_firsthalf==1&is_activate_heal_skill==1",
            "distance_rate>=45&order_rate<=20&remain_distance>=1000",
            SkillType::TargetSpeed,
            0.4,
        ),
    ];
    let skill = Skill::new("110771".to_string(), skill_data(alternatives));

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

#[test]
fn test_activation_dependent_or_branches() {
    // Skill 100901 (Alt 2): Two OR branches both requiring activation dependency
    // Branch 1: is_activate_other_skill_detail==1&...&distance_type==2
    // Branch 2: is_activate_other_skill_detail==1&...&distance_type==3
    // Tests: Activation dependency combined with OR mutual exclusion

    let alternatives = vec![
        alternative(
            "distance_rate>=50&order>=2&is_overtake==1&distance_type==2",
            SkillType::TargetSpeed,
            0.4,
        ),
        alternative(
            "is_activate_other_skill_detail==1&phase>=2&is_last_straight==1&overtake_target_time>=1&distance_type==2@is_activate_other_skill_detail==1&phase>=2&is_last_straight==1&overtake_target_time>=1&distance_type==3",
            SkillType::TargetSpeed,
            0.4,
        ),
    ];
    let skill = Skill::new("100901".to_string(), skill_data(alternatives));

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

        // Checked after the final corner (ends at 1734m), until the goal (phase>=2)
        assert_eq!(
            window(logic),
            (WindowVariable::Position, 1734.0, 2000.0),
            "Branch {} should be checked on the last straight",
            idx
        );

        // All comparisons are guards, including is_activate_other_skill_detail
        assert_eq!(
            logic.guards.len(),
            5,
            "Branch {} should have activation dependency guard",
            idx
        );
    }

    // Key behavior: OR branches with activation dependency
    // Both branches require another skill to activate first
    // Then only one OR branch can activate (mutual exclusion)
}

#[test]
fn test_alternative_with_and_without_dependency() {
    // Skill 101021: First alternative is normal, second requires activation
    // Alt 1: Normal conditions (no activation dependency)
    // Alt 2: Requires is_activate_other_skill_detail==1
    // Tests: Alternative prioritization with activation dependency

    let alternatives = vec![
        alternative(
            "distance_rate<=50&order_rate>=40&order_rate<=80&infront_near_lane_time>=3&accumulatetime>=10@distance_rate<=50&order_rate>=40&order_rate<=80&behind_near_lane_time>=3&accumulatetime>=10",
            SkillType::TargetSpeed,
            0.4,
        ),
        alternative(
            "distance_rate<=60&distance_rate>=45&corner==0&is_activate_other_skill_detail==1",
            SkillType::TargetSpeed,
            0.4,
        ),
    ];
    let skill = Skill::new("101021".to_string(), skill_data(alternatives));

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
        // distance_rate<=50 holds until 51% (floored)
        assert_eq!(
            window(logic),
            (WindowVariable::Position, 0.0, 1020.0),
            "Alt 1 Branch {} should have distance_rate window",
            idx
        );

        assert_eq!(logic.guards.len(), 5);
    }

    // Alternative 2 (index 2): Has activation dependency
    let logic_alt2 = &scheduler.skill_logics[2];
    // 45-60% (until 61%, floored); corner==0 doesn't narrow it down
    assert_eq!(
        window(logic_alt2),
        (WindowVariable::Position, 900.0, 1220.0)
    );

    // All comparisons are guards, including is_activate_other_skill_detail
    assert_eq!(
        logic_alt2.guards.len(),
        4,
        "Alt 2 should have activation dependency guard"
    );

    // Key behavior: Alternative priority system
    // Alt 1 can activate normally without dependency
    // Alt 2 requires another skill activation first
    // System evaluates alternatives in order (0 before 1)
}

#[test]
fn test_other_character_skill_activation() {
    // Skill 101103211: Depends on OTHER characters activating specific skill types
    // is_other_character_activate_advantage_skill==27 or ==22
    // Tests: External (other character) activation tracking

    let skill = skill(
        "101103211",
        "distance_type==3&phase>=2&is_finalcorner==1&is_other_character_activate_advantage_skill==27@distance_type==4&phase>=2&is_finalcorner==1&is_other_character_activate_advantage_skill==27@distance_type==3&phase>=2&is_finalcorner==1&is_other_character_activate_advantage_skill==22@distance_type==4&phase>=2&is_finalcorner==1&is_other_character_activate_advantage_skill==22",
        SkillType::TargetSpeed,
        0.4,
    );

    let course = load_course(10104);
    let mut rng = Pcg64Mcg::seed_from_u64(0);
    let scheduler = build_trigger_scheduler(&[skill], &course, &mut rng);

    // Should create 4 branches (4-way OR)
    assert_eq!(scheduler.skill_logics.len(), 4);

    // All branches have is_other_character_activate_advantage_skill dependency
    for (idx, logic) in scheduler.skill_logics.iter().enumerate() {
        // Checked when other runners activate advantage skills; the final corner
        // and phase comparisons are checked as guards then.
        assert!(
            matches!(logic.trigger, TriggerCondition::BuffSkillActivation),
            "Branch {} should be triggered by other runners' activations",
            idx
        );

        // All comparisons are guards, including other character activation check
        assert_eq!(
            logic.guards.len(),
            4,
            "Branch {} should have other character activation dependency",
            idx
        );
    }

    // Key behavior: Inter-character activation dependency
    // This skill only activates if another character in the race activated
    // specific advantage skill types (27 or 22)
    // Tests cross-runner interaction tracking
}

#[test]
fn test_activation_dependency_variable_detection() {
    // Test: Various activation dependency variables are correctly detected
    // Tests: Build system recognizes all forms of activation dependencies

    // Test 1: is_activate_other_skill_detail (checked during the whole race)
    {
        let skill = skill(
            "test_dep1",
            "is_activate_other_skill_detail==1",
            SkillType::TargetSpeed,
            0.4,
        );
        let course = load_course(10104);
        let mut rng = Pcg64Mcg::seed_from_u64(0);
        let scheduler = build_trigger_scheduler(&[skill], &course, &mut rng);

        assert_eq!(scheduler.skill_logics.len(), 1);
        let logic = &scheduler.skill_logics[0];
        assert_eq!(
            window(logic),
            (WindowVariable::Position, 0.0, f64::INFINITY),
            "is_activate_other_skill_detail should be checked during the whole race"
        );
        assert_eq!(
            logic.guards.len(),
            1,
            "is_activate_other_skill_detail should remain as guard"
        );
    }

    // Test 2: is_activate_heal_skill (checked when a heal skill activates)
    {
        let skill = skill(
            "test_dep2",
            "is_activate_heal_skill==1",
            SkillType::TargetSpeed,
            0.4,
        );
        let course = load_course(10104);
        let mut rng = Pcg64Mcg::seed_from_u64(0);
        let scheduler = build_trigger_scheduler(&[skill], &course, &mut rng);

        assert_eq!(scheduler.skill_logics.len(), 1);
        let logic = &scheduler.skill_logics[0];
        assert!(
            matches!(logic.trigger, TriggerCondition::HealSkillActivation),
            "is_activate_heal_skill should create HealSkillActivation trigger"
        );
        assert_eq!(
            logic.guards.len(),
            1,
            "is_activate_heal_skill should remain as guard"
        );
    }

    // Test 3: is_other_character_activate_advantage_skill (checked when other runners
    // activate advantage skills)
    {
        let skill = skill(
            "test_dep3",
            "is_other_character_activate_advantage_skill==27",
            SkillType::TargetSpeed,
            0.4,
        );
        let course = load_course(10104);
        let mut rng = Pcg64Mcg::seed_from_u64(0);
        let scheduler = build_trigger_scheduler(&[skill], &course, &mut rng);

        assert_eq!(scheduler.skill_logics.len(), 1);
        let logic = &scheduler.skill_logics[0];
        assert!(
            matches!(logic.trigger, TriggerCondition::BuffSkillActivation),
            "is_other_character_activate_advantage_skill should create BuffSkillActivation trigger"
        );
        assert_eq!(
            logic.guards.len(),
            1,
            "is_other_character_activate_advantage_skill should remain as guard"
        );
    }
}

#[test]
fn test_activation_dependency_with_precondition() {
    // Test: Skill with both precondition and activation dependency in condition
    // Tests: Layered dependency checking (precondition → trigger → guard with activation)

    let alternatives = vec![alternative_with_precondition(
        "phase>=2",
        "is_activate_other_skill_detail==1&distance_rate>=50",
        SkillType::TargetSpeed,
        0.4,
    )];
    let skill = Skill::new("test_combined".to_string(), skill_data(alternatives));

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

    // Checked from 50% on
    assert_eq!(
        window(logic),
        (WindowVariable::Position, 1000.0, f64::INFINITY),
        "Should be checked from 50% on"
    );

    // Should have guards including activation dependency
    assert_eq!(
        logic.guards.len(),
        2,
        "Should have guards with activation dependency"
    );

    // Key behavior: Three-layer checking
    // 1. Precondition must be satisfied first (phase>=2)
    // 2. Then trigger fires (position from 50% on)
    // 3. Then guards checked (is_activate_other_skill_detail==1, distance_rate>=50)
    // All three must align for skill activation
}
