use super::*;
use crate::race::runner::Runner;
use crate::race::solver::RaceSolver;
use crate::skill::Skill;
use crate::test_utils::{load_course, race_params, ready_solver, seeded_config, skill_data, uma};

/// Ready race with a single Late Surger with the given skills.
fn create_test_race_solver(skills: Vec<Skill>) -> RaceSolver<Runner> {
    ready_solver(10101, uma(Style::Late), skills, &seeded_config(12345))
}

#[test]
fn test_is_used_skill_id_with_activated_skill() {
    let mut solver = create_test_race_solver(vec![]);

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
    let state = create_test_race_solver(vec![]);

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
    let mut solver = create_test_race_solver(vec![]);

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
    let mut state = create_test_race_solver(vec![]);

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
    let skill = Skill::new("203061".to_string(), skill_data(vec![]));
    let solver = create_test_race_solver(vec![skill]);

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

    let mut solver = create_test_race_solver(vec![]);
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

/// Ready race with a team of two Late Surgers (team 0) and an individual Late
/// Surger (team 1).
fn create_team_race_solver() -> RaceSolver<Runner> {
    let mut solver = RaceSolver::new(load_course(10101), race_params(), &seeded_config(0));
    let member = || (uma(Style::Late), Motivation::Normal, vec![]);
    solver.add_team(vec![member(), member()]);
    solver.add_runner(uma(Style::Late), Motivation::Normal, vec![]);
    solver.ready_runners()
}

/// First runner of `team`.
fn team_runner(solver: &RaceSolver<Runner>, team: u8) -> &Runner {
    let mut runners = solver.runners.iter();
    runners.find(|r| r.initial_state.team == team).unwrap()
}

#[test]
fn test_style_temptation_opponent_count_excludes_own_team() {
    let mut solver = create_team_race_solver();
    // Both team members and the individual runner are rushed.
    solver.state.rushed_by_team[0][Style::Late as usize] = 2;
    solver.state.rushed_by_team[1][Style::Late as usize] = 1;

    let state = &solver.state;
    let all = TemptationVar::RunningStyleTemptationCountSashi;
    let opponents = TemptationVar::RunningStyleTemptationOpponentCountSashi;
    let other_style = TemptationVar::RunningStyleTemptationOpponentCountSenko;
    for (team, expected_opponents) in [(0, 1), (1, 2)] {
        let runner = team_runner(&solver, team);
        assert_eq!(get_temptation_value(&all, state, runner), 3, "team {team}");
        assert_eq!(
            get_temptation_value(&opponents, state, runner),
            expected_opponents,
            "team {team}"
        );
        assert_eq!(
            get_temptation_value(&other_style, state, runner),
            0,
            "team {team}"
        );
    }
}

#[test]
fn test_style_temptation_opponent_count_excludes_self() {
    let mut solver = create_team_race_solver();
    // Only the individual runner is rushed: it has no rushed opponents.
    solver.state.rushed_by_team[1][Style::Late as usize] = 1;

    let var = TemptationVar::RunningStyleTemptationOpponentCountSashi;
    let individual = team_runner(&solver, 1);
    assert_eq!(get_temptation_value(&var, &solver.state, individual), 0);
    let member = team_runner(&solver, 0);
    assert_eq!(get_temptation_value(&var, &solver.state, member), 1);
}

#[test]
fn test_temptation_opponent_count_by_position() {
    let mut solver = create_test_race_solver(vec![]);
    let counter = &mut solver.runners[0].rush_counter;
    counter.ahead = 3;
    counter.behind = 4;
    counter.opponents_ahead = 1;
    counter.opponents_behind = 2;

    let state = &solver.state;
    let runner = &solver.runners[0];
    let actual = [
        TemptationVar::TemptationCountInfront,
        TemptationVar::TemptationCountBehind,
        TemptationVar::TemptationOpponentCountInfront,
        TemptationVar::TemptationOpponentCountBehind,
    ]
    .map(|var| get_temptation_value(&var, state, runner));
    assert_eq!(actual, [3, 4, 1, 2]);
}

#[test]
fn test_runaways_count_as_front_runners() {
    let mut solver = RaceSolver::new(load_course(10101), race_params(), &seeded_config(0));
    for style in [Style::Runaway, Style::Front, Style::Pace] {
        solver.add_runner(uma(style), Motivation::Normal, vec![]);
    }
    let solver = solver.ready_runners();
    let state = &solver.state;

    let value = |var, style| get_running_style_value(&var, state, style);
    let nige_otherself = RunningStyleVar::RunningStyleCountNigeOtherself;
    assert_eq!(value(nige_otherself, Style::Runaway), 1);
    assert_eq!(value(nige_otherself, Style::Front), 1);
    assert_eq!(value(nige_otherself, Style::Pace), 2);
    assert_eq!(
        value(RunningStyleVar::RunningStyleCountSame, Style::Runaway),
        2
    );
}

#[test]
fn test_most_popular_runaway_counts_as_front_runner() {
    let solver = ready_solver(10101, uma(Style::Runaway), vec![], &seeded_config(0));
    let runner = &solver.runners[0];

    let var = RunningStyleVar::RunningStyleEqualPopularityOne;
    assert_eq!(
        get_running_style_value(&var, &solver.state, runner.adjusted.style),
        1
    );
}

#[test]
fn test_activate_count_all_team_reads_own_team() {
    let mut solver = create_team_race_solver();
    solver.state.team_activations = vec![7, 2];

    let var = ActivationVar::ActivateCountAllTeam;
    for (team, expected) in [(0, 7), (1, 2)] {
        let runner = team_runner(&solver, team);
        let actual = get_activation_value(&var, &solver.state, runner, None);
        assert_eq!(actual, expected, "team {team}");
    }
}
