//! Runtime tests for distance-based triggers (random points, remaining distance,
//! and ranges made of monotonic comparisons).

use crate::configuration::{Configuration, GameBehavior};
use crate::course::{Course, Phase};
use crate::race::parameters::*;
use crate::race::solver::RaceSolver;
use crate::uma::skill::*;
use crate::uma::*;

/// Length of the test course (10104) in meters.
const COURSE_DISTANCE: f64 = 2000.0;

/// Build a skill with the given condition and a harmless effect.
/// Wisdom check is disabled so the skill is always available.
fn skill(condition: &str) -> Skill {
    Skill::new(
        "999999".to_string(),
        SkillData {
            rarity: SkillRarity::White,
            alternatives: vec![SkillAlternative {
                base_duration: 10000,
                condition: condition.to_string(),
                effects: vec![SkillEffect {
                    skill_type: SkillType::TargetSpeed,
                    target: SkillTarget::Oneself,
                    modifier: 1,
                }],
                precondition: String::new(),
            }],
            wisdom_check: false,
        },
    )
}

fn course() -> Course {
    Course::from_json_file("data/course_data.json", "10104").expect("Failed to load course data")
}

/// Run a single-runner race with the given skill condition and return the
/// positions at which the skill activated.
fn activation_positions(condition: &str, rng_seed: u64) -> Vec<f64> {
    let params = RaceParameters {
        track_condition: TrackCondition::Firm,
        weather: Weather::Sunny,
        season: Season::Spring,
        time: Time::None,
        grade: Grade::G1,
        gates: 9,
        rng_seed,
    };
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
    let config = Configuration {
        game_behavior: GameBehavior::LATEST,
        ..Default::default()
    };
    let mut solver = RaceSolver::new(course(), params, &config);
    solver.add_runner(uma, Motivation::Normal, vec![skill(condition)]);
    let mut solver = solver.ready_runners();
    solver.run();
    solver.runners[0]
        .activated_skills
        .iter()
        .map(|record| record.distance)
        .collect()
}

/// Assert that the skill activates exactly once within `[min, max]` meters.
fn assert_activates_within(condition: &str, min: f64, max: f64) {
    for seed in 0..5 {
        let positions = activation_positions(condition, seed);
        assert_eq!(
            positions.len(),
            1,
            "`{condition}` (seed {seed}) should activate once, got {positions:?}"
        );
        assert!(
            (min..=max).contains(&positions[0]),
            "`{condition}` (seed {seed}) activated at {}, expected [{min}, {max}]",
            positions[0]
        );
    }
}

/// Maximum distance covered in a single frame (15 FPS) at a generous speed.
const FRAME_DISTANCE: f64 = 30.0 / 15.0;

#[test]
fn test_random_point_activates_within_phase() {
    let (start, end) = Phase::Mid.boundaries(&course());
    assert_activates_within("phase_random==1", start, end + FRAME_DISTANCE);
}

#[test]
fn test_remain_distance_activates_near_goal() {
    let threshold = COURSE_DISTANCE - 300.0;
    assert_activates_within(
        "remain_distance<=300",
        threshold,
        threshold + FRAME_DISTANCE,
    );
}

#[test]
fn test_reversed_remain_distance_activates_early() {
    // remain_distance>=350 holds from the start until 1650m
    assert_activates_within("remain_distance>=350", 0.0, COURSE_DISTANCE - 350.0);
}

#[test]
fn test_remain_distance_range_activates_within_range() {
    // Skill 100771
    // Detected as a window: 399 <= remain_distance <= 401
    let min = COURSE_DISTANCE - 401.0;
    let max = COURSE_DISTANCE - 399.0;
    assert_activates_within("remain_distance<=401&remain_distance>=399", min, max);
}

#[test]
fn test_distance_rate_range_in_any_order_activates_within_range() {
    // A range that isn't detected as a window because the bounds aren't siblings
    let min = COURSE_DISTANCE * 0.5;
    let max = COURSE_DISTANCE * 0.61; // guards are floored
    assert_activates_within(
        "distance_rate>=50&ground_type==1&distance_rate<=60",
        min,
        max,
    );
}

#[test]
fn test_distance_triggers_are_positions_in_order() {
    use crate::trigger::builder::build_trigger_scheduler;
    use crate::units::furlongs_to_meters;
    use rand::SeedableRng;

    let skills = [
        skill("remain_distance<=300"), // 1700m
        skill("distance_rate>=50"),    // 1000m
        skill("furlong>=3"),           // 3 furlongs
    ];
    let mut rng = rand_pcg::Pcg64Mcg::seed_from_u64(0);
    let mut scheduler = build_trigger_scheduler(&skills, &course(), &mut rng);

    let order: Vec<(usize, f64)> = std::iter::from_fn(|| scheduler.distance_triggers.pop())
        .map(|t| (t.logic_idx, t.threshold_meters))
        .collect();
    assert_eq!(
        order,
        vec![
            (2, furlongs_to_meters(3.0)),
            (1, 1000.0),
            (0, COURSE_DISTANCE - 300.0)
        ]
    );
}

#[test]
fn test_reversed_remain_distance_window_range() {
    use crate::trigger::builder::build_trigger_scheduler;
    use rand::SeedableRng;

    let mut rng = rand_pcg::Pcg64Mcg::seed_from_u64(0);
    let scheduler = build_trigger_scheduler(&[skill("remain_distance>=350")], &course(), &mut rng);

    assert_eq!(scheduler.window_triggers.len(), 1);
    let window = &scheduler.window_triggers[0];
    assert_eq!(window.min_threshold, 0.0);
    assert_eq!(window.max_threshold, COURSE_DISTANCE - 350.0);
}

#[test]
fn test_remain_distance_pair_window_range() {
    use crate::trigger::builder::build_trigger_scheduler;
    use rand::SeedableRng;

    let mut rng = rand_pcg::Pcg64Mcg::seed_from_u64(0);
    let scheduler = build_trigger_scheduler(
        &[skill("remain_distance<=401&remain_distance>=399")],
        &course(),
        &mut rng,
    );

    // A single window, no distance triggers or guards
    assert!(scheduler.distance_triggers.is_empty());
    assert!(scheduler.skill_logics[0].guards.is_empty());
    assert_eq!(scheduler.window_triggers.len(), 1);
    let window = &scheduler.window_triggers[0];
    assert_eq!(window.min_threshold, COURSE_DISTANCE - 401.0);
    assert_eq!(window.max_threshold, COURSE_DISTANCE - 399.0);
}
