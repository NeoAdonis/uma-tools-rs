//! Comprehensive tests for skill activation trigger-guard separation system

use super::*;

// ============================================================================
// BASIC SPLITTING TESTS
// ============================================================================

#[test]
fn test_split_simple_guard_only() {
    // Condition with only static guards (no triggers, no monotonic guards)
    let condition = parse_condition("ground_type==2&motivation>=4").unwrap();
    let branches = split_condition(&condition, vec![]);

    // Should have 1 branch
    assert_eq!(branches.len(), 1);

    let (triggers, guards) = &branches[0];

    // Should have continuous trigger (fallback when no explicit triggers or monotonic guards)
    assert_eq!(triggers.len(), 1);
    assert!(matches!(triggers[0], TriggerCondition::Continuous));

    // Should preserve both guards
    assert!(guards.is_some());
}

#[test]
fn test_split_hp_trigger_with_guards() {
    // Condition: hp_per<=50 & ground_type==2
    // hp_per is decreasing, so <= is the normal direction (not reversed)
    let condition = parse_condition("hp_per<=50&ground_type==2").unwrap();
    let branches = split_condition(&condition, vec![]);

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
    assert!(guards.is_some());
}

#[test]
fn test_split_random_gate_with_guards() {
    // Condition: phase_random==1 & hp_per>=20
    let condition = parse_condition("phase_random==1&hp_per>=20").unwrap();

    // Simulate random activation
    let activation = vec![800.0];

    let branches = split_condition(&condition, activation);

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
    assert!(guards.is_some());
}

#[test]
fn test_split_onetime_event() {
    // Condition: is_last_straight_onetime==1
    let condition = parse_condition("is_last_straight_onetime==1").unwrap();
    let branches = split_condition(&condition, vec![]);

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
    assert!(guards.is_none());
}

#[test]
fn test_split_or_condition() {
    // Condition: (phase_random==1 & hp_per>=20) @ (corner!=0 & ground_type==1)
    // This should create 2 separate branches
    let condition =
        parse_condition("phase_random==1&hp_per>=20@corner!=0&ground_type==1").unwrap();

    let activation = vec![800.0];

    let branches = split_condition(&condition, activation);

    // Should have 2 branches (one for each OR alternative)
    assert_eq!(branches.len(), 2);

    // Branch 1: random trigger + hp guard
    let (triggers1, guards1) = &branches[0];
    assert!(
        triggers1
            .iter()
            .any(|t| matches!(t, TriggerCondition::DistanceCovered { .. }))
    );
    assert!(guards1.is_some());

    // Branch 2: corner trigger + ground_type guard
    let (triggers2, guards2) = &branches[1];
    assert!(
        triggers2
            .iter()
            .any(|t| matches!(t, TriggerCondition::CornerTransition { .. }))
    );
    assert!(guards2.is_some());
}

// ============================================================================
// OPERATOR DIRECTION TESTS (THE FIX)
// ============================================================================

#[test]
fn test_distance_rate_less_than_becomes_window() {
    // Bug fix: distance_rate<=50 should become Window [0, 50], not DistanceCovered trigger
    // distance_rate is monotonically increasing, so <= is reversed direction
    let condition = parse_condition("distance_rate<=50&ground_type==2").unwrap();
    let branches = split_condition(&condition, vec![]);

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
    assert!(guards.is_some());
}

#[test]
fn test_distance_rate_greater_than_stays_trigger() {
    // distance_rate>=50 should remain as DistanceCovered trigger (correct direction)
    let condition = parse_condition("distance_rate>=50&ground_type==2").unwrap();
    let branches = split_condition(&condition, vec![]);

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
    assert!(guards.is_some());
}

#[test]
fn test_accumulatetime_less_than_becomes_window() {
    // Bug fix: accumulatetime<=10 should become Window [0, 10], not Time trigger
    // accumulatetime is monotonically increasing, so <= is reversed direction
    let condition = parse_condition("accumulatetime<=10&ground_type==2").unwrap();
    let branches = split_condition(&condition, vec![]);

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
    assert!(guards.is_some());
}

#[test]
fn test_accumulatetime_greater_than_stays_trigger() {
    // accumulatetime>=10 should remain as Time trigger (correct direction)
    let condition = parse_condition("accumulatetime>=10&ground_type==2").unwrap();
    let branches = split_condition(&condition, vec![]);

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
    assert!(guards.is_some());
}

#[test]
fn test_hp_per_greater_than_becomes_window() {
    // Bug fix: hp_per>=50 should become Window [50, 100] for decreasing variable
    // hp_per is decreasing (damage), so >= is reversed direction
    let condition = parse_condition("hp_per>=50&ground_type==2").unwrap();
    let branches = split_condition(&condition, vec![]);

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
    assert!(guards.is_some());
}

#[test]
fn test_hp_per_less_than_stays_trigger() {
    // hp_per<=50 should remain as HpPercent trigger (correct direction for decreasing)
    let condition = parse_condition("hp_per<=50&ground_type==2").unwrap();
    let branches = split_condition(&condition, vec![]);

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
    assert!(guards.is_some());
}

// ============================================================================
// BROKEN SKILLS FROM ANALYSIS (now fixed)
// ============================================================================

#[test]
fn test_skill_100901_distance_rate_less_than() {
    // Skill 100901: "distance_rate<=50&corner!=0&order_rate<=50&ground_type==1"
    // With corner!=0 (explicit trigger), distance_rate<=50 stays as guard (Rule 1)
    // But this is actually where the bug manifests!
    // distance_rate<=50 as a guard will be checked when corner trigger fires,
    // but at that point we might be at 60% distance, making the guard fail.
    //
    // The FIX: When a monotonic guard with reversed operator is promoted,
    // it should become a Window trigger instead of a threshold trigger.
    // However, per MonotonicGuard promotion rules, it won't be promoted
    // when there's already an explicit trigger (corner).
    //
    // The real solution: distance_rate<=50 should ALWAYS become a window
    // when it's reversed direction, even if there's an explicit trigger.
    // This requires a deeper fix in the promotion logic.

    let condition = parse_condition("distance_rate<=50&corner!=0&order_rate<=50&ground_type==1").unwrap();
    let branches = split_condition(&condition, vec![]);

    assert_eq!(branches.len(), 1);
    let (triggers, guards) = &branches[0];

    // AFTER FIX: order_rate is a DynamicGuard!
    // So this becomes Continuous trigger (checked every frame)
    // This is correct because order_rate can change during the race
    assert_eq!(triggers.len(), 1);
    assert!(matches!(triggers[0], TriggerCondition::Continuous));

    // All conditions become guards
    assert!(guards.is_some());
}

#[test]
fn test_less_operator_also_converts() {
    // Test that < (not just <=) also converts for increasing variables
    let condition = parse_condition("distance_rate<50&ground_type==2").unwrap();
    let branches = split_condition(&condition, vec![]);

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

    assert!(guards.is_some());
}

#[test]
fn test_greater_operator_also_converts() {
    // Test that > (not just >=) also converts for decreasing variables
    let condition = parse_condition("hp_per>50&ground_type==2").unwrap();
    let branches = split_condition(&condition, vec![]);

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

    assert!(guards.is_some());
}

#[test]
fn test_window_detection_still_works() {
    // Ensure existing window detection (distance_rate>=50 & distance_rate<=60) still works
    let condition = parse_condition("distance_rate>=50&distance_rate<=60").unwrap();
    let branches = split_condition(&condition, vec![]);

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
    assert!(guards.is_none());
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

    let condition = parse_condition("phase<=1&accumulatetime>=5&blocked_front_continuetime>=1").unwrap();
    let branches = split_condition(&condition, vec![]);

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
    assert!(guards.is_some());

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
    let branches = split_condition(&condition, vec![]);

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
    assert!(guards.is_some());
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
    let branches = split_condition(&condition, vec![]);

    assert_eq!(branches.len(), 1);
    let (triggers, guards) = &branches[0];

    // Should use Continuous trigger (not Time) because blocked_front_continuetime is DynamicGuard
    assert_eq!(triggers.len(), 1);
    assert!(matches!(triggers[0], TriggerCondition::Continuous));

    // All conditions should be guards
    assert!(guards.is_some());
}

#[test]
fn test_static_guards_allow_monotonic_trigger() {
    // Skill with only StaticGuards can safely use monotonic triggers
    // Example: accumulatetime>=5 & ground_type==2
    //
    // This is safe because ground_type never changes during race.
    // If guard fails at t=5s, it will never pass.

    let condition = parse_condition("accumulatetime>=5&ground_type==2").unwrap();
    let branches = split_condition(&condition, vec![]);

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
    assert!(guards.is_some());
}
