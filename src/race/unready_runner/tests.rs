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
        runner.skills.iter().all(Skill::should_activate_out_of_gate),
        "All skills should be out-of-gate"
    );
    (runner, state)
}

/// IDs of the skills that activate on the first frame the runner moves.
fn race_start_ids<'a>(skills: &'a [Skill], scheduler: &TriggerScheduler) -> Vec<&'a str> {
    scheduler
        .race_start_triggers
        .iter()
        .map(|&logic_idx| {
            skills[scheduler.skill_logics[logic_idx].skill_index]
                .id
                .as_str()
        })
        .collect()
}

/// IDs of the out-of-gate skills activated before the race.
fn activated_ids(runner: &UnreadyRunner) -> Vec<&str> {
    race_start_ids(&runner.skills, &runner.trigger_scheduler)
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

#[test]
fn test_out_of_gate_skills_are_counted_as_activated() {
    let speed = skill("speed", "always==1", SkillType::SpeedUp, 800000);
    let mut solver = ready_solver(10104, uma(Style::Pace), vec![speed], &seeded_config(0));
    solver.run();

    let counts = &solver.runners[0].trigger_scheduler.activation_counts;
    assert_eq!(counts.all, 1);
    assert_eq!(counts.start, 1);
}

const CONNECT_ID: &str = "201632";
const METI_RESONANCE_ID: &str = "110502211";

/// Run the out-of-gate step of a race on `course_id` with one runner per skill
/// set, using the skills in `data/skill_data.json`.
///
/// Runners are returned in the order of their skill sets.
fn sympathy_runners(course_id: u32, skill_sets: &[&[&str]]) -> Vec<Runner> {
    let skills = Skill::load_skills_from_file("data/skill_data.json").unwrap();
    let mut solver = RaceSolver::new(load_course(course_id), race_params(), &seeded_config(0));
    for (i, ids) in skill_sets.iter().enumerate() {
        // Guts doesn't affect speed, so it tells runners apart.
        let uma = Uma {
            guts: 800 + i as u16,
            ..uma(Style::Pace)
        };
        let skills = ids.iter().map(|id| skills[*id].clone()).collect();
        solver.add_runner(uma, Motivation::Normal, skills);
    }
    let mut runners = solver.ready_runners().runners;
    runners.sort_by_key(|runner| runner.base.guts);
    runners
}

/// Number of times each runner activated "Sympathy" before the race.
fn sympathy_activations(runners: &[Runner]) -> Vec<usize> {
    runners
        .iter()
        .map(|runner| {
            race_start_ids(&runner.available_skills, &runner.trigger_scheduler)
                .iter()
                .filter(|&&id| id == SYMPATHY_ID)
                .count()
        })
        .collect()
}

#[test]
fn test_sympathy_needs_five_runners_with_it() {
    let runners = sympathy_runners(10104, &[&[SYMPATHY_ID] as &[_]; 4]);
    assert_eq!(sympathy_activations(&runners), vec![0; 4]);

    let runners = sympathy_runners(10104, &[&[SYMPATHY_ID] as &[_]; 5]);
    assert_eq!(sympathy_activations(&runners), vec![1; 5]);
}

#[test]
fn test_connect_activates_sympathy_for_every_runner_with_it() {
    // Course 10104 is turf, as required by "Connect".
    let runners = sympathy_runners(10104, &[&[CONNECT_ID], &[SYMPATHY_ID], &[], &[SYMPATHY_ID]]);
    assert_eq!(sympathy_activations(&runners), vec![0, 1, 0, 1]);

    // "Connect" gives +60 speed and "Sympathy" +40 speed.
    let speed = runners[2].adjusted.speed;
    assert_eq!(runners[0].adjusted.speed, speed + 60);
    assert_eq!(runners[1].adjusted.speed, speed + 40);
    assert_eq!(runners[3].adjusted.speed, speed + 40);
}

#[test]
fn test_connect_activates_own_sympathy() {
    let runners = sympathy_runners(10104, &[&[CONNECT_ID, SYMPATHY_ID], &[]]);
    assert_eq!(sympathy_activations(&runners), vec![1, 0]);
    assert_eq!(runners[0].adjusted.speed, runners[1].adjusted.speed + 100);
}

#[test]
fn test_meti_resonance_activates_sympathy() {
    // Course 10104 is a Medium turf course, as required by "METI" Resonance.
    let runners = sympathy_runners(10104, &[&[METI_RESONANCE_ID], &[SYMPATHY_ID]]);
    assert_eq!(sympathy_activations(&runners), vec![0, 1]);
}

#[test]
fn test_sympathy_is_not_activated_if_connect_isnt() {
    // Course 10913 is dirt, so "Connect" doesn't activate.
    let runners = sympathy_runners(10913, &[&[CONNECT_ID], &[SYMPATHY_ID]]);
    assert!(runners[0].trigger_scheduler.race_start_triggers.is_empty());
    assert_eq!(sympathy_activations(&runners), vec![0, 0]);
}

#[test]
fn test_sympathy_activates_once() {
    // Activated both by its own condition and by "Connect", listed before and after
    // it.
    let mut skill_sets: Vec<&[&str]> = vec![&[SYMPATHY_ID]; 5];
    skill_sets[0] = &[CONNECT_ID, SYMPATHY_ID];
    skill_sets[4] = &[SYMPATHY_ID, CONNECT_ID];
    let runners = sympathy_runners(10104, &skill_sets);
    assert_eq!(sympathy_activations(&runners), vec![1; 5]);
}

#[test]
fn test_random_lot_is_rolled_before_the_race() {
    let (mut runner, state) = unready_runner(vec![
        skill("never", "random_lot==0", SkillType::SpeedUp, 400000),
        skill("always", "random_lot==100", SkillType::SpeedUp, 800000),
    ]);
    let speed = runner.adjusted.speed;

    runner.apply_out_of_gate_skills(&state);

    assert_eq!(runner.adjusted.speed, speed + 80);
    assert_eq!(activated_ids(&runner), vec!["always"]);
}

#[test]
fn test_failed_random_lot_falls_through_to_next_alternative() {
    let alternatives = vec![
        alternative("random_lot==0", SkillType::SpeedUp, 1000000),
        alternative("always==1", SkillType::SpeedUp, 400000),
    ];
    let skill = Skill::new("speed".to_string(), skill_data(alternatives));
    let (mut runner, state) = unready_runner(vec![skill]);
    let speed = runner.adjusted.speed;

    runner.apply_out_of_gate_skills(&state);

    assert_eq!(runner.adjusted.speed, speed + 40);
    assert_eq!(activated_ids(&runner), vec!["speed"]);
}

#[test]
fn test_gambler_activation_rate() {
    // As the only runner, she's the most popular: `popularity<=3&random_lot==30`.
    const GAMBLER_ID: &str = "202441";
    const RACES: u64 = 300;
    let skills = Skill::load_skills_from_file("data/skill_data.json").unwrap();
    let course = load_course(10104);

    let activations = (0..RACES)
        .filter(|&seed| {
            let mut solver = RaceSolver::new(course.clone(), race_params(), &seeded_config(seed));
            let gambler = skills[GAMBLER_ID].clone();
            solver.add_runner(uma(Style::Pace), Motivation::Normal, vec![gambler]);
            let runner = &solver.ready_runners().runners[0];
            !race_start_ids(&runner.available_skills, &runner.trigger_scheduler).is_empty()
        })
        .count();

    // 90 expected; the bounds are about 4 standard deviations away.
    assert!(
        (60..=120).contains(&activations),
        "{activations} activations"
    );
}

#[test]
fn test_out_of_gate_skill_activates_once_with_several_satisfied_branches() {
    // Course 10104 is turf (ground_type==1): both OR branches hold.
    let (mut runner, state) = unready_runner(vec![skill(
        "speed",
        "ground_type==1@always==1",
        SkillType::SpeedUp,
        400000,
    )]);
    let speed = runner.adjusted.speed;

    runner.apply_out_of_gate_skills(&state);

    assert_eq!(runner.adjusted.speed, speed + 40);
    assert_eq!(activated_ids(&runner), vec!["speed"]);
}

/// Speed gains, over 50 races, of a skill whose two alternatives are eligible
/// with a 50% lottery each (`lottery==50`), giving +100 and +40 speed.
fn lottery_speed_gains(lottery: &str) -> std::collections::HashSet<u16> {
    let condition = format!("{lottery}==50");
    let alternatives = vec![
        alternative(&condition, SkillType::SpeedUp, 1000000),
        alternative(&condition, SkillType::SpeedUp, 400000),
    ];
    let skill = Skill::new("speed".to_string(), skill_data(alternatives));
    let course = load_course(10104);

    (0..50)
        .map(|seed| {
            let state = RaceSolver::new(course.clone(), race_params(), &seeded_config(seed)).state;
            let skills = vec![skill.clone()];
            let mut runner =
                UnreadyRunner::new(uma(Style::Pace), 1, Motivation::Normal, &state, skills);
            let speed = runner.adjusted.speed;
            runner.apply_out_of_gate_skills(&state);
            runner.adjusted.speed - speed
        })
        .collect()
}

#[test]
fn test_random_lot_is_rolled_by_each_alternative() {
    // The second alternative can activate after the lottery of the first one failed.
    assert_eq!(lottery_speed_gains("random_lot"), [0, 40, 100].into());
}

#[test]
fn test_random_lot_shared_draw_is_shared_by_alternatives() {
    // With a single draw per skill, the second alternative can't activate after the
    // lottery of the first one failed.
    assert_eq!(lottery_speed_gains("random_lot_shared"), [0, 100].into());
}

/// Out-of-gate skill alternative with a precondition.
fn alternative_with_precondition(
    precondition: &str,
    condition: &str,
    modifier: i32,
) -> SkillAlternative {
    SkillAlternative {
        precondition: Some(precondition.parse().unwrap()),
        ..alternative(condition, SkillType::SpeedUp, modifier)
    }
}

/// Speed gained before the race by a runner with the given skill alternatives,
/// on a 2000m turf course (10104).
fn out_of_gate_speed_gain(alternatives: Vec<SkillAlternative>) -> u16 {
    let skill = Skill::new("speed".to_string(), skill_data(alternatives));
    let (mut runner, state) = unready_runner(vec![skill]);
    let speed = runner.adjusted.speed;
    runner.apply_out_of_gate_skills(&state);
    runner.adjusted.speed - speed
}

#[test]
fn test_out_of_gate_precondition_is_required() {
    // Course 10104 is Medium (distance_type==3) turf (ground_type==1).
    let gain = |precondition| {
        out_of_gate_speed_gain(vec![alternative_with_precondition(
            precondition,
            "always==1",
            400000,
        )])
    };
    assert_eq!(gain("ground_type==2"), 0);
    assert_eq!(gain("ground_type==1"), 40);
    // Like "Long or Extended distance" preconditions: any OR branch suffices.
    assert_eq!(gain("distance_type==3@distance_type==4"), 40);
}

#[test]
fn test_out_of_gate_condition_is_required_with_satisfied_precondition() {
    // Course 10104 is turf (ground_type==1).
    let gain = out_of_gate_speed_gain(vec![alternative_with_precondition(
        "ground_type==1",
        "ground_type==2",
        400000,
    )]);
    assert_eq!(gain, 0);
}

#[test]
fn test_unsatisfied_precondition_falls_through_to_next_alternative() {
    // Like "All-in!!": a bonus alternative for specific tracks, then a default one.
    let gain = out_of_gate_speed_gain(vec![
        alternative_with_precondition("ground_type==2", "always==1", 1000000),
        alternative("always==1", SkillType::SpeedUp, 400000),
    ]);
    assert_eq!(gain, 40);
}
