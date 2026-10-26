use super::*;
use crate::course::geometry::*;
use crate::course::{CourseData, Phase as CoursePhase};
use crate::race::all_out_spurt::AllOutSpurt;
use crate::race::duel::Duel;
use crate::race::last_spurt::{LastSpurt, LastSpurtCategory};
use crate::race::parameters::*;
use crate::race::runner::Runner;
use crate::race::rush::RushState;
use crate::race::solver::RaceSolver;
use crate::race::transitions::SegmentState;
use crate::skill::{Skill, SkillType};
use crate::test_utils::*;

/// Ready race with a single Late Surger with the given skills.
fn create_test_race_solver(skills: Vec<Skill>) -> RaceSolver<Runner> {
    ready_solver(10101, uma(Style::Late), skills, &seeded_config(12345))
}

/// Ready race with a single Late Surger holding a skill with the given ID,
/// activated through the alternatives in `mask`.
fn solver_with_activated_skill(skill_id: i32, mask: u8) -> RaceSolver<Runner> {
    let skill = Skill::new(skill_id, skill_data(vec![]));
    let mut solver = create_test_race_solver(vec![skill]);
    solver.runners[0].activated_alternatives[0] = mask;
    solver
}

#[test]
fn test_is_used_skill_id_with_activated_skill() {
    let solver = solver_with_activated_skill(202051, 1);

    // Create comparison: is_used_skill_id == 202051.
    let comp = Comparison {
        var: VarType::ActivationQuery(ActivationQueryVar::IsUsedSkillId),
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
        var: VarType::ActivationQuery(ActivationQueryVar::IsUsedSkillId),
        op: CompOp::Equal,
        value: 202051,
    };

    // Should return false because skill 202051 is not activated.
    assert!(!EvaluatorFull.evaluate(&comp, None, &state.state, &state.runners[0]));
}

#[test]
fn test_is_used_skill_id_with_different_skill() {
    // Different from what we're checking.
    let solver = solver_with_activated_skill(203781, 1);

    // Create comparison: is_used_skill_id == 202051.
    let comp = Comparison {
        var: VarType::ActivationQuery(ActivationQueryVar::IsUsedSkillId),
        op: CompOp::Equal,
        value: 202051,
    };

    // Should return false because skill 202051 is not activated (only 203781 is).
    assert!(!EvaluatorFull.evaluate(&comp, None, &solver.state, &solver.runners[0]));
}

#[test]
fn test_is_used_skill_id_not_equal() {
    let state = solver_with_activated_skill(202051, 1);

    // Create comparison: is_used_skill_id != 202051.
    let comp = Comparison {
        var: VarType::ActivationQuery(ActivationQueryVar::IsUsedSkillId),
        op: CompOp::NotEqual,
        value: 202051,
    };

    // Should return false because skill 202051 IS activated.
    assert!(!EvaluatorFull.evaluate(&comp, None, &state.state, &state.runners[0]));
}

/// Ready race with a skill with two alternatives (like "聖女の試練"), the given
/// one activated if any, and the results of `is_used_skill_id==204452` and
/// `is_used_skill_id_with_detail_one==204452`.
fn is_used_skill_id_with_alternative(activated: Option<usize>) -> (bool, bool) {
    let alternatives = vec![
        alternative("phase==1", SkillType::TargetSpeed, 0.35),
        alternative("phase==1", SkillType::CurrentSpeed, -0.15),
    ];
    let skill = Skill::new(204452, skill_data(alternatives));
    let mut solver = create_test_race_solver(vec![skill]);
    if let Some(alternative_index) = activated {
        solver.runners[0].activate_skill(0, alternative_index, &solver.state);
    }

    let evaluate = |var| {
        let comp = Comparison {
            var: VarType::ActivationQuery(var),
            op: CompOp::Equal,
            value: 204452,
        };
        EvaluatorFull.evaluate(&comp, None, &solver.state, &solver.runners[0])
    };
    (
        evaluate(ActivationQueryVar::IsUsedSkillId),
        evaluate(ActivationQueryVar::IsUsedSkillIdWithDetailOne),
    )
}

#[test]
fn test_is_used_skill_id_with_detail_one_first_alternative() {
    assert_eq!(is_used_skill_id_with_alternative(Some(0)), (true, true));
}

#[test]
fn test_is_used_skill_id_with_detail_one_second_alternative() {
    // The skill was used, but not through its first alternative.
    assert_eq!(is_used_skill_id_with_alternative(Some(1)), (true, false));
}

#[test]
fn test_is_used_skill_id_with_detail_one_not_activated() {
    assert_eq!(is_used_skill_id_with_alternative(None), (false, false));
}

#[test]
fn test_is_used_skill_id_with_detail_one_not_equal() {
    // Only the second alternative activated.
    let solver = solver_with_activated_skill(204452, 0b10);

    let comp = Comparison {
        var: VarType::ActivationQuery(ActivationQueryVar::IsUsedSkillIdWithDetailOne),
        op: CompOp::NotEqual,
        value: 204452,
    };
    assert!(EvaluatorFull.evaluate(&comp, None, &solver.state, &solver.runners[0]));
}

#[test]
fn test_is_exist_skill_id_in_available_skills() {
    let skill = Skill::new(203061, skill_data(vec![]));
    let solver = create_test_race_solver(vec![skill]);

    // Create comparison: is_exist_skill_id == 203061.
    let comp = Comparison {
        var: VarType::Lineup(LineupVar::IsExistSkillId),
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
    let member = || (uma(Style::Late), Mood::Normal, vec![]);
    solver.add_team(vec![member(), member()]);
    solver.add_runner(uma(Style::Late), Mood::Normal, vec![]);
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
        solver.add_runner(uma(style), Mood::Normal, vec![]);
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

/// Single comparison parsed from a condition string, e.g. `"hp_per>=50"`.
fn comp(condition: &str) -> Comparison {
    match condition.parse::<Condition>().unwrap().branches.as_slice() {
        [branch] => match branch.as_slice() {
            [comp] => comp.clone(),
            _ => panic!("expected a single comparison: {condition}"),
        },
        _ => panic!("expected a single comparison: {condition}"),
    }
}

#[test]
fn test_is_exist_skill_id_not_equal() {
    let solver = create_test_race_solver(vec![]);
    let (state, runner) = (&solver.state, &solver.runners[0]);

    // No runner has the skill.
    assert!(EvaluatorFull.evaluate(&comp("is_exist_skill_id!=203061"), None, state, runner));
    assert!(!EvaluatorFull.evaluate(&comp("is_exist_skill_id==203061"), None, state, runner));
}

#[test]
fn test_base_stat_values() {
    let mut solver = create_test_race_solver(vec![]);
    solver.runners[0].base = Uma {
        speed: 1101,
        stamina: 902,
        power: 803,
        guts: 704,
        wit: 605,
        ..uma(Style::Late)
    };

    let base = &solver.runners[0].base;
    let actual = [
        BaseStatVar::BaseSpeed,
        BaseStatVar::BaseStamina,
        BaseStatVar::BasePower,
        BaseStatVar::BaseGuts,
        BaseStatVar::BaseWiz,
    ]
    .map(|stat| get_base_stat(&stat, base));
    assert_eq!(actual, [1101, 902, 803, 704, 605]);
}

#[test]
fn test_entry_values() {
    let mut solver = create_test_race_solver(vec![]);
    let initial_state = &mut solver.runners[0].initial_state;
    initial_state.fan_count = 12345;
    initial_state.popularity_rank = 3;
    initial_state.gate_block = 5;

    let initial_state = &solver.runners[0].initial_state;
    let actual = [
        EntryVar::FanCount,
        EntryVar::Popularity,
        EntryVar::PostNumber,
    ]
    .map(|var| get_entry_value(&var, initial_state));
    assert_eq!(actual, [12345, 3, 5]);
}

#[test]
fn test_mood_values() {
    let mut solver = create_test_race_solver(vec![]);

    let cases = [
        (Mood::Awful, 1),
        (Mood::Bad, 2),
        (Mood::Normal, 3),
        (Mood::Good, 4),
        (Mood::Great, 5),
    ];
    for (mood, expected) in cases {
        solver.runners[0].initial_state.mood = mood;
        let actual = get_entry_value(&EntryVar::Motivation, &solver.runners[0].initial_state);
        assert_eq!(actual, expected, "{mood:?}");
    }
}

#[test]
fn test_race_param_values() {
    let params = RaceParameters {
        track_condition: TrackCondition::Soft,
        weather: Weather::Rainy,
        season: Season::Sakura,
        time: Time::Night,
        grade: Grade::G2,
        gates: 9,
    };

    let actual = [
        RaceParamVar::GroundCondition,
        RaceParamVar::Grade,
        RaceParamVar::Season,
        RaceParamVar::Time,
        RaceParamVar::Weather,
    ]
    .map(|var| get_race_param_value(&var, &params));
    assert_eq!(actual, [3, 200, 5, 4, 3]);
}

#[test]
fn test_course_values() {
    // Sapporo 1200m turf: 2 corners, clockwise.
    let course = load_course(10101);

    let actual = [
        CourseVar::CornerCount,
        CourseVar::CourseDistance,
        CourseVar::DistanceType,
        CourseVar::GroundType,
        CourseVar::IsAbroad,
        CourseVar::IsBasisDistance,
        CourseVar::IsDirtgrade,
        CourseVar::IsTightTrack,
        CourseVar::Rotation,
        CourseVar::TrackId,
    ]
    .map(|var| get_course_value(&var, &course));
    assert_eq!(actual, [2, 1200, 1, 1, 0, 1, 0, 1, 1, 10001]);
}

#[test]
fn test_course_flags_by_track() {
    // (track ID, distance, is_abroad, is_basis_distance, is_dirtgrade,
    // is_tight_track).
    let cases = [
        // Longchamp.
        (10201, 2400, 1, 1, 0, 0),
        // Ooi.
        (10101, 2000, 0, 1, 1, 0),
        // Kawasaki.
        (10103, 2100, 0, 0, 1, 1),
    ];

    for (race_track_id, distance, abroad, basis, dirtgrade, tight) in cases {
        let data = CourseData {
            race_track_id,
            distance,
            ..course_data()
        };
        let course = Course::new_test(0, data);
        let actual = [
            CourseVar::IsAbroad,
            CourseVar::IsBasisDistance,
            CourseVar::IsDirtgrade,
            CourseVar::IsTightTrack,
        ]
        .map(|var| get_course_value(&var, &course));
        assert_eq!(
            actual,
            [abroad, basis, dirtgrade, tight],
            "track {race_track_id}"
        );
    }
}

/// Synthetic 2000m course with two corners (1000-1200 and 1400-1800), a
/// straight on each end (opposite and facing the audience) and two slopes
/// (uphill at 200-300 and downhill at 500-600).
fn location_course() -> Course {
    let data = CourseData {
        corners: vec![
            Corner {
                start: 1000.0,
                length: 200.0,
            },
            Corner {
                start: 1400.0,
                length: 400.0,
            },
        ],
        straights: vec![
            Straight {
                start: 0.0,
                end: 1000.0,
                front_type: FrontType::Opposite,
            },
            Straight {
                start: 1800.0,
                end: 2000.0,
                front_type: FrontType::Audience,
            },
        ],
        slopes: vec![
            Slope::new(200.0, 100.0, 1.5),
            Slope::new(500.0, 100.0, -1.0),
        ],
        ..course_data()
    };
    Course::new_test(0, data)
}

/// Move a runner to `position`, entering the corner, straight and slope there
/// (if any).
fn place(runner: &mut Runner, course: &Course, position: f64) {
    runner.position = position;
    runner.segments = SegmentState::default();
    runner.segments.update(position, course);
}

#[test]
fn test_location_values_by_position() {
    let mut solver = create_test_race_solver(vec![]);
    let course = location_course();

    // (position, corner, is_finalcorner, is_finalcorner_laterhalf,
    // is_last_straight, slope, straight_front_type).
    let cases = [
        (0.0, 0, 0, 0, 0, 0, 2),
        (250.0, 0, 0, 0, 0, 1, 2),
        (550.0, 0, 0, 0, 0, 2, 2),
        // First of two corners is the third one.
        (1100.0, 3, 0, 0, 0, 0, 0),
        (1300.0, 0, 0, 0, 0, 0, 0),
        (1400.0, 4, 1, 0, 0, 0, 0),
        (1599.9, 4, 1, 0, 0, 0, 0),
        (1600.0, 4, 1, 1, 0, 0, 0),
        (1800.0, 0, 1, 1, 1, 0, 1),
    ];

    for (position, corner, final_corner, laterhalf, last_straight, slope, front) in cases {
        place(&mut solver.runners[0], &course, position);
        let runner = &solver.runners[0];
        let actual = [
            SpatialVar::Corner,
            SpatialVar::IsFinalcorner,
            SpatialVar::IsFinalcornerLaterhalf,
            SpatialVar::IsLastStraight,
            SpatialVar::Slope,
            SpatialVar::StraightFrontType,
        ]
        .map(|var| get_location_value(&var, &course, runner));
        assert_eq!(
            actual,
            [corner, final_corner, laterhalf, last_straight, slope, front],
            "position {position}"
        );
    }
}

#[test]
fn test_location_values_without_corners() {
    let mut solver = create_test_race_solver(vec![]);
    let course = Course::new_test(0, course_data());
    place(&mut solver.runners[0], &course, 100.0);

    let runner = &solver.runners[0];
    let actual = [
        SpatialVar::Corner,
        SpatialVar::IsFinalcorner,
        SpatialVar::IsFinalcornerLaterhalf,
        SpatialVar::IsLastStraight,
    ]
    .map(|var| get_location_value(&var, &course, runner));
    // The only straight is the last one.
    assert_eq!(actual, [0, 0, 0, 1]);
}

#[test]
fn test_distance_location_values() {
    let mut solver = create_test_race_solver(vec![]);
    let course = location_course();
    place(&mut solver.runners[0], &course, 1500.0);

    let runner = &solver.runners[0];
    let actual = [
        SpatialVar::DistanceRate,
        SpatialVar::Furlong,
        SpatialVar::RemainDistance,
    ]
    .map(|var| get_location_value(&var, &course, runner));
    // 1500m are 7.46 furlongs.
    assert_eq!(actual, [75, 7, 500]);
}

#[test]
fn test_lane_type_values() {
    let mut solver = create_test_race_solver(vec![]);
    let course = location_course();

    let cases = [(0.0, 0), (0.2, 0), (0.3, 1), (0.5, 2), (0.7, 3)];
    for (lane, expected) in cases {
        solver.runners[0].lane.current = lane;
        let actual = get_location_value(&SpatialVar::LaneType, &course, &solver.runners[0]);
        assert_eq!(actual, expected, "lane {lane}");
    }
}

#[test]
fn test_distance_values() {
    let mut solver = create_test_race_solver(vec![]);
    let diff = &mut solver.runners[0].position_diff;
    diff.from_prev = 7.6;
    diff.from_next = 5.0;
    diff.rate_from_first = 42.7;
    diff.from_first = 12.34;

    let runner = &solver.runners[0];
    let actual = [
        DistanceVar::BashinDiffBehind,
        DistanceVar::BashinDiffInfront,
        DistanceVar::DistanceDiffRate,
        DistanceVar::DistanceDiffTop,
        DistanceVar::DistanceDiffTopFloat,
    ]
    .map(|var| get_distance_value(&var, runner));
    // 1 bashin is 2.5m.
    assert_eq!(actual, [3, 2, 42, 12, 123]);
}

#[test]
fn test_rank_values() {
    let mut solver = create_test_race_solver(vec![]);
    let order = &mut solver.runners[0].order;
    order.order = 4;
    order.delta = -2;
    order.overtakes_since_late = 5;
    order.overtakes_since_last_corner = 6;
    order.overtakes_since_mid = 7;

    let runner = &solver.runners[0];
    let actual = [
        OrderVar::Order,
        OrderVar::ChangeOrderOnetime,
        OrderVar::ChangeOrderUpEndAfter,
        OrderVar::ChangeOrderUpFinalcornerAfter,
        OrderVar::ChangeOrderUpMiddle,
    ]
    .map(|var| get_rank_value(&var, runner));
    assert_eq!(actual, [4, -2, 5, 6, 7]);
}

#[test]
fn test_order_rate_is_rewritten_to_rounded_order() {
    let mut solver = create_test_race_solver(vec![]);

    // 9 * 50% = 4.5 rounds to 5; 12 * 50% = 6 rounds to 6.
    for (gates, order) in [(9, 5), (12, 6)] {
        solver.state.params.gates = gates;
        let entry = &solver.runners[0].initial_state;
        for op in [CompOp::Greater, CompOp::LessEqual] {
            let comp = Comparison {
                var: VarType::Order(OrderVar::OrderRate),
                op,
                value: 50,
            };
            let expected = Comparison {
                var: VarType::Order(OrderVar::Order),
                op,
                value: order,
            };
            let actual = rewrite_comparison(&comp, &solver.state, entry);
            assert_eq!(actual, Some(expected), "gates {gates}, {op}");
        }
    }
}

#[test]
#[should_panic(expected = "should have been rewritten")]
fn test_order_rate_value_panics() {
    let solver = create_test_race_solver(vec![]);
    get_rank_value(&OrderVar::OrderRate, &solver.runners[0]);
}

#[test]
fn test_ignored_popularity_is_rewritten_away() {
    let mut solver = create_test_race_solver(vec![]);
    let comp = comp("popularity>=10");

    let actual = rewrite_comparison(&comp, &solver.state, &solver.runners[0].initial_state);
    assert_eq!(actual, Some(comp.clone()));
    solver.runners[0].initial_state.ignores_popularity = true;
    let actual = rewrite_comparison(&comp, &solver.state, &solver.runners[0].initial_state);
    assert_eq!(actual, None);
}

#[test]
fn test_other_comparisons_are_not_rewritten() {
    let mut solver = create_test_race_solver(vec![]);
    solver.runners[0].initial_state.ignores_popularity = true;

    for condition in ["order<=5", "hp_per>=20", "running_style==3"] {
        let comp = comp(condition);
        let actual = rewrite_comparison(&comp, &solver.state, &solver.runners[0].initial_state);
        assert_eq!(actual, Some(comp), "{condition}");
    }
}

#[test]
fn test_order_rate_in_continue_boundaries() {
    let mut solver = create_test_race_solver(vec![]);
    // The best order shouldn't matter.
    solver.runners[0].order.best = 1;

    // Top X% of 9 runners, rounded: 20% => 2, 40% => 4, 50% => 5, 80% => 7.
    // Top X% of 12 runners, rounded: 20% => 2, 40% => 5, 50% => 6, 80% => 10.
    // (runners, worst order, [in20, in40, in50, in80]).
    let cases = [
        (9, 2, [1, 1, 1, 1]),
        (9, 3, [0, 1, 1, 1]),
        (9, 5, [0, 0, 1, 1]),
        (9, 6, [0, 0, 0, 1]),
        (9, 7, [0, 0, 0, 1]),
        (9, 8, [0, 0, 0, 0]),
        (12, 2, [1, 1, 1, 1]),
        (12, 3, [0, 1, 1, 1]),
        (12, 5, [0, 1, 1, 1]),
        (12, 6, [0, 0, 1, 1]),
        (12, 7, [0, 0, 0, 1]),
        (12, 10, [0, 0, 0, 1]),
        (12, 11, [0, 0, 0, 0]),
    ];
    for (runners, worst, expected) in cases {
        solver.runners[0].order.runner_count = runners;
        solver.runners[0].order.worst = worst;
        let actual = [
            OrderVar::OrderRateIn20Continue,
            OrderVar::OrderRateIn40Continue,
            OrderVar::OrderRateIn50Continue,
            OrderVar::OrderRateIn80Continue,
        ]
        .map(|var| get_rank_value(&var, &solver.runners[0]));
        assert_eq!(actual, expected, "runners {runners}, worst order {worst}");
    }
}

#[test]
fn test_order_rate_out_continue_boundaries() {
    let mut solver = create_test_race_solver(vec![]);
    // The worst order shouldn't matter.
    solver.runners[0].order.worst = 12;

    // Top X% of 9 runners, rounded: 20% => 2, 40% => 4, 50% => 5, 70% => 6.
    // Top X% of 12 runners, rounded: 20% => 2, 40% => 5, 50% => 6, 70% => 8.
    // The rounded order itself counts as outside.
    // (runners, best order, [out20, out40, out50, out70]).
    let cases = [
        (9, 1, [0, 0, 0, 0]),
        (9, 2, [1, 0, 0, 0]),
        (9, 3, [1, 0, 0, 0]),
        (9, 4, [1, 1, 0, 0]),
        (9, 5, [1, 1, 1, 0]),
        (9, 6, [1, 1, 1, 1]),
        (12, 1, [0, 0, 0, 0]),
        (12, 2, [1, 0, 0, 0]),
        (12, 4, [1, 0, 0, 0]),
        (12, 5, [1, 1, 0, 0]),
        (12, 6, [1, 1, 1, 0]),
        (12, 7, [1, 1, 1, 0]),
        (12, 8, [1, 1, 1, 1]),
    ];
    for (runners, best, expected) in cases {
        solver.runners[0].order.runner_count = runners;
        solver.runners[0].order.best = best;
        let actual = [
            OrderVar::OrderRateOut20Continue,
            OrderVar::OrderRateOut40Continue,
            OrderVar::OrderRateOut50Continue,
            OrderVar::OrderRateOut70Continue,
        ]
        .map(|var| get_rank_value(&var, &solver.runners[0]));
        assert_eq!(actual, expected, "runners {runners}, best order {best}");
    }
}

const BLOCKING_VARS: [BlockingVar; 14] = [
    BlockingVar::BlockedFront,
    BlockingVar::BlockedFrontContinuetime,
    BlockingVar::BlockedSideContinuetime,
    BlockingVar::BlockedAllContinuetime,
    BlockingVar::IsSurrounded,
    BlockingVar::NearCount,
    BlockingVar::NearInfrontCount,
    BlockingVar::InfrontNearLaneTime,
    BlockingVar::BehindNearLaneTime,
    BlockingVar::BehindNearLaneTimeSet1,
    BlockingVar::IsBehindIn,
    BlockingVar::OvertakeTargetNoOrderUpTime,
    BlockingVar::OvertakeTargetTime,
    BlockingVar::Visiblehorse,
];

#[test]
fn test_blocking_values() {
    let mut solver = create_test_race_solver(vec![]);
    let runner = &mut solver.runners[0];
    runner.blocked.speed_limit = Some(10.0);
    runner.blocked.front_time = 3.7;
    runner.blocked.side_time = 5.2;
    runner.surrounded.front = true;
    runner.surrounded.back = true;
    runner.surrounded.side = true;
    runner.nearby.count = 4;
    runner.nearby.ahead_same_lane_count = 2;
    runner.nearby.ahead_same_lane_time = 1.9;
    runner.nearby.behind_same_lane_time = 2.5;
    runner.nearby.behind_set1_time = 6.1;
    runner.nearby.behind_in = true;
    runner.overtake_target_time = 8.4;
    runner.being_overtake_target_time = 9.0;
    runner.visible_count = 11;

    let runner = &solver.runners[0];
    let actual = BLOCKING_VARS.map(|var| get_blocking_value(&var, runner));
    // Blocked on all sides for as long as the shortest of both.
    assert_eq!(actual, [1, 3, 5, 3, 1, 4, 2, 1, 2, 6, 1, 8, 9, 11]);
}

#[test]
fn test_blocking_values_when_free() {
    let mut solver = create_test_race_solver(vec![]);
    // Not surrounded unless blocked on all sides.
    solver.runners[0].surrounded.front = true;
    solver.runners[0].surrounded.back = true;

    let runner = &solver.runners[0];
    let actual = BLOCKING_VARS.map(|var| get_blocking_value(&var, runner));
    assert_eq!(actual, [0; 14]);
}

#[test]
fn test_hp_values() {
    let mut solver = create_test_race_solver(vec![]);
    let hp = &mut solver.runners[0].hp;
    hp.current = hp.maximum * 0.4999;
    hp.has_emptied = true;

    let runner = &solver.runners[0];
    let actual =
        [StatusVar::HpPer, StatusVar::IsHpEmptyOnetime].map(|var| get_status_value(&var, runner));
    assert_eq!(actual, [49, 1]);
}

#[test]
fn test_is_lastspurt_by_position() {
    let mut solver = create_test_race_solver(vec![]);
    let var = StatusVar::IsLastspurt;

    // Not calculated yet.
    assert_eq!(get_status_value(&var, &solver.runners[0]), 0);

    solver.runners[0].last_spurt = Some(LastSpurt {
        max_speed: 20.0,
        target_distance: 1000.0,
        category: LastSpurtCategory::Full,
    });
    for (position, expected) in [(999.9, 0), (1000.0, 1)] {
        solver.runners[0].position = position;
        let actual = get_status_value(&var, &solver.runners[0]);
        assert_eq!(actual, expected, "position {position}");
    }
}

#[test]
fn test_lastspurt_values() {
    let mut solver = create_test_race_solver(vec![]);
    let var = StatusVar::Lastspurt;

    // Not calculated yet.
    assert_eq!(get_status_value(&var, &solver.runners[0]), -1);

    let cases = [
        (LastSpurtCategory::CannotFinish, 0),
        (LastSpurtCategory::AboveBase, 1),
        (LastSpurtCategory::Full, 2),
    ];
    for (category, expected) in cases {
        solver.runners[0].last_spurt = Some(LastSpurt {
            max_speed: 20.0,
            target_distance: 1000.0,
            category,
        });
        let actual = get_status_value(&var, &solver.runners[0]);
        assert_eq!(actual, expected, "{category:?}");
    }
}

#[test]
fn test_is_badstart_by_start_delay() {
    let mut solver = create_test_race_solver(vec![]);

    for (start_delay, expected) in [(0.0, 0), (0.08, 0), (0.081, 1)] {
        solver.runners[0].start_delay = start_delay;
        let actual = get_status_value(&StatusVar::IsBadstart, &solver.runners[0]);
        assert_eq!(actual, expected, "start delay {start_delay}");
    }
}

#[test]
fn test_is_goodstart_by_start_delay() {
    let mut solver = create_test_race_solver(vec![]);

    for (start_delay, expected) in [(0.0, 1), (0.019, 1), (0.02, 0), (0.1, 0)] {
        solver.runners[0].start_delay = start_delay;
        let actual = get_status_value(&StatusVar::IsGoodstart, &solver.runners[0]);
        assert_eq!(actual, expected, "start delay {start_delay}");
    }
}

#[test]
fn test_is_move_lane_by_lane_speed() {
    let mut solver = create_test_race_solver(vec![]);

    // Inward, still, outward.
    for (speed, expected) in [(-0.01, 1), (0.0, 0), (0.01, 2)] {
        solver.runners[0].lane.speed = speed;
        let actual = get_status_value(&StatusVar::IsMoveLane, &solver.runners[0]);
        assert_eq!(actual, expected, "lane speed {speed}");
    }
}

#[test]
fn test_is_overtake_without_targets() {
    let solver = create_test_race_solver(vec![]);
    let actual = get_status_value(&StatusVar::IsOvertake, &solver.runners[0]);
    assert_eq!(actual, 0);
}

#[test]
fn test_temptation_status_values() {
    let mut solver = create_test_race_solver(vec![]);
    let rush = |is_rushed| RushState {
        section: 2,
        start_time: 5.0,
        end_time: 15.0,
        max_duration: 12.0,
        is_rushed,
        style: Style::Late,
    };

    // (rush state, is_temptation, temptation_count).
    let cases = [
        (None, 0, 0),
        (Some(rush(true)), 1, 1),
        (Some(rush(false)), 0, 1),
    ];
    for (rush_state, is_temptation, count) in cases {
        let label = rush_state.as_ref().map(|s| s.is_rushed);
        solver.runners[0].rush_state = rush_state;
        let actual = [StatusVar::IsTemptation, StatusVar::TemptationCount]
            .map(|var| get_status_value(&var, &solver.runners[0]));
        assert_eq!(actual, [is_temptation, count], "is_rushed {label:?}");
    }
}

#[test]
fn test_activation_counts() {
    let mut solver = create_test_race_solver(vec![]);
    let counts = &mut solver.runners[0].trigger_scheduler.activation_counts;
    counts.all = 6;
    counts.start = 1;
    counts.middle = 2;
    counts.later_half = 3;
    counts.end_after = 4;
    counts.heal = 5;
    counts.green = 3;

    let (state, runner) = (&solver.state, &solver.runners[0]);
    let actual = [
        ActivationVar::ActivateCountAll,
        ActivationVar::ActivateCountStart,
        ActivationVar::ActivateCountMiddle,
        ActivationVar::ActivateCountLaterHalf,
        ActivationVar::ActivateCountEndAfter,
        ActivationVar::ActivateCountHeal,
        ActivationVar::ActivateCountParamSkill,
    ]
    .map(|var| get_activation_value(&var, state, runner, None));
    assert_eq!(actual, [6, 1, 2, 3, 4, 5, 3]);
}

#[test]
fn test_is_activate_other_skill_detail_without_logic() {
    let solver = solver_with_activated_skill(1, 0b10);

    let var = ActivationVar::IsActivateOtherSkillDetail;
    let actual = get_activation_value(&var, &solver.state, &solver.runners[0], None);
    assert_eq!(actual, 0);
}

#[test]
fn test_compete_fight_count_by_duel_state() {
    let mut solver = create_test_race_solver(vec![]);

    // Still counts after leaving the duel.
    for (duel, expected) in [(Duel::Pending, 0), (Duel::start(800), 1), (Duel::Ended, 1)] {
        solver.runners[0].duel = duel;
        let var = ActivationVar::CompeteFightCount;
        let actual = get_activation_value(&var, &solver.state, &solver.runners[0], None);
        assert_eq!(actual, expected, "{duel:?}");
    }
}

#[test]
fn test_running_style_counts() {
    let mut solver = RaceSolver::new(load_course(10101), race_params(), &seeded_config(0));
    let styles = [
        Style::Runaway,
        Style::Front,
        Style::Pace,
        Style::Pace,
        Style::Late,
        Style::End,
    ];
    for style in styles {
        solver.add_runner(uma(style), Mood::Normal, vec![]);
    }
    let solver = solver.ready_runners();
    let state = &solver.state;

    let vars = [
        RunningStyleVar::RunningStyle,
        RunningStyleVar::RunningStyleCountNigeOtherself,
        RunningStyleVar::RunningStyleCountSenkoOtherself,
        RunningStyleVar::RunningStyleCountSashiOtherself,
        RunningStyleVar::RunningStyleCountOikomiOtherself,
        RunningStyleVar::RunningStyleCountSame,
        RunningStyleVar::RunningStyleCountSameRate,
    ];
    // Front: 2 (Runaway included), Pace: 2, Late: 1, End: 1, with 9 gates.
    let cases = [
        (Style::Pace, [2, 2, 1, 1, 1, 2, 22]),
        (Style::End, [4, 2, 2, 1, 0, 1, 11]),
    ];
    for (style, expected) in cases {
        let actual = vars.map(|var| get_running_style_value(&var, state, style));
        assert_eq!(actual, expected, "{style:?}");
    }
}

#[test]
fn test_style_temptation_counts() {
    let mut solver = create_team_race_solver();
    solver.state.rushed_by_team[0][Style::Front as usize] = 1;
    solver.state.rushed_by_team[0][Style::Pace as usize] = 2;
    solver.state.rushed_by_team[0][Style::End as usize] = 3;
    solver.state.rushed_by_team[1][Style::Front as usize] = 2;
    solver.state.rushed_by_team[1][Style::End as usize] = 1;

    let runner = team_runner(&solver, 0);
    let actual = [
        TemptationVar::RunningStyleTemptationCountNige,
        TemptationVar::RunningStyleTemptationCountSenko,
        TemptationVar::RunningStyleTemptationCountSashi,
        TemptationVar::RunningStyleTemptationCountOikomi,
        TemptationVar::RunningStyleTemptationOpponentCountNige,
        TemptationVar::RunningStyleTemptationOpponentCountSenko,
        TemptationVar::RunningStyleTemptationOpponentCountSashi,
        TemptationVar::RunningStyleTemptationOpponentCountOikomi,
    ]
    .map(|var| get_temptation_value(&var, &solver.state, runner));
    assert_eq!(actual, [3, 2, 0, 4, 2, 0, 0, 1]);
}

#[test]
fn test_variable_value_dispatch() {
    let mut solver = create_test_race_solver(vec![]);
    let runner = &mut solver.runners[0];
    runner.base.speed = 1101;
    runner.initial_state.fan_count = 12345;
    runner.order.order = 4;
    runner.position_diff.from_first = 12.0;
    runner.position = 1000.0;
    runner.phase = CoursePhase::Mid;
    runner.hp.current = runner.hp.maximum / 2.0;
    runner.visible_count = 11;
    runner.rush_counter.ahead = 3;
    runner.trigger_scheduler.activation_counts.all = 6;
    solver.state.elapsed_time = 42.5;

    let (state, runner) = (&solver.state, &solver.runners[0]);
    let cases = [
        (VarType::BaseStat(BaseStatVar::BaseSpeed), 1101),
        (VarType::Entry(EntryVar::FanCount), 12345),
        (VarType::Order(OrderVar::Order), 4),
        (VarType::Distance(DistanceVar::DistanceDiffTop), 12),
        (VarType::Phase(PhaseVar::Phase), 1),
        (VarType::Course(CourseVar::CourseDistance), 1200),
        (VarType::Spatial(SpatialVar::RemainDistance), 200),
        (VarType::Activation(ActivationVar::ActivateCountAll), 6),
        (VarType::RaceParam(RaceParamVar::Grade), 100),
        (VarType::RunningStyle(RunningStyleVar::RunningStyle), 3),
        (VarType::Status(StatusVar::HpPer), 50),
        (VarType::Blocking(BlockingVar::Visiblehorse), 11),
        (
            VarType::Temptation(TemptationVar::TemptationCountInfront),
            3,
        ),
        (VarType::AccumulateTime, 42),
        (VarType::Always, 1),
    ];
    for (var, expected) in cases {
        let actual = get_variable_value(&var, state, runner, None);
        assert_eq!(actual, expected, "{var}");
    }
}

#[test]
fn test_run_at_full_speed_random_follows_all_out_spurt() {
    let mut solver = create_test_race_solver(vec![]);
    let var = VarType::RandomSpatial(RandomSpatialVar::RunAtFullSpeedRandom);

    let cases = [
        (AllOutSpurt::Pending, 0),
        (AllOutSpurt::Active { speed_bonus: 0.5 }, 1),
        (AllOutSpurt::Ended, 0),
    ];
    for (all_out_spurt, expected) in cases {
        solver.runners[0].all_out_spurt = all_out_spurt;
        let actual = get_variable_value(&var, &solver.state, &solver.runners[0], None);
        assert_eq!(actual, expected, "{all_out_spurt:?}");
    }
}

/// Fresh race state and unready Late Surger, before the race starts.
fn create_out_of_gate_runner() -> (RaceState, UnreadyRunner) {
    let solver = RaceSolver::new(load_course(10101), race_params(), &seeded_config(0));
    let state = solver.state;
    let mut runner = UnreadyRunner::new(uma(Style::Late), 1, Mood::Good, &state, vec![]);
    runner.base.speed = 1101;
    (state, runner)
}

#[test]
fn test_out_of_gate_values() {
    let (state, runner) = create_out_of_gate_runner();

    let cases = [
        ("base_speed==1101", true),
        ("base_speed>1101", false),
        ("motivation==4", true),
        ("distance_type==1", true),
        ("weather==1", true),
        ("weather==3", false),
        ("running_style==3", true),
        ("always==1", true),
    ];
    for (condition, expected) in cases {
        let actual = EvaluatorOutOfGate.evaluate(&comp(condition), None, &state, &runner);
        assert_eq!(actual, expected, "{condition}");
    }
}

#[test]
fn test_out_of_gate_in_race_variables_are_zero() {
    let (state, runner) = create_out_of_gate_runner();

    for var in ["hp_per", "phase", "order", "corner", "accumulatetime"] {
        let comp = comp(&format!("{var}==0"));
        assert!(
            EvaluatorOutOfGate.evaluate(&comp, None, &state, &runner),
            "{var}"
        );
    }
}

#[test]
#[should_panic(expected = "should be handled in evaluate()")]
fn test_is_exist_skill_id_value_panics() {
    let solver = create_test_race_solver(vec![]);
    let var = VarType::Lineup(LineupVar::IsExistSkillId);
    get_variable_value(&var, &solver.state, &solver.runners[0], None);
}

#[test]
fn test_same_skill_horse_count_value() {
    let skill = Skill::new(200331, skill_data(vec![]));
    let mut solver = create_test_race_solver(vec![skill]);
    solver.state.existing_skills.insert(200331, 3);

    let var = LineupVar::SameSkillHorseCount;
    assert_eq!(get_lineup_value(&var, &solver.state, 200331), 3);
    // Skills nobody has, or no skill being evaluated.
    assert_eq!(get_lineup_value(&var, &solver.state, 200332), 0);
    let var = VarType::Lineup(var);
    let actual = get_variable_value(&var, &solver.state, &solver.runners[0], None);
    assert_eq!(actual, 0);
}

#[test]
fn test_same_skill_horse_count_out_of_gate() {
    let (mut state, mut runner) = create_out_of_gate_runner();
    state.existing_skills.insert(200331, 2);
    runner.evaluated_skill_id = 200331;

    let cases = [
        ("same_skill_horse_count==2", true),
        ("same_skill_horse_count>=3", false),
    ];
    for (condition, expected) in cases {
        let actual = EvaluatorOutOfGate.evaluate(&comp(condition), None, &state, &runner);
        assert_eq!(actual, expected, "{condition}");
    }
}

#[test]
#[should_panic(expected = "it's a trigger variable")]
fn test_random_variable_value_panics() {
    let solver = create_test_race_solver(vec![]);
    let var = VarType::Random(RandomVar::RandomLot);
    get_variable_value(&var, &solver.state, &solver.runners[0], None);
}

#[test]
#[should_panic(expected = "it's a trigger variable")]
fn test_random_phase_variable_value_panics() {
    let solver = create_test_race_solver(vec![]);
    let var = VarType::RandomPhase(RandomPhaseVar::PhaseRandom);
    get_variable_value(&var, &solver.state, &solver.runners[0], None);
}

#[test]
#[should_panic(expected = "it's a trigger variable")]
fn test_random_spatial_variable_value_panics() {
    let solver = create_test_race_solver(vec![]);
    let var = VarType::RandomSpatial(RandomSpatialVar::CornerRandom);
    get_variable_value(&var, &solver.state, &solver.runners[0], None);
}

#[test]
#[should_panic(expected = "it's a trigger variable")]
fn test_is_last_straight_onetime_value_panics() {
    let solver = create_test_race_solver(vec![]);
    let var = SpatialVar::IsLastStraightOnetime;
    get_location_value(&var, &solver.state.course, &solver.runners[0]);
}

#[test]
#[should_panic(expected = "should be handled in evaluate()")]
fn test_is_used_skill_id_value_panics() {
    let solver = create_test_race_solver(vec![]);
    let var = VarType::ActivationQuery(ActivationQueryVar::IsUsedSkillId);
    get_variable_value(&var, &solver.state, &solver.runners[0], None);
}

#[test]
#[should_panic(expected = "should be handled in evaluate()")]
fn test_is_used_skill_id_with_detail_one_value_panics() {
    let solver = create_test_race_solver(vec![]);
    let var = VarType::ActivationQuery(ActivationQueryVar::IsUsedSkillIdWithDetailOne);
    get_variable_value(&var, &solver.state, &solver.runners[0], None);
}

/// Evaluate `is_popularity_top_character_activate_advantage_skill==value`.
fn popularity_top_activated(solver: &RaceSolver<Runner>, value: i32) -> bool {
    let comp = Comparison {
        var: VarType::ActivationQuery(
            ActivationQueryVar::IsPopularityTopCharacterActivateAdvantageSkill,
        ),
        op: CompOp::Equal,
        value,
    };
    EvaluatorFull.evaluate(&comp, None, &solver.state, &solver.runners[0])
}

#[test]
fn test_is_popularity_top_character_activate_advantage_skill() {
    let mut solver = create_test_race_solver(vec![]);
    solver.runners[0].initial_state.popularity_rank = 2;
    // Nothing activated.
    assert!(!popularity_top_activated(&solver, 9));
    assert!(!popularity_top_activated(&solver, -1));

    // A debuff (or any other non-buff skill) only counts for -1.
    let top = &mut solver.state.popularity_top_activations;
    top.any_activated = true;
    assert!(!popularity_top_activated(&solver, 9));
    assert!(popularity_top_activated(&solver, -1));

    // A heal.
    let top = &mut solver.state.popularity_top_activations;
    top.buff_activated.insert(SkillType::Recovery);
    assert!(popularity_top_activated(&solver, 9));
    assert!(!popularity_top_activated(&solver, 27));
    assert!(popularity_top_activated(&solver, -1));

    // The runner's own activations count too if she's the most popular one.
    solver.runners[0].initial_state.popularity_rank = 1;
    solver.runners[0].activation_flags = solver.state.popularity_top_activations.clone();
    assert!(popularity_top_activated(&solver, 9));
    assert!(popularity_top_activated(&solver, -1));
}

#[test]
fn test_succession_skill_count() {
    let skill = |id: i32| Skill::new(id, skill_data(vec![]));
    // Inherited uniques, a unique and a white skill.
    let skills = [900011, 900021, 100011, 200012].map(skill).to_vec();
    let solver = create_test_race_solver(skills);
    let actual = get_status_value(&StatusVar::SuccessionSkillCount, &solver.runners[0]);
    assert_eq!(actual, 2);
}

#[test]
#[should_panic(expected = "should be handled in evaluate()")]
fn test_is_other_character_activate_advantage_skill_value_panics() {
    let solver = create_test_race_solver(vec![]);
    let var = VarType::ActivationQuery(ActivationQueryVar::IsOtherCharacterActivateAdvantageSkill);
    get_variable_value(&var, &solver.state, &solver.runners[0], None);
}
