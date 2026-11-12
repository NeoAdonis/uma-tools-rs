use super::*;
use crate::configuration::GameVersion;
use crate::race::timed_boost::TimedBoost;
use crate::test_utils::{assert_close, ready_runner, seeded_config};
use crate::uma::*;

#[test]
fn test_desirable_lead() {
    use EffectiveStyle::*;
    // At -1000m, the distance term is zero.
    let cases = [
        (Runaway, Front, -1000.0, 2.0),
        (Front, End, -1000.0, 8.0),
        (Late, End, -1000.0, 3.0),
        (Front, Pace, 2000.0, 4.9),
    ];
    for (style, other, distance, expected) in cases {
        let actual = desirable_lead(style, other, distance).unwrap();
        assert_close!(actual, expected, "{style:?} -> {other:?}, {distance}m");
    }
}

#[test]
fn test_desirable_lead_only_for_later_styles() {
    use EffectiveStyle::*;
    let styles = [Runaway, Front, Pace, Late, End];
    for style in styles {
        for other in styles {
            let lead = desirable_lead(style, other, 2000.0);
            assert_eq!(lead.is_some(), other > style, "{style:?} -> {other:?}");
        }
    }
}

#[test]
fn test_target_speed_bonus() {
    const LATEST: GameVersion = GameVersion::LATEST;
    // Guts 2000 -> sqrt(1) * 0.3.
    let base = 0.3;
    let cases = [
        (EffectiveStyle::Runaway, true, LATEST, base * 0.2 * 7.0),
        (EffectiveStyle::Front, true, LATEST, base * 4.0),
        (
            EffectiveStyle::Front,
            true,
            GameVersion::MechaUmaUpdate,
            base * 2.0,
        ),
        (EffectiveStyle::Pace, true, LATEST, base),
        (EffectiveStyle::Late, false, LATEST, base * 0.8),
    ];
    for (style, is_alone, version, expected) in cases {
        let actual = target_speed_bonus(style, 2000.0, is_alone, version);
        assert_close!(
            actual,
            expected,
            "{style:?} (alone: {is_alone}), {version:?}"
        );
    }

    let actual = target_speed_bonus(EffectiveStyle::Pace, 500.0, false, LATEST);
    assert_close!(actual, 0.5 * 0.3);
}

#[test]
fn test_hp_cost_distance_brackets() {
    let cases = [
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
    ];
    for (distance, coef) in cases {
        let actual = hp_cost(EffectiveStyle::Front, distance);
        let expected = 20.0 * coef;
        assert_close!(actual, expected, "{distance}m");
    }
}

#[test]
fn test_hp_cost_style_coefficients() {
    for (style, coef) in [(EffectiveStyle::Runaway, 1.2), (EffectiveStyle::Late, 0.8)] {
        let actual = hp_cost(style, 2400);
        assert_close!(actual, 20.0 * coef, "{style:?}");
    }
}

#[test]
fn test_secure_lead_check_window() {
    let (mut runner, mut state) = ready_runner(EffectiveStyle::Front, seeded_config(42));
    state.elapsed_time = 100.0;
    for (section, expected) in [(9, false), (10, true), (14, true), (15, false)] {
        runner.section = section;
        let actual = runner.is_secure_lead_check_due(&state);
        assert_eq!(actual, expected, "section {section}");
    }

    let (mut end_closer, mut state) = ready_runner(EffectiveStyle::End, seeded_config(42));
    state.elapsed_time = 100.0;
    end_closer.section = 12;
    assert!(!end_closer.is_secure_lead_check_due(&state));
}

#[test]
fn test_secure_lead_activation_and_expiry() {
    let (mut runner, mut state) = ready_runner(EffectiveStyle::Front, seeded_config(42));
    runner.section = 12;

    // Sufficient lead never triggers.
    let (lead_insufficient, is_alone) = (false, false);
    for t in 0..50 {
        let elapsed = t as f64 * CHECK_INTERVAL;
        state.elapsed_time = elapsed;
        runner.try_secure_lead(lead_insufficient, is_alone, &state);
        let next_check = elapsed + CHECK_INTERVAL;
        assert_eq!(runner.secure_lead, TimedBoost::Idle { next_check });
    }

    // Insufficient lead eventually triggers.
    let lead_insufficient = true;
    let hp_before = runner.hp.current();
    let mut elapsed = 0.0;
    while matches!(runner.secure_lead, TimedBoost::Idle { .. }) {
        elapsed += CHECK_INTERVAL;
        state.elapsed_time = elapsed;
        runner.try_secure_lead(lead_insufficient, is_alone, &state);
        assert!(elapsed < 1000.0, "secure lead never triggered");
    }

    let expected_modifier = target_speed_bonus(
        EffectiveStyle::Front,
        runner.adjusted.guts,
        is_alone,
        GameVersion::LATEST,
    );
    let expected_boost = TimedBoost::start(expected_modifier, elapsed, DURATION, COOLDOWN);
    assert_eq!(runner.secure_lead, expected_boost);
    let expected_hp = hp_before - hp_cost(EffectiveStyle::Front, state.course.distance);
    assert_close!(runner.hp.current(), expected_hp);

    // The boost ends after its duration.
    runner.secure_lead.expire(elapsed + DURATION);
    assert!(matches!(runner.secure_lead, TimedBoost::Idle { .. }));
}

#[test]
fn test_secure_lead_chance() {
    // ? The chance should scale with wit, but the formula is unknown: this
    // pins the 20% assumed for now, and changes with it.
    let (mut runner, mut state) = ready_runner(EffectiveStyle::Front, seeded_config(42));
    runner.section = 12;
    let checks = 1000;
    let mut activations = 0;
    for check in 0..checks {
        state.elapsed_time = check as f64 * 10.0;
        runner.secure_lead = TimedBoost::Idle { next_check: 0.0 };
        let (lead_insufficient, is_alone) = (true, false);
        runner.try_secure_lead(lead_insufficient, is_alone, &state);
        if matches!(runner.secure_lead, TimedBoost::Active { .. }) {
            activations += 1;
        }
    }
    let rate = activations as f64 / checks as f64;
    assert!((0.17..0.23).contains(&rate), "{rate}");
}
