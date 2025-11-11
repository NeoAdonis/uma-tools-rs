//! Comprehensive tests for skill activation trigger-guard separation system

use super::super::skill_activation::*;
use crate::course::geometry::*;
use crate::course::*;
use crate::parser::{CompOp, parse_condition};
use crate::trigger::builder::build_trigger_scheduler;
use crate::uma::skill::*;

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
    assert!(matches!(
        triggers[0],
        TriggerCondition::DistanceCovered { .. }
    ));

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
            .any(|t| matches!(t, TriggerCondition::DistanceCovered { .. }))
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
        TriggerCondition::DistanceCovered {
            threshold: 50.0,
            operator: CompOp::GreaterEqual,
        }
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
        },
    )
}

#[test]
fn test_precondition_simple_parsing() {
    use rand::SeedableRng;
    use rand_pcg::Pcg64Mcg;

    // Create a skill with a simple precondition
    let skill =
        create_skill_with_precondition("test_precondition", "distance_rate>=80", "phase>=2");

    // Build trigger scheduler
    let course = create_test_course();
    let mut rng = Pcg64Mcg::seed_from_u64(0);
    let scheduler = build_trigger_scheduler(&[skill], &course, &mut rng);

    // Verify skill logic was created
    assert_eq!(scheduler.skill_logics.len(), 1);
    let logic = &scheduler.skill_logics[0];

    // Verify precondition was parsed and stored
    assert!(
        logic.precondition.is_some(),
        "Precondition should be parsed"
    );

    // Verify the precondition contains the expected comparison
    let precond = logic.precondition.as_ref().unwrap();
    // The precondition should be "phase>=2"
    if let crate::parser::ComparisonTree::Single(comp) = &precond.comp_tree {
        assert!(matches!(
            comp.var,
            crate::condition::variables::VarType::Phase(_)
        ));
        assert_eq!(comp.op, CompOp::GreaterEqual);
        assert_eq!(comp.value, 2);
    } else {
        panic!("Expected single comparison in precondition");
    }
}

#[test]
fn test_precondition_complex_and_parsing() {
    use crate::parser::BinOp;
    use rand::SeedableRng;
    use rand_pcg::Pcg64Mcg;

    // Create a skill with a complex AND precondition
    let skill = create_skill_with_precondition(
        "test_complex",
        "distance_rate>=80",
        "phase>=2&hp_per<=50&accumulate_time>=10",
    );

    let course = create_test_course();
    let mut rng = Pcg64Mcg::seed_from_u64(0);
    let scheduler = build_trigger_scheduler(&[skill], &course, &mut rng);

    assert_eq!(scheduler.skill_logics.len(), 1);
    let logic = &scheduler.skill_logics[0];

    // Verify precondition exists
    assert!(
        logic.precondition.is_some(),
        "Complex precondition should be parsed"
    );

    // Verify it's a binary AND structure (not fully validating the tree, just structure)
    let precond = logic.precondition.as_ref().unwrap();
    match &precond.comp_tree {
        crate::parser::ComparisonTree::Binary { op, .. } => {
            assert_eq!(*op, BinOp::And, "Expected AND operator");
        }
        _ => panic!("Expected binary AND structure for complex precondition"),
    }
}

#[test]
fn test_empty_precondition_treated_as_none() {
    use rand::SeedableRng;
    use rand_pcg::Pcg64Mcg;

    // Create a skill with empty precondition
    let skill = create_skill_with_precondition(
        "test_empty",
        "distance_rate>=80",
        "", // Empty precondition
    );

    let course = create_test_course();
    let mut rng = Pcg64Mcg::seed_from_u64(0);
    let scheduler = build_trigger_scheduler(&[skill], &course, &mut rng);

    assert_eq!(scheduler.skill_logics.len(), 1);
    let logic = &scheduler.skill_logics[0];

    // Empty precondition should be treated as None
    assert!(
        logic.precondition.is_none(),
        "Empty precondition should be None"
    );
}

#[test]
fn test_or_branches_share_same_precondition() {
    use rand::SeedableRng;
    use rand_pcg::Pcg64Mcg;

    // Create a skill with OR condition and a precondition
    let skill = create_skill_with_precondition(
        "test_or",
        "distance_rate>=80@hp_per<=50", // OR condition
        "phase>=2",                     // Shared precondition
    );

    let course = create_test_course();
    let mut rng = Pcg64Mcg::seed_from_u64(0);
    let scheduler = build_trigger_scheduler(&[skill], &course, &mut rng);

    // OR condition should create 2 branches
    assert_eq!(
        scheduler.skill_logics.len(),
        2,
        "OR condition should create 2 branches"
    );

    // Both branches should have the same precondition
    let logic1 = &scheduler.skill_logics[0];
    let logic2 = &scheduler.skill_logics[1];

    assert!(
        logic1.precondition.is_some(),
        "First branch should have precondition"
    );
    assert!(
        logic2.precondition.is_some(),
        "Second branch should have precondition"
    );

    // Verify both have phase>=2
    for (idx, logic) in [logic1, logic2].iter().enumerate() {
        if let Some(precond) = &logic.precondition {
            if let crate::parser::ComparisonTree::Single(comp) = &precond.comp_tree {
                assert!(
                    matches!(comp.var, crate::condition::variables::VarType::Phase(_)),
                    "Branch {} should have phase precondition",
                    idx
                );
                assert_eq!(comp.value, 2, "Branch {} should have phase>=2", idx);
            }
        }
    }
}

#[test]
fn test_precondition_only_in_alternative_zero() {
    use rand::SeedableRng;
    use rand_pcg::Pcg64Mcg;

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
        },
    );

    let course = create_test_course();
    let mut rng = Pcg64Mcg::seed_from_u64(0);
    let scheduler = build_trigger_scheduler(&[skill], &course, &mut rng);

    // Should have 2 skill logics (one per alternative)
    assert_eq!(scheduler.skill_logics.len(), 2);

    // First logic should have precondition
    assert!(
        scheduler.skill_logics[0].precondition.is_some(),
        "First alternative should have precondition"
    );

    // Second logic should NOT have precondition
    assert!(
        scheduler.skill_logics[1].precondition.is_none(),
        "Second alternative should not have precondition"
    );
}

#[test]
fn test_precondition_satisfied_array_initialization() {
    use rand::SeedableRng;
    use rand_pcg::Pcg64Mcg;

    // Create multiple skills with and without preconditions
    let skill1 = create_skill_with_precondition("skill1", "distance_rate>=80", "phase>=2");
    let skill2 = create_skill_with_precondition("skill2", "hp_per<=50", ""); // No precondition
    let skill3 = create_skill_with_precondition("skill3", "accumulate_time>=10", "hp_per<=30");

    let course = create_test_course();
    let mut rng = Pcg64Mcg::seed_from_u64(0);
    let scheduler = build_trigger_scheduler(&[skill1, skill2, skill3], &course, &mut rng);

    // Should have 3 skill logics
    assert_eq!(scheduler.skill_logics.len(), 3);

    // Verify precondition_satisfied array has same length
    assert_eq!(
        scheduler.precondition_satisfied.len(),
        scheduler.skill_logics.len(),
        "precondition_satisfied array should match skill_logics length"
    );

    // Verify initialization values
    // skill1: has precondition → should start as false (locked)
    assert_eq!(
        scheduler.precondition_satisfied[0], false,
        "Skill with precondition should start locked"
    );

    // skill2: no precondition → should start as true (unlocked)
    assert_eq!(
        scheduler.precondition_satisfied[1], true,
        "Skill without precondition should start unlocked"
    );

    // skill3: has precondition → should start as false (locked)
    assert_eq!(
        scheduler.precondition_satisfied[2], false,
        "Skill with precondition should start locked"
    );
}

#[test]
fn test_precondition_satisfied_index_correspondence() {
    use rand::SeedableRng;
    use rand_pcg::Pcg64Mcg;

    // Create skills with preconditions
    let skills = vec![
        create_skill_with_precondition("skill1", "distance_rate>=80", "phase>=2"),
        create_skill_with_precondition("skill2", "hp_per<=50", "accumulate_time>=10"),
        create_skill_with_precondition("skill3", "distance_rate>=50", ""),
    ];

    let course = create_test_course();
    let mut rng = Pcg64Mcg::seed_from_u64(0);
    let scheduler = build_trigger_scheduler(&skills, &course, &mut rng);

    // Verify parallel array correspondence
    for (idx, logic) in scheduler.skill_logics.iter().enumerate() {
        let has_precondition = logic.precondition.is_some();
        let is_unlocked = scheduler.precondition_satisfied[idx];

        // If has precondition, should start locked (false)
        // If no precondition, should start unlocked (true)
        assert_eq!(
            !has_precondition, is_unlocked,
            "Index {} mismatch: has_precondition={} but is_unlocked={}",
            idx, has_precondition, is_unlocked
        );
    }
}
