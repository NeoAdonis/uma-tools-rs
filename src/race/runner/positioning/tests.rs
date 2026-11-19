use super::*;

#[test]
fn test_speed_increases_stop_pacing_down() {
    use crate::test_utils::*;
    // (current speed modifier, target speed modifier, whether she stops).
    let cases = [
        (0.0, 0.0, false),
        (0.00009, -0.00009, false),
        (-0.0001, 0.0, false),
        (0.0, -0.5, false),
        (0.0, 0.0001, true),
        (0.0001, 0.0, true),
    ];
    for (current, target, stops) in cases {
        let (mut runner, mut state) = ready_runner(BaseStyle::Late, seeded_config(0));
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

    // Nor do they start while skills increase the speed (the pacemaker is 0m
    // ahead, too close for a Late runner).
    for (target, starts) in [(0.0, true), (-0.1, true), (0.1, false)] {
        let (mut runner, mut state) = ready_runner(BaseStyle::Late, seeded_config(0));
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

#[test]
fn test_wit_check_never_passes_below_10_wit() {
    use crate::test_utils::*;

    // The chance is negative below 10 wit and exactly 0 at 10.
    for wit in [7.68, 10.0] {
        let (mut runner, _) = ready_runner(BaseStyle::Late, seeded_config(0));
        runner.adjusted.wit = wit;
        for _ in 0..20 {
            assert!(
                !runner.roll_positioning_wit_check(0.2, &mut RaceRng::seed_from_u64(0)),
                "{wit}"
            );
        }
    }
}

#[test]
fn test_checks_are_scheduled_from_the_previous_check() {
    use crate::race::solver::DELTA_TIME;
    use crate::test_utils::*;

    let (mut runner, mut state) = ready_runner(BaseStyle::Pace, seeded_config(0));
    // Exits a mode on frame 0, so the first check is 46 frames later.
    state.elapsed_time = 10.0;
    runner.enter_positioning(PositioningStrategy::PaceUp(1.0), &state);
    runner.exit_positioning(state.elapsed_time);

    // Within the target range of a Pace runner: no mode starts.
    let mut checks = vec![];
    for frame in 0..=150 {
        let before = runner.next_positioning_check;
        runner.update_positioning_non_front(4.0, Default::default(), &state);
        if runner.next_positioning_check != before {
            checks.push(frame);
        }
        state.elapsed_time += DELTA_TIME;
    }
    // Each check is the first frame at least 2s after the previous one, not
    // aligned to a 2s grid.
    assert_eq!(checks, [46, 77, 108, 139]);
}

/// Makes `runner` rushed with `style` and ready for a check at `state`'s time.
fn rush_ready_for_check(runner: &mut Runner, style: EffectiveStyle) {
    use crate::race::rush::{self, RushState};

    runner.next_positioning_check = 0.0;
    runner.rush_state = Some(RushState {
        section: 0,
        start_time: 0.0,
        end_time: f64::MAX,
        max_duration: rush::MAX_DURATION,
        is_rushed: true,
        checks_to_fail: 0,
        failed_checks: 0,
        style,
    });
}

#[test]
fn test_rushed_non_front_runners_pace_up_without_wit() {
    use crate::test_utils::*;

    for rushed in [false, true] {
        let (mut runner, mut state) = ready_runner(BaseStyle::Pace, seeded_config(0));
        state.elapsed_time = 10.0;
        runner.adjusted.wit = 5.0;
        if rushed {
            rush_ready_for_check(&mut runner, EffectiveStyle::Pace);
        }
        // Far behind the pacemaker.
        runner.update_positioning_non_front(30.0, Default::default(), &state);
        let paces_up = matches!(runner.positioning_strategy, PositioningStrategy::PaceUp(_));
        assert_eq!(paces_up, rushed, "{rushed}");
    }
}

#[test]
fn test_rushed_runners_positioning_as_front_enter_modes_without_wit() {
    use crate::test_utils::*;

    // (lead, strategy lead, expected mode when rushed). The original style
    // doesn't matter: the usual entry conditions apply to every rushed runner
    // positioning as a front runner.
    let cases = [
        (Some(1.0), None, PositioningStrategy::FrontSpeedUp),
        (None, None, PositioningStrategy::FrontOvertake),
        (Some(20.0), Some(1.0), PositioningStrategy::Normal),
    ];
    for style in [BaseStyle::Late, BaseStyle::Front] {
        for (lead, strategy_lead, expected) in cases {
            for rushed in [false, true] {
                let (mut runner, mut state) = ready_runner(style, seeded_config(0));
                state.elapsed_time = 10.0;
                runner.adjusted.wit = 5.0;
                if rushed {
                    rush_ready_for_check(&mut runner, EffectiveStyle::Front);
                }
                runner.update_positioning_front(
                    lead,
                    strategy_lead,
                    false,
                    Default::default(),
                    &state,
                );
                let expected = if rushed {
                    expected
                } else {
                    PositioningStrategy::Normal
                };
                assert_eq!(
                    runner.positioning_strategy, expected,
                    "{style:?} {lead:?} {strategy_lead:?} {rushed}"
                );
            }
        }
    }
}

/// Pins `passes` outcomes at `times` for position keep wit checks.
fn pin_wit_checks(runner: &mut Runner, pins: &[(f64, bool)]) {
    use crate::race::pins::PinnedWitCheck;

    let pins = pins
        .iter()
        .map(|&(time, passes)| PinnedWitCheck { time, passes });
    runner.pins.positioning = pins.collect();
}

/// Runs a position keep check at `state`'s time for a runner that would try to
/// enter a mode if she passed the wit check, returning the mode she's in.
fn run_check(runner: &mut Runner, front: bool, state: &RaceState) -> PositioningStrategy {
    if front {
        // First and close to the runner behind.
        runner.update_positioning_front(Some(1.0), None, false, Default::default(), state);
    } else {
        // Far behind the pacemaker.
        runner.update_positioning_non_front(30.0, Default::default(), state);
    }
    runner.positioning_strategy
}

#[test]
fn test_pinned_wit_check_replaces_the_roll() {
    use crate::test_utils::*;

    // (front runner, wit, pinned outcome, whether she enters a mode).
    let cases = [
        (true, 5.0, true, true),
        (true, 2000.0, false, false),
        (false, 5.0, true, true),
        (false, 2000.0, false, false),
    ];
    for (front, wit, passes, enters) in cases {
        let style = if front {
            BaseStyle::Front
        } else {
            BaseStyle::Pace
        };
        for seed in 0..10 {
            let (mut runner, mut state) = ready_runner(style, seeded_config(seed));
            state.elapsed_time = 10.0;
            runner.adjusted.wit = wit;
            // The pin is due before the check, which it fixes all the same.
            pin_wit_checks(&mut runner, &[(3.0, passes)]);
            let mode = run_check(&mut runner, front, &state);
            let entered = mode != PositioningStrategy::Normal;
            assert_eq!(entered, enters, "{front} {wit} {passes} {seed}");
        }
    }
}

#[test]
fn test_pins_are_used_once_and_not_before_their_time() {
    use crate::test_utils::*;

    let (mut runner, mut state) = ready_runner(BaseStyle::Pace, seeded_config(0));
    // She never passes the roll.
    runner.adjusted.wit = 5.0;
    pin_wit_checks(&mut runner, &[(11.0, true), (30.0, true)]);

    // A pin of a later time doesn't affect the check.
    state.elapsed_time = 10.0;
    assert_eq!(
        run_check(&mut runner, false, &state),
        PositioningStrategy::Normal
    );
    assert_eq!(runner.pinned_wit_checks_used, 0);

    // The first check at or after its time uses it...
    state.elapsed_time = 12.0;
    let mode = run_check(&mut runner, false, &state);
    assert!(matches!(mode, PositioningStrategy::PaceUp(_)));
    assert_eq!(runner.pinned_wit_checks_used, 1);

    // ...and only that one: the next check rolls (the last pin is still due
    // later).
    runner.reset_positioning(0.0);
    state.elapsed_time = 14.0;
    assert_eq!(
        run_check(&mut runner, false, &state),
        PositioningStrategy::Normal
    );
    assert_eq!(runner.pinned_wit_checks_used, 1);
}

#[test]
fn test_rushed_checks_use_no_pins() {
    use crate::test_utils::*;

    let (mut runner, mut state) = ready_runner(BaseStyle::Pace, seeded_config(0));
    state.elapsed_time = 10.0;
    rush_ready_for_check(&mut runner, EffectiveStyle::Pace);
    pin_wit_checks(&mut runner, &[(0.0, false)]);
    let mode = run_check(&mut runner, false, &state);
    assert!(matches!(mode, PositioningStrategy::PaceUp(_)));
    assert_eq!(runner.pinned_wit_checks_used, 0);
}

#[test]
fn test_pinned_outcome_leaves_later_checks_unchanged() {
    use crate::test_utils::*;

    let mut checked = 0;
    for seed in 0..40 {
        // The first check is pinned to pass or fail (drawing a threshold only
        // in the former), and the second one rolls.
        let later_modes = [true, false].map(|passes| {
            let (mut runner, mut state) = ready_runner(BaseStyle::Pace, seeded_config(seed));
            runner.adjusted.wit = 600.0;
            pin_wit_checks(&mut runner, &[(0.0, passes)]);
            state.elapsed_time = 10.0;
            run_check(&mut runner, false, &state);
            runner.reset_positioning(0.0);
            state.elapsed_time = 12.0;
            run_check(&mut runner, false, &state)
        });
        assert_eq!(later_modes[0], later_modes[1], "{seed}");
        checked += (later_modes[0] != PositioningStrategy::Normal) as u32;
    }
    // Both outcomes of the later check are covered.
    assert!(0 < checked && checked < 40, "{checked}");
}

/// Pins `distance` thresholds at `times` for position keep exits.
fn pin_thresholds(runner: &mut Runner, pins: &[(f64, f64)]) {
    use crate::race::pins::PinnedThreshold;

    let pins = pins
        .iter()
        .map(|&(time, distance)| PinnedThreshold { time, distance });
    runner.pins.positioning_thresholds = pins.collect();
}

#[test]
fn test_pinned_threshold_replaces_the_roll() {
    use crate::test_utils::*;

    // (style, distance behind the pacemaker, whether it's a Pace Up).
    let cases = [(BaseStyle::Pace, 30.0, true), (BaseStyle::Late, 0.0, false)];
    for (style, behind, up) in cases {
        let (mut runner, mut state) = ready_runner(style, seeded_config(0));
        state.elapsed_time = 10.0;
        runner.adjusted.wit = 2000.0;
        // The wit check is pinned too, so it passes.
        pin_wit_checks(&mut runner, &[(0.0, true)]);
        // The pin is due before the check, which it fixes all the same.
        pin_thresholds(&mut runner, &[(3.0, 123.0)]);
        runner.update_positioning_non_front(behind, Default::default(), &state);
        let expected = if up {
            PositioningStrategy::PaceUp(123.0)
        } else {
            PositioningStrategy::PaceDown(123.0)
        };
        assert_eq!(runner.positioning_strategy, expected, "{up}");
    }
}

#[test]
fn test_pinned_thresholds_are_used_once_and_not_before_their_time() {
    use crate::test_utils::*;

    let (mut runner, mut state) = ready_runner(BaseStyle::Pace, seeded_config(0));
    // Rushed runners pass the wit check, and still use pins for thresholds.
    rush_ready_for_check(&mut runner, EffectiveStyle::Pace);
    pin_thresholds(&mut runner, &[(11.0, 123.0), (30.0, 456.0)]);
    let mut enter = |runner: &mut Runner, time| {
        runner.reset_positioning(0.0);
        state.elapsed_time = time;
        runner.update_positioning_non_front(30.0, Default::default(), &state);
        runner.positioning_strategy
    };

    // A pin of a later time doesn't affect the threshold.
    let first = enter(&mut runner, 10.0);
    assert!(matches!(first, PositioningStrategy::PaceUp(t) if t < 100.0));
    assert_eq!(runner.pinned_thresholds_used, 0);

    // The first threshold at or after its time uses it...
    let second = enter(&mut runner, 12.0);
    assert_eq!(second, PositioningStrategy::PaceUp(123.0));
    assert_eq!(runner.pinned_thresholds_used, 1);

    // ...and only that one: the next one rolls (the last pin is still due
    // later).
    let third = enter(&mut runner, 14.0);
    assert!(matches!(third, PositioningStrategy::PaceUp(t) if t < 100.0));
    assert_eq!(runner.pinned_thresholds_used, 1);
}

#[test]
fn test_pinned_threshold_leaves_later_rolls_unchanged() {
    use crate::test_utils::*;

    for seed in 0..10 {
        // The first threshold is pinned or rolled, and the second one rolls.
        let thresholds = [true, false].map(|pinned| {
            let (mut runner, mut state) = ready_runner(BaseStyle::Pace, seeded_config(seed));
            rush_ready_for_check(&mut runner, EffectiveStyle::Pace);
            if pinned {
                pin_thresholds(&mut runner, &[(0.0, 123.0)]);
            }
            state.elapsed_time = 10.0;
            runner.update_positioning_non_front(30.0, Default::default(), &state);
            runner.reset_positioning(0.0);
            state.elapsed_time = 12.0;
            runner.update_positioning_non_front(30.0, Default::default(), &state);
            runner.positioning_strategy
        });
        assert!(
            matches!(thresholds[0], PositioningStrategy::PaceUp(t) if t < 100.0),
            "{seed}"
        );
        assert_eq!(thresholds[0], thresholds[1], "{seed}");
    }
}
