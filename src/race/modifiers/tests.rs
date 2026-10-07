use super::*;
use crate::configuration::GameVersion;
use crate::course::Phase;
use crate::race::last_spurt::*;
use crate::race::parameters::*;
use crate::race::runner::Runner;
use crate::race::solver::RaceSolver;
use crate::skill::*;
use crate::test_utils::{ready_solver, seeded_config, uma};
use crate::uma::*;

const EPSILON: f64 = 1e-9;

#[test]
fn test_wiz_limit_break_multiplier_brackets() {
    let cases = [
        (0, 0.0),
        (1200, 0.0),
        (1201, 0.02),
        (1220, 0.02),
        (1221, 0.04),
        (1325, 0.14),
        (1400, 0.2),
        (1401, 0.26),
        (1600, 0.8),
        (1601, 0.81),
        (2000, 1.0),
        (2001, 1.01),
        (2100, 1.01),
        (2101, 1.02),
        (3200, 1.12),
        (5000, 1.12),
    ];
    for (wiz, expected) in cases {
        let actual = wiz_limit_break_multiplier(wiz);
        assert!(
            (actual - expected).abs() < EPSILON,
            "wiz {wiz}: expected {expected}, got {actual}"
        );
    }
}

#[test]
fn test_wiz_limit_break_phase_coefficient() {
    use Phase::*;
    let phases = [Early, Mid, Late, Last];
    let table = [
        (Style::Front, [0.26, 0.23, 0.19, 0.16]),
        (Style::Pace, [0.21, 0.21, 0.21, 0.21]),
        (Style::Late, [0.19, 0.18, 0.23, 0.24]),
        (Style::End, [0.15, 0.17, 0.25, 0.27]),
        (Style::Runaway, [0.26, 0.23, 0.19, 0.16]),
    ];
    for (style, row) in table {
        for (phase, expected) in phases.into_iter().zip(row) {
            assert_eq!(wiz_limit_break_phase_coefficient(style, phase), expected);
        }
    }
}

fn speed_skill(rarity: SkillRarity, skill_type: SkillType) -> Skill {
    Skill::new(
        "test".to_string(),
        SkillData {
            rarity,
            alternatives: vec![SkillAlternative {
                base_duration: 30000,
                base_cooldown: DEFAULT_COOLDOWN,
                condition: "phase==3".parse().unwrap(),
                precondition: None,
                effects: vec![SkillEffect {
                    skill_type,
                    target: SkillTarget::Oneself,
                    modifier: 3500,
                }],
            }],
            popularity: None,
            wisdom_check: false,
        },
    )
}

/// Build a ready race with a single high-wiz runner that only has the given
/// skill.
fn high_wiz_race(skill: Skill) -> RaceSolver<Runner> {
    let uma = Uma {
        wiz: 1500,
        ..uma(Style::Pace)
    };
    ready_solver(10101, uma, vec![skill], &seeded_config(42))
}

/// Activate the only skill of a single runner with high wiz and return
/// (target speed modifier, current speed modifier, expected Wiz Limit Break
/// bonus rate).
fn activate_on_high_wiz_runner(skill: Skill) -> (f64, f64, f64) {
    let mut solver = high_wiz_race(skill);
    let runner = &mut solver.runners[0];

    let bonus = wiz_limit_break_phase_coefficient(runner.adjusted.style, runner.phase)
        * wiz_limit_break_multiplier(runner.adjusted.wiz);
    runner.activate_skill(0, 0, &solver.state);
    (
        runner.skill_modifiers.target_speed,
        runner.skill_modifiers.current_speed,
        bonus,
    )
}

#[test]
fn test_skill_duration_scales_with_course_distance() {
    let mut solver = high_wiz_race(speed_skill(SkillRarity::White, SkillType::TargetSpeed));
    let runner = &mut solver.runners[0];
    runner.activate_skill(0, 0, &solver.state);

    // 3 s base duration.
    let expected = 3.0 * solver.state.course.distance as f64 / 1000.0;
    let effect = &runner.active_timed_effects[0];
    let duration = effect.expires_at - solver.state.elapsed_time;
    assert!(
        (duration - expected).abs() < EPSILON,
        "expected {expected} s, got {duration} s"
    );
}

fn boosted(bonus: f64) -> f64 {
    0.35 * (1.0 + bonus)
}

#[test]
fn test_wiz_limit_break_boosts_gold_target_speed() {
    let (target_speed, _, bonus) =
        activate_on_high_wiz_runner(speed_skill(SkillRarity::Gold, SkillType::TargetSpeed));
    assert!(bonus > 0.0);
    assert!((target_speed - boosted(bonus)).abs() < EPSILON);
}

#[test]
fn test_wiz_limit_break_boosts_unique_current_speed_with_deceleration() {
    let (_, current_speed, bonus) = activate_on_high_wiz_runner(speed_skill(
        SkillRarity::UniqueNatural,
        SkillType::CurrentSpeedWithNaturalDeceleration,
    ));
    assert!((current_speed - boosted(bonus)).abs() < EPSILON);
}

#[test]
fn test_wiz_limit_break_ignores_white_skills() {
    let (target_speed, _, _) =
        activate_on_high_wiz_runner(speed_skill(SkillRarity::White, SkillType::TargetSpeed));
    assert!((target_speed - 0.35).abs() < EPSILON);
}

#[test]
fn test_wiz_limit_break_ignores_plain_current_speed() {
    let (_, current_speed, _) =
        activate_on_high_wiz_runner(speed_skill(SkillRarity::Gold, SkillType::CurrentSpeed));
    assert!((current_speed - 0.35).abs() < EPSILON);
}

#[test]
fn test_stamina_limit_break_distance_factor() {
    for distance in (1000..=3600).step_by(100) {
        let expected = match distance {
            ..=2100 => 0.0,
            2200 => 0.5,
            2300..=2400 => 1.0,
            2500..=2600 => 1.5,
            _ => 1.8,
        };
        assert_eq!(
            stamina_limit_break_distance_factor(distance, GameVersion::LATEST),
            expected,
            "distance {distance}"
        );
    }
}

#[test]
fn test_stamina_limit_break_base_buff() {
    let buff =
        |stamina, distance| stamina_limit_break_base_buff(stamina, distance, GameVersion::LATEST);
    assert_eq!(buff(1000, 2400), 0.0);
    assert_eq!(buff(1200, 2400), 0.0);
    assert_eq!(buff(1600, 2100), 0.0);
    assert!((buff(1300, 2400) - 10.0 * 0.0085).abs() < EPSILON);
}

/// Build a single high-stamina runner on a long course, in its last spurt with
/// the given current speed and a last spurt speed of 20 m/s.
fn run_stamina_limit_break_runner(speed: f64) -> (f64, f64) {
    let uma = Uma {
        stamina: 1600,
        ..uma(Style::Pace)
    };
    let mut solver = ready_solver(10105, uma, vec![], &seeded_config(42));
    let runner = &mut solver.runners[0];

    runner.position = 2000.0;
    runner.speed = speed;
    runner.last_spurt = Some(LastSpurt {
        max_speed: 20.0,
        target_distance: 1950.0,
        category: LastSpurtCategory::Full,
    });
    runner.update_target_speed(&solver.state);
    let buff = runner.stamina_limit_break;

    // Buff must persist even if the last spurt is gone (e.g. HP recalculation).
    runner.last_spurt = None;
    runner.update_target_speed(&solver.state);
    assert_eq!(runner.stamina_limit_break, buff);

    (buff, runner.stamina_limit_break_base)
}

#[test]
fn test_stamina_limit_break_activates_at_last_spurt_speed() {
    let (buff, base) = run_stamina_limit_break_runner(20.0);
    assert!(base > 0.0);
    assert_eq!(buff, base);
}

#[test]
fn test_stamina_limit_break_waits_for_last_spurt_speed() {
    let (buff, _) = run_stamina_limit_break_runner(19.9);
    assert_eq!(buff, 0.0);
}

#[test]
fn test_aptitude_multipliers() {
    // Test that aptitude multipliers are ordered correctly.
    let s_coef = strategy_proficiency_multiplier(Aptitude::S);
    let a_coef = strategy_proficiency_multiplier(Aptitude::A);
    let b_coef = strategy_proficiency_multiplier(Aptitude::B);
    let c_coef = strategy_proficiency_multiplier(Aptitude::C);

    assert!(s_coef > a_coef);
    assert!(a_coef > b_coef);
    assert!(b_coef > c_coef);
}

#[test]
fn test_track_condition_modifiers() {
    // Test that track conditions apply appropriate modifiers.
    let heavy_modifier = track_speed_modifier(TrackCondition::Heavy);
    let firm_modifier = track_speed_modifier(TrackCondition::Firm);

    // Heavy track should have more penalty than firm.
    assert!(heavy_modifier > firm_modifier);
}
