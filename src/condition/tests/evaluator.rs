use super::super::evaluator::*;
use super::super::parser::*;
use super::super::variables::*;
use crate::course::*;
use crate::race::parameters::*;
use crate::race::runner::Runner;
use crate::race::solver::RaceSolver;
use crate::uma::skill::*;
use crate::uma::*;

/// Helper to create a minimal test race state
fn create_test_race_solver() -> RaceSolver<Runner> {
    let uma = Uma {
        speed: 1200,
        stamina: 1200,
        power: 1200,
        guts: 1200,
        wiz: 1200,
        style: Style::Late,
        distance_aptitude: Aptitude::S,
        track_aptitude: Aptitude::S,
        style_aptitude: Aptitude::S,
    };

    let course_data = CourseData {
        distance: 2000,
        distance_type: DistanceType::Mile,
        surface: Track::Turf,
        turn: Direction::Left,
        course_set_status: vec![],
        finish_time_min: 120000,
        finish_time_max: 150000,
        lane_max: 10000,
        corners: vec![],
        slopes: vec![],
        straights: vec![],
        race_track_id: 10006,
    };

    let course = Course::new_test(10006, course_data);

    let params = RaceParameters {
        condition: TrackCondition::Good,
        weather: Weather::Sunny,
        season: Season::Spring,
        grade: Grade::G1,
        time: Time::Noon,
        gates: 18,
        rng_seed: 12345,
    };

    let skills = vec![];

    let mut solver = RaceSolver::new(course, params);
    solver.add_runner(uma, Motivation::Good, skills);
    solver.ready_runners()
}

#[test]
fn test_is_used_skill_id_with_activated_skill() {
    let mut solver = create_test_race_solver();

    // Add skill ID "202051" to activated_skill_ids
    solver.runners[0]
        .activated_skill_ids
        .insert("202051".to_string());

    // Create comparison: is_used_skill_id == 202051
    let comp = Comp {
        var: VarType::Activation(ActivationVar::IsUsedSkillId),
        op: CompOp::Equal,
        value: 202051,
    };

    // Should return true because skill 202051 is activated
    assert!(EvaluatorFull.evaluate(&comp, &solver.state, &solver.runners[0]));
}

#[test]
fn test_is_used_skill_id_without_activated_skill() {
    let state = create_test_race_solver();

    // No skills in activated_skills

    // Create comparison: is_used_skill_id == 202051
    let comp = Comp {
        var: VarType::Activation(ActivationVar::IsUsedSkillId),
        op: CompOp::Equal,
        value: 202051,
    };

    // Should return false because skill 202051 is not activated
    assert!(!EvaluatorFull.evaluate(&comp, &state.state, &state.runners[0]));
}

#[test]
fn test_is_used_skill_id_with_different_skill() {
    let mut solver = create_test_race_solver();

    // Add skill ID "203781" to activated_skill_ids (different from what we're checking)
    solver.runners[0]
        .activated_skill_ids
        .insert("203781".to_string());

    // Create comparison: is_used_skill_id == 202051
    let comp = Comp {
        var: VarType::Activation(ActivationVar::IsUsedSkillId),
        op: CompOp::Equal,
        value: 202051,
    };

    // Should return false because skill 202051 is not activated (only 203781 is)
    assert!(!EvaluatorFull.evaluate(&comp, &solver.state, &solver.runners[0]));
}

#[test]
fn test_is_used_skill_id_not_equal() {
    let mut state = create_test_race_solver();

    // Add skill ID "202051" to activated_skill_ids
    state.runners[0]
        .activated_skill_ids
        .insert("202051".to_string());

    // Create comparison: is_used_skill_id != 202051
    let comp = Comp {
        var: VarType::Activation(ActivationVar::IsUsedSkillId),
        op: CompOp::NotEqual,
        value: 202051,
    };

    // Should return false because skill 202051 IS activated
    assert!(!EvaluatorFull.evaluate(&comp, &state.state, &state.runners[0]));
}

#[test]
fn test_is_exist_skill_id_in_available_skills() {
    let uma = Uma {
        speed: 1200,
        stamina: 1200,
        power: 1200,
        guts: 1200,
        wiz: 1200,
        style: Style::Late,
        distance_aptitude: Aptitude::S,
        track_aptitude: Aptitude::S,
        style_aptitude: Aptitude::S,
    };

    let course_data = CourseData {
        distance: 2000,
        distance_type: DistanceType::Mile,
        surface: Track::Turf,
        turn: Direction::Left,
        course_set_status: vec![],
        finish_time_min: 120000,
        finish_time_max: 150000,
        lane_max: 10000,
        corners: vec![],
        slopes: vec![],
        straights: vec![],
        race_track_id: 10006,
    };

    let course = Course::new_test(10006, course_data);

    let params = RaceParameters {
        condition: TrackCondition::Good,
        weather: Weather::Sunny,
        season: Season::Spring,
        grade: Grade::G1,
        time: Time::Noon,
        gates: 18,
        rng_seed: 12345,
    };

    // Create a skill with ID "203061"
    let skill_data = SkillData {
        rarity: SkillRarity::White,
        alternatives: vec![],
    };
    let skill = Skill::new("203061".to_string(), skill_data);

    let skills = vec![skill];

    let mut solver = RaceSolver::new(course, params);
    solver.add_runner(uma, Motivation::Good, skills);
    let solver = solver.ready_runners();

    // Create comparison: is_exist_skill_id == 203061
    let comp = Comp {
        var: VarType::Activation(ActivationVar::IsExistSkillId),
        op: CompOp::Equal,
        value: 203061,
    };

    // Should return true because skill 203061 is in available_skills
    assert!(EvaluatorFull.evaluate(&comp, &solver.state, &solver.runners[0]));
}
