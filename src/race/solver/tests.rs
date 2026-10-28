use super::*;
use crate::configuration::Roll;
use crate::course::Phase;
use crate::course::geometry::{Segment, Slope};
use crate::race::rush::RushState;
use crate::race::spot_struggle::SpotStruggle;
use crate::test_utils::*;
use crate::trigger::skill_activation::{OvertakeExtension, PendingEffect};
use crate::units::CourseWidths;
use std::sync::Arc;

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
        let mut solver = RaceSolver::new(load_course(course_id), race_params(), seeded_config(0));
        for style in [Style::Front, Style::Pace, Style::Late, Style::End] {
            solver.add_runner(uma(style), Mood::Normal, vec![]);
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
                    runner.segments.corner(),
                    feature_at(position, course.corners.iter().map(Segment::range)),
                    "{context}"
                );
                assert_eq!(
                    runner.segments.straight(),
                    feature_at(position, course.straights.iter().map(Segment::range)),
                    "{context}"
                );
                assert_eq!(
                    runner.segments.slope(),
                    feature_at(position, course.slopes.iter().map(Segment::range)),
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
        24,
        "remain_distance<=800&phase>=2",
        SkillType::TargetSpeed,
        0.35,
    );

    let uma = Uma {
        stamina: 1200,
        ..uma(Style::Pace)
    };
    let mut solver = ready_solver(10108, uma, vec![skill], seeded_config(0));
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
    let mut solver = RaceSolver::new(load_course(10101), race_params(), config);
    for &(style, _) in runners {
        solver.add_runner(uma(style), Mood::Normal, vec![]);
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

/// Positioning strategy of the runner at `position`.
fn strategy_at(solver: &RaceSolver<Runner>, position: f64) -> PositioningStrategy {
    let mut runners = solver.runners.iter();
    runners
        .find(|r| r.position == position)
        .unwrap()
        .positioning_strategy
}

/// Set the positioning strategy of the runner at `position`.
fn set_strategy_at(solver: &mut RaceSolver<Runner>, position: f64, strategy: PositioningStrategy) {
    let mut runners = solver.runners.iter_mut();
    runners
        .find(|r| r.position == position)
        .unwrap()
        .positioning_strategy = strategy;
}

/// Update positioning strategies many times without moving, so that wit checks
/// to enter a mode eventually pass.
fn update_positioning_repeatedly(solver: &mut RaceSolver<Runner>) {
    for _ in 0..50 {
        for runner in &mut solver.runners {
            runner.next_positioning_check = 0.0;
        }
        solver.update_positioning_strategy();
    }
}

#[test]
fn test_front_speed_up_distance_by_front_runner_count() {
    use PositioningStrategy::*;

    // A single front runner may keep speeding up while less than 12.5m ahead;
    // otherwise the limit is 4.5m. Runaways and runners acting as front runners
    // because they're rushed don't count as front runners.
    // (style behind, whether rushed and acting as a front runner, expected).
    let cases = [
        (Style::Pace, false, FrontSpeedUp),
        (Style::Runaway, false, FrontSpeedUp),
        (Style::Pace, true, FrontSpeedUp),
        (Style::Front, false, Normal),
    ];
    for (behind, rushed, expected) in cases {
        let mut solver = positioning_solver(
            &[(Style::Front, 108.0), (behind, 100.0)],
            GameVersion::LATEST,
        );
        if rushed {
            set_rushed(&mut solver, 1);
            let rush_state = solver.runners[1].rush_state.as_mut().unwrap();
            rush_state.style = Style::Front;
        }
        set_strategy_at(&mut solver, 108.0, FrontSpeedUp);
        solver.update_positioning_strategy();
        assert_eq!(
            strategy_at(&solver, 108.0),
            expected,
            "{behind:?} behind (rushed: {rushed})"
        );
    }
}

#[test]
fn test_front_speed_up_only_exits_when_far_ahead() {
    use PositioningStrategy::*;

    // Overtaken: stays (before 1.5 Anniv., so that Pace Up Ex doesn't apply).
    let mut solver = positioning_solver(
        &[(Style::Front, 100.0), (Style::Pace, 110.0)],
        GameVersion::FirstAnniversary,
    );
    set_strategy_at(&mut solver, 100.0, FrontSpeedUp);
    solver.update_positioning_strategy();
    assert_eq!(strategy_at(&solver, 100.0), FrontSpeedUp);

    // First by less than 4.5m: stays; by more: exits.
    for (lead, expected) in [(4.0, FrontSpeedUp), (5.0, Normal)] {
        let mut solver = positioning_solver(
            &[(Style::Front, 100.0 + lead), (Style::Pace, 100.0)],
            GameVersion::FirstAnniversary,
        );
        set_strategy_at(&mut solver, 100.0 + lead, FrontSpeedUp);
        solver.update_positioning_strategy();
        assert_eq!(
            strategy_at(&solver, 100.0 + lead),
            expected,
            "{lead}m ahead"
        );
    }
}

#[test]
fn test_lone_front_runner_never_speeds_up() {
    use PositioningStrategy::*;

    let mut solver = positioning_solver(&[(Style::Front, 100.0)], GameVersion::LATEST);
    update_positioning_repeatedly(&mut solver);
    assert_eq!(strategy_at(&solver, 100.0), Normal);

    set_strategy_at(&mut solver, 100.0, FrontSpeedUp);
    solver.update_positioning_strategy();
    assert_eq!(strategy_at(&solver, 100.0), Normal);
}

#[test]
fn test_front_overtake_when_not_first_in_strategy() {
    use PositioningStrategy::*;

    // Before 1.5 Anniv., so that Pace Up Ex doesn't apply. The Pace runner is
    // first overall, so no front runner can speed up.
    let mut solver = positioning_solver(
        &[
            (Style::Pace, 120.0),
            (Style::Front, 110.0),
            (Style::Front, 100.0),
        ],
        GameVersion::FirstAnniversary,
    );
    update_positioning_repeatedly(&mut solver);
    // First among front runners.
    assert_eq!(strategy_at(&solver, 110.0), Normal);
    assert_eq!(strategy_at(&solver, 100.0), FrontOvertake);

    // Runaways are a different strategy: the Front runner is first in hers.
    let mut solver = positioning_solver(
        &[(Style::Runaway, 150.0), (Style::Front, 110.0)],
        GameVersion::FirstAnniversary,
    );
    update_positioning_repeatedly(&mut solver);
    assert_eq!(strategy_at(&solver, 110.0), Normal);
}

#[test]
fn test_front_overtake_exits_when_far_ahead_in_strategy() {
    use PositioningStrategy::*;

    // (style, lead over the next runner with the same strategy, expected). The
    // Pace runner ahead shows that the lead only counts the same strategy.
    let cases = [
        (Style::Front, 9.0, FrontOvertake),
        (Style::Front, 11.0, Normal),
        (Style::Runaway, 27.0, FrontOvertake),
        (Style::Runaway, 28.0, Normal),
    ];
    for (style, lead, expected) in cases {
        let mut solver = positioning_solver(
            &[(Style::Pace, 200.0), (style, 100.0 + lead), (style, 100.0)],
            GameVersion::FirstAnniversary,
        );
        set_strategy_at(&mut solver, 100.0 + lead, FrontOvertake);
        solver.update_positioning_strategy();
        assert_eq!(
            strategy_at(&solver, 100.0 + lead),
            expected,
            "{style:?} {lead}m ahead"
        );
    }
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
    let mut solver = ready_solver(10101, uma(Style::Pace), vec![], seeded_config(rng_seed));
    solver.runners[0].start_delay = start_delay;
    solver
}

/// `(position, speed, hp)` of the only runner.
fn runner_state(solver: &RaceSolver<Runner>) -> (f64, f64, f64) {
    let runner = &solver.runners[0];
    (runner.position, runner.speed, runner.hp.current())
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
    let mut solver = RaceSolver::new(load_course(10204), race_params(), seeded_config(0));
    for _ in 0..9 {
        // Low wit for a high rush chance.
        let uma = Uma {
            wit: 100,
            ..uma(Style::Late)
        };
        solver.add_runner(uma, Mood::Normal, vec![]);
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
    let mut solver = RaceSolver::new(load_course(10101), race_params(), seeded_config(0));
    for (i, style) in [Style::Front, Style::Pace, Style::Late, Style::End]
        .into_iter()
        .enumerate()
    {
        let uma = Uma {
            speed: 1000 + 50 * i as u16,
            ..uma(style)
        };
        solver.add_runner(uma, Mood::Normal, vec![]);
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
    runner.lane.current = CourseWidths(lane);
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
    let runner = &mut runners[i];
    runner.segments.update(runner.position, &state.course);
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
    hp.set_current(hp.maximum() * percent / 100.0);
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
    // Order rates count every gate: make it 4, one per runner.
    let duel_solver = |runners| {
        let mut solver = duel_solver(runners, GameVersion::LATEST);
        solver.state.params.gates = 4;
        solver
    };

    // 2nd and 3rd out of 4: top 50% and top 75%.
    let mut solver = duel_solver(&[(1100.0, 0.1), (1050.0, 0.1), (1048.0, 0.1), (1000.0, 0.1)]);
    update_duels(&mut solver, DUEL_FRAMES);
    assert_eq!(duelists(&solver), [false, true, true, false]);

    // 3rd and 4th out of 4: top 75% and top 100%.
    let mut solver = duel_solver(&[(1100.0, 0.1), (1050.0, 0.1), (1000.0, 0.1), (998.0, 0.1)]);
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
        runner.lane.current = CourseWidths(lane);
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
    runner.lane.current = CourseWidths(0.5);
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
fn test_skill_target_lane_in_course_widths() {
    let mut solver = positioning_solver(&[(Style::Front, 100.0)], GameVersion::LATEST);
    let runner = &mut solver.runners[0];
    runner.lane = crate::race::runner::LaneState::new(CourseWidths(0.4));
    runner.skill_modifiers.target_lane = Some(CourseWidths(0.5));
    // Fixed mode.
    solver.update_blocking_status(DELTA_TIME);
    assert_eq!(solver.runners[0].lane.target, CourseWidths(0.5));

    let RaceSolver { runners, state, .. } = &mut solver;
    let runner = &mut runners[0];
    for _ in 0..2000 {
        runner.update_lane(&state.course, DELTA_TIME);
    }
    assert_close!(runner.lane.current.0, 0.5);
}

#[test]
fn test_move_lane_modifier_requires_lane_movement() {
    let mut solver = positioning_solver(&[(Style::Front, 100.0)], GameVersion::LATEST);
    let RaceSolver { runners, state, .. } = &mut solver;
    let runner = &mut runners[0];
    runner.skill_modifiers.lane_speed = 0.1;
    runner.lane.current = CourseWidths(0.4);
    runner.lane.target = CourseWidths(0.4);

    runner.update_lane(&state.course, DELTA_TIME);
    assert_eq!(runner.lane.speed, 0.0);
    runner.update_target_speed(state);
    let still_speed = runner.target_speed;

    runner.lane.target = CourseWidths(0.7);
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
    runner.lane.current = CourseWidths(0.4);
    runner.skill_modifiers.lane_speed = 0.1;

    // Moving out: (current speed + skill modifier) course widths per second.
    runner.lane.target = CourseWidths(0.7);
    runner.update_lane(&state.course, DELTA_TIME);
    let current_speed = runner.lane.current_speed;
    assert_close!(current_speed, 0.03 * DELTA_TIME);
    assert_close!(runner.lane.speed, current_speed + 0.1);
    let moved = runner.lane.current.0 - 0.4;
    assert_close!(moved, runner.lane.speed * DELTA_TIME);

    // Moving in: faster the further out the runner is.
    runner.lane.target = CourseWidths(0.0);
    let lane = runner.lane.current;
    runner.update_lane(&state.course, DELTA_TIME);
    let direction_modifier = -(1.0 + lane.0);
    let expected = (runner.lane.current_speed + 0.1) * direction_modifier;
    assert_close!(runner.lane.speed, expected);
}

#[test]
fn test_lane_change_reaching_target_resets_current_speed() {
    let mut solver = positioning_solver(&[(Style::Front, 100.0)], GameVersion::LATEST);
    let RaceSolver { runners, state, .. } = &mut solver;
    let runner = &mut runners[0];
    runner.lane.current = CourseWidths(0.4);
    runner.lane.target = CourseWidths(0.401);
    runner.lane.current_speed = 0.02;

    runner.update_lane(&state.course, DELTA_TIME);
    assert_eq!(runner.lane.current, CourseWidths(0.401));
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
    runner.lane.current = CourseWidths(0.4);
    runner.lane.target = CourseWidths(0.7);
    runner.blocked.side_out = true;

    runner.update_lane(&state.course, DELTA_TIME);
    runner.update_lane(&state.course, DELTA_TIME);
    assert_eq!(runner.lane.current, CourseWidths(0.4));
    assert_eq!(runner.lane.speed, 0.0);
    assert_close!(runner.lane.current_speed, 0.03 * 2.0 * DELTA_TIME);

    // Blocked only towards the outside, so moving in is still possible.
    runner.lane.target = CourseWidths(0.0);
    runner.update_lane(&state.course, DELTA_TIME);
    assert!(runner.lane.current < CourseWidths(0.4));
    assert!(runner.lane.speed < 0.0);
}

#[test]
fn test_normal_mode_lane_targets_in_course_widths() {
    let mut solver = positioning_solver(&[(Style::Front, 100.0)], GameVersion::LATEST);
    let target = |solver: &RaceSolver<Runner>| {
        solver.runners[0]
            .normal_mode_lane_decision(&solver.state.course, &solver.runners)
            .target
            .0
    };

    // Move inward by 0.05 course widths in the early phase.
    let runner = &mut solver.runners[0];
    runner.phase = Phase::Early;
    runner.lane.current = CourseWidths(0.4);
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
    assert_close!(
        ahead.lane.current.0,
        0.19 + CourseWidths::from_horse_lanes(0.4).0
    );
    assert_eq!(ahead.lane.target, ahead.lane.current);
    assert!(ahead.should_update_target_lane());
    assert_eq!(behind.lane.current, CourseWidths(0.19));
}

#[test]
fn test_overlap_bumps_outer_runner_behind() {
    let mut solver = struggle_solver(
        &[(Style::Front, 200.0, 0.19), (Style::Front, 199.8, 0.2)],
        GameVersion::LATEST,
    );
    solver.resolve_overlaps();
    let (ahead, behind) = (&solver.runners[0], &solver.runners[1]);
    assert_eq!(ahead.lane.current, CourseWidths(0.19));
    assert_close!(
        behind.lane.current.0,
        0.19 + CourseWidths::from_horse_lanes(0.4).0
    );
    assert_eq!(behind.lane.target, behind.lane.current);
    assert!(behind.should_update_target_lane());
}

#[test]
fn test_no_bump_without_overlap() {
    // Too far apart.
    let mut solver = struggle_solver(
        &[(Style::Front, 200.0, 0.2), (Style::Front, 199.6, 0.19)],
        GameVersion::LATEST,
    );
    solver.resolve_overlaps();
    assert_eq!(solver.runners[0].lane.current, CourseWidths(0.2));
    assert_eq!(solver.runners[1].lane.current, CourseWidths(0.19));

    // Too far apart laterally.
    let mut solver = struggle_solver(
        &[(Style::Front, 200.0, 0.215), (Style::Front, 199.8, 0.19)],
        GameVersion::LATEST,
    );
    solver.resolve_overlaps();
    assert_eq!(solver.runners[0].lane.current, CourseWidths(0.215));
    assert_eq!(solver.runners[1].lane.current, CourseWidths(0.19));
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
    solver.runners[0].lane.target = CourseWidths(0.0);

    solver.resolve_overlaps();
    solver.update_blocking_status(DELTA_TIME);
    let bumped = &solver.runners[0];
    assert!(bumped.blocked.side_in);
    // Too close to the runner inside in mid-race, so she targets 2 horse lanes
    // away from her. Being on her target lane after the bump forces the update.
    let expected = 0.19 + CourseWidths::from_horse_lanes(2.0).0;
    assert_close!(bumped.lane.target.0, expected);
}

/// Mid-race solver with a runner 30m ahead of another in the same lane, slower
/// and aiming slower, so she can be caught within 15 seconds.
fn far_ahead_solver(visible_distance: f64) -> RaceSolver<Runner> {
    let mut solver = struggle_solver(
        &[(Style::Pace, 230.0, 0.2), (Style::Pace, 200.0, 0.2)],
        GameVersion::LATEST,
    );
    for runner in &mut solver.runners {
        runner.phase = Phase::Mid;
    }
    for (runner, speed) in solver.runners.iter_mut().zip([15.0, 18.0]) {
        runner.speed = speed;
        runner.target_speed = speed;
    }
    solver.runners[1].skill_modifiers.visible_distance = visible_distance;
    solver
}

#[test]
fn test_runners_beyond_base_field_of_view_are_not_seen() {
    let mut solver = far_ahead_solver(0.0);

    solver.update_blocking_status(DELTA_TIME);
    assert_eq!(solver.runners[1].visible_count, 0);
    assert!(!solver.runners[1].is_in_overtake_mode());
    assert_eq!(solver.runners[0].being_overtake_target_time, 0.0);
}

#[test]
fn test_closest_front_blocker_is_overtake_target() {
    // Less than 1m ahead and not slower, so only a target as the closest blocker.
    let mut solver = struggle_solver(
        &[
            (Style::Pace, 201.5, 0.2),
            (Style::Pace, 200.5, 0.2),
            (Style::Pace, 200.0, 0.2),
        ],
        GameVersion::LATEST,
    );
    for runner in &mut solver.runners {
        runner.speed = 15.0;
        runner.target_speed = 15.0;
    }

    solver.update_blocking_status(DELTA_TIME);
    let gate_roll = solver.runners[1].gate_roll;
    let behind = &solver.runners[2];
    assert_eq!(behind.blocked.front_blocker, Some(gate_roll));
    assert!(behind.is_in_overtake_mode());
    assert_close!(solver.runners[1].being_overtake_target_time, DELTA_TIME);
    // The runner furthest ahead also blocks both runners behind, but she's only
    // the closest blocker of the one right behind her.
    assert_close!(solver.runners[0].being_overtake_target_time, DELTA_TIME);
}

#[test]
fn test_fov_bonus_extends_visibility_and_overtake_targets() {
    // Like a +15m Fov skill, so the field of view reaches 35m.
    let mut solver = far_ahead_solver(15.0);

    solver.update_blocking_status(DELTA_TIME);
    assert_eq!(solver.runners[1].visible_count, 1);
    assert!(solver.runners[1].is_in_overtake_mode());
    assert_close!(solver.runners[0].being_overtake_target_time, DELTA_TIME);
}

#[test]
fn test_fov_bonus_narrows_field_of_view_at_a_given_distance() {
    // 10m ahead, 3 horse lanes apart: within the 20m field of view, whose half
    // width there is 3.875 horse lanes, but not within a 40m one (2.4375).
    let mut solver = struggle_solver(
        &[
            (Style::Pace, 210.0, CourseWidths::from_horse_lanes(3.0).0),
            (Style::Pace, 200.0, 0.0),
        ],
        GameVersion::LATEST,
    );
    assert!(solver.runners[1].is_visible(&solver.runners[0]));

    solver.runners[1].skill_modifiers.visible_distance = 20.0;
    assert!(!solver.runners[1].is_visible(&solver.runners[0]));
}

#[test]
fn test_fov_debuffs_cannot_shorten_field_of_view_below_zero() {
    // Three stacked -10m debuffs.
    let mut solver = far_ahead_solver(-30.0);
    solver.runners[0].position = solver.runners[1].position;

    assert_eq!(solver.runners[1].visible_distance(), 0.0);
    assert!(!solver.runners[1].is_visible(&solver.runners[0]));
    solver.update_blocking_status(DELTA_TIME);
    assert_eq!(solver.runners[1].visible_count, 0);
}

#[test]
fn test_individual_runners_are_all_opponents() {
    let mut solver = RaceSolver::new(load_course(10101), race_params(), seeded_config(0));
    for _ in 0..3 {
        solver.add_runner(uma(Style::Pace), Mood::Normal, vec![]);
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
    let mut solver = RaceSolver::new(load_course(10101), race_params(), seeded_config(0));
    let member = || (uma(Style::Pace), Mood::Normal, vec![]);
    solver.add_team(vec![member(), member(), member()]);
    solver.add_runner(uma(Style::Late), Mood::Normal, vec![]);
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
    let mut solver = RaceSolver::new(load_course(10101), race_params(), seeded_config(0));
    let member = || (uma(Style::Pace), Mood::Normal, vec![]);
    solver.add_team(vec![member(), member(), member(), member()]);
}

#[test]
#[should_panic(expected = "Team must have between 1 and 3 runners")]
fn test_empty_team_panics() {
    let mut solver = RaceSolver::new(load_course(10101), race_params(), seeded_config(0));
    solver.add_team(vec![]);
}

#[test]
#[should_panic(expected = "Invalid gates 0")]
fn test_no_gates_panics() {
    let params = RaceParameters {
        gates: 0,
        ..race_params()
    };
    RaceSolver::new(load_course(10101), params, seeded_config(0));
}

#[test]
#[should_panic(expected = "Invalid gates 19")]
fn test_too_many_gates_panics() {
    let params = RaceParameters {
        gates: MAX_GATES + 1,
        ..race_params()
    };
    RaceSolver::new(load_course(10101), params, seeded_config(0));
}

#[test]
#[should_panic(expected = "Cannot add more runners than available gates")]
fn test_runner_exceeding_gates_panics() {
    // 9 gates.
    let mut solver = RaceSolver::new(load_course(10101), race_params(), seeded_config(0));
    for _ in 0..10 {
        solver.add_runner(uma(Style::Pace), Mood::Normal, vec![]);
    }
}

#[test]
#[should_panic(expected = "Cannot add more runners than available gates")]
fn test_team_exceeding_gates_panics() {
    // 9 gates: 8 individual runners leave room for one more, not a team of two.
    let mut solver = RaceSolver::new(load_course(10101), race_params(), seeded_config(0));
    for _ in 0..8 {
        solver.add_runner(uma(Style::Pace), Mood::Normal, vec![]);
    }
    let member = || (uma(Style::Pace), Mood::Normal, vec![]);
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
    let mut solver = RaceSolver::new(load_course(10101), race_params(), seeded_config(0));
    let member = || (uma(Style::Pace), Mood::Normal, vec![]);
    solver.add_team(vec![member(), member()]);
    solver.add_runner(uma(Style::Pace), Mood::Normal, vec![]);
    solver.add_runner(uma(Style::Pace), Mood::Normal, vec![]);
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
    let mut solver = RaceSolver::new(load_course(10204), race_params(), seeded_config(0));
    // Low wit for a high rush chance.
    let member = |style| {
        let uma = Uma {
            wit: 100,
            ..uma(style)
        };
        (uma, Mood::Normal, vec![])
    };
    solver.add_team(vec![
        member(Style::Late),
        member(Style::Late),
        member(Style::Pace),
    ]);
    solver.add_team(vec![member(Style::Late), member(Style::End)]);
    for style in [Style::Runaway, Style::Runaway, Style::Front, Style::Pace] {
        let (uma, mood, skills) = member(style);
        solver.add_runner(uma, mood, skills);
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
    let mut solver = RaceSolver::new(load_course(10101), race_params(), seeded_config(0));
    let member = || (uma(Style::Pace), Mood::Normal, vec![]);
    solver.add_team(vec![member(), member()]);
    solver.add_runner(uma(Style::Pace), Mood::Normal, vec![]);
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
    let mut solver = RaceSolver::new(load_course(10101), race_params(), seeded_config(0));
    let skills = || {
        vec![
            // Out-of-gate.
            skill(32, "always==1", SkillType::SpeedUp, 80.0),
            skill(25, "phase==1", SkillType::TargetSpeed, 0.1),
            skill(24, "phase>=2", SkillType::TargetSpeed, 0.1),
        ]
    };
    let member = |style| (uma(style), Mood::Normal, skills());
    solver.add_team(vec![member(Style::Front), member(Style::Late)]);
    solver.add_runner(uma(Style::Pace), Mood::Normal, skills());
    solver.add_runner(uma(Style::End), Mood::Normal, vec![]);
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

/// Ready race with one runner per `(style, team)`, placed in that order from
/// first to last, 10 meters apart.
fn targeting_solver(runners: &[(Style, u8)]) -> RaceSolver<Runner> {
    let mut solver = RaceSolver::new(load_course(10101), race_params(), seeded_config(0));
    for _ in runners {
        solver.add_runner(uma(Style::Late), Mood::Normal, vec![]);
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
fn test_random_team_member_targets_one_ally_or_oneself() {
    // Runners 1 and 3 are allies of runner 2.
    let solver = targeting_solver(&[
        (Style::Late, 0),
        (Style::Late, 1),
        (Style::Late, 1),
        (Style::Late, 1),
    ]);
    let mut seen = [false; 4];
    for _ in 0..100 {
        let selected = targets(&solver, 2, SkillTarget::RandomAlly);
        assert_eq!(selected.len(), 1);
        seen[selected[0]] = true;
    }
    assert_eq!(seen, [false, true, true, true]);
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
fn test_targets_oneself_and_nobody_by_uma_id() {
    let solver = targeting_solver(&[(Style::Late, 0), (Style::Late, 1)]);
    assert_eq!(targets(&solver, 1, SkillTarget::Oneself), [1]);
    assert_eq!(
        targets(&solver, 1, SkillTarget::UmaId { value: 1001 }),
        [] as [usize; 0]
    );
}

#[test]
fn test_targets_runners_ahead_in_field_of_view() {
    // Runners 40m, 30m, 20m and 10m ahead of runner 4, all in the same lane.
    let mut solver = targeting_solver(&[0, 1, 2, 3, 4, 5].map(|team| (Style::Late, team)));
    for runner in &mut solver.runners {
        runner.lane.current = CourseWidths(0.0);
    }

    // Within the base 20m field of view, nearest first; not runners behind.
    let in_fov = |max| SkillTarget::InFov { max };
    assert_eq!(targets(&solver, 4, in_fov(5)), [3, 2]);
    assert_eq!(targets(&solver, 4, in_fov(1)), [3]);

    // Out of the field of view once far enough to the side.
    solver.runners[3].lane.current = CourseWidths::from_horse_lanes(5.0);
    assert_eq!(targets(&solver, 4, in_fov(5)), [2]);
}

#[test]
fn test_targets_rushed_runners() {
    let mut solver = targeting_solver(&[
        (Style::Runaway, 0),
        (Style::Pace, 1),
        (Style::Front, 2),
        (Style::Late, 3),
        (Style::Pace, 4),
        (Style::Front, 5),
    ]);
    for i in [0, 2, 4, 5] {
        set_rushed(&mut solver, i);
    }

    // Nearest first.
    let ahead = |max| SkillTarget::RushedAhead { max };
    assert_eq!(targets(&solver, 3, ahead(5)), [2, 0]);
    assert_eq!(targets(&solver, 3, ahead(1)), [2]);
    let behind = |max| SkillTarget::RushedBehind { max };
    assert_eq!(targets(&solver, 3, behind(5)), [4, 5]);
    assert_eq!(targets(&solver, 3, behind(1)), [4]);

    // Runaways count as Front runners; runner 1 isn't rushed.
    let with_style = |style| SkillTarget::RushedStrategy { style };
    assert_eq!(targets(&solver, 3, with_style(Style::Front)), [0, 2, 5]);
    assert_eq!(targets(&solver, 3, with_style(Style::Pace)), [4]);
}

#[test]
fn test_scoreboard_time() {
    let mut solver = ready_solver(10101, uma(Style::Pace), vec![], seeded_config(0));
    let min = solver.state.course.finish_time_min;
    let max = solver.state.course.finish_time_max;

    // Not finished.
    assert_eq!(solver.scoreboard_time(), None);

    // Within the course range: scaled as is.
    let finish_time = (min + max) / 2.0 / FINISH_TIME_SCALE;
    solver.state.finish_time = Some(finish_time);
    assert_close!(
        solver.scoreboard_time().unwrap(),
        finish_time * FINISH_TIME_SCALE
    );

    // Out of range: clamped, plus a random offset of up to 1 second.
    for (finish_time, bound) in [(min / FINISH_TIME_SCALE - 5.0, min), (max, max)] {
        solver.state.finish_time = Some(finish_time);
        let time = solver.scoreboard_time().unwrap();
        assert!(
            (bound..bound + 1.0).contains(&time),
            "finish time {finish_time}: expected {bound} plus up to 1s, got {time}"
        );
    }
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
            target: SkillTarget::BehindSelf { count: 1 },
            ..effect(SkillType::CurrentSpeed, -0.15)
        },
        duration: 3.0,
        wit_bonus: 0.0,
    });
    solver.apply_pending_effects();

    assert!(solver.runners[0].pending_effects.is_empty());
    assert_close!(solver.runners[0].skill_modifiers.current_speed, 0.0);
    assert_close!(solver.runners[1].skill_modifiers.current_speed, -0.15);
    assert_close!(solver.runners[2].skill_modifiers.current_speed, 0.0);
}

#[test]
fn test_activated_effects_on_others_are_queued_for_targets() {
    // A buff on oneself, a debuff on the runner behind, and a pre-race effect on
    // everyone (applied before the race, if at all, so never queued).
    let mut alternative = alternative("always==1", SkillType::TargetSpeed, 0.1);
    alternative.effects.extend([
        SkillEffect {
            target: SkillTarget::BehindSelf { count: 1 },
            ..effect(SkillType::CurrentSpeed, -0.15)
        },
        SkillEffect {
            target: SkillTarget::All,
            ..effect(SkillType::SpeedUp, -60.0)
        },
    ]);
    let debuff = Arc::new(Skill {
        id: 8,
        ..synthetic_skill(vec![alternative])
    });

    let mut solver = RaceSolver::new(load_course(10101), race_params(), seeded_config(0));
    solver.add_runner(uma(Style::Late), Mood::Normal, vec![debuff]);
    solver.add_runner(uma(Style::Late), Mood::Normal, vec![]);
    let mut solver = solver.ready_runners();
    // The caster is ahead.
    for runner in &mut solver.runners {
        runner.position = if runner.available_skills().is_empty() {
            90.0
        } else {
            100.0
        };
    }
    sort_by_position(&mut solver);

    solver.runners[0].activate_skill(0, 0, &solver.state);

    // Only the debuff is queued, with its duration scaled to the 1200m course.
    let caster = &solver.runners[0];
    let queued: Vec<_> = caster
        .pending_effects
        .iter()
        .map(|pending| (pending.effect.skill_type, pending.duration))
        .collect();
    assert_eq!(queued.len(), 1);
    assert_eq!(queued[0].0, SkillType::CurrentSpeed);
    assert_close!(queued[0].1, 3.0 * 1.2);
    // The caster only gets her own buff, and only it counts as one.
    assert_close!(caster.skill_modifiers.target_speed, 0.1);
    assert_close!(caster.skill_modifiers.current_speed, 0.0);
    let buffs = &caster.activation_flags.buff_activated;
    assert_eq!(buffs.iter().collect::<Vec<_>>(), [&SkillType::TargetSpeed]);

    solver.apply_pending_effects();
    assert!(solver.runners[0].pending_effects.is_empty());
    assert_close!(solver.runners[1].skill_modifiers.current_speed, -0.15);
}

/// Effect of the given type, lasting 3 seconds, as received from another
/// runner.
fn received(skill_type: SkillType, modifier: f64) -> PendingEffect {
    PendingEffect {
        effect: SkillEffect {
            target: SkillTarget::All,
            ..effect(skill_type, modifier)
        },
        duration: 3.0,
        wit_bonus: 0.0,
    }
}

#[test]
fn test_lane_and_duration_effects_set_skill_modifiers() {
    let mut solver = ready_solver(10101, uma(Style::Late), vec![], seeded_config(0));
    let effects = [
        (SkillType::LaneMovementSpeed, 0.02),
        (SkillType::LaneMovementSpeed, 0.03),
        (SkillType::LaneChange, 0.5),
        (SkillType::ExtendEvolvedDuration, 2.0),
        (SkillType::ExtendEvolvedDuration, 1.5),
    ];
    for (skill_type, modifier) in effects {
        solver.runners[0].receive_skill_effect(&received(skill_type, modifier), &solver.state);
    }

    // Lane speed adds up, duration scaling multiplies.
    let mods = &solver.runners[0].skill_modifiers;
    assert_close!(mods.lane_speed, 0.05);
    assert_eq!(mods.target_lane, Some(CourseWidths(0.5)));
    assert_close!(mods.duration_scaling, 3.0);

    // Back to defaults once they expire.
    solver.runners[0].update_skill_durations(3.0);
    let mods = &solver.runners[0].skill_modifiers;
    assert_close!(mods.lane_speed, 0.0);
    assert_eq!(mods.target_lane, None);
    assert_close!(mods.duration_scaling, 1.0);
}

#[test]
fn test_duration_scaling_extends_activated_skills() {
    let skill = skill(32, "always==1", SkillType::TargetSpeed, 0.15);
    let mut solver = ready_solver(10101, uma(Style::Late), vec![skill], seeded_config(0));
    let extend = received(SkillType::ExtendEvolvedDuration, 2.0);
    solver.runners[0].receive_skill_effect(&extend, &solver.state);

    solver.runners[0].activate_skill(0, 0, &solver.state);

    // 3s base duration, scaled to the 1200m course, then doubled.
    let speed_effect = solver.runners[0]
        .active_timed_effects
        .iter()
        .find(|timed| timed.effect.skill_type == SkillType::TargetSpeed)
        .unwrap();
    assert_close!(speed_effect.expires_at, 3.0 * 1.2 * 2.0);
}

#[test]
fn test_rush_time_up_extends_rush_of_runners_bound_to_rush() {
    let mut solver = targeting_solver(&[(Style::Late, 0), (Style::Late, 1)]);
    set_rushed(&mut solver, 0);
    // Not bound to rush (whatever was rolled when readying the race).
    solver.runners[1].rush_state = None;
    let extend = received(SkillType::RushTimeUp, 5.0);

    for runner in &mut solver.runners {
        runner.receive_skill_effect(&extend, &solver.state);
    }

    let max_duration = solver.runners[0].rush_state.as_ref().unwrap().max_duration;
    assert_close!(max_duration, 12.0 + 5.0);
    // Runners not bound to rush are unaffected.
    assert!(solver.runners[1].rush_state.is_none());
}

/// Run a race with one runner per skill list and return, for each list, whether
/// the runner activated `skill_id`.
fn activates(skills: Vec<Vec<Arc<Skill>>>, skill_id: i32) -> Vec<bool> {
    activates_with_most_popular(skills, None, skill_id)
}

/// Same as [`activates`], making the runner of the list at index `most_popular`
/// (if any) the most popular one. Skill lists must be distinct.
fn activates_with_most_popular(
    skills: Vec<Vec<Arc<Skill>>>,
    most_popular: Option<usize>,
    skill_id: i32,
) -> Vec<bool> {
    let mut solver = RaceSolver::new(load_course(10101), race_params(), seeded_config(0));
    let ids: Vec<Vec<i32>> = skills
        .iter()
        .map(|skills| skills.iter().map(|skill| skill.id).collect())
        .collect();
    for skills in skills {
        solver.add_runner(uma(Style::Late), Mood::Normal, skills);
    }
    let mut solver = solver.ready_runners();

    // Runners are built in gate order, so find each one by her skills.
    let has_skills = |runner: &Runner, ids: &[i32]| {
        runner
            .available_skills()
            .iter()
            .map(|skill| &skill.id)
            .eq(ids.iter())
    };
    if let Some(most_popular) = most_popular {
        for runner in &mut solver.runners {
            let is_most_popular = has_skills(runner, &ids[most_popular]);
            runner.initial_state.popularity_rank = if is_most_popular { 1 } else { 2 };
        }
    }
    solver.run();

    ids.iter()
        .map(|ids| {
            let runner = solver
                .runners
                .iter()
                .find(|runner| has_skills(runner, ids))
                .unwrap();
            runner.activated_mask(skill_id) != 0
        })
        .collect()
}

/// Heal skill activating 5 seconds into the race.
fn heal() -> Arc<Skill> {
    skill(22, "accumulatetime>=5", SkillType::Recovery, 0.055)
}

#[test]
fn test_other_runner_advantage_skill_triggers_skill() {
    let react = || {
        skill(
            29,
            "is_other_character_activate_advantage_skill==9",
            SkillType::TargetSpeed,
            0.35,
        )
    };
    // Own heals don't count.
    assert_eq!(
        activates(vec![vec![heal()], vec![react()], vec![heal(), react()]], 29),
        [false, true, true]
    );
    assert_eq!(activates(vec![vec![heal(), react()]], 29), [false]);
}

#[test]
fn test_other_runner_advantage_skill_is_only_seen_on_its_frame() {
    // The heal activates long before the rest of the condition is true.
    let react = skill(
        29,
        "is_other_character_activate_advantage_skill==9&accumulatetime>=20",
        SkillType::TargetSpeed,
        0.35,
    );
    assert_eq!(
        activates(vec![vec![heal()], vec![react]], 29),
        [false, false]
    );
}

#[test]
fn test_most_popular_runner_skill_triggers_skill() {
    let react = |value| {
        skill(
            29,
            &format!("is_popularity_top_character_activate_advantage_skill=={value}"),
            SkillType::TargetSpeed,
            0.35,
        )
    };
    // Debuffs only count as any skill.
    let debuff = || skill(8, "accumulatetime>=5", SkillType::TargetSpeed, -0.1);
    let react_to = |top: Arc<Skill>, value, most_popular| {
        activates_with_most_popular(vec![vec![top], vec![react(value)]], Some(most_popular), 29)
    };
    assert_eq!(react_to(heal(), 9, 0), [false, true]);
    assert_eq!(react_to(heal(), -1, 0), [false, true]);
    assert_eq!(react_to(debuff(), 9, 0), [false, false]);
    assert_eq!(react_to(debuff(), -1, 0), [false, true]);
    // Only the most popular runner counts...
    assert_eq!(react_to(heal(), 9, 1), [false, false]);
    // ...even if she's the runner herself.
    assert_eq!(
        activates_with_most_popular(vec![vec![heal(), react(9)]], Some(0), 29),
        [true]
    );
}

#[test]
fn test_same_skill_horse_count_counts_runners_with_the_skill() {
    // Resolved when the runner is created, for the skill of the logic.
    let pair = || {
        skill(
            37,
            "same_skill_horse_count==2",
            SkillType::TargetSpeed,
            0.15,
        )
    };
    assert_eq!(
        activates(vec![vec![pair()], vec![pair()]], 37),
        [true, true]
    );
    assert_eq!(activates(vec![vec![pair()], vec![]], 37), [false, false]);
}

#[test]
fn test_ignored_popularity_satisfies_popularity_conditions() {
    let ignore = || skill(23, "always==1", SkillType::IgnorePopularity, 1.0);
    // During the race and out of the gate.
    let popular = || skill(27, "popularity>=10", SkillType::TargetSpeed, 0.15);
    let green = || skill(18, "popularity>=10", SkillType::SpeedUp, 40.0);
    assert_eq!(activates(vec![vec![popular(), green()]], 27), [false]);
    assert_eq!(activates(vec![vec![popular(), green()]], 18), [false]);
    assert_eq!(activates(vec![vec![ignore(), popular()]], 27), [true]);
    assert_eq!(activates(vec![vec![ignore(), green()]], 18), [true]);
    // Whatever the order of the skills.
    assert_eq!(activates(vec![vec![green(), ignore()]], 18), [true]);
}

#[test]
fn test_order_rates_count_every_gate() {
    let mut solver = ready_solver(10101, uma(Style::Late), vec![], seeded_config(0));
    solver.step(DELTA_TIME);

    // A single runner, but every gate counts: 6% to 16% of 9 round to 1st place.
    let order = &solver.runners[0].order;
    assert_eq!(solver.state.params.gates, 9);
    assert_eq!(order.runner_count, 9);
    assert_eq!(order.rates, 6..=16);
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
    let skill = Arc::new(synthetic_skill(alternatives));
    let mut solver = ready_solver(10101, uma(Style::Late), vec![skill], seeded_config(0));
    solver.run();

    let mask = solver.runners[0].activated_alternatives[0];
    (0..u8::BITS as usize)
        .filter(|&alternative| mask & (1 << alternative) != 0)
        .collect()
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

/// Mask of activated alternatives of a skill whose first alternative is a stat
/// boost (out of gate) with the given condition, continued during the race by
/// a target speed alternative (like "Gambit").
fn out_of_gate_continuation_mask(condition: &str) -> u8 {
    let alternatives = vec![
        alternative(condition, SkillType::SpeedUp, 40.0),
        alternative(
            "is_activate_other_skill_detail==1&accumulatetime>=10",
            SkillType::TargetSpeed,
            0.15,
        ),
    ];
    let skill = Arc::new(synthetic_skill(alternatives));
    let mut solver = ready_solver(10101, uma(Style::Late), vec![skill], seeded_config(0));
    solver.run();
    solver.runners[0].activated_alternatives[0]
}

#[test]
fn test_continuation_of_out_of_gate_alternative_activates_during_race() {
    assert_eq!(out_of_gate_continuation_mask("always==1"), 0b11);
    // Never without the out-of-gate alternative.
    assert_eq!(out_of_gate_continuation_mask("course_distance==9999"), 0);
}

#[test]
fn test_forced_activation_disables_other_alternatives() {
    let alternatives = vec![
        alternative("accumulatetime>=999", SkillType::TargetSpeed, 0.15),
        alternative("accumulatetime>=5", SkillType::TargetSpeed, 0.15),
    ];
    let skill = Arc::new(synthetic_skill(alternatives));
    let mut solver = ready_solver(10101, uma(Style::Late), vec![skill], seeded_config(0));
    // Bypassing conditions, like random gold activations.
    solver.runners[0].activate_skill(0, 0, &solver.state);
    solver.run();
    assert_eq!(solver.runners[0].activated_alternatives[0], 0b01);
}

/// Out-of-gate skill with the given effects on oneself, whose condition is
/// always satisfied.
fn out_of_gate_skill(id: i32, base_duration: f64, effects: &[(SkillType, f64)]) -> Arc<Skill> {
    let alternative = SkillAlternative {
        base_duration,
        effects: effects
            .iter()
            .map(|&(skill_type, modifier)| effect(skill_type, modifier))
            .collect(),
        ..alternative("always==1", SkillType::SpeedUp, 0.0)
    };
    Arc::new(Skill {
        id,
        ..synthetic_skill(vec![alternative])
    })
}

/// IDs and distances of the skills activated by the only runner.
fn activations(solver: &RaceSolver<Runner>) -> Vec<(i32, f64)> {
    solver.runners[0]
        .activated_skills
        .iter()
        .map(|record| (record.skill_id, record.distance))
        .collect()
}

#[test]
fn test_out_of_gate_skill_activates_on_first_frame_of_movement() {
    let speed = out_of_gate_skill(32, f64::INFINITY, &[(SkillType::SpeedUp, 40.0)]);
    let mut solver = ready_solver(10101, uma(Style::Pace), vec![speed], seeded_config(0));
    // Longer than a frame: the runner doesn't move during the first one.
    solver.runners[0].start_delay = 0.09;

    solver.step(DELTA_TIME);
    assert!(activations(&solver).is_empty());
    assert_eq!(solver.runners[0].position, 0.0);

    // Activated before moving, on the frame she leaves the gate.
    solver.step(DELTA_TIME);
    assert_eq!(activations(&solver), vec![(32, 0.0)]);
    assert!(solver.runners[0].position > 0.0);
    let counts = &solver.runners[0].trigger_scheduler.activation_counts;
    assert_eq!((counts.all, counts.start), (1, 1));
}

#[test]
fn test_out_of_gate_timed_effects_apply_from_first_frame() {
    // Like "Perfect Concentration": start delay before the race, then acceleration.
    let concentration = out_of_gate_skill(
        7,
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
        seeded_config(0),
    );
    // At most 0.1 * 0.4 seconds, within the first frame.
    assert!(solver.runners[0].start_delay <= 0.04);

    solver.step(DELTA_TIME);
    let runner = &solver.runners[0];
    assert_eq!(activations(&solver), vec![(7, 0.0)]);
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
            16,
            f64::INFINITY,
            &[(SkillType::WitUp, 40.0), (SkillType::Fov, 10.0)],
        )
        .alternatives[0]
            .clone()
    };
    let fov = Arc::new(Skill {
        id: 16,
        ..synthetic_skill(vec![fov])
    });
    let mut solver = ready_solver(10101, uma(Style::Pace), vec![fov], seeded_config(0));

    solver.run();
    let runner = &solver.runners[0];
    // Activated only once, even without cooldown.
    assert_eq!(activations(&solver), vec![(16, 0.0)]);
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
    let speed = out_of_gate_skill(32, f64::INFINITY, &[(SkillType::SpeedUp, 40.0)]);
    let follow = skill(14, "is_activate_any_skill==1", SkillType::TargetSpeed, 0.15);
    let mut solver = ready_solver(
        10101,
        uma(Style::Pace),
        vec![speed, follow],
        seeded_config(0),
    );
    solver.runners[0].start_delay = 0.0;

    solver.step(DELTA_TIME);
    assert_eq!(activations(&solver), vec![(32, 0.0), (14, 0.0)]);
}

#[test]
fn test_out_of_gate_style_change_is_applied_before_the_race() {
    // Like "Great Escape".
    let escape = skill(11, "running_style==1", SkillType::ChangeStyleRunaway, 0.0);
    let solver = ready_solver(10101, uma(Style::Front), vec![escape], seeded_config(0));
    assert_eq!(solver.runners[0].adjusted.style, Style::Runaway);
}

/// Gold skill with the given condition.
fn gold_skill(id: i32, condition: &str, skill_type: SkillType, modifier: f64) -> Arc<Skill> {
    Arc::new(Skill {
        id,
        rarity: SkillRarity::Gold,
        ..synthetic_skill(vec![alternative(condition, skill_type, modifier)])
    })
}

/// Sorted IDs of the skills activated by the only runner after a whole race,
/// with "Adventure of 564"-like skill activating `modifier` random gold
/// skills in the second half of the race.
fn random_gold_activations(modifier: f64, mut skills: Vec<Arc<Skill>>) -> Vec<i32> {
    let adventure = skill(
        3,
        "distance_rate>=50",
        SkillType::ActivateRandomGold,
        modifier,
    );
    skills.push(adventure);
    let mut solver = ready_solver(10101, uma(Style::Pace), skills, seeded_config(0));
    solver.run();

    let mut ids: Vec<_> = activations(&solver).into_iter().map(|(id, _)| id).collect();
    ids.sort();
    ids
}

/// Gold skill that never activates on its own (there are no heals).
fn forced_gold_skill(id: i32) -> Arc<Skill> {
    gold_skill(
        id,
        "is_activate_heal_skill==1",
        SkillType::TargetSpeed,
        0.15,
    )
}

#[test]
fn test_activate_random_gold_activates_one_skill_per_unit() {
    let skills = vec![
        forced_gold_skill(1),
        forced_gold_skill(5),
        forced_gold_skill(6),
    ];
    let ids = random_gold_activations(2.0, skills);

    // The adventure skill (3) and two distinct gold skills.
    assert_eq!(ids.len(), 3, "{ids:?}");
    assert!(ids.contains(&3), "{ids:?}");
    assert!(ids.windows(2).all(|pair| pair[0] != pair[1]), "{ids:?}");
}

#[test]
fn test_activate_random_gold_bypasses_conditions() {
    let ids = random_gold_activations(1.0, vec![forced_gold_skill(15)]);
    assert_eq!(ids, vec![3, 15]);
}

#[test]
fn test_activate_random_gold_skips_skills_in_cooldown_and_out_of_gate_skills() {
    let skills = vec![
        // Activates on its own before the adventure skill (3), and is still in
        // cooldown then.
        gold_skill(10, "distance_rate>=10", SkillType::TargetSpeed, 0.15),
        // Activates before the race.
        gold_skill(18, "always==1", SkillType::SpeedUp, 40.0),
        // White skills can't be picked.
        skill(
            35,
            "is_activate_heal_skill==1",
            SkillType::TargetSpeed,
            0.15,
        ),
        forced_gold_skill(15),
    ];
    // Only the forced skill (15) can be picked, although two skills should be.
    let ids = random_gold_activations(2.0, skills);
    assert_eq!(ids, vec![3, 10, 15, 18]);
}

/// Gold skill activating on its own once, early in the race, with a cooldown
/// ending before the second half of the race.
fn early_gold_skill(id: i32, base_duration: f64) -> Arc<Skill> {
    let alternative = SkillAlternative {
        base_duration,
        base_cooldown: 10.0,
        ..alternative(
            "distance_rate>=10&distance_rate<=12",
            SkillType::TargetSpeed,
            0.15,
        )
    };
    Arc::new(Skill {
        id,
        rarity: SkillRarity::Gold,
        ..synthetic_skill(vec![alternative])
    })
}

#[test]
fn test_activate_random_gold_picks_activated_skills_again_after_cooldown() {
    let ids = random_gold_activations(1.0, vec![early_gold_skill(10, 3.0)]);
    assert_eq!(ids, vec![3, 10, 10]);
}

#[test]
fn test_activate_random_gold_skips_active_skills() {
    // Its cooldown ends before the adventure skill (3), but not its effect.
    let ids = random_gold_activations(1.0, vec![early_gold_skill(10, f64::INFINITY)]);
    assert_eq!(ids, vec![3, 10]);
}

#[test]
fn test_activate_random_gold_picks_skills_that_failed_the_wit_check() {
    let config = Configuration {
        wit_checks: Roll::Never,
        ..seeded_config(0)
    };
    let mut solver = ready_solver(10101, uma(Style::Pace), vec![forced_gold_skill(15)], config);
    let runner = &mut solver.runners[0];
    assert_eq!(runner.failed_skills().len(), 1);
    let activate_random_gold = |runner: &mut Runner, state: &RaceState| {
        runner.apply_skill_effect(
            &effect(SkillType::ActivateRandomGold, 1.0),
            0.0,
            OvertakeExtension::default(),
            0.0,
            None,
            state,
        );
    };

    activate_random_gold(runner, &solver.state);
    let ids: Vec<_> = activations(&solver).into_iter().map(|(id, _)| id).collect();
    assert_eq!(ids, vec![15]);

    // The skill has no logics, but its cooldown still applies.
    let runner = &mut solver.runners[0];
    assert!(runner.cooldown_until[0][0] > solver.state.elapsed_time);
    activate_random_gold(runner, &solver.state);
    assert_eq!(activations(&solver).len(), 1);
}

/// Unique skill with the given alternatives.
fn unique_skill(id: i32, alternatives: Vec<SkillAlternative>) -> Arc<Skill> {
    Arc::new(Skill {
        id,
        rarity: SkillRarity::UniqueNatural,
        ..synthetic_skill(alternatives)
    })
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
fn unique_reactivation_runner(modifier: f64, mut skills: Vec<Arc<Skill>>) -> Runner {
    let again = skill(4, "distance_rate>=50", SkillType::ActivateUnique, modifier);
    skills.push(again);
    let mut solver = ready_solver(10101, uma(Style::Pace), skills, seeded_config(0));
    solver.run();
    solver.runners.swap_remove(0)
}

/// Sorted IDs of the skills activated by `runner`.
fn sorted_activation_ids(runner: &Runner) -> Vec<i32> {
    let mut ids: Vec<_> = runner
        .activated_skills
        .iter()
        .map(|record| record.skill_id)
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
        34,
        vec![lasting_alternative(
            "distance_rate>=10",
            SkillType::TargetSpeed,
            0.35,
        )],
    );
    let runner = unique_reactivation_runner(1.0, vec![unique]);
    assert_eq!(sorted_activation_ids(&runner), vec![4, 34, 34]);
    // Both activations' effects stack.
    assert_eq!(active_effect_count(&runner, SkillType::TargetSpeed), 2);
}

#[test]
fn test_activate_unique_bypasses_conditions() {
    let unique = unique_skill(
        34,
        vec![alternative(
            "is_activate_heal_skill==1",
            SkillType::TargetSpeed,
            0.35,
        )],
    );
    let runner = unique_reactivation_runner(1.0, vec![unique]);
    assert_eq!(sorted_activation_ids(&runner), vec![4, 34]);
}

#[test]
fn test_activate_unique_uses_the_alternative_that_activated() {
    let unique = unique_skill(
        34,
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
        unique_skill(18, vec![alternative("always==1", SkillType::SpeedUp, 40.0)]),
        // Gold skills can't be picked.
        forced_gold_skill(17),
        never(13),
        never(31),
    ];
    // Only the first eligible unique activates.
    let runner = unique_reactivation_runner(1.0, skills);
    assert_eq!(sorted_activation_ids(&runner), vec![4, 13, 18]);
}

/// Skill with a target speed effect on oneself whose value scales as
/// `value_scaling`. It never activates on its own.
fn scaled_skill(id: i32, value_scaling: ValueScaling, modifier: f64) -> Arc<Skill> {
    let mut alternative = alternative(
        "is_activate_heal_skill==1",
        SkillType::TargetSpeed,
        modifier,
    );
    alternative.effects[0].value_scaling = value_scaling;
    Arc::new(Skill {
        id,
        ..synthetic_skill(vec![alternative])
    })
}

/// Ready race with a single runner having `skills` and a [`scaled_skill`] last.
fn scaled_skill_solver(
    uma: Uma,
    mut skills: Vec<Arc<Skill>>,
    value_scaling: ValueScaling,
    config: Configuration,
) -> RaceSolver<Runner> {
    skills.push(scaled_skill(30, value_scaling, 0.25));
    ready_solver(10101, uma, skills, config)
}

/// Target speed gained by the first runner when activating her last skill.
fn activate_last_skill(solver: &mut RaceSolver<Runner>) -> f64 {
    let runner = &mut solver.runners[0];
    let before = runner.skill_modifiers.target_speed;
    let skill_index = runner.available_skills().len() - 1;
    runner.activate_skill(skill_index, 0, &solver.state);
    runner.skill_modifiers.target_speed - before
}

#[test]
fn test_direct_values_are_not_scaled() {
    let mut solver = scaled_skill_solver(
        uma(Style::Late),
        vec![],
        ValueScaling::Direct,
        seeded_config(0),
    );
    assert_close!(activate_last_skill(&mut solver), 0.25);
}

#[test]
fn test_skill_count_scaling() {
    // Nine other skills, plus the scaled skill.
    let others: Vec<_> = (0..9)
        .map(|i| skill(i, "always==1", SkillType::TargetSpeed, 0.1))
        .collect();
    let mut solver = scaled_skill_solver(
        uma(Style::Late),
        others,
        ValueScaling::MultiplySkillNum,
        seeded_config(0),
    );
    assert_close!(activate_last_skill(&mut solver), 0.25 * 1.1);
}

#[test]
fn test_skill_count_includes_failed_wit_checks() {
    let config = Configuration {
        wit_checks: Roll::Never,
        ..seeded_config(0)
    };
    let skills = (0..3)
        .map(|i| skill(i, "always==1", SkillType::TargetSpeed, 0.1))
        .collect();
    let solver = ready_solver(10101, uma(Style::Late), skills, config);
    let runner = &solver.runners[0];
    assert!(runner.available_skills().is_empty());
    assert_eq!(runner.failed_skills().len(), 3);
    assert_eq!(runner.skill_count(), 3);
}

#[test]
fn test_scenario_values_assume_the_maximum() {
    let cases = [
        (ValueScaling::UnityCupSpeed, 1.2),
        (ValueScaling::UnityCupWit, 1.2),
        (ValueScaling::Climax, 1.2),
        (ValueScaling::MultiplyArcGlobalPotentialLevel, 1.2),
        (ValueScaling::UafWinCount, 1.2),
        (ValueScaling::CookingPoints, 1.2),
        (ValueScaling::ResearchLevel, 1.2),
        (ValueScaling::DevelopmentPoints, 1.2),
        (ValueScaling::OnsenLegendaryInn, 1.2),
        (ValueScaling::TeamRank, 1.2),
        // TODO: Update once the Excitement Points multipliers are known.
        (ValueScaling::ExcitementPoints, 1.0),
        (ValueScaling::FanLove, 3.0),
    ];
    for (value_scaling, multiplier) in cases {
        let mut solver =
            scaled_skill_solver(uma(Style::Late), vec![], value_scaling, seeded_config(0));
        assert_close!(
            activate_last_skill(&mut solver),
            0.25 * multiplier,
            "{value_scaling:?}"
        );
    }
}

#[test]
fn test_fan_count_scaling() {
    let mut solver = scaled_skill_solver(
        uma(Style::Late),
        vec![],
        ValueScaling::FanCount,
        seeded_config(0),
    );
    // No fans by default.
    assert_close!(activate_last_skill(&mut solver), 0.25 * 0.8);
    solver.runners[0].initial_state.fan_count = 120000;
    assert_close!(activate_last_skill(&mut solver), 0.25 * 1.1);
}

#[test]
fn test_late_race_corner_overtakes_are_tracked() {
    let mut solver = scaled_skill_solver(
        uma(Style::Late),
        vec![],
        ValueScaling::CornerOvertakeCount,
        seeded_config(0),
    );
    // On the 1200m course, the Late race starts at 800m, in the second corner
    // (675-934m), followed by the final straight.
    let course = solver.state.course.clone();
    let runner = &mut solver.runners[0];
    let overtake = |runner: &mut Runner, position, phase, from, to| {
        runner.position = position;
        runner.phase = phase;
        runner.segments.update(position, &course);
        runner.order.order = from;
        runner.update_order(to, &course);
    };

    // In a corner, but not in the Late race.
    overtake(runner, 700.0, Phase::Mid, 9, 8);
    // Two at once in a Late race corner, and being overtaken doesn't count.
    overtake(runner, 850.0, Phase::Late, 8, 6);
    assert_eq!(runner.order.delta, -2);
    overtake(runner, 860.0, Phase::Late, 6, 7);
    assert_eq!(runner.order.delta, 1);
    // On the final straight.
    overtake(runner, 1000.0, Phase::Last, 7, 5);
    assert_eq!(runner.order.overtakes_in_late_corners, 2);
    // Other overtake counters (the last corner starts at 675m).
    assert_eq!(runner.order.overtakes_since_mid, 5);
    assert_eq!(runner.order.overtakes_since_late, 4);
    assert_eq!(runner.order.overtakes_since_last_corner, 5);

    assert_close!(activate_last_skill(&mut solver), 0.25 * 1.1);
}

#[test]
fn test_random_scaling_distribution() {
    for value_scaling in [ValueScaling::MultiplyRandom1, ValueScaling::MultiplyRandom2] {
        let mut solver =
            scaled_skill_solver(uma(Style::Late), vec![], value_scaling, seeded_config(0));
        let mut counts = [0; 3];
        let rolls = 10000;
        for _ in 0..rolls {
            let gained = activate_last_skill(&mut solver) / 0.25;
            let index = [0.0, 0.02, 0.04]
                .iter()
                .position(|&multiplier| (gained - multiplier).abs() < 1e-9)
                .unwrap_or_else(|| panic!("unexpected multiplier {gained}"));
            counts[index] += 1;
        }
        for (count, expected) in counts.into_iter().zip([0.6, 0.3, 0.1]) {
            assert_close!(count as f64 / rolls as f64, expected, tolerance = 0.02);
        }
    }
}

#[test]
fn test_raw_stat_scaling_uses_the_highest_raw_stat() {
    // Raw stats above 1200 are halved in the base stats, but not here.
    let uma = Uma {
        stamina: 1500,
        ..uma(Style::Late)
    };
    let mut solver = scaled_skill_solver(
        uma,
        vec![],
        ValueScaling::MultiplyMaximumRawStatus,
        seeded_config(0),
    );
    assert_close!(activate_last_skill(&mut solver), 0.25 * 1.2);

    let uma = Uma {
        speed: 700,
        stamina: 700,
        power: 700,
        guts: 700,
        wit: 750,
        ..uma
    };
    let mut solver = scaled_skill_solver(
        uma,
        vec![],
        ValueScaling::MultiplyMaximumRawStatus,
        seeded_config(0),
    );
    assert_close!(activate_last_skill(&mut solver), 0.25 * 0.9);
}

#[test]
fn test_speed_scaling_uses_adjusted_speed() {
    let cases = [
        (ValueScaling::MultiplySpeed1, 1850, 2.0),
        (ValueScaling::MultiplySpeed2, 1500, 2.0),
    ];
    for (value_scaling, speed, multiplier) in cases {
        let mut solver =
            scaled_skill_solver(uma(Style::Late), vec![], value_scaling, seeded_config(0));
        solver.runners[0].adjusted.speed = speed;
        let gained = activate_last_skill(&mut solver);
        assert_close!(gained, 0.25 * multiplier, "{value_scaling:?}");
    }
}

#[test]
fn test_buffs_scaled_to_zero_still_count() {
    let mut solver = scaled_skill_solver(
        uma(Style::Late),
        vec![],
        ValueScaling::MultiplySpeed1,
        seeded_config(0),
    );
    // Too slow for any bonus.
    solver.runners[0].adjusted.speed = 1600;
    assert_close!(activate_last_skill(&mut solver), 0.0);
    let buffs = &solver.runners[0].activation_flags.buff_activated;
    assert!(buffs.contains(&SkillType::TargetSpeed));
}

#[test]
fn test_green_skill_count_scaling() {
    let green = |id| skill(id, "always==1", SkillType::SpeedUp, 10.0);
    // Green skills that never activate don't count.
    let skills = vec![
        green(19),
        green(20),
        green(21),
        skill(26, "is_activate_heal_skill==1", SkillType::SpeedUp, 10.0),
    ];
    let mut solver = scaled_skill_solver(
        uma(Style::Late),
        skills,
        ValueScaling::MultiplyActivateSpecificTagSkillCount,
        seeded_config(0),
    );
    // Before the race starts, no green skill has activated yet.
    assert_close!(activate_last_skill(&mut solver), 0.0);

    // Green skills activate on the first frame the runner moves.
    while solver.runners[0]
        .activated_alternatives
        .iter()
        .filter(|&&mask| mask != 0)
        .count()
        < 4
    {
        solver.step(DELTA_TIME);
    }
    // Also counted for `activate_count_param_skill`.
    let counts = &solver.runners[0].trigger_scheduler.activation_counts;
    assert_eq!(counts.green, 3);
    assert_close!(activate_last_skill(&mut solver), 0.25);
}

#[test]
fn test_distance_from_first_adds_to_value() {
    let mut solver = scaled_skill_solver(
        uma(Style::Late),
        vec![],
        ValueScaling::AddDistanceDiffTop,
        seeded_config(0),
    );
    solver.runners[0].position_diff.from_first = 19.0;
    assert_close!(activate_last_skill(&mut solver), 0.25);
    solver.runners[0].position_diff.from_first = 20.0;
    assert_close!(activate_last_skill(&mut solver), 0.35);
}

#[test]
fn test_bashin_from_first_adds_to_value() {
    let mut solver = scaled_skill_solver(
        uma(Style::Late),
        vec![],
        ValueScaling::AddBashinDiffTop,
        seeded_config(0),
    );
    // 8 bashin are 20 meters.
    solver.runners[0].position_diff.from_first = 19.9;
    assert_close!(activate_last_skill(&mut solver), 0.25);
    solver.runners[0].position_diff.from_first = 20.0;
    assert_close!(activate_last_skill(&mut solver), 0.45);
}

#[test]
fn test_medium_distance_adds_to_value() {
    let skills = || vec![scaled_skill(30, ValueScaling::AddDistanceTypeMedium, 0.25)];
    // 1200m (Short).
    let mut solver = ready_solver(10101, uma(Style::Late), skills(), seeded_config(0));
    assert_close!(activate_last_skill(&mut solver), 0.25);
    // 2400m (Medium).
    let mut solver = ready_solver(10606, uma(Style::Late), skills(), seeded_config(0));
    assert_close!(activate_last_skill(&mut solver), 0.45);
}

#[test]
fn test_longest_mid_race_side_block_is_tracked() {
    let mut solver = scaled_skill_solver(
        uma(Style::Late),
        vec![],
        ValueScaling::MultiplyBlockedSideMaxTimeMidRace,
        seeded_config(0),
    );
    let runner = &mut solver.runners[0];
    let block = |runner: &mut Runner, phase, side_in, side_out, time| {
        runner.phase = phase;
        runner.blocked.side_in = side_in;
        runner.blocked.side_out = side_out;
        runner.update_blocked_time(time);
    };

    // Before the Mid race, blocking doesn't count.
    block(runner, Phase::Early, true, false, 5.0);
    assert_close!(runner.blocked.max_mid_side_time, 0.0);
    // Blocking that continues into the Mid race only counts from then on, but
    // the overall timer keeps running. Changing sides doesn't reset either.
    block(runner, Phase::Mid, false, true, 1.0);
    block(runner, Phase::Mid, true, false, 2.0);
    assert_close!(runner.blocked.side_time, 8.0);
    assert_close!(runner.blocked.max_mid_side_time, 3.0);
    // Reset once both sides are clear; the longest blocking is kept.
    block(runner, Phase::Mid, false, false, 1.0);
    block(runner, Phase::Mid, true, false, 2.0);
    assert_close!(runner.blocked.side_time, 2.0);
    assert_close!(runner.blocked.max_mid_side_time, 3.0);
    // After the Mid race, blocking doesn't count either.
    block(runner, Phase::Late, true, false, 10.0);
    assert_close!(runner.blocked.max_mid_side_time, 3.0);

    assert_close!(activate_last_skill(&mut solver), 0.25 * 2.0);
}

#[test]
fn test_largest_lead_before_late_race_is_tracked() {
    let mut solver = RaceSolver::new(load_course(10101), race_params(), seeded_config(0));
    let skills = vec![scaled_skill(30, ValueScaling::MultiplyTopLeadAmount, 0.25)];
    solver.add_runner(uma(Style::Front), Mood::Normal, skills);
    solver.add_runner(uma(Style::Late), Mood::Normal, vec![]);
    let mut solver = solver.ready_runners();
    let update = |solver: &mut RaceSolver<Runner>, front, late, phase| {
        for runner in &mut solver.runners {
            let is_front = runner.base.style == Style::Front;
            runner.position = if is_front { front } else { late };
            runner.phase = phase;
        }
        sort_by_position(solver);
        solver.update_order_values();
    };

    update(&mut solver, 100.0, 85.0, Phase::Early);
    update(&mut solver, 200.0, 195.0, Phase::Mid);
    // Leads in the Late race, or by the other runner, don't count.
    update(&mut solver, 1000.0, 950.0, Phase::Late);
    update(&mut solver, 1100.0, 1200.0, Phase::Late);
    let front = runner_with_style(&mut solver, Style::Front);
    assert_close!(front.position_diff.max_lead, 15.0);
    let late = runner_with_style(&mut solver, Style::Late);
    assert_close!(late.position_diff.max_lead, 0.0);

    // The other runner is ahead now; move the scaled skill's owner first.
    solver.runners.swap(0, 1);
    assert_close!(activate_last_skill(&mut solver), 0.25 * 1.4);
}

/// Skill with a 3-second target speed effect whose duration scales as
/// `duration_scaling`. It never activates on its own.
fn timed_skill(id: i32, duration_scaling: DurationScaling) -> Arc<Skill> {
    let alternative = SkillAlternative {
        duration_scaling,
        ..alternative("is_activate_heal_skill==1", SkillType::TargetSpeed, 0.25)
    };
    Arc::new(Skill {
        id,
        ..synthetic_skill(vec![alternative])
    })
}

/// Ready race on a 1200m course with a single runner having `skills`.
fn timed_skill_solver(skills: Vec<Arc<Skill>>) -> RaceSolver<Runner> {
    ready_solver(10101, uma(Style::Late), skills, seeded_config(0))
}

/// Remaining duration of each of the first runner's active timed effects.
fn remaining_durations(solver: &RaceSolver<Runner>) -> Vec<f64> {
    solver.runners[0]
        .active_timed_effects
        .iter()
        .map(|timed| timed.expires_at - solver.state.elapsed_time)
        .collect()
}

#[test]
fn test_duration_scaling_by_runner_situation() {
    // Base duration of 3 seconds, scaled to the 1200m course.
    let base = 3.0 * 1.2;
    // Base stamina of 1200 + 800 / 2 = 1600, so full HP is 0.8 × 1600 + 1200
    // = 2480, within the HP bands tested below.
    let uma = Uma {
        stamina: 2000,
        ..uma(Style::Late)
    };
    /// Changes the runner's situation before activating the skill.
    type Setup = fn(&mut Runner);
    let cases: [(DurationScaling, Setup, f64); 6] = [
        (DurationScaling::Direct, |_| {}, 1.0),
        (
            DurationScaling::MultiplyDistanceDiffTop,
            |runner| runner.position_diff.from_first = 25.0,
            1.2,
        ),
        (DurationScaling::MultiplyRemainHp1, |_| {}, 2.0),
        (DurationScaling::MultiplyRemainHp2, |_| {}, 3.0),
        (
            DurationScaling::MultiplyBlockedSideMaxContinueTimePhaseMiddleRun,
            |runner| runner.blocked.max_mid_side_time = 4.5,
            3.0,
        ),
        // Only extended later, by overtaking.
        (DurationScaling::IncrementOrderUp, |_| {}, 1.0),
    ];
    for (duration_scaling, setup, multiplier) in cases {
        let skills = vec![timed_skill(33, duration_scaling)];
        let mut solver = ready_solver(10101, uma, skills, seeded_config(0));
        setup(&mut solver.runners[0]);
        solver.runners[0].activate_skill(0, 0, &solver.state);
        let durations = remaining_durations(&solver);
        assert_eq!(durations.len(), 1, "{duration_scaling:?}");
        assert_close!(durations[0], base * multiplier, "{duration_scaling:?}");
    }
}

#[test]
fn test_overtakes_extend_effects() {
    // Two effects extended by overtaking, and another skill's effect that isn't.
    let mut alternative =
        timed_skill(12, DurationScaling::IncrementOrderUp).alternatives[0].clone();
    alternative.effects.push(effect(
        SkillType::Acceleration,
        alternative.effects[0].modifier,
    ));
    let extended = Arc::new(Skill {
        id: 12,
        ..synthetic_skill(vec![alternative])
    });
    let direct = timed_skill(9, DurationScaling::Direct);
    let mut solver = timed_skill_solver(vec![extended, direct]);
    let course = solver.state.course.clone();
    solver.runners[0].activate_skill(0, 0, &solver.state);
    solver.runners[0].activate_skill(1, 0, &solver.state);
    let change_order = |solver: &mut RaceSolver<Runner>, from, to| {
        let runner = &mut solver.runners[0];
        runner.order.order = from;
        runner.update_order(to, &course);
    };

    // Each extension adds 1 second, scaled to the 1200m course.
    let base = 3.0 * 1.2;
    // Two overtakes at once.
    change_order(&mut solver, 5, 3);
    let durations = remaining_durations(&solver);
    assert_close!(durations[0], base + 2.0 * 1.2);
    assert_close!(durations[1], base + 2.0 * 1.2);
    assert_close!(durations[2], base);
    // Being overtaken doesn't extend them.
    change_order(&mut solver, 3, 4);
    assert_close!(remaining_durations(&solver)[0], base + 2.0 * 1.2);
    // Up to three extensions in total.
    change_order(&mut solver, 4, 1);
    change_order(&mut solver, 2, 1);
    let durations = remaining_durations(&solver);
    assert_close!(durations[0], base + 3.0 * 1.2);
    assert_close!(durations[1], base + 3.0 * 1.2);
    assert_close!(durations[2], base);
}

#[test]
fn test_uphills_climbed_are_counted() {
    let mut solver = timed_skill_solver(vec![]);
    let course = course_with_slopes(vec![
        Slope::new(100.0, 100.0, 2.0),
        Slope::new(300.0, 100.0, -2.0),
        // Right after the previous one.
        Slope::new(400.0, 100.0, 1.5),
    ]);
    let mut climbed_at = |position| {
        move_on_slopes(&mut solver, &course, position);
        solver.runners[0].uphills_climbed
    };

    // Only counted once the end of an uphill is reached.
    assert_eq!(climbed_at(150.0), 0);
    assert_eq!(climbed_at(200.0), 1);
    // Downhills don't count.
    assert_eq!(climbed_at(350.0), 1);
    assert_eq!(climbed_at(450.0), 1);
    assert_eq!(climbed_at(500.0), 2);
}

/// Synthetic course with the given slopes.
fn course_with_slopes(slopes: Vec<Slope>) -> Course {
    Course {
        slopes,
        ..synthetic_course()
    }
}

/// Move the first runner to `position` on `course`, applying slope effects.
fn move_on_slopes(solver: &mut RaceSolver<Runner>, course: &Course, position: f64) {
    let RaceSolver { runners, state, .. } = solver;
    let runner = &mut runners[0];
    runner.position = position;
    let transitions = runner.segments.update(position, course);
    runner.update_slope(
        &course.slopes,
        &transitions.slope,
        DELTA_TIME,
        &mut *state.rng.borrow_mut(),
    );
}

#[test]
fn test_downhill_mode_checked_on_entry_then_every_second() {
    let mut solver = timed_skill_solver(vec![]);
    let course = course_with_slopes(vec![Slope::new(100.0, 200.0, -2.0)]);
    // Uphills aren't mistaken for downhill mode.
    let uphill = course_with_slopes(vec![Slope::new(50.0, 40.0, 2.0)]);
    move_on_slopes(&mut solver, &uphill, 50.0);
    assert!(solver.runners[0].is_going_uphill());
    assert!(!solver.runners[0].is_in_downhill_mode());
    move_on_slopes(&mut solver, &uphill, 90.0);

    // Guaranteed to enter downhill mode on every check.
    solver.runners[0].adjusted.wit = 2500;

    move_on_slopes(&mut solver, &course, 99.0);
    assert!(!solver.runners[0].is_in_downhill_mode());
    move_on_slopes(&mut solver, &course, 100.0);
    assert!(solver.runners[0].is_in_downhill_mode());
    assert!(!solver.runners[0].is_going_uphill());

    // After exiting downhill mode, the next check is a second (15 frames) after
    // entry.
    solver.runners[0].slope_speed_modifier = None;
    for frame in 1..15 {
        move_on_slopes(&mut solver, &course, 100.0 + frame as f64);
        assert!(!solver.runners[0].is_in_downhill_mode(), "frame {frame}");
    }
    move_on_slopes(&mut solver, &course, 115.0);
    assert!(solver.runners[0].is_in_downhill_mode());

    // Exiting the downhill exits downhill mode.
    move_on_slopes(&mut solver, &course, 300.0);
    assert!(!solver.runners[0].is_in_downhill_mode());
}

#[test]
fn test_uphills_climbed_extend_duration() {
    // Base duration of 3 seconds plus 0.5 seconds per uphill (up to 4), scaled to
    // the 1200m course.
    for (uphills, base_duration) in [(0, 3.0), (2, 4.0), (4, 5.0), (6, 5.0)] {
        let mut solver =
            timed_skill_solver(vec![timed_skill(33, DurationScaling::IncrementUphillCount)]);
        solver.runners[0].uphills_climbed = uphills;
        solver.runners[0].activate_skill(0, 0, &solver.state);
        let durations = remaining_durations(&solver);
        assert_close!(durations[0], base_duration * 1.2, "{uphills} uphills");
    }
}

/// Skill with a 0.25 target speed effect, and a 0.05 one applied as
/// `additional`. It never activates on its own.
fn additional_skill(id: i32, additional: AdditionalActivation) -> Arc<Skill> {
    let mut alternative = alternative("is_activate_heal_skill==1", SkillType::TargetSpeed, 0.25);
    alternative.effects.push(SkillEffect {
        additional,
        ..effect(SkillType::TargetSpeed, 0.05)
    });
    Arc::new(Skill {
        id,
        ..synthetic_skill(vec![alternative])
    })
}

#[test]
fn test_additional_effects_apply_on_overtakes() {
    let skills = vec![additional_skill(2, AdditionalActivation::OrderUp)];
    let mut solver = timed_skill_solver(skills);
    let course = solver.state.course.clone();
    let change_order = |solver: &mut RaceSolver<Runner>, from, to| {
        let runner = &mut solver.runners[0];
        runner.order.order = from;
        runner.update_order(to, &course);
        runner.skill_modifiers.target_speed
    };

    // Only the regular effect applies on activation.
    solver.runners[0].activate_skill(0, 0, &solver.state);
    assert_close!(solver.runners[0].skill_modifiers.target_speed, 0.25);
    // Two overtakes at once; being overtaken doesn't count.
    assert_close!(change_order(&mut solver, 5, 3), 0.35);
    assert_close!(change_order(&mut solver, 3, 4), 0.35);
    // Up to three times.
    assert_close!(change_order(&mut solver, 4, 2), 0.4);

    // Expires with the other effects.
    solver.state.elapsed_time = 3.0 * 1.2;
    solver.runners[0].update_skill_durations(solver.state.elapsed_time);
    assert!(solver.runners[0].active_timed_effects.is_empty());
    assert_close!(change_order(&mut solver, 2, 1), 0.0);
}

#[test]
fn test_additional_effects_apply_on_other_skill_activations() {
    // Other skills only boost acceleration, so that target speed only comes from
    // the skill with additional effects.
    let other = |id| {
        skill(
            id,
            "is_activate_heal_skill==1",
            SkillType::Acceleration,
            0.1,
        )
    };
    let mut skills = vec![
        additional_skill(13, AdditionalActivation::ActivateAnySkill1),
        additional_skill(31, AdditionalActivation::ActivateAnySkill2),
    ];
    skills.extend([1, 5, 6, 36].map(other));
    let mut solver = timed_skill_solver(skills);
    let activate = |solver: &mut RaceSolver<Runner>, skill_index| {
        let runner = &mut solver.runners[0];
        runner.activate_skill(skill_index, 0, &solver.state);
        let additional_triggers = |additional| {
            runner
                .active_timed_effects
                .iter()
                .find(|timed| timed.effect.additional == additional)
                .map(|timed| timed.additional_triggers)
        };
        (
            additional_triggers(AdditionalActivation::ActivateAnySkill1),
            additional_triggers(AdditionalActivation::ActivateAnySkill2),
        )
    };

    // A skill's own activation doesn't count.
    assert_eq!(activate(&mut solver, 0), (Some(0), None));
    assert_close!(solver.runners[0].skill_modifiers.target_speed, 0.25);
    // Activating a skill with additional effects counts for the others.
    assert_eq!(activate(&mut solver, 1), (Some(1), Some(0)));
    assert_eq!(activate(&mut solver, 2), (Some(2), Some(1)));
    // Up to three and two times respectively.
    assert_eq!(activate(&mut solver, 3), (Some(3), Some(2)));
    assert_eq!(activate(&mut solver, 4), (Some(3), Some(2)));
    assert_close!(
        solver.runners[0].skill_modifiers.target_speed,
        0.25 + 0.05 * 3.0 + 0.25 + 0.05 * 2.0
    );
}
