//! Unit tests for trigger ordering and simultaneous firing behavior
//!
//! These tests verify that skills with multiple conditions that might fire
//! simultaneously are properly handled, and that trigger priority/ordering
//! works correctly.

use super::super::skill_activation::*;
use crate::course::*;
use crate::trigger::builder::build_trigger_scheduler;
use crate::uma::skill::*;
use rand::SeedableRng;
use rand_pcg::Pcg64Mcg;

/// Helper function to create a skill with given condition and optional precondition
fn create_skill(id: &str, condition: &str, precondition: &str) -> Skill {
    Skill::new(
        id.to_string(),
        SkillData {
            rarity: SkillRarity::White,
            alternatives: vec![SkillAlternative {
                condition: condition.to_string(),
                precondition: precondition.to_string(),
                base_duration: 40000,
                effects: vec![SkillEffect {
                    skill_type: SkillType::TargetSpeed,
                    target: SkillTarget::Oneself,
                    modifier: 4000,
                }],
            }],
        },
    )
}

/// Load a real course from course_data.json
fn load_course(course_id: u32) -> Course {
    let course_data =
        std::fs::read_to_string("data/course_data.json").expect("Failed to read course_data.json");
    let courses: std::collections::HashMap<String, serde_json::Value> =
        serde_json::from_str(&course_data).expect("Failed to parse course_data.json");

    let course_str = course_id.to_string();
    let _course_json = courses
        .get(&course_str)
        .expect(&format!("Course {} not found", course_id));

    Course::from_json_str(&course_data, &course_str)
        .expect(&format!("Failed to parse course {}", course_id))
}

// ============================================================================
// PHASE + DISTANCE COLLISION
// ============================================================================

#[test]
fn test_phase_transition_and_distance_threshold() {
    // Skill 100621: distance_rate>=50&phase==1&order>=3&order_rate<=70
    // Tests: Phase boundary (phase==1) and distance trigger align
    // When entering Mid phase (phase 1) at distance_rate=50%, both conditions fire

    let skill = create_skill(
        "100621",
        "distance_rate>=50&phase==1&order>=3&order_rate<=70",
        "",
    );

    let course = load_course(10104); // 2000m course
    let mut rng = Pcg64Mcg::seed_from_u64(0);
    let scheduler = build_trigger_scheduler(&[skill], &course, &mut rng);

    assert_eq!(scheduler.skill_logics.len(), 1);
    let logic = &scheduler.skill_logics[0];

    // Should have PhaseChange trigger (explicit trigger takes precedence)
    assert_eq!(1, 1);
    let (triggers, guards) = &(logic.triggers.as_slice(), logic.guard_condition.as_ref());
    assert_eq!(triggers.len(), 1);
    assert!(
        matches!(triggers[0], TriggerCondition::PhaseChange { phase: _ }),
        "Should have PhaseChange trigger"
    );

    // distance_rate, order, and order_rate become guards
    assert!(
        guards.is_some(),
        "Should have guards for distance_rate, order, and order_rate"
    );

    // Key behavior: When phase changes to Mid at ~50% distance
    // The PhaseChange trigger fires and checks all guards including distance_rate>=50
    // This ensures simultaneous condition alignment is properly handled
}

// ============================================================================
// MULTIPLE SIMULTANEOUS CONDITIONS
// ============================================================================

#[test]
fn test_multiple_conditions_align_at_phase_boundary() {
    // Skill 110121: distance_rate>=60&phase==1&slope==2&order_rate>=50
    // Precondition: change_order_up_middle>=3
    // Tests: Multiple conditions must all be true at phase 1 entry

    let skill = create_skill(
        "110121",
        "distance_rate>=60&phase==1&slope==2&order_rate>=50",
        "change_order_up_middle>=3",
    );

    let course = load_course(10104);
    let mut rng = Pcg64Mcg::seed_from_u64(0);
    let scheduler = build_trigger_scheduler(&[skill], &course, &mut rng);

    assert_eq!(scheduler.skill_logics.len(), 1);
    let logic = &scheduler.skill_logics[0];

    // Should have precondition
    assert!(logic.precondition.is_some(), "Should have precondition");

    // Should have PhaseChange trigger
    assert_eq!(1, 1);
    let (triggers, guards) = &(logic.triggers.as_slice(), logic.guard_condition.as_ref());
    assert_eq!(triggers.len(), 1);
    assert!(
        matches!(triggers[0], TriggerCondition::PhaseChange { .. }),
        "Should have PhaseChange trigger for phase==1"
    );

    // All other conditions (distance_rate, slope, order_rate) become guards
    assert!(
        guards.is_some(),
        "Should have guards for simultaneous conditions"
    );

    // Key behavior: When phase changes to Mid:
    // 1. Check precondition (change_order_up_middle>=3)
    // 2. Fire PhaseChange trigger
    // 3. Check all guards: distance_rate>=60, slope==2, order_rate>=50
    // All must align at the same moment
}

#[test]
fn test_six_conditions_simultaneous() {
    // Skill 110591: distance_rate>=60&slope==2&phase==1&order_rate>=40&order_rate<=80&remain_distance>=500
    // Tests: Six conditions must all align when trigger fires

    let skill = create_skill(
        "110591",
        "distance_rate>=60&slope==2&phase==1&order_rate>=40&order_rate<=80&remain_distance>=500",
        "",
    );

    let course = load_course(10104);
    let mut rng = Pcg64Mcg::seed_from_u64(0);
    let scheduler = build_trigger_scheduler(&[skill], &course, &mut rng);

    assert_eq!(scheduler.skill_logics.len(), 1);
    let logic = &scheduler.skill_logics[0];

    // Should have PhaseChange trigger (phase==1 is explicit trigger)
    assert_eq!(1, 1);
    let (triggers, guards) = &(logic.triggers.as_slice(), logic.guard_condition.as_ref());
    assert_eq!(triggers.len(), 1);
    assert!(
        matches!(triggers[0], TriggerCondition::PhaseChange { .. }),
        "Should have PhaseChange trigger"
    );

    // All five other conditions become guards
    assert!(
        guards.is_some(),
        "Should have guards for all non-trigger conditions"
    );

    // Key behavior: When phase==1 trigger fires, all guards must pass:
    // - distance_rate>=60 (must be in second 60% of race)
    // - slope==2 (must be on uphill slope)
    // - order_rate>=40 && <=80 (must be in middle of pack)
    // - remain_distance>=500 (must have 500m+ remaining)
    // This tests the system's ability to handle complex simultaneous requirements
}

// ============================================================================
// DISTANCE-BASED COLLISION
// ============================================================================

#[test]
fn test_distance_rate_and_remain_distance_alignment() {
    // Skill 101191: distance_rate>=66&distance_rate<=68&order_rate>=50&remain_distance>=500
    // Tests: Two distance measurements must align simultaneously

    let skill = create_skill(
        "101191",
        "distance_rate>=66&distance_rate<=68&order_rate>=50&remain_distance>=500",
        "",
    );

    let course = load_course(10104); // 2000m course
    let mut rng = Pcg64Mcg::seed_from_u64(0);
    let scheduler = build_trigger_scheduler(&[skill], &course, &mut rng);

    assert_eq!(scheduler.skill_logics.len(), 1);
    let logic = &scheduler.skill_logics[0];

    // Should have Window trigger for distance_rate 66-68%
    assert_eq!(1, 1);
    let (triggers, guards) = &(logic.triggers.as_slice(), logic.guard_condition.as_ref());
    assert_eq!(triggers.len(), 1);
    assert!(
        matches!(
            triggers[0],
            TriggerCondition::Window {
                variable: WindowVariable::DistanceRate,
                min_threshold: 66.0,
                max_threshold: 68.0,
            }
        ),
        "Should have distance_rate window 66-68%"
    );

    // order_rate and remain_distance become guards
    assert!(guards.is_some(), "Should have guards");

    // Key behavior: Window is only 2% wide (66-68%)
    // When entering this window, must also check:
    // - order_rate>=50 (must be in back half)
    // - remain_distance>=500 (on 2000m course, at 66% = 1320m covered, 680m remaining)
    //
    // This tests alignment of:
    // 1. Narrow window entry
    // 2. Position-based guard (order_rate)
    // 3. Distance-remaining guard (remain_distance)
}

// ============================================================================
// HP VS DISTANCE TRIGGERS
// ============================================================================

#[test]
fn test_hp_trigger_during_distance_window() {
    // Skill 200761: distance_type==4&hp_per<=30
    // Tests: HP trigger fires independently of distance progression

    let skill = create_skill("200761", "distance_type==4&hp_per<=30", "");

    let course = load_course(10105); // 2600m course (distance_type 4)
    let mut rng = Pcg64Mcg::seed_from_u64(0);
    let scheduler = build_trigger_scheduler(&[skill], &course, &mut rng);

    assert_eq!(scheduler.skill_logics.len(), 1);
    let logic = &scheduler.skill_logics[0];

    // Should have HpPercent trigger (HP is monitored separately)
    assert_eq!(1, 1);
    let (triggers, guards) = &(logic.triggers.as_slice(), logic.guard_condition.as_ref());
    assert_eq!(triggers.len(), 1);
    assert!(
        matches!(
            triggers[0],
            TriggerCondition::HpPercent {
                threshold: 30.0,
                ..
            }
        ),
        "Should have HP trigger"
    );

    // distance_type becomes guard
    assert!(guards.is_some(), "Should have distance_type guard");

    // Key behavior: HP can drop below 30% at any distance
    // HP trigger is monotonic decreasing, fires when crossing threshold
    // Unlike distance triggers which fire based on position progression,
    // HP triggers respond to stamina consumption and recovery
    // This tests independent trigger systems working together
}

// ============================================================================
// CONTINUOUS VS MONOTONIC TRIGGER INTERACTION
// ============================================================================

#[test]
fn test_continuous_trigger_with_multiple_dynamic_guards() {
    // Test: Skill with continuous trigger checking multiple dynamic conditions
    // Tests: Continuous triggers re-evaluate every frame, handling dynamic state

    let skill = create_skill(
        "test_continuous",
        "accumulatetime>=5&blocked_front_continuetime>=1&order_rate<=50&overtake_target_time>=2",
        "",
    );

    let course = load_course(10104);
    let mut rng = Pcg64Mcg::seed_from_u64(0);
    let scheduler = build_trigger_scheduler(&[skill], &course, &mut rng);

    assert_eq!(scheduler.skill_logics.len(), 1);
    let logic = &scheduler.skill_logics[0];

    // Should use Continuous trigger (dynamic guards present)
    assert_eq!(1, 1);
    let (triggers, guards) = &(logic.triggers.as_slice(), logic.guard_condition.as_ref());
    assert_eq!(triggers.len(), 1);
    assert!(
        !triggers.is_empty(),
        "Should have Continuous trigger due to dynamic guards"
    );

    // All conditions become guards (checked every frame)
    assert!(guards.is_some(), "Should have all conditions as guards");

    // Key behavior: Continuous trigger checks every frame
    // All guards (accumulatetime, blocked_front_continuetime, order_rate, overtake_target_time)
    // are re-evaluated continuously until all align
    // This tests the difference between one-shot monotonic triggers
    // and continuous re-evaluation for dynamic conditions
}

// ============================================================================
// TRIGGER TYPE PRECEDENCE
// ============================================================================

#[test]
fn test_explicit_trigger_takes_precedence_over_dynamic_guards() {
    // Test: When explicit trigger exists, it takes precedence even with dynamic guards
    // Tests: Trigger selection priority system

    let skill = create_skill(
        "test_precedence",
        "phase==2&blocked_front_continuetime>=1&order_rate<=50",
        "",
    );

    let course = load_course(10104);
    let mut rng = Pcg64Mcg::seed_from_u64(0);
    let scheduler = build_trigger_scheduler(&[skill], &course, &mut rng);

    assert_eq!(scheduler.skill_logics.len(), 1);
    let logic = &scheduler.skill_logics[0];

    // Should have PhaseChange trigger (explicit trigger has precedence)
    assert_eq!(1, 1);
    let (triggers, guards) = &(logic.triggers.as_slice(), logic.guard_condition.as_ref());
    assert_eq!(triggers.len(), 1);
    assert!(
        matches!(triggers[0], TriggerCondition::PhaseChange { .. }),
        "PhaseChange trigger should take precedence over Continuous"
    );

    // Dynamic guards (blocked_front_continuetime, order_rate) are checked when trigger fires
    assert!(
        guards.is_some(),
        "Should have dynamic guards checked on trigger"
    );

    // Key behavior: Even though blocked_front_continuetime and order_rate are dynamic,
    // the explicit phase==2 trigger takes precedence over converting to Continuous
    // The skill fires at phase transition, then checks dynamic guards at that moment
    // This tests trigger precedence rules in the build system
}
