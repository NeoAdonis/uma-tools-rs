use super::*;
use crate::configuration::{Configuration, GameVersion};
use crate::race::last_spurt::*;
use crate::race::runner::{MAX_TARGET_SPEED, Runner};
use crate::race::solver::{DELTA_TIME, RaceSolver};
use crate::skill::*;
use crate::test_utils::{alternative, assert_close, ready_solver, seeded_config, skill_data, uma};
use crate::trigger::skill_activation::TriggerCondition;
use crate::uma::*;

/// Same as skill 204222, but without wisdom check so it is always available.
fn full_throttle_skill() -> Skill {
    let alternative = SkillAlternative {
        base_duration: 20000,
        ..alternative(
            "run_at_full_speed_random==1&distance_type==3",
            SkillType::AccelerationFullThrottle,
            0.2,
        )
    };
    Skill::new("204222".to_string(), skill_data(vec![alternative]))
}

/// Build a solver with a single runner on a 2000m course whose threshold stat
/// is speed. Raw speed 2500 gives an adjusted speed of 1850 * 1.2 = 2220.
fn solver(
    speed: u16,
    game_version: GameVersion,
    skills: Vec<Skill>,
    rng_seed: u64,
) -> RaceSolver<Runner> {
    let uma = Uma {
        speed,
        stamina: 1600,
        power: 1200,
        ..uma(Style::Late)
    };
    let config = Configuration {
        game_version,
        ..seeded_config(rng_seed)
    };
    ready_solver(10204, uma, skills, &config)
}

/// Put the runner in her last spurt (20 m/s) with the given current speed.
fn enter_last_spurt(solver: &mut RaceSolver<Runner>, speed: f64) {
    let runner = &mut solver.runners[0];
    runner.position = 1500.0;
    runner.speed = speed;
    runner.last_spurt = Some(LastSpurt {
        max_speed: 20.0,
        target_distance: 1400.0,
        category: LastSpurtCategory::Full,
    });
    runner.update_target_speed(&solver.state);
}

#[test]
fn test_base_acceleration() {
    let latest = GameVersion::LATEST;
    assert_eq!(base_acceleration(2000, 1200, 2000, latest), 0.0);
    assert_eq!(
        base_acceleration(2200, 1200, 2000, GameVersion::MechaUmaUpdate),
        0.0
    );
    // 0.068 * 0.0006 * sqrt(500 * 1200) / 2^1.5.
    let expected = 0.068 * 0.0006 * 600000f64.sqrt() / 8f64.sqrt();
    assert_close!(base_acceleration(2001, 1200, 2000, latest), expected);
    assert_close!(expected, 0.011173, tolerance = 1e-6);
    // Longer courses accelerate less.
    assert!(base_acceleration(2001, 1200, 3000, latest) < expected);
}

#[test]
fn test_activates_at_last_spurt_speed() {
    let mut solver = solver(2500, GameVersion::LATEST, vec![], 42);
    assert_eq!(solver.runners[0].adjusted.speed, 2220);
    assert!(solver.runners[0].full_throttle_base > 0.0);

    enter_last_spurt(&mut solver, 19.9);
    assert_eq!(solver.runners[0].full_throttle, FullThrottle::Pending);

    enter_last_spurt(&mut solver, 20.0);
    assert_eq!(
        solver.runners[0].full_throttle,
        FullThrottle::Active { speed_bonus: 0.0 }
    );
}

#[test]
fn test_ineligible_runner_never_activates() {
    for (speed, behavior) in [
        // Raw 2000 is adjusted to 1600 * 1.2 = 1920.
        (2000, GameVersion::LATEST),
        (2500, GameVersion::MechaUmaUpdate),
    ] {
        let mut solver = solver(speed, behavior, vec![], 42);
        assert_eq!(solver.runners[0].full_throttle_base, 0.0);
        enter_last_spurt(&mut solver, 20.0);
        assert_eq!(solver.runners[0].full_throttle, FullThrottle::Pending);
    }
}

#[test]
fn test_speed_bonus_increases_target_speed() {
    let mut solver = solver(2500, GameVersion::LATEST, vec![], 42);
    enter_last_spurt(&mut solver, 20.0);
    let target_before = solver.runners[0].target_speed;

    let runner = &mut solver.runners[0];
    runner.skill_modifiers.full_throttle_acceleration = 0.4;
    runner.update_speed(DELTA_TIME, solver.state.course.base_speed());

    let expected = (runner.full_throttle_base + 0.04) * DELTA_TIME;
    assert_close!(runner.full_throttle.speed_bonus(), expected);

    runner.update_target_speed(&solver.state);
    assert_close!(runner.target_speed - target_before, expected);
}

#[test]
fn test_hp_out_drops_speed_to_minimum() {
    let mut solver = solver(2500, GameVersion::LATEST, vec![], 42);
    enter_last_spurt(&mut solver, 20.0);

    let runner = &mut solver.runners[0];
    runner.speed = 24.0;
    runner.hp.current = 0.001;
    runner.update_hp(DELTA_TIME, &solver.state);
    assert_eq!(runner.speed, runner.min_speed);
    assert_eq!(runner.full_throttle, FullThrottle::Ended);

    // Can't be activated again, even if HP is recovered.
    runner.hp.recover(100.0);
    runner.speed = 20.0;
    runner.update_target_speed(&solver.state);
    assert_eq!(runner.full_throttle, FullThrottle::Ended);
}

#[test]
fn test_speed_is_capped() {
    let mut solver = solver(2500, GameVersion::LATEST, vec![], 42);
    let runner = &mut solver.runners[0];
    runner.is_start_dash = false;
    runner.speed = MAX_TARGET_SPEED - 0.01;
    runner.acceleration = 1.0;
    runner.update_speed(DELTA_TIME, solver.state.course.base_speed());
    assert_eq!(runner.speed, MAX_TARGET_SPEED);
}

#[test]
fn test_random_trigger_scheduled_on_activation() {
    let mut solver = solver(2500, GameVersion::LATEST, vec![full_throttle_skill()], 42);
    {
        let scheduler = &solver.runners[0].trigger_scheduler;
        assert_eq!(scheduler.full_throttle_triggers, vec![0]);
        let logic = &scheduler.skill_logics[0];
        assert!(matches!(logic.trigger, TriggerCondition::FullThrottleStart));
        // `distance_type==3` is resolved when the runner is created.
        assert!(logic.guards.is_empty());
        assert!(scheduler.position_windows.is_empty());
    }

    enter_last_spurt(&mut solver, 20.0);

    let scheduler = &solver.runners[0].trigger_scheduler;
    assert!(scheduler.full_throttle_triggers.is_empty());
    // A single 10m segment between the current position and the goal.
    let windows: Vec<_> = scheduler.position_windows.iter().collect();
    assert_eq!(windows.len(), 1);
    assert_eq!(windows[0].logic_idx, 0);
    assert!((1500.0..2000.0).contains(&windows[0].min_threshold));
    assert_eq!(windows[0].max_threshold, windows[0].min_threshold + 10.0);
}

#[test]
fn test_full_race() {
    let mut activated_any = false;
    for seed in 0..5 {
        let mut with = solver(2500, GameVersion::LATEST, vec![full_throttle_skill()], seed);
        with.run();
        let runner = &with.runners[0];
        assert_ne!(runner.full_throttle, FullThrottle::Pending, "seed {seed}");

        // The skill activates after the last spurt begins.
        let target_distance = runner.last_spurt.as_ref().unwrap().target_distance;
        for record in &runner.activated_skills {
            assert!(record.distance >= target_distance, "seed {seed}");
        }
        activated_any |= !runner.activated_skills.is_empty();

        // Without Full-Throttle Sprint (and skills), the runner is slower.
        // Nothing else differs between these game behaviors for this runner.
        let mut with = solver(2500, GameVersion::LATEST, vec![], seed);
        let mut without = solver(2500, GameVersion::MechaUmaUpdate, vec![], seed);
        with.run();
        without.run();
        let with_time = with.runners[0].finish_time.unwrap();
        let without_time = without.runners[0].finish_time.unwrap();
        assert!(with_time < without_time, "seed {seed}");
    }
    assert!(activated_any);
}
