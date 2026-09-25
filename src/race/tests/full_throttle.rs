use crate::configuration::{Configuration, GameBehavior};
use crate::course::Course;
use crate::race::full_throttle::*;
use crate::race::last_spurt::*;
use crate::race::parameters::*;
use crate::race::runner::{MAX_SPEED, Runner};
use crate::race::solver::RaceSolver;
use crate::trigger::skill_activation::TriggerCondition;
use crate::trigger::system::DistanceTriggerType;
use crate::uma::skill::*;
use crate::uma::*;

const EPSILON: f64 = 1e-9;
const DELTA_TIME: f64 = 1.0 / 15.0;

/// Same as skill 204222, but without wisdom check so it is always available.
// TODO: Replace with a global switch to disable wisdom checks (see docs/TODOS.md).
fn full_throttle_skill() -> Skill {
    Skill::new(
        "204222".to_string(),
        SkillData {
            rarity: SkillRarity::White,
            alternatives: vec![SkillAlternative {
                base_duration: 20000,
                condition: "run_at_full_speed_random==1&distance_type==3".to_string(),
                effects: vec![SkillEffect {
                    skill_type: SkillType::AccelerationFullThrottle,
                    target: SkillTarget::Oneself,
                    modifier: 2000,
                }],
                precondition: String::new(),
            }],
            wisdom_check: false,
        },
    )
}

fn params(rng_seed: u64) -> RaceParameters {
    RaceParameters {
        track_condition: TrackCondition::Firm,
        weather: Weather::Sunny,
        season: Season::Spring,
        time: Time::None,
        grade: Grade::G1,
        gates: 9,
        rng_seed,
    }
}

fn uma(speed: u16) -> Uma {
    Uma {
        speed,
        stamina: 1600,
        power: 1200,
        guts: 800,
        wiz: 1000,
        style: Style::Late,
        distance_aptitude: Aptitude::A,
        track_aptitude: Aptitude::A,
        style_aptitude: Aptitude::A,
    }
}

/// Build a solver with a single runner on a 2000m course whose threshold stat
/// is speed. Raw speed 2500 gives an adjusted speed of 1850 * 1.2 = 2220.
fn solver(
    speed: u16,
    game_behavior: GameBehavior,
    skills: Vec<Skill>,
    rng_seed: u64,
) -> RaceSolver<Runner> {
    let course = Course::from_json_file("data/course_data.json", "10204")
        .expect("Failed to load course data");
    let config = Configuration {
        game_behavior,
        ..Default::default()
    };
    let mut solver = RaceSolver::new(course, params(rng_seed), &config);
    solver.add_runner(uma(speed), Motivation::Normal, skills);
    solver.ready_runners()
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
    let latest = GameBehavior::LATEST;
    assert_eq!(base_acceleration(2000, 1200, 2000, latest), 0.0);
    assert_eq!(
        base_acceleration(2200, 1200, 2000, GameBehavior::MechaUmaUpdate),
        0.0
    );
    // 0.068 * 0.0006 * sqrt(500 * 1200) / 2^1.5
    let expected = 0.068 * 0.0006 * 600000f64.sqrt() / 8f64.sqrt();
    assert!((base_acceleration(2001, 1200, 2000, latest) - expected).abs() < EPSILON);
    assert!((expected - 0.011173).abs() < 1e-6);
    // Longer courses accelerate less
    assert!(base_acceleration(2001, 1200, 3000, latest) < expected);
}

#[test]
fn test_activates_at_last_spurt_speed() {
    let mut solver = solver(2500, GameBehavior::LATEST, vec![], 42);
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
        // Raw 2000 is adjusted to 1600 * 1.2 = 1920
        (2000, GameBehavior::LATEST),
        (2500, GameBehavior::MechaUmaUpdate),
    ] {
        let mut solver = solver(speed, behavior, vec![], 42);
        assert_eq!(solver.runners[0].full_throttle_base, 0.0);
        enter_last_spurt(&mut solver, 20.0);
        assert_eq!(solver.runners[0].full_throttle, FullThrottle::Pending);
    }
}

#[test]
fn test_speed_bonus_increases_target_speed() {
    let mut solver = solver(2500, GameBehavior::LATEST, vec![], 42);
    enter_last_spurt(&mut solver, 20.0);
    let target_before = solver.runners[0].target_speed;

    let runner = &mut solver.runners[0];
    runner.skill_modifiers.full_throttle_acceleration = 0.4;
    runner.update_speed(DELTA_TIME, solver.state.course.base_speed());

    let expected = (runner.full_throttle_base + 0.04) * DELTA_TIME;
    assert!((runner.full_throttle.speed_bonus() - expected).abs() < EPSILON);

    runner.update_target_speed(&solver.state);
    assert!((runner.target_speed - target_before - expected).abs() < EPSILON);
}

#[test]
fn test_hp_out_drops_speed_to_minimum() {
    let mut solver = solver(2500, GameBehavior::LATEST, vec![], 42);
    enter_last_spurt(&mut solver, 20.0);

    let runner = &mut solver.runners[0];
    runner.speed = 24.0;
    runner.hp.current = 0.001;
    runner.update_hp(DELTA_TIME, &solver.state);
    assert_eq!(runner.speed, runner.min_speed);
    assert_eq!(runner.full_throttle, FullThrottle::Ended);

    // Can't be activated again, even if HP is recovered
    runner.hp.recover(100.0);
    runner.speed = 20.0;
    runner.update_target_speed(&solver.state);
    assert_eq!(runner.full_throttle, FullThrottle::Ended);
}

#[test]
fn test_speed_is_capped() {
    let mut solver = solver(2500, GameBehavior::LATEST, vec![], 42);
    let runner = &mut solver.runners[0];
    runner.is_start_dash = false;
    runner.speed = MAX_SPEED - 0.01;
    runner.acceleration = 1.0;
    runner.update_speed(DELTA_TIME, solver.state.course.base_speed());
    assert_eq!(runner.speed, MAX_SPEED);
}

#[test]
fn test_random_trigger_scheduled_on_activation() {
    let mut solver = solver(2500, GameBehavior::LATEST, vec![full_throttle_skill()], 42);
    {
        let scheduler = &solver.runners[0].trigger_scheduler;
        assert_eq!(scheduler.full_throttle_triggers, vec![0]);
        let logic = &scheduler.skill_logics[0];
        assert!(matches!(
            logic.triggers[..],
            [TriggerCondition::FullThrottleStart]
        ));
        // Only distance_type remains as guard
        assert_eq!(logic.guards.len(), 1);
        assert!(scheduler.distance_triggers.is_empty());
    }

    enter_last_spurt(&mut solver, 20.0);

    let scheduler = &solver.runners[0].trigger_scheduler;
    assert!(scheduler.full_throttle_triggers.is_empty());
    let trigger = scheduler.distance_triggers.peek().unwrap();
    assert_eq!(trigger.logic_idx, 0);
    assert_eq!(trigger.trigger_type, DistanceTriggerType::DistanceRate);
    assert!((1500.0..2000.0).contains(&trigger.threshold_meters));
}

#[test]
fn test_full_race() {
    let mut activated_any = false;
    for seed in 0..5 {
        let mut with = solver(
            2500,
            GameBehavior::LATEST,
            vec![full_throttle_skill()],
            seed,
        );
        with.run();
        let runner = &with.runners[0];
        assert_ne!(runner.full_throttle, FullThrottle::Pending, "seed {seed}");

        // The skill activates after the last spurt begins
        let target_distance = runner.last_spurt.as_ref().unwrap().target_distance;
        for record in &runner.activated_skills {
            assert!(record.distance >= target_distance, "seed {seed}");
        }
        activated_any |= !runner.activated_skills.is_empty();

        // Without Full-Throttle Sprint (and skills), the runner is slower.
        // Nothing else differs between these game behaviors for this runner.
        let mut with = solver(2500, GameBehavior::LATEST, vec![], seed);
        let mut without = solver(2500, GameBehavior::MechaUmaUpdate, vec![], seed);
        with.run();
        without.run();
        let with_time = with.runners[0].finish_time.unwrap();
        let without_time = without.runners[0].finish_time.unwrap();
        assert!(with_time < without_time, "seed {seed}");
    }
    assert!(activated_any);
}
