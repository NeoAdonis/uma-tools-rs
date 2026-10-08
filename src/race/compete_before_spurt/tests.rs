use super::*;
use crate::configuration::{Configuration, GameVersion};
use crate::race::runner::Runner;
use crate::race::solver::RaceState;
use crate::skill::{SkillEffect, SkillTarget, SkillType};
use crate::test_utils::{assert_close, ready_solver, seeded_config, uma};
use crate::uma::*;

#[test]
fn test_lead_threshold_table() {
    use Style::*;
    // Distance term is zero at -1000m, so the coefficient is returned as-is.
    let cases = [
        (Runaway, 0.0),
        (Front, 0.0),
        (Pace, 2.5),
        (Late, 5.0),
        (End, 10.0),
    ];
    for (style, expected) in cases {
        let actual = lead_threshold(style, -1000.0);
        assert_close!(actual, expected, "{style:?}");
    }
}

#[test]
fn test_lead_threshold_distance_term() {
    let threshold = lead_threshold(Style::Late, 2000.0);
    assert_close!(threshold, 5.9);
}

#[test]
fn test_target_speed_bonus() {
    // Power 1500, guts 3000 -> (1 * 2 + 1) * 0.1.
    let base = 0.3;
    let cases = [
        (Style::Runaway, false, base * 0.2),
        (Style::Runaway, true, base * 0.2 * 2.0),
        (Style::Front, false, base * 0.8),
        (Style::Front, true, base * 0.8 * 1.1),
        (Style::Pace, false, base),
        (Style::Pace, true, base),
        (Style::Late, true, base),
        (Style::End, true, base),
    ];
    for (style, is_alone, expected) in cases {
        let actual = target_speed_bonus(style, 1500, 3000, is_alone);
        assert_close!(actual, expected, "{style:?} (alone: {is_alone})");
    }

    // Power 375 -> sqrt(0.25) * 2 = 1; guts 3000 * 2^-5 -> 0.5.
    let actual = target_speed_bonus(Style::Pace, 375, 3000 / 32, false);
    assert_close!(actual, 1.5 * 0.1, tolerance = 1e-3);
}

#[test]
fn test_hp_cost() {
    let cases = [
        (Style::Runaway, 1.5),
        (Style::Front, 1.2),
        (Style::Pace, 1.0),
        (Style::Late, 1.0),
        (Style::End, 1.0),
    ];
    for (style, coef) in cases {
        // 2400m -> distance coefficient 1.0.
        let far = hp_cost(style, 2400, true);
        assert_close!(far, 20.0 * coef, "{style:?} far");
        let near = hp_cost(style, 2400, false);
        assert_close!(near, 20.0 * (coef + NEAR_FACTOR), "{style:?} near");
    }

    // 1200m -> distance coefficient 0.3.
    let actual = hp_cost(Style::Front, 1200, false);
    assert_close!(actual, 20.0 * (1.2 * 0.3 + 0.5));
}

#[test]
fn test_stamina_keep_chance() {
    let expected = 0.3 * (1.0 + 1000f64.powf(0.03));
    assert_close!(stamina_keep_chance(1000), expected);
    assert!(stamina_keep_chance(300) < stamina_keep_chance(1000));
    assert_close!(stamina_keep_chance(2500), 1.0);
}

fn ready_runner(style: Style, game_version: GameVersion) -> (Runner, RaceState) {
    let config = Configuration {
        game_version,
        ..seeded_config(42)
    };
    let solver = ready_solver(10101, uma(style), vec![], &config);
    let runner = solver.runners.into_iter().next().unwrap();
    (runner, solver.state)
}

#[test]
fn test_compete_check_window() {
    let (mut runner, mut state) = ready_runner(Style::End, GameVersion::LATEST);
    state.elapsed_time = 100.0;
    for section in 0..24 {
        runner.section = section;
        assert_eq!(
            runner.is_compete_check_due(&state),
            SECTIONS.contains(&section),
            "section {section}"
        );
    }

    let (mut old_runner, mut state) = ready_runner(Style::Pace, GameVersion::SecondAnniversary);
    state.elapsed_time = 100.0;
    old_runner.section = 12;
    assert!(!old_runner.is_compete_check_due(&state));
    assert_eq!(old_runner.stamina_keep_ratio, 0.0);
}

#[test]
fn test_stamina_keep_ratio_range() {
    let (runner, _) = ready_runner(Style::Pace, GameVersion::LATEST);
    assert!(STAMINA_KEEP_RATIO.contains(&runner.stamina_keep_ratio));
}

#[test]
fn test_compete_activation_and_expiry() {
    let (mut runner, mut state) = ready_runner(Style::Front, GameVersion::LATEST);
    runner.section = 12;
    // Never enter Stamina Keep.
    runner.stamina_keep_ratio = 0.0;

    // Neither far from lead nor nearby runners.
    state.elapsed_time = 10.0;
    runner.try_compete_before_spurt(false, true, &state);
    assert!(runner.compete_before_spurt.is_none());
    assert_close!(runner.next_compete_check, 10.0 + CHECK_INTERVAL);

    // Far from lead.
    let hp_before = runner.hp.current;
    state.elapsed_time = 12.0;
    runner.try_compete_before_spurt(true, true, &state);
    let compete = runner.compete_before_spurt.as_ref().unwrap();
    let expected_modifier = target_speed_bonus(
        Style::Front,
        runner.adjusted.power,
        runner.adjusted.guts,
        true,
    );
    assert_close!(compete.modifier, expected_modifier);
    assert_close!(compete.expires_at, 12.0 + DURATION);
    let expected_hp = hp_before - hp_cost(Style::Front, state.course.distance, true);
    assert_close!(runner.hp.current, expected_hp);
    assert_close!(runner.next_compete_check, 12.0 + DURATION + COOLDOWN);

    // Boost is applied to target speed.
    runner.update_target_speed(&state);
    let boosted = runner.target_speed;
    runner.expire_compete_before_spurt(12.0 + DURATION - 0.01);
    assert!(runner.compete_before_spurt.is_some());
    runner.expire_compete_before_spurt(12.0 + DURATION);
    assert!(runner.compete_before_spurt.is_none());
    runner.update_target_speed(&state);
    assert_close!(boosted - runner.target_speed, expected_modifier);

    // Nearby runners only: near factor is added to the HP cost.
    let hp_before = runner.hp.current;
    runner.nearby.count = 1;
    state.elapsed_time = 20.0;
    runner.try_compete_before_spurt(false, false, &state);
    assert!(runner.compete_before_spurt.is_some());
    let expected_hp = hp_before - hp_cost(Style::Front, state.course.distance, false);
    assert_close!(runner.hp.current, expected_hp);
}

#[test]
fn test_stamina_keep_blocks_compete_until_recovery() {
    let (mut runner, mut state) = ready_runner(Style::Late, GameVersion::LATEST);
    runner.section = 12;
    runner.position = 700.0;
    runner.hp.current = 1.0;

    // Low HP is eventually noticed, and competing is blocked from then on.
    let mut elapsed = 0.0;
    while !runner.is_stamina_keep {
        elapsed += CHECK_INTERVAL;
        state.elapsed_time = elapsed;
        runner.compete_before_spurt = None;
        runner.try_compete_before_spurt(true, true, &state);
        assert!(elapsed < 1000.0, "stamina keep never triggered");
    }
    assert!(runner.compete_before_spurt.is_none());
    state.elapsed_time = elapsed + CHECK_INTERVAL;
    runner.try_compete_before_spurt(true, true, &state);
    assert!(runner.compete_before_spurt.is_none());

    // Recovering enough HP resets Stamina Keep and allows competing again.
    let recovery = SkillEffect {
        skill_type: SkillType::Recovery,
        target: SkillTarget::Oneself,
        modifier: 1.0,
    };
    runner.apply_skill_effect(&recovery, 0.0, 0.0, &state);
    assert!(!runner.is_stamina_keep);
    state.elapsed_time = elapsed + 2.0 * CHECK_INTERVAL;
    runner.try_compete_before_spurt(true, true, &state);
    assert!(!runner.is_stamina_keep);
    assert!(runner.compete_before_spurt.is_some());
}
