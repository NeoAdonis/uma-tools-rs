use super::*;
use crate::race::solver::{DELTA_TIME, Entrant, RaceSolver};
use crate::test_utils::{assert_close, load_course, race_params, seeded_config, uma};
use crate::uma::*;
use crate::units::CourseWidths;

/// Solver with Mid race runners at the given `(position, lane)` pairs, in
/// order.
fn lane_solver(runners: &[(f64, f64)]) -> RaceSolver<Runner> {
    let mut solver = RaceSolver::new(load_course(10101), race_params(), seeded_config(0));
    for _ in runners {
        solver.add_runner(Entrant::new(uma(Style::Pace), vec![]));
    }
    let mut solver = solver.ready_runners();
    for (runner, &(position, lane)) in solver.runners.iter_mut().zip(runners) {
        runner.position = position;
        runner.lane = LaneState::new(CourseWidths(lane));
        runner.phase = Phase::Mid;
    }
    solver
}

/// One horse lane in course widths.
const LANE: f64 = CourseWidths::from_horse_lanes(1.0).0;

#[test]
fn test_extra_lane_candidate_window() {
    // 1 horse lane inward during the Mid race, checked up to 3m past the
    // furthest target.
    let lane = 0.3 - LANE;
    let mut solver = lane_solver(&[(100.0, 0.3), (105.0, 0.6), (107.9, lane)]);
    let extra = |solver: &RaceSolver<Runner>, with_target: bool| {
        let targets = if with_target {
            vec![&solver.runners[1]]
        } else {
            vec![]
        };
        let runner = &solver.runners[0];
        runner.extra_lane_candidate(targets, &runner.visible_ahead(&solver.runners))
    };
    assert_eq!(extra(&solver, true), None);
    solver.runners[2].position = 108.1;
    assert_eq!(extra(&solver, true), Some(CourseWidths(lane)));

    // Without targets, checked up to 3m past the runner herself.
    solver.runners[2].position = 102.9;
    assert_eq!(extra(&solver, false), None);
    solver.runners[2].position = 103.1;
    assert_eq!(extra(&solver, false), Some(CourseWidths(lane)));
}

#[test]
fn test_target_lane_candidates_use_boundary_uma_distance() {
    // The target is the inner boundary of her crowd, and the uma 2.5m ahead of
    // her is the outer one.
    let outer = 0.4 + 1.5 * LANE;
    let solver = lane_solver(&[
        (100.0, 0.4),
        (105.0, 0.4),
        (107.5, outer),
        // Blocks 1 horse lane outside the outer boundary, which is checked up to
        // 0.5m past the outer boundary uma, not the target.
        (107.8, outer + LANE),
    ]);
    let runner = &solver.runners[0];
    let visible_ahead = runner.visible_ahead(&solver.runners);
    let candidates: Vec<CourseWidths> = runner
        .target_lane_candidates(&solver.runners[1], &solver.runners, &visible_ahead)
        .collect();
    // 1 horse lane inside the outer boundary is blocked by the target herself.
    let expected = [CourseWidths(0.4 - LANE), CourseWidths(0.4 + LANE)];
    assert_eq!(candidates, expected);
}

#[test]
fn test_only_visible_runners_block_lane_candidates() {
    // At 1m, the field of view is less than 1.3 horse lanes wide to each side.
    let mut solver = lane_solver(&[(100.0, 0.3), (101.0, 0.3 + 3.0 * LANE)]);
    let is_valid = |solver: &RaceSolver<Runner>, lane| {
        let runner = &solver.runners[0];
        runner.is_lane_candidate_valid(
            CourseWidths(lane),
            5.0,
            &runner.visible_ahead(&solver.runners),
        )
    };
    assert!(is_valid(&solver, 0.3 + 3.0 * LANE));
    solver.runners[1].lane.current = CourseWidths(0.3 + LANE);
    assert!(!is_valid(&solver, 0.3 + LANE));
}

#[test]
fn test_overtake_lane_falls_back_to_current_lane() {
    // The only candidate (1 horse lane inward) is blocked, and nobody is ahead
    // of the blocker.
    let mut solver = lane_solver(&[(100.0, 0.3), (102.0, 0.3 - LANE), (50.0, 0.5)]);
    let runner = &mut solver.runners[0];
    runner.overtake_state = Some(OvertakeState::default());
    runner.lane.target = CourseWidths(0.5);
    let decide = |solver: &RaceSolver<Runner>| {
        solver.runners[0].overtake_lane_target(&solver.state.course, &solver.runners)
    };
    assert_close!(decide(&solver).0, 0.3);

    // The extra move lane, outside, is still preferred over the current lane.
    solver.runners[0].extra_move_lane = Some(CourseWidths(0.5));
    assert_close!(decide(&solver).0, 0.5);
}

#[test]
fn test_overtake_lane_ignores_runners_ahead_before_the_final_corner() {
    // A runner far ahead on the outside doesn't pull her out: the extra move
    // lane is only set upon entering the final corner.
    let mut solver = lane_solver(&[(100.0, 0.3), (115.0, 0.5)]);
    solver.runners[0].overtake_state = Some(OvertakeState::default());
    let target = solver.runners[0].overtake_lane_target(&solver.state.course, &solver.runners);
    assert_close!(target.0, 0.3 - LANE);
    assert_eq!(solver.runners[0].extra_move_lane, None);
}

#[test]
fn test_crowd_only_includes_umas_ahead_of_the_target() {
    let solver = lane_solver(&[
        (100.0, 0.4),
        (105.0, 0.4),
        // Behind the target: not in her crowd.
        (104.0, 0.4 + 1.5 * LANE),
        // Ahead of the target: the inner boundary of her crowd.
        (106.0, 0.4 - 1.5 * LANE),
    ]);
    let runners = &solver.runners;
    let (inner, outer) = runners[0].find_crowd_boundaries(&runners[1], runners);
    assert_eq!(inner.gate_roll, runners[3].gate_roll);
    assert_eq!(outer.gate_roll, runners[1].gate_roll);
}

#[test]
fn test_lane_space_follows_side_blocking() {
    let mut solver = lane_solver(&[(100.0, 0.4)]);
    let runner = &mut solver.runners[0];
    runner.blocked.side_out = true;
    let has_lane_space_to = |runner: &Runner, lane| runner.has_lane_space_to(CourseWidths(lane));
    assert!(!has_lane_space_to(runner, 0.5));
    assert!(has_lane_space_to(runner, 0.3));
    assert!(has_lane_space_to(runner, 0.4));

    runner.blocked.side_out = false;
    runner.blocked.side_in = true;
    assert!(has_lane_space_to(runner, 0.5));
    assert!(!has_lane_space_to(runner, 0.3));
}

#[test]
fn test_move_time_counts_lateral_movement_in_one_direction() {
    let mut solver = lane_solver(&[(100.0, 0.4)]);
    let course = solver.state.course.clone();
    let runner = &mut solver.runners[0];
    let step = |runner: &mut Runner, target| {
        runner.lane.target = CourseWidths(target);
        runner.update_lane(&course, DELTA_TIME);
        runner.lane.move_time
    };
    assert_close!(step(runner, 0.5), DELTA_TIME);
    assert_close!(step(runner, 0.5), 2.0 * DELTA_TIME);

    // Turning back restarts it.
    assert_close!(step(runner, 0.3), DELTA_TIME);

    // Blocked towards the target: not moving.
    runner.blocked.side_in = true;
    assert_eq!(step(runner, 0.3), 0.0);
}

#[test]
fn test_target_lane_updates_when_close_or_blocked_towards_it() {
    let mut solver = lane_solver(&[(100.0, 0.4)]);
    let runner = &mut solver.runners[0];
    runner.lane.target = CourseWidths(0.4 + 0.4 * LANE);
    assert!(runner.should_update_target_lane());

    // Far from the target lane: only updated if blocked towards it.
    runner.lane.target = CourseWidths(0.5);
    assert!(!runner.should_update_target_lane());
    runner.blocked.side_in = true;
    assert!(!runner.should_update_target_lane());
    runner.blocked.side_out = true;
    assert!(runner.should_update_target_lane());
}

#[test]
fn test_normal_mode_extra_move_lane_from_final_corner() {
    let mut solver = lane_solver(&[(100.0, 0.3)]);
    let final_corner = solver.state.course.corners.last().unwrap().start;
    let target = |solver: &RaceSolver<Runner>| {
        solver.runners[0].normal_mode_lane_target(&solver.state.course, &solver.runners)
    };
    let runner = &mut solver.runners[0];
    runner.phase = Phase::Late;
    runner.extra_move_lane = Some(CourseWidths(0.5));

    // Otherwise, keep straight.
    runner.position = final_corner - 1.0;
    runner.lane.target = CourseWidths(0.6);
    assert_close!(target(&solver).0, 0.3);

    // In the final corner, not only on the final straight.
    solver.runners[0].position = final_corner + 1.0;
    assert_close!(target(&solver).0, 0.5);
}

#[test]
fn test_normal_mode_moves_inward_up_to_the_rail() {
    let mut solver = lane_solver(&[(100.0, 0.02)]);
    let course = solver.state.course.clone();
    let runner = &mut solver.runners[0];
    // The target may be inside the rail...
    let target = runner.normal_mode_lane_target(&course, &[]);
    assert_close!(target.0, 0.02 - 0.05);

    // ...but the runner stops at it.
    runner.lane.target = target;
    for _ in 0..60 {
        runner.update_lane(&course, DELTA_TIME);
        assert!(runner.lane.current >= CourseWidths(0.0));
    }
    assert_eq!(runner.lane.current, CourseWidths(0.0));
    assert_eq!(runner.lane.speed, 0.0);

    // Once at the rail, the target is updated as if blocked towards it, and
    // she keeps straight.
    assert!(runner.should_update_target_lane());
    let target = runner.normal_mode_lane_target(&course, &[]);
    assert_eq!(target, CourseWidths(0.0));
}

#[test]
fn test_normal_mode_moves_away_from_inside_uma_even_if_blocked() {
    let mut solver = lane_solver(&[(100.0, 0.3), (100.5, 0.3 - LANE)]);
    let runner = &mut solver.runners[0];
    runner.blocked.side_in = true;
    runner.blocked.side_out = true;
    let target = solver.runners[0].normal_mode_lane_target(&solver.state.course, &solver.runners);
    assert_close!(target.0, 0.3 - LANE + 2.0 * LANE);
}

#[test]
fn test_overtake_lane_ignores_blocking_on_the_other_side() {
    // Blocked towards the outside, but free to move 1 horse lane inward.
    let mut solver = lane_solver(&[(100.0, 0.3), (50.0, 0.5)]);
    let runner = &mut solver.runners[0];
    runner.overtake_state = Some(OvertakeState::default());
    runner.blocked.side_out = true;
    let target = solver.runners[0].overtake_lane_target(&solver.state.course, &solver.runners);
    assert_close!(target.0, 0.3 - LANE);
}

#[test]
fn test_fixed_mode_unless_blocked_towards_skill_lane() {
    let mut solver = lane_solver(&[(100.0, 0.4)]);
    let runner = &mut solver.runners[0];
    assert_eq!(runner.fixed_mode_lane_target(), None);

    runner.skill_modifiers.target_lane = Some(CourseWidths(0.5));
    assert_eq!(runner.fixed_mode_lane_target(), Some(CourseWidths(0.5)));
    runner.blocked.side_in = true;
    assert_eq!(runner.fixed_mode_lane_target(), Some(CourseWidths(0.5)));
    runner.blocked.side_out = true;
    assert_eq!(runner.fixed_mode_lane_target(), None);
}

#[test]
fn test_extra_move_lane_set_upon_entering_final_corner() {
    let mut solver = lane_solver(&[(100.0, 0.05)]);
    let course = solver.state.course.clone();
    let entering = |corner: usize, straight: usize| Transitions {
        corner: Transition {
            exited: None,
            entered: Some(corner),
        },
        straight: Transition {
            exited: None,
            entered: Some(straight),
        },
        ..Default::default()
    };
    let (last_corner, last_straight) = (course.corners.len() - 1, course.straights.len() - 1);
    let runner = &mut solver.runners[0];

    // Not the final corner, nor the final straight before the 1st Anniv.
    runner.update_final_extra_move_lane(&entering(0, last_straight), &course, GameVersion::LATEST);
    let original = GameVersion::Original;
    runner.update_final_extra_move_lane(&entering(last_corner, 0), &course, original);
    assert_eq!(runner.extra_move_lane, None);

    // 0.05 / 0.1 * 0.5, plus up to 0.1.
    runner.update_final_extra_move_lane(&entering(last_corner, 0), &course, GameVersion::LATEST);
    let extra = runner.extra_move_lane.unwrap().0;
    assert!((0.25..0.35).contains(&extra), "{extra}");

    // Clamped beyond 0.1 course widths, and set on the final straight before
    // the 1st Anniv.
    runner.lane.current = CourseWidths(0.3);
    runner.update_final_extra_move_lane(&entering(0, last_straight), &course, original);
    let extra = runner.extra_move_lane.unwrap().0;
    assert!((0.5..0.6).contains(&extra), "{extra}");
}

#[test]
fn test_target_lane_decision_only_when_target_lane_should_update() {
    let mut solver = lane_solver(&[(100.0, 0.3)]);
    let decide = |solver: &RaceSolver<Runner>| {
        solver.runners[0].target_lane_decision(&solver.state.course, &solver.runners)
    };
    assert!(decide(&solver).is_some());
    solver.runners[0].lane.target = CourseWidths(0.5);
    assert!(decide(&solver).is_none());
}

#[test]
fn test_target_lane_decision_fixed_mode_overrides_overtake_mode() {
    let mut solver = lane_solver(&[(100.0, 0.3)]);
    let runner = &mut solver.runners[0];
    runner.overtake_state = Some(OvertakeState::default());
    runner.skill_modifiers.target_lane = Some(CourseWidths(0.6));
    runner.extra_move_lane = Some(CourseWidths(0.45));
    let target = solver.runners[0].target_lane_decision(&solver.state.course, &solver.runners);
    assert_eq!(target, Some(CourseWidths(0.6)));
}

#[test]
fn test_target_lane_decision_mode_selection() {
    // Alone in the Mid race, normal mode moves 0.05 course widths inward, and
    // overtake mode 1 horse lane inward.
    let normal = 0.3 - 0.05;
    let overtake = 0.3 - LANE;
    let mut solver = lane_solver(&[(100.0, 0.3)]);
    let move_lane_point = solver.state.course.move_lane_point().unwrap();
    let target = |solver: &mut RaceSolver<Runner>, position| {
        solver.runners[0].position = position;
        let decision =
            solver.runners[0].target_lane_decision(&solver.state.course, &solver.runners);
        decision.unwrap().0
    };

    // Not in overtake mode.
    assert_close!(target(&mut solver, move_lane_point + 500.0), normal);

    // In overtake mode, normal mode is still used up to 200m before the move
    // lane point.
    solver.runners[0].overtake_state = Some(OvertakeState::default());
    assert_close!(target(&mut solver, move_lane_point - 200.1), overtake);
    assert_close!(target(&mut solver, move_lane_point - 200.0), normal);
    assert_close!(target(&mut solver, move_lane_point), normal);
    assert_close!(target(&mut solver, move_lane_point + 0.1), overtake);

    // Only during the Early and Mid race. In the Late race, overtake mode moves
    // to the extra move lane on the outside, while normal mode keeps straight.
    let runner = &mut solver.runners[0];
    runner.phase = Phase::Late;
    runner.extra_move_lane = Some(CourseWidths(0.5));
    assert_close!(target(&mut solver, move_lane_point - 100.0), 0.5);
    solver.runners[0].overtake_state = None;
    assert_close!(target(&mut solver, move_lane_point - 100.0), 0.3);
}

#[test]
fn test_entering_overtake_mode_decides_the_target_lane() {
    // Far from her target and free to move, so it shouldn't be updated, until
    // she enters overtake mode behind a blocker in her lane. In the Early
    // race, the candidate 1 horse lane inside her is the cheapest.
    let mut solver = lane_solver(&[(0.0, 0.3), (0.0, 0.3)]);
    let move_lane_point = solver.state.course.move_lane_point().unwrap();
    let decide = |solver: &RaceSolver<Runner>| {
        solver.runners[0].target_lane_decision(&solver.state.course, &solver.runners)
    };
    let enter = |solver: &mut RaceSolver<Runner>, position: f64| {
        solver.runners[0].position = position;
        solver.runners[1].position = position + 1.5;
        let blocker = solver.runners[1].gate_roll;
        let runner = &mut solver.runners[0];
        runner.phase = Phase::Early;
        runner.overtake_state = None;
        runner.lane.target = CourseWidths(0.5);
        runner.blocked.side_in = false;
        runner.blocked.side_out = false;
        runner.blocked.front_blocker = Some(blocker);
        runner.track_front_blocker_target(DELTA_TIME);
    };

    enter(&mut solver, move_lane_point + 100.0);
    assert_close!(decide(&solver).unwrap().0, 0.3 - LANE);
    // Only on the update she entered it.
    solver.runners[0].reset_overtake_checks();
    assert_eq!(decide(&solver), None);

    // Not while normal mode is still used before the move lane point.
    enter(&mut solver, move_lane_point - 100.0);
    assert_eq!(decide(&solver), None);
}
