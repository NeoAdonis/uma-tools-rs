use super::*;

#[test]
fn test_speed_effects_stop_pacing_down() {
    use crate::test_utils::*;
    // (current speed modifier, target speed modifier, whether she stops).
    let cases = [
        (0.0, 0.0, false),
        (0.00009, -0.00009, false),
        (-0.0001, 0.0, true),
        (0.0, 0.0001, true),
    ];
    for (current, target, stops) in cases {
        let (mut runner, mut state) = ready_runner(Style::Late, seeded_config(0));
        state.elapsed_time = 10.0;
        runner.enter_positioning(PositioningStrategy::PaceDown(10.0), &state);
        runner.skill_modifiers.current_speed = current;
        runner.skill_modifiers.target_speed = target;
        // Not yet beyond the threshold.
        runner.update_positioning_non_front(5.0, Default::default(), &state);
        let pacing_down = matches!(
            runner.positioning_strategy,
            PositioningStrategy::PaceDown(_)
        );
        assert_eq!(pacing_down, !stops, "{current} {target}");
    }

    // Nor do they start while skills change the speed (the pacemaker is 0m
    // ahead, too close for a Late runner).
    for (target, starts) in [(0.0, true), (0.1, false)] {
        let (mut runner, mut state) = ready_runner(Style::Late, seeded_config(0));
        state.elapsed_time = 10.0;
        runner.next_positioning_check = 0.0;
        runner.skill_modifiers.target_speed = target;
        runner.update_positioning_non_front(0.0, Default::default(), &state);
        let pacing_down = matches!(
            runner.positioning_strategy,
            PositioningStrategy::PaceDown(_)
        );
        assert_eq!(pacing_down, starts, "{target}");
    }
}

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

    // Pace down is milder in the Mid race from 1.5 anniv. onward.
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
