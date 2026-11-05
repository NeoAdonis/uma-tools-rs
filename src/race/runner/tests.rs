use super::*;

#[test]
fn test_start_type_from_delay() {
    let cases = [
        (0.019, StartType::Good),
        (0.02, StartType::Normal),
        (0.08, StartType::Normal),
        (0.081, StartType::Late),
    ];
    for (delay, expected) in cases {
        assert_eq!(StartType::from_delay(delay), expected, "delay {delay}");
    }
}

#[test]
fn test_rates_at_order_round_to_nearest() {
    // 9 runners: 5% => 0.45, 6% => 0.54, 16% => 1.44, 17% => 1.53, 50% => 4.5,
    // 61% => 5.49, 100% => 9.
    // (runners, order, rates).
    let cases = [
        (9, 0, -5..=5),
        (9, 1, 6..=16),
        (9, 5, 50..=61),
        (9, 9, 95..=105),
        (12, 6, 46..=54),
        (1, 1, 50..=149),
    ];
    for (runners, order, expected) in cases {
        let actual = rates_at_order(order, runners);
        assert_eq!(actual, expected, "runners {runners}, order {order}");
    }
    // Every rate rounds to a single order.
    for order in 0..9 {
        let (current, next) = (rates_at_order(order, 9), rates_at_order(order + 1, 9));
        assert_eq!(current.end() + 1, *next.start(), "order {order}");
    }
}

#[test]
fn test_update_order_rate_tracks_best_and_worst_orders() {
    use crate::test_utils::*;
    let mut solver = ready_solver(10101, uma(Style::Late), vec![], seeded_config(0));
    let runner = &mut solver.runners[0];

    // (order, elapsed time, [rate, best, worst]). The best and worst orders
    // are only tracked after 5 seconds.
    let cases = [
        (3, 5.0, [34, u8::MAX, 0]),
        (5, 5.1, [56, 5, 5]),
        (2, 10.0, [23, 2, 5]),
        (7, 10.0, [78, 2, 7]),
        (9, 10.0, [100, 2, 9]),
    ];
    for (order, elapsed_time, expected) in cases {
        runner.order.order = order;
        runner.update_order_rate(9, elapsed_time);
        let o = &runner.order;
        assert_eq!(o.runner_count, 9);
        assert_eq!([o.rate, o.best, o.worst], expected, "order {order}");
        assert_eq!(o.rates, rates_at_order(order, 9), "order {order}");
        assert_eq!(o.best_rates, rates_at_order(o.best, 9), "order {order}");
        assert_eq!(o.worst_rates, rates_at_order(o.worst, 9), "order {order}");
    }
}

#[test]
fn test_update_position_slows_down_off_rail_in_corners() {
    use crate::course::rail::rail_from_turns;
    use crate::test_utils::*;
    // Synthetic 2000m course whose 2m segments from 500m to 800m (a corner)
    // turn by 0.01 radians.
    let turn: Vec<f64> = (0..1000)
        .map(|segment| {
            if (250..400).contains(&segment) {
                0.01
            } else {
                0.0
            }
        })
        .collect();
    let course = Course {
        rail: rail_from_turns(1.0, 2000.0, &[0.0; 1000], &turn),
        ..synthetic_course()
    };
    // Course distance covered in one frame from `position` at `lane`, after a
    // 1m move at the same lane.
    let advance = |position: f64, lane: f64| {
        let lane = CourseWidths(lane);
        let mut solver =
            ready_solver_on(course.clone(), uma(Style::Late), vec![], seeded_config(0));
        solver.state.elapsed_time = 10.0;
        let runner = &mut solver.runners[0];
        runner.position = position;
        runner.speed = 20.0;
        runner.lane = LaneState::new(lane);
        runner.last_move = LastMove {
            distance: 1.0,
            world: course.rail.world_position(position - 1.0, lane),
        };
        runner.update_position(0.1, &solver.state);
        runner.position - position
    };

    // Straight before the corner, then within it.
    assert_close!(advance(300.0, 1.0), 2.0);
    assert_close!(advance(600.0, 0.0), 2.0);
    let expected = 2.0 / (1.0 + 0.005 * 11.25);
    // Lanes turn between keyframes rather than evenly.
    assert_close!(advance(600.0, 1.0), expected, tolerance = 1e-5);
}

#[test]
fn test_start_dash_caps_speed_at_its_end() {
    use crate::race::solver::DELTA_TIME;
    use crate::test_utils::*;
    // 2000m course: the start dash ends at 0.85 * 20 m/s, below the minimum
    // speed with 800 guts (17.4 m/s).
    let uma = Uma {
        guts: 800,
        ..uma(Style::Pace)
    };
    let mut solver = ready_solver(10104, uma, vec![], seeded_config(0));
    let base_speed = solver.state.course.base_speed();
    let runner = &mut solver.runners[0];
    assert!(runner.is_start_dash);

    // Below the threshold, the start dash goes on, ignoring the minimum speed.
    runner.speed = 15.0;
    runner.acceleration = 24.5;
    runner.update_speed(DELTA_TIME, base_speed);
    assert!(runner.is_start_dash);
    assert_close!(runner.speed, 15.0 + 24.5 * DELTA_TIME);

    // Reaching it ends the start dash without carrying its acceleration over.
    runner.update_speed(DELTA_TIME, base_speed);
    assert!(!runner.is_start_dash);
    assert_eq!(runner.speed, 17.0);

    // Speed is then raised to the minimum speed.
    runner.acceleration = 0.4;
    runner.update_speed(DELTA_TIME, base_speed);
    assert_close!(runner.speed, 17.4);
}

#[test]
fn test_target_speed_scales_with_positioning_strategy() {
    use crate::test_utils::*;

    let mut solver = ready_solver(10101, uma(Style::Pace), vec![], seeded_config(42));
    let state = &solver.state;
    let runner = &mut solver.runners[0];
    // Added after scaling.
    runner.section_speed_modifier = 0.0;
    runner.update_target_speed(state);
    let normal = runner.target_speed;

    let strategy = PositioningStrategy::FrontOvertake;
    runner.positioning_strategy = strategy;
    runner.update_target_speed(state);
    let multiplier = strategy.speed_multiplier(runner.phase, GameVersion::LATEST);
    assert_close!(runner.target_speed, normal * multiplier);
}

#[test]
fn test_target_speed_adds_modifiers() {
    use crate::test_utils::*;

    let mut solver = ready_solver(10101, uma(Style::Pace), vec![], seeded_config(42));
    let state = &solver.state;
    let runner = &mut solver.runners[0];
    // Rolled when readying runners.
    runner.section_speed_modifier = 0.0;
    runner.update_target_speed(state);
    let base = runner.target_speed;

    // Distinct values, so that no term can make up for another.
    runner.slope_speed_modifier = Some(-0.1);
    runner.skill_modifiers.target_speed = 0.2;
    runner.section_speed_modifier = 0.03;
    runner.stamina_limit_break = 0.05;
    runner.start_duel();
    let (guts, aptitude) = (runner.adjusted.guts, runner.adjusted.style_aptitude);
    runner.spot_struggle = SpotStruggle::start(guts, aptitude, state.elapsed_time);
    let bonuses = runner.duel.speed_bonus() + runner.spot_struggle.speed_bonus(state.elapsed_time);
    assert!(bonuses > 0.0);

    runner.update_target_speed(state);
    assert_close!(
        runner.target_speed - base,
        -0.1 + 0.2 + 0.03 + 0.05 + bonuses
    );
}

#[test]
fn test_stamina_limit_break_activates_at_last_spurt_speed() {
    use crate::race::last_spurt::{LastSpurt, LastSpurtCategory};
    use crate::test_utils::*;

    // (current speed, whether the buff activates), with a last spurt speed of
    // 20 m/s.
    for (speed, activates) in [(20.0, true), (19.9, false)] {
        // A high-stamina runner on a long course.
        let uma = Uma {
            stamina: 1600,
            ..uma(Style::Pace)
        };
        let mut solver = ready_solver(10105, uma, vec![], seeded_config(42));
        let runner = &mut solver.runners[0];
        assert!(runner.stamina_limit_break_base > 0.0);

        runner.position = 2000.0;
        runner.speed = speed;
        runner.last_spurt = Some(LastSpurt {
            max_speed: 20.0,
            target_distance: 1950.0,
            category: LastSpurtCategory::Full,
        });
        runner.update_target_speed(&solver.state);
        let buff = runner.stamina_limit_break;
        let expected = if activates {
            runner.stamina_limit_break_base
        } else {
            0.0
        };
        assert_eq!(buff, expected, "speed {speed}");

        // The buff stays even if the last spurt is gone (e.g. HP recalculation).
        runner.last_spurt = None;
        runner.update_target_speed(&solver.state);
        assert_eq!(runner.stamina_limit_break, buff, "speed {speed}");
    }
}

#[test]
fn test_acceleration_multipliers() {
    use crate::test_utils::*;

    // 1000 power on flat ground: 0.0006 * sqrt(500 * 1000), times the style
    // and phase, track aptitude and distance aptitude multipliers.
    let base = 0.0006 * f64::sqrt(500.0 * 1000.0);
    // (style, phase, track aptitude, distance aptitude, multipliers).
    let cases = [
        (Style::Pace, Phase::Early, Aptitude::A, Aptitude::A, 0.985),
        (
            Style::Front,
            Phase::Late,
            Aptitude::B,
            Aptitude::E,
            0.996 * 0.9 * 0.6,
        ),
    ];
    for (style, phase, track_aptitude, distance_aptitude, multipliers) in cases {
        let uma = Uma {
            track_aptitude,
            distance_aptitude,
            ..uma(style)
        };
        let mut solver = ready_solver(10101, uma, vec![], seeded_config(0));
        let runner = &mut solver.runners[0];
        runner.is_start_dash = false;
        runner.phase = phase;
        runner.accelerate(0.0);
        assert_close!(
            runner.acceleration,
            base * multipliers,
            "{style:?} {phase:?}"
        );
    }
}

#[test]
fn test_speed_is_capped() {
    use crate::race::solver::DELTA_TIME;
    use crate::test_utils::*;

    let mut solver = ready_solver(10204, uma(Style::Late), vec![], seeded_config(42));
    let runner = &mut solver.runners[0];
    runner.is_start_dash = false;
    runner.speed = MAX_TARGET_SPEED - 0.01;
    runner.acceleration = 1.0;
    runner.update_speed(DELTA_TIME, solver.state.course.base_speed());
    assert_eq!(runner.speed, MAX_TARGET_SPEED);
}
