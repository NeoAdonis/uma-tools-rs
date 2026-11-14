use super::*;
use crate::configuration::GameVersion;
use crate::race::charge_up::ChargeUpOutcome;

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
    let mut solver = ready_solver(10101, uma(BaseStyle::Late), vec![], seeded_config(0));
    let runner = &mut solver.runners[0];

    // (order, elapsed time, [best, worst]). The best and worst orders
    // are only tracked after 5 seconds.
    let cases = [
        (3, 5.0, [u8::MAX, 0]),
        (5, 5.1, [5, 5]),
        (2, 10.0, [2, 5]),
        (7, 10.0, [2, 7]),
        (9, 10.0, [2, 9]),
    ];
    for (order, elapsed_time, expected) in cases {
        runner.order.order = order;
        runner.update_order_rate(9, elapsed_time);
        let o = &runner.order;
        assert_eq!(o.runner_count, 9);
        assert_eq!([o.best, o.worst], expected, "order {order}");
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
    // Course distance covered in the last of `frames` frames from `position`
    // at `lane`, after a steady 2m move at the same lane, with `speed` before
    // the 0.5 m/s of current speed skills.
    let advance = |position: f64, lane: f64, speed: f64, frames: usize| {
        let lane = CourseWidths(lane);
        let mut solver = ready_solver_on(
            course.clone(),
            uma(BaseStyle::Late),
            vec![],
            seeded_config(0),
        );
        solver.state.elapsed_time = 10.0;
        let runner = &mut solver.runners[0];
        runner.position = position;
        // Current speed skills add to the speed.
        runner.speed = speed;
        runner.skill_modifiers.current_speed = 0.5;
        runner.lane = LaneState::new(lane);
        runner.last_move_start = Some(course.rail.world_position(position - 2.0, lane));
        let mut start = position;
        for _ in 0..frames {
            start = runner.position;
            runner.update_position(0.1, &solver.state);
        }
        runner.position - start
    };

    // Straight before the corner, then within it.
    assert_close!(advance(300.0, 1.0, 19.5, 1), 2.0);
    assert_close!(advance(600.0, 0.0, 19.5, 1), 2.0);
    let expected = 2.0 / (1.0 + 0.005 * 11.25);
    // Lanes turn between keyframes rather than evenly.
    assert_close!(advance(600.0, 1.0, 19.5, 1), expected, tolerance = 1e-5);
    // The slowed down move covers the base world distance, so the next one
    // isn't slowed down, and the one after it is again.
    assert_close!(advance(600.0, 1.0, 19.5, 2), 2.0, tolerance = 1e-5);
    assert_close!(advance(600.0, 1.0, 19.5, 3), expected, tolerance = 1e-5);
    // On a straight on the rail, losing speed slows down and gaining it doesn't
    // speed up.
    assert_close!(advance(300.0, 0.0, 18.5, 1), 1.9 * 1.9 / 2.0);
    assert_close!(advance(300.0, 0.0, 20.5, 1), 2.1);
}

#[test]
fn test_finish_time_is_interpolated_within_the_frame() {
    use crate::course::rail::rail_from_turns;
    use crate::race::solver::DELTA_TIME;
    use crate::test_utils::*;

    // Straight 2000m course, finished 1.5m after the start of the frame, at
    // 30 m/s (about 2m per frame).
    let course = Course {
        rail: rail_from_turns(1.0, 2000.0, &[0.0], &[0.0]),
        ..synthetic_course()
    };
    let lane = CourseWidths(0.0);
    let mut solver = ready_solver_on(
        course.clone(),
        uma(BaseStyle::Late),
        vec![],
        seeded_config(0),
    );
    solver.state.elapsed_time = 10.0;
    let runner = &mut solver.runners[0];
    runner.position = 1998.5;
    runner.speed = 30.0;
    runner.lane = LaneState::new(lane);
    // Steady move, so the frame isn't slowed down.
    let previous = runner.position - runner.speed * DELTA_TIME;
    runner.last_move_start = Some(course.rail.world_position(previous, lane));
    runner.update_position(DELTA_TIME, &solver.state);
    let expected = 10.0 - DELTA_TIME + 1.5 / 30.0;
    assert_close!(runner.finish_time.unwrap(), expected);
}

#[test]
fn test_start_dash_caps_speed_at_its_end() {
    use crate::race::solver::DELTA_TIME;
    use crate::test_utils::*;
    // 2000m course: the start dash ends at 0.85 * 20 m/s, below the minimum
    // speed with 800 guts (17.4 m/s).
    let uma = Uma {
        guts: 800,
        ..uma(BaseStyle::Pace)
    };
    let mut solver = ready_solver(10104, uma, vec![], seeded_config(0));
    let base_speed = solver.state.course.base_speed();
    let runner = &mut solver.runners[0];
    assert!(runner.is_start_dash);

    // Below the threshold, the start dash goes on, ignoring the minimum speed.
    runner.speed = 15.0;
    runner.acceleration = 24.5;
    runner.update_speed(DELTA_TIME, base_speed, None);
    assert!(runner.is_start_dash);
    assert_close!(runner.speed, 15.0 + 24.5 * DELTA_TIME);

    // Reaching it ends the start dash without carrying its acceleration over.
    runner.update_speed(DELTA_TIME, base_speed, None);
    assert!(!runner.is_start_dash);
    assert_eq!(runner.speed, 17.0);

    // Speed is then raised to the minimum speed.
    runner.acceleration = 0.4;
    runner.update_speed(DELTA_TIME, base_speed, None);
    assert_close!(runner.speed, 17.4);

    // A target speed below the threshold holds speed there, so the start dash
    // goes on, still below the minimum speed.
    runner.is_start_dash = true;
    (runner.speed, runner.acceleration) = (16.0, 24.5);
    runner.target_speed = 16.97;
    runner.update_speed(DELTA_TIME, base_speed, None);
    assert!(runner.is_start_dash);
    assert_eq!(runner.speed, 16.97);
}

#[test]
fn test_target_speed_scales_with_positioning_strategy() {
    use crate::test_utils::*;

    let mut solver = ready_solver(10101, uma(BaseStyle::Pace), vec![], seeded_config(42));
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

    let mut solver = ready_solver(10101, uma(BaseStyle::Pace), vec![], seeded_config(42));
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
    let time = state.elapsed_time;
    runner.secure_lead = TimedBoost::start(0.07, time, 2.0, 1.0);
    runner.compete_before_spurt = TimedBoost::start(0.11, time, 2.0, 1.0);
    runner.all_out_spurt = AllOutSpurt::Active { speed_bonus: 0.13 };
    runner.start_duel(0.0);
    let (guts, aptitude) = (runner.adjusted.guts, runner.raw.style_aptitude);
    runner.spot_struggle = SpotStruggle::start(guts, aptitude, time);
    let bonuses = runner.duel.speed_bonus() + runner.spot_struggle.speed_bonus(time);
    assert!(bonuses > 0.0);

    runner.update_target_speed(state);
    assert_close!(
        runner.target_speed - base,
        -0.1 + 0.2 + 0.03 + 0.05 + 0.07 + 0.11 + 0.13 + bonuses
    );
}

#[test]
fn test_pace_down_and_downhill_mode_save_hp() {
    use crate::race::solver::DELTA_TIME;
    use crate::test_utils::*;

    // (pace down, downhill mode, HP consumption multiplier).
    let cases = [
        (false, false, 1.0),
        (true, false, 0.6),
        (false, true, 0.4),
        (true, true, 0.24),
    ];
    let mut base = None;
    for (pace_down, downhill, multiplier) in cases {
        let mut solver = ready_solver(10101, uma(BaseStyle::Pace), vec![], seeded_config(42));
        let state = &solver.state;
        let runner = &mut solver.runners[0];
        runner.speed = 20.0;
        if pace_down {
            runner.positioning_strategy = PositioningStrategy::PaceDown(0.0);
        }
        if downhill {
            runner.slope_speed_modifier = Some(0.5);
        }
        let before = runner.hp.current();
        runner.update_hp(DELTA_TIME, false, state);
        let consumed = before - runner.hp.current();
        let base = *base.get_or_insert(consumed);
        assert_close!(
            consumed,
            base * multiplier,
            "pace down: {pace_down}, downhill: {downhill}"
        );
    }
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
            ..uma(BaseStyle::Pace)
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
fn test_section_speed_modifier_ignored_during_last_spurt() {
    use crate::race::last_spurt::{LastSpurt, LastSpurtCategory};
    use crate::test_utils::*;

    // (position, whether the section speed modifier applies), with a last
    // spurt from 1950m.
    for (position, applies) in [(1949.0, true), (1950.0, false)] {
        let mut solver = ready_solver(10105, uma(BaseStyle::Pace), vec![], seeded_config(42));
        let state = &solver.state;
        let runner = &mut solver.runners[0];
        runner.phase = Phase::Late;
        runner.position = position;
        // Below the last spurt speed, so that nothing else starts.
        runner.speed = 19.0;
        runner.last_spurt = Some(LastSpurt {
            max_speed: 20.0,
            target_distance: 1950.0,
            category: LastSpurtCategory::Full,
        });
        runner.section_speed_modifier = 0.0;
        runner.update_target_speed(state);
        let without = runner.target_speed;
        runner.section_speed_modifier = 0.1;
        runner.update_target_speed(state);
        let expected = if applies { 0.1 } else { 0.0 };
        assert_close!(
            runner.target_speed - without,
            expected,
            "position {position}"
        );
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
        (
            BaseStyle::Pace,
            Phase::Early,
            Aptitude::A,
            Aptitude::A,
            0.985,
        ),
        (
            BaseStyle::Front,
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
fn test_acceleration_adds_modifiers() {
    use crate::test_utils::*;

    let mut solver = ready_solver(10101, uma(BaseStyle::Pace), vec![], seeded_config(0));
    let runner = &mut solver.runners[0];
    runner.is_start_dash = false;
    runner.accelerate(0.0);
    let base = runner.acceleration;

    runner.skill_modifiers.acceleration = 0.2;
    runner.start_duel(0.0);
    let duel = runner.duel.acceleration_bonus();
    assert!(duel > 0.0);
    runner.accelerate(0.0);
    assert_close!(runner.acceleration - base, 0.2 + duel);
}

#[test]
fn test_deceleration_by_state() {
    use crate::test_utils::*;

    // (phase, pacing down, out of HP, deceleration).
    let cases = [
        (Phase::Early, false, false, -1.2),
        (Phase::Mid, false, false, -0.8),
        (Phase::Late, false, false, -1.0),
        (Phase::Mid, true, false, -0.5),
        (Phase::Mid, true, true, -1.2),
    ];
    for (phase, pace_down, out_of_hp, expected) in cases {
        let mut solver = ready_solver(10101, uma(BaseStyle::Pace), vec![], seeded_config(0));
        let runner = &mut solver.runners[0];
        runner.phase = phase;
        if pace_down {
            runner.positioning_strategy = PositioningStrategy::PaceDown(0.0);
        }
        if out_of_hp {
            runner.hp.set_current(0.0);
        }
        runner.decelerate();
        assert_eq!(
            runner.acceleration, expected,
            "{phase:?}, pacing down: {pace_down}, out of HP: {out_of_hp}"
        );
    }
}

#[test]
fn test_speed_stops_at_target_speed() {
    use crate::race::solver::DELTA_TIME;
    use crate::test_utils::*;

    let mut solver = ready_solver(10204, uma(BaseStyle::Late), vec![], seeded_config(42));
    let base_speed = solver.state.course.base_speed();
    let runner = &mut solver.runners[0];
    runner.is_start_dash = false;
    runner.min_speed = 0.0;
    // (speed, acceleration, target speed, expected speed).
    let cases = [
        (19.99, 1.0, 20.0, 20.0),
        (19.0, 1.0, 20.0, 19.0 + DELTA_TIME),
        (20.01, -1.0, 20.0, 20.0),
        (21.0, -1.0, 20.0, 21.0 - DELTA_TIME),
        // At the target speed, she decelerates, but stays there.
        (20.0, -1.0, 20.0, 20.0),
    ];
    for (speed, acceleration, target_speed, expected) in cases {
        (runner.speed, runner.acceleration) = (speed, acceleration);
        runner.target_speed = target_speed;
        runner.update_speed(DELTA_TIME, base_speed, None);
        assert_close!(runner.speed, expected, "from {speed} at {acceleration}");
    }

    // Target speed is capped, and so is speed with it.
    runner.skill_modifiers.target_speed = 100.0;
    runner.update_target_speed(&solver.state);
    let runner = &mut solver.runners[0];
    assert_eq!(runner.target_speed, MAX_TARGET_SPEED);
    (runner.speed, runner.acceleration) = (MAX_TARGET_SPEED - 0.01, 1.0);
    runner.update_speed(DELTA_TIME, base_speed, None);
    assert_eq!(runner.speed, MAX_TARGET_SPEED);
}

#[test]
fn test_target_speed_has_a_floor() {
    use crate::race::modifiers::uphill_speed_modifier;
    use crate::test_utils::*;

    let mut solver = ready_solver(10101, uma(BaseStyle::Pace), vec![], seeded_config(42));
    let state = &solver.state;
    let runner = &mut solver.runners[0];
    runner.slope_speed_modifier = Some(uphill_speed_modifier(1.0, 1.0));
    assert_eq!(runner.slope_speed_modifier, Some(-200.0));

    // The start dash floor is fixed, whatever the minimum speed.
    runner.is_start_dash = true;
    runner.update_target_speed(state);
    assert_eq!(runner.target_speed, START_DASH_MIN_TARGET_SPEED);

    runner.is_start_dash = false;
    runner.update_target_speed(state);
    assert_eq!(runner.target_speed, runner.min_speed);
    assert_ne!(runner.min_speed, START_DASH_MIN_TARGET_SPEED);
}

#[test]
fn test_rush_events_record_the_position_at_each_change() {
    use crate::race::events::EventEnd;
    use crate::race::rush::{self, RushState};
    use crate::race::solver::RaceSolver;
    use crate::test_utils::{ready_solver, seeded_config, uma};

    let mut solver = ready_solver(10101, uma(BaseStyle::Late), vec![], seeded_config(0));
    let RaceSolver { runners, state, .. } = &mut solver;
    let runner = &mut runners[0];
    runner.rush_state = Some(RushState {
        section: 3,
        start_time: 0.0,
        end_time: f64::MAX,
        max_duration: rush::MAX_DURATION,
        is_rushed: false,
        failed_checks: 0,
        style: EffectiveStyle::Pace,
    });
    let kind = EventKind::Rush {
        style: EffectiveStyle::Pace,
    };

    runner.position = 100.0;
    state.elapsed_time = 3.0;
    runner.start_rush(state);
    assert_eq!(runner.events(), [Event::start(kind, 3.0, 100.0)]);

    runner.position = 150.0;
    state.elapsed_time = 7.0;
    runner.end_rush(state);
    let end = EventEnd {
        time: 7.0,
        distance: 150.0,
    };
    let event = Event {
        end: Some(end),
        ..Event::start(kind, 3.0, 100.0)
    };
    assert_eq!(runner.events(), [event]);
}

#[test]
fn test_last_spurt_and_charge_up_release_events() {
    use crate::race::solver::RaceSolver;
    use crate::test_utils::{ready_solver, seeded_config, uma};

    // (power conserved, position the last spurt starts at). Releasing needs some
    // power, and both events wait for the spurt.
    for (conserved, target_distance) in [(500.0, 1000.0), (0.0, 1000.0), (500.0, 1000.1)] {
        let uma = Uma {
            power: 1400,
            ..uma(BaseStyle::Pace)
        };
        let mut solver = ready_solver(10810, uma, vec![], seeded_config(0));
        let RaceSolver { runners, state, .. } = &mut solver;
        let runner = &mut runners[0];
        let last_spurt = LastSpurt {
            max_speed: 20.0,
            target_distance,
            category: LastSpurtCategory::Full,
        };
        runner.last_spurt = Some(last_spurt);
        runner.charge_up.as_mut().unwrap().conserved = conserved;
        let name = format!("conserved {conserved}, spurt at {target_distance}");

        // Events the runner should have after a frame that updates her phase from
        // `position` at `time`.
        let expected = |runner: &Runner, time: f64, position: f64| {
            let mut events = vec![Event::start(EventKind::LastSpurt, time, position)];
            if conserved > 0.0 {
                let Some(ChargeUpOutcome::Released(release)) =
                    runner.charge_up.as_ref().map(|c| c.outcome.clone())
                else {
                    panic!("{name}: no release");
                };
                let kind = EventKind::ChargeUpRelease {
                    conserved: release.conserved,
                    acceleration: release.acceleration,
                };
                events.push(Event::start(kind, time, position));
            }
            events
        };

        // The runner already moved on to the next position, so the events are recorded
        // at the previous one.
        runner.position = 1010.0;
        state.elapsed_time = 50.0;
        runner.update_phase(1000.0, state);
        if target_distance > 1000.0 {
            assert!(runner.events().is_empty(), "{name}");
        } else {
            assert_eq!(runner.events(), expected(runner, 50.0, 1000.0), "{name}");
        }

        // Nothing is logged again, even if the last spurt is recalculated.
        runner.last_spurt = Some(LastSpurt {
            target_distance: 900.0,
            ..last_spurt
        });
        state.elapsed_time = 51.0;
        runner.update_phase(1010.0, state);
        runner.update_phase(1020.0, state);
        let (time, position) = if target_distance > 1000.0 {
            (51.0, 1010.0)
        } else {
            (50.0, 1000.0)
        };
        assert_eq!(runner.events(), expected(runner, time, position), "{name}");
    }
}
