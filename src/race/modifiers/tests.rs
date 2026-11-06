use super::*;
use crate::configuration::GameVersion;
use crate::course::Phase;
use crate::test_utils::assert_close;
use crate::uma::Style;

#[test]
fn test_wit_limit_break_multiplier_brackets() {
    let cases = [
        (1200.0, 0.0),
        (1201.0, 0.02),
        (1220.0, 0.02),
        (1221.0, 0.04),
        (1400.0, 0.2),
        (1401.0, 0.26),
        (1600.0, 0.8),
        (1601.0, 0.81),
        (2000.0, 1.0),
        (2001.0, 1.01),
        (2100.0, 1.01),
        (2101.0, 1.02),
        (3200.0, 1.12),
        // Capped from here.
        (3201.0, 1.12),
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
    assert_eq!(buff(1200.0, GameVersion::LATEST), 0.0);
    // In the game since 2.5 Anniversary.
    let since = GameVersion::TwoPointFiveAnniversary;
    assert_close!(buff(1300.0, since), 10.0 * 0.0085);
    assert_eq!(buff(1300.0, GameVersion::SecondAnniversary), 0.0);
}

#[test]
fn test_base_target_speed() {
    // 20 m/s base speed, 1000 speed. The speed stat only counts from the Late
    // race, scaled by distance aptitude (B: 0.9).
    let late_bonus = f64::sqrt(500.0 * 1000.0) * 0.9 * 0.002;
    let cases = [
        (Style::Pace, Phase::Mid, 20.0 * 0.991),
        (Style::Late, Phase::Late, 20.0 * 0.994 + late_bonus),
    ];
    for (style, phase, expected) in cases {
        let actual = base_target_speed(20.0, style, phase, 1000.0, Aptitude::B);
        assert_close!(actual, expected, "{style:?} {phase:?}");
    }
}

#[test]
fn test_section_speed_modifier_range() {
    use rand::SeedableRng;

    // 1000 wit: up to 1000 * log10(100) / 550000 of the 20 m/s base speed,
    // and down to 0.0065 less.
    let max = 20.0 * 2000.0 / 550000.0;
    let min = max - 20.0 * 0.0065;
    let mut rng = rand::rngs::StdRng::seed_from_u64(0);
    let rolls: Vec<f64> = (0..1000)
        .map(|_| roll_section_speed_modifier(&mut rng, 1000.0, 20.0))
        .collect();
    assert!(rolls.iter().all(|roll| (min..=max).contains(roll)));
    // Rolls spread over the whole range.
    let lowest = rolls.iter().copied().fold(f64::MAX, f64::min);
    let highest = rolls.iter().copied().fold(f64::MIN, f64::max);
    assert!(
        lowest < min + 0.01 && highest > max - 0.01,
        "{lowest}..{highest}"
    );
}

#[test]
fn test_adjusted_power_by_track_condition() {
    // One row per distinct modifier on each surface.
    let cases = [
        (Track::Turf, TrackCondition::Firm, 1000.0),
        (Track::Turf, TrackCondition::Soft, 950.0),
        (Track::Dirt, TrackCondition::Good, 950.0),
        (Track::Dirt, TrackCondition::Heavy, 900.0),
    ];
    for (track, condition, expected) in cases {
        let actual = calculate_adjusted_power(1000.0, track, condition);
        assert_eq!(actual, expected, "{track:?} {condition:?}");
    }
}

#[test]
fn test_adjusted_wit_by_style_aptitude() {
    for (aptitude, expected) in [(Aptitude::S, 1100.0), (Aptitude::B, 850.0)] {
        let actual = calculate_adjusted_wit(1000.0, aptitude);
        assert_eq!(actual, expected, "{aptitude:?}");
    }
}

#[test]
fn test_course_speed_modifier_averages_stat_bonuses() {
    // Speed 1200 (above 900: 0.2) and power 500 (up to 600: 0.1).
    let base = Stats {
        speed: 1200.0,
        power: 500.0,
        ..Stats::default()
    };
    let stats = [ThresholdStat::Speed, ThresholdStat::Power];
    assert_close!(course_speed_modifier(&base, &stats), 1.15);
}

#[test]
fn test_slope_speed_modifiers() {
    // (slope percent, downhill modifier).
    for (slope, expected) in [(-1.0, 0.4), (-4.0, 0.7)] {
        assert_close!(downhill_speed_modifier(slope), expected, "slope {slope}");
    }
    // (slope percent, power, uphill modifier).
    for (slope, power, expected) in [(1.0, 1000.0, -0.2), (2.0, 800.0, -0.5)] {
        let actual = uphill_speed_modifier(slope, power);
        assert_close!(actual, expected, "slope {slope}, power {power}");
    }
}
