//! Runtime tests of preconditions: skills unlocked during a race once their
//! precondition is satisfied, and static preconditions resolved before it.

use crate::race::runner::Runner;
use crate::race::solver::RaceSolver;
use crate::skill::*;
use crate::test_utils::{
    alternative_with_precondition, load_course, ready_solver, seeded_config, skill,
    synthetic_skill, uma,
};
use crate::uma::*;
use std::sync::Arc;

/// 2000m medium turf course with 4 corners and no slopes.
const RUNTIME_COURSE: u32 = 10104;

/// Always available target speed skill, locked until `precondition` is
/// satisfied.
fn speed_skill(id: i32, condition: &str, precondition: &str) -> Arc<Skill> {
    let alternative =
        alternative_with_precondition(precondition, condition, SkillType::TargetSpeed, 0.035);
    Arc::new(Skill {
        id,
        ..synthetic_skill(vec![alternative])
    })
}

/// Ready race with a single runner with the given skills.
fn ready_runner(skills: Vec<Arc<Skill>>) -> RaceSolver<Runner> {
    ready_solver(RUNTIME_COURSE, uma(Style::Front), skills, seeded_config(42))
}

/// Run a race with a single runner and return where each skill activated.
fn activation_distances(skills: Vec<Arc<Skill>>) -> Vec<(i32, f64)> {
    let mut solver = ready_runner(skills);
    solver.run();
    solver.runners[0]
        .activated_skills
        .iter()
        .map(|record| (record.skill_id, record.distance))
        .collect()
}

/// Where a skill activated, if it did.
fn activation_distance(activations: &[(i32, f64)], id: i32) -> Option<f64> {
    activations
        .iter()
        .find(|&&(skill_id, _)| skill_id == id)
        .map(|(_, distance)| *distance)
}

#[test]
fn test_precondition_unlocks_skill_once_satisfied() {
    // The condition holds from 10% on, but the skill must wait for Late phase
    let activations = activation_distances(vec![speed_skill(
        4,
        "distance_rate>=10&order<=18",
        "phase>=2",
    )]);

    let late_start = load_course(RUNTIME_COURSE).section_length() * 16.0;
    let distance = activation_distance(&activations, 4).expect("Skill should activate");
    assert!(
        (late_start..late_start + 30.0).contains(&distance),
        "Should activate right after Late phase starts ({late_start}m), got {distance}m"
    );
}

#[test]
fn test_heal_precondition_unlocks_on_heal_activation() {
    // Skill 110771 (Joy to the World) waits for a heal in the first half of
    // Mid phase (17%-42% of the race)
    let precondition = "phase_firsthalf==1&is_activate_heal_skill==1";
    let joy = || speed_skill(3, "distance_rate>=45&order<=18", precondition);
    let heal = skill(2, "distance_rate>=20", SkillType::Recovery, 0.035);

    let activations = activation_distances(vec![heal, joy()]);
    assert!(activation_distance(&activations, 2).is_some());
    let distance = activation_distance(&activations, 3).expect("Heal should unlock skill");
    assert!(
        (900.0..930.0).contains(&distance),
        "Should activate at 45% (900m), got {distance}m"
    );

    // Without a heal, the skill stays locked
    let activations = activation_distances(vec![joy()]);
    assert_eq!(activation_distance(&activations, 3), None);
}

#[test]
fn test_last_straight_onetime_precondition() {
    // Skill 100911 has `is_last_straight_onetime==1` in its precondition, which
    // can only be checked when entering the last straight
    let activations = activation_distances(vec![speed_skill(
        8,
        "is_last_straight==1&order<=18",
        "phase>=2&is_last_straight_onetime==1",
    )]);

    let straight_start = load_course(RUNTIME_COURSE).straights.last().unwrap().start;
    let distance = activation_distance(&activations, 8).expect("Skill should activate");
    assert!(
        (straight_start..straight_start + 30.0).contains(&distance),
        "Should activate when entering the last straight ({straight_start}m), got {distance}m"
    );
}

#[test]
fn test_static_preconditions_resolved_at_race_start() {
    // Course 10104 is a medium distance (distance_type==3) turf course
    let solver = ready_runner(vec![
        speed_skill(5, "distance_rate>=10&order<=18", "distance_type==3"),
        speed_skill(7, "distance_rate>=10&order<=18", "distance_type==1"),
        speed_skill(6, "distance_rate>=10&order<=18", "ground_type==1&order<=18"),
    ]);
    let scheduler = &solver.runners[0].trigger_scheduler;

    // True static comparisons are removed; unsatisfiable windows are dropped
    let windows: Vec<_> = scheduler
        .precondition_windows
        .iter()
        .map(|window| &scheduler.skill_logics[window.logic_idx])
        .collect();
    assert_eq!(windows.len(), 2);
    assert_eq!(windows[0].skill_index, 0);
    assert!(windows[0].guards.is_empty());
    assert_eq!(windows[1].skill_index, 2);
    assert_eq!(windows[1].guards.len(), 1, "Only order<=18 should remain");

    let activations = activation_distances(vec![
        speed_skill(5, "distance_rate>=10&order<=18", "distance_type==3"),
        speed_skill(7, "distance_rate>=10&order<=18", "distance_type==1"),
    ]);
    assert!(activation_distance(&activations, 5).is_some());
    assert_eq!(activation_distance(&activations, 7), None);
}
