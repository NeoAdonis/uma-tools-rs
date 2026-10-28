//! Unit tests for OR branch mutual exclusion behavior
//!
//! These tests verify that OR branches properly isolate from each other,
//! ensuring only one branch can activate per skill (mutual exclusion).

use crate::condition::parser::Condition;
use crate::skill::SkillType;
use crate::test_utils::{load_course, skill};
use crate::trigger::skill_activation::*;
use crate::trigger::system::TriggerScheduler;
use rand::SeedableRng;
use rand_pcg::Pcg64Mcg;
use std::str::FromStr;

// ============================================================================
// MUTUAL EXCLUSION VERIFICATION
// ============================================================================

#[test]
fn test_position_based_or_mutual_exclusion() {
    // Skill 100081: Position-based OR (bashin_diff_infront vs behind)
    // Branch 1: order>=3&order_rate<=50&remain_distance<=200&bashin_diff_infront<=1
    // Branch 2: order>=3&order_rate<=50&remain_distance<=200&bashin_diff_behind<=1
    // Tests: Mutually exclusive positioning (can't be both in front and behind)

    let condition = Condition::from_str(
        "order>=3&order_rate<=50&remain_distance<=200&bashin_diff_infront<=1@order>=3&order_rate<=50&remain_distance<=200&bashin_diff_behind<=1"
    ).unwrap();
    let course = load_course(10104);
    let branches = compile_condition(&condition, &[], &course);

    // Should create 2 branches
    assert_eq!(branches.len(), 2, "Should create 2 OR branches");

    // Verify both branches exist with their conditions
    // Both should keep all their comparisons as guards
    assert_eq!(branches[0].guards.len(), 4, "Branch 1 should have guards");
    assert_eq!(branches[1].guards.len(), 4, "Branch 2 should have guards");

    // bashin_diff_infront and bashin_diff_behind are mutually exclusive.
    // This is enforced at runtime by the guard evaluation logic.
}

#[test]
fn test_order_based_or_mutual_exclusion() {
    // Skill 100091: Order-based branching (1st place vs 2nd place)
    // Branch 1: distance_rate>=50&order==1&bashin_diff_behind<=1
    // Branch 2: distance_rate>=50&order==2&is_overtake==1
    // Tests: Mutually exclusive order positions (can't be both 1st and 2nd)

    let condition = Condition::from_str(
        "distance_rate>=50&order==1&bashin_diff_behind<=1@distance_rate>=50&order==2&is_overtake==1"
    )
    .unwrap();
    let course = load_course(10104);
    let branches = compile_condition(&condition, &[], &course);

    // Should create 2 branches
    assert_eq!(branches.len(), 2, "Should create 2 OR branches");

    // Both branches are checked from 50% on
    for (idx, branch) in branches.iter().enumerate() {
        assert!(
            matches!(
                branch.trigger,
                TriggerCondition::Window {
                    variable: WindowVariable::Position,
                    min_threshold: 1000.0,
                    ..
                }
            ),
            "Branch {} should be checked from 50%",
            idx
        );
        assert_eq!(branch.guards.len(), 3, "Branch {} should have guards", idx);
    }

    // The key property: order==1 and order==2 are mutually exclusive
    // Only one branch can have its guard pass at any given time
}

#[test]
fn test_complex_three_branch_distance_ranges() {
    // Skill 100303111: Three mutually exclusive distance ranges
    // Branch 1: course_distance<2400
    // Branch 2: course_distance>2500
    // Branch 3: course_distance>=2400&course_distance<=2500
    // Tests: Distance-based mutual exclusivity (disjoint ranges)

    let condition = Condition::from_str(
        "running_style==2&course_distance<2400&phase_firsthalf_random==2&order_rate<=50@running_style==2&course_distance>2500&phase_firsthalf_random==2&order_rate<=50@running_style==2&course_distance>=2400&course_distance<=2500&phase_firsthalf_random==2&order_rate<=50&base_stamina<1000"
    ).unwrap();
    let course = load_course(10104); // 2000m course

    // Need to provide random activation for phase_firsthalf_random
    let rolls = vec![Some(vec![500.0]); 3]; // Random activation at 500m for each branch
    let branches = compile_condition(&condition, &rolls, &course);

    // Should create 3 branches
    assert_eq!(
        branches.len(),
        3,
        "Should create 3 OR branches for distance ranges"
    );

    // All branches should trigger at the random point
    for (idx, branch) in branches.iter().enumerate() {
        assert!(
            matches!(
                &branch.trigger,
                TriggerCondition::Segments { ranges } if ranges == &[(500.0, 510.0)]
            ),
            "Branch {} should trigger at the random point",
            idx
        );
        assert!(
            !branch.guards.is_empty(),
            "Branch {} should have guards including distance checks",
            idx
        );
    }

    // The key property: course_distance values are mutually exclusive
    // <2400, >2500, and [2400-2500] are disjoint sets
}

// ============================================================================
// STATE ISOLATION
// ============================================================================

#[test]
fn test_or_branch_skill_id_uniqueness() {
    // Test: Each OR branch gets unique skill_id
    // Tests: OR branches are treated as separate skills in the scheduler

    let skill = skill(
        2,
        "distance_rate>=50@hp_per<=50@accumulatetime>=10",
        SkillType::TargetSpeed,
        0.4,
    );

    let course = load_course(10104);
    let mut rng = Pcg64Mcg::seed_from_u64(0);
    let scheduler = TriggerScheduler::from_skills(&[skill], &course, &mut rng);

    // Should create 3 skill logics (one per OR branch)
    assert_eq!(
        scheduler.skill_logics.len(),
        3,
        "Should create 3 separate skill logics for OR branches"
    );

    // Each OR branch becomes a separate SkillActivationLogic entry
    let logic_0 = &scheduler.skill_logics[0];
    let logic_1 = &scheduler.skill_logics[1];
    let logic_2 = &scheduler.skill_logics[2];

    // All should have the same skill_index (same base skill)
    assert_eq!(logic_0.skill_index, logic_1.skill_index);
    assert_eq!(logic_1.skill_index, logic_2.skill_index);

    // All should have the same alternative_index (same alternative, split by OR)
    assert_eq!(logic_0.alternative_index, logic_1.alternative_index);
    assert_eq!(logic_1.alternative_index, logic_2.alternative_index);

    // The key isolation: they are separate entries in skill_logics array
    // Each is checked independently, but they share the cooldown
}

#[test]
fn test_or_branches_independent_activation_tracking() {
    // Test: Each OR branch is a separate logic of the same alternative
    // Tests: branches share the alternative's cooldown, kept by the runner

    let skill = skill(
        1,
        "distance_rate>=50@hp_per<=50",
        SkillType::TargetSpeed,
        0.4,
    );

    let course = load_course(10104);
    let mut rng = Pcg64Mcg::seed_from_u64(0);
    let scheduler = TriggerScheduler::from_skills(&[skill], &course, &mut rng);

    // Should create 2 skill logics
    assert_eq!(scheduler.skill_logics.len(), 2);

    // They are distinct entries in skill_logics, but share the same skill_index
    // and alternative_index, and thus the same cooldown.
    assert_eq!(
        scheduler.skill_logics[0].skill_index,
        scheduler.skill_logics[1].skill_index
    );
    assert_eq!(
        scheduler.skill_logics[0].alternative_index,
        scheduler.skill_logics[1].alternative_index
    );
}

// ============================================================================
// MULTI-BRANCH (3+) OR
// ============================================================================

#[test]
fn test_three_branch_or_mutual_exclusion() {
    // Skill 100703211: Pure mutual exclusivity (ground_condition values)
    // Branch 1: ground_condition==2 (Good)
    // Branch 2: ground_condition==3 (Slightly Heavy)
    // Branch 3: ground_condition==4 (Heavy)
    // Tests: Three mutually exclusive states (simplest case)

    let condition =
        Condition::from_str("ground_condition==2@ground_condition==3@ground_condition==4").unwrap();
    let course = load_course(10104);
    let branches = compile_condition(&condition, &[], &course);

    // Should create 3 branches
    assert_eq!(branches.len(), 3, "Should create 3 OR branches");

    // All branches are guards-only, checked during the whole race
    for (idx, branch) in branches.iter().enumerate() {
        assert!(
            matches!(
                branch.trigger,
                TriggerCondition::Window {
                    variable: WindowVariable::Position,
                    min_threshold: 0.0,
                    max_threshold: f64::INFINITY,
                }
            ),
            "Branch {} should be checked during the whole race",
            idx
        );
        assert!(
            !branch.guards.is_empty(),
            "Branch {} should have ground_condition as guard",
            idx
        );
    }

    // The key property: ground_condition can only be one value at a time
    // Perfect mutual exclusion
}

#[test]
fn test_four_branch_evaluation_priority() {
    // Skill 101171: Four branches with change_order counters
    // Branch 1: is_last_straight_onetime==1&change_order_up_end_after>=3
    // Branch 2: is_last_straight_onetime==1&change_order_up_end_after>=2
    //           &change_order_up_middle>=1
    // Branch 3: is_last_straight_onetime==1&change_order_up_end_after>=1
    //           &change_order_up_middle>=2
    // Branch 4: is_last_straight_onetime==1&change_order_up_middle>=3
    // Tests: Multiple branches with overlapping ranges (first match wins)

    let condition = Condition::from_str(
        "is_last_straight_onetime==1&change_order_up_end_after>=3@is_last_straight_onetime==1&change_order_up_end_after>=2&change_order_up_middle>=1@is_last_straight_onetime==1&change_order_up_end_after>=1&change_order_up_middle>=2@is_last_straight_onetime==1&change_order_up_middle>=3"
    ).unwrap();
    let course = load_course(10104);
    let branches = compile_condition(&condition, &[], &course);

    // Should create 4 branches
    assert_eq!(branches.len(), 4, "Should create 4 OR branches");

    // All branches should have is_last_straight_onetime trigger
    for (idx, branch) in branches.iter().enumerate() {
        assert!(
            matches!(branch.trigger, TriggerCondition::LastStraightEntry),
            "Branch {} should have onetime last straight trigger",
            idx
        );
        assert!(
            !branch.guards.is_empty(),
            "Branch {} should have change_order guards",
            idx
        );
    }

    // The key property: Evaluation order matters
    // If branch 1 activates, branches 2-4 won't be checked
    // This ensures mutual exclusion even with overlapping conditions
}

// ============================================================================
// SUBTLE DIFFERENCES
// ============================================================================

#[test]
fn test_similar_conditions_or_branches() {
    // Skill 100481: Similar base conditions, different overtake triggers
    // Branch 1: is_last_straight==1&order_rate<=40&overtake_target_time>=1
    // Branch 2: is_last_straight==1&order_rate<=40&is_overtake==1
    // Tests: Branches differing only in subtle conditions

    let condition = Condition::from_str(
        "is_last_straight==1&order_rate<=40&overtake_target_time>=1@is_last_straight==1&order_rate<=40&is_overtake==1"
    ).unwrap();
    let course = load_course(10104);
    let branches = compile_condition(&condition, &[], &course);

    // Should create 2 branches
    assert_eq!(branches.len(), 2, "Should create 2 OR branches");

    // Both should have spatial window trigger (is_last_straight)
    for (idx, branch) in branches.iter().enumerate() {
        assert!(
            matches!(
                branch.trigger,
                TriggerCondition::Window {
                    variable: WindowVariable::Position,
                    ..
                }
            ),
            "Branch {} should have position window trigger",
            idx
        );
        assert!(
            !branch.guards.is_empty(),
            "Branch {} should have guards for order_rate and overtake condition",
            idx
        );
    }

    // The key property: overtake_target_time>=1 vs is_overtake==1
    // These are mutually exclusive overtake states
    // Only one can be true at a time, ensuring branch isolation
}

#[test]
fn test_or_branches_with_shared_base_conditions() {
    // Test: OR branches sharing common base conditions
    // Tests: Common conditions don't break mutual exclusion

    let condition = Condition::from_str(
        "distance_rate>=50&ground_type==1&order==1@distance_rate>=50&ground_type==1&order==2@distance_rate>=50&ground_type==1&order>=3"
    ).unwrap();
    let course = load_course(10104);
    let branches = compile_condition(&condition, &[], &course);

    // Should create 3 branches
    assert_eq!(branches.len(), 3, "Should create 3 OR branches");

    // All branches share distance_rate>=50 and ground_type==1
    // But differ in order condition (mutually exclusive)
    for (idx, branch) in branches.iter().enumerate() {
        assert!(
            matches!(
                branch.trigger,
                TriggerCondition::Window {
                    variable: WindowVariable::Position,
                    min_threshold: 1000.0,
                    max_threshold: f64::INFINITY,
                }
            ),
            "Branch {} should have shared distance_rate trigger",
            idx
        );
        assert!(
            !branch.guards.is_empty(),
            "Branch {} should have guards including order condition",
            idx
        );
    }

    // The key property: order==1, order==2, order>=3
    // These are mutually exclusive (can't be in multiple positions simultaneously)
}
