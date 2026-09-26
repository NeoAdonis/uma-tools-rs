//! Comprehensive tests for skill activation trigger-guard separation system

use super::*;
use crate::course::geometry::*;
use crate::parser::{CompOp, parse_condition};
use crate::skill::*;
use crate::trigger::builder::build_trigger_scheduler;

mod windows;

/// Create a simple test course for testing spatial triggers
fn create_test_course() -> Course {
    Course::new_test(
        10001,
        CourseData {
            distance: 2000,
            distance_type: DistanceType::Medium,
            surface: Track::Turf,
            turn: Direction::Left,
            course_set_status: vec![],
            finish_time_min: 140000,
            finish_time_max: 160000,
            corners: vec![
                Corner {
                    start: 100.0,
                    length: 150.0,
                },
                Corner {
                    start: 500.0,
                    length: 200.0,
                },
                Corner {
                    start: 1200.0,
                    length: 180.0,
                },
            ],
            slopes: vec![Slope::new(800.0, 100.0, 500)],
            straights: vec![
                Straight {
                    start: 0.0,
                    end: 100.0,
                    front_type: FrontType::Audience,
                },
                Straight {
                    start: 1500.0,
                    end: 2000.0,
                    front_type: FrontType::Audience,
                },
            ],
            race_track_id: 1,
            lane_max: 10000,
        },
    )
}

// ============================================================================
// BASIC SPLITTING TESTS
// ============================================================================

#[test]
fn test_split_simple_guard_only() {
    // Condition with only static guards (no triggers, no monotonic guards)
    let condition = parse_condition("ground_type==2&motivation>=4").unwrap();
    let branches = split_condition(&condition, vec![], &create_test_course());

    // Should have 1 branch
    assert_eq!(branches.len(), 1);

    let (triggers, guards) = &branches[0];

    // Should have continuous trigger (fallback when no explicit triggers or monotonic guards)
    assert_eq!(triggers.len(), 1);
    assert!(matches!(triggers[0], TriggerCondition::Continuous));

    // Should preserve both guards
    assert!(!guards.is_empty());
}

#[test]
fn test_split_hp_trigger_with_guards() {
    // Condition: hp_per<=50 & ground_type==2
    // hp_per is decreasing, so <= is the normal direction (not reversed)
    let condition = parse_condition("hp_per<=50&ground_type==2").unwrap();
    let branches = split_condition(&condition, vec![], &create_test_course());

    // Should have 1 branch
    assert_eq!(branches.len(), 1);

    let (triggers, guards) = &branches[0];

    // Should remain as HpPercent trigger (correct direction for decreasing variable)
    assert_eq!(triggers.len(), 1);
    assert!(matches!(
        triggers[0],
        TriggerCondition::HpPercent {
            threshold: 50.0,
            operator: CompOp::LessEqual,
        }
    ));

    // Should have ground_type guard
    assert!(!guards.is_empty());
}

#[test]
fn test_split_random_gate_with_guards() {
    // Condition: phase_random==1 & hp_per>=20
    let condition = parse_condition("phase_random==1&hp_per>=20").unwrap();

    // Simulate random activation
    let activation = vec![800.0];

    let branches = split_condition(&condition, activation, &create_test_course());

    // Should have 1 branch
    assert_eq!(branches.len(), 1);

    let (triggers, guards) = &branches[0];

    // Should have random gate trigger
    assert_eq!(triggers.len(), 1);
    assert!(matches!(triggers[0], TriggerCondition::Position { .. }));

    // Should have hp_per guard (NOT phase_random)
    assert!(!guards.is_empty());
}

#[test]
fn test_split_onetime_event() {
    // Condition: is_last_straight_onetime==1
    let condition = parse_condition("is_last_straight_onetime==1").unwrap();
    let branches = split_condition(&condition, vec![], &create_test_course());

    // Should have 1 branch
    assert_eq!(branches.len(), 1);

    let (triggers, guards) = &branches[0];

    // Should extract onetime trigger
    assert_eq!(triggers.len(), 1);
    assert!(matches!(
        triggers[0],
        TriggerCondition::StraightTransition {
            is_last: true,
            is_onetime: true,
            is_entry: true
        }
    ));

    // No guards
    assert!(guards.is_empty());
}

#[test]
fn test_split_or_condition() {
    // Condition: (phase_random==1 & hp_per>=20) @ (corner!=0 & ground_type==1)
    // This should create 2 separate branches
    let condition = parse_condition("phase_random==1&hp_per>=20@corner!=0&ground_type==1").unwrap();

    let activation = vec![800.0];

    let branches = split_condition(&condition, activation, &create_test_course());

    // Should have 2 branches (one for each OR alternative)
    assert_eq!(branches.len(), 2);

    // Branch 1: random trigger + hp guard
    let (triggers1, guards1) = &branches[0];
    assert!(
        triggers1
            .iter()
            .any(|t| matches!(t, TriggerCondition::Position { .. }))
    );
    assert!(!guards1.is_empty());

    // Branch 2: corner trigger (Continuous fallback) + guards
    let (triggers2, guards2) = &branches[1];
    assert!(
        triggers2
            .iter()
            .any(|t| matches!(t, TriggerCondition::Continuous))
    );
    assert!(!guards2.is_empty());
}

// ============================================================================
// OPERATOR DIRECTION TESTS
// ============================================================================

#[test]
fn test_distance_rate_less_than_becomes_window() {
    // distance_rate<=50 should become Window [0, 50], not DistanceCovered trigger
    // distance_rate is monotonically increasing, so <= is reversed direction
    let condition = parse_condition("distance_rate<=50&ground_type==2").unwrap();
    let branches = split_condition(&condition, vec![], &create_test_course());

    assert_eq!(branches.len(), 1);
    let (triggers, guards) = &branches[0];

    // Should create Window trigger, not DistanceCovered
    assert_eq!(triggers.len(), 1);
    assert!(matches!(
        triggers[0],
        TriggerCondition::Window {
            variable: WindowVariable::DistanceRate,
            min_threshold: 0.0,
            max_threshold: 50.0,
        }
    ));

    // ground_type should remain as guard
    assert!(!guards.is_empty());
}

#[test]
fn test_distance_rate_greater_than_stays_trigger() {
    // distance_rate>=50 should remain as DistanceCovered trigger (correct direction)
    let condition = parse_condition("distance_rate>=50&ground_type==2").unwrap();
    let branches = split_condition(&condition, vec![], &create_test_course());

    assert_eq!(branches.len(), 1);
    let (triggers, guards) = &branches[0];

    // Should remain as DistanceCovered trigger
    assert_eq!(triggers.len(), 1);
    assert!(matches!(
        triggers[0],
        TriggerCondition::DistanceCovered { threshold: 50.0 }
    ));

    // ground_type should remain as guard
    assert!(!guards.is_empty());
}

#[test]
fn test_accumulatetime_less_than_becomes_window() {
    // accumulatetime<=10 should become Window [0, 10], not Time trigger
    // accumulatetime is monotonically increasing, so <= is reversed direction
    let condition = parse_condition("accumulatetime<=10&ground_type==2").unwrap();
    let branches = split_condition(&condition, vec![], &create_test_course());

    assert_eq!(branches.len(), 1);
    let (triggers, guards) = &branches[0];

    // Should create Window trigger, not Time
    assert_eq!(triggers.len(), 1);
    assert!(matches!(
        triggers[0],
        TriggerCondition::Window {
            variable: WindowVariable::AccumulateTime,
            min_threshold: 0.0,
            max_threshold: 10.0,
        }
    ));

    // ground_type should remain as guard
    assert!(!guards.is_empty());
}

#[test]
fn test_accumulatetime_greater_than_stays_trigger() {
    // accumulatetime>=10 should remain as Time trigger (correct direction)
    let condition = parse_condition("accumulatetime>=10&ground_type==2").unwrap();
    let branches = split_condition(&condition, vec![], &create_test_course());

    assert_eq!(branches.len(), 1);
    let (triggers, guards) = &branches[0];

    // Should remain as Time trigger
    assert_eq!(triggers.len(), 1);
    assert!(matches!(
        triggers[0],
        TriggerCondition::Time {
            threshold: 10.0,
            operator: CompOp::GreaterEqual,
        }
    ));

    // ground_type should remain as guard
    assert!(!guards.is_empty());
}

#[test]
fn test_hp_per_greater_than_becomes_window() {
    // hp_per>=50 should become Window [50, 100] for decreasing variable
    // hp_per is decreasing, so >= is reversed direction
    let condition = parse_condition("hp_per>=50&ground_type==2").unwrap();
    let branches = split_condition(&condition, vec![], &create_test_course());

    assert_eq!(branches.len(), 1);
    let (triggers, guards) = &branches[0];

    // Should create Window trigger for reversed direction
    assert_eq!(triggers.len(), 1);
    assert!(matches!(
        triggers[0],
        TriggerCondition::Window {
            variable: WindowVariable::HpPercent,
            min_threshold: 50.0,
            max_threshold: 100.0,
        }
    ));

    // ground_type should remain as guard
    assert!(!guards.is_empty());
}

#[test]
fn test_hp_per_less_than_stays_trigger() {
    // hp_per<=50 should remain as HpPercent trigger (correct direction for decreasing)
    let condition = parse_condition("hp_per<=50&ground_type==2").unwrap();
    let branches = split_condition(&condition, vec![], &create_test_course());

    assert_eq!(branches.len(), 1);
    let (triggers, guards) = &branches[0];

    // Should remain as HpPercent trigger
    assert_eq!(triggers.len(), 1);
    assert!(matches!(
        triggers[0],
        TriggerCondition::HpPercent {
            threshold: 50.0,
            operator: CompOp::LessEqual,
        }
    ));

    // ground_type should remain as guard
    assert!(!guards.is_empty());
}

// ============================================================================
// EDGE CASES
// ============================================================================

#[test]
fn test_skill_100901_distance_rate_less_than() {
    // Skill 100901: "distance_rate<=50&corner!=0&order_rate<=50&ground_type==1"
    //
    // Expected behavior:
    // - distance_rate<=50 becomes Window [0, 50]
    // - order_rate<=50 is DynamicGuard
    // - corner!=0, ground_type==1 are all guards
    // - Checked continuously every frame

    let condition =
        parse_condition("distance_rate<=50&corner!=0&order_rate<=50&ground_type==1").unwrap();
    let branches = split_condition(&condition, vec![], &create_test_course());

    assert_eq!(branches.len(), 1);
    let (triggers, guards) = &branches[0];

    // Should have Window trigger for distance_rate<=50
    assert_eq!(triggers.len(), 1);
    assert!(matches!(
        triggers[0],
        TriggerCondition::Window {
            variable: WindowVariable::DistanceRate,
            min_threshold: 0.0,
            max_threshold: 50.0,
        }
    ));

    // All conditions become guards (checked continuously)
    assert!(!guards.is_empty());
}

#[test]
fn test_less_operator_also_converts() {
    // Test that < (not just <=) also converts for increasing variables
    let condition = parse_condition("distance_rate<50&ground_type==2").unwrap();
    let branches = split_condition(&condition, vec![], &create_test_course());

    assert_eq!(branches.len(), 1);
    let (triggers, guards) = &branches[0];

    // Should create Window trigger
    assert_eq!(triggers.len(), 1);
    assert!(matches!(
        triggers[0],
        TriggerCondition::Window {
            variable: WindowVariable::DistanceRate,
            min_threshold: 0.0,
            max_threshold: 50.0,
        }
    ));

    assert!(!guards.is_empty());
}

#[test]
fn test_greater_operator_also_converts() {
    // Test that > (not just >=) also converts for decreasing variables
    let condition = parse_condition("hp_per>50&ground_type==2").unwrap();
    let branches = split_condition(&condition, vec![], &create_test_course());

    assert_eq!(branches.len(), 1);
    let (triggers, guards) = &branches[0];

    // Should create Window trigger
    assert_eq!(triggers.len(), 1);
    assert!(matches!(
        triggers[0],
        TriggerCondition::Window {
            variable: WindowVariable::HpPercent,
            min_threshold: 50.0,
            max_threshold: 100.0,
        }
    ));

    assert!(!guards.is_empty());
}

#[test]
fn test_window_detection_still_works() {
    // Ensure existing window detection (distance_rate>=50 & distance_rate<=60) still works
    let condition = parse_condition("distance_rate>=50&distance_rate<=60").unwrap();
    let branches = split_condition(&condition, vec![], &create_test_course());

    assert_eq!(branches.len(), 1);
    let (triggers, guards) = &branches[0];

    // Should detect as window
    assert_eq!(triggers.len(), 1);
    assert!(matches!(
        triggers[0],
        TriggerCondition::Window {
            variable: WindowVariable::DistanceRate,
            min_threshold: 50.0,
            max_threshold: 60.0,
        }
    ));

    // No remaining guards
    assert!(guards.is_empty());
}

// ============================================================================
// PHASE OPERATOR BUG (Similar to monotonic variable bug)
// ============================================================================

#[test]
fn test_phase_less_than_equal_bug() {
    // Skill 200441: "phase<=1&accumulatetime>=5&blocked_front_continuetime>=1"
    //
    // SEMANTIC ISSUE: phase<=1 means "while in Early OR Mid phase"
    // But currently it creates PhaseChange { phase: Mid } which only fires at Mid entry
    //
    // This is wrong because:
    // 1. If all guards pass at t=5s during Early phase, skill should activate
    // 2. But PhaseChange(Mid) hasn't fired yet
    // 3. Skill won't activate until Mid phase begins
    //
    // EXPECTED FIX: phase<=1 should become a GUARD, not a TRIGGER
    // - The trigger should be accumulatetime>=5 (Time trigger)
    // - The guards should be: phase<=1 & blocked_front_continuetime>=1
    // - At runtime: when Time(5s) fires, check if phase is <=Mid

    let condition =
        parse_condition("phase<=1&accumulatetime>=5&blocked_front_continuetime>=1").unwrap();
    let branches = split_condition(&condition, vec![], &create_test_course());

    assert_eq!(branches.len(), 1);
    let (triggers, guards) = &branches[0];

    // AFTER FIXES:
    // 1. phase<=1 is now a guard (not a trigger) ✓
    // 2. blocked_front_continuetime is a DynamicGuard
    // 3. Therefore: Convert to Continuous trigger!
    //
    // This is correct because blocked_front_continuetime can change during the race.
    // We must check continuously, not just at t=5s.

    assert_eq!(triggers.len(), 1);
    assert!(matches!(triggers[0], TriggerCondition::Continuous));

    // All conditions should be guards (checked every frame)
    assert!(!guards.is_empty());

    // Success! Now the skill will activate when:
    // - phase is <=Mid AND
    // - accumulatetime is >=5s AND
    // - blocked_front_continuetime is >=1
    // Checked continuously every frame
}

#[test]
fn test_phase_equality_still_creates_trigger() {
    // Ensure phase==2 still creates a PhaseChange trigger (not a guard)
    let condition = parse_condition("phase==2&accumulatetime>=5").unwrap();
    let branches = split_condition(&condition, vec![], &create_test_course());

    assert_eq!(branches.len(), 1);
    let (triggers, guards) = &branches[0];

    // Should have PhaseChange trigger (phase==2 is equality, so it's a trigger)
    // accumulatetime should remain as guard (Rule 1: MonotonicGuard stays as guard with explicit trigger)
    assert_eq!(triggers.len(), 1);
    assert!(matches!(
        triggers[0],
        TriggerCondition::PhaseChange { phase: _ }
    ));

    // accumulatetime should be a guard
    assert!(!guards.is_empty());
}

// ============================================================================
// DYNAMIC GUARD + MONOTONIC TRIGGER BUG
// ============================================================================

#[test]
fn test_dynamic_guard_forces_continuous() {
    // Skill with DynamicGuard should use Continuous trigger, not monotonic
    // Example: accumulatetime>=5 & blocked_front_continuetime>=1
    //
    // Problem: If we use Time(5s) trigger, it fires once and is removed from heap.
    // If blocked_front_continuetime=0 at t=5s, guard fails and skill is lost.
    // Even if blocked_front_continuetime becomes 1 at t=6s, trigger is gone!
    //
    // Solution: Convert to Continuous trigger when DynamicGuards present

    let condition = parse_condition("accumulatetime>=5&blocked_front_continuetime>=1").unwrap();
    let branches = split_condition(&condition, vec![], &create_test_course());

    assert_eq!(branches.len(), 1);
    let (triggers, guards) = &branches[0];

    // Should use Continuous trigger (not Time) because blocked_front_continuetime is DynamicGuard
    assert_eq!(triggers.len(), 1);
    assert!(matches!(triggers[0], TriggerCondition::Continuous));

    // All conditions should be guards
    assert!(!guards.is_empty());
}

#[test]
fn test_static_guards_allow_monotonic_trigger() {
    // Skill with only StaticGuards can safely use monotonic triggers
    // Example: accumulatetime>=5 & ground_type==2
    //
    // This is safe because ground_type never changes during race.
    // If guard fails at t=5s, it will never pass.

    let condition = parse_condition("accumulatetime>=5&ground_type==2").unwrap();
    let branches = split_condition(&condition, vec![], &create_test_course());

    assert_eq!(branches.len(), 1);
    let (triggers, guards) = &branches[0];

    // Should promote accumulatetime to Time trigger (no DynamicGuards)
    assert_eq!(triggers.len(), 1);
    assert!(matches!(
        triggers[0],
        TriggerCondition::Time {
            threshold: 5.0,
            operator: CompOp::GreaterEqual
        }
    ));

    // ground_type should be a guard
    assert!(!guards.is_empty());
}

// ============================================================================
// ACTIVATE_COUNT_* VARIABLE TESTS
// ============================================================================
// These tests verify that activate_count_* variables (MonotonicGuard role)
// are correctly handled in trigger-guard separation:
// - When explicit triggers exist: activate_count_* stays as guard
// - When no explicit triggers: activate_count_* promotes to Continuous trigger

#[test]
fn test_activate_count_with_window_trigger() {
    // Real Skill ID: 101051
    // Condition: distance_rate>=50&distance_rate<=55&order_rate>=40&order_rate<=80&distance_type==3&activate_count_all>=7
    //
    // Expected behavior:
    // - Window pattern detected on distance_rate → creates Window trigger
    // - Window trigger is explicit, so it takes precedence over DynamicGuard continuous requirement
    // - activate_count_all stays as guard (MonotonicGuard)
    // - order_rate stays as guard (DynamicGuard)
    // - All other conditions checked when window trigger fires

    let condition = parse_condition(
        "distance_rate>=50&distance_rate<=55&order_rate>=40&order_rate<=80&distance_type==3&activate_count_all>=7",
    )
    .unwrap();
    let branches = split_condition(&condition, vec![], &create_test_course());

    assert_eq!(branches.len(), 1);
    let (triggers, guards) = &branches[0];

    // Should have Window trigger (explicit trigger takes precedence)
    assert_eq!(triggers.len(), 1);
    assert!(matches!(
        triggers[0],
        TriggerCondition::Window {
            variable: WindowVariable::DistanceRate,
            min_threshold: 50.0,
            max_threshold: 55.0,
        }
    ));

    // All other conditions should be guards (including activate_count_all, order_rate)
    assert!(!guards.is_empty());
}

#[test]
fn test_activate_count_with_monotonic_promoted() {
    // Real Skill ID: 110041
    // Condition: distance_rate>=50&activate_count_heal>=1&order<=3
    //
    // Expected behavior:
    // - Window [50, ∞) detected on distance_rate → promotes to DistanceCovered trigger
    // - order stays as DynamicGuard
    // - activate_count_heal stays as guard (MonotonicGuard)
    // - All conditions checked continuously

    let condition = parse_condition("distance_rate>=50&activate_count_heal>=1&order<=3").unwrap();
    let branches = split_condition(&condition, vec![], &create_test_course());

    assert_eq!(branches.len(), 1);
    let (triggers, guards) = &branches[0];

    // Should use Window trigger
    assert_eq!(triggers.len(), 1);
    assert!(matches!(
        triggers[0],
        TriggerCondition::Window {
            variable: WindowVariable::DistanceRate,
            min_threshold: 50.0,
            max_threshold: 100.0,
        }
    ));

    // activate_count_heal, distance_rate, and order should remain as guards
    assert!(!guards.is_empty());
}

#[test]
fn test_activate_count_with_phase_trigger() {
    // Real Skill ID: 110011
    // Condition: phase==1&order>=2&order_rate<=80&activate_count_middle>=2
    //
    // Expected behavior:
    // - phase==1 creates PhaseChange trigger (explicit trigger)
    // - PhaseChange trigger is explicit, so it takes precedence (Rule 1)
    // - order and order_rate are DynamicGuards but don't force Continuous (explicit trigger exists)
    // - activate_count_middle stays as guard (MonotonicGuard)
    // - All other conditions checked when phase trigger fires

    let condition =
        parse_condition("phase==1&order>=2&order_rate<=80&activate_count_middle>=2").unwrap();
    let branches = split_condition(&condition, vec![], &create_test_course());

    assert_eq!(branches.len(), 1);
    let (triggers, guards) = &branches[0];

    // Should have PhaseChange trigger (explicit trigger takes precedence)
    assert_eq!(triggers.len(), 1);
    assert!(matches!(triggers[0], TriggerCondition::PhaseChange { .. }));

    // activate_count_middle and other conditions should be guards
    assert!(!guards.is_empty());
}

#[test]
fn test_activate_count_with_spatial_trigger() {
    // Real Skill ID: 111051 (simplified to one branch)
    // Condition: is_last_straight==1&phase>=2&order>=2&activate_count_middle==7
    //
    // Expected behavior
    // - is_last_straight==1 creates Window trigger [straight.start, straight.end]
    // - Window trigger is explicit, so it takes precedence
    // - order is DynamicGuard but doesn't force Continuous (explicit trigger exists)
    // - phase>=2 stays as guard (comparison operator)
    // - activate_count_middle stays as guard (MonotonicGuard)
    // - All other conditions checked while within the last straight window

    let condition =
        parse_condition("is_last_straight==1&phase>=2&order>=2&activate_count_middle==7").unwrap();
    let branches = split_condition(&condition, vec![], &create_test_course());

    assert_eq!(branches.len(), 1);
    let (triggers, guards) = &branches[0];

    // Should have Window trigger for last straight
    assert_eq!(triggers.len(), 1);
    assert!(matches!(
        triggers[0],
        TriggerCondition::Window {
            variable: WindowVariable::SpatialPosition,
            ..
        }
    ));

    // All other conditions (including activate_count_middle) should be guards
    assert!(!guards.is_empty());
}

#[test]
fn test_activate_count_alone_continuous() {
    // Real Skill ID: 102101211
    // Condition: activate_count_middle>=3
    //
    // Expected behavior:
    // - No explicit triggers available
    // - activate_count_middle is MonotonicGuard but can't be promoted to threshold trigger
    // - Use Continuous trigger (check every frame)

    let condition = parse_condition("activate_count_middle>=3").unwrap();
    let branches = split_condition(&condition, vec![], &create_test_course());

    assert_eq!(branches.len(), 1);
    let (triggers, guards) = &branches[0];

    // Should use Continuous trigger (no explicit triggers, activate_count can't be promoted)
    assert_eq!(triggers.len(), 1);
    assert!(matches!(triggers[0], TriggerCondition::Continuous));

    // activate_count_middle should be a guard
    assert!(!guards.is_empty());
}

#[test]
fn test_activate_count_with_static_guard_continuous() {
    // Real Skill ID: 109001211
    // Condition: distance_type==2&activate_count_start>=1
    //
    // Expected behavior:
    // - No explicit triggers available
    // - distance_type==2 is StaticGuard (can't be promoted)
    // - activate_count_start is MonotonicGuard but can't be promoted to threshold trigger
    // - Use Continuous trigger (check every frame)

    let condition = parse_condition("distance_type==2&activate_count_start>=1").unwrap();
    let branches = split_condition(&condition, vec![], &create_test_course());

    assert_eq!(branches.len(), 1);
    let (triggers, guards) = &branches[0];

    // Should use Continuous trigger (no promotable triggers available)
    assert_eq!(triggers.len(), 1);
    assert!(matches!(triggers[0], TriggerCondition::Continuous));

    // Both conditions should be guards
    assert!(!guards.is_empty());
}

// ============================================================
// PRECONDITION TESTS
// ============================================================

/// Helper function to create a test skill with precondition.
fn create_skill_with_precondition(id: &str, condition: &str, precondition: &str) -> Skill {
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
            wisdom_check: true,
        },
    )
}

/// Build a scheduler for the given skills on the test course.
fn build_test_scheduler(skills: &[Skill]) -> crate::trigger::system::TriggerScheduler {
    use rand::SeedableRng;
    let mut rng = rand_pcg::Pcg64Mcg::seed_from_u64(0);
    build_trigger_scheduler(skills, &create_test_course(), &mut rng)
}

/// Compile a precondition on the test course.
fn compile_test_precondition(precondition: &str) -> Vec<PreconditionBranch> {
    compile_precondition(
        &parse_condition(precondition).unwrap(),
        &create_test_course(),
    )
}

/// Get the `(min, max)` thresholds of a precondition window trigger.
fn window_bounds(branch: &PreconditionBranch) -> (f64, f64) {
    match branch.trigger {
        TriggerCondition::Window {
            variable: WindowVariable::SpatialPosition,
            min_threshold,
            max_threshold,
        } => (min_threshold, max_threshold),
        ref other => panic!("Expected position window, got {:?}", other),
    }
}

/// Assert that two floats are (almost) equal.
fn assert_close(actual: f64, expected: f64) {
    assert!(
        actual == expected || (actual - expected).abs() < 1e-9,
        "expected {expected}, got {actual}"
    );
}

#[test]
fn test_precondition_creates_unlock_logic_before_skill_logic() {
    let skill =
        create_skill_with_precondition("test_precondition", "distance_rate>=80", "phase>=2");
    let scheduler = build_test_scheduler(&[skill]);

    assert_eq!(scheduler.skill_logics.len(), 2);

    // Unlock logic comes first, keeps the comparison as guard
    let unlock = &scheduler.skill_logics[0];
    assert_eq!(unlock.action, LogicAction::Unlock(0));
    assert!(unlock.precondition.is_none());
    assert_eq!(unlock.guards.len(), 1);
    assert_eq!(unlock.guards[0].op, CompOp::GreaterEqual);
    assert_eq!(unlock.guards[0].value, 2);

    // Skill logic waits for the precondition
    let skill_logic = &scheduler.skill_logics[1];
    assert_eq!(skill_logic.action, LogicAction::Activate);
    assert_eq!(skill_logic.precondition, Some(0));

    // Starts locked; its window is queued separately from regular windows
    assert_eq!(scheduler.precondition_satisfied, vec![false]);
    let windows: Vec<_> = scheduler.precondition_windows.iter().collect();
    assert_eq!(windows.len(), 1);
    assert_eq!(windows[0].logic_idx, 0);
    assert!(scheduler.position_windows.is_empty());
}

#[test]
fn test_empty_precondition_treated_as_none() {
    let skill = create_skill_with_precondition("test_empty", "distance_rate>=80", "");
    let scheduler = build_test_scheduler(&[skill]);

    assert_eq!(scheduler.skill_logics.len(), 1);
    assert!(scheduler.skill_logics[0].precondition.is_none());
    assert!(scheduler.precondition_satisfied.is_empty());
    assert!(scheduler.precondition_windows.is_empty());
}

#[test]
fn test_or_condition_branches_share_precondition() {
    let skill = create_skill_with_precondition(
        "test_or",
        "distance_rate>=80@hp_per<=50", // OR condition
        "phase>=2",                     // Shared precondition
    );
    let scheduler = build_test_scheduler(&[skill]);

    // 1 unlock logic + 2 condition branches, all tied to the same precondition
    assert_eq!(scheduler.skill_logics.len(), 3);
    assert_eq!(scheduler.skill_logics[0].action, LogicAction::Unlock(0));
    assert_eq!(scheduler.skill_logics[1].precondition, Some(0));
    assert_eq!(scheduler.skill_logics[2].precondition, Some(0));
    assert_eq!(scheduler.precondition_satisfied.len(), 1);
}

#[test]
fn test_or_precondition_creates_one_unlock_logic_per_branch() {
    let skill = create_skill_with_precondition("test_or", "distance_rate>=80", "phase==1@phase==3");
    let scheduler = build_test_scheduler(&[skill]);

    assert_eq!(scheduler.skill_logics.len(), 3);
    assert_eq!(scheduler.skill_logics[0].action, LogicAction::Unlock(0));
    assert_eq!(scheduler.skill_logics[1].action, LogicAction::Unlock(0));
    assert_eq!(scheduler.skill_logics[2].precondition, Some(0));
    assert_eq!(scheduler.precondition_windows.len(), 2);
}

#[test]
fn test_precondition_only_in_alternative_zero() {
    // Create a skill with 2 alternatives - only first has precondition
    let skill = Skill::new(
        "test_multi_alt".to_string(),
        SkillData {
            rarity: SkillRarity::White,
            alternatives: vec![
                SkillAlternative {
                    condition: "distance_rate>=80".to_string(),
                    precondition: "phase>=2".to_string(), // Has precondition
                    base_duration: 40000,
                    effects: vec![SkillEffect {
                        skill_type: SkillType::TargetSpeed,
                        target: SkillTarget::Oneself,
                        modifier: 4000,
                    }],
                },
                SkillAlternative {
                    condition: "hp_per<=50".to_string(),
                    precondition: "".to_string(), // No precondition
                    base_duration: 40000,
                    effects: vec![SkillEffect {
                        skill_type: SkillType::TargetSpeed,
                        target: SkillTarget::Oneself,
                        modifier: 3000,
                    }],
                },
            ],
            wisdom_check: true,
        },
    );
    let scheduler = build_test_scheduler(&[skill]);

    // Unlock logic, then one logic per alternative
    assert_eq!(scheduler.skill_logics.len(), 3);
    assert_eq!(scheduler.skill_logics[1].alternative_index, 0);
    assert_eq!(scheduler.skill_logics[1].precondition, Some(0));
    assert_eq!(scheduler.skill_logics[2].alternative_index, 1);
    assert!(scheduler.skill_logics[2].precondition.is_none());
}

#[test]
fn test_precondition_ids_are_assigned_per_alternative() {
    let skills = [
        create_skill_with_precondition("skill1", "distance_rate>=80", "phase>=2"),
        create_skill_with_precondition("skill2", "hp_per<=50", ""),
        create_skill_with_precondition("skill3", "distance_rate>=50", "order<=3"),
    ];
    let scheduler = build_test_scheduler(&skills);

    assert_eq!(scheduler.precondition_satisfied, vec![false, false]);

    let preconditions: Vec<_> = scheduler
        .skill_logics
        .iter()
        .filter(|logic| logic.action == LogicAction::Activate)
        .map(|logic| (logic.skill_index, logic.precondition))
        .collect();
    assert_eq!(preconditions, vec![(0, Some(0)), (1, None), (2, Some(1))]);
}

#[test]
fn test_unsatisfiable_precondition_skips_alternative() {
    // The test course has no downhill slopes and only 3 corners
    for precondition in ["slope==2", "corner==4", "phase==1&distance_rate>=90"] {
        let skill = create_skill_with_precondition("test", "distance_rate>=80", precondition);
        let scheduler = build_test_scheduler(&[skill]);

        assert!(
            scheduler.skill_logics.is_empty(),
            "{precondition} should never be satisfied"
        );
        assert!(scheduler.distance_triggers.is_empty());
    }
}

#[test]
fn test_precondition_window_ranges() {
    // Test course: 2000m, sections of 83.33m, corners at 100-250, 500-700 and
    // 1200-1380, uphill at 800-900. Windows are widened by 10m on each side.
    let section = 2000.0 / 24.0;
    let cases = [
        ("distance_rate>=50", (990.0, f64::INFINITY)),
        ("distance_rate>=50&distance_rate<=51", (990.0, 1050.0)),
        ("remain_distance<=600", (1389.0, f64::INFINITY)),
        (
            "remain_distance<=305&remain_distance>=300",
            (1684.0, 1710.0),
        ),
        ("phase==1", (section * 4.0 - 10.0, section * 16.0 + 10.0)),
        ("phase>=2", (section * 16.0 - 10.0, 2010.0)),
        (
            "phase_firsthalf==1",
            (section * 4.0 - 10.0, section * 10.0 + 10.0),
        ),
        ("is_finalcorner==1", (1190.0, f64::INFINITY)),
        ("is_finalcorner==1&corner!=0", (1190.0, 1390.0)),
        ("corner==2", (490.0, 710.0)),
        ("slope==1", (790.0, 910.0)),
        ("phase>=1&slope==1&order_rate>=40", (790.0, 910.0)),
        // Nothing position-based: window covers the whole race
        ("change_order_onetime<0", (0.0, f64::INFINITY)),
        ("distance_type==3", (0.0, f64::INFINITY)),
    ];

    for (precondition, (min, max)) in cases {
        let branches = compile_test_precondition(precondition);
        assert_eq!(branches.len(), 1, "{precondition}");
        let (actual_min, actual_max) = window_bounds(&branches[0]);
        assert_close(actual_min, min);
        assert_close(actual_max, max);
        // Comparisons are kept as guards
        assert_eq!(
            branches[0].guards.len(),
            precondition.split('&').count(),
            "{precondition}"
        );
    }
}

#[test]
fn test_precondition_event_branches() {
    let branches = compile_test_precondition("phase_firsthalf==1&is_activate_heal_skill==1");
    assert_eq!(branches.len(), 1);
    assert!(matches!(
        branches[0].trigger,
        TriggerCondition::HealSkillActivation
    ));
    // Event comparison is removed; the rest is checked when the event happens
    assert_eq!(branches[0].guards.len(), 1);
    assert_eq!(
        branches[0].guards[0].var,
        crate::condition::variables::VarType::Phase(
            crate::condition::variables::PhaseVar::PhaseFirsthalf
        )
    );

    let branches = compile_test_precondition("phase>=2&is_last_straight_onetime==1");
    assert!(matches!(
        branches[0].trigger,
        TriggerCondition::StraightTransition {
            is_last: true,
            is_onetime: true,
            ..
        }
    ));
    assert_eq!(branches[0].guards.len(), 1);

    // Events can't happen where the rest of the branch is false
    assert!(compile_test_precondition("slope==2&is_activate_heal_skill==1").is_empty());
}

#[test]
fn test_precondition_event_is_indexed_before_skill() {
    let skill = create_skill_with_precondition(
        "test_straight",
        "is_last_straight_onetime==1",
        "phase>=2&is_last_straight_onetime==1",
    );
    let scheduler = build_test_scheduler(&[skill]);

    // Both logics fire on the last straight; the unlock logic must go first
    assert!(scheduler.precondition_windows.is_empty());
    assert_eq!(scheduler.straight_triggers.len(), 1);
    assert_eq!(scheduler.straight_triggers[0].logic_indices, vec![0, 1]);
    assert_eq!(scheduler.skill_logics[0].action, LogicAction::Unlock(0));
}
