use super::*;
use crate::configuration::{Configuration, Roll};
use crate::race::solver::RaceSolver;
use crate::test_utils::{
    alternative, load_course, race_params, ready_solver, seeded_config, skill, skill_data, uma,
};

/// Build a runner with the given skills on a 2000m turf course (10104), and the
/// race state it was built with.
fn unready_runner(skills: Vec<Skill>) -> (UnreadyRunner, RaceState) {
    let state = RaceSolver::new(load_course(10104), race_params(), &seeded_config(0)).state;
    let runner = UnreadyRunner::new(uma(Style::Pace), 1, Motivation::Normal, &state, skills);
    assert!(
        runner.runtime_skills.is_empty(),
        "All skills should be out-of-gate"
    );
    (runner, state)
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
    let alternatives = vec![
        alternative("ground_type==2", SkillType::SpeedUp, 1000000),
        alternative("ground_type==1", SkillType::SpeedUp, 400000),
        alternative("always==1", SkillType::SpeedUp, 800000),
    ];
    let skill = Skill::new("speed".to_string(), skill_data(alternatives));
    let (mut runner, state) = unready_runner(vec![skill]);
    let speed = runner.adjusted.speed;

    runner.apply_out_of_gate_skills(&state);

    assert_eq!(runner.adjusted.speed, speed + 40);
    assert_eq!(activated_ids(&runner), vec!["speed"]);
}

#[test]
fn test_runtime_skills_activate_alongside_out_of_gate_skills() {
    // Out-of-gate skills listed first must not shift the skills the trigger
    // scheduler refers to.
    let skills = vec![
        skill("speed", "always==1", SkillType::SpeedUp, 400000),
        skill("delay", "always==1", SkillType::SetStartDelay, 0),
        skill("late", "distance_rate>=50", SkillType::TargetSpeed, 3500),
    ];
    let mut solver = ready_solver(10104, uma(Style::Pace), skills, &seeded_config(0));
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

/// Build a ready runner with low wisdom and the given skills, which all require
/// a wisdom check.
fn checked_runner(config: Configuration, skill_count: usize) -> Runner {
    let skills = (0..skill_count)
        .map(|i| {
            let alternative = alternative("distance_rate>=50", SkillType::TargetSpeed, 1);
            let data = SkillData {
                wisdom_check: true,
                ..skill_data(vec![alternative])
            };
            Skill::new(i.to_string(), data)
        })
        .collect();
    let uma = Uma {
        wiz: 100,
        ..uma(Style::Pace)
    };
    let mut solver = ready_solver(10104, uma, skills, &config);
    solver.runners.remove(0)
}

fn config(wisdom_checks: Roll, rushing: Roll) -> Configuration {
    Configuration {
        wisdom_checks,
        rushing,
        ..seeded_config(0)
    }
}

#[test]
fn test_wisdom_checks_can_be_forced() {
    // With 100 wisdom, each skill has a 20% chance to pass the check.
    let runner = checked_runner(config(Roll::Always, Roll::Random), 20);
    assert_eq!(runner.available_skills.len(), 20);
    let runner = checked_runner(config(Roll::Never, Roll::Random), 20);
    assert!(runner.available_skills.is_empty());
}

#[test]
fn test_rushing_can_be_forced() {
    for seed in 0..10 {
        let always = Configuration {
            seed: Some(seed),
            ..config(Roll::Random, Roll::Always)
        };
        let never = Configuration {
            seed: Some(seed),
            ..config(Roll::Random, Roll::Never)
        };
        assert!(
            checked_runner(always, 0).rush_state.is_some(),
            "seed {seed}"
        );
        assert!(checked_runner(never, 0).rush_state.is_none(), "seed {seed}");
    }
}

#[test]
fn test_forced_rolls_do_not_shift_other_random_outcomes() {
    let random = checked_runner(config(Roll::Random, Roll::Random), 20);
    let forced = checked_runner(config(Roll::Always, Roll::Random), 20);
    assert_eq!(random.start_delay, forced.start_delay);
}
