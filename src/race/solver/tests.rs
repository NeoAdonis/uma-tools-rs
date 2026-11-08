use super::*;
use crate::configuration::Roll;
use crate::course::Phase;
use crate::course::geometry::{Segment, Slope};
use crate::race::pins::{PinnedActivation, RushPins, SkillPin};
use crate::race::rush::{self, RushState};
use crate::race::spot_struggle::SpotStruggle;
use crate::race::timed_boost::TimedBoost;
use crate::test_utils::*;
use crate::trigger::skill_activation::{OvertakeExtension, PendingEffect};
use crate::units::CourseWidths;
use std::collections::HashSet;
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
fn test_position_derived_state_matches_positions_after_each_frame() {
    // Skills are checked against the runner's state; everything derived from the
    // position must match the position at the end of each frame, except for the
    // phase and section, which match the one before the last move.
    for course_id in [10101, 10104, 10606, 10913] {
        let mut solver = RaceSolver::new(load_course(course_id), race_params(), seeded_config(0));
        for style in [Style::Front, Style::Pace, Style::Late, Style::End] {
            solver.add_runner(Entrant::new(uma(style), vec![]));
        }
        let mut solver = solver.ready_runners();

        while solver.state.finish_time.is_none() {
            let before: Vec<_> = solver.runners.iter().map(|r| (r.id, r.position)).collect();
            solver.step(DELTA_TIME);

            let course = &solver.state.course;
            // Runners still in the gate aren't updated (no skills are checked).
            let started = solver
                .runners
                .iter()
                .filter(|runner| runner.start_delay <= solver.state.elapsed_time);
            for runner in started {
                let position = runner.position;
                let (_, prev_position) = *before.iter().find(|(id, _)| *id == runner.id).unwrap();
                let context = format!("course {course_id}, positions {prev_position}, {position}");
                assert_eq!(
                    runner.phase,
                    Phase::from_position(prev_position, course),
                    "{context}"
                );
                assert_eq!(
                    runner.section,
                    course.section_from_position(prev_position),
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
fn test_skills_see_positions_and_phase_from_earlier_frames() {
    // (condition, boundary deciding it, how many frames before the activation
    // the position it's decided from is). On this 2400m course, the Late phase
    // starts at 1600m and the final corner at 1910m. Less than 800m remain past
    // 1600m, so the last case only holds for the frame where the position is
    // past it but the phase isn't yet.
    let cases = [
        ("is_finalcorner==1", 1910.0, 1),
        ("phase>=2", 1600.0, 2),
        ("phase>=2&is_finalcorner==1", 1910.0, 1),
        ("phase==1&remain_distance<=799", 1600.0, 1),
    ];
    for (condition, boundary, lag) in cases {
        let skill = skill(24, condition, SkillType::TargetSpeed, 0.35);
        let uma = Uma {
            stamina: 1200,
            ..uma(Style::Pace)
        };
        let mut solver = ready_solver(10108, uma, vec![skill], seeded_config(0));

        // Positions at the end of each frame, from the start.
        let mut positions = vec![solver.runners[0].position];
        while solver.runners[0].activated_skills.is_empty() {
            assert!(!solver.step_frame(), "{condition}: never activated");
            positions.push(solver.runners[0].position);
        }
        let frame = (solver.runners[0].activated_skills[0].time / DELTA_TIME).round() as usize;
        let seen = &positions[frame - lag - 1..=frame - lag];
        assert!(
            seen[1] >= boundary && seen[0] < boundary,
            "{condition}: {seen:?}"
        );
    }
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
        solver.add_runner(Entrant::new(uma(style), vec![]));
    }
    let mut solver = solver.ready_runners();
    solver.state.elapsed_time = 10.0;
    // Runners are readied in gate order, so give each position to a runner with
    // its style (those with the same style are identical).
    let mut unplaced: Vec<&mut Runner> = solver.runners.iter_mut().collect();
    for &(style, position) in runners {
        let i = unplaced
            .iter()
            .position(|runner| runner.style == style)
            .unwrap();
        unplaced.swap_remove(i).position = position;
    }
    sort_by_position(&mut solver);
    solver
}

fn sort_by_position(solver: &mut RaceSolver<Runner>) {
    solver
        .runners
        .sort_by(|a, b| b.position.total_cmp(&a.position));
}

fn runner_with_style(solver: &mut RaceSolver<Runner>, style: Style) -> &mut Runner {
    solver
        .runners
        .iter_mut()
        .find(|runner| runner.raw.style == style)
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

    // Checked again after a 1s cooldown and the 2s interval.
    runner_with_style(&mut solver, Style::Front).position = 120.0;
    sort_by_position(&mut solver);
    solver.update_positioning_strategy();
    let next_check = solver.state.elapsed_time + 3.0;
    let runner = runner_with_style(&mut solver, Style::Front);
    assert_eq!(runner.positioning_strategy, PositioningStrategy::Normal);
    assert_eq!(runner.next_positioning_check, next_check);
}

#[test]
fn test_pace_up_ex_runaway_and_front_are_the_same_strategy() {
    for (behind, ahead) in [
        (Style::Runaway, Style::Front),
        (Style::Front, Style::Runaway),
    ] {
        let mut solver =
            positioning_solver(&[(behind, 100.0), (ahead, 110.0)], GameVersion::LATEST);
        solver.update_positioning_strategy();
        assert!(!is_pace_up_ex(&mut solver, behind));
        assert!(!is_pace_up_ex(&mut solver, ahead));
    }
    // A runner whose strategy should be behind still counts.
    let mut solver = positioning_solver(
        &[(Style::Runaway, 100.0), (Style::Pace, 110.0)],
        GameVersion::LATEST,
    );
    solver.update_positioning_strategy();
    assert!(is_pace_up_ex(&mut solver, Style::Runaway));
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
fn test_positioning_is_first_checked_at_the_gate() {
    // A late surger leaves the gate first, but positioning is first checked
    // before anyone moves: nobody is ahead, so both pace down instead of the pace
    // chaser entering Pace Up Ex behind her.
    let config = Configuration {
        rushing: Roll::Never,
        ..seeded_config(0)
    };
    let mut solver = RaceSolver::new(load_course(10101), race_params(), config);
    for style in [Style::Late, Style::Pace] {
        solver.add_runner(Entrant::new(uma(style), vec![]));
    }
    let mut solver = solver.ready_runners();
    for runner in &mut solver.runners {
        runner.start_delay = if runner.style == Style::Late {
            0.0
        } else {
            0.05
        };
    }
    solver.step_frame();
    for runner in &solver.runners {
        let strategy = runner.positioning_strategy;
        assert!(
            matches!(strategy, PositioningStrategy::PaceDown(_)),
            "{:?}: {strategy:?}",
            runner.style
        );
    }
}

#[test]
fn test_pace_up_ex_waits_for_the_next_check() {
    // Behind an End runner: Pace Up Ex is due, but only checked from the
    // normal strategy (not interrupting her Pace Down, 10m from the pacemaker
    // and below its 20m threshold), once her next check is due.
    let mut solver = positioning_solver(
        &[(Style::Pace, 100.0), (Style::End, 110.0)],
        GameVersion::LATEST,
    );
    let pace_down = PositioningStrategy::PaceDown(20.0);
    runner_with_style(&mut solver, Style::Pace).positioning_strategy = pace_down;
    solver.update_positioning_strategy();
    let next_check = solver.state.elapsed_time + 0.1;
    let runner = runner_with_style(&mut solver, Style::Pace);
    assert_eq!(runner.positioning_strategy, pace_down);

    runner.positioning_strategy = PositioningStrategy::Normal;
    runner.next_positioning_check = next_check;
    solver.update_positioning_strategy();
    assert!(!is_pace_up_ex(&mut solver, Style::Pace));

    solver.state.elapsed_time += 0.1;
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
fn test_pacemaker_changed_in_one_point_five_anniversary() {
    use PositioningStrategy::*;

    // Before, the pacemaker is the frontmost runner with the most aggressive
    // style, not the Pace leader: 2m behind her, a Late runner paces down.
    // From 1.5 anniv., the pacemaker is the leader, 22m ahead.
    // ? The leader is an approximation of the current pacemaker.
    for (version, paces_down) in [
        (GameVersion::FirstAnniversary, true),
        (GameVersion::OnePointFiveAnniversary, false),
    ] {
        let mut solver = positioning_solver(
            &[
                (Style::Pace, 120.0),
                (Style::Front, 100.0),
                (Style::Late, 98.0),
            ],
            version,
        );
        update_positioning_repeatedly(&mut solver);
        let actual = matches!(strategy_at(&solver, 98.0), PaceDown(_));
        assert_eq!(actual, paces_down, "{version:?}");
    }

    // Only the frontmost Front runner: 7.8m behind her is within the range of
    // a Late runner (7.54m to 8.12m on 1200m), unlike 0.2m ahead of the other.
    let mut solver = positioning_solver(
        &[
            (Style::Pace, 120.0),
            (Style::Front, 100.0),
            (Style::Late, 92.2),
            (Style::Front, 92.0),
        ],
        GameVersion::FirstAnniversary,
    );
    update_positioning_repeatedly(&mut solver);
    assert_eq!(strategy_at(&solver, 92.2), Normal);
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
    // (styles behind, whether the first one is rushed and acting as a front
    // runner, expected).
    let cases: [(&[Style], bool, PositioningStrategy); 5] = [
        (&[Style::Pace], false, FrontSpeedUp),
        (&[Style::Pace, Style::Late], false, FrontSpeedUp),
        (&[Style::Runaway], false, FrontSpeedUp),
        (&[Style::Pace], true, FrontSpeedUp),
        (&[Style::Front], false, Normal),
    ];
    for (behind, rushed, expected) in cases {
        let mut runners = vec![(Style::Front, 108.0)];
        runners.extend(behind.iter().copied().zip([100.0, 90.0]));
        let mut solver = positioning_solver(&runners, GameVersion::LATEST);
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

    // Overtaken: stays (before 1.5 anniv., so that Pace Up Ex doesn't apply).
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
        if expected == Normal {
            // Checked again after a 1s cooldown and the 2s interval.
            let runner = solver.runners.iter().find(|r| r.position > 100.0);
            let next_check = solver.state.elapsed_time + 3.0;
            assert_eq!(runner.unwrap().next_positioning_check, next_check);
        }
    }
}

#[test]
fn test_front_overtake_when_not_first_in_strategy() {
    use PositioningStrategy::*;

    // Before 1.5 anniv., so that Pace Up Ex doesn't apply. The Pace runner is
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

    // Runaways and front runners are the same strategy.
    for (behind, ahead) in [
        (Style::Front, Style::Runaway),
        (Style::Runaway, Style::Front),
    ] {
        let mut solver = positioning_solver(
            &[(Style::Pace, 200.0), (ahead, 150.0), (behind, 110.0)],
            GameVersion::FirstAnniversary,
        );
        update_positioning_repeatedly(&mut solver);
        assert_eq!(
            strategy_at(&solver, 110.0),
            FrontOvertake,
            "{behind:?} behind {ahead:?}"
        );
    }
}

#[test]
fn test_front_overtake_exits_when_far_ahead_in_strategy() {
    use PositioningStrategy::*;

    // (style, style behind, lead over the runner behind, expected). The Pace
    // runner ahead shows that the lead only counts the same strategy, where
    // runaways and front runners are the same strategy.
    let cases = [
        (Style::Front, Style::Front, 9.0, FrontOvertake),
        (Style::Front, Style::Front, 11.0, Normal),
        (Style::Runaway, Style::Runaway, 27.0, FrontOvertake),
        (Style::Runaway, Style::Runaway, 28.0, Normal),
        (Style::Front, Style::Runaway, 9.0, FrontOvertake),
        (Style::Runaway, Style::Front, 27.0, FrontOvertake),
    ];
    for (style, behind, lead, expected) in cases {
        let mut solver = positioning_solver(
            &[(Style::Pace, 200.0), (style, 100.0 + lead), (behind, 100.0)],
            GameVersion::FirstAnniversary,
        );
        set_strategy_at(&mut solver, 100.0 + lead, FrontOvertake);
        solver.update_positioning_strategy();
        assert_eq!(
            strategy_at(&solver, 100.0 + lead),
            expected,
            "{style:?} {lead}m ahead of {behind:?}"
        );
    }
}

#[test]
fn test_positioning_only_starts_in_first_ten_sections() {
    let mut solver = positioning_solver(
        &[(Style::Front, 100.0), (Style::End, 110.0)],
        GameVersion::LATEST,
    );
    solver.update_positioning_strategy();
    assert!(is_pace_up_ex(&mut solver, Style::Front));

    // Still underway after the first 10 sections, until it ends.
    runner_with_style(&mut solver, Style::Front).section = POSITIONING_END_SECTION;
    solver.update_positioning_strategy();
    assert!(is_pace_up_ex(&mut solver, Style::Front));

    runner_with_style(&mut solver, Style::Front).position = 120.0;
    sort_by_position(&mut solver);
    solver.update_positioning_strategy();
    assert!(!is_pace_up_ex(&mut solver, Style::Front));

    // Behind the End runner again, once checks would have resumed.
    runner_with_style(&mut solver, Style::Front).position = 100.0;
    sort_by_position(&mut solver);
    solver.state.elapsed_time += 10.0;
    solver.update_positioning_strategy();
    assert_eq!(
        runner_with_style(&mut solver, Style::Front).positioning_strategy,
        PositioningStrategy::Normal
    );
}

#[test]
fn test_positioning_lasts_one_section_at_most() {
    // Far behind a runner ahead whose strategy keeps her in hers: a Pace runner
    // paces up, a Runaway overtakes, and either front runner paces up (Ex)
    // behind an End runner, until they've run the length of a section (50m),
    // or three for runaways.
    let (old, latest) = (GameVersion::FirstAnniversary, GameVersion::LATEST);
    for (style, ahead, version, length) in [
        (Style::Pace, Style::Front, old, 50.0),
        (Style::Runaway, Style::Runaway, old, 150.0),
        (Style::Front, Style::End, latest, 50.0),
        (Style::Runaway, Style::End, latest, 150.0),
    ] {
        let mut solver = positioning_solver(&[(ahead, 300.0), (style, 100.0)], version);
        update_positioning_repeatedly(&mut solver);
        let i = solver.runners.iter().position(|r| r.position == 100.0);
        let i = i.unwrap();
        assert_ne!(
            solver.runners[i].positioning_strategy,
            PositioningStrategy::Normal
        );

        solver.runners[i].position = 100.0 + length - 0.1;
        solver.update_positioning_strategy();
        assert_ne!(
            solver.runners[i].positioning_strategy,
            PositioningStrategy::Normal,
            "{style:?} before the end"
        );

        // Checked again after a 1s cooldown and the 2s interval.
        solver.runners[i].position = 100.0 + length;
        solver.update_positioning_strategy();
        let runner = &solver.runners[i];
        assert_eq!(runner.positioning_strategy, PositioningStrategy::Normal);
        let next_check = solver.state.elapsed_time + 3.0;
        assert_eq!(runner.next_positioning_check, next_check);
    }
}

/// Solver with a single runner whose start delay is overridden.
fn start_delay_solver(start_delay: f64, rng_seed: u64) -> RaceSolver<Runner> {
    let mut solver = ready_solver(10101, uma(Style::Pace), vec![], seeded_config(rng_seed));
    solver.runners[0].start_delay = start_delay;
    // Positioning (checked from the start delay on) would change HP consumption.
    solver.runners[0].next_positioning_check = f64::INFINITY;
    solver
}

/// `(position, speed, hp)` of the only runner.
fn runner_state(solver: &RaceSolver<Runner>) -> (f64, f64, f64) {
    let runner = &solver.runners[0];
    (runner.position, runner.speed, runner.hp.current())
}

#[test]
fn test_start_delay_within_first_frame_prorates_distance_only() {
    let mut delayed = start_delay_solver(0.03, 0);
    let mut reference = start_delay_solver(0.0, 0);
    delayed.step(DELTA_TIME);
    reference.step(DELTA_TIME);

    let (position, speed, hp) = runner_state(&delayed);
    let (ref_position, ref_speed, ref_hp) = runner_state(&reference);
    assert_close!(
        ref_position,
        ref_speed * DELTA_TIME,
        "position without delay"
    );
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
        solver.add_runner(Entrant::new(uma, vec![]));
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
        solver.add_runner(Entrant::new(uma, vec![]));
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
    runners.sort_by(|a, b| a.finish_time.unwrap().total_cmp(&b.finish_time.unwrap()));
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
fn test_spot_struggle_lateral_exit_in_both_directions() {
    let mut solver = struggle_solver(
        &[(Style::Front, 200.0, 0.0), (Style::Front, 198.0, 0.0)],
        GameVersion::LATEST,
    );
    update_struggle(&mut solver);

    // Within 0.416 course widths, even away from the rail.
    place(&mut solver, 0, 200.0, 0.3);
    place(&mut solver, 1, 198.0, 0.3);
    update_struggle(&mut solver);
    assert_eq!(strugglers(&solver), [true, true]);

    // Beyond 0.416 course widths.
    place(&mut solver, 1, 198.0, 0.72);
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
fn test_spot_struggle_ends_upon_entering_ninth_section() {
    let mut solver = struggle_solver(&[(Style::Front, 330.0, 0.0)], GameVersion::LATEST);
    let elapsed_time = solver.state.elapsed_time;
    let RaceSolver { runners, state, .. } = &mut solver;
    let runner = &mut runners[0];
    runner.start_spot_struggle(elapsed_time);

    // Section 7 (0-indexed) is still fine.
    runner.position = 355.0;
    runner.update_phase(runner.position, state);
    assert!(runner.spot_struggle.is_active(elapsed_time));

    runner.position = 405.0;
    runner.update_phase(runner.position, state);
    assert_eq!(runner.spot_struggle, SpotStruggle::Ended);
}

#[test]
fn test_spot_struggle_not_before_first_anniversary() {
    for (version, struggle) in [
        (GameVersion::Original, false),
        (GameVersion::FirstAnniversary, true),
    ] {
        let mut solver = struggle_solver(
            &[(Style::Front, 200.0, 0.0), (Style::Front, 198.0, 0.0)],
            version,
        );
        update_struggle(&mut solver);
        assert_eq!(strugglers(&solver), [struggle, struggle], "{version:?}");
    }
}

#[test]
fn test_is_alone_counts_same_style_within_distance() {
    let cases = [
        (Style::Front, 590.0, 0, false),
        (Style::Front, 589.9, 0, true),
        (Style::Pace, 595.0, 0, true),
        // From the runner behind.
        (Style::Front, 595.0, 1, false),
    ];
    for (style, position, i, expected) in cases {
        let solver = struggle_solver(
            &[(Style::Front, 600.0, 0.2), (style, position, 0.2)],
            GameVersion::LATEST,
        );
        let actual = solver.is_alone(i, 10.0);
        assert_eq!(actual, expected, "{style:?} at {position}m, runner {i}");
    }
}

#[test]
fn test_secure_lead_needs_insufficient_lead_over_later_strategy() {
    let lead = secure_lead::desirable_lead(Style::Front, Style::Pace, 1200.0).unwrap();
    // Secure lead of the leader after each of 50 checks.
    let checks = |solver: &mut RaceSolver<Runner>| -> Vec<TimedBoost> {
        (1..=50)
            .map(|check| {
                solver.state.elapsed_time = 10.0 + check as f64 * secure_lead::CHECK_INTERVAL;
                solver.update_secure_lead();
                solver.runners[0].secure_lead
            })
            .collect()
    };
    let is_active = |boost: &&TimedBoost| matches!(boost, TimedBoost::Active { .. });
    let pace_behind = |position| {
        let runners = [(Style::Front, 600.0, 0.2), (Style::Pace, position, 0.2)];
        struggle_solver(&runners, GameVersion::LATEST)
    };

    let mut solver = pace_behind(600.0 - lead - 0.1);
    assert!(!checks(&mut solver).iter().any(|boost| is_active(&boost)));

    let mut solver = pace_behind(600.0 - lead + 0.1);
    let boost = *checks(&mut solver).iter().find(is_active).unwrap();
    // Alone: the Pace runner has another style.
    let guts = solver.runners[0].adjusted.guts;
    let expected = secure_lead::target_speed_bonus(Style::Front, guts, true, GameVersion::LATEST);
    assert_eq!(boost.speed_bonus(), expected);
}

#[test]
fn test_compete_before_spurt_when_far_from_first() {
    let threshold = compete_before_spurt::lead_threshold(Style::Front, 1200.0);
    for (behind, expected) in [(threshold + 0.1, true), (threshold - 0.1, false)] {
        let mut solver = struggle_solver(
            &[
                (Style::Runaway, 610.0, 0.2),
                (Style::Front, 610.0 - behind, 0.2),
            ],
            GameVersion::LATEST,
        );
        for runner in &mut solver.runners {
            // Never enter Stamina Keep, the only random roll.
            // ? Competing has no chance for now (its formula is unknown).
            runner.stamina_keep.ratio = 0.0;
        }
        solver.update_order_values();
        solver.update_compete_before_spurt();
        let actual = matches!(
            solver.runners[1].compete_before_spurt,
            TimedBoost::Active { .. }
        );
        assert_eq!(actual, expected, "{behind}m behind");
    }
}

/// Frames until a duel starts: the first frame two runners are close (frame
/// 0), then the 31 frames it takes to add up to 2s.
const DUEL_FRAMES: usize = 32;

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

/// Run `frames` duel updates, with up-to-date order rates.
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
    // Lane gap threshold is 0.25 course widths (about 2.8 horse lanes), from
    // the rail or away from it.
    for lanes in [[0.0, 0.249], [0.5, 0.3]] {
        let runners = [(1000.0, lanes[0]), (998.0, lanes[1])];
        let mut solver = duel_solver(&runners, GameVersion::LATEST);
        update_duels(&mut solver, DUEL_FRAMES);
        assert_eq!(duelists(&solver), [true, true], "{lanes:?}");
    }
}

#[test]
fn test_duel_speed_difference() {
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
fn test_duel_not_before_first_anniversary() {
    for (version, duel) in [
        (GameVersion::Original, false),
        (GameVersion::FirstAnniversary, true),
    ] {
        let mut solver = duel_solver(&[(1000.0, 0.1), (998.0, 0.1)], version);
        update_duels(&mut solver, DUEL_FRAMES * 2);
        assert_eq!(duelists(&solver), [duel, duel], "{version:?}");
    }
}

#[test]
fn test_duel_close_time_is_kept_per_pair() {
    // The first two runners are close from the start, and the last one gets
    // close to the one ahead of her a frame before they would start dueling.
    // Gates make the pairs share a slot if the pair index mixes them up.
    let cases: [(&[f64], &[u8], &[bool]); 2] = [
        (
            &[1000.0, 998.0, 960.0, 950.0],
            &[1, 4, 2, 3],
            &[true, true, false, false],
        ),
        (&[1000.0, 998.0, 990.0], &[1, 3, 2], &[true, true, false]),
    ];
    for (positions, gates, expected) in cases {
        let runners: Vec<_> = positions.iter().map(|&position| (position, 0.1)).collect();
        let mut solver = duel_solver(&runners, GameVersion::LATEST);
        for (runner, &gate) in solver.runners.iter_mut().zip(gates) {
            runner.gate_roll = gate;
        }
        update_duels(&mut solver, DUEL_FRAMES - 1);
        let last = positions.len() - 1;
        place_on_straight(&mut solver, last, positions[last - 1] - 1.0, 0.1);
        update_duels(&mut solver, 1);
        assert_eq!(duelists(&solver), expected, "gates {gates:?}");
    }
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

    // Threshold is 0.12 course widths, exclusive.
    let target_speed_at = |runner: &mut Runner, lane: f64| {
        runner.lane.current = CourseWidths(lane);
        runner.update_target_speed(state);
        runner.target_speed
    };
    let near_fence = target_speed_at(runner, 0.12);
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
fn test_move_lane_modifier_requires_skill_and_lane_movement() {
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
    let expected = (0.0002 * runner.adjusted.power).sqrt();
    assert_close!(runner.target_speed - still_speed, expected);

    // Only with a skill that changes the lane speed.
    runner.skill_modifiers.lane_speed = 0.0;
    runner.update_target_speed(state);
    assert_eq!(runner.target_speed, still_speed);
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
fn test_pace_down_lane_target() {
    // 0.18 course widths from the inner fence.
    let mut solver = positioning_solver(&[(Style::Front, 100.0)], GameVersion::LATEST);
    let runner = &mut solver.runners[0];
    runner.phase = Phase::Early;
    runner.lane.current = CourseWidths(0.4);
    runner.positioning_strategy = PositioningStrategy::PaceDown(0.0);
    let decision =
        solver.runners[0].normal_mode_lane_decision(&solver.state.course, &solver.runners);
    assert_close!(decision.target.0, 0.18);
}

#[test]
fn test_overlap_bumps_outer_runner() {
    // Bumped 0.4 horse lanes away from the inner runner, ahead or behind. In
    // the same lane, the runner behind is bumped.
    // ? Assumed, as the game's choice is unknown.
    let bumped = 0.19 + CourseWidths::from_horse_lanes(0.4).0;
    for (lanes, outer) in [([0.2, 0.19], 0), ([0.19, 0.2], 1), ([0.19, 0.19], 1)] {
        let mut solver = struggle_solver(
            &[
                (Style::Front, 200.0, lanes[0]),
                (Style::Front, 199.8, lanes[1]),
            ],
            GameVersion::LATEST,
        );
        solver.resolve_overlaps();
        let (outer, inner) = (&solver.runners[outer], &solver.runners[1 - outer]);
        assert_close!(outer.lane.current.0, bumped, "{lanes:?}");
        assert_eq!(outer.lane.target, outer.lane.current, "{lanes:?}");
        assert!(outer.should_update_target_lane(), "{lanes:?}");
        assert_eq!(inner.lane.current, CourseWidths(0.19), "{lanes:?}");
    }
}

#[test]
fn test_no_bump_at_the_outer_edge() {
    // Positioned on course 10101.
    let edge = load_course(10101).width.0;
    let mut solver = struggle_solver(
        &[(Style::Front, 200.0, edge), (Style::Front, 199.8, edge)],
        GameVersion::LATEST,
    );
    // Returns, rather than bumping the runner behind in place forever.
    solver.resolve_overlaps();
    for runner in &solver.runners {
        assert_eq!(runner.lane.current, CourseWidths(edge));
    }
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

    // Right at the edge of a 30m field of view.
    let mut solver = far_ahead_solver(10.0);
    solver.update_blocking_status(DELTA_TIME);
    assert_eq!(solver.runners[1].visible_count, 1);
}

#[test]
fn test_behind_in_checks_only_the_runner_right_behind() {
    let cases: [(&[(f64, f64)], bool); 3] = [
        (&[(599.0, 0.2)], true),
        (&[(599.0, 0.3)], false),
        (&[(599.0, 0.4), (598.0, 0.1)], false),
    ];
    for (behind, expected) in cases {
        let mut runners = vec![(Style::Pace, 600.0, 0.3)];
        runners.extend(
            behind
                .iter()
                .map(|&(position, lane)| (Style::Pace, position, lane)),
        );
        let mut solver = struggle_solver(&runners, GameVersion::LATEST);
        solver.update_blocking_status(DELTA_TIME);
        assert_eq!(solver.runners[0].nearby.behind_in, expected, "{behind:?}");
    }
}

#[test]
fn test_runners_close_behind_are_counted() {
    let mut solver = struggle_solver(
        &[(Style::Pace, 600.0, 0.2), (Style::Pace, 596.0, 0.2)],
        GameVersion::LATEST,
    );
    solver.update_blocking_status(DELTA_TIME);
    assert_eq!(solver.runners[0].nearby.behind_set1_count, 1);
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
fn test_team_members_are_allies() {
    let mut solver = RaceSolver::new(load_course(10101), race_params(), seeded_config(0));
    let member = || Entrant::new(uma(Style::Pace), vec![]);
    solver.add_team(vec![member(), member(), member()]);
    solver.add_runner(Entrant::new(uma(Style::Late), vec![]));
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
    let member = || Entrant::new(uma(Style::Pace), vec![]);
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
        solver.add_runner(Entrant::new(uma(Style::Pace), vec![]));
    }
}

#[test]
#[should_panic(expected = "Cannot add more runners than available gates")]
fn test_team_exceeding_gates_panics() {
    // 9 gates: 8 individual runners leave room for one more, not a team of two.
    let mut solver = RaceSolver::new(load_course(10101), race_params(), seeded_config(0));
    for _ in 0..8 {
        solver.add_runner(Entrant::new(uma(Style::Pace), vec![]));
    }
    let member = || Entrant::new(uma(Style::Pace), vec![]);
    solver.add_team(vec![member(), member()]);
}

#[test]
fn test_team_can_fill_the_last_gates() {
    // 9 gates: 6 individual runners leave room for a team of three.
    let mut solver = RaceSolver::new(load_course(10101), race_params(), seeded_config(0));
    for _ in 0..6 {
        solver.add_runner(Entrant::new(uma(Style::Pace), vec![]));
    }
    let member = || Entrant::new(uma(Style::Pace), vec![]);
    solver.add_team(vec![member(), member(), member()]);
    assert_eq!(solver.runners().len(), 9);
}

#[test]
fn test_style_of_the_most_popular_runner() {
    // A much stronger Late runner is the most popular, whatever the order.
    for strong_first in [true, false] {
        let mut solver = RaceSolver::new(load_course(10101), race_params(), seeded_config(0));
        let weak = Uma {
            speed: 100,
            stamina: 100,
            power: 100,
            guts: 100,
            wit: 100,
            ..uma(Style::Pace)
        };
        let mut umas = [uma(Style::Late), weak];
        if !strong_first {
            umas.reverse();
        }
        for uma in umas {
            solver.add_runner(Entrant::new(uma, vec![]));
        }
        let solver = solver.ready_runners();
        let most_popular = solver
            .runners
            .iter()
            .find(|runner| runner.initial_state.popularity_rank == 1)
            .unwrap();
        assert_eq!(most_popular.raw.style, Style::Late);
        assert_eq!(solver.state.style_most_popular, Style::Late);
    }
}

/// Put runner `i` in rush state.
fn set_rushed(solver: &mut RaceSolver<Runner>, i: usize) {
    solver.runners[i].rush_state = Some(RushState {
        section: 0,
        start_time: 1.0,
        end_time: 4.0,
        max_duration: 12.0,
        is_rushed: true,
        failed_checks: 0,
        style: Style::Pace,
    });
}

#[test]
fn test_rush_counters_by_position_count_opponents() {
    // In position order: team 0 (×2), team 1 and team 2.
    let mut solver = RaceSolver::new(load_course(10101), race_params(), seeded_config(0));
    let member = || Entrant::new(uma(Style::Pace), vec![]);
    solver.add_team(vec![member(), member()]);
    solver.add_runner(Entrant::new(uma(Style::Pace), vec![]));
    solver.add_runner(Entrant::new(uma(Style::Pace), vec![]));
    let mut solver = solver.ready_runners();
    solver.state.elapsed_time = 10.0;
    solver.runners.sort_by_key(|r| r.initial_state.team);
    for (i, position) in [400.0, 300.0, 200.0, 100.0].into_iter().enumerate() {
        solver.runners[i].position = position;
    }

    // Rushed: both team 0 runners and the team 1 runner.
    set_rushed(&mut solver, 0);
    set_rushed(&mut solver, 1);
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
            // Team 0 leader: of both runners behind, only the team 1 runner is
            // an opponent.
            (0, 0, 2, 1),
            // Team 0 member: the leader ahead is an ally.
            (1, 0, 1, 1),
            // Team 1: does not count itself.
            (2, 2, 0, 0),
            // Team 2: every rushed runner ahead is an opponent.
            (3, 3, 0, 0),
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
        Entrant::new(uma, vec![])
    };
    solver.add_team(vec![
        member(Style::Late),
        member(Style::Late),
        member(Style::Pace),
    ]);
    solver.add_team(vec![member(Style::Late), member(Style::End)]);
    for style in [Style::Runaway, Style::Runaway, Style::Front, Style::Pace] {
        solver.add_runner(member(style));
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
            runaway_rushed |= runner.raw.style == Style::Runaway;
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
    let member = |style| Entrant::new(uma(style), skills());
    solver.add_team(vec![member(Style::Front), member(Style::Late)]);
    solver.add_runner(Entrant::new(uma(Style::Pace), skills()));
    solver.add_runner(Entrant::new(uma(Style::End), vec![]));
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
        solver.add_runner(Entrant::new(uma(Style::Late), vec![]));
    }
    let mut solver = solver.ready_runners();
    solver.state.elapsed_time = 10.0;
    // Runners are built in gate order, so assign everything here.
    for (i, (runner, &(style, team))) in solver.runners.iter_mut().zip(runners).enumerate() {
        runner.raw.style = style;
        runner.style = style;
        runner.initial_state.team = team;
        runner.position = 1000.0 - 10.0 * i as f64;
    }
    solver
}

/// Targets of a skill effect casted by runner `caster` (in position order).
fn targets(solver: &RaceSolver<Runner>, caster: usize, target: SkillTarget) -> Vec<usize> {
    solver.select_targets(caster, target, 0.0)
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
    // More places than runners.
    assert_eq!(
        targets(&solver, 2, SkillTarget::AheadOfPosition { position: 10 }),
        [0, 1, 3, 4, 5]
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
    for roll in [0.0, 0.3, 0.5, 0.7, 0.999] {
        let selected = solver.select_targets(2, SkillTarget::RandomAlly, roll);
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
    assert_eq!(
        targets(&solver, 2, SkillTarget::AheadOfPosition { position: 3 }),
        [0]
    );
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
    solver.add_runner(Entrant::new(uma(Style::Late), vec![debuff]));
    solver.add_runner(Entrant::new(uma(Style::Late), vec![]));
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
    assert_eq!(buffs.iter().collect::<Vec<_>>(), [SkillType::TargetSpeed]);

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
        target_roll: 0.0,
        targets: None,
        record: None,
    }
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
        solver.add_runner(Entrant::new(uma(Style::Late), skills));
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
fn test_later_alternative_activates_again_after_cooldown() {
    let alternatives = vec![
        alternative("accumulatetime>=999", SkillType::TargetSpeed, 0.15),
        SkillAlternative {
            base_cooldown: 30.0,
            ..alternative("accumulatetime>=5", SkillType::TargetSpeed, 0.15)
        },
    ];
    let skill = Arc::new(synthetic_skill(alternatives));
    let mut solver = ready_solver(10101, uma(Style::Late), vec![skill], seeded_config(0));
    solver.run();
    assert!(solver.runners[0].activated_skills.len() > 1);
}

#[test]
fn test_hp_triggers_activate_within_their_range() {
    // (condition, HP percentage, whether the skill activates).
    let cases = [
        ("hp_per>=50", 49.9, false),
        ("hp_per>=50", 50.0, true),
        ("hp_per<=30", 30.9, true),
        ("hp_per<=30", 31.0, false),
    ];
    for (condition, percent, activates) in cases {
        let skills = vec![skill(1, condition, SkillType::TargetSpeed, 0.15)];
        let mut solver = ready_solver(10101, uma(Style::Late), skills, seeded_config(0));
        set_hp_percent(&mut solver, 0, percent);
        solver.runners[0].check_skills(&mut solver.state);
        let activated = !solver.runners[0].activated_skills.is_empty();
        assert_eq!(activated, activates, "{condition} at {percent}%");
    }
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

#[test]
fn test_activation_outlasting_race_removes_triggers_that_cant_succeed() {
    let once_per_race = vec![
        alternative("accumulatetime>=5", SkillType::TargetSpeed, 0.15),
        alternative("accumulatetime>=10", SkillType::TargetSpeed, 0.15),
        alternative(
            "is_activate_other_skill_detail==1&accumulatetime>=999",
            SkillType::TargetSpeed,
            0.15,
        ),
    ];
    let short_cooldown = SkillAlternative {
        base_cooldown: 30.0,
        ..alternative("accumulatetime>=5", SkillType::TargetSpeed, 0.15)
    };
    let skills = vec![
        Arc::new(synthetic_skill(once_per_race)),
        Arc::new(synthetic_skill(vec![short_cooldown])),
    ];
    let mut solver = ready_solver(10101, uma(Style::Late), skills, seeded_config(0));
    solver.run();

    // Only the continuation and the skill that can activate again are left.
    let scheduler = &solver.runners[0].trigger_scheduler;
    let mut left: Vec<_> = scheduler
        .time_windows
        .iter()
        .map(|window| {
            let logic = &scheduler.skill_logics[window.logic_idx];
            (logic.skill_index, logic.alternative_index)
        })
        .collect();
    left.sort_unstable();
    assert_eq!(left, [(0, 2), (1, 0)]);
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
    // It covers the moves of its usual duration (scaled with the course
    // distance) from the race start, the first one ending at the first check, so
    // it's gone by the check a frame after that duration.
    let distance = solver.state.course.distance as f64;
    let [effect] = runner.active_timed_effects.as_slice() else {
        panic!("Expected one active effect");
    };
    assert_close!(effect.expires_at, DELTA_TIME + 3.0 * distance / 1000.0);
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
    assert_eq!(solver.runners[0].style, Style::Runaway);
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
fn random_gold_activations(modifier: f64, skills: Vec<Arc<Skill>>) -> Vec<i32> {
    let mut ids = random_gold_activation_order(modifier, skills);
    ids.sort();
    ids
}

/// Like [`random_gold_activations`], but in activation order.
fn random_gold_activation_order(modifier: f64, mut skills: Vec<Arc<Skill>>) -> Vec<i32> {
    let adventure = skill(
        3,
        "distance_rate>=50",
        SkillType::ActivateRandomGold,
        modifier,
    );
    skills.push(adventure);
    let mut solver = ready_solver(10101, uma(Style::Pace), skills, seeded_config(0));
    solver.run();
    activations(&solver).into_iter().map(|(id, _)| id).collect()
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
fn test_activate_random_gold_records_the_source_skill_first() {
    let ids = random_gold_activation_order(1.0, vec![forced_gold_skill(1)]);
    assert_eq!(ids, vec![3, 1]);
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
    let skills = vec![with_wit_check(forced_gold_skill(15))];
    let mut solver = ready_solver(10101, uma(Style::Pace), skills, config);
    let runner = &mut solver.runners[0];
    assert_eq!(runner.failed_skills().len(), 1);
    let activate_random_gold = |runner: &mut Runner, state: &RaceState| {
        runner.apply_skill_effect(
            &effect(SkillType::ActivateRandomGold, 1.0),
            0.0,
            OvertakeExtension::default(),
            0.0,
            // As if cast by the skill itself, whose stream picks it.
            Some(0),
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
fn test_activate_unique_records_the_source_skill_first() {
    let unique = unique_skill(
        34,
        vec![alternative(
            "distance_rate>=10",
            SkillType::TargetSpeed,
            0.35,
        )],
    );
    let runner = unique_reactivation_runner(1.0, vec![unique]);
    let ids: Vec<_> = runner
        .activated_skills
        .iter()
        .map(|record| record.skill_id)
        .collect();
    assert_eq!(ids, vec![34, 4, 34]);
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
        .map(|i| with_wit_check(skill(i, "always==1", SkillType::TargetSpeed, 0.1)))
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
        (ValueScaling::FanLove, 3.0),
        // TODO: Update the following once the multipliers are known.
        (ValueScaling::ExcitementPoints, 1.0),
        (ValueScaling::PerformanceLevel, 1.2),
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
    solver.runners[0].initial_state.fan_count = 120000;
    assert_close!(activate_last_skill(&mut solver), 0.25 * 1.1);
}

#[test]
fn test_cinderella_gray_scaling() {
    // "Cinderella Gray" Oguri Cap's unique skill: its first alternative has
    // the scaling and the second one doesn't, with otherwise equal effects.
    // (course, fan count, bonus).
    let cases = [
        (10502, 1, 0.1),      // Nakayama 1600m (Mile).
        (10504, 200000, 0.0), // Nakayama 2000m (Medium).
        (10506, 146737, 0.0), // Arima Kinen, just below the fan count threshold.
        (10506, 146738, 0.2), // Arima Kinen, just above the fan count threshold.
    ];
    for (course_id, fan_count, bonus) in cases {
        let uma = Uma {
            fan_count: Some(fan_count),
            character_id: Some(100603),
            ..uma(Style::Late)
        };
        let skills = vec![data_skill(120061)];
        let mut solver = ready_solver(course_id, uma, skills, seeded_config(0));
        let runner = &mut solver.runners[0];
        let mut gained = |alternative| {
            let before = runner.skill_modifiers.target_speed;
            runner.activate_skill(0, alternative, &solver.state);
            runner.skill_modifiers.target_speed - before
        };
        let (scaled, unscaled) = (gained(0), gained(1));
        assert_close!(scaled - unscaled, bonus, "{course_id} {fan_count}");
    }
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
        (ValueScaling::MultiplySpeed1, 1850.0, 2.0),
        (ValueScaling::MultiplySpeed2, 1500.0, 2.0),
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
fn test_effects_on_oneself_are_buffs_unless_negative() {
    let mut solver = scaled_skill_solver(
        uma(Style::Late),
        vec![],
        ValueScaling::MultiplySpeed1,
        seeded_config(0),
    );
    // Too slow for any bonus, but scaled to zero still counts.
    solver.runners[0].adjusted.speed = 1600.0;
    assert_close!(activate_last_skill(&mut solver), 0.0);
    let buffs = &solver.runners[0].activation_flags.buff_activated;
    assert!(buffs.contains(SkillType::TargetSpeed));

    let skills = vec![skill(
        1,
        "is_activate_heal_skill==1",
        SkillType::TargetSpeed,
        -0.25,
    )];
    let mut solver = ready_solver(10101, uma(Style::Late), skills, seeded_config(0));
    assert_close!(activate_last_skill(&mut solver), -0.25);
    let buffs = &solver.runners[0].activation_flags.buff_activated;
    assert!(!buffs.contains(SkillType::TargetSpeed));
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
    solver.runners[0].position_diff.from_first = 20.0;
    assert_close!(activate_last_skill(&mut solver), 0.45);
}

#[test]
fn test_medium_distance_adds_to_value() {
    let skills = vec![scaled_skill(30, ValueScaling::AddDistanceTypeMedium, 0.25)];
    // 2400m (Medium).
    let mut solver = ready_solver(10606, uma(Style::Late), skills, seeded_config(0));
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
    block(runner, Phase::Mid, true, false, 1.0);
    assert_close!(runner.blocked.side_time, 7.0);
    assert_close!(runner.blocked.max_mid_side_time, 2.0);
    // Reset once both sides are clear; the longest blocking is kept.
    block(runner, Phase::Mid, false, false, 1.0);
    block(runner, Phase::Mid, true, false, 1.0);
    assert_close!(runner.blocked.side_time, 1.0);
    assert_close!(runner.blocked.max_mid_side_time, 2.0);
    // After the Mid race, blocking doesn't count either.
    block(runner, Phase::Late, true, false, 10.0);
    assert_close!(runner.blocked.max_mid_side_time, 2.0);

    // The longest blocking (2 s) reaches the second band.
    assert_close!(activate_last_skill(&mut solver), 0.25 * 2.0);
}

#[test]
fn test_largest_lead_before_late_race_is_tracked() {
    let mut solver = RaceSolver::new(load_course(10101), race_params(), seeded_config(0));
    let skills = vec![scaled_skill(30, ValueScaling::MultiplyTopLeadAmount, 0.25)];
    solver.add_runner(Entrant::new(uma(Style::Front), skills));
    solver.add_runner(Entrant::new(uma(Style::Late), vec![]));
    let mut solver = solver.ready_runners();
    let update = |solver: &mut RaceSolver<Runner>, front, late, phase| {
        for runner in &mut solver.runners {
            let is_front = runner.raw.style == Style::Front;
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
            |runner| runner.blocked.max_mid_side_time = 4.0,
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
    let runner = &mut solver.runners[0];
    runner.position = position;
    let transitions = runner.segments.update(position, course);
    runner.update_slope(&course.slopes, &transitions.slope, DELTA_TIME);
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
    solver.runners[0].adjusted.wit = 2500.0;

    move_on_slopes(&mut solver, &course, 99.0);
    assert!(!solver.runners[0].is_in_downhill_mode());
    move_on_slopes(&mut solver, &course, 100.0);
    assert!(solver.runners[0].is_in_downhill_mode());
    assert!(!solver.runners[0].is_going_uphill());

    // After exiting downhill mode, the next checks are every second after
    // entry: 16 frames, since 15 fall short of it, then 15 more thanks to the
    // time left over.
    for (check, frames) in [(116, 16), (131, 15)] {
        solver.runners[0].slope_speed_modifier = None;
        for position in check - frames + 1..check {
            move_on_slopes(&mut solver, &course, position as f64);
            assert!(
                !solver.runners[0].is_in_downhill_mode(),
                "position {position}"
            );
        }
        move_on_slopes(&mut solver, &course, check as f64);
        assert!(solver.runners[0].is_in_downhill_mode(), "position {check}");
    }

    // Exiting the downhill exits downhill mode.
    move_on_slopes(&mut solver, &course, 300.0);
    assert!(!solver.runners[0].is_in_downhill_mode());
}

#[test]
fn test_downhill_mode_chance_scales_with_wit() {
    // A 0.04% chance with 1 Wit, rather than a guaranteed entry.
    let mut solver = timed_skill_solver(vec![]);
    solver.runners[0].adjusted.wit = 1.0;
    let course = course_with_slopes(vec![Slope::new(100.0, 200.0, -2.0)]);
    move_on_slopes(&mut solver, &course, 99.0);
    move_on_slopes(&mut solver, &course, 100.0);
    assert!(!solver.runners[0].is_in_downhill_mode());
}

#[test]
fn test_uphills_climbed_extend_duration() {
    // Base duration of 3 seconds plus 0.5 seconds per uphill, scaled to the
    // 1200m course.
    let mut solver =
        timed_skill_solver(vec![timed_skill(33, DurationScaling::IncrementUphillCount)]);
    solver.runners[0].uphills_climbed = 2;
    solver.runners[0].activate_skill(0, 0, &solver.state);
    assert_close!(remaining_durations(&solver)[0], 4.0 * 1.2);
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

#[test]
fn test_front_blocker_limits_speed() {
    // Two blockers ahead in the same lane; the closest one (1m ahead) sets the
    // limit, at 99.4% of her speed.
    let mut solver = struggle_solver(
        &[
            (Style::Pace, 201.5, 0.2),
            (Style::Pace, 201.0, 0.2),
            (Style::Pace, 200.0, 0.2),
        ],
        GameVersion::LATEST,
    );
    for (runner, speed) in solver.runners.iter_mut().zip([15.0, 16.0, 20.0]) {
        runner.speed = speed;
        runner.target_speed = speed;
    }

    solver.update_blocking_status(DELTA_TIME);
    let behind = &solver.runners[2];
    assert_close!(behind.blocked.speed_ratio.unwrap(), 0.994);
    assert_eq!(
        behind.blocked.front_blocker,
        Some(solver.runners[1].gate_roll)
    );
}

#[test]
fn test_front_blocker_speed_is_read_in_gate_order() {
    // Runners move in gate order, so the runner behind (1m back) is limited by
    // the blocker's new speed if the blocker's gate is lower, or by her old
    // speed otherwise.
    for blocker_moves_first in [true, false] {
        let mut solver = struggle_solver(
            &[(Style::Pace, 201.0, 0.2), (Style::Pace, 200.0, 0.2)],
            GameVersion::LATEST,
        );
        let (blocker_gate, behind_gate) = if blocker_moves_first { (1, 2) } else { (2, 1) };
        for (runner, (gate, speed)) in solver
            .runners
            .iter_mut()
            .zip([(blocker_gate, 16.0), (behind_gate, 20.0)])
        {
            runner.gate_roll = gate;
            runner.speed = speed;
            runner.is_start_dash = false;
        }
        solver.update_blocking_status(DELTA_TIME);
        solver.step(DELTA_TIME);

        let speed = |gate| {
            solver
                .runners
                .iter()
                .find(|r| r.gate_roll == gate)
                .unwrap()
                .speed
        };
        let new_speed = speed(blocker_gate);
        assert!(
            (new_speed - 16.0).abs() > 0.01,
            "the blocker's speed must change"
        );
        let blocker_speed = if blocker_moves_first { new_speed } else { 16.0 };
        assert_close!(speed(behind_gate), 0.994 * blocker_speed);
    }
}

/// Ready race on a 1200m course with two Pace runners having `skills`.
fn pair_solver(skills: Vec<Arc<Skill>>) -> RaceSolver<Runner> {
    let mut solver = RaceSolver::new(load_course(10101), race_params(), seeded_config(0));
    for _ in 0..2 {
        solver.add_runner(Entrant::new(uma(Style::Pace), skills.clone()));
    }
    solver.ready_runners()
}

/// Assert that every runner first activated a skill at `expected` seconds.
fn assert_first_activations(solver: &RaceSolver<Runner>, expected: f64) {
    for runner in &solver.runners {
        let record = runner.activated_skills.first().expect("a skill activated");
        assert_close!(record.time, expected);
    }
}

#[test]
fn test_skills_see_nearby_state_from_a_frame_earlier() {
    // The runner behind closes in by about 0.13m per frame, so she's near (less
    // than 3m away) after the first step. Blocking and nearby state is only
    // updated after skill checks, so they see it a step later.
    let mut solver = pair_solver(vec![negligible_skill("near_count>=1")]);
    solver.state.elapsed_time = 10.0;
    for (i, (position, speed)) in [(203.05, 14.0), (200.0, 20.0)].into_iter().enumerate() {
        place(&mut solver, i, position, 0.2);
        let runner = &mut solver.runners[i];
        runner.speed = speed;
        runner.is_start_dash = false;
    }

    for _ in 0..3 {
        solver.step(DELTA_TIME);
    }
    assert_first_activations(&solver, 10.0 + 2.0 * DELTA_TIME);
}

#[test]
fn test_blocked_time_counts_from_the_first_move() {
    // Identical runners side by side from the gate stay side-blocked. Nothing
    // is updated before they move, so 1s of blocking is only reached on the
    // 17th frame (15 frames after the first update).
    let mut solver = pair_solver(vec![negligible_skill("blocked_side_continuetime>=1")]);
    for runner in &mut solver.runners {
        runner.start_delay = 0.0;
    }

    for _ in 0..18 {
        solver.step(DELTA_TIME);
    }
    assert_first_activations(&solver, 17.0 * DELTA_TIME);
}

#[test]
fn test_edge_gaps_are_infinite() {
    let mut solver = RaceSolver::new(load_course(10101), race_params(), seeded_config(0));
    for _ in 0..3 {
        solver.add_runner(Entrant::new(uma(Style::Pace), vec![]));
    }
    let mut solver = solver.ready_runners();
    for (runner, position) in solver.runners.iter_mut().zip([300.0, 200.0, 100.0]) {
        runner.position = position;
    }
    solver.update_order_values();

    let gaps = solver
        .runners
        .iter()
        .map(|r| (r.position_diff.from_next, r.position_diff.from_prev))
        .collect::<Vec<_>>();
    let inf = f64::INFINITY;
    assert_eq!(gaps, [(inf, 100.0), (100.0, 100.0), (100.0, inf)]);
}

#[test]
fn test_lone_runner_has_no_lead() {
    let mut solver = RaceSolver::new(load_course(10101), race_params(), seeded_config(0));
    solver.add_runner(Entrant::new(uma(Style::Front), vec![]));
    let mut solver = solver.ready_runners();
    solver.runners[0].position = 100.0;
    solver.update_order_values();
    assert_eq!(solver.runners[0].position_diff.max_lead, 0.0);
}

#[test]
fn test_distance_rate_is_relative_to_the_field_spread() {
    // A spread field, and a level one, where the rate is 0.
    let cases: [(&[f64], &[f64]); 2] = [
        (&[500.0, 400.0, 350.0, 100.0], &[0.0, 25.0, 37.5, 100.0]),
        (&[0.0, 0.0], &[0.0, 0.0]),
    ];
    for (positions, expected) in cases {
        let mut solver = RaceSolver::new(load_course(10101), race_params(), seeded_config(0));
        for _ in positions {
            solver.add_runner(Entrant::new(uma(Style::Pace), vec![]));
        }
        let mut solver = solver.ready_runners();
        for (runner, &position) in solver.runners.iter_mut().zip(positions) {
            runner.position = position;
        }
        solver.update_order_values();
        let rates: Vec<_> = solver
            .runners
            .iter()
            .map(|runner| runner.position_diff.rate_from_first)
            .collect();
        assert_eq!(rates, expected, "{positions:?}");
    }
}

#[test]
fn test_lone_runner_keeps_regular_pace() {
    for style in [Style::Front, Style::Pace, Style::Late, Style::End] {
        let mut solver = positioning_solver(&[(style, 200.0)], GameVersion::LATEST);
        solver.runners[0].section = 2;
        update_positioning_repeatedly(&mut solver);
        assert_eq!(
            solver.runners[0].positioning_strategy,
            PositioningStrategy::Normal,
            "{style:?}"
        );
    }
}

#[test]
fn test_start_dash_consumes_hp_at_base_speed() {
    // A lone runner on firm turf never paces down, and consumes 20 HP/s at
    // the base speed in the early race.
    let config = Configuration {
        rushing: Roll::Never,
        ..seeded_config(0)
    };
    let mut solver = ready_solver(10101, uma(Style::Front), vec![], config);
    let base_speed = solver.state.course.base_speed();
    let mut dash_frames = 0;
    loop {
        let runner = &solver.runners[0];
        let (start_dash, hp) = (runner.is_start_dash, runner.hp.current());
        let has_started = runner.start_delay <= solver.state.elapsed_time + DELTA_TIME;
        solver.step_frame();
        let runner = &solver.runners[0];
        let consumed = hp - runner.hp.current();
        if !has_started {
            assert_eq!(consumed, 0.0);
        } else if start_dash {
            // Whatever the current speed, including the frame that ends it.
            assert_close!(consumed, 20.0 * DELTA_TIME);
            dash_frames += 1;
        } else {
            // Then, the current speed is used again, which consumes less.
            let speed_diff = runner.speed - base_speed + 12.0;
            assert_close!(consumed, 20.0 * speed_diff.powi(2) / 144.0 * DELTA_TIME);
            assert!(runner.speed < base_speed);
            break;
        }
    }
    assert!(dash_frames > 1);
}

#[test]
fn test_step_frame_matches_run() {
    let new_solver = || {
        let mut solver = RaceSolver::new(load_course(10101), race_params(), seeded_config(0));
        solver.add_runner(Entrant::new(uma(Style::Front), vec![]));
        solver.add_runner(Entrant::new(uma(Style::End), vec![]));
        solver.ready_runners()
    };
    let mut run = new_solver();
    run.run();

    // Not finished until the last frame, when margins are recorded.
    let mut stepped = new_solver();
    while !stepped.step_frame() {
        assert!(stepped.runners.iter().all(|r| r.finish_margin.is_none()));
    }
    let results = |solver: &RaceSolver<Runner>| {
        let runners = solver.runners.iter();
        runners
            .map(|r| (r.gate_roll, r.finish_time, r.finish_margin))
            .collect::<Vec<_>>()
    };
    assert_eq!(results(&stepped), results(&run));

    // Once every runner has finished, frames don't advance anymore.
    let elapsed_time = stepped.state.elapsed_time;
    assert!(stepped.step_frame());
    assert_eq!(stepped.state.elapsed_time, elapsed_time);
}

/// Gates of four runners in a race with four gates, in the order they were
/// added, with gate 1 set for the third one if `set`.
fn gates_in_order(
    gate_assignment: Assignment,
    default_gate: DefaultGate,
    seed: u64,
    set: bool,
) -> Vec<u8> {
    let config = Configuration {
        gate_assignment,
        default_gate,
        ..seeded_config(seed)
    };
    let params = RaceParameters {
        gates: 4,
        ..race_params()
    };
    let mut solver = RaceSolver::new(load_course(10101), params, config);
    let ids: Vec<_> = (0..4)
        .map(|index| {
            let gate = (set && index == 2).then_some(1);
            let entrant = Entrant::new(uma(Style::Pace), vec![]);
            solver.add_runner(Entrant { gate, ..entrant })
        })
        .collect();
    let solver = solver.ready_runners();
    for runner in &solver.runners {
        // With up to 8 gates, every gate is a bracket of its own.
        assert_eq!(runner.initial_state.gate_block, runner.gate_roll);
    }
    ids.iter().map(|&id| solver.runner(id).gate_roll).collect()
}

#[test]
fn test_gate_assignments() {
    use Assignment::{All, Unset};
    use DefaultGate::{InOrder, Random};
    // In order, unset gates take the free ones.
    assert_eq!(gates_in_order(Unset, InOrder, 0, true), [2, 3, 1, 4]);
    assert_eq!(gates_in_order(Unset, InOrder, 0, false), [1, 2, 3, 4]);
    // In order for everyone: the set gate is ignored.
    assert_eq!(gates_in_order(All, InOrder, 0, true), [1, 2, 3, 4]);

    let mut random_unset = HashSet::new();
    for seed in 0..20 {
        // Random for unset gates: the set gate is kept.
        let gates = gates_in_order(Unset, Random, seed, true);
        assert_eq!(gates[2], 1);
        let mut sorted = gates.clone();
        sorted.sort();
        assert_eq!(sorted, [1, 2, 3, 4]);
        random_unset.insert(gates);

        // Random for everyone: the set gate is ignored, as if it wasn't set.
        assert_eq!(
            gates_in_order(All, Random, seed, true),
            gates_in_order(Unset, Random, seed, false)
        );
    }
    assert!(random_unset.len() > 1, "unset gates should be random");
}

/// Moods of three runners, in the order they were added: the first one set to
/// Bad, the others unset.
fn moods_in_order(mood_assignment: Assignment, default_mood: DefaultMood, seed: u64) -> Vec<Mood> {
    let config = Configuration {
        mood_assignment,
        default_mood,
        ..seeded_config(seed)
    };
    let mut solver = RaceSolver::new(load_course(10101), race_params(), config);
    let ids = [Some(Mood::Bad), None, None].map(|mood| {
        let entrant = Entrant::new(uma(Style::Pace), vec![]);
        solver.add_runner(Entrant { mood, ..entrant })
    });
    let solver = solver.ready_runners();
    let moods = ids.map(|id| solver.runner(id).initial_state.mood);
    for (id, mood) in ids.into_iter().zip(moods) {
        // Base stats follow the assigned mood.
        let runner = solver.runner(id);
        assert_eq!(runner.base, runner.raw.to_base(mood));
    }
    moods.to_vec()
}

#[test]
fn test_mood_assignments() {
    use Assignment::{All, Unset};
    use Mood::{Bad, Good};
    let good = DefaultMood::Fixed(Good);
    assert_eq!(moods_in_order(Unset, good, 0), [Bad, Good, Good]);
    assert_eq!(moods_in_order(All, good, 0), [Good, Good, Good]);
    // By default, every runner gets a Great mood.
    let default = Configuration::default();
    assert_eq!(
        moods_in_order(default.mood_assignment, default.default_mood, 0),
        [Mood::Great; 3]
    );

    let mut random_unset = HashSet::new();
    for seed in 0..20 {
        // Random for unset moods: the set mood is kept.
        let moods = moods_in_order(Unset, DefaultMood::Random, seed);
        assert_eq!(moods[0], Bad);
        random_unset.insert(moods[1..].to_vec());

        // Random for everyone: the unset ones get the same moods.
        let all = moods_in_order(All, DefaultMood::Random, seed);
        assert_eq!(all[1..], moods[1..]);
    }
    assert!(random_unset.len() > 1, "unset moods should be random");
}

/// Three runners ready to race, all bound to rush. The first one is an end
/// closer with a skill that multiplies her start delay and the given `pins`.
fn ready_with_pins(pins: Pins, seed: u64) -> RaceSolver<Runner> {
    let config = Configuration {
        wit_checks: Roll::Always,
        rushing: Roll::Always,
        ..seeded_config(seed)
    };
    let mut solver = RaceSolver::new(load_course(10101), race_params(), config);
    let skill = skill(1, "always==1", SkillType::MultiplyStartDelay, 0.4);
    solver.add_runner(Entrant {
        pins,
        ..Entrant::new(uma(Style::End), vec![skill])
    });
    for _ in 0..2 {
        solver.add_runner(Entrant::new(uma(Style::Pace), vec![]));
    }
    solver.ready_runners()
}

/// Mutable runner with ID `id`.
fn runner_mut(solver: &mut RaceSolver<Runner>, id: RunnerId) -> &mut Runner {
    let gate = solver.gates_by_id[id.0 as usize];
    let runner = solver.runners.iter_mut().find(|r| r.gate_roll == gate);
    runner.expect("every ID has a runner")
}

/// Rush state of a runner bound to rush.
fn rush_of(runner: &Runner) -> &RushState {
    runner.rush_state.as_ref().expect("she's bound to rush")
}

#[test]
fn test_pins_change_only_their_outcome() {
    // (pins, check of the pinned runner, restore of the pinned outcome from the
    // unpinned runner).
    type Check = fn(&Runner) -> bool;
    type Restore = fn(&mut Runner, &Runner);
    let rush_pins = |rush| Pins {
        rush,
        ..Pins::default()
    };
    let cases: [(Pins, Check, Restore); 4] = [
        (
            Pins {
                start_delay: Some(0.25),
                ..Pins::default()
            },
            |r| r.start_delay == 0.25,
            |r, unpinned| r.start_delay = unpinned.start_delay,
        ),
        // Whatever `Roll::Always` says.
        (
            rush_pins(RushPins {
                rushes: Some(false),
                ..RushPins::default()
            }),
            |r| r.rush_state.is_none(),
            |r, unpinned| r.rush_state = unpinned.rush_state.clone(),
        ),
        // A style end closers never roll.
        (
            rush_pins(RushPins {
                style: Some(Style::End),
                ..RushPins::default()
            }),
            |r| rush_of(r).style == Style::End,
            |r, unpinned| r.rush_state.as_mut().unwrap().style = rush_of(unpinned).style,
        ),
        // A section outside the rolled ones.
        (
            rush_pins(RushPins {
                section: Some(12),
                ..RushPins::default()
            }),
            |r| rush_of(r).section == 12,
            |r, unpinned| r.rush_state.as_mut().unwrap().section = rush_of(unpinned).section,
        ),
    ];
    let id = RunnerId(0);
    for (pins, check, restore) in cases {
        for seed in 0..10 {
            let unpinned = ready_with_pins(Pins::default(), seed);
            // The skill applies when unpinned (at most 40% of 0.1 s).
            assert!(unpinned.runner(id).start_delay <= 0.04, "seed {seed}");

            let mut pinned = ready_with_pins(pins.clone(), seed);
            let runner = runner_mut(&mut pinned, id);
            assert!(check(runner), "{pins:?}, seed {seed}");
            // Every other outcome, hers included, is the same.
            restore(runner, unpinned.runner(id));
            runner.pins = Pins::default();
            assert_eq!(pinned.runners, unpinned.runners, "{pins:?}, seed {seed}");
        }
    }
}

#[test]
fn test_pinned_failed_checks_set_rush_duration() {
    for (failed_checks, seed) in [(0, 0), (1, 1), (2, 2), (2, 3)] {
        let pins = Pins {
            rush: RushPins {
                rushes: Some(true),
                section: Some(1),
                failed_checks: Some(failed_checks),
                ..RushPins::default()
            },
            ..Pins::default()
        };
        let mut solver = ready_with_pins(pins, seed);
        let id = RunnerId(0);
        while !solver.runner(id).has_rushed() || solver.runner(id).is_rushed() {
            assert!(!solver.step_frame(), "the race ended first");
        }
        // Each check happens on the first frame after it's due.
        let rush_state = rush_of(solver.runner(id));
        let duration = rush_state.end_time - rush_state.start_time;
        let checks = (failed_checks + 1) as f64;
        let expected = rush::CHECK_INTERVAL * checks;
        assert!(
            (expected..expected + checks * DELTA_TIME).contains(&duration),
            "{failed_checks} failed checks, seed {seed}: {duration} s"
        );
    }
}

#[test]
#[should_panic(expected = "Invalid start delay -0.01")]
fn test_negative_start_delay_pin_panics() {
    add_with_pins(Pins {
        start_delay: Some(-0.01),
        ..Pins::default()
    });
}

#[test]
#[should_panic(expected = "Invalid start delay NaN")]
fn test_nan_start_delay_pin_panics() {
    add_with_pins(Pins {
        start_delay: Some(f64::NAN),
        ..Pins::default()
    });
}

#[test]
#[should_panic(expected = "Invalid rush section 0: expected 1 to 23")]
fn test_first_rush_section_pin_panics() {
    add_with_pins(rush_section_pins(0));
}

#[test]
#[should_panic(expected = "Invalid rush section 24: expected 1 to 23")]
fn test_rush_section_pin_past_the_goal_panics() {
    add_with_pins(rush_section_pins(24));
}

#[test]
fn test_pinned_wit_checks_override_configuration() {
    // (`wit_checks`, whether the skill has a wit check, pin, whether it passes).
    let cases = [
        (Roll::Always, true, Some(false), false),
        (Roll::Never, true, Some(true), true),
        // Like a unique.
        (Roll::Always, false, Some(false), false),
        // `wit_checks` only forces skills that have a wit check.
        (Roll::Never, false, None, true),
    ];
    for (wit_checks, has_wit_check, pin, passes) in cases {
        let config = Configuration {
            wit_checks,
            ..seeded_config(0)
        };
        let mut skill = skill(1, "always==1", SkillType::TargetSpeed, 0.1);
        if has_wit_check {
            skill = with_wit_check(skill);
        }
        let mut pins = Pins::default();
        if let Some(pin) = pin {
            pins.skills.insert(1, SkillPin::WitCheck(pin));
        }
        let mut solver = RaceSolver::new(load_course(10101), race_params(), config);
        let id = solver.add_runner(Entrant {
            pins,
            ..Entrant::new(uma(Style::Pace), vec![skill])
        });
        let solver = solver.ready_runners();
        let runner = solver.runner(id);
        let case = (wit_checks, has_wit_check, pin);
        assert_eq!(runner.available_skills().len(), passes as usize, "{case:?}");
        assert_eq!(runner.failed_skills().len(), !passes as usize, "{case:?}");
    }
}

#[test]
fn test_pinned_wit_check_changes_no_other_runner() {
    let mut pins = Pins::default();
    pins.skills.insert(1, SkillPin::WitCheck(false));
    let id = RunnerId(0);
    for seed in 0..10 {
        let unpinned = ready_with_pins(Pins::default(), seed);
        let pinned = ready_with_pins(pins.clone(), seed);
        assert_eq!(pinned.runner(id).failed_skills().len(), 1, "seed {seed}");
        assert_eq!(others(&pinned, id), others(&unpinned, id), "seed {seed}");
    }
}

/// Runners other than the one with ID `id`.
fn others(solver: &RaceSolver<Runner>, id: RunnerId) -> Vec<&Runner> {
    let gate = solver.runner(id).gate_roll;
    let others = solver.runners.iter().filter(|r| r.gate_roll != gate);
    others.collect()
}

#[test]
#[should_panic(expected = "No skill 1 to pin")]
fn test_wit_check_pin_without_the_skill_panics() {
    let mut pins = Pins::default();
    pins.skills.insert(1, SkillPin::WitCheck(true));
    add_with_pins(pins);
}

/// Pinned activation of alternative 0 at `time`, with the usual targets.
fn pinned_at(time: f64) -> PinnedActivation {
    PinnedActivation {
        time,
        alternative: 0,
        targets: None,
    }
}

/// Pins with `activations` of the skill with ID `id`.
fn activation_pins(id: i32, activations: Vec<PinnedActivation>) -> Pins {
    let mut pins = Pins::default();
    pins.skills.insert(id, SkillPin::Activations(activations));
    pins
}

/// Single runner with `skills` and `pins`, ready to race.
fn ready_alone(style: Style, skills: Vec<Arc<Skill>>, pins: Pins) -> RaceSolver<Runner> {
    let config = Configuration {
        wit_checks: Roll::Never,
        ..seeded_config(0)
    };
    let mut solver = RaceSolver::new(load_course(10101), race_params(), config);
    solver.add_runner(Entrant {
        pins,
        ..Entrant::new(uma(style), skills)
    });
    solver.ready_runners()
}

#[test]
fn test_pinned_activations_happen_at_their_times() {
    // Unsorted, the second one during the cooldown of the first. The skill
    // would activate on the first frame, and fails its wit check.
    for times in [vec![], vec![2.0], vec![5.0, 2.0]] {
        let skill = with_wit_check(skill(1, "always==1", SkillType::TargetSpeed, 0.1));
        let pins = activation_pins(1, times.iter().map(|&time| pinned_at(time)).collect());
        let mut solver = ready_alone(Style::Pace, vec![skill], pins);
        // Start times of the frames each activation happened on.
        let mut frames = Vec::new();
        loop {
            let start = solver.state.elapsed_time;
            let count = solver.runners[0].activated_skills.len();
            if solver.step_frame() {
                break;
            }
            let activated = solver.runners[0].activated_skills.len() - count;
            frames.extend(std::iter::repeat_n(start, activated));
        }
        let mut times = times.clone();
        times.sort_by(f64::total_cmp);
        assert_eq!(frames.len(), times.len(), "{times:?}");
        for (start, time) in frames.into_iter().zip(&times) {
            assert!(
                (start..start + DELTA_TIME).contains(time),
                "{time} at {start}"
            );
        }
    }
}

#[test]
fn test_pinned_out_of_gate_activations_replace_their_conditions() {
    let delay = ready_alone(Style::End, vec![], Pins::default()).runners[0].start_delay;
    // (condition, pinned activations, start delay multiplier). End closers
    // aren't front runners (`running_style==1`).
    let cases = [
        ("always==1", None, 0.4),
        ("always==1", Some(vec![]), 1.0),
        ("running_style==1", None, 1.0),
        ("running_style==1", Some(vec![pinned_at(0.0)]), 0.4),
    ];
    for (condition, activations, multiplier) in cases {
        let skill = skill(1, condition, SkillType::MultiplyStartDelay, 0.4);
        let pins = activations
            .clone()
            .map_or_else(Pins::default, |a| activation_pins(1, a));
        let mut solver = ready_alone(Style::End, vec![skill], pins);
        let case = (condition, &activations);
        assert_close!(
            solver.runners[0].start_delay,
            delay * multiplier,
            "{case:?}"
        );
        // They still activate on the first frame she moves.
        while solver.state.elapsed_time < delay {
            solver.step_frame();
        }
        let activated = !solver.runners[0].activated_skills.is_empty();
        assert_eq!(activated, multiplier < 1.0, "{case:?}");
    }
}

#[test]
fn test_forced_activations_skip_pinned_skills() {
    let cases = [
        (SkillType::ActivateRandomGold, SkillRarity::Gold),
        (SkillType::ActivateUnique, SkillRarity::Unique),
    ];
    for (forcing, rarity) in cases {
        for pinned in [false, true] {
            let forced = Arc::new(Skill {
                rarity,
                ..Arc::unwrap_or_clone(forced_gold_skill(15))
            });
            let skills = vec![forced, skill(3, "distance_rate>=50", forcing, 1.0)];
            let pins = if pinned {
                activation_pins(15, vec![])
            } else {
                Pins::default()
            };
            let mut solver = ready_alone(Style::Pace, skills, pins);
            solver.run();
            let ids: Vec<_> = activations(&solver).into_iter().map(|(id, _)| id).collect();
            let expected = if pinned { vec![3] } else { vec![3, 15] };
            assert_eq!(ids, expected, "{forcing:?}, pinned: {pinned}");
        }
    }
}

#[test]
fn test_pinned_targets_replace_the_usual_ones() {
    // (pinned targets, runners reached).
    let cases = [
        (None, vec![0, 1, 2]),
        (Some(vec![RunnerId(2)]), vec![2]),
        (Some(vec![]), vec![]),
    ];
    for (targets, reached) in cases {
        let alternative = SkillAlternative {
            effects: vec![SkillEffect {
                target: SkillTarget::All,
                ..effect(SkillType::TargetSpeed, -0.1)
            }],
            ..alternative("always==1", SkillType::TargetSpeed, 0.0)
        };
        let skill = Arc::new(Skill {
            id: 1,
            ..synthetic_skill(vec![alternative])
        });
        let activation = PinnedActivation {
            targets: targets.clone(),
            ..pinned_at(1.0)
        };
        let mut solver = RaceSolver::new(load_course(10101), race_params(), seeded_config(0));
        solver.add_runner(Entrant {
            pins: activation_pins(1, vec![activation]),
            ..Entrant::new(uma(Style::Pace), vec![skill])
        });
        for _ in 0..2 {
            solver.add_runner(Entrant::new(uma(Style::Pace), vec![]));
        }
        let mut solver = solver.ready_runners();
        while solver.runner(RunnerId(0)).activated_skills.is_empty() {
            assert!(!solver.step_frame(), "the race ended first");
        }
        let affected: Vec<_> = (0..3)
            .filter(|&id| !solver.runner(RunnerId(id)).active_timed_effects.is_empty())
            .collect();
        assert_eq!(affected, reached, "{targets:?}");
        // Her record gives the runners reached.
        let record = &solver.runner(RunnerId(0)).activated_skills[0];
        let recorded: Vec<_> = record.affected.iter().map(|id| id.0).collect();
        assert_eq!(recorded, reached, "{targets:?}");
    }
}

#[test]
fn test_records_give_the_duration_and_affected_runners() {
    let on_self = |skill_type, modifier| effect(skill_type, modifier);
    let debuff = SkillEffect {
        target: SkillTarget::All,
        ..effect(SkillType::TargetSpeed, -0.1)
    };
    // (base duration, effects, duration on the 1200 m course, runners
    // affected). Effects on herself make her the only one recorded.
    let cases = [
        (
            3.0,
            vec![on_self(SkillType::TargetSpeed, 0.1)],
            3.6,
            vec![0],
        ),
        (0.0, vec![on_self(SkillType::Recovery, 0.01)], 0.0, vec![0]),
        (
            f64::INFINITY,
            vec![on_self(SkillType::Fov, 1.0)],
            f64::INFINITY,
            vec![0],
        ),
        (
            3.0,
            vec![on_self(SkillType::TargetSpeed, 0.1), debuff.clone()],
            3.6,
            vec![0],
        ),
        // Runners reached by several effects (all of them, her included) are
        // recorded once.
        (3.0, vec![debuff.clone(), debuff.clone()], 3.6, vec![0, 1]),
    ];
    for (base_duration, effects, duration, affected) in cases {
        let alternative = SkillAlternative {
            base_duration,
            effects: effects.clone(),
            ..alternative("always==1", SkillType::TargetSpeed, 0.0)
        };
        let skill = Arc::new(Skill {
            id: 1,
            ..synthetic_skill(vec![alternative])
        });
        let mut solver = RaceSolver::new(load_course(10101), race_params(), seeded_config(0));
        solver.add_runner(Entrant::new(uma(Style::Pace), vec![skill]));
        solver.add_runner(Entrant::new(uma(Style::Pace), vec![]));
        let mut solver = solver.ready_runners();
        while solver.runner(RunnerId(0)).activated_skills.is_empty() {
            assert!(!solver.step_frame(), "the race ended first");
        }
        let record = &solver.runner(RunnerId(0)).activated_skills[0];
        let recorded: Vec<_> = record.affected.iter().map(|id| id.0).collect();
        let case = (base_duration, &effects);
        assert_close!(record.duration, duration, "{case:?}");
        assert_eq!(recorded, affected, "{case:?}");
    }
}

#[test]
#[should_panic(expected = "Invalid alternative 1 for skill 1: expected 0 to 0")]
fn test_pinned_activation_of_a_missing_alternative_panics() {
    let skill = skill(1, "always==1", SkillType::TargetSpeed, 0.1);
    add_with_activations(skill, vec![pinned_alternative_at(1.0, 1)]);
}

#[test]
#[should_panic(expected = "Invalid time -1 for skill 1: expected a finite, non-negative number")]
fn test_pinned_activation_before_the_start_panics() {
    let skill = skill(1, "always==1", SkillType::TargetSpeed, 0.1);
    add_with_activations(skill, vec![pinned_at(-1.0)]);
}

#[test]
#[should_panic(expected = "Invalid time 1 for skill 1: out-of-gate alternatives activate at 0")]
fn test_pinned_out_of_gate_activation_during_the_race_panics() {
    let skill = out_of_gate_skill(1, 1.0, &[(SkillType::SpeedUp, 40.0)]);
    add_with_activations(skill, vec![pinned_at(1.0)]);
}

#[test]
#[should_panic(expected = "Skill 1 activates out of gate 2 times: expected at most once")]
fn test_pinned_out_of_gate_activation_twice_panics() {
    let skill = out_of_gate_skill(1, 1.0, &[(SkillType::SpeedUp, 40.0)]);
    add_with_activations(skill, vec![pinned_at(0.0), pinned_at(0.0)]);
}

/// Run a race alone with `skill` (ID 1), pinned to activate at `times`, and
/// return whether each pinned activation met its conditions, and the times of
/// the activations they allowed but weren't pinned.
fn condition_report(skill: Arc<Skill>, times: &[f64]) -> (Vec<bool>, Vec<f64>) {
    let pins = activation_pins(1, times.iter().map(|&time| pinned_at(time)).collect());
    let mut solver = ready_alone(Style::Pace, vec![skill], pins);
    solver.run();
    let runner = &solver.runners[0];
    let met = runner.activated_skills.iter().map(|r| r.conditions_met);
    let missing = runner.missing_activations.iter().map(|r| r.time);
    (met.collect(), missing.collect())
}

#[test]
fn test_pinned_skills_report_what_their_conditions_allow() {
    let window = || skill(1, "accumulatetime>=10", SkillType::TargetSpeed, 0.1);
    // The first frame the skill's conditions allow.
    let (_, missing) = condition_report(window(), &[]);
    let [open] = missing[..] else {
        panic!("expected a single activation: {missing:?}");
    };
    assert!((10.0..10.0 + DELTA_TIME).contains(&open), "{open}");

    // Mid race, from 200 to 800m, is reached in the first 20 seconds and
    // lasts more than 25 seconds.
    let random = || skill(1, "phase_random==1", SkillType::TargetSpeed, 0.1);
    // (skill, pinned times, whether each met its conditions, missing times).
    let cases = [
        // A wit check can explain a missing activation, unless she has one.
        (with_wit_check(window()), vec![], vec![], vec![]),
        (window(), vec![open - DELTA_TIME / 2.0], vec![true], vec![]),
        (window(), vec![open - 2.0], vec![false], vec![open]),
        // Unconfirmed, the activation she would have had starts the cooldown.
        (
            with_wit_check(window()),
            vec![open + 1.0],
            vec![false],
            vec![open],
        ),
        // Random variables can be anywhere in their area, and explain a missing
        // activation.
        (random(), vec![25.0], vec![true], vec![]),
        (random(), vec![3.0], vec![false], vec![]),
        (random(), vec![], vec![], vec![]),
    ];
    for (skill, times, met, missing) in cases {
        let case = (skill.wit_check, &skill.alternatives[0].condition, &times);
        assert_eq!(
            condition_report(skill.clone(), &times),
            (met, missing),
            "{case:?}"
        );
    }
}

#[test]
fn test_unpinned_skills_meet_their_conditions() {
    let skill = skill(1, "accumulatetime>=10", SkillType::TargetSpeed, 0.1);
    let mut solver = ready_alone(Style::Pace, vec![skill], Pins::default());
    solver.run();
    let runner = &solver.runners[0];
    let [record] = &runner.activated_skills[..] else {
        panic!(
            "expected a single activation: {:?}",
            runner.activated_skills
        );
    };
    assert_eq!((record.skill_id, record.alternative), (1, 0));
    assert!(
        (10.0..10.0 + DELTA_TIME).contains(&record.time),
        "{record:?}"
    );
    assert!(record.conditions_met);
    assert!(runner.missing_activations.is_empty());
}

#[test]
fn test_out_of_gate_activations_are_recorded_at_the_start() {
    // She moves on the second frame, which activates the skill.
    let delay = 1.5 * DELTA_TIME;
    let pins = Pins {
        start_delay: Some(delay),
        ..Pins::default()
    };
    let skill = out_of_gate_skill(1, 1.0, &[(SkillType::SpeedUp, 40.0)]);
    let mut solver = ready_alone(Style::Pace, vec![skill], pins);
    while solver.state.elapsed_time < delay {
        solver.step_frame();
    }
    let record = &solver.runners[0].activated_skills[0];
    assert_eq!((record.time, record.distance), (0.0, 0.0));
}

#[test]
fn test_pinned_out_of_gate_skills_report_what_their_conditions_allow() {
    let skill = |condition| skill(1, condition, SkillType::MultiplyStartDelay, 0.4);
    // End closers aren't front runners (`running_style==1`). (skill, pinned
    // activations, whether it met its conditions, whether it would have
    // activated).
    let cases = [
        (
            skill("running_style==1"),
            vec![pinned_at(0.0)],
            Some(false),
            false,
        ),
        (skill("always==1"), vec![pinned_at(0.0)], Some(true), false),
        (skill("always==1"), vec![], None, true),
        // Random lots always hit, and like wit checks, can explain a missing
        // activation.
        (
            skill("random_lot==1"),
            vec![pinned_at(0.0)],
            Some(true),
            false,
        ),
        (skill("random_lot==100"), vec![], None, false),
        (with_wit_check(skill("always==1")), vec![], None, false),
    ];
    for (skill, activations, met, missing) in cases {
        let case = (
            skill.wit_check,
            &skill.alternatives[0].condition,
            &activations,
        );
        let pins = activation_pins(1, activations.clone());
        let mut solver = ready_alone(Style::End, vec![skill.clone()], pins);
        solver.run();
        let runner = &solver.runners[0];
        let record = runner.activated_skills.first();
        assert_eq!(record.map(|r| r.conditions_met), met, "{case:?}");
        // Base duration of 3 s on a 1200 m course.
        let expected = missing.then_some((1, 0, 0.0, 0.0, 3.6, vec![runner.id]));
        let record = runner.missing_activations.first();
        let record = record.map(|r| {
            let affected = r.affected.clone();
            (
                r.skill_id,
                r.alternative,
                r.time,
                r.distance,
                r.duration,
                affected,
            )
        });
        assert_eq!(record, expected, "{case:?}");
    }
}

/// Pinned activation of `alternative` at `time`, with the usual targets.
fn pinned_alternative_at(time: f64, alternative: usize) -> PinnedActivation {
    PinnedActivation {
        alternative,
        ..pinned_at(time)
    }
}

/// Add a runner with `skill`, pinned to activate as in `activations`.
fn add_with_activations(skill: Arc<Skill>, activations: Vec<PinnedActivation>) {
    let mut solver = RaceSolver::new(load_course(10101), race_params(), seeded_config(0));
    solver.add_runner(Entrant {
        pins: activation_pins(skill.id, activations),
        ..Entrant::new(uma(Style::Pace), vec![skill])
    });
}

#[test]
#[should_panic(expected = "No runner with RunnerId(1) to target")]
fn test_pinned_target_outside_the_race_panics() {
    let activation = PinnedActivation {
        targets: Some(vec![RunnerId(1)]),
        ..pinned_at(1.0)
    };
    let skill = skill(1, "always==1", SkillType::TargetSpeed, 0.1);
    ready_alone(
        Style::Pace,
        vec![skill],
        activation_pins(1, vec![activation]),
    );
}

/// `skill` with a wit check.
fn with_wit_check(skill: Arc<Skill>) -> Arc<Skill> {
    Arc::new(Skill {
        wit_check: true,
        ..Arc::unwrap_or_clone(skill)
    })
}

/// Pins of the rush section only.
fn rush_section_pins(section: u8) -> Pins {
    Pins {
        rush: RushPins {
            section: Some(section),
            ..RushPins::default()
        },
        ..Pins::default()
    }
}

/// Add a runner with `pins` to a race.
fn add_with_pins(pins: Pins) {
    let mut solver = RaceSolver::new(load_course(10101), race_params(), seeded_config(0));
    solver.add_runner(Entrant {
        pins,
        ..Entrant::new(uma(Style::Pace), vec![])
    });
}

/// Entrant with the given gate.
fn entrant_at(gate: u8) -> Entrant {
    Entrant {
        gate: Some(gate),
        ..Entrant::new(uma(Style::Pace), vec![])
    }
}

#[test]
fn test_last_gate_can_be_set() {
    let params = RaceParameters {
        gates: MAX_GATES,
        ..race_params()
    };
    let mut solver = RaceSolver::new(load_course(10101), params, seeded_config(0));
    let id = solver.add_runner(entrant_at(MAX_GATES));
    let solver = solver.ready_runners();
    assert_eq!(solver.runner(id).gate_roll, MAX_GATES);
}

#[test]
#[should_panic(expected = "Gate 1 is already set for another runner")]
fn test_add_runner_rejects_gates_of_other_runners() {
    let mut solver = RaceSolver::new(load_course(10101), race_params(), seeded_config(0));
    solver.add_runner(entrant_at(1));
    solver.add_runner(entrant_at(2));
    solver.add_runner(entrant_at(1));
}

#[test]
#[should_panic(expected = "Gate 2 is already set for another runner")]
fn test_add_team_rejects_gates_of_other_members() {
    let mut solver = RaceSolver::new(load_course(10101), race_params(), seeded_config(0));
    solver.add_team(vec![entrant_at(2), entrant_at(2)]);
}

#[test]
#[should_panic(expected = "Invalid gate 10: expected 1 to 9")]
fn test_add_runner_rejects_gates_out_of_range() {
    let mut solver = RaceSolver::new(load_course(10101), race_params(), seeded_config(0));
    solver.add_runner(entrant_at(10));
}

#[test]
fn test_runner_finds_runners_by_id_after_reordering() {
    let mut solver = RaceSolver::new(load_course(10101), race_params(), seeded_config(0));
    let styles = [Style::End, Style::Late, Style::Pace, Style::Front];
    let ids = styles.map(|style| solver.add_runner(Entrant::new(uma(style), vec![])));
    let mut solver = solver.ready_runners();
    solver.run();
    for (id, style) in ids.into_iter().zip(styles) {
        assert_eq!(solver.runner(id).raw.style, style);
    }
}

#[test]
#[should_panic(expected = "No runner with RunnerId(1) in this race")]
fn test_runner_rejects_runners_of_other_races() {
    let new_solver = || RaceSolver::new(load_course(10101), race_params(), seeded_config(0));
    let mut other = new_solver();
    other.add_runner(Entrant::new(uma(Style::Pace), vec![]));
    let id = other.add_runner(Entrant::new(uma(Style::Pace), vec![]));
    let mut solver = new_solver();
    solver.add_runner(Entrant::new(uma(Style::Pace), vec![]));
    solver.ready_runners().runner(id);
}
