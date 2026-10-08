//! Runtime tests for skill cooldowns: once a skill alternative activates, none of
//! its OR branches can activate again until the cooldown, scaled by the course
//! distance, is over.

use crate::common::{load_course, race_params, seeded_config};
use uma_tools::race::runner::Runner;
use uma_tools::race::solver::RaceSolver;
use uma_tools::skill::*;
use uma_tools::uma::*;

const COURSE_ID: u32 = 10104;

/// Build a skill with the given condition and base cooldown, w/o wisdom check.
fn skill(condition: &str, base_cooldown: f64) -> Skill {
    Skill::new(
        "999999".to_string(),
        SkillData {
            rarity: SkillRarity::White,
            alternatives: vec![SkillAlternative {
                base_duration: 1.0,
                base_cooldown,
                condition: condition.parse().unwrap(),
                effects: vec![SkillEffect {
                    skill_type: SkillType::TargetSpeed,
                    target: SkillTarget::Oneself,
                    modifier: 0.0001,
                }],
                precondition: None,
            }],
            popularity: None,
            wisdom_check: false,
        },
    )
}

/// Run a single-runner race and return the finished runner.
fn run(condition: &str, base_cooldown: f64) -> Runner {
    let uma = Uma {
        speed: 1200,
        stamina: 1200,
        power: 1000,
        guts: 800,
        wiz: 1000,
        style: Style::Pace,
        distance_aptitude: Aptitude::A,
        track_aptitude: Aptitude::A,
        style_aptitude: Aptitude::A,
    };
    let mut solver = RaceSolver::new(load_course(COURSE_ID), race_params(), &seeded_config(0));
    solver.add_runner(
        uma,
        Motivation::Normal,
        vec![skill(condition, base_cooldown)],
    );
    let mut solver = solver.ready_runners();
    solver.run();
    solver.runners.swap_remove(0)
}

#[test]
fn test_default_cooldown_activates_once() {
    // Always true, checked every frame.
    let runner = run("hp_per<=100", DEFAULT_COOLDOWN);
    assert_eq!(runner.activated_skills.len(), 1);
}

#[test]
fn test_short_cooldown_reactivates() {
    let base_cooldown = 5.0;
    let runner = run("hp_per<=100", base_cooldown);

    // Scaled by the course distance.
    let distance = load_course(COURSE_ID).distance as f64;
    let cooldown = base_cooldown * distance / 1000.0;
    let finish_time = runner.finish_time.expect("runner should finish");
    let expected = (finish_time / cooldown).floor() as usize;

    let count = runner.activated_skills.len();
    assert!(
        (expected.saturating_sub(1)..=expected + 1).contains(&count),
        "expected about {expected} activations ({cooldown} s cooldown over {finish_time} s), got {count}"
    );

    // Activations are spread along the race, never in the same frame.
    let positions: Vec<f64> = runner
        .activated_skills
        .iter()
        .map(|record| record.distance)
        .collect();
    assert!(
        positions.windows(2).all(|pair| pair[1] > pair[0]),
        "positions should strictly increase, got {positions:?}"
    );
}

#[test]
fn test_or_branches_share_cooldown() {
    // Both branches are true for the whole race.
    let runner = run("hp_per<=100@distance_rate>=0", DEFAULT_COOLDOWN);
    assert_eq!(runner.activated_skills.len(), 1);
}
