use super::*;

#[test]
fn test_positioning_speed_multiplier() {
    use PositioningStrategy::*;
    let multiplier =
        |strategy: PositioningStrategy, phase, version| strategy.speed_multiplier(phase, version);
    let latest = GameVersion::LATEST;
    // Only pace down depends on the phase.
    let cases = [
        (Normal, 1.0),
        (FrontSpeedUp, 1.04),
        (FrontOvertake, 1.05),
        (PaceUp(5.0), 1.04),
        (PaceUpEx, 2.0),
    ];
    for (strategy, expected) in cases {
        let actual = multiplier(strategy, Phase::Mid, latest);
        assert_eq!(actual, expected, "{strategy:?}");
    }

    // Pace down is milder in the Mid race from 1.5 Anniv. onward.
    let cases = [
        (Phase::Early, GameVersion::FirstAnniversary, 0.915),
        (Phase::Mid, GameVersion::FirstAnniversary, 0.915),
        (Phase::Early, GameVersion::OnePointFiveAnniversary, 0.915),
        (Phase::Mid, GameVersion::OnePointFiveAnniversary, 0.945),
    ];
    for (phase, version, expected) in cases {
        let actual = multiplier(PaceDown(5.0), phase, version);
        assert_eq!(actual, expected, "{phase:?} {version:?}");
    }
}
