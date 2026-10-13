use super::*;
use crate::race::solver::RaceSolver;
use crate::test_utils::{assert_close, effect, load_course, race_params, seeded_config, uma};

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
fn test_positioning_speed_multiplier() {
    use PositioningStrategy::*;
    let multiplier =
        |strategy: PositioningStrategy, phase, version| strategy.speed_multiplier(phase, version);
    let latest = GameVersion::LATEST;
    for phase in [Phase::Early, Phase::Mid] {
        assert_eq!(multiplier(Normal, phase, latest), 1.0);
        assert_eq!(multiplier(FrontSpeedUp, phase, latest), 1.04);
        assert_eq!(multiplier(FrontOvertake, phase, latest), 1.05);
        assert_eq!(multiplier(PaceUp(5.0), phase, latest), 1.04);
        assert_eq!(multiplier(PaceUpEx, phase, latest), 2.0);
    }

    // Pace down is milder in the Mid race from 1.5 Anniv. onward.
    let cases = [
        (Phase::Early, GameVersion::FirstAnniversary, 0.915),
        (Phase::Mid, GameVersion::FirstAnniversary, 0.915),
        (Phase::Early, GameVersion::OnePointFiveAnniversary, 0.915),
        (Phase::Mid, GameVersion::OnePointFiveAnniversary, 0.945),
    ];
    for (phase, version, expected) in cases {
        let actual = multiplier(PaceDown(5.0), phase, version);
        assert_eq!(actual, expected, "{phase:?} {version:?}");
    }
}

fn timed(skill_type: SkillType, value: f64) -> ActiveTimedEffect {
    ActiveTimedEffect {
        effect: effect(skill_type, value),
        value,
        expires_at: 10.0,
        overtake_extension: OvertakeExtension::default(),
        additional_triggers: 0,
    }
}

#[test]
fn test_skill_modifiers_from_effects() {
    let mods = SkillModifiers::from_effects(&[]);
    assert_eq!(mods.target_speed, 0.0);
    assert_eq!(mods.target_lane, None);
    assert_eq!(mods.duration_scaling, 1.0);

    let effects = [
        timed(SkillType::TargetSpeed, 0.15),
        timed(SkillType::TargetSpeed, 0.25),
        timed(SkillType::CurrentSpeed, 0.1),
        timed(SkillType::CurrentSpeedWithNaturalDeceleration, 0.05),
        timed(SkillType::Acceleration, 0.2),
        timed(SkillType::AccelerationAllOutSpurt, 0.3),
        timed(SkillType::LaneMovementSpeed, 0.02),
        timed(SkillType::Fov, -5.0),
        timed(SkillType::LaneChange, 0.5),
        timed(SkillType::ExtendEvolvedDuration, 1.5),
        timed(SkillType::ExtendEvolvedDuration, 2.0),
        // Not a modifier.
        timed(SkillType::Recovery, 0.05),
    ];
    let mods = SkillModifiers::from_effects(&effects);
    assert_close!(mods.target_speed, 0.4);
    assert_close!(mods.current_speed, 0.15);
    assert_close!(mods.acceleration, 0.2);
    assert_close!(mods.all_out_spurt_acceleration, 0.3);
    assert_close!(mods.lane_speed, 0.02);
    assert_close!(mods.visible_distance, -5.0);
    assert_eq!(mods.target_lane, Some(0.5));
    // Multiplicative.
    assert_close!(mods.duration_scaling, 3.0);
}

/// Solver with Mid race runners at the given `(position, lane)` pairs, in
/// order.
fn lane_solver(runners: &[(f64, f64)]) -> RaceSolver<Runner> {
    let mut solver = RaceSolver::new(load_course(10101), race_params(), &seeded_config(0));
    for _ in runners {
        solver.add_runner(uma(Style::Pace), Mood::Normal, vec![]);
    }
    let mut solver = solver.ready_runners();
    for (runner, &(position, lane)) in solver.runners.iter_mut().zip(runners) {
        runner.position = position;
        runner.lane = LaneState::new(lane);
        runner.phase = Phase::Mid;
    }
    solver
}

/// One horse lane in course widths.
const LANE: f64 = 1.0 / crate::units::HORSE_LANES_PER_COURSE_WIDTH;

#[test]
fn test_extra_lane_candidate_window() {
    // 1 horse lane inward during the Mid race, checked up to 3m past the
    // furthest target.
    let lane = 0.3 - horse_lanes_to_course_widths(1.0);
    let mut solver = lane_solver(&[(100.0, 0.3), (105.0, 0.6), (107.9, lane)]);
    let extra = |solver: &RaceSolver<Runner>, with_target: bool| {
        let targets = if with_target {
            vec![&solver.runners[1]]
        } else {
            vec![]
        };
        solver.runners[0].extra_lane_candidate(&targets, &solver.runners)
    };
    assert_eq!(extra(&solver, true), None);
    solver.runners[2].position = 108.1;
    assert_eq!(extra(&solver, true), Some(lane));

    // Without targets, checked up to 3m past the runner herself.
    solver.runners[2].position = 102.9;
    assert_eq!(extra(&solver, false), None);
    solver.runners[2].position = 103.1;
    assert_eq!(extra(&solver, false), Some(lane));
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
    let candidates: Vec<f64> = runner
        .target_lane_candidates(&solver.runners[1], &solver.runners)
        .collect();
    // 1 horse lane inside the outer boundary is blocked by the target herself.
    let expected = [
        0.4 - horse_lanes_to_course_widths(1.0),
        0.4 + horse_lanes_to_course_widths(1.0),
    ];
    assert_eq!(candidates, expected);
}

#[test]
fn test_only_visible_runners_block_lane_candidates() {
    // At 1m, the field of view is less than 1.3 horse lanes wide to each side.
    let mut solver = lane_solver(&[(100.0, 0.3), (101.0, 0.3 + 3.0 * LANE)]);
    let is_valid = |solver: &RaceSolver<Runner>, lane| {
        solver.runners[0].is_lane_candidate_valid(lane, 5.0, &solver.runners)
    };
    assert!(is_valid(&solver, 0.3 + 3.0 * LANE));
    solver.runners[1].lane.current = 0.3 + LANE;
    assert!(!is_valid(&solver, 0.3 + LANE));
}

#[test]
fn test_overtake_lane_falls_back_to_current_lane() {
    // The only candidate (1 horse lane inward) is blocked, and nobody is ahead
    // of the blocker.
    let mut solver = lane_solver(&[(100.0, 0.3), (102.0, 0.3 - LANE), (50.0, 0.5)]);
    let runner = &mut solver.runners[0];
    runner.overtake_state = Some(OvertakeState::default());
    runner.lane.target = 0.5;
    let decide = |solver: &RaceSolver<Runner>| {
        solver.runners[0].overtake_lane_decision(&solver.state.course, &solver.runners)
    };
    assert_close!(decide(&solver).target, 0.3);

    // The extra move lane, now outside, is still preferred over the current lane.
    solver.runners[2].position = 115.0;
    let decision = decide(&solver);
    assert_eq!(decision.extra_move_lane, Some(0.5));
    assert_close!(decision.target, 0.5);
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
    assert!(!runner.has_lane_space_to(0.5));
    assert!(runner.has_lane_space_to(0.3));
    assert!(runner.has_lane_space_to(0.4));

    runner.blocked.side_out = false;
    runner.blocked.side_in = true;
    assert!(runner.has_lane_space_to(0.5));
    assert!(!runner.has_lane_space_to(0.3));
}

#[test]
fn test_target_lane_updates_when_close_or_blocked_towards_it() {
    let mut solver = lane_solver(&[(100.0, 0.4)]);
    let runner = &mut solver.runners[0];
    runner.lane.target = 0.4 + 0.4 * LANE;
    assert!(runner.should_update_target_lane());

    // Far from the target lane: only updated if blocked towards it.
    runner.lane.target = 0.5;
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
    runner.extra_move_lane = Some(0.5);

    // Otherwise, keep straight.
    runner.position = final_corner - 1.0;
    runner.lane.target = 0.6;
    assert_close!(target(&solver), 0.3);

    // In the final corner, not only on the final straight.
    solver.runners[0].position = final_corner + 1.0;
    assert_close!(target(&solver), 0.5);
}

#[test]
fn test_normal_mode_moves_away_from_inside_uma_even_if_blocked() {
    let mut solver = lane_solver(&[(100.0, 0.3), (100.5, 0.3 - LANE)]);
    let runner = &mut solver.runners[0];
    runner.blocked.side_in = true;
    runner.blocked.side_out = true;
    let target = solver.runners[0].normal_mode_lane_target(&solver.state.course, &solver.runners);
    assert_close!(target, 0.3 - LANE + 2.0 * LANE);
}

#[test]
fn test_overtake_lane_ignores_blocking_on_the_other_side() {
    // Blocked towards the outside, but free to move 1 horse lane inward.
    let mut solver = lane_solver(&[(100.0, 0.3), (50.0, 0.5)]);
    let runner = &mut solver.runners[0];
    runner.overtake_state = Some(OvertakeState::default());
    runner.blocked.side_out = true;
    let decision = solver.runners[0].overtake_lane_decision(&solver.state.course, &solver.runners);
    assert_close!(decision.target, 0.3 - horse_lanes_to_course_widths(1.0));
}

#[test]
fn test_fixed_mode_unless_blocked_towards_skill_lane() {
    let mut solver = lane_solver(&[(100.0, 0.4)]);
    let runner = &mut solver.runners[0];
    assert_eq!(runner.fixed_mode_lane_target(), None);

    runner.skill_modifiers.target_lane = Some(0.5);
    assert_eq!(runner.fixed_mode_lane_target(), Some(0.5));
    runner.blocked.side_in = true;
    assert_eq!(runner.fixed_mode_lane_target(), Some(0.5));
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
    let extra = runner.extra_move_lane.unwrap();
    assert!((0.25..0.35).contains(&extra), "{extra}");

    // Clamped beyond 0.1 course widths, and set on the final straight before
    // the 1st Anniv.
    runner.lane.current = 0.3;
    runner.update_final_extra_move_lane(&entering(0, last_straight), &course, original);
    let extra = runner.extra_move_lane.unwrap();
    assert!((0.5..0.6).contains(&extra), "{extra}");
}

#[test]
fn test_extra_move_lane_kept_after_mid_race() {
    let mut solver = lane_solver(&[(100.0, 0.3), (105.0, 0.4)]);
    let runner = &mut solver.runners[0];
    runner.extra_move_lane = Some(0.35);
    runner.phase = Phase::Late;
    let next = solver.runners[0].next_extra_move_lane(&solver.runners);
    assert_eq!(next, Some(0.35));

    // Furthest visible runner ahead during the Mid race.
    solver.runners[0].phase = Phase::Mid;
    let next = solver.runners[0].next_extra_move_lane(&solver.runners);
    assert_eq!(next, Some(0.4));
}
