use super::*;
use crate::test_utils::{assert_close, effect};

#[test]
fn test_start_type_from_delay() {
    let cases = [
        (0.0, StartType::Good),
        (0.019, StartType::Good),
        (0.02, StartType::Normal),
        (0.08, StartType::Normal),
        (0.081, StartType::Late),
    ];
    for (delay, expected) in cases {
        assert_eq!(StartType::from_delay(delay), expected, "delay {delay}");
    }
}

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

fn timed(skill_type: SkillType, value: f64) -> ActiveTimedEffect {
    ActiveTimedEffect {
        effect: effect(skill_type, value),
        value,
        expires_at: 10.0,
        overtake_extension: OvertakeExtension::default(),
        additional_triggers: 0,
    }
}

#[test]
fn test_skill_modifiers_from_effects() {
    let mods = SkillModifiers::from_effects(&[]);
    assert_eq!(mods.target_speed, 0.0);
    assert_eq!(mods.target_lane, None);
    assert_eq!(mods.duration_scaling, 1.0);

    let effects = [
        timed(SkillType::TargetSpeed, 0.15),
        timed(SkillType::TargetSpeed, 0.25),
        timed(SkillType::CurrentSpeed, 0.1),
        timed(SkillType::CurrentSpeedWithNaturalDeceleration, 0.05),
        timed(SkillType::Acceleration, 0.2),
        timed(SkillType::AccelerationAllOutSpurt, 0.3),
        timed(SkillType::LaneMovementSpeed, 0.02),
        timed(SkillType::Fov, -5.0),
        timed(SkillType::LaneChange, 0.5),
        timed(SkillType::ExtendEvolvedDuration, 1.5),
        timed(SkillType::ExtendEvolvedDuration, 2.0),
        // Not a modifier.
        timed(SkillType::Recovery, 0.05),
    ];
    let mods = SkillModifiers::from_effects(&effects);
    assert_close!(mods.target_speed, 0.4);
    assert_close!(mods.current_speed, 0.15);
    assert_close!(mods.acceleration, 0.2);
    assert_close!(mods.all_out_spurt_acceleration, 0.3);
    assert_close!(mods.lane_speed, 0.02);
    assert_close!(mods.visible_distance, -5.0);
    assert_eq!(mods.target_lane, Some(0.5));
    // Multiplicative.
    assert_close!(mods.duration_scaling, 3.0);
}
