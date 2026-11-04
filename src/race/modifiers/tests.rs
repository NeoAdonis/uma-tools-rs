use super::*;
use crate::configuration::GameVersion;
use crate::course::Phase;
use crate::test_utils::assert_close;
use crate::uma::Style;

#[test]
fn test_wit_limit_break_multiplier_brackets() {
    let cases = [
        (1200, 0.0),
        (1201, 0.02),
        (1220, 0.02),
        (1221, 0.04),
        (1400, 0.2),
        (1401, 0.26),
        (1600, 0.8),
        (1601, 0.81),
        (2000, 1.0),
        (2001, 1.01),
        (2100, 1.01),
        (2101, 1.02),
        (3200, 1.12),
        // Capped from here.
        (3201, 1.12),
    ];
    for (wit, expected) in cases {
        let actual = wit_limit_break_multiplier(wit);
        assert_close!(actual, expected, "wit {wit}");
    }
}

#[test]
fn test_wit_limit_break_phase_coefficient() {
    // One phase per style, covering the arms shared by several styles or
    // phases.
    let cases = [
        (Style::Runaway, Phase::RunUp, 0.26),
        (Style::Front, Phase::Last, 0.16),
        (Style::Pace, Phase::Mid, 0.21),
        (Style::Late, Phase::Late, 0.23),
        (Style::End, Phase::Early, 0.15),
    ];
    for (style, phase, expected) in cases {
        let actual = wit_limit_break_phase_coefficient(style, phase);
        assert_eq!(actual, expected, "{style:?} {phase:?}");
    }
}

#[test]
fn test_stamina_limit_break_distance_factor() {
    // (distance, factor before Mecha Umamusume, factor since).
    let cases = [
        (2100, 0.0, 0.0),
        (2101, 0.5, 0.5),
        (2200, 0.5, 0.5),
        (2201, 1.0, 1.0),
        (2400, 1.0, 1.0),
        (2401, 1.2, 1.5),
        (2600, 1.2, 1.5),
        (2601, 1.5, 1.8),
    ];
    for (distance, before, since) in cases {
        let factor = |version| stamina_limit_break_distance_factor(distance, version);
        assert_eq!(
            factor(GameVersion::TwoPointFiveAnniversary),
            before,
            "{distance}m"
        );
        assert_eq!(factor(GameVersion::MechaUmaUpdate), since, "{distance}m");
    }
}

#[test]
fn test_stamina_limit_break_base_buff() {
    let buff = |stamina, version| stamina_limit_break_base_buff(stamina, 2400, version);
    assert_eq!(buff(1200, GameVersion::LATEST), 0.0);
    assert_close!(buff(1300, GameVersion::LATEST), 10.0 * 0.0085);
    // Not in the game yet.
    assert_eq!(buff(1300, GameVersion::SecondAnniversary), 0.0);
}

#[test]
fn test_slope_speed_modifiers() {
    // (slope percent, downhill modifier).
    for (slope, expected) in [(-1.0, 0.4), (-4.0, 0.7)] {
        assert_close!(downhill_speed_modifier(slope), expected, "slope {slope}");
    }
    // (slope percent, power, uphill modifier).
    for (slope, power, expected) in [(1.0, 1000, -0.2), (2.0, 800, -0.5)] {
        let actual = uphill_speed_modifier(slope, power);
        assert_close!(actual, expected, "slope {slope}, power {power}");
    }
}
