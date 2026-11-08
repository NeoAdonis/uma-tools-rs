use super::*;
use crate::configuration::{Configuration, GameVersion};
use crate::race::solver::RaceSolver;
use crate::skill::SkillType;
use crate::test_utils::{assert_close, effect, ready_solver, seeded_config, uma};
use crate::trigger::skill_activation::OvertakeExtension;
use crate::uma::*;

/// Build a solver with a single runner on a 2000m course.
fn solver(wit: u16, game_version: GameVersion, rng_seed: u64) -> RaceSolver<Runner> {
    let config = Configuration {
        game_version,
        ..seeded_config(rng_seed)
    };
    ready_solver(
        10204,
        Uma {
            wit,
            ..uma(Style::Late)
        },
        vec![],
        config,
    )
}

/// Move the runner just past the start of `section` and update her phase.
fn move_to_section(solver: &mut RaceSolver<Runner>, section: u8) {
    let position = solver.state.course.section_length() * section as f64 + 0.1;
    let runner = &mut solver.runners[0];
    runner.position = position;
    runner.update_phase(runner.position, &mut solver.state);
}

/// Move the runner 1m forward and update her phase from where she was.
fn next_frame(solver: &mut RaceSolver<Runner>) {
    let runner = &mut solver.runners[0];
    let position = runner.position;
    runner.position += 1.0;
    runner.update_phase(position, &mut solver.state);
}

/// Calculate the last spurt of the solver's runner from her position.
fn calculate(solver: &mut RaceSolver<Runner>) -> LastSpurt {
    let runner = &mut solver.runners[0];
    runner.calculate_last_spurt(runner.position, &solver.state)
}

/// Late-race base target speed of the solver's runner.
fn late_base_target_speed(solver: &RaceSolver<Runner>) -> f64 {
    let runner = &solver.runners[0];
    base_target_speed(
        solver.state.course.base_speed(),
        runner.style,
        Phase::Late,
        runner.adjusted.speed,
        runner.raw.distance_aptitude,
    )
}

/// Set HP at `share` of the way from what a last spurt needs at late-race base
/// target speed to what it needs at max speed, so that the result is
/// `AboveBase` (with `share` in (0, 1)).
fn set_hp_above_base(solver: &mut RaceSolver<Runner>, share: f64) {
    let base_speed = late_base_target_speed(solver);
    let runner = &solver.runners[0];
    let at_base = spurt_hp_required(runner, &solver.state, base_speed, runner.position);
    let at_max = spurt_hp_required(
        runner,
        &solver.state,
        last_spurt_speed_max(runner, &solver.state),
        runner.position,
    );
    solver.runners[0]
        .hp
        .set_current(at_base + (at_max - at_base) * share);
}

/// Placeholder last spurt to detect whether it has been recalculated.
fn placeholder() -> LastSpurt {
    LastSpurt {
        max_speed: 0.0,
        target_distance: 0.0,
        category: LastSpurtCategory::CannotFinish,
    }
}

#[test]
fn test_speed_max_above_late_base_target_speed() {
    // Guarantees that the candidate list is never empty.
    let mut solver = solver(1000, GameVersion::Original, 0);
    let aptitudes = [Aptitude::S, Aptitude::A, Aptitude::D, Aptitude::G];
    let styles = [
        Style::Runaway,
        Style::Front,
        Style::Pace,
        Style::Late,
        Style::End,
    ];
    for game_version in [GameVersion::Original, GameVersion::LATEST] {
        solver.state.config.game_version = game_version;
        for style in styles {
            for distance_aptitude in aptitudes {
                for stat in [1.0, 100.0, 600.0, 1200.0, 2000.0] {
                    let runner = &mut solver.runners[0];
                    runner.style = style;
                    runner.raw.distance_aptitude = distance_aptitude;
                    runner.adjusted.speed = stat;
                    runner.adjusted.guts = stat;
                    let max = last_spurt_speed_max(&solver.runners[0], &solver.state);
                    let base = late_base_target_speed(&solver);
                    assert!(
                        max > base,
                        "{game_version:?} {style:?} {distance_aptitude:?} {stat}: {max} <= {base}"
                    );
                }
            }
        }
    }
}

#[test]
fn test_speed_max() {
    // On a 2000m course (20 m/s base speed), with an End Closer (1.0 Late phase
    // multiplier). Guts only adds speed from the 1st Anniversary onward.
    // (version, distance aptitude, speed, guts, max speed).
    let cases = [
        (GameVersion::Original, Aptitude::A, 1200.0, 800.0, 24.385846),
        (
            GameVersion::FirstAnniversary,
            Aptitude::B,
            1200.0,
            800.0,
            24.275807,
        ),
    ];
    let mut solver = solver(1000, GameVersion::Original, 0);
    for (game_version, distance_aptitude, speed, guts, expected) in cases {
        solver.state.config.game_version = game_version;
        let runner = &mut solver.runners[0];
        runner.style = Style::End;
        runner.raw.distance_aptitude = distance_aptitude;
        runner.adjusted.speed = speed;
        runner.adjusted.guts = guts;
        let max = last_spurt_speed_max(&solver.runners[0], &solver.state);
        assert_close!(max, expected, tolerance = 1e-6, "{game_version:?}");
    }
}

#[test]
fn test_calculated_a_frame_after_entering_late_phase() {
    let mut solver = solver(1000, GameVersion::LATEST, 0);

    move_to_section(&mut solver, 4);
    assert_eq!(solver.runners[0].phase, Phase::Mid);
    assert!(solver.runners[0].last_spurt.is_none());

    move_to_section(&mut solver, 16);
    assert_eq!(solver.runners[0].phase, Phase::Late);
    assert!(solver.runners[0].last_spurt.is_none());

    let position = solver.runners[0].position;
    next_frame(&mut solver);
    let last_spurt = solver.runners[0].last_spurt.as_ref().unwrap();
    // Full HP is enough to spurt at max speed right away. The runner is slightly
    // past the start of the Late phase, so the spurt starts at her position
    // before the move.
    assert_eq!(last_spurt.category, LastSpurtCategory::Full);
    assert_eq!(last_spurt.target_distance, position);
}

#[test]
fn test_uses_hp_at_decision() {
    let mut solver = solver(1000, GameVersion::LATEST, 0);
    move_to_section(&mut solver, 16);
    set_hp_above_base(&mut solver, 0.5);
    next_frame(&mut solver);
    let last_spurt = solver.runners[0].last_spurt.as_ref().unwrap();
    assert_eq!(last_spurt.category, LastSpurtCategory::AboveBase);

    let mut low_hp = self::solver(1000, GameVersion::LATEST, 0);
    move_to_section(&mut low_hp, 16);
    low_hp.runners[0].hp.set_current(1.0);
    next_frame(&mut low_hp);
    let last_spurt = low_hp.runners[0].last_spurt.as_ref().unwrap();
    assert_eq!(last_spurt.category, LastSpurtCategory::CannotFinish);
    // Only the final 60m.
    let goal = low_hp.state.course.length();
    assert_close!(last_spurt.target_distance, goal - 60.0);
}

#[test]
fn test_candidate_speeds_are_steps_from_max() {
    for rng_seed in 0..20 {
        let mut solver = solver(300, GameVersion::LATEST, rng_seed);
        move_to_section(&mut solver, 16);
        set_hp_above_base(&mut solver, 0.5);
        let max = last_spurt_speed_max(&solver.runners[0], &solver.state);
        let base = late_base_target_speed(&solver);
        let last_spurt = calculate(&mut solver);

        assert_eq!(last_spurt.category, LastSpurtCategory::AboveBase);
        assert!(last_spurt.max_speed >= base && last_spurt.max_speed <= max);
        let steps = (max - last_spurt.max_speed) / 0.1;
        assert_close!(
            steps,
            steps.round(),
            tolerance = 1e-6,
            "seed {rng_seed}: {} is not a 0.1 step from {max}",
            last_spurt.max_speed
        );
    }
}

#[test]
fn test_high_wit_always_accepts_best_candidate() {
    // Raw 2400 wit is 1800 after halving above 1200, so 15 + 0.05 * 1800 =
    // 105% chance, which must not panic.
    let results: Vec<_> = (0..10)
        .map(|rng_seed| {
            let mut solver = solver(2400, GameVersion::LATEST, rng_seed);
            move_to_section(&mut solver, 16);
            set_hp_above_base(&mut solver, 0.5);
            let last_spurt = calculate(&mut solver);
            assert_eq!(last_spurt.category, LastSpurtCategory::AboveBase);
            (last_spurt.max_speed, last_spurt.target_distance)
        })
        .collect();
    assert!(results.iter().all(|result| *result == results[0]));
}

#[test]
fn test_best_candidate_uses_up_hp() {
    // With high wit, the best candidate is always accepted. With more HP, a
    // longer spurt at a lower speed can finish first.
    // (HP share from base to max speed, whether the best is the max speed).
    for (share, is_max) in [(0.5, true), (0.75, false)] {
        let mut solver = solver(2400, GameVersion::LATEST, 0);
        move_to_section(&mut solver, 16);
        set_hp_above_base(&mut solver, share);
        let max = last_spurt_speed_max(&solver.runners[0], &solver.state);
        let last_spurt = calculate(&mut solver);
        assert_eq!(last_spurt.category, LastSpurtCategory::AboveBase);
        assert_eq!(last_spurt.max_speed == max, is_max, "{share} HP share");

        // The spurt uses up the HP, running the rest at late-race base target
        // speed (both until 60m before the goal line).
        let course = &solver.state.course;
        let spurt_distance = course.length() - 60.0 - last_spurt.target_distance;
        let remaining = remaining_spurt_distance(solver.runners[0].position, course) - 60.0;
        let base_distance = remaining - spurt_distance;
        // Otherwise, the distance would be clamped, leaving HP unused.
        assert!(base_distance > 0.0, "{share} HP share: whole distance");
        let hp = |speed, distance| {
            let condition = solver.state.params.track_condition;
            let guts = solver.runners[0].adjusted.guts;
            estimate_hp_last_spurt(speed, course, condition, distance, guts)
        };
        let base_speed = late_base_target_speed(&solver);
        assert_close!(
            hp(base_speed, base_distance) + hp(last_spurt.max_speed, spurt_distance),
            solver.runners[0].hp.current(),
            "{share} HP share"
        );
    }
}

#[test]
fn test_recovery_recalculates_from_late_phase() {
    // (version, section, whether the last spurt is recalculated).
    let cases = [
        (GameVersion::LATEST, 4, false),
        (GameVersion::LATEST, 17, true),
        (GameVersion::Original, 17, false),
    ];
    for (version, section, recalculates) in cases {
        let mut solver = solver(1000, version, 0);
        move_to_section(&mut solver, section);
        let runner = &mut solver.runners[0];
        runner.last_spurt = Some(placeholder());
        runner.apply_skill_effect(
            &effect(SkillType::Recovery, 0.05),
            0.0,
            OvertakeExtension::default(),
            0.0,
            None,
            &solver.state,
        );
        // Counted for `activate_count_heal`.
        assert_eq!(runner.trigger_scheduler.activation_counts.heal, 1);
        let max_speed = runner.last_spurt.unwrap().max_speed;
        assert_eq!(
            max_speed > 0.0,
            recalculates,
            "{version:?}, section {section}"
        );
    }
}

#[test]
fn test_hp_loss_is_not_a_heal() {
    let mut solver = solver(1000, GameVersion::LATEST, 0);
    move_to_section(&mut solver, 17);
    let runner = &mut solver.runners[0];
    runner.last_spurt = Some(placeholder());
    runner.stamina_keep.is_active = true;
    let hp_before = runner.hp.current();

    // 2% of maximum HP.
    runner.apply_skill_effect(
        &effect(SkillType::Recovery, -0.02),
        0.0,
        OvertakeExtension::default(),
        0.0,
        None,
        &solver.state,
    );
    assert_close!(runner.hp.current(), hp_before - runner.hp.maximum() * 0.02);
    assert_eq!(runner.last_spurt.as_ref().unwrap().max_speed, 0.0);
    assert_eq!(runner.trigger_scheduler.activation_counts.heal, 0);
    assert!(runner.stamina_keep.is_active);
}

#[test]
fn test_slowest_candidate_when_every_check_fails() {
    // Low acceptance chance (17.5%), so that every check fails for some seed.
    for rng_seed in 0..1000 {
        let mut solver = solver(50, GameVersion::LATEST, rng_seed);
        move_to_section(&mut solver, 16);
        set_hp_above_base(&mut solver, 0.5);

        // Replay the checks, one per candidate, with a copy of the RNG.
        let runner = &solver.runners[0];
        let max = last_spurt_speed_max(runner, &solver.state);
        let base = late_base_target_speed(&solver);
        let candidates = (0..)
            .take_while(|&step| max - 0.1 * step as f64 >= base)
            .count();
        let chance = 0.15 + 0.0005 * runner.adjusted.wit;
        let mut rng = runner.rng.last_spurt.clone();
        if (0..candidates).any(|_| rng.random_bool(chance)) {
            continue;
        }

        // The slowest candidate takes the longest, so it's the last one.
        let last_spurt = calculate(&mut solver);
        assert_eq!(last_spurt.category, LastSpurtCategory::AboveBase);
        let slowest = max - 0.1 * (candidates - 1) as f64;
        assert_close!(last_spurt.max_speed, slowest, "seed {rng_seed}");
        // Not enough HP to spurt any later at that speed.
        let position = solver.runners[0].position;
        assert_close!(last_spurt.target_distance, position, "seed {rng_seed}");
        return;
    }
    panic!("every check should fail for some seed");
}

#[test]
fn test_hp_required_to_finish() {
    let mut solver = solver(1000, GameVersion::LATEST, 0);
    move_to_section(&mut solver, 16);

    // From the Late phase, it's the HP needed for a full last spurt.
    // (HP over the required HP, last spurt category).
    let required = solver.runners[0].hp_required_to_finish(&solver.state);
    let cases = [
        (0.0, LastSpurtCategory::Full),
        (-1.0, LastSpurtCategory::AboveBase),
    ];
    for (offset, category) in cases {
        let runner = &mut solver.runners[0];
        runner.hp.set_current(required + offset);
        let last_spurt = runner.calculate_last_spurt(runner.position, &solver.state);
        assert_eq!(last_spurt.category, category, "{offset} HP over");
    }

    // Before it, the rest of the Mid phase is run at its base target speed.
    let late_start = Phase::Late.boundaries(&solver.state.course).0;
    let runner = &mut solver.runners[0];
    runner.position = late_start;
    let at_late_start = runner.hp_required_to_finish(&solver.state);
    runner.position = late_start - 100.0;
    let before = runner.hp_required_to_finish(&solver.state);
    // 0.998 Mid phase multiplier for a Late Surger.
    let mid_speed = 0.998 * solver.state.course.base_speed();
    let hp_per_second = calculate_hp_per_second(
        mid_speed,
        &solver.state.course,
        solver.state.params.track_condition,
        Phase::Mid,
        runner.adjusted.guts,
        1.0,
    );
    assert_close!(before - at_late_start, hp_per_second * 100.0 / mid_speed);
}
