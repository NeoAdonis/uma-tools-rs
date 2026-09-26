//! Tests for compiling conditions into triggers and guards

use super::*;
use crate::course::geometry::*;
use crate::parser::{CompOp, parse_condition};
use crate::skill::*;
use crate::trigger::builder::build_trigger_scheduler;

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

/// Compile a condition on the test course, with a single random point at 1000m.
fn compile(condition: &str) -> Vec<CompiledBranch> {
    compile_condition(
        &parse_condition(condition).unwrap(),
        &[1000.0],
        &create_test_course(),
    )
}

/// Compile a single-branch condition.
fn compile_one(condition: &str) -> CompiledBranch {
    let mut branches = compile(condition);
    assert_eq!(branches.len(), 1, "{condition}");
    branches.remove(0)
}

/// Get the variable and `(min, max)` thresholds of a window trigger.
fn window(branch: &CompiledBranch) -> (WindowVariable, f64, f64) {
    match branch.trigger {
        TriggerCondition::Window {
            variable,
            min_threshold,
            max_threshold,
        } => (variable, min_threshold, max_threshold),
        ref other => panic!("Expected window, got {:?}", other),
    }
}

/// Guards as strings, for readable assertions.
fn guards(branch: &CompiledBranch) -> Vec<String> {
    branch.guards.iter().map(|comp| comp.to_string()).collect()
}

/// Assert that two floats are (almost) equal.
fn assert_close(actual: f64, expected: f64, context: &str) {
    assert!(
        actual == expected || (actual - expected).abs() < 1e-9,
        "{context}: expected {expected}, got {actual}"
    );
}

#[test]
fn test_every_comparison_is_a_guard() {
    // Including the ones that define the trigger
    let branch = compile_one("phase==2&is_finalcorner==1&corner!=0&order<=3&ground_type==1");
    assert_eq!(
        guards(&branch),
        vec![
            "phase == 2",
            "is_finalcorner == 1",
            "corner != 0",
            "order <= 3",
            "ground_type == 1"
        ]
    );
}

#[test]
fn test_unevaluable_comparisons_are_not_guards() {
    // Random variables and is_last_straight_onetime can't be evaluated
    assert_eq!(
        guards(&compile_one("phase_random==1&order<=3")),
        vec!["order <= 3"]
    );
    assert_eq!(
        guards(&compile_one("run_at_full_speed_random==1&order<=3")),
        vec!["order <= 3"]
    );
    assert_eq!(
        guards(&compile_one("is_last_straight_onetime==1&order<=3")),
        vec!["order <= 3"]
    );
    // random_lot is rolled when building: if compiled, it succeeded
    assert!(compile_one("random_lot==50").guards.is_empty());
}

#[test]
fn test_events_are_the_trigger() {
    let cases = [
        ("phase_random==1&order<=3", "Position { meters: 1000.0 }"),
        ("run_at_full_speed_random==1", "FullThrottleStart"),
        ("is_last_straight_onetime==1", "LastStraightEntry"),
        ("is_activate_any_skill==1&phase>=2", "AnySkillActivation"),
        ("is_activate_heal_skill==1&phase>=2", "HealSkillActivation"),
    ];
    for (condition, trigger) in cases {
        assert_eq!(format!("{:?}", compile_one(condition).trigger), trigger);
    }
}

#[test]
fn test_activation_events_stay_as_guards() {
    // Evaluable: true on frames where a skill activated
    assert_eq!(
        guards(&compile_one("is_activate_any_skill==1&phase>=2")),
        vec!["is_activate_any_skill == 1", "phase >= 2"]
    );
}

#[test]
fn test_position_window_ranges() {
    // Test course: 2000m, sections of 83.33m, corners at 100-250, 500-700 and
    // 1200-1380, uphill at 800-900, straights at 0-100 and 1500-2000 (both facing
    // the audience). Values are floored when evaluated, so `<=N` holds until N + 1
    let section = 2000.0 / 24.0;
    let furlong = crate::units::FURLONG_TO_METERS;
    let cases = [
        ("distance_rate>=50", (1000.0, f64::INFINITY)),
        ("distance_rate<=50", (0.0, 1020.0)),
        ("distance_rate==50", (1000.0, 1020.0)),
        ("distance_rate>=50&distance_rate<=51", (1000.0, 1040.0)),
        ("remain_distance<=600", (1399.0, f64::INFINITY)),
        ("remain_distance>=600", (0.0, 1400.0)),
        (
            "remain_distance<=305&remain_distance>=300",
            (1694.0, 1700.0),
        ),
        ("furlong>=5", (5.0 * furlong, f64::INFINITY)),
        ("phase==1", (section * 4.0, section * 16.0)),
        ("phase>=2", (section * 16.0, 2000.0)),
        ("phase<=1", (0.0, section * 16.0)),
        ("phase_firstquarter==1", (section * 4.0, section * 7.0)),
        ("phase_firsthalf==1", (section * 4.0, section * 10.0)),
        ("phase_laterhalf==1", (section * 10.0, section * 16.0)),
        // Final corner "or beyond" (and its second half)
        ("is_finalcorner==1", (1200.0, f64::INFINITY)),
        ("is_finalcorner_laterhalf==1", (1290.0, f64::INFINITY)),
        // After the final corner
        ("is_last_straight==1", (1380.0, f64::INFINITY)),
        // Smallest range containing all corners/slopes/straights of that type
        ("corner!=0", (100.0, 1380.0)),
        ("corner==2", (500.0, 700.0)),
        ("slope==1", (800.0, 900.0)),
        ("straight_front_type==1", (0.0, 2000.0)),
        // Intersections
        ("is_finalcorner==1&corner!=0", (1200.0, 1380.0)),
        ("phase==1&corner!=0", (section * 4.0, section * 16.0)),
        ("phase>=1&slope==1&order_rate>=40", (800.0, 900.0)),
        // Nothing position-based: window covers the whole race
        ("order<=3", (0.0, f64::INFINITY)),
        ("distance_type==3", (0.0, f64::INFINITY)),
    ];

    for (condition, (min, max)) in cases {
        let (variable, actual_min, actual_max) = window(&compile_one(condition));
        assert_eq!(variable, WindowVariable::Position, "{condition}");
        assert_close(actual_min, min, condition);
        assert_close(actual_max, max, condition);
    }
}

#[test]
fn test_time_and_hp_windows() {
    // Used when the branch doesn't depend on the position
    let cases = [
        (
            "accumulatetime>=5",
            WindowVariable::Time,
            (5.0, f64::INFINITY),
        ),
        (
            "accumulatetime<=10&order<=3",
            WindowVariable::Time,
            (0.0, 11.0),
        ),
        ("hp_per<=30", WindowVariable::HpPercent, (0.0, 31.0)),
        (
            "hp_per>=50&order<=3",
            WindowVariable::HpPercent,
            (50.0, f64::INFINITY),
        ),
    ];
    for (condition, expected_variable, (min, max)) in cases {
        let (variable, actual_min, actual_max) = window(&compile_one(condition));
        assert_eq!(variable, expected_variable, "{condition}");
        assert_close(actual_min, min, condition);
        assert_close(actual_max, max, condition);
    }

    // Position windows take precedence; time and HP stay as guards
    let branch = compile_one("distance_rate>=50&hp_per<=30");
    assert_eq!(window(&branch).0, WindowVariable::Position);
    assert_eq!(guards(&branch), vec!["distance_rate >= 50", "hp_per <= 30"]);
}

#[test]
fn test_unsatisfiable_branches_are_dropped() {
    // The test course has 3 corners and no downhill slopes
    for condition in [
        "slope==2",
        "corner==4",
        "phase==0&distance_rate>=50",
        "hp_per>=50&hp_per<=30",
        "slope==2&is_activate_heal_skill==1",
    ] {
        assert!(compile(condition).is_empty(), "{condition}");
    }

    // Only the satisfiable OR branch is kept
    let branches = compile("slope==2&order<=3@slope==1&order<=3");
    assert_eq!(branches.len(), 1);
    assert_eq!(guards(&branches[0]), vec!["slope == 1", "order <= 3"]);
}

#[test]
fn test_phase_equality_is_a_window() {
    let branch = compile_one("phase==1&distance_rate>=60");
    let (_, min, max) = window(&branch);
    assert_close(min, 1200.0, "min");
    assert_close(max, 2000.0 / 24.0 * 16.0, "max");
}

#[test]
fn test_spatial_comparisons_are_kept() {
    let branch = compile_one("slope==1&order<=3");
    assert_eq!(guards(&branch), vec!["slope == 1", "order <= 3"]);
    let (_, min, max) = window(&branch);
    assert_eq!((min, max), (800.0, 900.0));
}

#[test]
fn test_multiple_trigger_comparisons_are_combined() {
    let branch = compile_one("phase>=2&is_finalcorner==1&corner!=0&is_activate_any_skill==1");
    assert!(matches!(
        branch.trigger,
        TriggerCondition::AnySkillActivation
    ));
    assert_eq!(branch.guards.len(), 4);
}

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
    let windows: Vec<_> = scheduler.position_windows.iter().collect();
    assert_eq!(windows.len(), 1);
    assert_eq!(windows[0].logic_idx, 1);
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
        assert!(scheduler.position_windows.is_empty());
    }
}

#[test]
fn test_precondition_time_and_hp_windows_cover_the_race() {
    // The precondition queue only tracks positions
    let skill = create_skill_with_precondition("test", "distance_rate>=80", "accumulatetime>=10");
    let scheduler = build_test_scheduler(&[skill]);

    let windows: Vec<_> = scheduler.precondition_windows.iter().collect();
    assert_eq!(windows.len(), 1);
    assert_eq!(
        (windows[0].min_threshold, windows[0].max_threshold),
        (0.0, f64::INFINITY)
    );
    assert!(scheduler.time_windows.is_empty());
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
    assert_eq!(scheduler.last_straight_triggers, vec![0, 1]);
    assert_eq!(scheduler.skill_logics[0].action, LogicAction::Unlock(0));
}
