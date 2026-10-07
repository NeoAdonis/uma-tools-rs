use super::*;
use crate::configuration::{Configuration, GameVersion};
use crate::course::DistanceType;
use crate::race::last_spurt::*;
use crate::race::runner::{PositioningStrategy, Runner};
use crate::race::solver::RaceSolver;
use crate::test_utils::{load_course, race_params, seeded_config, uma};
use crate::uma::*;

const EPSILON: f64 = 1e-9;
const DELTA_TIME: f64 = 1.0 / 15.0;

/// Build a solver with a single runner on a 2000m (Medium) course.
fn solver(
    power: u16,
    style: Style,
    motivation: Motivation,
    game_version: GameVersion,
    rng_seed: u64,
) -> RaceSolver<Runner> {
    let uma = Uma {
        stamina: 1200,
        power,
        ..uma(style)
    };
    let config = Configuration {
        game_version,
        ..seeded_config(rng_seed)
    };
    let mut solver = RaceSolver::new(load_course(10204), race_params(), &config);
    solver.add_runner(uma, motivation, vec![]);
    solver.ready_runners()
}

/// Solver with an eligible Late Surger (raw power 1600, normal motivation).
fn eligible_solver() -> RaceSolver<Runner> {
    solver(
        1600,
        Style::Late,
        Motivation::Normal,
        GameVersion::LATEST,
        42,
    )
}

/// Run `update_charge_up` for `seconds`, starting at `start`. Returns end time.
fn conserve_for(runner: &mut Runner, start: f64, seconds: f64) -> f64 {
    let frames = (seconds / DELTA_TIME).round() as usize;
    let mut elapsed = start;
    for _ in 0..frames {
        elapsed += DELTA_TIME;
        runner.update_charge_up(DELTA_TIME, elapsed);
    }
    elapsed
}

fn conserved(runner: &Runner) -> f64 {
    runner.charge_up.as_ref().unwrap().conserved
}

/// Put the runner in her last spurt and let her release conserved power.
fn enter_last_spurt(solver: &mut RaceSolver<Runner>, conserved: f64) {
    let runner = &mut solver.runners[0];
    runner.charge_up.as_mut().unwrap().conserved = conserved;
    runner.position = 1500.0;
    runner.last_spurt = Some(LastSpurt {
        max_speed: 20.0,
        target_distance: 1400.0,
        category: LastSpurtCategory::Full,
    });
    solver.state.elapsed_time = 60.0;
    runner.update_phase(&mut solver.state);
}

#[test]
fn test_charge_power() {
    assert_eq!(charge_power(1200, 1200, 0), 1200.0);
    // Raw 1600: base 1400 + 0.5 * 400.
    assert_eq!(charge_power(1400, 1600, 0), 1600.0);
    assert_eq!(charge_power(1400, 1600, 60), 1660.0);
    // Below 1200 the raw term is zero.
    assert_eq!(charge_power(1000, 1000, 0), 1000.0);
}

#[test]
fn test_motivation_affects_base_part_only() {
    let solver = solver(
        1600,
        Style::Late,
        Motivation::Great,
        GameVersion::LATEST,
        42,
    );
    // Base (1200 + 200) * 1.04 = 1456, plus 0.5 * 400 without motivation.
    let charge_up = solver.runners[0].charge_up.as_ref().unwrap();
    assert!((charge_up.power - 1656.0).abs() < EPSILON);
}

#[test]
fn test_eligibility() {
    let is_eligible = |power, motivation, version| {
        solver(power, Style::Late, motivation, version, 42).runners[0]
            .charge_up
            .is_some()
    };
    assert!(is_eligible(
        1600,
        Motivation::Normal,
        GameVersion::SecondAnniversary
    ));
    assert!(is_eligible(1202, Motivation::Normal, GameVersion::LATEST));
    assert!(!is_eligible(1200, Motivation::Normal, GameVersion::LATEST));
    // 1200 * 0.96 = 1152, plus 0.5 * 20.
    assert!(!is_eligible(1220, Motivation::Awful, GameVersion::LATEST));
    assert!(!is_eligible(
        1600,
        Motivation::Normal,
        GameVersion::OnePointFiveAnniversary
    ));
}

#[test]
fn test_coefficients() {
    assert_eq!(style_coefficient(Style::Runaway), 1.0);
    assert_eq!(style_coefficient(Style::Front), 1.0);
    assert_eq!(style_coefficient(Style::Pace), 0.75);
    assert_eq!(style_coefficient(Style::Late), 1.0);
    assert_eq!(style_coefficient(Style::End), 0.7);

    use DistanceType::*;
    for distance_type in [Short, Mile, Medium, Long] {
        assert_eq!(
            strategy_distance_coefficient(Style::Front, distance_type),
            1.0
        );
        assert_eq!(
            strategy_distance_coefficient(Style::Runaway, distance_type),
            1.0
        );
    }
    let pace = [0.7, 0.8, 0.9, 0.9];
    let late = [0.75, 0.7, 0.875, 1.0];
    let end = [0.7, 0.75, 0.86, 0.9];
    for (i, distance_type) in [Short, Mile, Medium, Long].into_iter().enumerate() {
        assert_eq!(
            strategy_distance_coefficient(Style::Pace, distance_type),
            pace[i]
        );
        assert_eq!(
            strategy_distance_coefficient(Style::Late, distance_type),
            late[i]
        );
        assert_eq!(
            strategy_distance_coefficient(Style::End, distance_type),
            end[i]
        );
    }

    assert_eq!(distance_type_coefficient(Short), 0.45);
    assert_eq!(distance_type_coefficient(Mile), 1.0);
    assert_eq!(distance_type_coefficient(Medium), 0.875);
    assert_eq!(distance_type_coefficient(Long), 0.8);

    assert_eq!(activity_coefficient(false, false), 1.0);
    assert_eq!(activity_coefficient(true, false), 0.98);
    assert_eq!(activity_coefficient(false, true), 0.8);
    assert!((activity_coefficient(true, true) - 0.784).abs() < EPSILON);
}

#[test]
fn test_release_formulas() {
    // Expected: sqrt(400 * 130) * 0.001 * 0.875 * 0.8.
    let expected = 52000f64.sqrt() * 0.001 * 0.875 * 0.8;
    let acceleration = release_acceleration(1600.0, Style::Late, DistanceType::Medium, 0.8);
    assert!((acceleration - expected).abs() < EPSILON);
    assert!((acceleration - 0.159625).abs() < 1e-6);

    // 100 * sqrt(1600) * 1.0 / 1450.
    let duration = release_duration(100.0, 1600, DistanceType::Mile);
    assert!((duration - 4000.0 / 1450.0).abs() < EPSILON);
}

#[test]
fn test_conservation_rate() {
    let mut late_surger = eligible_solver();
    let runner = &mut late_surger.runners[0];
    conserve_for(runner, 0.0, 1.0);
    assert!((conserved(runner) - 4.2).abs() < 1e-6);

    runner.charge_up.as_mut().unwrap().conserved = 0.0;
    runner.positioning_strategy = PositioningStrategy::PaceDown(5.0);
    conserve_for(runner, 1.0, 1.0);
    assert!((conserved(runner) - 6.7).abs() < 1e-6);

    let mut end_closer = solver(
        1600,
        Style::End,
        Motivation::Normal,
        GameVersion::LATEST,
        42,
    );
    let runner = &mut end_closer.runners[0];
    conserve_for(runner, 0.0, 1.0);
    assert!((conserved(runner) - 4.2 * 0.7).abs() < 1e-6);
}

#[test]
fn test_blocked_and_cooldown() {
    let mut solver = eligible_solver();
    let runner = &mut solver.runners[0];
    runner.positioning_strategy = PositioningStrategy::PaceUp(5.0);
    let elapsed = conserve_for(runner, 0.0, 1.0);
    assert_eq!(conserved(runner), 0.0);

    // Cooldown lasts 1.5s after exiting.
    runner.positioning_strategy = PositioningStrategy::Normal;
    let elapsed = conserve_for(runner, elapsed, 1.4);
    assert_eq!(conserved(runner), 0.0);
    conserve_for(runner, elapsed, 1.0);
    assert!(conserved(runner) > 0.0);

    for strategy in [
        PositioningStrategy::FrontSpeedUp,
        PositioningStrategy::FrontOvertake,
        PositioningStrategy::PaceUpEx,
    ] {
        let mut other = eligible_solver();
        let runner = &mut other.runners[0];
        runner.positioning_strategy = strategy;
        conserve_for(runner, 0.0, 1.0);
        assert_eq!(conserved(runner), 0.0);
    }
}

#[test]
fn test_no_conservation_after_section_10() {
    let mut solver = eligible_solver();
    let runner = &mut solver.runners[0];
    runner.section = 10;
    conserve_for(runner, 0.0, 1.0);
    assert_eq!(conserved(runner), 0.0);
}

#[test]
fn test_spot_struggle_penalty() {
    let mut solver = solver(
        1600,
        Style::Front,
        Motivation::Normal,
        GameVersion::LATEST,
        42,
    );
    let runner = &mut solver.runners[0];
    runner.charge_up.as_mut().unwrap().conserved = 100.0;
    runner.start_spot_struggle(10.0);
    assert!((conserved(runner) - 95.0).abs() < EPSILON);
    // Already struggling: no further penalty.
    runner.start_spot_struggle(10.1);
    assert!((conserved(runner) - 95.0).abs() < EPSILON);

    // Struggling blocks conservation.
    conserve_for(runner, 10.1, 1.0);
    assert!((conserved(runner) - 95.0).abs() < EPSILON);
}

#[test]
fn test_rush_penalty() {
    // Find a seed where the runner is bound to rush.
    let mut solver = (0..)
        .map(|seed| {
            solver(
                1600,
                Style::Late,
                Motivation::Normal,
                GameVersion::LATEST,
                seed,
            )
        })
        .find(|s| s.runners[0].rush_state.is_some())
        .unwrap();
    let rush_section = solver.runners[0].rush_state.as_ref().unwrap().section;
    let runner = &mut solver.runners[0];
    runner.charge_up.as_mut().unwrap().conserved = 100.0;

    // Enter the rush section.
    runner.section = rush_section - 1;
    runner.position = solver.state.course.distance as f64 / 24.0 * rush_section as f64 + 1.0;
    solver.state.elapsed_time = 10.0;
    runner.update_phase(&mut solver.state);
    assert!(runner.is_rushed());
    assert!((conserved(runner) - 80.0).abs() < EPSILON);

    // Rushing blocks conservation.
    conserve_for(runner, 10.0, 1.0);
    assert!((conserved(runner) - 80.0).abs() < EPSILON);
}

#[test]
fn test_release() {
    let mut solver = eligible_solver();
    enter_last_spurt(&mut solver, 100.0);

    let runner = &solver.runners[0];
    let ChargeUpOutcome::Released(release) = &runner.charge_up.as_ref().unwrap().outcome else {
        panic!("Expected release");
    };
    let duration = 100.0 * 2000f64.sqrt() * 0.875 / 1450.0;
    let acceleration = 52000f64.sqrt() * 0.001 * 0.875;
    assert!((release.duration - duration).abs() < EPSILON);
    assert!((release.acceleration - acceleration).abs() < EPSILON);
    assert_eq!(release.start_time, 60.0);
    assert_eq!(release.start_position, 1500.0);

    // Bonus only while active.
    let charge_up = runner.charge_up.as_ref().unwrap();
    assert_eq!(charge_up.acceleration(59.9), 0.0);
    assert_eq!(charge_up.acceleration(60.0 + duration / 2.0), acceleration);
    assert_eq!(charge_up.acceleration(60.0 + duration), 0.0);

    // Released only once.
    let before = runner.charge_up.clone();
    let runner = &mut solver.runners[0];
    runner.charge_up.as_mut().unwrap().conserved = 500.0;
    solver.state.elapsed_time = 61.0;
    runner.update_phase(&mut solver.state);
    assert_eq!(
        runner.charge_up.as_ref().unwrap().outcome,
        before.unwrap().outcome
    );
}

#[test]
fn test_release_increases_acceleration() {
    let mut solver = eligible_solver();
    enter_last_spurt(&mut solver, 100.0);
    let runner = &mut solver.runners[0];
    runner.is_start_dash = false;
    runner.phase = crate::course::Phase::Late;
    runner.speed = 15.0;
    runner.target_speed = 20.0;

    runner.update_acceleration(59.0);
    let without = runner.acceleration;
    runner.update_acceleration(60.5);
    let with = runner.acceleration;
    let expected = 52000f64.sqrt() * 0.001 * 0.875;
    assert!((with - without - expected).abs() < EPSILON);
}

#[test]
fn test_insufficient_release() {
    let mut solver = eligible_solver();
    // 3 * sqrt(2000) * 0.875 / 1450 = 0.081s < 0.2s.
    enter_last_spurt(&mut solver, 3.0);
    let charge_up = solver.runners[0].charge_up.as_ref().unwrap();
    assert_eq!(charge_up.outcome, ChargeUpOutcome::Insufficient);
    assert_eq!(charge_up.acceleration(60.05), 0.0);
}

#[test]
fn test_full_race() {
    for seed in 0..5 {
        let mut solver = solver(
            1600,
            Style::Late,
            Motivation::Normal,
            GameVersion::LATEST,
            seed,
        );
        solver.run();
        let runner = &solver.runners[0];
        let charge_up = runner.charge_up.as_ref().unwrap();
        let ChargeUpOutcome::Released(release) = &charge_up.outcome else {
            panic!("seed {seed}: expected release, got {:?}", charge_up.outcome);
        };
        let target_distance = runner.last_spurt.as_ref().unwrap().target_distance;
        assert!(release.start_position >= target_distance, "seed {seed}");
        assert!(release.conserved > 0.0, "seed {seed}");
        // Nothing is conserved after the release.
        assert_eq!(release.conserved, charge_up.conserved, "seed {seed}");
    }
}
