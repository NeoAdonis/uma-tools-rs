use super::super::modifiers::*;
use crate::configuration::{GAME_BEHAVIOR, GameBehavior};
use crate::course::{Course, Phase};
use crate::race::parameters::*;
use crate::race::solver::RaceSolver;
use crate::uma::skill::*;
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
                condition: String::new(),
                precondition: String::new(),
                effects: vec![SkillEffect {
                    skill_type,
                    target: SkillTarget::Oneself,
                    modifier: 3500,
                }],
            }],
            wisdom_check: false,
        },
    )
}

/// Activate the only skill of a single runner with high wiz and return
/// (target speed modifier, current speed modifier, expected Wiz Limit Break bonus rate).
fn activate_on_high_wiz_runner(skill: Skill) -> (f64, f64, f64) {
    let course = Course::from_json_file("data/course_data.json", "10101")
        .expect("Failed to load course data");
    let params = RaceParameters {
        track_condition: TrackCondition::Firm,
        weather: Weather::Sunny,
        season: Season::Spring,
        time: Time::None,
        grade: Grade::G1,
        gates: 9,
        rng_seed: 42,
    };
    let uma = Uma {
        speed: 1200,
        stamina: 1000,
        power: 1000,
        guts: 800,
        wiz: 1500,
        style: Style::Pace,
        distance_aptitude: Aptitude::A,
        track_aptitude: Aptitude::A,
        style_aptitude: Aptitude::A,
    };

    let mut solver = RaceSolver::new(course, params);
    solver.add_runner(uma, Motivation::Normal, vec![skill]);
    let mut solver = solver.ready_runners();
    let runner = &mut solver.runners[0];

    let bonus = wiz_limit_break_phase_coefficient(runner.adjusted.style, runner.phase)
        * wiz_limit_break_multiplier(runner.adjusted.wiz);
    runner.activate_skill(0, 0, 0.0, &solver.state.course, TrackCondition::Firm);
    (
        runner.skill_modifiers.target_speed,
        runner.skill_modifiers.current_speed,
        bonus,
    )
}

fn boosted(bonus: f64) -> f64 {
    if GAME_BEHAVIOR >= GameBehavior::FifthAnniversary {
        0.35 * (1.0 + bonus)
    } else {
        0.35
    }
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
