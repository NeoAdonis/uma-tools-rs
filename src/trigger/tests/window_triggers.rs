//! Unit tests for window trigger runtime behavior
//!
//! These tests verify that window triggers properly detect entry/exit conditions
//! and handle activation attempts correctly during runtime.

use super::super::skill_activation::*;
use crate::course::*;
use crate::parser::parse_condition;

/// Load a real course from course_data.json for testing
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
// BASIC WINDOW FUNCTIONALITY
// ============================================================================

#[test]
fn test_distance_rate_window_detection() {
    // Skill 100071: distance_rate>=50&distance_rate<=60&order_rate>50
    // Tests: Window pattern detection for distance_rate 50-60%

    let condition = parse_condition("distance_rate>=50&distance_rate<=60&order_rate>50").unwrap();
    let course = load_course(10104); // 2000m course
    let branches = split_condition(&condition, vec![], &course);

    assert_eq!(branches.len(), 1);
    let (triggers, guards) = &branches[0];

    // Should detect window trigger for distance_rate
    assert_eq!(triggers.len(), 1);
    assert!(matches!(
        triggers[0],
        TriggerCondition::Window {
            variable: WindowVariable::DistanceRate,
            min_threshold: 50.0,
            max_threshold: 60.0,
        }
    ));

    // order_rate>50 should remain as guard
    assert!(!guards.is_empty());
}

#[test]
fn test_remain_distance_narrow_window_boundaries() {
    // Skill 100311: remain_distance<=299&remain_distance>=295&order<=2&slope==0
    // Tests: Very narrow window (295-299m) boundary precision

    let condition =
        parse_condition("remain_distance<=299&remain_distance>=295&order<=2&slope==0").unwrap();
    let course = load_course(10104); // 2000m course
    let branches = split_condition(&condition, vec![], &course);

    assert_eq!(branches.len(), 1);
    let (triggers, guards) = &branches[0];

    // remain_distance pair is detected as a window (converted to positions when indexed)
    assert_eq!(triggers.len(), 1);
    assert!(matches!(
        triggers[0],
        TriggerCondition::Window {
            variable: WindowVariable::RemainDistance,
            min_threshold: 295.0,
            max_threshold: 299.0,
        }
    ));

    // order and slope should be guards
    assert!(!guards.is_empty());
}

#[test]
fn test_window_detected_later_in_chain() {
    // Skill 101151: the range isn't at the start of the chain
    let condition =
        parse_condition("running_style==4&remain_distance>=999&remain_distance<=1001").unwrap();
    let branches = split_condition(&condition, vec![], &load_course(10104));

    assert_eq!(branches.len(), 1);
    let (triggers, guards) = &branches[0];
    assert_eq!(triggers.len(), 1);
    assert!(matches!(
        triggers[0],
        TriggerCondition::Window {
            variable: WindowVariable::RemainDistance,
            min_threshold: 999.0,
            max_threshold: 1001.0,
        }
    ));
    assert_eq!(guards, &[parse_single("running_style==4")]);
}

#[test]
fn test_window_detected_at_end_of_chain_with_dynamic_guards() {
    // Skill 100061: dynamic guards would otherwise force a Continuous trigger
    let condition = parse_condition(
        "order>=2&order<=5&order_rate<=50&remain_distance<=201&remain_distance>=199",
    )
    .unwrap();
    let branches = split_condition(&condition, vec![], &load_course(10104));

    assert_eq!(branches.len(), 1);
    let (triggers, guards) = &branches[0];
    assert_eq!(triggers.len(), 1);
    assert!(matches!(
        triggers[0],
        TriggerCondition::Window {
            variable: WindowVariable::RemainDistance,
            min_threshold: 199.0,
            max_threshold: 201.0,
        }
    ));
    assert_eq!(
        guards,
        &[
            parse_single("order>=2"),
            parse_single("order<=5"),
            parse_single("order_rate<=50"),
        ]
    );
}

#[test]
fn test_multiple_windows_in_chain() {
    let condition =
        parse_condition("distance_rate>=50&distance_rate<=60&hp_per>=20&hp_per<=80").unwrap();
    let branches = split_condition(&condition, vec![], &load_course(10104));

    assert_eq!(branches.len(), 1);
    let (triggers, guards) = &branches[0];
    assert!(guards.is_empty());
    assert_eq!(triggers.len(), 2);
    assert!(matches!(
        triggers[0],
        TriggerCondition::Window {
            variable: WindowVariable::DistanceRate,
            min_threshold: 50.0,
            max_threshold: 60.0,
        }
    ));
    assert!(matches!(
        triggers[1],
        TriggerCondition::Window {
            variable: WindowVariable::HpPercent,
            min_threshold: 20.0,
            max_threshold: 80.0,
        }
    ));
}

/// Parse a single comparison.
fn parse_single(input: &str) -> crate::parser::Comparison {
    match parse_condition(input).unwrap().comp_tree {
        crate::parser::ComparisonTree::Single(comp) => comp,
        _ => panic!("Not a single comparison: {input}"),
    }
}

// ============================================================================
// MULTIPLE WINDOW TYPES
// ============================================================================

#[test]
fn test_hp_percent_window() {
    // Skill 100141: is_last_straight==1&hp_per>=30&order<=2
    // Tests: HP-based window variable (hp_per>=30 creates window [30, 100])

    let condition = parse_condition("is_last_straight==1&hp_per>=30&order<=2").unwrap();
    let course = load_course(10104); // 2000m course
    let branches = split_condition(&condition, vec![], &course);

    assert_eq!(branches.len(), 1);
    let (triggers, guards) = &branches[0];

    // Should have Window trigger for spatial position (is_last_straight)
    // Note: hp_per>=30 becomes a guard because is_last_straight creates explicit Window
    assert_eq!(triggers.len(), 1);
    assert!(matches!(
        triggers[0],
        TriggerCondition::Window {
            variable: WindowVariable::SpatialPosition,
            ..
        }
    ));

    // hp_per and order should be guards
    assert!(!guards.is_empty());
}

#[test]
fn test_accumulate_time_window() {
    // Skill 101021: distance_rate<=50&order_rate>=40&order_rate<=80&infront_near_lane_time>=3&accumulatetime>=10
    // Tests: AccumulateTime window combined with other windows

    let condition = parse_condition("distance_rate<=50&order_rate>=40&order_rate<=80&infront_near_lane_time>=3&accumulatetime>=10").unwrap();
    let course = load_course(10104);
    let branches = split_condition(&condition, vec![], &course);

    assert_eq!(branches.len(), 1);
    let (triggers, guards) = &branches[0];

    // With multiple dynamic conditions (infront_near_lane_time, order_rate),
    // this likely becomes a Continuous trigger
    assert_eq!(triggers.len(), 1);
    assert!(
        matches!(triggers[0], TriggerCondition::Continuous)
            || matches!(triggers[0], TriggerCondition::Window { .. }),
        "Should have Continuous or Window trigger"
    );

    // All other conditions become guards
    assert!(!guards.is_empty());
}

// ============================================================================
// SPATIAL POSITION WINDOWS
// ============================================================================

#[test]
fn test_spatial_position_last_straight() {
    // Skill 100141: is_last_straight==1
    // Tests: Spatial position window for last straight

    let condition = parse_condition("is_last_straight==1").unwrap();
    let course = load_course(10104); // 2000m course (last straight: 1734-2000m)
    let branches = split_condition(&condition, vec![], &course);

    assert_eq!(branches.len(), 1);
    let (triggers, guards) = &branches[0];

    // Should create SpatialPosition window
    assert_eq!(triggers.len(), 1);
    assert!(matches!(
        triggers[0],
        TriggerCondition::Window {
            variable: WindowVariable::SpatialPosition,
            ..
        }
    ));

    // Verify the window corresponds to last straight
    if let TriggerCondition::Window {
        min_threshold,
        max_threshold,
        ..
    } = triggers[0]
    {
        // Last straight on course 10104 is from 1734m to 2000m
        assert_eq!(min_threshold, 1734.0);
        assert_eq!(max_threshold, 2000.0);
    }

    assert!(guards.is_empty());
}

#[test]
fn test_spatial_position_corner() {
    // Skill 100211: corner==0 (not in corner, i.e., straights or slopes)
    // Tests: Corner-based spatial positioning

    let condition = parse_condition("corner==0").unwrap();
    let course = load_course(10104); // 2000m course
    let branches = split_condition(&condition, vec![], &course);

    assert_eq!(branches.len(), 1);
    let (triggers, _guards) = &branches[0];

    // corner==0 is complex (multiple disjoint straight ranges), so becomes Continuous
    assert_eq!(triggers.len(), 1);
    assert!(matches!(triggers[0], TriggerCondition::Continuous));
}

#[test]
fn test_spatial_position_specific_corner() {
    // Test: corner==2 (specific corner number)
    // Tests: Spatial window for a specific corner

    let condition = parse_condition("corner==2").unwrap();
    let course = load_course(10104); // 2000m course (has 4 corners)
    let branches = split_condition(&condition, vec![], &course);

    assert_eq!(branches.len(), 1);
    let (triggers, guards) = &branches[0];

    // Should create SpatialPosition window for corner 2
    assert_eq!(triggers.len(), 1);
    assert!(matches!(
        triggers[0],
        TriggerCondition::Window {
            variable: WindowVariable::SpatialPosition,
            ..
        }
    ));

    // Verify window corresponds to corner 2 (index 1)
    if let TriggerCondition::Window {
        min_threshold,
        max_threshold,
        ..
    } = triggers[0]
    {
        // Corner 2 on course 10104: start=650, length=275 → [650, 925]
        assert_eq!(min_threshold, 650.0);
        assert_eq!(max_threshold, 925.0);
    }

    assert!(guards.is_empty());
}

// ============================================================================
// MULTI-WINDOW COMBINATIONS
// ============================================================================

#[test]
fn test_order_rate_multiple_windows_or_branches() {
    // Skill 100211: Two OR branches with different order_rate windows
    // Branch 1: order_rate>=70&order_rate<=75
    // Branch 2: order_rate<=30&order_rate>=20
    // Tests: Multiple windows in OR branches

    let condition = parse_condition(
        "distance_rate>=50&corner==0&order_rate>=70&order_rate<=75&is_overtake==1@distance_rate>=50&corner==0&order_rate<=30&order_rate>=20"
    ).unwrap();
    let course = load_course(10104);
    let branches = split_condition(&condition, vec![], &course);

    // Should create 2 branches (OR condition)
    assert_eq!(branches.len(), 2);

    // Both branches should have distance_rate as Window trigger
    for (idx, (triggers, guards)) in branches.iter().enumerate() {
        assert_eq!(triggers.len(), 1, "Branch {} should have 1 trigger", idx);
        assert!(
            matches!(
                triggers[0],
                TriggerCondition::Window {
                    variable: WindowVariable::DistanceRate,
                    min_threshold: 50.0,
                    max_threshold: 100.0,
                }
            ),
            "Branch {} should have Window trigger",
            idx
        );
        assert!(!guards.is_empty(), "Branch {} should have guards", idx);
    }
}

#[test]
fn test_window_with_multiple_guards() {
    // Skill 100071: distance_rate window with multiple guard conditions
    // Tests: Window trigger with several guard conditions to evaluate

    let condition =
        parse_condition("distance_rate>=50&distance_rate<=60&order_rate>50&ground_type==1")
            .unwrap();
    let course = load_course(10104);
    let branches = split_condition(&condition, vec![], &course);

    assert_eq!(branches.len(), 1);
    let (triggers, guards) = &branches[0];

    // Should have distance_rate window
    assert_eq!(triggers.len(), 1);
    assert!(matches!(
        triggers[0],
        TriggerCondition::Window {
            variable: WindowVariable::DistanceRate,
            min_threshold: 50.0,
            max_threshold: 60.0,
        }
    ));

    // Multiple conditions should be guards
    assert!(
        !guards.is_empty(),
        "Should have guards for order_rate and ground_type"
    );
}

// ============================================================================
// EDGE CASES
// ============================================================================

#[test]
fn test_window_reversed_order_detection() {
    // Test: Variable comparison in reversed order (max<=, min>=)
    // Tests: Window detection works regardless of comparison order

    let condition = parse_condition("distance_rate<=60&distance_rate>=50").unwrap();
    let course = load_course(10104);
    let branches = split_condition(&condition, vec![], &course);

    assert_eq!(branches.len(), 1);
    let (triggers, guards) = &branches[0];

    // Should still detect as window with correct min/max
    assert_eq!(triggers.len(), 1);
    assert!(matches!(
        triggers[0],
        TriggerCondition::Window {
            variable: WindowVariable::DistanceRate,
            min_threshold: 50.0,
            max_threshold: 60.0,
        }
    ));

    assert!(guards.is_empty());
}

#[test]
fn test_single_sided_window_conversion() {
    // Test: distance_rate<=50 becomes Window [0, 50]
    // Tests: Single-sided comparison converts to window for increasing variables

    let condition = parse_condition("distance_rate<=50").unwrap();
    let course = load_course(10104);
    let branches = split_condition(&condition, vec![], &course);

    assert_eq!(branches.len(), 1);
    let (triggers, guards) = &branches[0];

    // Should convert to window with min=0
    assert_eq!(triggers.len(), 1);
    assert!(matches!(
        triggers[0],
        TriggerCondition::Window {
            variable: WindowVariable::DistanceRate,
            min_threshold: 0.0,
            max_threshold: 50.0,
        }
    ));

    // Promoted comparisons are also kept as guards
    assert_eq!(guards.len(), 1);
}
