use crate::race::runner::Runner;
use crate::race::solver::{DELTA_TIME, RaceSolver};
use crate::test_utils::{load_course, race_params, seeded_config, uma};
use crate::uma::*;

fn ready_runners(count: usize) -> Vec<Runner> {
    let course = load_course(10101);
    let params = race_params();
    let mut solver = RaceSolver::new(course, params, seeded_config(42));
    for _ in 0..count {
        solver.add_runner(uma(Style::Pace), Mood::Normal, vec![]);
    }
    solver.ready_runners().runners
}

/// Simulate one blocking update cycle between a runner ahead and a runner
/// behind.
fn update_cycle(ahead: &mut Runner, behind: &mut Runner, is_near: bool) {
    ahead.reset_blocking_status();
    behind.reset_blocking_status();
    if is_near {
        behind.update_near_same_lane_by(ahead);
        ahead.nearby.behind_set1_count += 1;
    }
    ahead.update_near_time(DELTA_TIME);
    behind.update_near_time(DELTA_TIME);
}

#[test]
fn test_near_counts_do_not_accumulate_across_cycles() {
    let mut runners = ready_runners(2);
    let (left, right) = runners.split_at_mut(1);
    let (ahead, behind) = (&mut left[0], &mut right[0]);

    // Well beyond u8::MAX cycles; would overflow if counts were never reset
    for _ in 0..1000 {
        update_cycle(ahead, behind, true);
        assert_eq!(behind.nearby.ahead_same_lane_count, 1);
        assert_eq!(ahead.nearby.behind_same_lane_count, 1);
        assert_eq!(ahead.nearby.behind_set1_count, 1);
    }
}

#[test]
fn test_near_times_reset_after_separation() {
    let mut runners = ready_runners(2);
    let (left, right) = runners.split_at_mut(1);
    let (ahead, behind) = (&mut left[0], &mut right[0]);

    for _ in 0..30 {
        update_cycle(ahead, behind, true);
    }
    assert!(behind.nearby.ahead_same_lane_time > 1.9);
    assert!(ahead.nearby.behind_same_lane_time > 1.9);
    assert!(ahead.nearby.behind_set1_time > 1.9);

    update_cycle(ahead, behind, false);
    assert_eq!(behind.nearby.ahead_same_lane_time, 0.0);
    assert_eq!(ahead.nearby.behind_same_lane_time, 0.0);
    assert_eq!(ahead.nearby.behind_set1_time, 0.0);
}

#[test]
fn test_near_times_reset_on_order_change() {
    let mut runners = ready_runners(2);
    let (left, right) = runners.split_at_mut(1);
    let (ahead, behind) = (&mut left[0], &mut right[0]);

    for _ in 0..30 {
        update_cycle(ahead, behind, true);
    }
    // Still near, but each runner's order changed this frame.
    ahead.order.delta = -1;
    behind.order.delta = 1;
    update_cycle(ahead, behind, true);
    assert_eq!(behind.nearby.ahead_same_lane_time, 0.0);
    assert_eq!(ahead.nearby.behind_same_lane_time, 0.0);
    assert_eq!(ahead.nearby.behind_set1_time, 0.0);

    // They start counting again once the order holds.
    ahead.order.delta = 0;
    behind.order.delta = 0;
    update_cycle(ahead, behind, true);
    assert_eq!(behind.nearby.ahead_same_lane_time, DELTA_TIME);
    assert_eq!(ahead.nearby.behind_set1_time, DELTA_TIME);
}
