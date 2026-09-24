use crate::configuration::{Configuration, GameBehavior};
use crate::course::Course;
use crate::race::parameters::*;
use crate::race::runner::Runner;
use crate::race::secure_lead::*;
use crate::race::solver::{RaceSolver, RaceState};
use crate::uma::*;

const EPSILON: f64 = 1e-9;

#[test]
fn test_desirable_lead_table() {
    use Style::*;
    // Distance term is zero at -1000m, so the coefficient is returned as-is
    let cases = [
        (Runaway, Front, 2.0),
        (Runaway, Pace, 7.0),
        (Runaway, Late, 8.0),
        (Runaway, End, 8.0),
        (Front, Pace, 4.0),
        (Front, Late, 8.0),
        (Front, End, 8.0),
        (Pace, Late, 5.0),
        (Pace, End, 6.0),
        (Late, End, 3.0),
    ];
    for (style, other, expected) in cases {
        let actual = desirable_lead(style, other, -1000.0).unwrap();
        assert!(
            (actual - expected).abs() < EPSILON,
            "{style:?} -> {other:?}: expected {expected}, got {actual}"
        );
    }
}

#[test]
fn test_desirable_lead_only_for_later_styles() {
    use Style::*;
    let styles = [Runaway, Front, Pace, Late, End];
    for style in styles {
        for other in styles {
            let lead = desirable_lead(style, other, 2000.0);
            assert_eq!(lead.is_some(), other > style, "{style:?} -> {other:?}");
        }
    }
}

#[test]
fn test_desirable_lead_distance_term() {
    let lead = desirable_lead(Style::Front, Style::Pace, 2000.0).unwrap();
    assert!((lead - 4.9).abs() < EPSILON);
}

#[test]
fn test_target_speed_bonus() {
    let base = 0.3; // guts 2000 -> sqrt(1) * 0.3
    let cases = [
        (Style::Runaway, false, base * 0.2),
        (Style::Runaway, true, base * 0.2 * 7.0),
        (Style::Front, false, base),
        (Style::Front, true, base * 4.0),
        (Style::Pace, false, base),
        (Style::Pace, true, base),
        (Style::Late, false, base * 0.8),
        (Style::Late, true, base * 0.8),
    ];
    for (style, is_alone, expected) in cases {
        let actual = target_speed_bonus(style, 2000, is_alone, GameBehavior::LATEST);
        assert!(
            (actual - expected).abs() < EPSILON,
            "{style:?} (alone: {is_alone}): expected {expected}, got {actual}"
        );
    }

    let actual = target_speed_bonus(Style::Pace, 500, false, GameBehavior::LATEST);
    assert!((actual - 0.5 * 0.3).abs() < EPSILON);
}

#[test]
fn test_hp_cost_distance_brackets() {
    let cases = [
        (1000, 0.3),
        (1400, 0.3),
        (1401, 0.3),
        (1800, 0.3),
        (1801, 0.5),
        (2100, 0.5),
        (2101, 0.8),
        (2200, 0.8),
        (2201, 1.0),
        (2400, 1.0),
        (2401, 1.1),
        (2600, 1.1),
        (2601, 1.2),
        (3600, 1.2),
    ];
    for (distance, coef) in cases {
        let actual = hp_cost(Style::Front, distance);
        let expected = 20.0 * coef;
        assert!(
            (actual - expected).abs() < EPSILON,
            "{distance}m: expected {expected}, got {actual}"
        );
    }
}

#[test]
fn test_hp_cost_style_coefficients() {
    let cases = [
        (Style::Runaway, 1.2),
        (Style::Front, 1.0),
        (Style::Pace, 0.8),
        (Style::Late, 0.8),
    ];
    for (style, coef) in cases {
        let actual = hp_cost(style, 2400);
        assert!((actual - 20.0 * coef).abs() < EPSILON, "{style:?}");
    }
}

fn ready_runner(style: Style) -> (Runner, RaceState) {
    let course = Course::from_json_file("data/course_data.json", "10101")
        .expect("Failed to load course data");
    let params = RaceParameters {
        track_condition: TrackCondition::Firm,
        weather: Weather::Sunny,
        season: Season::Spring,
        time: Time::None,
        grade: Grade::G1,
        gates: 9,
        rng_seed: 42,
    };
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
    let config = Configuration {
        game_behavior: GameBehavior::LATEST,
        ..Default::default()
    };
    let mut solver = RaceSolver::new(course, params, &config);
    solver.add_runner(uma, Motivation::Normal, vec![]);
    let solver = solver.ready_runners();
    let runner = solver.runners.into_iter().next().unwrap();
    (runner, solver.state)
}

#[test]
fn test_secure_lead_check_window() {
    let (mut runner, mut state) = ready_runner(Style::Front);
    state.elapsed_time = 100.0;
    for section in 0..24 {
        runner.section = section;
        assert_eq!(
            runner.is_secure_lead_check_due(&state),
            SECTIONS.contains(&section),
            "section {section}"
        );
    }

    let (mut end_closer, mut state) = ready_runner(Style::End);
    state.elapsed_time = 100.0;
    end_closer.section = 12;
    assert!(!end_closer.is_secure_lead_check_due(&state));
}

#[test]
fn test_secure_lead_activation_and_expiry() {
    let (mut runner, mut state) = ready_runner(Style::Front);
    runner.section = 12;

    // Sufficient lead never triggers
    for t in 0..50 {
        let elapsed = t as f64 * CHECK_INTERVAL;
        state.elapsed_time = elapsed;
        runner.try_secure_lead(false, false, &state);
        assert!(runner.secure_lead.is_none());
        assert!((runner.next_secure_lead_check - (elapsed + CHECK_INTERVAL)).abs() < EPSILON);
    }

    // Insufficient lead eventually triggers
    let hp_before = runner.hp.current;
    let mut elapsed = 0.0;
    while runner.secure_lead.is_none() {
        elapsed += CHECK_INTERVAL;
        state.elapsed_time = elapsed;
        runner.try_secure_lead(true, false, &state);
        assert!(elapsed < 1000.0, "secure lead never triggered");
    }

    let secure_lead = runner.secure_lead.as_ref().unwrap();
    let expected_modifier = target_speed_bonus(
        Style::Front,
        runner.adjusted.guts,
        false,
        GameBehavior::LATEST,
    );
    assert!((secure_lead.modifier - expected_modifier).abs() < EPSILON);
    assert!((secure_lead.expires_at - (elapsed + DURATION)).abs() < EPSILON);
    let expected_hp = hp_before - hp_cost(Style::Front, state.course.distance);
    assert!((runner.hp.current - expected_hp).abs() < EPSILON);
    assert!((runner.next_secure_lead_check - (elapsed + DURATION + COOLDOWN)).abs() < EPSILON);

    // Boost is applied to target speed
    runner.update_target_speed(&state);
    let boosted = runner.target_speed;
    runner.expire_secure_lead(elapsed + DURATION - 0.01);
    assert!(runner.secure_lead.is_some());
    runner.expire_secure_lead(elapsed + DURATION);
    assert!(runner.secure_lead.is_none());
    runner.update_target_speed(&state);
    assert!((boosted - runner.target_speed - expected_modifier).abs() < EPSILON);
}
