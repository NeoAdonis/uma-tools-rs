use super::*;
use crate::course::LaneType;
use crate::race::spot_struggle::SpotStruggle;
use crate::test_utils::race_params;

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
    for course_id in ["10101", "10104", "10606", "10913"] {
        let course = Course::from_json_file("data/course_data.json", course_id)
            .expect("Failed to load course data");
        let mut solver = RaceSolver::new(course, race_params(0), &Configuration::default());
        for style in [Style::Front, Style::Pace, Style::Late, Style::End] {
            let uma = Uma {
                speed: 1200,
                stamina: 1000,
                power: 1000,
                guts: 800,
                wiz: 1000,
                style,
                distance_aptitude: Aptitude::A,
                track_aptitude: Aptitude::A,
                style_aptitude: Aptitude::A,
            };
            solver.add_runner(uma, Motivation::Normal, vec![]);
        }
        let mut solver = solver.ready_runners();

        while solver.state.finish_time.is_none() {
            solver.step(1.0 / 15.0);

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
    let course = Course::from_json_file("data/course_data.json", "10108")
        .expect("Failed to load course data");
    assert_eq!(course.distance, 2400);
    let skill = Skill::new(
        "late".to_string(),
        SkillData {
            rarity: SkillRarity::White,
            alternatives: vec![SkillAlternative {
                base_duration: 30000,
                base_cooldown: DEFAULT_COOLDOWN,
                condition: "remain_distance<=800&phase>=2".to_string(),
                effects: vec![SkillEffect {
                    skill_type: SkillType::TargetSpeed,
                    target: SkillTarget::Oneself,
                    modifier: 3500,
                }],
                precondition: String::new(),
            }],
            popularity: None,
            wisdom_check: false,
        },
    );

    let mut solver = RaceSolver::new(course, race_params(0), &Configuration::default());
    let uma = Uma {
        speed: 1200,
        stamina: 1200,
        power: 1000,
        guts: 800,
        wiz: 1000,
        style: Style::Pace,
        distance_aptitude: Aptitude::A,
        track_aptitude: Aptitude::A,
        style_aptitude: Aptitude::A,
    };
    solver.add_runner(uma, Motivation::Normal, vec![skill]);
    let mut solver = solver.ready_runners();
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
    let course = Course::from_json_file("data/course_data.json", "10101")
        .expect("Failed to load course data");
    let config = Configuration {
        game_version,
        ..Configuration::default()
    };
    let mut solver = RaceSolver::new(course, race_params(0), &config);
    for &(style, _) in runners {
        let uma = Uma {
            speed: 1200,
            stamina: 1000,
            power: 1000,
            guts: 800,
            wiz: 1000,
            style,
            distance_aptitude: Aptitude::A,
            track_aptitude: Aptitude::A,
            style_aptitude: Aptitude::A,
        };
        solver.add_runner(uma, Motivation::Normal, vec![]);
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

const FRAME: f64 = 1.0 / 15.0;

/// Solver with a single runner whose start delay is overridden.
fn start_delay_solver(start_delay: f64, rng_seed: u64) -> RaceSolver<Runner> {
    let course = Course::from_json_file("data/course_data.json", "10101")
        .expect("Failed to load course data");
    let mut solver = RaceSolver::new(course, race_params(rng_seed), &Configuration::default());
    let uma = Uma {
        speed: 1200,
        stamina: 1000,
        power: 1000,
        guts: 800,
        wiz: 1000,
        style: Style::Pace,
        distance_aptitude: Aptitude::A,
        track_aptitude: Aptitude::A,
        style_aptitude: Aptitude::A,
    };
    solver.add_runner(uma, Motivation::Normal, vec![]);
    let mut solver = solver.ready_runners();
    solver.runners[0].start_delay = start_delay;
    solver
}

/// `(position, speed, hp)` of the only runner.
fn runner_state(solver: &RaceSolver<Runner>) -> (f64, f64, f64) {
    let runner = &solver.runners[0];
    (runner.position, runner.speed, runner.hp.current)
}

fn assert_close(actual: f64, expected: f64, context: &str) {
    assert!(
        (actual - expected).abs() < 1e-9,
        "{context}: expected {expected}, got {actual}"
    );
}

#[test]
fn test_no_start_delay_moves_full_frame() {
    let mut solver = start_delay_solver(0.0, 0);
    solver.step(FRAME);
    let (position, speed, _) = runner_state(&solver);
    assert_close(position, speed * FRAME, "position");
}

#[test]
fn test_start_delay_within_first_frame_prorates_distance_only() {
    let mut delayed = start_delay_solver(0.03, 0);
    let mut reference = start_delay_solver(0.0, 0);
    delayed.step(FRAME);
    reference.step(FRAME);

    let (position, speed, hp) = runner_state(&delayed);
    let (_, ref_speed, ref_hp) = runner_state(&reference);
    // Acceleration and HP consumption are unaffected.
    assert_close(speed, ref_speed, "speed");
    assert_close(hp, ref_hp, "hp");
    assert_close(position, speed * (FRAME - 0.03), "position");
}

#[test]
fn test_start_delay_longer_than_a_frame_skips_whole_frames() {
    let mut delayed = start_delay_solver(0.09, 0);
    let (start_position, start_speed, start_hp) = runner_state(&delayed);

    // First frame is entirely within the start delay: nothing happens.
    delayed.step(FRAME);
    assert_eq!(
        runner_state(&delayed),
        (start_position, start_speed, start_hp)
    );

    // Second frame: full acceleration and HP consumption, prorated distance.
    delayed.step(FRAME);
    let mut reference = start_delay_solver(0.0, 0);
    reference.step(FRAME);
    let (position, speed, hp) = runner_state(&delayed);
    let (_, ref_speed, ref_hp) = runner_state(&reference);
    assert_close(speed, ref_speed, "speed");
    assert_close(hp, ref_hp, "hp");
    assert_close(position, speed * (2.0 * FRAME - 0.09), "position");
    assert!(position > 0.0, "Runner must move forward, got {position}");
}

#[test]
fn test_longer_start_delay_loses_a_frame_of_acceleration() {
    // Both leave the gate on the second frame, but only the one that leaves on the
    // first frame accelerates on it.
    let mut early = start_delay_solver(0.05, 0);
    let mut late = start_delay_solver(0.09, 0);
    for _ in 0..2 {
        early.step(FRAME);
        late.step(FRAME);
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
                solver.step(FRAME);
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
    let course = Course::from_json_file("data/course_data.json", "10204")
        .expect("Failed to load course data");
    let mut solver = RaceSolver::new(course, race_params(0), &Configuration::default());
    for _ in 0..9 {
        // Low wisdom for a high rush chance.
        let uma = Uma {
            speed: 1200,
            stamina: 1000,
            power: 1000,
            guts: 800,
            wiz: 100,
            style: Style::Late,
            distance_aptitude: Aptitude::A,
            track_aptitude: Aptitude::A,
            style_aptitude: Aptitude::A,
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
    let course = Course::from_json_file("data/course_data.json", "10101")
        .expect("Failed to load course data");
    let mut solver = RaceSolver::new(course, race_params(0), &Configuration::default());
    for (i, style) in [Style::Front, Style::Pace, Style::Late, Style::End]
        .into_iter()
        .enumerate()
    {
        let uma = Uma {
            speed: 1000 + 50 * i as u16,
            stamina: 1000,
            power: 1000,
            guts: 800,
            wiz: 1000,
            style,
            distance_aptitude: Aptitude::A,
            track_aptitude: Aptitude::A,
            style_aptitude: Aptitude::A,
        };
        solver.add_runner(uma, Motivation::Normal, vec![]);
    }
    let mut solver = solver.ready_runners();

    // Finish times are fixed once set, even though runners keep moving.
    let mut first_finish = None;
    while !solver.runners.iter().all(Runner::has_finished) {
        solver.step(1.0 / 15.0);
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
        assert!((margin - expected).abs() < 1e-9);
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
    // 1.8m apart laterally is within 0.165 course widths (~1.86m).
    let mut solver = struggle_solver(
        &[(Style::Front, 200.0, 1.0), (Style::Front, 197.0, 2.8)],
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
        &[(Style::Front, 200.0, 1.0), (Style::Front, 197.0, 2.9)],
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
            (Style::Runaway, 150.0, 5.0),
            (Style::Front, 137.0, 0.0),
            (Style::Front, 135.0, 0.0),
        ],
        GameVersion::LATEST,
    );
    update_struggle(&mut solver);
    assert_eq!(strugglers(&solver), [false, true, true]);

    let mut solver = struggle_solver(
        &[
            (Style::Runaway, 149.0, 5.0),
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
            (Style::Pace, 210.0, 5.0),
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
            (Style::Runaway, 210.0, 5.0),
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
            (Style::Front, 210.0, 5.0),
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
            (Style::Front, 197.0, 1.0),
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

    // 0.416 course widths = 4.68m.
    place(&mut solver, 1, 198.0, 4.7);
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

#[test]
fn test_force_in_applies_only_away_from_inner_fence() {
    let mut solver = positioning_solver(&[(Style::End, 100.0)], GameVersion::LATEST);
    let RaceSolver { runners, state, .. } = &mut solver;
    let runner = &mut runners[0];
    runner.phase = Phase::Early;
    let force_in = runner.force_in_modifier;
    assert!(force_in > 0.0);

    // 0.12 course widths = 1.35m.
    let target_speed_at = |runner: &mut Runner, lane: f64| {
        runner.lane.current = lane;
        runner.update_target_speed(state);
        runner.target_speed
    };
    let near_fence = target_speed_at(runner, 1.34);
    let away_from_fence = target_speed_at(runner, 1.36);
    assert!((away_from_fence - near_fence - force_in).abs() < 1e-9);
    assert_eq!(target_speed_at(runner, 0.0), near_fence);

    // Only in the early phase.
    runner.phase = Phase::Mid;
    let mid_near = target_speed_at(runner, 0.0);
    let mid_away = target_speed_at(runner, 5.0);
    assert_eq!(mid_near, mid_away);
}

#[test]
fn test_lane_type_in_course_widths() {
    let mut solver = positioning_solver(&[(Style::Front, 100.0)], GameVersion::LATEST);
    let runner = &mut solver.runners[0];
    // Thresholds are 0.2, 0.4 and 0.6 course widths (2.25m, 4.5m and 6.75m),
    // regardless of the course's total width.
    let cases = [
        (0.0, LaneType::Inner),
        (2.2, LaneType::Inner),
        (2.3, LaneType::Middle),
        (4.4, LaneType::Middle),
        (4.6, LaneType::Outer),
        (6.7, LaneType::Outer),
        (6.8, LaneType::Outside),
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
    runner.lane.current = 5.0;
    runner.skill_modifiers.target_lane = Some(0.5);
    for _ in 0..2000 {
        runner.update_lane(&state.course, 1.0 / 15.0);
    }
    assert!((runner.lane.current - 5.625).abs() < 1e-9);
}

#[test]
fn test_normal_mode_lane_targets_in_course_widths() {
    let mut solver = positioning_solver(&[(Style::Front, 100.0)], GameVersion::LATEST);
    let target = |solver: &RaceSolver<Runner>| {
        solver.runners[0]
            .normal_mode_lane_decision(&solver.state.course, &solver.runners)
            .target
    };

    // Move inward by 0.05 course widths (0.5625m) in the early phase.
    let runner = &mut solver.runners[0];
    runner.phase = Phase::Early;
    runner.lane.current = 5.0;
    assert!((target(&solver) - 4.4375).abs() < 1e-9);

    // Pace down targets 0.18 course widths (2.025m) from the inner fence.
    solver.runners[0].positioning_strategy = PositioningStrategy::PaceDown(0.0);
    assert!((target(&solver) - 2.025).abs() < 1e-9);
}
