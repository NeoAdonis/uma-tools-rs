//! Tests for compiling conditions into triggers and guards.

use super::*;
use crate::condition::parser::{CompOp, Condition};
use crate::course::geometry::*;
use crate::race::rng::RngSeed;
use crate::skill::*;
use crate::test_utils::{alternative, assert_close, synthetic_course, synthetic_skill};
use crate::trigger::system::TriggerScheduler;
use std::str::FromStr;
use std::sync::Arc;

/// Create a simple test course for testing spatial triggers.
fn create_test_course() -> Course {
    Course {
        id: 10001,
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
        slopes: vec![Slope::new(800.0, 100.0, 0.05)],
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
        ..synthetic_course()
    }
}

/// Compile a condition on the test course, with a single random point at
/// 1000m for every branch.
fn compile(condition: &str) -> Vec<CompiledBranch> {
    let condition = Condition::from_str(condition).unwrap();
    let rolls = vec![Some(vec![1000.0]); condition.branches.len()];
    compile_condition(&condition, &rolls, &create_test_course())
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

#[test]
fn test_every_comparison_is_a_guard() {
    // Including the ones that define the trigger, and the evaluable activation
    // events (true on frames where a skill activated).
    let branch = compile_one(
        "phase==2&is_finalcorner==1&corner!=0&order<=3&ground_type==1&is_activate_any_skill==1",
    );
    assert_eq!(
        guards(&branch),
        vec![
            "phase == 2",
            "is_finalcorner == 1",
            "corner != 0",
            "order <= 3",
            "ground_type == 1",
            "is_activate_any_skill == 1"
        ]
    );
}

#[test]
fn test_unevaluable_comparisons_are_not_guards() {
    // Random variables and is_last_straight_onetime can't be evaluated.
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
    // `random_lot` is rolled when building: if compiled, it succeeded.
    assert!(compile_one("random_lot==50").guards.is_empty());
}

#[test]
fn test_events_are_the_trigger() {
    let cases = [
        (
            "phase_random==1&order<=3",
            "Segments { ranges: [(1000.0, 1010.0)] }",
        ),
        ("run_at_full_speed_random==1", "AllOutSpurtStart"),
        ("is_last_straight_onetime==1", "LastStraightEntry"),
        ("is_activate_any_skill==1&phase>=2", "AnySkillActivation"),
        ("is_activate_heal_skill==1&phase>=2", "HealSkillActivation"),
        (
            "is_other_character_activate_advantage_skill==27",
            "BuffSkillActivation",
        ),
    ];
    for (condition, trigger) in cases {
        assert_eq!(format!("{:?}", compile_one(condition).trigger), trigger);
    }
}

#[test]
fn test_position_window_ranges() {
    // Test course: 2000m, sections of 83.33m, corners at 100-250, 500-700 and
    // 1200-1380, uphill at 800-900, straights at 0-100 and 1500-2000 (both facing
    // the audience). Values are floored when evaluated, so `<=N` holds until N + 1,
    // and `>N` from N + 1.
    let section = 2000.0 / 24.0;
    let furlong = crate::units::FURLONG_TO_METERS;
    let cases = [
        ("distance_rate>=50", (1000.0, f64::INFINITY)),
        ("distance_rate<=50", (0.0, 1020.0)),
        ("distance_rate==50", (1000.0, 1020.0)),
        ("distance_rate>=50&distance_rate<=51", (1000.0, 1040.0)),
        ("distance_rate>50", (1020.0, f64::INFINITY)),
        ("distance_rate<50", (0.0, 1000.0)),
        ("remain_distance<=600", (1399.0, f64::INFINITY)),
        ("remain_distance>=600", (0.0, 1400.0)),
        ("remain_distance<600", (1400.0, f64::INFINITY)),
        ("remain_distance>600", (0.0, 1399.0)),
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
        // Final corner "or beyond" (and its second half).
        ("is_finalcorner==1", (1200.0, f64::INFINITY)),
        ("is_finalcorner_laterhalf==1", (1290.0, f64::INFINITY)),
        // After the final corner.
        ("is_last_straight==1", (1380.0, f64::INFINITY)),
        // Smallest range containing all corners/slopes/straights of that type.
        ("corner!=0", (100.0, 1380.0)),
        ("corner==2", (100.0, 250.0)),
        ("corner==3", (500.0, 700.0)),
        ("slope==1", (800.0, 900.0)),
        ("straight_front_type==1", (0.0, 2000.0)),
        // Intersections.
        ("is_finalcorner==1&corner!=0", (1200.0, 1380.0)),
        ("phase==1&corner!=0", (section * 4.0, section * 16.0)),
        ("phase>=1&slope==1&order_rate>=40", (800.0, 900.0)),
        // Nothing position-based: window covers the whole race.
        ("order<=3", (0.0, f64::INFINITY)),
        ("distance_type==3", (0.0, f64::INFINITY)),
        ("distance_rate!=50", (0.0, f64::INFINITY)),
        // Not in a corner: its gaps aren't a range.
        ("corner==0", (0.0, f64::INFINITY)),
        // Evaluable activation comparisons that aren't events.
        ("is_activate_other_skill_detail==1", (0.0, f64::INFINITY)),
    ];

    for (condition, (min, max)) in cases {
        let (variable, actual_min, actual_max) = window(&compile_one(condition));
        assert_eq!(variable, WindowVariable::Position, "{condition}");
        assert_close!(actual_min, min, "{condition}");
        assert_close!(actual_max, max, "{condition}");
    }
}

#[test]
fn test_time_and_hp_windows() {
    // Used when the branch doesn't depend on the position.
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
        assert_close!(actual_min, min, "{condition}");
        assert_close!(actual_max, max, "{condition}");
    }

    // Position windows take precedence; time and HP stay as guards.
    let branch = compile_one("distance_rate>=50&hp_per<=30");
    assert_eq!(window(&branch).0, WindowVariable::Position);
    assert_eq!(guards(&branch), vec!["distance_rate >= 50", "hp_per <= 30"]);
}

#[test]
fn test_unsatisfiable_branches_are_dropped() {
    // The test course has no downhill slopes and 3 corners (the second to the
    // fourth).
    for condition in [
        "slope==2",
        "corner==1",
        "phase==0&distance_rate>=50",
        "hp_per>=50&hp_per<=30",
        "slope==2&is_activate_heal_skill==1",
    ] {
        assert!(compile(condition).is_empty(), "{condition}");
    }

    // Only the satisfiable OR branch is kept.
    let branches = compile("slope==2&order<=3@slope==1&order<=3");
    assert_eq!(branches.len(), 1);
    assert_eq!(guards(&branches[0]), vec!["slope == 1", "order <= 3"]);
}

/// Helper function to create a test skill with precondition.
fn create_skill_with_precondition(id: i32, condition: &str, precondition: &str) -> Arc<Skill> {
    let alternative = SkillAlternative {
        precondition: (!precondition.is_empty()).then(|| precondition.parse().unwrap()),
        ..alternative(condition, SkillType::TargetSpeed, 0.4)
    };
    Arc::new(Skill {
        id,
        ..synthetic_skill(vec![alternative])
    })
}

/// Build a scheduler for the given skills on the test course.
fn build_test_scheduler(skills: &[Arc<Skill>]) -> crate::trigger::system::TriggerScheduler {
    let rng_seed = RngSeed::new(0);
    TriggerScheduler::from_skills(skills, &create_test_course(), rng_seed)
}

#[test]
fn test_precondition_creates_unlock_logic_before_skill_logic() {
    let skill = create_skill_with_precondition(7, "distance_rate>=80", "phase>=2");
    let scheduler = build_test_scheduler(&[skill]);

    assert_eq!(scheduler.skill_logics.len(), 2);

    // Unlock logic comes first, keeps the comparison as guard.
    let unlock = &scheduler.skill_logics[0];
    assert_eq!(unlock.action, LogicAction::Unlock(0));
    assert!(unlock.precondition.is_none());
    assert_eq!(unlock.guards.len(), 1);
    assert_eq!(unlock.guards[0].op, CompOp::GreaterEqual);
    assert_eq!(unlock.guards[0].value, 2);

    // Skill logic waits for the precondition.
    let skill_logic = &scheduler.skill_logics[1];
    assert_eq!(skill_logic.action, LogicAction::Activate);
    assert_eq!(skill_logic.precondition, Some(0));

    // Starts locked; its window is queued separately from regular windows.
    assert_eq!(scheduler.precondition_satisfied, vec![false]);
    let windows: Vec<_> = scheduler.precondition_windows.iter().collect();
    assert_eq!(windows.len(), 1);
    assert_eq!(windows[0].logic_idx, 0);
    let windows: Vec<_> = scheduler.position_windows.iter().collect();
    assert_eq!(windows.len(), 1);
    assert_eq!(windows[0].logic_idx, 1);
}

#[test]
fn test_or_condition_branches_share_precondition() {
    let skill = create_skill_with_precondition(
        6,
        // OR condition.
        "distance_rate>=80@hp_per<=50",
        // Shared precondition.
        "phase>=2",
    );
    let scheduler = build_test_scheduler(&[skill]);

    // 1 unlock logic + 2 condition branches, logics of the same alternative
    // tied to the same precondition.
    assert_eq!(scheduler.skill_logics.len(), 3);
    assert_eq!(scheduler.skill_logics[0].action, LogicAction::Unlock(0));
    for logic in &scheduler.skill_logics[1..] {
        let indices = (logic.skill_index, logic.alternative_index);
        assert_eq!((indices, logic.precondition), ((0, 0), Some(0)));
    }
    assert_eq!(scheduler.precondition_satisfied.len(), 1);
}

#[test]
fn test_or_precondition_creates_one_unlock_logic_per_branch() {
    let skill = create_skill_with_precondition(6, "distance_rate>=80", "phase==1@phase==3");
    let scheduler = build_test_scheduler(&[skill]);

    assert_eq!(scheduler.skill_logics.len(), 3);
    assert_eq!(scheduler.skill_logics[0].action, LogicAction::Unlock(0));
    assert_eq!(scheduler.skill_logics[1].action, LogicAction::Unlock(0));
    assert_eq!(scheduler.skill_logics[2].precondition, Some(0));
    assert_eq!(scheduler.precondition_windows.len(), 2);
}

#[test]
fn test_precondition_ids_are_assigned_per_alternative() {
    // Only the first alternative of the first skill has a precondition.
    let first = Arc::new(synthetic_skill(vec![
        SkillAlternative {
            precondition: Some("phase>=2".parse().unwrap()),
            ..alternative("distance_rate>=80", SkillType::TargetSpeed, 0.4)
        },
        alternative("hp_per<=50", SkillType::TargetSpeed, 0.3),
    ]));
    let second = create_skill_with_precondition(3, "distance_rate>=50", "order<=3");
    let scheduler = build_test_scheduler(&[first, second]);

    assert_eq!(scheduler.precondition_satisfied, vec![false, false]);

    let preconditions: Vec<_> = scheduler
        .skill_logics
        .iter()
        .filter(|logic| logic.action == LogicAction::Activate)
        .map(|logic| {
            (
                logic.skill_index,
                logic.alternative_index,
                logic.precondition,
            )
        })
        .collect();
    assert_eq!(
        preconditions,
        vec![(0, 0, Some(0)), (0, 1, None), (1, 0, Some(1))]
    );
}

#[test]
fn test_unsatisfiable_precondition_skips_alternative() {
    // The test course has no downhill slopes.
    let skill = create_skill_with_precondition(4, "distance_rate>=80", "slope==2");
    let scheduler = build_test_scheduler(&[skill]);

    assert!(scheduler.skill_logics.is_empty());
    assert!(scheduler.position_windows.is_empty());
}

#[test]
fn test_unsatisfiable_condition_skips_precondition() {
    // The test course has no downhill slopes, so neither the skill nor its
    // precondition is scheduled.
    let skill = create_skill_with_precondition(4, "slope==2", "phase>=2");
    let scheduler = build_test_scheduler(&[skill]);

    assert!(scheduler.skill_logics.is_empty());
    assert!(scheduler.precondition_satisfied.is_empty());
}

#[test]
fn test_precondition_time_and_hp_windows_cover_the_race() {
    // The precondition queue only tracks positions.
    let skill = create_skill_with_precondition(4, "distance_rate>=80", "accumulatetime>=10");
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
        8,
        "is_last_straight_onetime==1",
        "phase>=2&is_last_straight_onetime==1",
    );
    let scheduler = build_test_scheduler(&[skill]);

    // Both logics fire on the last straight; the unlock logic must go first.
    assert!(scheduler.precondition_windows.is_empty());
    assert_eq!(scheduler.last_straight_triggers, vec![0, 1]);
    assert_eq!(scheduler.skill_logics[0].action, LogicAction::Unlock(0));
}

#[test]
fn test_branches_use_their_own_rolls() {
    let condition = Condition::from_str(
        "distance_type==1&all_corner_random==1@distance_type==2&all_corner_random==1",
    )
    .unwrap();
    let course = create_test_course();

    let rolls = [Some(vec![100.0, 200.0]), Some(vec![500.0])];
    let triggers: Vec<String> = compile_condition(&condition, &rolls, &course)
        .iter()
        .map(|branch| format!("{:?}", branch.trigger))
        .collect();
    assert_eq!(
        triggers,
        [
            "Segments { ranges: [(100.0, 110.0), (200.0, 210.0)] }",
            "Segments { ranges: [(500.0, 510.0)] }"
        ]
    );

    // Branches whose roll failed are dropped.
    let rolls = [None, Some(vec![500.0])];
    let branches = compile_condition(&condition, &rolls, &course);
    assert_eq!(branches.len(), 1);
    assert_eq!(guards(&branches[0]), vec!["distance_type == 2"]);
}

#[test]
fn test_segments_are_trimmed_to_the_branch() {
    // Corner 3 of the test course is [500-700].
    let condition = Condition::from_str("all_corner_random==1&corner==3").unwrap();
    let course = create_test_course();

    // Segments outside the corner are dropped, and the ones crossing its edges
    // trimmed.
    let rolls = [Some(vec![95.0, 495.0, 600.0, 695.0, 900.0])];
    let branches = compile_condition(&condition, &rolls, &course);
    assert_eq!(branches.len(), 1);
    let TriggerCondition::Segments { ranges } = &branches[0].trigger else {
        panic!("expected segments, got {:?}", branches[0].trigger);
    };
    assert_eq!(ranges, &[(500.0, 505.0), (600.0, 610.0), (695.0, 700.0)]);

    // No segment on the corner: the branch can never be satisfied.
    let rolls = [Some(vec![95.0, 900.0])];
    assert!(compile_condition(&condition, &rolls, &course).is_empty());
}

#[test]
fn test_alternatives_share_random_rolls() {
    // The first alternative only adds a requirement to the second one, so both
    // must be checked at the same random point.
    let skill = Arc::new(synthetic_skill(vec![
        alternative("phase_random==1&order<=3", SkillType::TargetSpeed, 0.35),
        alternative("phase_random==1", SkillType::TargetSpeed, 0.15),
    ]));
    let scheduler = build_test_scheduler(&[skill]);

    let logics = &scheduler.skill_logics;
    assert_eq!(logics.len(), 2);
    assert!(matches!(
        logics[0].trigger,
        TriggerCondition::Segments { .. }
    ));
    assert_eq!(logics[0].trigger, logics[1].trigger);
}
