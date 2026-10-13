use super::*;

#[test]
fn test_positioning_speed_multiplier() {
    use PositioningStrategy::*;
    let multiplier =
        |strategy: PositioningStrategy, phase, version| strategy.speed_multiplier(phase, version);
    let latest = GameVersion::LATEST;
    for phase in [Phase::Early, Phase::Mid] {
        assert_eq!(multiplier(Normal, phase, latest), 1.0);
        assert_eq!(multiplier(FrontSpeedUp, phase, latest), 1.04);
        assert_eq!(multiplier(FrontOvertake, phase, latest), 1.05);
        assert_eq!(multiplier(PaceUp(5.0), phase, latest), 1.04);
        assert_eq!(multiplier(PaceUpEx, phase, latest), 2.0);
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
