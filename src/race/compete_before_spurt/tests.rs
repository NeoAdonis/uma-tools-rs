use super::*;
use crate::configuration::{Configuration, GameVersion};
use crate::race::timed_boost::TimedBoost;
use crate::skill::SkillType;
use crate::test_utils::{assert_close, effect, ready_runner, seeded_config};
use crate::trigger::skill_activation::OvertakeExtension;
use crate::uma::*;

#[test]
fn test_lead_threshold() {
    use EffectiveStyle::*;
    // At -1000m, the distance term is zero.
    let cases = [
        (Front, -1000.0, 0.0),
        (End, -1000.0, 10.0),
        (Late, 2000.0, 5.9),
    ];
    for (style, distance, expected) in cases {
        let actual = lead_threshold(style, distance);
        assert_close!(actual, expected, "{style:?}, {distance}m");
    }
}

#[test]
fn test_target_speed_bonus() {
    // Power 1500, guts 3000 -> (1 * 2 + 1) * 0.1.
    let base = 0.3;
    let cases = [
        (EffectiveStyle::Runaway, true, base * 0.2 * 2.0),
        (EffectiveStyle::Front, false, base * 0.8),
        (EffectiveStyle::Front, true, base * 0.8 * 1.1),
        (EffectiveStyle::Pace, true, base),
    ];
    for (style, is_alone, expected) in cases {
        let actual = target_speed_bonus(style, 1500.0, 3000.0, is_alone);
        assert_close!(actual, expected, "{style:?} (alone: {is_alone})");
    }

    // Power 375 -> sqrt(0.25) * 2 = 1; guts 3000 * 2^-5 -> 0.5.
    let actual = target_speed_bonus(EffectiveStyle::Pace, 375.0, 3000.0 / 32.0, false);
    assert_close!(actual, 1.5 * 0.1, tolerance = 1e-3);
}

#[test]
fn test_hp_cost() {
    // 2400m -> distance coefficient 1.0; 1200m -> 0.3.
    let cases = [
        (EffectiveStyle::Runaway, 2400, true, 20.0 * 1.5),
        (
            EffectiveStyle::Pace,
            2400,
            false,
            20.0 * (1.0 + NEAR_FACTOR),
        ),
        (
            EffectiveStyle::Front,
            1200,
            false,
            20.0 * (1.2 * 0.3 + NEAR_FACTOR),
        ),
    ];
    for (style, distance, far_from_lead, expected) in cases {
        let actual = hp_cost(style, distance, far_from_lead);
        assert_close!(
            actual,
            expected,
            "{style:?}, {distance}m (far: {far_from_lead})"
        );
    }
}

#[test]
fn test_stamina_keep_chance() {
    let expected = 0.3 * (1.0 + 1000f64.powf(0.03));
    assert_close!(stamina_keep_chance(1000.0), expected);
    assert_close!(stamina_keep_chance(2500.0), 1.0);
}

#[test]
fn test_compete_check_window() {
    let (mut runner, mut state) = ready_runner(EffectiveStyle::End, seeded_config(42));
    state.elapsed_time = 100.0;
    for (section, expected) in [(9, false), (10, true), (14, true), (15, false)] {
        runner.section = section;
        let actual = runner.is_compete_check_due(&state);
        assert_eq!(actual, expected, "section {section}");
    }

    let config = Configuration {
        game_version: GameVersion::SecondAnniversary,
        ..seeded_config(42)
    };
    let (mut old_runner, mut state) = ready_runner(EffectiveStyle::Pace, config);
    state.elapsed_time = 100.0;
    old_runner.section = 12;
    assert!(!old_runner.is_compete_check_due(&state));
    assert_eq!(old_runner.stamina_keep.ratio, 0.0);
}

#[test]
fn test_compete_activation_and_expiry() {
    let (mut runner, mut state) = ready_runner(EffectiveStyle::Front, seeded_config(42));
    runner.section = 12;
    // Never enter stamina keep.
    runner.stamina_keep.ratio = 0.0;
    // ? Competing has no chance for now (its formula is unknown), so it starts
    // on the first check.

    // Neither far from lead nor nearby runners.
    let (far_from_lead, is_alone) = (false, true);
    state.elapsed_time = 10.0;
    runner.try_compete_before_spurt(far_from_lead, is_alone, &state);
    let next_check = 10.0 + CHECK_INTERVAL;
    assert_eq!(runner.compete_before_spurt, TimedBoost::Idle { next_check });

    // Far from lead.
    let far_from_lead = true;
    let hp_before = runner.hp.current();
    state.elapsed_time = 12.0;
    runner.try_compete_before_spurt(far_from_lead, is_alone, &state);
    let expected_modifier = target_speed_bonus(
        EffectiveStyle::Front,
        runner.adjusted.power,
        runner.adjusted.guts,
        is_alone,
    );
    let expected_boost = TimedBoost::start(expected_modifier, 12.0, DURATION, COOLDOWN);
    assert_eq!(runner.compete_before_spurt, expected_boost);
    let expected_hp =
        hp_before - hp_cost(EffectiveStyle::Front, state.course.distance, far_from_lead);
    assert_close!(runner.hp.current(), expected_hp);

    // The boost ends after its duration.
    runner.compete_before_spurt.expire(12.0 + DURATION);
    assert!(matches!(
        runner.compete_before_spurt,
        TimedBoost::Idle { .. }
    ));

    // Nearby runners only: near factor is added to the HP cost.
    let (far_from_lead, is_alone) = (false, false);
    let hp_before = runner.hp.current();
    runner.nearby.count = 1;
    state.elapsed_time = 20.0;
    runner.try_compete_before_spurt(far_from_lead, is_alone, &state);
    assert!(matches!(
        runner.compete_before_spurt,
        TimedBoost::Active { .. }
    ));
    let expected_hp =
        hp_before - hp_cost(EffectiveStyle::Front, state.course.distance, far_from_lead);
    assert_close!(runner.hp.current(), expected_hp);
}

#[test]
fn test_stamina_keep_starts_below_the_hp_to_keep() {
    // ? The HP required to finish is assumed (see `hp_required_to_finish`).
    // (HP over the HP to keep, whether she enters stamina keep).
    for (offset, enters) in [(-1.0, true), (1.0, false)] {
        let (mut runner, mut state) = ready_runner(EffectiveStyle::Late, seeded_config(42));
        runner.section = 12;
        runner.position = 700.0;
        let to_keep = runner.hp_required_to_finish(&state) * runner.stamina_keep.ratio;
        runner.hp.set_current(to_keep + offset);
        // Neither far from the lead nor with runners nearby, so she never
        // competes (which would consume HP). Many checks, so that the wit
        // roll eventually passes.
        let (far_from_lead, is_alone) = (false, true);
        for check in 1..=50 {
            state.elapsed_time = check as f64 * CHECK_INTERVAL;
            runner.compete_before_spurt = TimedBoost::Idle { next_check: 0.0 };
            runner.try_compete_before_spurt(far_from_lead, is_alone, &state);
        }
        assert_eq!(runner.stamina_keep.is_active, enters, "{offset} HP over");
    }
}

#[test]
fn test_stamina_keep_blocks_compete_until_recovery() {
    let (mut runner, mut state) = ready_runner(EffectiveStyle::Late, seeded_config(42));
    runner.section = 12;
    runner.position = 700.0;
    runner.hp.set_current(1.0);
    let (far_from_lead, is_alone) = (true, true);

    // Low HP is eventually noticed, and competing is blocked from then on.
    let mut elapsed = 0.0;
    let idle = TimedBoost::Idle { next_check: 0.0 };
    while !runner.stamina_keep.is_active {
        elapsed += CHECK_INTERVAL;
        state.elapsed_time = elapsed;
        runner.compete_before_spurt = idle;
        runner.try_compete_before_spurt(far_from_lead, is_alone, &state);
        assert!(elapsed < 1000.0, "stamina keep never triggered");
    }
    assert!(matches!(
        runner.compete_before_spurt,
        TimedBoost::Idle { .. }
    ));
    state.elapsed_time = elapsed + CHECK_INTERVAL;
    runner.try_compete_before_spurt(far_from_lead, is_alone, &state);
    assert!(matches!(
        runner.compete_before_spurt,
        TimedBoost::Idle { .. }
    ));

    // Recovering enough HP resets stamina keep and allows competing again.
    let recovery = effect(SkillType::Recovery, 1.0);
    runner.apply_skill_effect(
        &recovery,
        0.0,
        OvertakeExtension::default(),
        0.0,
        None,
        &state,
    );
    assert!(!runner.stamina_keep.is_active);
    state.elapsed_time = elapsed + 2.0 * CHECK_INTERVAL;
    runner.try_compete_before_spurt(far_from_lead, is_alone, &state);
    assert!(!runner.stamina_keep.is_active);
    assert!(matches!(
        runner.compete_before_spurt,
        TimedBoost::Active { .. }
    ));
}
