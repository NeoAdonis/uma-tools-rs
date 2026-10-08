use super::*;
use crate::configuration::{Configuration, GameVersion};
use crate::race::solver::RaceSolver;
use crate::skill::{SkillEffect, SkillTarget, SkillType};
use crate::test_utils::{assert_close, ready_solver, seeded_config, uma};
use crate::uma::*;

/// Build a solver with a single runner on a 2000m course.
fn solver(wiz: u16, game_version: GameVersion, rng_seed: u64) -> RaceSolver<Runner> {
    let config = Configuration {
        game_version,
        ..seeded_config(rng_seed)
    };
    ready_solver(
        10204,
        Uma {
            wiz,
            ..uma(Style::Late)
        },
        vec![],
        &config,
    )
}

/// Move the runner just past the start of `section` and update her phase.
fn move_to_section(solver: &mut RaceSolver<Runner>, section: u8) {
    let position = solver.state.course.section_length() * section as f64 + 0.1;
    let runner = &mut solver.runners[0];
    runner.position = position;
    runner.update_phase(&mut solver.state);
}

/// Late-race base target speed of the solver's runner.
fn late_base_target_speed(solver: &RaceSolver<Runner>) -> f64 {
    let uma = &solver.runners[0].adjusted;
    base_target_speed(
        solver.state.course.base_speed(),
        uma.style,
        Phase::Late,
        uma.speed,
        uma.distance_aptitude,
    )
}

/// Set HP halfway between what a last spurt needs at late-race base target
/// speed and at max speed, so that the result is `AboveBase`.
fn set_hp_above_base(solver: &mut RaceSolver<Runner>) {
    let base_speed = late_base_target_speed(solver);
    let runner = &solver.runners[0];
    let at_base = spurt_hp_required(runner, &solver.state, base_speed);
    let at_max = spurt_hp_required(
        runner,
        &solver.state,
        last_spurt_speed_max(runner, &solver.state),
    );
    solver.runners[0].hp.current = (at_base + at_max) / 2.0;
}

/// Placeholder last spurt to detect whether it has been recalculated.
fn placeholder() -> LastSpurt {
    LastSpurt {
        max_speed: 0.0,
        target_distance: 0.0,
        category: LastSpurtCategory::CannotFinish,
    }
}

fn recovery(modifier: f64) -> SkillEffect {
    SkillEffect {
        skill_type: SkillType::Recovery,
        target: SkillTarget::Oneself,
        modifier,
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
                for stat in [1, 100, 600, 1200, 2000] {
                    let runner = &mut solver.runners[0];
                    runner.adjusted.style = style;
                    runner.adjusted.distance_aptitude = distance_aptitude;
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
fn test_calculated_on_entering_late_phase() {
    let mut solver = solver(1000, GameVersion::LATEST, 0);

    move_to_section(&mut solver, 4);
    assert_eq!(solver.runners[0].phase, Phase::Mid);
    assert!(solver.runners[0].last_spurt.is_none());

    move_to_section(&mut solver, 16);
    assert_eq!(solver.runners[0].phase, Phase::Late);
    let last_spurt = solver.runners[0].last_spurt.as_ref().unwrap();
    // Full HP is enough to spurt at max speed right away. The runner is slightly
    // past the start of the Late phase, so the spurt starts at her position.
    assert_eq!(last_spurt.category, LastSpurtCategory::Full);
    let position = solver.runners[0].position;
    assert_close!(last_spurt.target_distance, position);
}

#[test]
fn test_uses_hp_at_late_phase_entry() {
    let mut solver = solver(1000, GameVersion::LATEST, 0);
    move_to_section(&mut solver, 15);
    set_hp_above_base(&mut solver);
    move_to_section(&mut solver, 16);
    let last_spurt = solver.runners[0].last_spurt.as_ref().unwrap();
    assert_eq!(last_spurt.category, LastSpurtCategory::AboveBase);

    let mut low_hp = self::solver(1000, GameVersion::LATEST, 0);
    move_to_section(&mut low_hp, 15);
    low_hp.runners[0].hp.current = 1.0;
    move_to_section(&mut low_hp, 16);
    let last_spurt = low_hp.runners[0].last_spurt.as_ref().unwrap();
    assert_eq!(last_spurt.category, LastSpurtCategory::CannotFinish);
}

#[test]
fn test_candidate_speeds_are_steps_from_max() {
    for rng_seed in 0..20 {
        let mut solver = solver(300, GameVersion::LATEST, rng_seed);
        move_to_section(&mut solver, 16);
        set_hp_above_base(&mut solver);
        let max = last_spurt_speed_max(&solver.runners[0], &solver.state);
        let base = late_base_target_speed(&solver);
        let last_spurt = calculate_last_spurt(&mut solver.runners[0], &solver.state);

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
fn test_high_wiz_always_accepts_best_candidate() {
    // 15 + 0.05 * 2000 = 115% chance, which must not panic.
    let results: Vec<_> = (0..10)
        .map(|rng_seed| {
            let mut solver = solver(2000, GameVersion::LATEST, rng_seed);
            move_to_section(&mut solver, 16);
            set_hp_above_base(&mut solver);
            let last_spurt = calculate_last_spurt(&mut solver.runners[0], &solver.state);
            assert_eq!(last_spurt.category, LastSpurtCategory::AboveBase);
            (last_spurt.max_speed, last_spurt.target_distance)
        })
        .collect();
    assert!(results.iter().all(|result| *result == results[0]));
}

#[test]
fn test_recovery_recalculates_only_from_late_phase() {
    let mut solver = solver(1000, GameVersion::LATEST, 0);
    move_to_section(&mut solver, 4);
    let runner = &mut solver.runners[0];
    runner.apply_skill_effect(&recovery(0.05), 0.0, 0.0, &solver.state);
    assert!(runner.last_spurt.is_none());

    move_to_section(&mut solver, 17);
    let runner = &mut solver.runners[0];
    runner.last_spurt = Some(placeholder());
    runner.apply_skill_effect(&recovery(0.05), 0.0, 0.0, &solver.state);
    assert!(runner.last_spurt.as_ref().unwrap().max_speed > 0.0);
}

#[test]
fn test_recovery_does_not_recalculate_before_first_anniversary() {
    let mut solver = solver(1000, GameVersion::Original, 0);
    move_to_section(&mut solver, 17);
    let runner = &mut solver.runners[0];
    runner.last_spurt = Some(placeholder());
    runner.apply_skill_effect(&recovery(0.05), 0.0, 0.0, &solver.state);
    assert_eq!(runner.last_spurt.as_ref().unwrap().max_speed, 0.0);
}

#[test]
fn test_hp_loss_is_not_a_heal() {
    let mut solver = solver(1000, GameVersion::LATEST, 0);
    move_to_section(&mut solver, 17);
    let runner = &mut solver.runners[0];
    runner.last_spurt = Some(placeholder());
    runner.is_stamina_keep = true;
    let hp_before = runner.hp.current;

    // 2% of maximum HP.
    runner.apply_skill_effect(&recovery(-0.02), 0.0, 0.0, &solver.state);
    assert_close!(runner.hp.current, hp_before - runner.hp.maximum * 0.02);
    assert_eq!(runner.last_spurt.as_ref().unwrap().max_speed, 0.0);
    assert_eq!(runner.trigger_scheduler.activation_counts.heal, 0);
    assert!(runner.is_stamina_keep);
}
