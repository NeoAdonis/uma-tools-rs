use super::*;
use crate::course::LaneType;
use crate::race::runner::RushState;
use crate::race::spot_struggle::SpotStruggle;
use crate::test_utils::{
    alternative, assert_close, load_course, race_params, ready_solver, seeded_config, skill,
    skill_data, uma,
};
use crate::trigger::skill_activation::PendingEffect;
use crate::units::horse_lanes_to_course_widths;

/// Index of the course feature containing `position`, given `(start, end)`
/// ranges.
fn feature_at(position: f64, ranges: impl Iterator<Item = (f64, f64)>) -> Option<usize> {
    ranges
        .enumerate()
        .find(|(_, (start, end))| position >= *start && position < *end)
        .map(|(idx, _)| idx)
}

#[test]
fn test_position_derived_state_is_consistent_after_each_frame() {
    // Skills are checked against the runner's state; everything derived from the
    // position must match the position at the end of each frame.
    for course_id in [10101, 10104, 10606, 10913] {
        let mut solver = RaceSolver::new(load_course(course_id), race_params(), &seeded_config(0));
        for style in [Style::Front, Style::Pace, Style::Late, Style::End] {
            solver.add_runner(uma(style), Motivation::Normal, vec![]);
        }
        let mut solver = solver.ready_runners();

        while solver.state.finish_time.is_none() {
            solver.step(DELTA_TIME);

            let course = &solver.state.course;
            // Runners still in the gate aren't updated (no skills are checked).
            let started = solver
                .runners
                .iter()
                .filter(|runner| runner.start_delay <= solver.state.elapsed_time);
            for runner in started {
                let position = runner.position;
                let context = format!("course {course_id}, position {position}");
                assert_eq!(
                    runner.phase,
                    Phase::from_position(position, course),
                    "{context}"
                );
                assert_eq!(
                    runner.section,
                    course.section_from_position(position),
                    "{context}"
                );
                assert_eq!(
                    runner.current_corner(),
                    feature_at(
                        position,
                        course.corners.iter().map(|c| (c.start, c.start + c.length))
                    ),
                    "{context}"
                );
                assert_eq!(
                    runner.current_straight(),
                    feature_at(position, course.straights.iter().map(|s| (s.start, s.end))),
                    "{context}"
                );
                assert_eq!(
                    runner.current_slope(),
                    feature_at(
                        position,
                        course.slopes.iter().map(|s| (s.start, s.start + s.length))
                    ),
                    "{context}"
                );
            }
        }
    }
}

#[test]
fn test_skills_see_state_after_the_move() {
    // On a 2400m course, Late phase starts at 1600m, i.e. with 800m remaining.
    // The distance trigger and the phase guard become true on the same frame, so
    // both must be seen at once.
    let skill = skill(
        "late",
        "remain_distance<=800&phase>=2",
        SkillType::TargetSpeed,
        0.35,
    );

    let uma = Uma {
        stamina: 1200,
        ..uma(Style::Pace)
    };
    let mut solver = ready_solver(10108, uma, vec![skill], &seeded_config(0));
    assert_eq!(solver.state.course.distance, 2400);
    solver.run();

    let activations = &solver.runners[0].activated_skills;
    assert_eq!(activations.len(), 1, "Skill should activate");
    assert!(
        (1600.0..1605.0).contains(&activations[0].distance),
        "Should activate right after 1600m, got {}m",
        activations[0].distance
    );
}

/// Solver with one runner per `(style, position)` pair, sorted by position and
/// past every start delay, ready for positioning strategy updates.
fn positioning_solver(runners: &[(Style, f64)], game_version: GameVersion) -> RaceSolver<Runner> {
    let config = Configuration {
        game_version,
        ..seeded_config(0)
    };
    let mut solver = RaceSolver::new(load_course(10101), race_params(), &config);
    for &(style, _) in runners {
        solver.add_runner(uma(style), Motivation::Normal, vec![]);
    }
    let mut solver = solver.ready_runners();
    solver.state.elapsed_time = 10.0;
    for (runner, &(_, position)) in solver.runners.iter_mut().zip(runners) {
        runner.position = position;
    }
    sort_by_position(&mut solver);
    solver
}

fn sort_by_position(solver: &mut RaceSolver<Runner>) {
    solver
        .runners
        .sort_by(|a, b| b.position.partial_cmp(&a.position).unwrap());
}

fn runner_with_style(solver: &mut RaceSolver<Runner>, style: Style) -> &mut Runner {
    solver
        .runners
        .iter_mut()
        .find(|runner| runner.base.style == style)
        .unwrap()
}

fn is_pace_up_ex(solver: &mut RaceSolver<Runner>, style: Style) -> bool {
    matches!(
        runner_with_style(solver, style).positioning_strategy,
        PositioningStrategy::PaceUpEx
    )
}

#[test]
fn test_pace_up_ex_front_runner_enters_and_exits() {
    let mut solver = positioning_solver(
        &[(Style::Front, 100.0), (Style::End, 110.0)],
        GameVersion::LATEST,
    );
    solver.update_positioning_strategy();
    assert!(is_pace_up_ex(&mut solver, Style::Front));
    assert!(!is_pace_up_ex(&mut solver, Style::End));

    // Still behind: stays in Pace Up Ex.
    solver.update_positioning_strategy();
    assert!(is_pace_up_ex(&mut solver, Style::Front));

    runner_with_style(&mut solver, Style::Front).position = 120.0;
    sort_by_position(&mut solver);
    solver.update_positioning_strategy();
    assert!(!is_pace_up_ex(&mut solver, Style::Front));
}

#[test]
fn test_pace_up_ex_runaway_behind_front() {
    let mut solver = positioning_solver(
        &[(Style::Runaway, 100.0), (Style::Front, 110.0)],
        GameVersion::LATEST,
    );
    solver.update_positioning_strategy();
    assert!(is_pace_up_ex(&mut solver, Style::Runaway));
    assert!(!is_pace_up_ex(&mut solver, Style::Front));
}

#[test]
fn test_pace_up_ex_non_front_depends_on_pacemaker() {
    // End pacemaker should be behind Late.
    let mut solver = positioning_solver(
        &[(Style::Late, 100.0), (Style::End, 110.0)],
        GameVersion::LATEST,
    );
    solver.update_positioning_strategy();
    assert!(is_pace_up_ex(&mut solver, Style::Late));

    // Pace pacemaker should be ahead of Late even if an End runner is ahead too.
    let mut solver = positioning_solver(
        &[
            (Style::Late, 100.0),
            (Style::Pace, 120.0),
            (Style::End, 110.0),
        ],
        GameVersion::LATEST,
    );
    solver.update_positioning_strategy();
    assert!(!is_pace_up_ex(&mut solver, Style::Late));
}

#[test]
fn test_pace_up_ex_overrides_other_strategies() {
    let mut solver = positioning_solver(
        &[(Style::Pace, 100.0), (Style::End, 110.0)],
        GameVersion::LATEST,
    );
    runner_with_style(&mut solver, Style::Pace).positioning_strategy =
        PositioningStrategy::PaceDown(3.0);
    solver.update_positioning_strategy();
    assert!(is_pace_up_ex(&mut solver, Style::Pace));
}

#[test]
fn test_pace_up_ex_not_before_one_point_five_anniversary() {
    let mut solver = positioning_solver(
        &[
            (Style::Front, 100.0),
            (Style::Late, 110.0),
            (Style::End, 120.0),
        ],
        GameVersion::FirstAnniversary,
    );
    for _ in 0..10 {
        solver.update_positioning_strategy();
        assert!(!is_pace_up_ex(&mut solver, Style::Front));
        assert!(!is_pace_up_ex(&mut solver, Style::Late));
    }
}

#[test]
fn test_single_front_runner_keeps_speeding_up_with_larger_lead() {
    // A lone front runner may keep speeding up while less than 12.5m ahead;
    // otherwise the limit is 4.5m.
    let mut solver = positioning_solver(
        &[(Style::Front, 108.0), (Style::Pace, 100.0)],
        GameVersion::LATEST,
    );
    runner_with_style(&mut solver, Style::Front).positioning_strategy =
        PositioningStrategy::FrontSpeedUp;
    solver.update_positioning_strategy();
    assert!(matches!(
        runner_with_style(&mut solver, Style::Front).positioning_strategy,
        PositioningStrategy::FrontSpeedUp
    ));

    let mut solver = positioning_solver(
        &[(Style::Front, 108.0), (Style::Runaway, 100.0)],
        GameVersion::LATEST,
    );
    runner_with_style(&mut solver, Style::Front).positioning_strategy =
        PositioningStrategy::FrontSpeedUp;
    solver.update_positioning_strategy();
    assert!(matches!(
        runner_with_style(&mut solver, Style::Front).positioning_strategy,
        PositioningStrategy::Normal
    ));
}

#[test]
fn test_positioning_ends_after_first_ten_sections() {
    let mut solver = positioning_solver(
        &[(Style::Front, 100.0), (Style::End, 110.0)],
        GameVersion::LATEST,
    );
    solver.update_positioning_strategy();
    assert!(is_pace_up_ex(&mut solver, Style::Front));

    runner_with_style(&mut solver, Style::Front).section = POSITIONING_END_SECTION;
    solver.update_positioning_strategy();
    assert!(matches!(
        runner_with_style(&mut solver, Style::Front).positioning_strategy,
        PositioningStrategy::Normal
    ));
}

/// Solver with a single runner whose start delay is overridden.
fn start_delay_solver(start_delay: f64, rng_seed: u64) -> RaceSolver<Runner> {
    let mut solver = ready_solver(10101, uma(Style::Pace), vec![], &seeded_config(rng_seed));
    solver.runners[0].start_delay = start_delay;
    solver
}

/// `(position, speed, hp)` of the only runner.
fn runner_state(solver: &RaceSolver<Runner>) -> (f64, f64, f64) {
    let runner = &solver.runners[0];
    (runner.position, runner.speed, runner.hp.current)
}

#[test]
fn test_no_start_delay_moves_full_frame() {
    let mut solver = start_delay_solver(0.0, 0);
    solver.step(DELTA_TIME);
    let (position, speed, _) = runner_state(&solver);
    assert_close!(position, speed * DELTA_TIME, "position");
}

#[test]
fn test_start_delay_within_first_frame_prorates_distance_only() {
    let mut delayed = start_delay_solver(0.03, 0);
    let mut reference = start_delay_solver(0.0, 0);
    delayed.step(DELTA_TIME);
    reference.step(DELTA_TIME);

    let (position, speed, hp) = runner_state(&delayed);
    let (_, ref_speed, ref_hp) = runner_state(&reference);
    // Acceleration and HP consumption are unaffected.
    assert_close!(speed, ref_speed, "speed");
    assert_close!(hp, ref_hp, "hp");
    assert_close!(position, speed * (DELTA_TIME - 0.03), "position");
}

#[test]
fn test_start_delay_longer_than_a_frame_skips_whole_frames() {
    let mut delayed = start_delay_solver(0.09, 0);
    let (start_position, start_speed, start_hp) = runner_state(&delayed);

    // First frame is entirely within the start delay: nothing happens.
    delayed.step(DELTA_TIME);
    assert_eq!(
        runner_state(&delayed),
        (start_position, start_speed, start_hp)
    );

    // Second frame: full acceleration and HP consumption, prorated distance.
    delayed.step(DELTA_TIME);
    let mut reference = start_delay_solver(0.0, 0);
    reference.step(DELTA_TIME);
    let (position, speed, hp) = runner_state(&delayed);
    let (_, ref_speed, ref_hp) = runner_state(&reference);
    assert_close!(speed, ref_speed, "speed");
    assert_close!(hp, ref_hp, "hp");
    assert_close!(position, speed * (2.0 * DELTA_TIME - 0.09), "position");
    assert!(position > 0.0, "Runner must move forward, got {position}");
}

#[test]
fn test_longer_start_delay_loses_a_frame_of_acceleration() {
    // Both leave the gate on the second frame, but only the one that leaves on the
    // first frame accelerates on it.
    let mut early = start_delay_solver(0.05, 0);
    let mut late = start_delay_solver(0.09, 0);
    for _ in 0..2 {
        early.step(DELTA_TIME);
        late.step(DELTA_TIME);
    }
    let (early_position, early_speed, _) = runner_state(&early);
    let (late_position, late_speed, _) = runner_state(&late);
    assert!(early_speed > late_speed);
    assert!(early_position > late_position);
}

#[test]
fn test_position_never_decreases_regardless_of_start_delay() {
    for rng_seed in 0..20 {
        for i in 0..=20 {
            let start_delay = i as f64 * 0.01;
            let mut solver = start_delay_solver(start_delay, rng_seed);
            let mut prev_position = 0.0;
            for _ in 0..30 {
                solver.step(DELTA_TIME);
                let position = solver.runners[0].position;
                assert!(
                    position >= prev_position,
                    "seed {rng_seed}, delay {start_delay}: {prev_position} -> {position}"
                );
                prev_position = position;
            }
        }
    }
}

#[test]
fn test_runners_bound_to_rush_do_rush() {
    let mut solver = RaceSolver::new(load_course(10204), race_params(), &seeded_config(0));
    for _ in 0..9 {
        // Low wisdom for a high rush chance.
        let uma = Uma {
            wiz: 100,
            ..uma(Style::Late)
        };
        solver.add_runner(uma, Motivation::Normal, vec![]);
    }
    let mut solver = solver.ready_runners();
    solver.run();

    let bound_to_rush = solver
        .runners
        .iter()
        .filter(|runner| runner.rush_state.is_some())
        .collect::<Vec<_>>();
    assert!(!bound_to_rush.is_empty());
    for runner in bound_to_rush {
        assert!(runner.has_rushed(), "gate {}", runner.gate_roll);
    }
}

#[test]
fn test_all_runners_finish_with_margins() {
    let mut solver = RaceSolver::new(load_course(10101), race_params(), &seeded_config(0));
    for (i, style) in [Style::Front, Style::Pace, Style::Late, Style::End]
        .into_iter()
        .enumerate()
    {
        let uma = Uma {
            speed: 1000 + 50 * i as u16,
            ..uma(style)
        };
        solver.add_runner(uma, Motivation::Normal, vec![]);
    }
    let mut solver = solver.ready_runners();

    // Finish times are fixed once set, even though runners keep moving.
    let mut first_finish = None;
    while !solver.runners.iter().all(Runner::has_finished) {
        solver.step(DELTA_TIME);
        let winner = solver.runners.iter().find(|runner| runner.has_finished());
        if let Some(winner) = winner {
            let id = (winner.gate_roll, winner.finish_time.unwrap());
            assert_eq!(*first_finish.get_or_insert(id), id);
        }
    }
    solver.update_finish_margins();

    let mut runners: Vec<&Runner> = solver.runners.iter().collect();
    runners.sort_by(|a, b| a.finish_time.partial_cmp(&b.finish_time).unwrap());
    assert_eq!(runners[0].finish_margin, None);
    for pair in runners.windows(2) {
        let expected = (pair[1].finish_time.unwrap() - pair[0].finish_time.unwrap()) * 1.18 * 15.0;
        let margin = pair[1].finish_margin.expect("non-winners have a margin");
        assert!(margin >= 0.0);
        assert_close!(margin, expected);
    }
}

/// Solver with one runner per `(style, position, lane)` on a 1200m course (50m
/// sections). Runners must be given in descending position order.
fn struggle_solver(runners: &[(Style, f64, f64)], game_version: GameVersion) -> RaceSolver<Runner> {
    let styles: Vec<_> = runners
        .iter()
        .map(|&(style, position, _)| (style, position))
        .collect();
    let mut solver = positioning_solver(&styles, game_version);
    for (i, &(_, position, lane)) in runners.iter().enumerate() {
        place(&mut solver, i, position, lane);
    }
    solver
}

/// Move runner `i` to `position` and `lane`, keeping its section consistent.
fn place(solver: &mut RaceSolver<Runner>, i: usize, position: f64, lane: f64) {
    let section = solver.state.course.section_from_position(position);
    let runner = &mut solver.runners[i];
    runner.position = position;
    runner.section = section;
    runner.lane.current = lane;
}

fn update_struggle(solver: &mut RaceSolver<Runner>) {
    solver.assert_runners_sorted();
    solver.update_spot_struggle();
}

/// Whether each runner (in position order) counts as a struggler.
fn strugglers(solver: &RaceSolver<Runner>) -> Vec<bool> {
    solver
        .runners
        .iter()
        .map(|runner| runner.spot_struggle.is_struggler())
        .collect()
}

/// Whether each runner (in position order) has ever struggled.
fn has_struggled(solver: &RaceSolver<Runner>) -> Vec<bool> {
    solver
        .runners
        .iter()
        .map(|runner| runner.spot_struggle.has_struggled())
        .collect()
}

#[test]
fn test_spot_struggle_starts_between_close_front_runners() {
    // 0.16 course widths apart laterally is within 0.165 course widths.
    let mut solver = struggle_solver(
        &[(Style::Front, 200.0, 0.1), (Style::Front, 197.0, 0.26)],
        GameVersion::LATEST,
    );
    update_struggle(&mut solver);
    let elapsed_time = solver.state.elapsed_time;
    for runner in &solver.runners {
        assert!(runner.spot_struggle.is_active(elapsed_time));
        let bonus = spot_struggle::target_speed_bonus(runner.adjusted.guts);
        assert_eq!(runner.spot_struggle.speed_bonus(elapsed_time), bonus);
    }

    // Too far apart laterally.
    let mut solver = struggle_solver(
        &[(Style::Front, 200.0, 0.1), (Style::Front, 197.0, 0.27)],
        GameVersion::LATEST,
    );
    update_struggle(&mut solver);
    assert_eq!(strugglers(&solver), [false, false]);
}

#[test]
fn test_spot_struggle_runaways_use_trigger_distance() {
    let mut solver = struggle_solver(
        &[(Style::Runaway, 200.0, 0.0), (Style::Runaway, 196.0, 0.0)],
        GameVersion::LATEST,
    );
    update_struggle(&mut solver);
    assert_eq!(strugglers(&solver), [false, false]);

    place(&mut solver, 1, 197.0, 0.0);
    update_struggle(&mut solver);
    assert_eq!(strugglers(&solver), [true, true]);
}

#[test]
fn test_spot_struggle_needs_any_runner_past_min_position() {
    let mut solver = struggle_solver(
        &[(Style::Front, 149.0, 0.0), (Style::Front, 147.0, 0.0)],
        GameVersion::LATEST,
    );
    update_struggle(&mut solver);
    assert_eq!(strugglers(&solver), [false, false]);

    // Only the leader needs to be past it.
    place(&mut solver, 0, 150.0, 0.0);
    update_struggle(&mut solver);
    assert_eq!(strugglers(&solver), [true, true]);

    // Any runner ahead past it will do, even with another strategy.
    let mut solver = struggle_solver(
        &[
            (Style::Runaway, 150.0, 0.5),
            (Style::Front, 137.0, 0.0),
            (Style::Front, 135.0, 0.0),
        ],
        GameVersion::LATEST,
    );
    update_struggle(&mut solver);
    assert_eq!(strugglers(&solver), [false, true, true]);

    let mut solver = struggle_solver(
        &[
            (Style::Runaway, 149.0, 0.5),
            (Style::Front, 137.0, 0.0),
            (Style::Front, 135.0, 0.0),
        ],
        GameVersion::LATEST,
    );
    update_struggle(&mut solver);
    assert_eq!(strugglers(&solver), [false, false, false]);
}

#[test]
fn test_spot_struggle_needs_one_runner_before_end_of_sixth_section() {
    // Both in section 6 (0-indexed).
    let mut solver = struggle_solver(
        &[(Style::Front, 310.0, 0.0), (Style::Front, 307.0, 0.0)],
        GameVersion::LATEST,
    );
    update_struggle(&mut solver);
    assert_eq!(strugglers(&solver), [false, false]);

    // Leader in section 6, the other still in section 5.
    let mut solver = struggle_solver(
        &[(Style::Front, 302.0, 0.0), (Style::Front, 299.0, 0.0)],
        GameVersion::LATEST,
    );
    update_struggle(&mut solver);
    assert_eq!(strugglers(&solver), [true, true]);
}

#[test]
fn test_spot_struggle_blocked_by_later_strategy_ahead() {
    let mut solver = struggle_solver(
        &[
            (Style::Pace, 210.0, 0.5),
            (Style::Front, 200.0, 0.0),
            (Style::Front, 198.0, 0.0),
        ],
        GameVersion::LATEST,
    );
    update_struggle(&mut solver);
    assert_eq!(strugglers(&solver), [false, false, false]);

    // Runaways are expected to be ahead of front runners.
    let mut solver = struggle_solver(
        &[
            (Style::Runaway, 210.0, 0.5),
            (Style::Front, 200.0, 0.0),
            (Style::Front, 198.0, 0.0),
        ],
        GameVersion::LATEST,
    );
    update_struggle(&mut solver);
    assert_eq!(strugglers(&solver), [false, true, true]);

    // But front runners are not expected to be ahead of runaways.
    let mut solver = struggle_solver(
        &[
            (Style::Front, 210.0, 0.5),
            (Style::Runaway, 200.0, 0.0),
            (Style::Runaway, 198.0, 0.0),
        ],
        GameVersion::LATEST,
    );
    update_struggle(&mut solver);
    assert_eq!(strugglers(&solver), [false, false, false]);
}

#[test]
fn test_spot_struggle_strategies_do_not_mix() {
    let mut solver = struggle_solver(
        &[(Style::Runaway, 200.0, 0.0), (Style::Front, 198.0, 0.0)],
        GameVersion::LATEST,
    );
    update_struggle(&mut solver);
    assert_eq!(strugglers(&solver), [false, false]);
}

#[test]
fn test_spot_struggle_latecomers_do_not_join() {
    let mut solver = struggle_solver(
        &[
            (Style::Front, 200.0, 0.0),
            (Style::Front, 198.0, 0.0),
            (Style::Front, 197.0, 0.1),
            (Style::Front, 190.0, 0.0),
        ],
        GameVersion::LATEST,
    );
    update_struggle(&mut solver);
    assert_eq!(strugglers(&solver), [true, true, true, false]);

    place(&mut solver, 3, 196.5, 0.0);
    update_struggle(&mut solver);
    assert_eq!(strugglers(&solver), [true, true, true, false]);
}

#[test]
fn test_spot_struggle_once_per_strategy() {
    let mut solver = struggle_solver(
        &[(Style::Front, 200.0, 0.0), (Style::Front, 198.0, 0.0)],
        GameVersion::LATEST,
    );
    update_struggle(&mut solver);
    assert_eq!(strugglers(&solver), [true, true]);

    // Falling behind ends it for the second runner, and the leader exits in the
    // same frame as the last struggler.
    place(&mut solver, 1, 194.9, 0.0);
    update_struggle(&mut solver);
    assert_eq!(strugglers(&solver), [false, false]);
    assert_eq!(has_struggled(&solver), [true, true]);

    // Getting close again doesn't start another one.
    place(&mut solver, 1, 198.0, 0.0);
    update_struggle(&mut solver);
    assert_eq!(strugglers(&solver), [false, false]);
}

#[test]
fn test_spot_struggle_exit_when_far_from_every_struggler() {
    let mut solver = struggle_solver(
        &[
            (Style::Front, 200.0, 0.0),
            (Style::Front, 198.0, 0.0),
            (Style::Front, 197.0, 0.0),
        ],
        GameVersion::LATEST,
    );
    update_struggle(&mut solver);
    assert_eq!(strugglers(&solver), [true, true, true]);

    // The leader running away from both doesn't make anyone exit.
    place(&mut solver, 0, 209.0, 0.0);
    update_struggle(&mut solver);
    assert_eq!(strugglers(&solver), [true, true, true]);

    // Being more than 5m behind only the leader is not enough.
    place(&mut solver, 0, 204.0, 0.0);
    place(&mut solver, 1, 198.0, 0.0);
    place(&mut solver, 2, 197.0, 0.0);
    update_struggle(&mut solver);
    assert_eq!(strugglers(&solver), [true, true, true]);

    // Being more than 5m behind both others is.
    place(&mut solver, 0, 202.0, 0.0);
    place(&mut solver, 2, 192.9, 0.0);
    update_struggle(&mut solver);
    assert_eq!(strugglers(&solver), [true, true, false]);

    // The second runner falls 6m behind the leader. The third one is right
    // behind her, but already exited, so the second one exits too, and the
    // leader exits in the same frame as the last struggler.
    place(&mut solver, 0, 204.0, 0.0);
    place(&mut solver, 2, 197.0, 0.0);
    update_struggle(&mut solver);
    assert_eq!(strugglers(&solver), [false, false, false]);
}

#[test]
fn test_spot_struggle_exits_cascade_in_same_frame() {
    let mut solver = struggle_solver(
        &[
            (Style::Front, 200.0, 0.0),
            (Style::Front, 198.0, 0.0),
            (Style::Front, 197.0, 0.0),
        ],
        GameVersion::LATEST,
    );
    update_struggle(&mut solver);

    // The third runner falls behind both. That leaves the second one 6m behind
    // the only remaining partner, so she exits, and then the leader does.
    place(&mut solver, 1, 194.0, 0.0);
    place(&mut solver, 2, 188.0, 0.0);
    update_struggle(&mut solver);
    assert_eq!(strugglers(&solver), [false, false, false]);
}

#[test]
fn test_spot_struggle_lateral_exit_in_both_directions() {
    let mut solver = struggle_solver(
        &[(Style::Front, 200.0, 0.0), (Style::Front, 198.0, 0.0)],
        GameVersion::LATEST,
    );
    update_struggle(&mut solver);

    // Beyond 0.416 course widths.
    place(&mut solver, 1, 198.0, 0.42);
    update_struggle(&mut solver);
    assert_eq!(strugglers(&solver), [false, false]);
}

#[test]
fn test_spot_struggle_expired_strugglers_still_count() {
    let mut solver = struggle_solver(
        &[
            (Style::Front, 200.0, 0.0),
            (Style::Front, 198.0, 0.0),
            (Style::Front, 197.0, 0.0),
        ],
        GameVersion::LATEST,
    );
    update_struggle(&mut solver);
    let elapsed_time = solver.state.elapsed_time;
    solver.runners[0].spot_struggle = SpotStruggle::Active {
        speed_bonus: 0.1,
        expires_at: elapsed_time,
    };

    // The second runner is only close to the leader, whose effects expired.
    place(&mut solver, 2, 191.0, 0.0);
    update_struggle(&mut solver);
    assert!(solver.runners[1].spot_struggle.is_active(elapsed_time));
    assert_eq!(strugglers(&solver), [true, true, false]);
}

#[test]
fn test_spot_struggle_expired_strugglers_can_exit() {
    let mut solver = struggle_solver(
        &[
            (Style::Front, 200.0, 0.0),
            (Style::Front, 198.0, 0.0),
            (Style::Front, 197.0, 0.0),
        ],
        GameVersion::LATEST,
    );
    update_struggle(&mut solver);
    let elapsed_time = solver.state.elapsed_time;
    solver.runners[2].spot_struggle = SpotStruggle::Active {
        speed_bonus: 0.1,
        expires_at: elapsed_time,
    };

    // The third runner, whose effects expired, falls behind both others.
    place(&mut solver, 2, 191.0, 0.0);
    update_struggle(&mut solver);
    assert_eq!(solver.runners[2].spot_struggle, SpotStruggle::Ended);
    assert_eq!(strugglers(&solver), [true, true, false]);
}

#[test]
fn test_spot_struggle_ends_upon_entering_tenth_section() {
    let mut solver = struggle_solver(&[(Style::Front, 380.0, 0.0)], GameVersion::LATEST);
    let elapsed_time = solver.state.elapsed_time;
    let RaceSolver { runners, state, .. } = &mut solver;
    let runner = &mut runners[0];
    runner.start_spot_struggle(elapsed_time);

    // Section 8 (0-indexed) is still fine.
    runner.position = 405.0;
    runner.update_phase(state);
    assert!(runner.spot_struggle.is_active(elapsed_time));

    runner.position = 455.0;
    runner.update_phase(state);
    assert_eq!(runner.spot_struggle, SpotStruggle::Ended);
}

#[test]
fn test_spot_struggle_not_before_first_anniversary() {
    let mut solver = struggle_solver(
        &[(Style::Front, 200.0, 0.0), (Style::Front, 198.0, 0.0)],
        GameVersion::Original,
    );
    update_struggle(&mut solver);
    assert_eq!(strugglers(&solver), [false, false]);
}

/// Frames at 15 FPS until a duel starts: the first frame two runners are close
/// (frame 0), then 2s worth of frames.
const DUEL_FRAMES: usize = 31;

/// Solver with one runner per `(position, lane)` on a 1200m course, whose final
/// straight starts at 934m. Runners must be given in descending position order.
fn duel_solver(runners: &[(f64, f64)], game_version: GameVersion) -> RaceSolver<Runner> {
    let styles: Vec<_> = runners
        .iter()
        .map(|&(position, _)| (Style::Pace, position))
        .collect();
    let mut solver = positioning_solver(&styles, game_version);
    for (i, &(position, lane)) in runners.iter().enumerate() {
        place_on_straight(&mut solver, i, position, lane);
    }
    solver
}

/// Move runner `i` to `position` and `lane`, keeping its section and straight
/// consistent. Only moving forward is supported.
fn place_on_straight(solver: &mut RaceSolver<Runner>, i: usize, position: f64, lane: f64) {
    place(solver, i, position, lane);
    let RaceSolver { runners, state, .. } = solver;
    runners[i].check_straight_start(&state.course.straights);
}

/// Run `frames` duel updates at 15 FPS, with up-to-date order rates.
fn update_duels(solver: &mut RaceSolver<Runner>, frames: usize) {
    for _ in 0..frames {
        solver.assert_runners_sorted();
        solver.update_order_values();
        solver.update_duels(DELTA_TIME);
    }
}

/// Set the HP percentage of runner `i`.
fn set_hp_percent(solver: &mut RaceSolver<Runner>, i: usize, percent: f64) {
    let hp = &mut solver.runners[i].hp;
    hp.current = hp.maximum * percent / 100.0;
}

/// Whether each runner (in position order) is currently dueling.
fn duelists(solver: &RaceSolver<Runner>) -> Vec<bool> {
    solver
        .runners
        .iter()
        .map(|runner| runner.duel.is_dueling())
        .collect()
}

#[test]
fn test_duel_starts_after_two_seconds_close() {
    let mut solver = duel_solver(&[(1000.0, 0.1), (998.0, 0.1)], GameVersion::LATEST);
    update_duels(&mut solver, DUEL_FRAMES - 1);
    assert_eq!(duelists(&solver), [false, false]);

    update_duels(&mut solver, 1);
    assert_eq!(duelists(&solver), [true, true]);
    for runner in &solver.runners {
        assert!(runner.duel.is_dueling());
    }
}

#[test]
fn test_duel_needs_continuous_proximity() {
    let mut solver = duel_solver(&[(1000.0, 0.1), (998.0, 0.1)], GameVersion::LATEST);
    update_duels(&mut solver, DUEL_FRAMES - 1);

    // Being apart for a single frame resets the time.
    place_on_straight(&mut solver, 0, 1001.0, 0.1);
    update_duels(&mut solver, 1);
    assert_eq!(duelists(&solver), [false, false]);

    place_on_straight(&mut solver, 1, 999.0, 0.1);
    update_duels(&mut solver, DUEL_FRAMES - 1);
    assert_eq!(duelists(&solver), [false, false]);
    update_duels(&mut solver, 1);
    assert_eq!(duelists(&solver), [true, true]);
}

#[test]
fn test_duel_proximity_in_course_widths() {
    // Lane gap threshold is 0.25 course widths.
    let mut solver = duel_solver(&[(1000.0, 0.0), (998.0, 0.249)], GameVersion::LATEST);
    update_duels(&mut solver, DUEL_FRAMES);
    assert_eq!(duelists(&solver), [true, true]);

    let mut solver = duel_solver(&[(1000.0, 0.0), (998.0, 0.251)], GameVersion::LATEST);
    update_duels(&mut solver, DUEL_FRAMES);
    assert_eq!(duelists(&solver), [false, false]);

    let mut solver = duel_solver(&[(1000.0, 0.0), (997.0, 0.0)], GameVersion::LATEST);
    update_duels(&mut solver, DUEL_FRAMES);
    assert_eq!(duelists(&solver), [false, false]);
}

#[test]
fn test_duel_speed_difference() {
    let mut solver = duel_solver(&[(1000.0, 0.1), (998.0, 0.1)], GameVersion::LATEST);
    solver.runners[0].speed = 20.0;
    solver.runners[1].speed = 19.4;
    update_duels(&mut solver, DUEL_FRAMES);
    assert_eq!(duelists(&solver), [true, true]);

    let mut solver = duel_solver(&[(1000.0, 0.1), (998.0, 0.1)], GameVersion::LATEST);
    solver.runners[0].speed = 20.0;
    solver.runners[1].speed = 19.3;
    update_duels(&mut solver, DUEL_FRAMES * 2);
    assert_eq!(duelists(&solver), [false, false]);

    // Only proximity needs to last; speeds just need to match for a frame.
    solver.runners[1].speed = 19.5;
    update_duels(&mut solver, 1);
    assert_eq!(duelists(&solver), [true, true]);
}

#[test]
fn test_duel_needs_both_runners_with_enough_hp() {
    let mut solver = duel_solver(&[(1000.0, 0.1), (998.0, 0.1)], GameVersion::LATEST);
    set_hp_percent(&mut solver, 1, 14.9);
    update_duels(&mut solver, DUEL_FRAMES * 2);
    assert_eq!(duelists(&solver), [false, false]);

    set_hp_percent(&mut solver, 1, 15.0);
    update_duels(&mut solver, 1);
    assert_eq!(duelists(&solver), [true, true]);
}

#[test]
fn test_duel_needs_one_runner_in_top_half() {
    // 2nd and 3rd out of 4: top 50% and top 75%.
    let mut solver = duel_solver(
        &[(1100.0, 0.1), (1050.0, 0.1), (1048.0, 0.1), (1000.0, 0.1)],
        GameVersion::LATEST,
    );
    update_duels(&mut solver, DUEL_FRAMES);
    assert_eq!(duelists(&solver), [false, true, true, false]);

    // 3rd and 4th out of 4: top 75% and top 100%.
    let mut solver = duel_solver(
        &[(1100.0, 0.1), (1050.0, 0.1), (1000.0, 0.1), (998.0, 0.1)],
        GameVersion::LATEST,
    );
    update_duels(&mut solver, DUEL_FRAMES);
    assert_eq!(duelists(&solver), [false, false, false, false]);
}

#[test]
fn test_duel_only_on_final_straight() {
    // On a corner, before the final straight.
    let mut solver = duel_solver(&[(900.0, 0.1), (898.0, 0.1)], GameVersion::LATEST);
    update_duels(&mut solver, DUEL_FRAMES * 2);
    assert_eq!(duelists(&solver), [false, false]);
}

#[test]
fn test_duel_exits_below_five_percent_hp() {
    let mut solver = duel_solver(&[(1000.0, 0.1), (998.0, 0.1)], GameVersion::LATEST);
    update_duels(&mut solver, DUEL_FRAMES);

    set_hp_percent(&mut solver, 1, 5.0);
    update_duels(&mut solver, 1);
    assert_eq!(duelists(&solver), [true, true]);

    set_hp_percent(&mut solver, 1, 4.9);
    update_duels(&mut solver, 1);
    assert_eq!(duelists(&solver), [true, false]);
    assert!(solver.runners[1].duel.has_ended());
}

#[test]
fn test_duel_exits_when_separated_ahead_or_behind() {
    // The last runner duels with the leader, who is in the top 50%.
    let mut solver = duel_solver(
        &[(1000.0, 0.1), (998.5, 0.1), (997.5, 0.1)],
        GameVersion::LATEST,
    );
    update_duels(&mut solver, DUEL_FRAMES);
    assert_eq!(duelists(&solver), [true, true, true]);

    // The leader gets ahead, but not 5m ahead of the second one.
    place_on_straight(&mut solver, 0, 1003.4, 0.1);
    update_duels(&mut solver, 1);
    assert_eq!(duelists(&solver), [true, true, true]);

    // Getting 5m ahead of everyone is enough to exit.
    place_on_straight(&mut solver, 0, 1003.5, 0.1);
    update_duels(&mut solver, 1);
    assert_eq!(duelists(&solver), [false, true, true]);

    // So is falling 5m behind everyone.
    let mut solver = duel_solver(
        &[(1000.0, 0.1), (998.5, 0.1), (997.5, 0.1)],
        GameVersion::LATEST,
    );
    update_duels(&mut solver, DUEL_FRAMES);
    place_on_straight(&mut solver, 0, 1006.0, 0.1);
    place_on_straight(&mut solver, 1, 1004.0, 0.1);
    update_duels(&mut solver, 1);
    assert_eq!(duelists(&solver), [true, true, false]);
}

#[test]
fn test_duel_exit_counts_former_duelists() {
    let mut solver = duel_solver(&[(1000.0, 0.1), (998.0, 0.1)], GameVersion::LATEST);
    update_duels(&mut solver, DUEL_FRAMES);

    // The second runner exits, but the leader keeps dueling while close to her.
    set_hp_percent(&mut solver, 1, 4.0);
    update_duels(&mut solver, 1);
    assert_eq!(duelists(&solver), [true, false]);
    update_duels(&mut solver, DUEL_FRAMES);
    assert_eq!(duelists(&solver), [true, false]);

    place_on_straight(&mut solver, 0, 1003.0, 0.1);
    update_duels(&mut solver, 1);
    assert_eq!(duelists(&solver), [false, false]);
}

#[test]
fn test_duel_exit_ignores_runners_who_never_dueled() {
    let mut solver = duel_solver(
        &[(1000.0, 0.1), (998.0, 0.1), (980.0, 0.1)],
        GameVersion::LATEST,
    );
    update_duels(&mut solver, DUEL_FRAMES);
    assert_eq!(duelists(&solver), [true, true, false]);

    // The second runner falls 5m behind the leader, right next to the third
    // runner, who never dueled. Lanes keep them from starting a new duel.
    place_on_straight(&mut solver, 1, 995.0, 0.5);
    place_on_straight(&mut solver, 2, 994.0, 0.1);
    update_duels(&mut solver, 1);
    assert_eq!(duelists(&solver), [false, false, false]);
}

#[test]
fn test_former_duelists_cannot_duel_again() {
    // The last runner only keeps the third one out of the top 50%.
    let mut solver = duel_solver(
        &[(1000.0, 0.1), (998.0, 0.1), (980.0, 0.1), (950.0, 0.1)],
        GameVersion::LATEST,
    );
    update_duels(&mut solver, DUEL_FRAMES);
    set_hp_percent(&mut solver, 1, 4.0);
    update_duels(&mut solver, 1);
    assert_eq!(duelists(&solver), [true, false, false, false]);

    // The third runner stays close to the one who left, even after she
    // recovers, but neither starts dueling.
    set_hp_percent(&mut solver, 1, 100.0);
    place_on_straight(&mut solver, 2, 997.0, 0.1);
    update_duels(&mut solver, DUEL_FRAMES * 2);
    assert_eq!(duelists(&solver), [true, false, false, false]);
    assert!(!solver.runners[2].duel.has_dueled());

    // She can still duel with the one who is still dueling.
    place_on_straight(&mut solver, 2, 998.0, 0.1);
    update_duels(&mut solver, DUEL_FRAMES);
    assert_eq!(duelists(&solver), [true, false, true, false]);
}

#[test]
fn test_compete_fight_count_after_duel() {
    let mut solver = duel_solver(&[(1000.0, 0.1), (998.0, 0.1)], GameVersion::LATEST);
    update_duels(&mut solver, DUEL_FRAMES);
    set_hp_percent(&mut solver, 1, 4.0);
    update_duels(&mut solver, 1);
    for runner in &solver.runners {
        assert!(runner.duel.has_dueled());
    }
}

#[test]
fn test_duel_not_before_first_anniversary() {
    let mut solver = duel_solver(&[(1000.0, 0.1), (998.0, 0.1)], GameVersion::Original);
    update_duels(&mut solver, DUEL_FRAMES * 2);
    assert_eq!(duelists(&solver), [false, false]);
}

#[test]
fn test_force_in_modifier_strategy_ranges() {
    let cases = [
        (Style::Runaway, 0.02),
        (Style::Front, 0.02),
        (Style::Pace, 0.01),
        (Style::Late, 0.01),
        (Style::End, 0.03),
    ];
    for (style, strategy_modifier) in cases {
        let solver = positioning_solver(&[(style, 100.0)], GameVersion::LATEST);
        let force_in = solver.runners[0].force_in_modifier;
        assert!(
            (strategy_modifier..strategy_modifier + 0.1).contains(&force_in),
            "{style:?}: {force_in}"
        );
    }
}

#[test]
fn test_force_in_applies_only_away_from_inner_fence() {
    let mut solver = positioning_solver(&[(Style::End, 100.0)], GameVersion::LATEST);
    let RaceSolver { runners, state, .. } = &mut solver;
    let runner = &mut runners[0];
    runner.phase = Phase::Early;
    runner.blocked.side_in = true;
    let force_in = runner.force_in_modifier;
    assert!(force_in > 0.0);

    // Threshold is 0.12 course widths.
    let target_speed_at = |runner: &mut Runner, lane: f64| {
        runner.lane.current = lane;
        runner.update_target_speed(state);
        runner.target_speed
    };
    let near_fence = target_speed_at(runner, 0.119);
    let away_from_fence = target_speed_at(runner, 0.121);
    assert_close!(away_from_fence - near_fence, force_in);
    assert_eq!(target_speed_at(runner, 0.0), near_fence);

    // Only in the early phase.
    runner.phase = Phase::Mid;
    let mid_near = target_speed_at(runner, 0.0);
    let mid_away = target_speed_at(runner, 0.5);
    assert_eq!(mid_near, mid_away);
}

#[test]
fn test_force_in_applies_only_when_inside_is_blocked() {
    let mut solver = positioning_solver(&[(Style::End, 100.0)], GameVersion::LATEST);
    let RaceSolver { runners, state, .. } = &mut solver;
    let runner = &mut runners[0];
    runner.phase = Phase::Early;
    runner.lane.current = 0.5;
    let mut target_speed_with = |side_in: bool, side_out: bool| {
        runner.blocked.side_in = side_in;
        runner.blocked.side_out = side_out;
        runner.update_target_speed(state);
        runner.target_speed
    };

    let open = target_speed_with(false, false);
    assert_eq!(target_speed_with(false, true), open);
    let blocked_in = target_speed_with(true, false);
    let force_in = solver.runners[0].force_in_modifier;
    assert_close!(blocked_in - open, force_in);
}

#[test]
fn test_lane_type_in_course_widths() {
    let mut solver = positioning_solver(&[(Style::Front, 100.0)], GameVersion::LATEST);
    let runner = &mut solver.runners[0];
    // Thresholds are 0.2, 0.4 and 0.6 course widths, regardless of the course's
    // total width.
    let cases = [
        (0.0, LaneType::Inner),
        (0.2, LaneType::Inner),
        (0.21, LaneType::Middle),
        (0.4, LaneType::Middle),
        (0.41, LaneType::Outer),
        (0.6, LaneType::Outer),
        (0.61, LaneType::Outside),
    ];
    for (lane, expected) in cases {
        runner.lane.current = lane;
        assert_eq!(runner.lane_type(), expected, "lane {lane}");
    }
}

#[test]
fn test_skill_target_lane_in_course_widths() {
    let mut solver = positioning_solver(&[(Style::Front, 100.0)], GameVersion::LATEST);
    let RaceSolver { runners, state, .. } = &mut solver;
    let runner = &mut runners[0];
    runner.lane.current = 0.4;
    runner.skill_modifiers.target_lane = Some(0.5);
    for _ in 0..2000 {
        runner.update_lane(&state.course, DELTA_TIME);
    }
    assert_close!(runner.lane.current, 0.5);
}

#[test]
fn test_move_lane_modifier_requires_lane_movement() {
    let mut solver = positioning_solver(&[(Style::Front, 100.0)], GameVersion::LATEST);
    let RaceSolver { runners, state, .. } = &mut solver;
    let runner = &mut runners[0];
    runner.skill_modifiers.lane_speed = 0.1;
    runner.lane.current = 0.4;
    runner.lane.target = 0.4;

    runner.update_lane(&state.course, DELTA_TIME);
    assert_eq!(runner.lane.speed, 0.0);
    runner.update_target_speed(state);
    let still_speed = runner.target_speed;

    runner.lane.target = 0.7;
    runner.update_lane(&state.course, DELTA_TIME);
    assert!(runner.lane.speed > 0.0);
    runner.update_target_speed(state);
    let expected = (0.0002 * runner.adjusted.power as f64).sqrt();
    assert_close!(runner.target_speed - still_speed, expected);
}

#[test]
fn test_lane_speed_is_directional_in_course_widths() {
    let mut solver = positioning_solver(&[(Style::Front, 100.0)], GameVersion::LATEST);
    let RaceSolver { runners, state, .. } = &mut solver;
    let runner = &mut runners[0];
    runner.lane.current = 0.4;
    runner.skill_modifiers.lane_speed = 0.1;

    // Moving out: (current speed + skill modifier) course widths per second.
    runner.lane.target = 0.7;
    runner.update_lane(&state.course, DELTA_TIME);
    let current_speed = runner.lane.current_speed;
    assert_close!(current_speed, 0.03 * DELTA_TIME);
    assert_close!(runner.lane.speed, current_speed + 0.1);
    let moved = runner.lane.current - 0.4;
    assert_close!(moved, runner.lane.speed * DELTA_TIME);

    // Moving in: faster the further out the runner is.
    runner.lane.target = 0.0;
    let lane = runner.lane.current;
    runner.update_lane(&state.course, DELTA_TIME);
    let direction_modifier = -(1.0 + lane);
    let expected = (runner.lane.current_speed + 0.1) * direction_modifier;
    assert_close!(runner.lane.speed, expected);
}

#[test]
fn test_lane_change_reaching_target_resets_current_speed() {
    let mut solver = positioning_solver(&[(Style::Front, 100.0)], GameVersion::LATEST);
    let RaceSolver { runners, state, .. } = &mut solver;
    let runner = &mut runners[0];
    runner.lane.current = 0.4;
    runner.lane.target = 0.401;
    runner.lane.current_speed = 0.02;

    runner.update_lane(&state.course, DELTA_TIME);
    assert_eq!(runner.lane.current, 0.401);
    assert_eq!(runner.lane.current_speed, 0.0);
    // The runner still moved on this frame.
    assert!(runner.lane.speed > 0.0);

    runner.update_lane(&state.course, DELTA_TIME);
    assert_eq!(runner.lane.speed, 0.0);
}

#[test]
fn test_side_blocked_runner_keeps_accelerating_without_moving() {
    let mut solver = positioning_solver(&[(Style::Front, 100.0)], GameVersion::LATEST);
    let RaceSolver { runners, state, .. } = &mut solver;
    let runner = &mut runners[0];
    runner.lane.current = 0.4;
    runner.lane.target = 0.7;
    runner.blocked.side_out = true;

    runner.update_lane(&state.course, DELTA_TIME);
    runner.update_lane(&state.course, DELTA_TIME);
    assert_eq!(runner.lane.current, 0.4);
    assert_eq!(runner.lane.speed, 0.0);
    assert_close!(runner.lane.current_speed, 0.03 * 2.0 * DELTA_TIME);

    // Blocked only towards the outside, so moving in is still possible.
    runner.lane.target = 0.0;
    runner.update_lane(&state.course, DELTA_TIME);
    assert!(runner.lane.current < 0.4);
    assert!(runner.lane.speed < 0.0);
}

#[test]
fn test_normal_mode_lane_targets_in_course_widths() {
    let mut solver = positioning_solver(&[(Style::Front, 100.0)], GameVersion::LATEST);
    let target = |solver: &RaceSolver<Runner>| {
        solver.runners[0]
            .normal_mode_lane_decision(&solver.state.course, &solver.runners)
            .target
    };

    // Move inward by 0.05 course widths in the early phase.
    let runner = &mut solver.runners[0];
    runner.phase = Phase::Early;
    runner.lane.current = 0.4;
    assert_close!(target(&solver), 0.35);

    // Pace down targets 0.18 course widths from the inner fence.
    solver.runners[0].positioning_strategy = PositioningStrategy::PaceDown(0.0);
    assert_close!(target(&solver), 0.18);
}

#[test]
fn test_overlap_bumps_outer_runner_ahead() {
    // Bumped 0.4 horse lanes away from the inner runner.
    let mut solver = struggle_solver(
        &[(Style::Front, 200.0, 0.2), (Style::Front, 199.8, 0.19)],
        GameVersion::LATEST,
    );
    solver.resolve_overlaps();
    let (ahead, behind) = (&solver.runners[0], &solver.runners[1]);
    assert_close!(ahead.lane.current, 0.19 + horse_lanes_to_course_widths(0.4));
    assert_eq!(ahead.lane.target, ahead.lane.current);
    assert!(ahead.force_lane_update);
    assert_eq!(behind.lane.current, 0.19);
    assert!(!behind.force_lane_update);
}

#[test]
fn test_overlap_bumps_outer_runner_behind() {
    let mut solver = struggle_solver(
        &[(Style::Front, 200.0, 0.19), (Style::Front, 199.8, 0.2)],
        GameVersion::LATEST,
    );
    solver.resolve_overlaps();
    let (ahead, behind) = (&solver.runners[0], &solver.runners[1]);
    assert_eq!(ahead.lane.current, 0.19);
    assert!(!ahead.force_lane_update);
    assert_close!(
        behind.lane.current,
        0.19 + horse_lanes_to_course_widths(0.4)
    );
    assert!(behind.force_lane_update);
}

#[test]
fn test_no_bump_without_overlap() {
    // Too far apart.
    let mut solver = struggle_solver(
        &[(Style::Front, 200.0, 0.2), (Style::Front, 199.6, 0.19)],
        GameVersion::LATEST,
    );
    solver.resolve_overlaps();
    assert_eq!(solver.runners[0].lane.current, 0.2);
    assert_eq!(solver.runners[1].lane.current, 0.19);

    // Too far apart laterally.
    let mut solver = struggle_solver(
        &[(Style::Front, 200.0, 0.215), (Style::Front, 199.8, 0.19)],
        GameVersion::LATEST,
    );
    solver.resolve_overlaps();
    assert_eq!(solver.runners[0].lane.current, 0.215);
    assert_eq!(solver.runners[1].lane.current, 0.19);
    assert!(solver.runners.iter().all(|r| !r.force_lane_update));
}

#[test]
fn test_bumped_runner_updates_target_lane_while_side_blocked() {
    let mut solver = struggle_solver(
        &[(Style::Front, 200.0, 0.2), (Style::Front, 199.8, 0.19)],
        GameVersion::LATEST,
    );
    for runner in &mut solver.runners {
        runner.phase = Phase::Mid;
    }
    // Heading inward, right into the other runner.
    solver.runners[0].lane.target = 0.0;

    solver.resolve_overlaps();
    solver.update_blocking_status(DELTA_TIME);
    let bumped = &solver.runners[0];
    assert!(bumped.blocked.side_in);
    assert!(!bumped.force_lane_update);
    // Too close to the runner inside in mid-race, so she targets 2 horse lanes
    // away from her. Without the forced update, the target would have stayed at
    // the bumped lane.
    let expected = 0.19 + horse_lanes_to_course_widths(2.0);
    assert_close!(bumped.lane.target, expected);
}

#[test]
fn test_individual_runners_are_all_opponents() {
    let mut solver = RaceSolver::new(load_course(10101), race_params(), &seeded_config(0));
    for _ in 0..3 {
        solver.add_runner(uma(Style::Pace), Motivation::Normal, vec![]);
    }
    let solver = solver.ready_runners();

    assert_eq!(solver.state.team_count, 3);
    for a in &solver.runners {
        for b in &solver.runners {
            assert_eq!(a.is_opponent(b), a.gate_roll != b.gate_roll);
        }
    }
}

#[test]
fn test_team_members_are_allies() {
    let mut solver = RaceSolver::new(load_course(10101), race_params(), &seeded_config(0));
    let member = || (uma(Style::Pace), Motivation::Normal, vec![]);
    solver.add_team(vec![member(), member(), member()]);
    solver.add_runner(uma(Style::Late), Motivation::Normal, vec![]);
    solver.add_team(vec![member(), member()]);
    let solver = solver.ready_runners();

    assert_eq!(solver.state.team_count, 3);
    let mut sizes = [0; 3];
    for runner in &solver.runners {
        sizes[runner.initial_state.team as usize] += 1;
    }
    assert_eq!(sizes, [3, 1, 2]);

    for a in &solver.runners {
        for b in &solver.runners {
            let same_team = a.initial_state.team == b.initial_state.team;
            assert_eq!(a.is_opponent(b), !same_team);
        }
    }
}

#[test]
#[should_panic(expected = "Team must have between 1 and 3 runners")]
fn test_team_larger_than_max_size_panics() {
    let mut solver = RaceSolver::new(load_course(10101), race_params(), &seeded_config(0));
    let member = || (uma(Style::Pace), Motivation::Normal, vec![]);
    solver.add_team(vec![member(), member(), member(), member()]);
}

#[test]
#[should_panic(expected = "Team must have between 1 and 3 runners")]
fn test_empty_team_panics() {
    let mut solver = RaceSolver::new(load_course(10101), race_params(), &seeded_config(0));
    solver.add_team(vec![]);
}

#[test]
#[should_panic(expected = "Cannot add more runners than available gates")]
fn test_team_exceeding_gates_panics() {
    // 9 gates: 8 individual runners leave room for one more, not a team of two.
    let mut solver = RaceSolver::new(load_course(10101), race_params(), &seeded_config(0));
    for _ in 0..8 {
        solver.add_runner(uma(Style::Pace), Motivation::Normal, vec![]);
    }
    let member = || (uma(Style::Pace), Motivation::Normal, vec![]);
    solver.add_team(vec![member(), member()]);
}

/// Put runner `i` in rush state.
fn set_rushed(solver: &mut RaceSolver<Runner>, i: usize) {
    solver.runners[i].rush_state = Some(RushState {
        section: 0,
        start_time: 1.0,
        end_time: 4.0,
        max_duration: 12.0,
        is_rushed: true,
        style: Style::Pace,
    });
}

#[test]
fn test_rush_counters_by_position_count_opponents() {
    // In position order: team 0 (×2), team 1 and team 2.
    let mut solver = RaceSolver::new(load_course(10101), race_params(), &seeded_config(0));
    let member = || (uma(Style::Pace), Motivation::Normal, vec![]);
    solver.add_team(vec![member(), member()]);
    solver.add_runner(uma(Style::Pace), Motivation::Normal, vec![]);
    solver.add_runner(uma(Style::Pace), Motivation::Normal, vec![]);
    let mut solver = solver.ready_runners();
    solver.state.elapsed_time = 10.0;
    solver.runners.sort_by_key(|r| r.initial_state.team);
    for (i, position) in [400.0, 300.0, 200.0, 100.0].into_iter().enumerate() {
        solver.runners[i].position = position;
    }

    // Rushed: the team 0 leader and the team 1 runner.
    set_rushed(&mut solver, 0);
    set_rushed(&mut solver, 2);
    solver.update_order_values();

    let counters = solver
        .runners
        .iter()
        .map(|r| {
            let c = &r.rush_counter;
            (c.ahead, c.opponents_ahead, c.behind, c.opponents_behind)
        })
        .collect::<Vec<_>>();
    assert_eq!(
        counters,
        [
            // Team 0 leader: the team 1 runner behind is an opponent.
            (0, 0, 1, 1),
            // Team 0 member: the leader ahead is an ally.
            (1, 0, 1, 1),
            // Team 1: does not count itself.
            (1, 1, 0, 0),
            // Team 2: both rushed runners ahead are opponents.
            (2, 2, 0, 0),
        ]
    );
}

#[test]
fn test_rushed_by_team_matches_runners() {
    let mut solver = RaceSolver::new(load_course(10204), race_params(), &seeded_config(0));
    // Low wisdom for a high rush chance.
    let member = |style| {
        let uma = Uma {
            wiz: 100,
            ..uma(style)
        };
        (uma, Motivation::Normal, vec![])
    };
    solver.add_team(vec![
        member(Style::Late),
        member(Style::Late),
        member(Style::Pace),
    ]);
    solver.add_team(vec![member(Style::Late), member(Style::End)]);
    for style in [Style::Runaway, Style::Runaway, Style::Front, Style::Pace] {
        let (uma, motivation, skills) = member(style);
        solver.add_runner(uma, motivation, skills);
    }
    let mut solver = solver.ready_runners();

    let mut max_rushed = 0;
    let mut runaway_rushed = false;
    while solver.state.finish_time.is_none() {
        solver.step(DELTA_TIME);

        let mut expected = vec![[0; Style::COUNT]; solver.state.team_count as usize];
        for runner in solver.runners.iter().filter(|r| r.is_rushed()) {
            // Runaways count as Front runners.
            expected[runner.initial_state.team as usize][runner.base_style() as usize] += 1;
            runaway_rushed |= runner.base.style == Style::Runaway;
        }
        assert_eq!(
            solver.state.rushed_by_team, expected,
            "t={}",
            solver.state.elapsed_time
        );
        let rushed = expected.iter().flatten().sum::<u8>();
        max_rushed = max_rushed.max(rushed);
    }
    assert!(max_rushed > 0, "no runner rushed");
    assert!(runaway_rushed, "no Runaway rushed");
}

#[test]
fn test_team_activations_sum_team_members() {
    let mut solver = RaceSolver::new(load_course(10101), race_params(), &seeded_config(0));
    let member = || (uma(Style::Pace), Motivation::Normal, vec![]);
    solver.add_team(vec![member(), member()]);
    solver.add_runner(uma(Style::Pace), Motivation::Normal, vec![]);
    let mut solver = solver.ready_runners();
    for runner in &mut solver.runners {
        let all = match runner.initial_state.team {
            0 => 3,
            _ => 4,
        };
        runner.trigger_scheduler.activation_counts.all = all;
    }

    // Recounting is idempotent.
    solver.update_team_activations();
    solver.update_team_activations();
    assert_eq!(solver.state.team_activations, [6, 4]);
}

#[test]
fn test_team_activations_match_runners_each_frame() {
    let mut solver = RaceSolver::new(load_course(10101), race_params(), &seeded_config(0));
    let skills = || {
        vec![
            // Out-of-gate.
            skill("speed", "always==1", SkillType::SpeedUp, 80.0),
            skill("mid", "phase==1", SkillType::TargetSpeed, 0.1),
            skill("late", "phase>=2", SkillType::TargetSpeed, 0.1),
        ]
    };
    let member = |style| (uma(style), Motivation::Normal, skills());
    solver.add_team(vec![member(Style::Front), member(Style::Late)]);
    solver.add_runner(uma(Style::Pace), Motivation::Normal, skills());
    solver.add_runner(uma(Style::End), Motivation::Normal, vec![]);
    let mut solver = solver.ready_runners();

    while solver.state.finish_time.is_none() {
        // Each frame sees the activations up to the end of the previous one.
        let mut expected = vec![0; solver.state.team_count as usize];
        for runner in &solver.runners {
            let team = runner.initial_state.team as usize;
            expected[team] += runner.trigger_scheduler.activation_counts.all;
        }
        solver.step(DELTA_TIME);
        assert_eq!(
            solver.state.team_activations, expected,
            "t={}",
            solver.state.elapsed_time
        );
    }
    // Every skill activated: 3 per runner with skills, none for the last runner.
    solver.update_team_activations();
    assert_eq!(solver.state.team_activations, [6, 3, 0]);
}

/// Ready race with one runner per `(style, team)`, placed in that order from first
/// to last, 10 meters apart.
fn targeting_solver(runners: &[(Style, u8)]) -> RaceSolver<Runner> {
    let mut solver = RaceSolver::new(load_course(10101), race_params(), &seeded_config(0));
    for _ in runners {
        solver.add_runner(uma(Style::Late), Motivation::Normal, vec![]);
    }
    let mut solver = solver.ready_runners();
    solver.state.elapsed_time = 10.0;
    // Runners are built in gate order, so assign everything here.
    for (i, (runner, &(style, team))) in solver.runners.iter_mut().zip(runners).enumerate() {
        runner.base.style = style;
        runner.adjusted.style = style;
        runner.initial_state.team = team;
        runner.position = 1000.0 - 10.0 * i as f64;
    }
    solver
}

/// Targets of a skill effect casted by runner `caster` (in position order).
fn targets(solver: &RaceSolver<Runner>, caster: usize, target: SkillTarget) -> Vec<usize> {
    solver.select_targets(caster, target)
}

#[test]
fn test_targets_nearest_runners_ahead_and_behind() {
    let solver = targeting_solver(&[0, 1, 2, 3, 4, 5].map(|team| (Style::Late, team)));
    assert_eq!(
        targets(&solver, 3, SkillTarget::AheadOfSelf { count: 2 }),
        [2, 1]
    );
    assert_eq!(
        targets(&solver, 3, SkillTarget::BehindSelf { count: 5 }),
        [4, 5]
    );
    // The first 4 places, oneself excluded.
    assert_eq!(
        targets(&solver, 2, SkillTarget::AheadOfPosition { position: 5 }),
        [0, 1, 3]
    );
}

#[test]
fn test_allies_count_towards_max_targets_but_are_not_debuffed() {
    // Runner 2 is an ally of runner 3.
    let solver = targeting_solver(&[
        (Style::Late, 0),
        (Style::Late, 1),
        (Style::Late, 2),
        (Style::Late, 2),
    ]);
    assert_eq!(
        targets(&solver, 3, SkillTarget::AheadOfSelf { count: 2 }),
        [1]
    );
    assert_eq!(targets(&solver, 3, SkillTarget::AllAllies), [2]);
    // Non-debuff targets include oneself when meant to.
    assert_eq!(targets(&solver, 3, SkillTarget::All), [0, 1, 2, 3]);
}

#[test]
fn test_targets_opponents_with_style() {
    let solver = targeting_solver(&[
        (Style::Front, 0),
        (Style::Runaway, 1),
        (Style::Front, 2),
        (Style::Pace, 3),
    ]);
    // Runaways count as Front runners; allies are excluded.
    assert_eq!(
        targets(
            &solver,
            2,
            SkillTarget::EnemyStrategy {
                style: Style::Front
            }
        ),
        [0, 1]
    );
}

#[test]
fn test_finished_runners_are_not_counted_as_targets() {
    let mut solver = targeting_solver(&[(Style::Late, 0), (Style::Late, 1), (Style::Late, 2)]);
    solver.runners[1].finish_time = Some(60.0);
    assert_eq!(
        targets(&solver, 2, SkillTarget::AheadOfSelf { count: 1 }),
        [0]
    );
}

#[test]
fn test_pending_effects_apply_to_targets() {
    let mut solver = targeting_solver(&[(Style::Late, 0), (Style::Late, 1), (Style::Late, 2)]);
    solver.runners[0].pending_effects.push(PendingEffect {
        effect: SkillEffect {
            skill_type: SkillType::CurrentSpeed,
            target: SkillTarget::BehindSelf { count: 1 },
            modifier: -0.15,
        },
        duration: 3.0,
        wiz_bonus: 0.0,
    });
    solver.apply_pending_effects();

    assert!(solver.runners[0].pending_effects.is_empty());
    assert_close!(solver.runners[0].skill_modifiers.current_speed, 0.0);
    assert_close!(solver.runners[1].skill_modifiers.current_speed, -0.15);
    assert_close!(solver.runners[2].skill_modifiers.current_speed, 0.0);
}

/// Run a race with one runner per skill list and return, for each list, whether
/// the runner activated `skill_id`.
fn activates(skills: Vec<Vec<Skill>>, skill_id: &str) -> Vec<bool> {
    let mut solver = RaceSolver::new(load_course(10101), race_params(), &seeded_config(0));
    let ids: Vec<Vec<String>> = skills
        .iter()
        .map(|skills| skills.iter().map(|skill| skill.id.clone()).collect())
        .collect();
    for skills in skills {
        solver.add_runner(uma(Style::Late), Motivation::Normal, skills);
    }
    let mut solver = solver.ready_runners();
    solver.run();

    // Runners are built in gate order, so find each one by her skills.
    ids.iter()
        .map(|ids| {
            let runner = solver
                .runners
                .iter()
                .find(|runner| {
                    runner
                        .available_skills
                        .iter()
                        .map(|skill| &skill.id)
                        .eq(ids.iter())
                })
                .unwrap();
            runner.activated_skill_ids.contains(skill_id)
        })
        .collect()
}

/// Heal skill activating 5 seconds into the race.
fn heal() -> Skill {
    skill("heal", "accumulatetime>=5", SkillType::Recovery, 0.055)
}

#[test]
fn test_other_runner_advantage_skill_triggers_skill() {
    let react = || {
        skill(
            "react",
            "is_other_character_activate_advantage_skill==9",
            SkillType::TargetSpeed,
            0.35,
        )
    };
    // Own heals don't count.
    assert_eq!(
        activates(
            vec![vec![heal()], vec![react()], vec![heal(), react()]],
            "react"
        ),
        [false, true, true]
    );
    assert_eq!(activates(vec![vec![heal(), react()]], "react"), [false]);
}

#[test]
fn test_other_runner_advantage_skill_is_only_seen_on_its_frame() {
    // The heal activates long before the rest of the condition is true.
    let react = skill(
        "react",
        "is_other_character_activate_advantage_skill==9&accumulatetime>=20",
        SkillType::TargetSpeed,
        0.35,
    );
    assert_eq!(
        activates(vec![vec![heal()], vec![react]], "react"),
        [false, false]
    );
}

#[test]
fn test_used_recovery_targets_runners_that_healed_this_frame() {
    let mut solver = targeting_solver(&[(Style::Late, 0), (Style::Late, 1), (Style::Late, 2)]);
    solver.runners[0]
        .activation_flags
        .buff_activated
        .insert(SkillType::Recovery);
    solver.runners[2]
        .activation_flags
        .buff_activated
        .insert(SkillType::TargetSpeed);
    assert_eq!(
        targets(&solver, 1, SkillTarget::UsedRecovery { value: 1 }),
        [0]
    );
}

/// Run a race with a single runner holding a skill with the given alternative
/// conditions, and return the indices of the alternatives that activated.
fn activated_alternatives(conditions: &[&str]) -> Vec<usize> {
    let alternatives = conditions
        .iter()
        .map(|condition| alternative(condition, SkillType::TargetSpeed, 0.15))
        .collect();
    let skill = Skill::new("test".to_string(), skill_data(alternatives));
    let mut solver = ready_solver(10101, uma(Style::Late), vec![skill], &seeded_config(0));
    solver.run();

    let mut activated: Vec<usize> = solver.runners[0]
        .activated_alternatives
        .iter()
        .map(|&(_, alternative)| alternative)
        .collect();
    activated.sort();
    activated
}

#[test]
fn test_skill_activates_through_a_single_alternative() {
    assert_eq!(
        activated_alternatives(&["accumulatetime>=5", "accumulatetime>=10"]),
        [0]
    );
    // Either alternative can be the one.
    assert_eq!(
        activated_alternatives(&["accumulatetime>=10", "accumulatetime>=5"]),
        [1]
    );
    // When both are satisfied on the same frame, the first one wins.
    assert_eq!(
        activated_alternatives(&["accumulatetime>=5", "accumulatetime>=5"]),
        [0]
    );
    // Even with different triggers: last straight entry and a position window.
    assert_eq!(
        activated_alternatives(&["is_last_straight_onetime==1", "is_last_straight==1"]),
        [0]
    );
}

#[test]
fn test_continuation_activates_after_other_alternative() {
    assert_eq!(
        activated_alternatives(&[
            "accumulatetime>=5",
            "is_activate_other_skill_detail==1&accumulatetime>=10",
        ]),
        [0, 1]
    );
    // Never without the other alternative.
    assert_eq!(
        activated_alternatives(&[
            "accumulatetime>=999",
            "is_activate_other_skill_detail==1&accumulatetime>=10",
        ]),
        Vec::<usize>::new()
    );
}

/// Out-of-gate skill with the given effects on oneself, whose condition is
/// always satisfied.
fn out_of_gate_skill(id: &str, base_duration: f64, effects: &[(SkillType, f64)]) -> Skill {
    let alternative = SkillAlternative {
        base_duration,
        effects: effects
            .iter()
            .map(|&(skill_type, modifier)| SkillEffect {
                skill_type,
                target: SkillTarget::Oneself,
                modifier,
            })
            .collect(),
        ..alternative("always==1", SkillType::SpeedUp, 0.0)
    };
    Skill::new(id.to_string(), skill_data(vec![alternative]))
}

/// IDs and distances of the skills activated by the only runner.
fn activations(solver: &RaceSolver<Runner>) -> Vec<(&str, f64)> {
    solver.runners[0]
        .activated_skills
        .iter()
        .map(|record| (record.skill_id.as_str(), record.distance))
        .collect()
}

#[test]
fn test_out_of_gate_skill_activates_on_first_frame_of_movement() {
    let speed = out_of_gate_skill("speed", f64::INFINITY, &[(SkillType::SpeedUp, 40.0)]);
    let mut solver = ready_solver(10101, uma(Style::Pace), vec![speed], &seeded_config(0));
    // Longer than a frame: the runner doesn't move during the first one.
    solver.runners[0].start_delay = 0.09;

    solver.step(DELTA_TIME);
    assert!(activations(&solver).is_empty());
    assert_eq!(solver.runners[0].position, 0.0);

    // Activated before moving, on the frame she leaves the gate.
    solver.step(DELTA_TIME);
    assert_eq!(activations(&solver), vec![("speed", 0.0)]);
    assert!(solver.runners[0].position > 0.0);
    let counts = &solver.runners[0].trigger_scheduler.activation_counts;
    assert_eq!((counts.all, counts.start), (1, 1));
}

#[test]
fn test_out_of_gate_timed_effects_apply_from_first_frame() {
    // Like "Perfect Concentration": start delay before the race, then acceleration.
    let concentration = out_of_gate_skill(
        "concentration",
        3.0,
        &[
            (SkillType::MultiplyStartDelay, 0.4),
            (SkillType::Acceleration, 0.2),
        ],
    );
    let mut solver = ready_solver(
        10101,
        uma(Style::Pace),
        vec![concentration],
        &seeded_config(0),
    );
    // At most 0.1 * 0.4 seconds, within the first frame.
    assert!(solver.runners[0].start_delay <= 0.04);

    solver.step(DELTA_TIME);
    let runner = &solver.runners[0];
    assert_eq!(activations(&solver), vec![("concentration", 0.0)]);
    assert_close!(runner.skill_modifiers.acceleration, 0.2);
    // Usual duration, scaled with the course distance.
    let distance = solver.state.course.distance as f64;
    let [effect] = runner.active_timed_effects.as_slice() else {
        panic!("Expected one active effect");
    };
    assert_close!(effect.expires_at, 3.0 * distance / 1000.0);
}

#[test]
fn test_out_of_gate_effects_last_until_the_end_of_the_race() {
    // Like the style-specific Fov whites, with a cooldown of 0.
    let fov = SkillAlternative {
        base_cooldown: 0.0,
        ..out_of_gate_skill(
            "fov",
            f64::INFINITY,
            &[(SkillType::WizUp, 40.0), (SkillType::Fov, 10.0)],
        )
        .alternatives[0]
            .clone()
    };
    let fov = Skill::new("fov".to_string(), skill_data(vec![fov]));
    let mut solver = ready_solver(10101, uma(Style::Pace), vec![fov], &seeded_config(0));

    solver.run();
    let runner = &solver.runners[0];
    // Activated only once, even without cooldown.
    assert_eq!(activations(&solver), vec![("fov", 0.0)]);
    assert_close!(runner.skill_modifiers.visible_distance, 10.0);
    assert!(
        runner
            .active_timed_effects
            .iter()
            .all(|effect| effect.expires_at == f64::INFINITY)
    );
}

#[test]
fn test_out_of_gate_skill_triggers_activation_dependent_skills() {
    let speed = out_of_gate_skill("speed", f64::INFINITY, &[(SkillType::SpeedUp, 40.0)]);
    let follow = skill(
        "follow",
        "is_activate_any_skill==1",
        SkillType::TargetSpeed,
        0.15,
    );
    let mut solver = ready_solver(
        10101,
        uma(Style::Pace),
        vec![speed, follow],
        &seeded_config(0),
    );
    solver.runners[0].start_delay = 0.0;

    solver.step(DELTA_TIME);
    assert_eq!(activations(&solver), vec![("speed", 0.0), ("follow", 0.0)]);
}

#[test]
fn test_out_of_gate_style_change_is_applied_before_the_race() {
    // Like "Great Escape".
    let escape = skill(
        "escape",
        "running_style==1",
        SkillType::ChangeStyleRunaway,
        0.0,
    );
    let solver = ready_solver(10101, uma(Style::Front), vec![escape], &seeded_config(0));
    assert_eq!(solver.runners[0].adjusted.style, Style::Runaway);
}

/// Gold skill with the given condition.
fn gold_skill(id: &str, condition: &str, skill_type: SkillType, modifier: f64) -> Skill {
    let data = SkillData {
        rarity: SkillRarity::Gold,
        ..skill_data(vec![alternative(condition, skill_type, modifier)])
    };
    Skill::new(id.to_string(), data)
}

/// Sorted IDs of the skills activated by the only runner after a whole race,
/// with "Adventure of 564"-like skill activating `modifier` random gold
/// skills in the second half of the race.
fn random_gold_activations(modifier: f64, mut skills: Vec<Skill>) -> Vec<String> {
    let adventure = skill(
        "adventure",
        "distance_rate>=50",
        SkillType::ActivateRandomGold,
        modifier,
    );
    skills.push(adventure);
    let mut solver = ready_solver(10101, uma(Style::Pace), skills, &seeded_config(0));
    solver.run();

    let mut ids: Vec<_> = activations(&solver)
        .into_iter()
        .map(|(id, _)| id.to_string())
        .collect();
    ids.sort();
    ids
}

/// Gold skill that never activates on its own (there are no heals).
fn forced_gold_skill(id: &str) -> Skill {
    gold_skill(
        id,
        "is_activate_heal_skill==1",
        SkillType::TargetSpeed,
        0.15,
    )
}

#[test]
fn test_activate_random_gold_activates_one_skill_per_10000() {
    let skills = vec![
        forced_gold_skill("a"),
        forced_gold_skill("b"),
        forced_gold_skill("c"),
    ];
    let ids = random_gold_activations(2.0, skills);

    // "adventure" and two distinct gold skills.
    assert_eq!(ids.len(), 3, "{ids:?}");
    assert!(ids.contains(&"adventure".to_string()), "{ids:?}");
    assert!(ids.windows(2).all(|pair| pair[0] != pair[1]), "{ids:?}");
}

#[test]
fn test_activate_random_gold_bypasses_conditions() {
    let ids = random_gold_activations(1.0, vec![forced_gold_skill("forced")]);
    assert_eq!(ids, vec!["adventure", "forced"]);
}

#[test]
fn test_activate_random_gold_skips_activated_and_out_of_gate_skills() {
    let skills = vec![
        // Activates on its own before "adventure".
        gold_skill("early", "distance_rate>=10", SkillType::TargetSpeed, 0.15),
        // Activates before the race.
        gold_skill("green", "always==1", SkillType::SpeedUp, 40.0),
        // White skills can't be picked.
        skill(
            "white",
            "is_activate_heal_skill==1",
            SkillType::TargetSpeed,
            0.15,
        ),
        forced_gold_skill("forced"),
    ];
    // Only "forced" can be picked, although two skills should be.
    let ids = random_gold_activations(2.0, skills);
    assert_eq!(ids, vec!["adventure", "early", "forced", "green"]);
}

/// Unique skill with the given alternatives.
fn unique_skill(id: &str, alternatives: Vec<SkillAlternative>) -> Skill {
    let data = SkillData {
        rarity: SkillRarity::UniqueNatural,
        ..skill_data(alternatives)
    };
    Skill::new(id.to_string(), data)
}

/// Alternative whose effect lasts until the end of the race.
fn lasting_alternative(condition: &str, skill_type: SkillType, modifier: f64) -> SkillAlternative {
    SkillAlternative {
        base_duration: f64::INFINITY,
        ..alternative(condition, skill_type, modifier)
    }
}

/// Runner after a whole race, with a skill activating `modifier` unique
/// skills again in the second half of the race.
fn unique_reactivation_runner(modifier: f64, mut skills: Vec<Skill>) -> Runner {
    let again = skill(
        "again",
        "distance_rate>=50",
        SkillType::ActivateUnique,
        modifier,
    );
    skills.push(again);
    let mut solver = ready_solver(10101, uma(Style::Pace), skills, &seeded_config(0));
    solver.run();
    solver.runners.swap_remove(0)
}

/// Sorted IDs of the skills activated by `runner`.
fn sorted_activation_ids(runner: &Runner) -> Vec<&str> {
    let mut ids: Vec<_> = runner
        .activated_skills
        .iter()
        .map(|record| record.skill_id.as_str())
        .collect();
    ids.sort();
    ids
}

/// Number of active timed effects of the given type.
fn active_effect_count(runner: &Runner, skill_type: SkillType) -> usize {
    runner
        .active_timed_effects
        .iter()
        .filter(|timed| timed.effect.skill_type == skill_type)
        .count()
}

#[test]
fn test_activate_unique_fires_the_unique_again() {
    let unique = unique_skill(
        "unique",
        vec![lasting_alternative(
            "distance_rate>=10",
            SkillType::TargetSpeed,
            0.35,
        )],
    );
    let runner = unique_reactivation_runner(1.0, vec![unique]);
    assert_eq!(
        sorted_activation_ids(&runner),
        vec!["again", "unique", "unique"]
    );
    // Both activations' effects stack.
    assert_eq!(active_effect_count(&runner, SkillType::TargetSpeed), 2);
}

#[test]
fn test_activate_unique_bypasses_conditions() {
    let unique = unique_skill(
        "unique",
        vec![alternative(
            "is_activate_heal_skill==1",
            SkillType::TargetSpeed,
            0.35,
        )],
    );
    let runner = unique_reactivation_runner(1.0, vec![unique]);
    assert_eq!(sorted_activation_ids(&runner), vec!["again", "unique"]);
}

#[test]
fn test_activate_unique_uses_the_alternative_that_activated() {
    let unique = unique_skill(
        "unique",
        vec![
            lasting_alternative("is_activate_heal_skill==1", SkillType::TargetSpeed, 0.35),
            lasting_alternative("distance_rate>=10", SkillType::Acceleration, 0.2),
        ],
    );
    let runner = unique_reactivation_runner(1.0, vec![unique]);
    assert_eq!(active_effect_count(&runner, SkillType::TargetSpeed), 0);
    assert_eq!(active_effect_count(&runner, SkillType::Acceleration), 2);
}

#[test]
fn test_activate_unique_skips_other_and_out_of_gate_skills() {
    let never = |id| {
        unique_skill(
            id,
            vec![alternative(
                "is_activate_heal_skill==1",
                SkillType::TargetSpeed,
                0.35,
            )],
        )
    };
    let skills = vec![
        // Activates before the race.
        unique_skill(
            "green",
            vec![alternative("always==1", SkillType::SpeedUp, 40.0)],
        ),
        // Gold skills can't be picked.
        forced_gold_skill("gold"),
        never("first"),
        never("second"),
    ];
    // Only the first eligible unique activates.
    let runner = unique_reactivation_runner(1.0, skills);
    assert_eq!(
        sorted_activation_ids(&runner),
        vec!["again", "first", "green"]
    );
}
