use super::*;
use crate::configuration::{Configuration, Roll};
use crate::race::solver::{Entrant, RaceSolver};
use crate::skill::SkillAlternative;
use crate::test_utils::*;
use std::sync::Arc;

/// Build a runner with the given skills on a 2000m turf course (10104), and the
/// race state it was built with.
fn unready_runner(skills: Vec<Arc<Skill>>) -> (UnreadyRunner, RaceState) {
    let state = RaceSolver::new(load_course(10104), race_params(), seeded_config(0)).state;
    let mut runner = UnreadyRunner::new(
        uma(Style::Pace),
        Mood::Normal,
        &state,
        skills,
        RngSeed::new(0),
    );
    runner.assign_gate(1, &state);
    assert!(
        runner
            .skills
            .iter()
            .all(|skill| skill.should_activate_out_of_gate()),
        "All skills should be out-of-gate"
    );
    (runner, state)
}

/// IDs of the skills that activate on the first frame the runner moves.
fn race_start_ids(skills: &[Arc<Skill>], scheduler: &TriggerScheduler) -> Vec<i32> {
    scheduler
        .race_start_triggers
        .iter()
        .map(|&logic_idx| skills[scheduler.skill_logics[logic_idx].skill_index].id)
        .collect()
}

/// IDs of the out-of-gate skills activated before the race.
fn activated_ids(runner: &UnreadyRunner) -> Vec<i32> {
    race_start_ids(&runner.skills, &runner.trigger_scheduler)
}

#[test]
fn test_stat_boost_is_applied() {
    // Stat boosts are in stat points (800000 in the skill data = +80 speed).
    let (mut runner, state) = unready_runner(vec![skill(8, "always==1", SkillType::SpeedUp, 80.0)]);
    let speed = runner.adjusted.speed;

    runner.apply_out_of_gate_skills(&state);

    assert_eq!(runner.adjusted.speed, speed + 80);
    assert_eq!(activated_ids(&runner), vec![8]);
}

#[test]
fn test_stat_boosts_by_type() {
    let stats = |uma: Uma| [uma.speed, uma.stamina, uma.power, uma.guts, uma.wit];
    let base = Uma {
        speed: 1000,
        stamina: 900,
        power: 800,
        guts: 700,
        wit: 600,
        ..uma(Style::Pace)
    };

    // (effect, modifier, [speed, stamina, power, guts, wit]).
    let cases = [
        (SkillType::SpeedUp, 60.0, [1060, 900, 800, 700, 600]),
        (SkillType::StaminaUp, 60.0, [1000, 960, 800, 700, 600]),
        (SkillType::PowerUp, 60.0, [1000, 900, 860, 700, 600]),
        (SkillType::GutsUp, 60.0, [1000, 900, 800, 760, 600]),
        (SkillType::WitUp, 60.0, [1000, 900, 800, 700, 660]),
        (SkillType::AllStatsUp, 60.0, [1060, 960, 860, 760, 660]),
        // Negative boosts lower stats, down to 1.
        (SkillType::AllStatsUp, -50.0, [950, 850, 750, 650, 550]),
        (SkillType::SpeedUp, -2000.0, [1, 900, 800, 700, 600]),
        // Not a stat boost.
        (SkillType::TargetSpeed, 60.0, [1000, 900, 800, 700, 600]),
    ];
    for (skill_type, modifier, expected) in cases {
        let mut uma = base;
        apply_stat_boost(&mut uma, &skill_type, modifier);
        assert_eq!(stats(uma), expected, "{skill_type:?} {modifier}");
    }

    // Stats never overflow.
    let mut uma = Uma {
        speed: u16::MAX - 10,
        ..base
    };
    apply_stat_boost(&mut uma, &SkillType::SpeedUp, 60.0);
    assert_eq!(uma.speed, u16::MAX);
}

#[test]
fn test_fan_count_falls_back_to_configured_default() {
    let config = Configuration {
        default_fan_count: 500000,
        ..seeded_config(0)
    };
    let state = RaceSolver::new(load_course(10104), race_params(), config).state;
    let fan_count = |fan_count| {
        let uma = Uma {
            fan_count,
            ..uma(Style::Pace)
        };
        UnreadyRunner::new(uma, Mood::Normal, &state, vec![], RngSeed::new(0))
            .initial_state
            .fan_count
    };

    assert_eq!(fan_count(None), 500000);
    assert_eq!(fan_count(Some(12345)), 12345);
}

#[test]
fn test_gate_brackets() {
    // Brackets of gates 1, 2, 3...
    let cases: [(u8, &[u8]); 7] = [
        // One gate per bracket.
        (5, &[1, 2, 3, 4, 5]),
        (8, &[1, 2, 3, 4, 5, 6, 7, 8]),
        // Extra gates go to the last brackets.
        (9, &[1, 2, 3, 4, 5, 6, 7, 8, 8]),
        (10, &[1, 2, 3, 4, 5, 6, 7, 7, 8, 8]),
        (16, &[1, 1, 2, 2, 3, 3, 4, 4, 5, 5, 6, 6, 7, 7, 8, 8]),
        // Then again from the last bracket.
        (17, &[1, 1, 2, 2, 3, 3, 4, 4, 5, 5, 6, 6, 7, 7, 8, 8, 8]),
        (18, &[1, 1, 2, 2, 3, 3, 4, 4, 5, 5, 6, 6, 7, 7, 7, 8, 8, 8]),
    ];
    for (gates, expected) in cases {
        let brackets: Vec<u8> = (1..=gates)
            .map(|gate| UnreadyRunner::gate_bracket(gate, gates))
            .collect();
        assert_eq!(brackets, expected, "{gates} gates");
    }
}

#[test]
fn test_rush_style_changes() {
    // Every runner rushes.
    let config = Configuration {
        rushing: Roll::Always,
        ..seeded_config(0)
    };
    let state = RaceSolver::new(load_course(10104), race_params(), config).state;
    let rush_styles = |style: Style| -> Vec<Style> {
        (0..2000)
            .map(|seed| {
                let mut runner = UnreadyRunner::new(
                    uma(style),
                    Mood::Normal,
                    &state,
                    vec![],
                    RngSeed::new(seed),
                );
                runner.assign_gate(1, &state);
                runner.build(&state).rush_state.unwrap().style
            })
            .collect()
    };
    // Share of `styles` that are `style`.
    let share = |styles: &[Style], style| {
        styles.iter().filter(|&&s| s == style).count() as f64 / styles.len() as f64
    };

    // Front-like styles keep their style; Pace runners always rush as Front.
    for (style, expected) in [
        (Style::Runaway, Style::Runaway),
        (Style::Front, Style::Front),
        (Style::Pace, Style::Front),
    ] {
        let styles = rush_styles(style);
        assert!(styles.iter().all(|&s| s == expected), "{style:?}");
    }

    // (style, [Front, Pace, Late] shares).
    for (style, expected) in [
        (Style::Late, [0.75, 0.25, 0.0]),
        (Style::End, [0.7, 0.2, 0.1]),
    ] {
        let styles = rush_styles(style);
        let actual = [Style::Front, Style::Pace, Style::Late].map(|s| share(&styles, s));
        for (actual, expected) in actual.into_iter().zip(expected) {
            assert_close!(actual, expected, tolerance = 0.04, "{style:?}");
        }
    }
}

/// Start delay of a runner before and after applying an out-of-gate skill with
/// the given effect.
fn start_delays(skill_type: SkillType, modifier: f64) -> (f64, f64) {
    let (mut runner, state) = unready_runner(vec![skill(2, "always==1", skill_type, modifier)]);
    let before = runner.start_delay;
    runner.apply_out_of_gate_skills(&state);
    assert_eq!(activated_ids(&runner), vec![2]);
    (before, runner.start_delay)
}

#[test]
fn test_start_delay_modifications() {
    let (before, after) = start_delays(SkillType::AddStartDelay, 0.1);
    assert_eq!(after, before + 0.1);

    // Like "Concentration": 40% of the rolled delay.
    let (before, after) = start_delays(SkillType::MultiplyStartDelay, 0.4);
    assert_eq!(after, before * 0.4);
}

#[test]
fn test_every_out_of_gate_skill_is_applied() {
    let (mut runner, state) = unready_runner(vec![
        skill(2, "always==1", SkillType::AddStartDelay, 0.1),
        skill(7, "always==1", SkillType::RushChanceUp, -3.0),
    ]);
    let start_delay = runner.start_delay;

    runner.apply_out_of_gate_skills(&state);

    assert_eq!(runner.start_delay, start_delay + 0.1);
    assert_eq!(runner.rush_chance_modifier, -3.0);
    assert_eq!(activated_ids(&runner), vec![2, 7]);
}

#[test]
fn test_unsatisfied_condition_is_not_applied() {
    // Course 10104 is turf (ground_type==1).
    let (mut runner, state) = unready_runner(vec![skill(
        3,
        "ground_type==2",
        SkillType::AddStartDelay,
        0.1,
    )]);
    let start_delay = runner.start_delay;

    runner.apply_out_of_gate_skills(&state);

    assert_eq!(runner.start_delay, start_delay);
    assert!(activated_ids(&runner).is_empty());
}

#[test]
fn test_unsatisfied_stat_boost_is_not_applied() {
    // Course 10104 is turf (ground_type==1).
    let (mut runner, state) =
        unready_runner(vec![skill(4, "ground_type==2", SkillType::SpeedUp, 80.0)]);
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
        alternative("ground_type==2", SkillType::SpeedUp, 100.0),
        alternative("ground_type==1", SkillType::SpeedUp, 40.0),
        alternative("always==1", SkillType::SpeedUp, 80.0),
    ];
    let skill = Arc::new(Skill {
        id: 8,
        ..synthetic_skill(alternatives)
    });
    let (mut runner, state) = unready_runner(vec![skill]);
    let speed = runner.adjusted.speed;

    runner.apply_out_of_gate_skills(&state);

    assert_eq!(runner.adjusted.speed, speed + 40);
    assert_eq!(activated_ids(&runner), vec![8]);
}

#[test]
fn test_runtime_skills_activate_alongside_out_of_gate_skills() {
    // Out-of-gate skills listed first must not shift the skills the trigger
    // scheduler refers to.
    let skills = vec![
        skill(8, "always==1", SkillType::SpeedUp, 40.0),
        skill(2, "always==1", SkillType::AddStartDelay, 0.1),
        skill(5, "distance_rate>=50", SkillType::TargetSpeed, 0.35),
    ];
    let mut solver = ready_solver(10104, uma(Style::Pace), skills, seeded_config(0));
    solver.run();

    let activations: Vec<_> = solver.runners[0]
        .activated_skills
        .iter()
        .map(|record| (record.skill_id, record.distance))
        .collect();
    assert_eq!(activations.len(), 3, "{activations:?}");
    assert_eq!(&activations[..2], &[(8, 0.0), (2, 0.0)]);
    assert_eq!(activations[2].0, 5);
    assert!(activations[2].1 >= 1000.0, "{activations:?}");
}

/// Build a ready runner with low wit and the given skills, which all require a
/// wit check.
fn checked_runner(config: Configuration, skill_count: usize) -> Runner {
    let skills = (0..skill_count)
        .map(|i| {
            let alternative = alternative("distance_rate>=50", SkillType::TargetSpeed, 0.0001);
            Arc::new(Skill {
                id: i as i32,
                wit_check: true,
                ..synthetic_skill(vec![alternative])
            })
        })
        .collect();
    let uma = Uma {
        wit: 100,
        ..uma(Style::Pace)
    };
    let mut solver = ready_solver(10104, uma, skills, config);
    solver.runners.remove(0)
}

fn config(wit_checks: Roll, rushing: Roll) -> Configuration {
    Configuration {
        wit_checks,
        rushing,
        ..seeded_config(0)
    }
}

#[test]
fn test_wit_checks_can_be_forced() {
    // With 100 wit, each skill has a 20% chance to pass the check.
    let runner = checked_runner(config(Roll::Always, Roll::Random), 20);
    assert_eq!(runner.available_skills().len(), 20);
    let runner = checked_runner(config(Roll::Never, Roll::Random), 20);
    assert!(runner.available_skills().is_empty());
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

/// Ready runner with the given wit and skills.
fn runner_with_wit(wit: u16, skills: Vec<Arc<Skill>>, seed: u64) -> Runner {
    let uma = Uma {
        wit,
        ..uma(Style::Pace)
    };
    ready_solver(10104, uma, skills, seeded_config(seed))
        .runners
        .remove(0)
}

#[test]
fn test_rush_chance_is_rounded_up() {
    // (base wit, rush chance): 19.00%, 13.26%, 11.01% and 9.74% before rounding.
    for (wit, expected) in [(300, 19.0), (600, 14.0), (900, 12.0), (1200, 10.0)] {
        let runner = runner_with_wit(wit, vec![], 0);
        assert_eq!(runner.rush_chance, expected, "{wit} wit");
    }
}

#[test]
fn test_rush_chance_is_clamped() {
    // Rush chance down by 3% (as in the skill data).
    let calm = |i| skill(i, "always==1", SkillType::RushChanceUp, -3.0);
    for seed in 0..10 {
        // Very low wit: over 100% (about 24,600%).
        let runner = runner_with_wit(1, vec![], seed);
        assert_eq!(runner.rush_chance, 100.0, "seed {seed}");
        assert!(runner.rush_state.is_some(), "seed {seed}");

        // High wit (8.7% at 1600 base wit), minus 12%: below 0% even when rounded up.
        let runner = runner_with_wit(2000, (0..4).map(calm).collect(), seed);
        assert_eq!(runner.rush_chance, 0.0, "seed {seed}");
        assert!(runner.rush_state.is_none(), "seed {seed}");
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
    let speed = skill(8, "always==1", SkillType::SpeedUp, 80.0);
    let mut solver = ready_solver(10104, uma(Style::Pace), vec![speed], seeded_config(0));
    solver.run();

    let counts = &solver.runners[0].trigger_scheduler.activation_counts;
    assert_eq!(counts.all, 1);
    assert_eq!(counts.start, 1);
}

const CONNECT_ID: i32 = 201632;
const METI_RESONANCE_ID: i32 = 110502211;

/// Run the out-of-gate step of a race on `course_id` with one runner per skill
/// set, using the skills in `data/skill_data.json`.
///
/// Runners are returned in the order of their skill sets.
fn sympathy_runners(course_id: u32, skill_sets: &[&[i32]]) -> Vec<Runner> {
    let skills = Skill::load_all_from_json_file("data/skill_data.json").unwrap();
    let mut solver = RaceSolver::new(load_course(course_id), race_params(), seeded_config(0));
    for (i, ids) in skill_sets.iter().enumerate() {
        // Guts doesn't affect speed, so it tells runners apart.
        let uma = Uma {
            guts: 800 + i as u16,
            ..uma(Style::Pace)
        };
        let skills = ids.iter().map(|id| skills[id].clone()).collect();
        solver.add_runner(Entrant::new(uma, skills));
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
            race_start_ids(runner.available_skills(), &runner.trigger_scheduler)
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
    let mut skill_sets: Vec<&[i32]> = vec![&[SYMPATHY_ID]; 5];
    skill_sets[0] = &[CONNECT_ID, SYMPATHY_ID];
    skill_sets[4] = &[SYMPATHY_ID, CONNECT_ID];
    let runners = sympathy_runners(10104, &skill_sets);
    assert_eq!(sympathy_activations(&runners), vec![1; 5]);
}

#[test]
fn test_random_lot_is_rolled_before_the_race() {
    let (mut runner, state) = unready_runner(vec![
        skill(6, "random_lot==0", SkillType::SpeedUp, 40.0),
        skill(1, "random_lot==100", SkillType::SpeedUp, 80.0),
    ]);
    let speed = runner.adjusted.speed;

    runner.apply_out_of_gate_skills(&state);

    assert_eq!(runner.adjusted.speed, speed + 80);
    assert_eq!(activated_ids(&runner), vec![1]);
}

#[test]
fn test_failed_random_lot_falls_through_to_next_alternative() {
    let alternatives = vec![
        alternative("random_lot==0", SkillType::SpeedUp, 100.0),
        alternative("always==1", SkillType::SpeedUp, 40.0),
    ];
    let skill = Arc::new(Skill {
        id: 8,
        ..synthetic_skill(alternatives)
    });
    let (mut runner, state) = unready_runner(vec![skill]);
    let speed = runner.adjusted.speed;

    runner.apply_out_of_gate_skills(&state);

    assert_eq!(runner.adjusted.speed, speed + 40);
    assert_eq!(activated_ids(&runner), vec![8]);
}

#[test]
fn test_gambler_activation_rate() {
    // As the only runner, she's the most popular: `popularity<=3&random_lot==30`.
    const GAMBLER_ID: i32 = 202441;
    const RACES: u64 = 300;
    let skills = Skill::load_all_from_json_file("data/skill_data.json").unwrap();
    let course = load_course(10104);

    let activations = (0..RACES)
        .filter(|&seed| {
            let mut solver = RaceSolver::new(course.clone(), race_params(), seeded_config(seed));
            let gambler = skills[&GAMBLER_ID].clone();
            solver.add_runner(Entrant::new(uma(Style::Pace), vec![gambler]));
            let runner = &solver.ready_runners().runners[0];
            !race_start_ids(runner.available_skills(), &runner.trigger_scheduler).is_empty()
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
        8,
        "ground_type==1@always==1",
        SkillType::SpeedUp,
        40.0,
    )]);
    let speed = runner.adjusted.speed;

    runner.apply_out_of_gate_skills(&state);

    assert_eq!(runner.adjusted.speed, speed + 40);
    assert_eq!(activated_ids(&runner), vec![8]);
}

/// Speed gains, over 50 races, of a skill whose two alternatives are eligible
/// with a 50% lottery each (`lottery==50`), giving +100 and +40 speed.
fn lottery_speed_gains(lottery: &str) -> std::collections::HashSet<u16> {
    let condition = format!("{lottery}==50");
    let alternatives = vec![
        alternative(&condition, SkillType::SpeedUp, 100.0),
        alternative(&condition, SkillType::SpeedUp, 40.0),
    ];
    let skill = Arc::new(Skill {
        id: 8,
        ..synthetic_skill(alternatives)
    });
    let course = load_course(10104);

    (0..50)
        .map(|seed| {
            let state = RaceSolver::new(course.clone(), race_params(), seeded_config(seed)).state;
            let skills = vec![skill.clone()];
            let mut runner = UnreadyRunner::new(
                uma(Style::Pace),
                Mood::Normal,
                &state,
                skills,
                RngSeed::new(seed),
            );
            runner.assign_gate(1, &state);
            let speed = runner.adjusted.speed;
            runner.apply_out_of_gate_skills(&state);
            runner.adjusted.speed - speed
        })
        .collect()
}

#[test]
fn test_lottery_draw_is_shared_by_alternatives() {
    // With a single draw per skill, the second alternative can't activate after the
    // lottery of the first one failed. `random_lot` is never evaluated twice for
    // the same skill in practice, so it shares the draw too.
    for lottery in ["random_lot", "random_lot_shared"] {
        assert_eq!(lottery_speed_gains(lottery), [0, 100].into(), "{lottery}");
    }
}

/// Out-of-gate skill alternative with a precondition.
fn alternative_with_precondition(
    precondition: &str,
    condition: &str,
    modifier: f64,
) -> SkillAlternative {
    SkillAlternative {
        precondition: Some(precondition.parse().unwrap()),
        ..alternative(condition, SkillType::SpeedUp, modifier)
    }
}

/// Speed gained before the race by a runner with the given skill alternatives,
/// on a 2000m turf course (10104).
fn out_of_gate_speed_gain(alternatives: Vec<SkillAlternative>) -> u16 {
    let skill = Arc::new(Skill {
        id: 8,
        ..synthetic_skill(alternatives)
    });
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
            40.0,
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
        40.0,
    )]);
    assert_eq!(gain, 0);
}

#[test]
fn test_unsatisfied_precondition_falls_through_to_next_alternative() {
    // Like "All-in!!": a bonus alternative for specific tracks, then a default one.
    let gain = out_of_gate_speed_gain(vec![
        alternative_with_precondition("ground_type==2", "always==1", 100.0),
        alternative("always==1", SkillType::SpeedUp, 40.0),
    ]);
    assert_eq!(gain, 40);
}

#[test]
fn test_in_race_alternatives_are_not_activated_before_the_race() {
    // The in-race alternative holds, but it's compiled into triggers instead.
    let alternatives = vec![
        alternative("course_distance==9999", SkillType::SpeedUp, 80.0),
        alternative("always==1", SkillType::TargetSpeed, 0.15),
    ];
    let (mut runner, state) = unready_runner(vec![Arc::new(Skill {
        id: 8,
        ..synthetic_skill(alternatives)
    })]);
    let speed = runner.adjusted.speed;

    runner.apply_out_of_gate_skills(&state);

    assert!(activated_ids(&runner).is_empty());
    assert_eq!(runner.adjusted.speed, speed);
    let logics = &runner.trigger_scheduler.skill_logics;
    assert_eq!(logics.len(), 1);
    assert_eq!(logics[0].alternative_index, 1);
}

#[test]
fn test_min_speed_scales_with_guts() {
    // 2000m course: 0.85 * 20 m/s, plus sqrt(200 * 800) * 0.001 from guts.
    let uma = Uma {
        guts: 800,
        ..uma(Style::Pace)
    };
    let runner = ready_solver(10104, uma, vec![], seeded_config(0))
        .runners
        .remove(0);
    assert_eq!(runner.adjusted.guts, 800);
    assert_close!(runner.min_speed, 17.0 + 0.4);
}
