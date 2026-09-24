//! Unit tests for precondition build-time behavior
//!
//! These tests complement the existing precondition tests in skill_activation.rs
//! by focusing on precondition-trigger interaction and edge cases.
//!
//! Note: Runtime precondition tests (unlock mechanics, per-frame checking) are
//! deferred to integration tests.

use super::super::skill_activation::*;
use crate::course::*;
use crate::trigger::builder::build_trigger_scheduler;
use crate::uma::skill::*;
use rand::SeedableRng;
use rand_pcg::Pcg64Mcg;

/// Helper function to create a skill with precondition
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
// PRECONDITION-TRIGGER INTERACTION (BUILD-TIME)
// ============================================================================

#[test]
fn test_precondition_with_window_trigger() {
    // Skill 100031: Complex precondition with spatial window trigger
    // Precondition: phase>=2&order<=3&bashin_diff_infront<=1&is_overtake==1
    // Condition: is_last_straight==1
    // Tests: Complex precondition properly separated from spatial window trigger

    let skill = create_skill_with_precondition(
        "100031",
        "is_last_straight==1",
        "phase>=2&order<=3&bashin_diff_infront<=1&is_overtake==1",
    );

    let course = load_course(10104); // 2000m course
    let mut rng = Pcg64Mcg::seed_from_u64(0);
    let scheduler = build_trigger_scheduler(&[skill], &course, &mut rng);

    // Should create 1 skill logic
    assert_eq!(scheduler.skill_logics.len(), 1);
    let logic = &scheduler.skill_logics[0];

    // Verify precondition exists and is separate from condition
    assert!(
        logic.precondition.is_some(),
        "Precondition should be parsed and stored separately"
    );

    // Verify trigger is spatial window (not affected by precondition)
    assert_eq!(1, 1);
    let (triggers, guards) = &(logic.triggers.as_slice(), logic.guards.as_slice());
    assert_eq!(triggers.len(), 1);
    assert!(
        matches!(
            triggers[0],
            TriggerCondition::Window {
                variable: WindowVariable::SpatialPosition,
                ..
            }
        ),
        "Condition should create spatial window trigger"
    );

    // is_last_straight==1 has no additional guards
    assert!(guards.is_empty());

    // Verify precondition_satisfied array is initialized correctly
    assert_eq!(
        scheduler.precondition_satisfied.len(),
        scheduler.skill_logics.len()
    );
    assert_eq!(
        scheduler.precondition_satisfied[0], false,
        "Skill with precondition should start locked"
    );
}

#[test]
fn test_precondition_with_or_condition() {
    // Test: Precondition applies to all OR branches equally
    // Tests: Each OR branch inherits the same precondition

    let skill = create_skill_with_precondition(
        "test_or_precond",
        "distance_rate>=80@hp_per<=50@accumulatetime>=10", // 3 OR branches
        "phase>=2&ground_type==1",                         // Shared precondition
    );

    let course = load_course(10104);
    let mut rng = Pcg64Mcg::seed_from_u64(0);
    let scheduler = build_trigger_scheduler(&[skill], &course, &mut rng);

    // Should create 3 branches (OR condition)
    assert_eq!(
        scheduler.skill_logics.len(),
        3,
        "OR condition with 3 branches should create 3 skill logics"
    );

    // Verify all branches have the same precondition
    for (idx, logic) in scheduler.skill_logics.iter().enumerate() {
        assert!(
            logic.precondition.is_some(),
            "Branch {} should have precondition",
            idx
        );

        // All should start locked (precondition not satisfied)
        assert_eq!(
            scheduler.precondition_satisfied[idx], false,
            "Branch {} should start locked",
            idx
        );
    }

    // Verify precondition count matches logic count
    assert_eq!(
        scheduler.precondition_satisfied.len(),
        scheduler.skill_logics.len(),
        "Precondition satisfied array should match skill logics length"
    );
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
                    precondition: "accumulatetime>=10".to_string(), // Different precondition
                    base_duration: 40000,
                    effects: vec![SkillEffect {
                        skill_type: SkillType::TargetSpeed,
                        target: SkillTarget::Oneself,
                        modifier: 3000,
                    }],
                },
                SkillAlternative {
                    condition: "is_last_straight==1".to_string(),
                    precondition: String::new(), // No precondition
                    base_duration: 40000,
                    effects: vec![SkillEffect {
                        skill_type: SkillType::TargetSpeed,
                        target: SkillTarget::Oneself,
                        modifier: 5000,
                    }],
                },
            ],
            wisdom_check: true,
        },
    );

    let course = load_course(10104);
    let mut rng = Pcg64Mcg::seed_from_u64(0);
    let scheduler = build_trigger_scheduler(&[skill], &course, &mut rng);

    // Should have 3 skill logics (one per alternative)
    assert_eq!(scheduler.skill_logics.len(), 3);

    // Alternative 1: has precondition (phase>=2)
    assert!(
        scheduler.skill_logics[0].precondition.is_some(),
        "First alternative should have precondition"
    );
    assert_eq!(
        scheduler.precondition_satisfied[0], false,
        "First alternative should start locked"
    );

    // Alternative 2: has different precondition (accumulatetime>=10)
    assert!(
        scheduler.skill_logics[1].precondition.is_some(),
        "Second alternative should have precondition"
    );
    assert_eq!(
        scheduler.precondition_satisfied[1], false,
        "Second alternative should start locked"
    );

    // Alternative 3: no precondition
    assert!(
        scheduler.skill_logics[2].precondition.is_none(),
        "Third alternative should not have precondition"
    );
    assert_eq!(
        scheduler.precondition_satisfied[2], true,
        "Third alternative should start unlocked"
    );

    // Verify each alternative's precondition is independent
    // (This is implicit in the data structure, but we verify the initialization is correct)
    assert_eq!(
        scheduler.precondition_satisfied,
        vec![false, false, true],
        "Precondition states should match alternative requirements"
    );
}
