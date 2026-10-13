use super::*;
use crate::configuration::GameVersion;
use crate::race::runner::Runner;
use crate::race::solver::RaceState;
use crate::race::timed_boost::TimedBoost;
use crate::test_utils::{assert_close, ready_solver, seeded_config, uma};
use crate::uma::*;

#[test]
fn test_desirable_lead_table() {
    use Style::*;
    // Distance term is zero at -1000m, so the coefficient is returned as-is.
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
        assert_close!(actual, expected, "{style:?} -> {other:?}");
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
    assert_close!(lead, 4.9);
}

#[test]
fn test_target_speed_bonus() {
    // Guts 2000 -> sqrt(1) * 0.3.
    let base = 0.3;
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
        let actual = target_speed_bonus(style, 2000, is_alone, GameVersion::LATEST);
        assert_close!(actual, expected, "{style:?} (alone: {is_alone})");
    }

    let actual = target_speed_bonus(Style::Pace, 500, false, GameVersion::LATEST);
    assert_close!(actual, 0.5 * 0.3);
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
        assert_close!(actual, expected, "{distance}m");
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
        assert_close!(actual, 20.0 * coef, "{style:?}");
    }
}

fn ready_runner(style: Style) -> (Runner, RaceState) {
    let solver = ready_solver(10101, uma(style), vec![], &seeded_config(42));
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

    // Sufficient lead never triggers.
    for t in 0..50 {
        let elapsed = t as f64 * CHECK_INTERVAL;
        state.elapsed_time = elapsed;
        runner.try_secure_lead(false, false, &state);
        let next_check = elapsed + CHECK_INTERVAL;
        assert_eq!(runner.secure_lead, TimedBoost::Idle { next_check });
    }

    // Insufficient lead eventually triggers.
    let hp_before = runner.hp.current;
    let mut elapsed = 0.0;
    while matches!(runner.secure_lead, TimedBoost::Idle { .. }) {
        elapsed += CHECK_INTERVAL;
        state.elapsed_time = elapsed;
        runner.try_secure_lead(true, false, &state);
        assert!(elapsed < 1000.0, "secure lead never triggered");
    }

    let expected_modifier = target_speed_bonus(
        Style::Front,
        runner.adjusted.guts,
        false,
        GameVersion::LATEST,
    );
    let expected_boost = TimedBoost::start(expected_modifier, elapsed, DURATION, COOLDOWN);
    assert_eq!(runner.secure_lead, expected_boost);
    let expected_hp = hp_before - hp_cost(Style::Front, state.course.distance);
    assert_close!(runner.hp.current, expected_hp);

    // Boost is applied to target speed.
    runner.update_target_speed(&state);
    let boosted = runner.target_speed;
    runner.secure_lead.expire(elapsed + DURATION);
    assert!(matches!(runner.secure_lead, TimedBoost::Idle { .. }));
    runner.update_target_speed(&state);
    assert_close!(boosted - runner.target_speed, expected_modifier);
}
