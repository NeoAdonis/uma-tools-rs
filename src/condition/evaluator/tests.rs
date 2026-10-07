use super::*;
use crate::course::*;
use crate::race::parameters::*;
use crate::race::runner::Runner;
use crate::race::solver::RaceSolver;
use crate::skill::*;
use crate::test_utils::seeded_config;

/// Helper to create a minimal test race state.
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
        track_condition: TrackCondition::Good,
        weather: Weather::Sunny,
        season: Season::Spring,
        grade: Grade::G1,
        time: Time::Noon,
        gates: 18,
    };

    let skills = vec![];

    let mut solver = RaceSolver::new(course, params, &seeded_config(12345));
    solver.add_runner(uma, Motivation::Good, skills);
    solver.ready_runners()
}

#[test]
fn test_is_used_skill_id_with_activated_skill() {
    let mut solver = create_test_race_solver();

    // Add skill ID "202051" to activated_skill_ids.
    solver.runners[0]
        .activated_skill_ids
        .insert("202051".to_string());

    // Create comparison: is_used_skill_id == 202051.
    let comp = Comparison {
        var: VarType::Activation(ActivationVar::IsUsedSkillId),
        op: CompOp::Equal,
        value: 202051,
    };

    // Should return true because skill 202051 is activated.
    assert!(EvaluatorFull.evaluate(&comp, None, &solver.state, &solver.runners[0]));
}

#[test]
fn test_is_used_skill_id_without_activated_skill() {
    let state = create_test_race_solver();

    // No skills in activated_skills.

    // Create comparison: is_used_skill_id == 202051.
    let comp = Comparison {
        var: VarType::Activation(ActivationVar::IsUsedSkillId),
        op: CompOp::Equal,
        value: 202051,
    };

    // Should return false because skill 202051 is not activated.
    assert!(!EvaluatorFull.evaluate(&comp, None, &state.state, &state.runners[0]));
}

#[test]
fn test_is_used_skill_id_with_different_skill() {
    let mut solver = create_test_race_solver();

    // Add skill ID "203781" to activated_skill_ids (different from what we're
    // checking).
    solver.runners[0]
        .activated_skill_ids
        .insert("203781".to_string());

    // Create comparison: is_used_skill_id == 202051.
    let comp = Comparison {
        var: VarType::Activation(ActivationVar::IsUsedSkillId),
        op: CompOp::Equal,
        value: 202051,
    };

    // Should return false because skill 202051 is not activated (only 203781 is).
    assert!(!EvaluatorFull.evaluate(&comp, None, &solver.state, &solver.runners[0]));
}

#[test]
fn test_is_used_skill_id_not_equal() {
    let mut state = create_test_race_solver();

    // Add skill ID "202051" to activated_skill_ids.
    state.runners[0]
        .activated_skill_ids
        .insert("202051".to_string());

    // Create comparison: is_used_skill_id != 202051.
    let comp = Comparison {
        var: VarType::Activation(ActivationVar::IsUsedSkillId),
        op: CompOp::NotEqual,
        value: 202051,
    };

    // Should return false because skill 202051 IS activated.
    assert!(!EvaluatorFull.evaluate(&comp, None, &state.state, &state.runners[0]));
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
        track_condition: TrackCondition::Good,
        weather: Weather::Sunny,
        season: Season::Spring,
        grade: Grade::G1,
        time: Time::Noon,
        gates: 18,
    };

    // Create a skill with ID "203061".
    let skill_data = SkillData {
        rarity: SkillRarity::White,
        alternatives: vec![],
        popularity: None,
        wisdom_check: true,
    };
    let skill = Skill::new("203061".to_string(), skill_data);

    let skills = vec![skill];

    let mut solver = RaceSolver::new(course, params, &seeded_config(12345));
    solver.add_runner(uma, Motivation::Good, skills);
    let solver = solver.ready_runners();

    // Create comparison: is_exist_skill_id == 203061.
    let comp = Comparison {
        var: VarType::Activation(ActivationVar::IsExistSkillId),
        op: CompOp::Equal,
        value: 203061,
    };

    // Should return true because skill 203061 is in available_skills.
    assert!(EvaluatorFull.evaluate(&comp, None, &solver.state, &solver.runners[0]));
}

#[test]
fn test_phase_values_by_position() {
    use crate::course::Phase as CoursePhase;

    let mut solver = create_test_race_solver();
    let section = solver.state.course.section_length();

    // (position in sections, phase, phase_firstquarter, phase_firsthalf,
    // phase_laterhalf).
    let cases = [
        // Early: sections 0-4 (quarter 0-1, halves 0-2 and 2-4).
        (0.0, 0, 0, 0, -1),
        (0.5, 0, 0, 0, -1),
        (1.0, 0, -1, 0, -1),
        (2.0, 0, -1, -1, 0),
        (3.9, 0, -1, -1, 0),
        // Mid: sections 4-16 (quarter 4-7, halves 4-10 and 10-16).
        (4.0, 1, 1, 1, -1),
        (6.9, 1, 1, 1, -1),
        (7.0, 1, -1, 1, -1),
        (10.0, 1, -1, -1, 1),
        (15.9, 1, -1, -1, 1),
        // Late: sections 16-20 (quarter 16-17, halves 16-18 and 18-20).
        (16.0, 2, 2, 2, -1),
        (17.0, 2, -1, 2, -1),
        (18.0, 2, -1, -1, 2),
        (19.9, 2, -1, -1, 2),
        // Last: sections 20-24 (quarter 20-21, halves 20-22 and 22-24).
        (20.0, 3, 3, 3, -1),
        (21.0, 3, -1, 3, -1),
        (22.0, 3, -1, -1, 3),
        (23.9, 3, -1, -1, 3),
    ];

    for (sections, phase, first_quarter, first_half, later_half) in cases {
        let position = sections * section;
        solver.runners[0].position = position;
        solver.runners[0].phase = CoursePhase::from_position(position, &solver.state.course);

        let course = &solver.state.course;
        let runner = &solver.runners[0];
        let actual = [
            PhaseVar::Phase,
            PhaseVar::PhaseFirstquarter,
            PhaseVar::PhaseFirsthalf,
            PhaseVar::PhaseLaterhalf,
        ]
        .map(|var| get_phase_value(&var, course, runner));

        assert_eq!(
            actual,
            [phase, first_quarter, first_half, later_half],
            "position {sections} sections"
        );
    }
}
