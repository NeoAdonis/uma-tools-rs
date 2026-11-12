use super::*;
use crate::course::Phase as CoursePhase;
use crate::course::geometry::*;
use crate::race::all_out_spurt::AllOutSpurt;
use crate::race::duel::Duel;
use crate::race::last_spurt::{LastSpurt, LastSpurtCategory};
use crate::race::parameters::*;
use crate::race::pins::Pins;
use crate::race::rng::RngSeed;
use crate::race::runner::Runner;
use crate::race::rush::RushState;
use crate::race::solver::{Entrant, RaceSolver};
use crate::race::transitions::SegmentState;
use crate::skill::Skill;
use crate::test_utils::*;
use std::sync::Arc;

/// Ready race with a single Late Surger with the given skills.
fn create_test_race_solver(skills: Vec<Arc<Skill>>) -> RaceSolver<Runner> {
    ready_solver(10101, uma(BaseStyle::Late), skills, seeded_config(12345))
}

/// Whole number of a single value.
fn int(value: Value) -> i32 {
    match value {
        Value::Scalar(value) if value.fract() == 0.0 => value as i32,
        _ => panic!("not a whole number"),
    }
}

/// Ready race with a single Late Surger holding a skill with the given ID,
/// activated through the alternatives in `mask`.
fn solver_with_activated_skill(skill_id: i32, mask: u8) -> RaceSolver<Runner> {
    let skill = Arc::new(Skill {
        id: skill_id,
        ..synthetic_skill(vec![])
    });
    let mut solver = create_test_race_solver(vec![skill]);
    solver.runners[0].activated_alternatives[0] = mask;
    solver
}

#[test]
fn test_is_used_skill_id() {
    // (activated skill, alternatives mask, [used==, used!=, detail_one==,
    // detail_one!=]), checking skill 204452.
    let cases = [
        // Not activated.
        (204452, 0b00, [false, true, false, true]),
        // Through its first alternative.
        (204452, 0b01, [true, false, true, false]),
        // Only through its second alternative.
        (204452, 0b10, [true, false, false, true]),
        // Another skill.
        (203781, 0b01, [false, true, false, true]),
    ];
    for (skill_id, mask, expected) in cases {
        let solver = solver_with_activated_skill(skill_id, mask);
        let (state, runner) = (&solver.state, &solver.runners[0]);
        let actual = [
            "is_used_skill_id==204452",
            "is_used_skill_id!=204452",
            "is_used_skill_id_with_detail_one==204452",
            "is_used_skill_id_with_detail_one!=204452",
        ]
        .map(|condition| EvaluatorFull.evaluate(&comp(condition), None, state, runner));
        assert_eq!(actual, expected, "skill {skill_id}, mask {mask:#04b}");
    }
}

#[test]
fn test_is_exist_skill_id() {
    // Skills in the lineup are those of every runner.
    let skill = Arc::new(Skill {
        id: 203061,
        ..synthetic_skill(vec![])
    });
    let solver = create_test_race_solver(vec![skill]);
    let (state, runner) = (&solver.state, &solver.runners[0]);
    let cases = [
        ("is_exist_skill_id==203061", true),
        ("is_exist_skill_id!=203061", false),
        ("is_exist_skill_id==203062", false),
        ("is_exist_skill_id!=203062", true),
    ];
    for (condition, expected) in cases {
        let actual = EvaluatorFull.evaluate(&comp(condition), None, state, runner);
        assert_eq!(actual, expected, "{condition}");
    }
}

#[test]
fn test_phase_values_by_position() {
    let mut solver = create_test_race_solver(vec![]);
    let section = solver.state.course.section_length();

    // (position in sections, phase, phase_firstquarter, phase_firsthalf,
    // phase_laterhalf).
    let cases = [
        // Early: sections 0-4 (quarter 0-1, halves 0-2 and 2-4).
        (0.0, 0, 0, 0, -1),
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
        .map(|var| phase_value(&var, course, runner));

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
    let mut solver = RaceSolver::new(load_course(10101), race_params(), seeded_config(0));
    let member = || Entrant::new(uma(BaseStyle::Late), vec![]);
    solver.add_team(vec![member(), member()]);
    solver.add_runner(Entrant::new(uma(BaseStyle::Late), vec![]));
    solver.ready_runners()
}

/// First runner of `team`.
fn team_runner(solver: &RaceSolver<Runner>, team: u8) -> &Runner {
    let mut runners = solver.runners.iter();
    runners.find(|r| r.initial_state.team == team).unwrap()
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
    .map(|var| temptation_value(&var, state, runner));
    assert_eq!(actual, [3, 4, 1, 2]);
}

#[test]
fn test_most_popular_runaway_counts_as_front_runner() {
    let mut solver = RaceSolver::new(load_course(10101), race_params(), seeded_config(0));
    solver.add_runner(entrant(EffectiveStyle::Runaway));
    let solver = solver.ready_runners();
    let runner = &solver.runners[0];

    let var = RunningStyleVar::RunningStyleEqualPopularityOne;
    assert_eq!(
        running_style_value(&var, &solver.state, runner.base_style()),
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
        let actual = activation_value(&var, &solver.state, runner, None);
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
fn test_base_stat_values() {
    let mut solver = create_test_race_solver(vec![]);
    // Fractions are truncated.
    solver.runners[0].base = Stats {
        speed: 1101.9,
        stamina: 902.0,
        power: 803.5,
        guts: 704.0,
        wit: 605.1,
    };

    let base = &solver.runners[0].base;
    let actual = [
        BaseStatVar::BaseSpeed,
        BaseStatVar::BaseStamina,
        BaseStatVar::BasePower,
        BaseStatVar::BaseGuts,
        BaseStatVar::BaseWiz,
    ]
    .map(|stat| base_stat(&stat, base));
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
    .map(|var| entry_value(&var, initial_state));
    assert_eq!(actual, [12345, 3, 5]);
}

#[test]
fn test_mood_values() {
    let mut solver = create_test_race_solver(vec![]);

    let cases = [(Mood::Awful, 1), (Mood::Great, 5)];
    for (mood, expected) in cases {
        solver.runners[0].initial_state.mood = mood;
        let actual = entry_value(&EntryVar::Motivation, &solver.runners[0].initial_state);
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
        career: false,
    };

    let actual = [
        RaceParamVar::GroundCondition,
        RaceParamVar::Grade,
        RaceParamVar::Season,
        RaceParamVar::Time,
        RaceParamVar::Weather,
    ]
    .map(|var| race_param_value(&var, &params));
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
    .map(|var| course_value(&var, &course));
    assert_eq!(actual, [2, 1200, 1, 1, 0, 1, 0, 1, 1, 10001]);
}

/// Synthetic 2000m course with two corners (1000-1200 and 1400-1800), a
/// straight on each end (opposite and facing the audience) and two slopes
/// (uphill at 200-300 and downhill at 500-600).
fn location_course() -> Course {
    Course {
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
        ..synthetic_course()
    }
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
        .map(|var| location_value(&var, &course, runner));
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
    let course = synthetic_course();
    place(&mut solver.runners[0], &course, 100.0);

    let runner = &solver.runners[0];
    let actual = [
        SpatialVar::Corner,
        SpatialVar::IsFinalcorner,
        SpatialVar::IsFinalcornerLaterhalf,
        SpatialVar::IsLastStraight,
    ]
    .map(|var| location_value(&var, &course, runner));
    // The only straight is the last one.
    assert_eq!(actual, [0, 0, 0, 1]);
}

#[test]
fn test_distance_location_values() {
    let mut solver = create_test_race_solver(vec![]);
    let course = location_course();
    place(&mut solver.runners[0], &course, 1500.5);

    let runner = &solver.runners[0];
    let actual = [
        SpatialVar::DistanceRate,
        SpatialVar::Furlong,
        SpatialVar::RemainDistance,
    ]
    .map(|var| location_value(&var, &course, runner));
    // 1500m are 7.5 furlongs. The remaining distance is counted from the
    // floored position (2000 - 1500).
    assert_eq!(actual, [75, 7, 500]);
}

#[test]
fn test_lane_type_values() {
    let mut solver = create_test_race_solver(vec![]);
    let course = location_course();

    let cases = [(0.0, 0), (0.2, 0), (0.3, 1), (0.5, 2), (0.7, 3)];
    for (lane, expected) in cases {
        solver.runners[0].lane.current = CourseWidths(lane);
        let actual = location_value(&SpatialVar::LaneType, &course, &solver.runners[0]);
        assert_eq!(actual, expected, "lane {lane}");
    }
}

#[test]
fn test_distance_values() {
    let mut solver = create_test_race_solver(vec![]);
    let diff = &mut solver.runners[0].position_diff;
    diff.rate_from_first = 42.7;
    diff.from_first = 12.34;

    let runner = &solver.runners[0];
    let actual = [
        DistanceVar::DistanceDiffRate,
        DistanceVar::DistanceDiffTop,
        DistanceVar::DistanceDiffTopFloat,
    ]
    .map(|var| int(distance_value(&var, runner)));
    assert_eq!(actual, [42, 12, 123]);
}

#[test]
fn test_bashin_diff_compares_the_unrounded_gap() {
    let mut solver = create_test_race_solver(vec![]);
    // (gap in meters, [<=1, >=1]). 1 bashin is 2.5m, and nobody on that side
    // counts as an infinite gap.
    let cases = [
        (2.49, [true, false]),
        (2.5, [true, true]),
        (2.51, [false, true]),
        (f64::INFINITY, [false, true]),
    ];
    let vars = [
        DistanceVar::BashinDiffBehind,
        DistanceVar::BashinDiffInfront,
    ];
    for (var, behind) in vars.into_iter().zip([true, false]) {
        for (gap, expected) in cases {
            // A gap on the other side that would give other results.
            let diff = &mut solver.runners[0].position_diff;
            (diff.from_prev, diff.from_next) = if behind { (gap, 5.0) } else { (5.0, gap) };
            let value = distance_value(&var, &solver.runners[0]);
            let actual = [CompOp::LessEqual, CompOp::GreaterEqual].map(|op| {
                let comp = Comparison {
                    var: VarType::Distance(var),
                    op,
                    value: 1,
                };
                comp.holds(value.clone())
            });
            assert_eq!(actual, expected, "{var:?} at {gap} m");
        }
    }
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
    .map(|var| rank_value(&var, runner));
    assert_eq!(actual, [4, -2, 5, 6, 7]);
}

#[test]
fn test_order_rate_is_the_range_of_rates_rounding_to_the_order() {
    let mut solver = create_test_race_solver(vec![]);
    solver.runners[0].order.order = 5;
    solver.runners[0].update_order_rate(9, 0.0);
    let var = VarType::OrderRate(OrderRateVar::OrderRate);
    let runner = &solver.runners[0];
    let actual = variable_value(&var, &solver.state, runner, None);
    assert!(matches!(actual, Value::Range(rates) if rates == (50..=61)));
}

#[test]
fn test_scalar_values_hold_if_the_comparison_does() {
    // Results for values below, equal to and above 5.
    let cases = [
        (CompOp::Equal, [false, true, false]),
        (CompOp::NotEqual, [true, false, true]),
        (CompOp::Greater, [false, false, true]),
        (CompOp::GreaterEqual, [false, true, true]),
        (CompOp::Less, [true, false, false]),
        (CompOp::LessEqual, [true, true, false]),
    ];
    for (op, expected) in cases {
        let comp = Comparison {
            var: VarType::Always,
            op,
            value: 5,
        };
        let actual = [4.99, 5.0, 5.01].map(|value| comp.holds(Value::Scalar(value)));
        assert_eq!(actual, expected, "{op}");
    }
}

#[test]
fn test_range_values_hold_if_any_value_does() {
    let comp = |op| Comparison {
        var: VarType::OrderRate(OrderRateVar::OrderRate),
        op,
        value: 50,
    };
    // (range, [==, !=, <=, >, >=, <]).
    let cases = [
        (40..=60, [true, false, true, false, true, false]),
        (50..=50, [true, false, true, false, true, false]),
        (51..=60, [false, true, false, true, true, false]),
        (40..=49, [false, true, true, false, false, true]),
    ];
    let ops = [
        CompOp::Equal,
        CompOp::NotEqual,
        CompOp::LessEqual,
        CompOp::Greater,
        CompOp::GreaterEqual,
        CompOp::Less,
    ];
    for (range, expected) in cases {
        let actual = ops.map(|op| comp(op).holds(Value::Range(range.clone())));
        assert_eq!(actual, expected, "{range:?}");
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
        solver.runners[0].order.worst = worst;
        solver.runners[0].update_order_rate(runners, 0.0);
        let actual = [
            OrderRateVar::OrderRateIn20Continue,
            OrderRateVar::OrderRateIn40Continue,
            OrderRateVar::OrderRateIn50Continue,
            OrderRateVar::OrderRateIn80Continue,
        ]
        .map(|var| int(order_rate_value(&var, &solver.runners[0].order)));
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
        solver.runners[0].order.best = best;
        solver.runners[0].update_order_rate(runners, 0.0);
        let actual = [
            OrderRateVar::OrderRateOut20Continue,
            OrderRateVar::OrderRateOut40Continue,
            OrderRateVar::OrderRateOut50Continue,
            OrderRateVar::OrderRateOut70Continue,
        ]
        .map(|var| int(order_rate_value(&var, &solver.runners[0].order)));
        assert_eq!(actual, expected, "runners {runners}, best order {best}");
    }
}

#[test]
fn test_order_rate_continue_holds_until_orders_are_tracked() {
    let solver = create_test_race_solver(vec![]);
    let order = &solver.runners[0].order;
    // No best nor worst order yet.
    assert_eq!((order.best, order.worst), (u8::MAX, 0));
    for var in [
        OrderRateVar::OrderRateIn20Continue,
        OrderRateVar::OrderRateOut70Continue,
    ] {
        assert_eq!(int(order_rate_value(&var, order)), 1, "{var}");
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
    runner.blocked.speed_ratio = Some(0.99);
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
    let actual = BLOCKING_VARS.map(|var| blocking_value(&var, runner));
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
    let actual = BLOCKING_VARS.map(|var| blocking_value(&var, runner));
    assert_eq!(actual, [0; 14]);
}

#[test]
fn test_hp_values() {
    let mut solver = create_test_race_solver(vec![]);
    let hp = &mut solver.runners[0].hp;
    hp.consume(hp.maximum());
    hp.recover(hp.maximum() * 0.4999);

    let runner = &solver.runners[0];
    let actual =
        [StatusVar::HpPer, StatusVar::IsHpEmptyOnetime].map(|var| status_value(&var, runner));
    assert_eq!(actual, [49, 1]);
}

#[test]
fn test_is_lastspurt_by_position() {
    let mut solver = create_test_race_solver(vec![]);
    let var = StatusVar::IsLastspurt;

    // Not calculated yet.
    assert_eq!(status_value(&var, &solver.runners[0]), 0);

    solver.runners[0].last_spurt = Some(LastSpurt {
        max_speed: 20.0,
        target_distance: 1000.0,
        category: LastSpurtCategory::Full,
    });
    for (position, expected) in [(999.9, 0), (1000.0, 1)] {
        solver.runners[0].position = position;
        let actual = status_value(&var, &solver.runners[0]);
        assert_eq!(actual, expected, "position {position}");
    }
}

#[test]
fn test_lastspurt_values() {
    let mut solver = create_test_race_solver(vec![]);
    let var = StatusVar::Lastspurt;

    // Not calculated yet.
    assert_eq!(status_value(&var, &solver.runners[0]), -1);

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
        let actual = status_value(&var, &solver.runners[0]);
        assert_eq!(actual, expected, "{category:?}");
    }
}

#[test]
fn test_start_types_by_start_delay() {
    let mut solver = create_test_race_solver(vec![]);

    // (start delay, [is_badstart, is_goodstart]).
    let cases = [(0.0, [0, 1]), (0.05, [0, 0]), (0.1, [1, 0])];
    for (start_delay, expected) in cases {
        solver.runners[0].start_delay = start_delay;
        let actual = [StatusVar::IsBadstart, StatusVar::IsGoodstart]
            .map(|var| status_value(&var, &solver.runners[0]));
        assert_eq!(actual, expected, "start delay {start_delay}");
    }
}

#[test]
fn test_is_move_lane_by_lane_speed_and_move_time() {
    let mut solver = create_test_race_solver(vec![]);

    // (lane speed, move time, expected): inward, outward, still, and moving
    // for less than 1 s.
    let cases = [
        (-0.01, 1.0, 1),
        (0.01, 1.0, 2),
        (0.0, 0.0, 0),
        (0.01, 0.999, 0),
    ];
    for (speed, move_time, expected) in cases {
        let runner = &mut solver.runners[0];
        runner.lane.speed = speed;
        runner.lane.move_time = move_time;
        let actual = status_value(&StatusVar::IsMoveLane, &solver.runners[0]);
        assert_eq!(
            actual, expected,
            "lane speed {speed}, move time {move_time}"
        );
    }
}

#[test]
fn test_is_overtake_without_targets() {
    let solver = create_test_race_solver(vec![]);
    let actual = status_value(&StatusVar::IsOvertake, &solver.runners[0]);
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
        failed_checks: 0,
        style: EffectiveStyle::Late,
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
            .map(|var| status_value(&var, &solver.runners[0]));
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
    .map(|var| activation_value(&var, state, runner, None));
    assert_eq!(actual, [6, 1, 2, 3, 4, 5, 3]);
}

#[test]
fn test_is_activate_other_skill_detail_without_logic() {
    let solver = solver_with_activated_skill(1, 0b10);

    let var = ActivationVar::IsActivateOtherSkillDetail;
    let actual = activation_value(&var, &solver.state, &solver.runners[0], None);
    assert_eq!(actual, 0);
}

#[test]
fn test_compete_fight_count_by_duel_state() {
    let mut solver = create_test_race_solver(vec![]);

    // Still counts after leaving the duel.
    for (duel, expected) in [
        (Duel::Pending, 0),
        (Duel::start(800.0), 1),
        (Duel::Ended, 1),
    ] {
        solver.runners[0].duel = duel;
        let var = ActivationVar::CompeteFightCount;
        let actual = activation_value(&var, &solver.state, &solver.runners[0], None);
        assert_eq!(actual, expected, "{duel:?}");
    }
}

#[test]
fn test_running_style_counts() {
    let mut solver = RaceSolver::new(load_course(10101), race_params(), seeded_config(0));
    let styles = [
        EffectiveStyle::Runaway,
        EffectiveStyle::Front,
        EffectiveStyle::Pace,
        EffectiveStyle::Pace,
        EffectiveStyle::Late,
        EffectiveStyle::End,
    ];
    for style in styles {
        solver.add_runner(entrant(style));
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
        // Runaways count as Front Runners.
        (BaseStyle::Front, [1, 1, 2, 1, 1, 2, 22]),
        (BaseStyle::Pace, [2, 2, 1, 1, 1, 2, 22]),
        (BaseStyle::End, [4, 2, 2, 1, 0, 1, 11]),
    ];
    for (style, expected) in cases {
        let actual = vars.map(|var| running_style_value(&var, state, style));
        assert_eq!(actual, expected, "{style:?}");
    }
}

#[test]
fn test_style_temptation_counts() {
    let mut solver = create_team_race_solver();
    solver.state.rushed_by_team[0][BaseStyle::Front as usize] = 1;
    solver.state.rushed_by_team[0][BaseStyle::Pace as usize] = 2;
    solver.state.rushed_by_team[0][BaseStyle::End as usize] = 3;
    // The individual runner, rushed herself.
    solver.state.rushed_by_team[1][BaseStyle::Front as usize] = 1;

    let vars = [
        TemptationVar::RunningStyleTemptationCountNige,
        TemptationVar::RunningStyleTemptationCountSenko,
        TemptationVar::RunningStyleTemptationCountSashi,
        TemptationVar::RunningStyleTemptationCountOikomi,
        TemptationVar::RunningStyleTemptationOpponentCountNige,
        TemptationVar::RunningStyleTemptationOpponentCountSenko,
        TemptationVar::RunningStyleTemptationOpponentCountSashi,
        TemptationVar::RunningStyleTemptationOpponentCountOikomi,
    ];
    // Opponents exclude the runner's own team, and so the runner herself.
    let cases = [(0, [2, 2, 0, 3, 1, 0, 0, 0]), (1, [2, 2, 0, 3, 1, 2, 0, 3])];
    for (team, expected) in cases {
        let runner = team_runner(&solver, team);
        let actual = vars.map(|var| temptation_value(&var, &solver.state, runner));
        assert_eq!(actual, expected, "team {team}");
    }
}

#[test]
fn test_variable_value_dispatch() {
    // Values derived by `variable_value` itself: other variables are passed
    // to the function of their category, chosen by type.
    let mut solver = create_test_race_solver(vec![]);
    // The current style, not the base one.
    solver.runners[0].style = EffectiveStyle::End;
    solver.state.elapsed_time = 42.5;

    let (state, runner) = (&solver.state, &solver.runners[0]);
    let cases = [
        (VarType::RunningStyle(RunningStyleVar::RunningStyle), 4),
        (VarType::AccumulateTime, 42),
        (VarType::Always, 1),
    ];
    for (var, expected) in cases {
        let actual = int(variable_value(&var, state, runner, None));
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
        let actual = int(variable_value(
            &var,
            &solver.state,
            &solver.runners[0],
            None,
        ));
        assert_eq!(actual, expected, "{all_out_spurt:?}");
    }
}

/// Fresh race state and unready Late Surger, before the race starts.
fn create_out_of_gate_runner() -> (RaceState, UnreadyRunner) {
    let solver = RaceSolver::new(load_course(10101), race_params(), seeded_config(0));
    let state = solver.state;
    let mut runner = UnreadyRunner::new(
        uma(BaseStyle::Late),
        Mood::Good,
        &state,
        vec![],
        Pins::default(),
        RngSeed::new(0),
    );
    runner.assign_gate(1, &state);
    runner.base.speed = 1101.0;
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
        // Variables only known during the race are 0.
        ("hp_per==0", true),
        ("phase==0", true),
        ("order==0", true),
        ("corner==0", true),
        ("accumulatetime==0", true),
    ];
    for (condition, expected) in cases {
        let actual = EvaluatorOutOfGate.evaluate(&comp(condition), None, &state, &runner);
        assert_eq!(actual, expected, "{condition}");
    }
}

#[test]
fn test_same_skill_horse_count_value() {
    let skill = Arc::new(Skill {
        id: 200331,
        ..synthetic_skill(vec![])
    });
    let mut solver = create_test_race_solver(vec![skill]);
    solver.state.existing_skills.insert(200331, 3);

    let var = LineupVar::SameSkillHorseCount;
    assert_eq!(int(lineup_value(&var, &solver.state, 200331)), 3);
    // Skills nobody has, or no skill being evaluated.
    assert_eq!(int(lineup_value(&var, &solver.state, 200332)), 0);
    let var = VarType::Lineup(var);
    let actual = int(variable_value(
        &var,
        &solver.state,
        &solver.runners[0],
        None,
    ));
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
    variable_value(&var, &solver.state, &solver.runners[0], None);
}

#[test]
#[should_panic(expected = "it's a trigger variable")]
fn test_is_last_straight_onetime_value_panics() {
    let solver = create_test_race_solver(vec![]);
    let var = SpatialVar::IsLastStraightOnetime;
    location_value(&var, &solver.state.course, &solver.runners[0]);
}

#[test]
fn test_succession_skill_count() {
    let skill = |id: i32| {
        Arc::new(Skill {
            id,
            ..synthetic_skill(vec![])
        })
    };
    // Inherited uniques, a unique and a white skill.
    let skills = [900011, 900021, 100011, 200012].map(skill).to_vec();
    let solver = create_test_race_solver(skills);
    let actual = status_value(&StatusVar::SuccessionSkillCount, &solver.runners[0]);
    assert_eq!(actual, 2);
}

#[test]
#[should_panic(expected = "is_exist_skill_id doesn't support operator")]
fn test_set_values_reject_order_operators() {
    let solver = create_test_race_solver(vec![]);
    // Built directly, as the parser rejects it.
    let comp = Comparison {
        var: VarType::Lineup(LineupVar::IsExistSkillId),
        op: CompOp::Greater,
        value: 203061,
    };
    EvaluatorFull.evaluate(&comp, None, &solver.state, &solver.runners[0]);
}

#[test]
fn test_lottery_draws_are_hit_by_higher_chances() {
    // (draw, [==0, ==37, ==38, ==100]).
    let cases = [
        (0.0, [false, true, true, true]),
        (37.4, [false, false, true, true]),
        (99.9, [false, false, false, true]),
    ];
    for (draw, expected) in cases {
        let actual = [0, 37, 38, 100].map(|chance| {
            let comp = Comparison {
                var: VarType::Random(RandomVar::RandomLot),
                op: CompOp::Equal,
                value: chance,
            };
            comp.holds(lot_hits(draw))
        });
        assert_eq!(actual, expected, "draw {draw}");
    }
}
