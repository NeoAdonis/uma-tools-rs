use super::*;

#[test]
fn test_start_type_from_delay() {
    let cases = [
        (0.0, StartType::Good),
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
    let cases = [(0, -5..=5), (1, 6..=16), (5, 50..=61), (9, 95..=105)];
    for (order, expected) in cases {
        assert_eq!(rates_at_order(order, 9), expected, "order {order}");
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

    // (order, should update range, [rate, best, worst]).
    let cases = [
        (3, false, [34, u8::MAX, 0]),
        (5, true, [56, 5, 5]),
        (2, true, [23, 2, 5]),
        (7, true, [78, 2, 7]),
        (9, false, [100, 2, 7]),
    ];
    for (order, should_update_range, expected) in cases {
        runner.order.order = order;
        runner.update_order_rate(9, should_update_range);
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
    use crate::test_utils::*;
    // Course distance covered in one frame from `position` at `lane`, after a
    // 1m move at the same lane.
    let advance = |position: f64, lane: f64| {
        let lane = CourseWidths(lane);
        let mut solver = ready_solver(10101, uma(Style::Late), vec![], seeded_config(0));
        solver.state.elapsed_time = 10.0;
        let runner = &mut solver.runners[0];
        runner.position = position;
        runner.speed = 20.0;
        runner.lane = LaneState::new(lane);
        runner.last_move = LastMove {
            distance: 1.0,
            lane,
        };
        runner.segments.update(position, &solver.state.course);
        runner.update_position(0.1, &solver.state);
        runner.position - position
    };

    // Straight before the first corner (400m), then within it.
    assert_eq!(advance(300.0, 1.0), 2.0);
    assert_eq!(advance(500.0, 0.0), 2.0);
    let corner = &load_course(10101).corners[0];
    let expected = 2.0 / (1.0 + corner.curvature() * 11.25);
    assert!((advance(500.0, 1.0) - expected).abs() < 1e-9);
}
