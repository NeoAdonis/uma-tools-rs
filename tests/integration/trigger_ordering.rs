//! Unit tests for conditions whose comparisons must all hold at the same time.
//!
//! Every comparison of a branch is a guard; the trigger only narrows down when
//! the guards are checked, to the part of the race where all of them can be
//! true.

use crate::common::{activation_logics, load_course, make_skill};
use rand::SeedableRng;
use rand_pcg::Pcg64Mcg;
use uma_tools::skill::Skill;
use uma_tools::trigger::builder::build_trigger_scheduler;
use uma_tools::trigger::skill_activation::*;
use uma_tools::trigger::system::TriggerScheduler;

/// Build a scheduler for a single skill.
fn scheduler(skill: Skill, course_id: u32) -> TriggerScheduler {
    let mut rng = Pcg64Mcg::seed_from_u64(0);
    build_trigger_scheduler(&[skill], &load_course(course_id), &mut rng)
}

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

/// Assert that two floats are (almost) equal.
fn assert_close(actual: f64, expected: f64) {
    assert!(
        actual == expected || (actual - expected).abs() < 1e-9,
        "expected {expected}, got {actual}"
    );
}

// Course 10104: 2000m, sections of 83.33m (Mid phase at 333-1333m, Late phase at
// 1333-1667m), no slopes
const SECTION: f64 = 2000.0 / 24.0;

// ============================================================================
// PHASE + DISTANCE
// ============================================================================

#[test]
fn test_phase_and_distance_intersect() {
    // Skill 100621: distance_rate>=50&phase==1&order>=3&order_rate<=70
    // The phase must still be Mid when reaching 50%
    let scheduler = scheduler(
        make_skill(
            "100621",
            "distance_rate>=50&phase==1&order>=3&order_rate<=70",
            "",
        ),
        10104,
    );

    assert_eq!(scheduler.skill_logics.len(), 1);
    let logic = &scheduler.skill_logics[0];
    let (variable, min, max) = window(logic);
    assert_eq!(variable, WindowVariable::Position);
    assert_close(min, 1000.0);
    assert_close(max, SECTION * 16.0);
    assert_eq!(logic.guards.len(), 4, "All comparisons are guards");
}

#[test]
fn test_impossible_on_course_is_skipped() {
    // Skill 110121: distance_rate>=60&phase==1&slope==2&order_rate>=50
    // Course 10104 has no downhill, so neither the skill nor its precondition is
    // scheduled
    let scheduler = scheduler(
        make_skill(
            "110121",
            "distance_rate>=60&phase==1&slope==2&order_rate>=50",
            "change_order_up_middle>=3",
        ),
        10104,
    );

    assert!(scheduler.skill_logics.is_empty());
    assert!(scheduler.precondition_satisfied.is_empty());
}

#[test]
fn test_six_conditions_intersect() {
    // Skill 110591 on course 10913 (2000m, downhill at 1000-1800m):
    // - distance_rate>=60: from 1200m
    // - slope==2: downhills (0-1800m)
    // - phase==1: until 1333m
    // - remain_distance>=500: until 1500m
    let scheduler = scheduler(
        make_skill(
            "110591",
            "distance_rate>=60&slope==2&phase==1&order_rate>=40&order_rate<=80&remain_distance>=500",
            "",
        ),
        10913,
    );

    let logics = activation_logics(&scheduler);
    assert_eq!(logics.len(), 1);
    let (_, min, max) = window(logics[0]);
    assert_close(min, 1200.0);
    assert_close(max, SECTION * 16.0);
    assert_eq!(logics[0].guards.len(), 6);
}

// ============================================================================
// DISTANCE RANGES
// ============================================================================

#[test]
fn test_distance_rate_and_remain_distance_intersect() {
    // Skill 101191: distance_rate>=66&distance_rate<=68&order_rate>=50&remain_distance>=500
    // 66-68% (floored, so until 69%) is 1320-1380m, well before 500m remain
    let scheduler = scheduler(
        make_skill(
            "101191",
            "distance_rate>=66&distance_rate<=68&order_rate>=50&remain_distance>=500",
            "",
        ),
        10104,
    );

    let logic = &scheduler.skill_logics[0];
    let (_, min, max) = window(logic);
    assert_close(min, 1320.0);
    assert_close(max, 1380.0);
    assert_eq!(logic.guards.len(), 4);
}

// ============================================================================
// NON-POSITION WINDOWS
// ============================================================================

#[test]
fn test_hp_window() {
    // Skill 200761: distance_type==4&hp_per<=30 (HP is floored, so until 31%)
    let scheduler = scheduler(
        make_skill("200761", "distance_type==4&hp_per<=30", ""),
        10105,
    );

    let logic = &scheduler.skill_logics[0];
    let (variable, min, max) = window(logic);
    assert_eq!(variable, WindowVariable::HpPercent);
    assert_eq!((min, max), (0.0, 31.0));
    assert_eq!(logic.guards.len(), 2);
}

#[test]
fn test_time_window_with_dynamic_guards() {
    // Checked every frame after 5 seconds until every guard holds
    let scheduler = scheduler(
        make_skill(
            "test_time",
            "accumulatetime>=5&blocked_front_continuetime>=1&order_rate<=50&overtake_target_time>=2",
            "",
        ),
        10104,
    );

    let logic = &scheduler.skill_logics[0];
    let (variable, min, max) = window(logic);
    assert_eq!(variable, WindowVariable::Time);
    assert_eq!((min, max), (5.0, f64::INFINITY));
    assert_eq!(logic.guards.len(), 4);
}

#[test]
fn test_phase_window_with_dynamic_guards() {
    // Checked every frame during Late phase (not only when entering it)
    let scheduler = scheduler(
        make_skill(
            "test_phase",
            "phase==2&blocked_front_continuetime>=1&order_rate<=50",
            "",
        ),
        10104,
    );

    let logic = &scheduler.skill_logics[0];
    let (_, min, max) = window(logic);
    assert_close(min, SECTION * 16.0);
    assert_close(max, SECTION * 20.0);
    assert_eq!(logic.guards.len(), 3);
}
