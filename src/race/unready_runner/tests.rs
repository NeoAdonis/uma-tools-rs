use super::*;
use crate::configuration::Configuration;
use crate::race::solver::RaceSolver;
use crate::test_utils::race_params;

/// Build a skill with one `(condition, effect type, modifier)` per alternative,
/// without wisdom check so it is always available.
fn skill_with_alternatives(id: &str, alternatives: Vec<(&str, SkillType, i32)>) -> Skill {
    Skill::new(
        id.to_string(),
        SkillData {
            rarity: SkillRarity::White,
            alternatives: alternatives
                .into_iter()
                .map(|(condition, skill_type, modifier)| SkillAlternative {
                    base_duration: 0,
                    base_cooldown: DEFAULT_COOLDOWN,
                    condition: condition.to_string(),
                    effects: vec![SkillEffect {
                        skill_type,
                        target: SkillTarget::Oneself,
                        modifier,
                    }],
                    precondition: String::new(),
                })
                .collect(),
            popularity: None,
            wisdom_check: false,
        },
    )
}

/// Build a skill with a single alternative and effect.
fn skill(id: &str, condition: &str, skill_type: SkillType, modifier: i32) -> Skill {
    skill_with_alternatives(id, vec![(condition, skill_type, modifier)])
}

/// Build a runner with the given skills on a 2000m turf course (10104), and the
/// race state it was built with.
fn unready_runner(skills: Vec<Skill>) -> (UnreadyRunner, RaceState) {
    let course = Course::from_json_file("data/course_data.json", "10104")
        .expect("Failed to load course data");
    let state = RaceSolver::new(course, race_params(0), &Configuration::default()).state;
    let runner = UnreadyRunner::new(uma(), 1, Motivation::Normal, &state, skills);
    assert!(
        runner.runtime_skills.is_empty(),
        "All skills should be out-of-gate"
    );
    (runner, state)
}

fn uma() -> Uma {
    Uma {
        speed: 1200,
        stamina: 1000,
        power: 1000,
        guts: 800,
        wiz: 1000,
        style: Style::Pace,
        distance_aptitude: Aptitude::A,
        track_aptitude: Aptitude::A,
        style_aptitude: Aptitude::A,
    }
}

/// IDs of the skills recorded as activated.
fn activated_ids(runner: &UnreadyRunner) -> Vec<&str> {
    runner
        .activated_skills
        .iter()
        .map(|record| record.skill_id.as_str())
        .collect()
}

#[test]
fn test_stat_boost_is_applied() {
    // Green skills store stat boosts in 10000ths (e.g. 800000 = +80 speed).
    let (mut runner, state) = unready_runner(vec![skill(
        "speed",
        "always==1",
        SkillType::SpeedUp,
        800000,
    )]);
    let speed = runner.adjusted.speed;

    runner.apply_out_of_gate_skills(&state);

    assert_eq!(runner.adjusted.speed, speed + 80);
    assert_eq!(activated_ids(&runner), vec!["speed"]);
}

#[test]
fn test_start_delay_is_set() {
    let (mut runner, state) = unready_runner(vec![skill(
        "delay",
        "always==1",
        SkillType::SetStartDelay,
        0,
    )]);

    runner.apply_out_of_gate_skills(&state);

    assert_eq!(runner.start_delay, 0.0);
    assert_eq!(activated_ids(&runner), vec!["delay"]);
}

#[test]
fn test_every_out_of_gate_skill_is_applied() {
    let (mut runner, state) = unready_runner(vec![
        skill("delay", "always==1", SkillType::SetStartDelay, 0),
        skill("rush", "always==1", SkillType::RushChanceUp, -30000),
    ]);

    runner.apply_out_of_gate_skills(&state);

    assert_eq!(runner.start_delay, 0.0);
    assert_eq!(runner.rush_chance_modifier, -3.0);
    assert_eq!(activated_ids(&runner), vec!["delay", "rush"]);
}

#[test]
fn test_unsatisfied_condition_is_not_applied() {
    // Course 10104 is turf (ground_type==1).
    let (mut runner, state) = unready_runner(vec![skill(
        "dirt_delay",
        "ground_type==2",
        SkillType::SetStartDelay,
        0,
    )]);
    let start_delay = runner.start_delay;

    runner.apply_out_of_gate_skills(&state);

    assert_eq!(runner.start_delay, start_delay);
    assert!(activated_ids(&runner).is_empty());
}

#[test]
fn test_unsatisfied_stat_boost_is_not_applied() {
    // Course 10104 is turf (ground_type==1).
    let (mut runner, state) = unready_runner(vec![skill(
        "dirt_speed",
        "ground_type==2",
        SkillType::SpeedUp,
        800000,
    )]);
    let speed = runner.adjusted.speed;

    runner.apply_out_of_gate_skills(&state);

    assert_eq!(runner.adjusted.speed, speed);
    assert!(activated_ids(&runner).is_empty());
}

#[test]
fn test_first_satisfied_alternative_is_applied() {
    // Course 10104 is turf (ground_type==1); only the second alternative holds, and
    // the third one must not be applied on top of it.
    let (mut runner, state) = unready_runner(vec![skill_with_alternatives(
        "speed",
        vec![
            ("ground_type==2", SkillType::SpeedUp, 1000000),
            ("ground_type==1", SkillType::SpeedUp, 400000),
            ("always==1", SkillType::SpeedUp, 800000),
        ],
    )]);
    let speed = runner.adjusted.speed;

    runner.apply_out_of_gate_skills(&state);

    assert_eq!(runner.adjusted.speed, speed + 40);
    assert_eq!(activated_ids(&runner), vec!["speed"]);
}

#[test]
fn test_runtime_skills_activate_alongside_out_of_gate_skills() {
    // Out-of-gate skills listed first must not shift the skills the trigger
    // scheduler refers to.
    let course = Course::from_json_file("data/course_data.json", "10104")
        .expect("Failed to load course data");
    let mut solver = RaceSolver::new(course, race_params(0), &Configuration::default());
    solver.add_runner(
        uma(),
        Motivation::Normal,
        vec![
            skill("speed", "always==1", SkillType::SpeedUp, 400000),
            skill("delay", "always==1", SkillType::SetStartDelay, 0),
            skill("late", "distance_rate>=50", SkillType::TargetSpeed, 3500),
        ],
    );
    let mut solver = solver.ready_runners();
    solver.run();

    let activations: Vec<_> = solver.runners[0]
        .activated_skills
        .iter()
        .map(|record| (record.skill_id.as_str(), record.distance))
        .collect();
    assert_eq!(activations.len(), 3, "{activations:?}");
    assert_eq!(&activations[..2], &[("speed", 0.0), ("delay", 0.0)]);
    assert_eq!(activations[2].0, "late");
    assert!(activations[2].1 >= 1000.0, "{activations:?}");
}
