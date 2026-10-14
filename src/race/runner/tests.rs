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
fn test_order_at_rate_rounds_to_nearest() {
    let order = OrderState {
        runner_count: 9,
        ..Default::default()
    };
    // 9 runners: 20% => 1.8, 40% => 3.6, 50% => 4.5, 70% => 6.3, 80% => 7.2.
    let actual = [0, 20, 40, 50, 70, 80, 100].map(|rate| order.order_at_rate(rate));
    assert_eq!(actual, [0, 2, 4, 5, 6, 7, 9]);
}

#[test]
fn test_update_order_rate_tracks_best_and_worst_orders() {
    use crate::test_utils::*;
    let mut solver = ready_solver(10101, uma(Style::Late), vec![], &seeded_config(0));
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
    }
}
