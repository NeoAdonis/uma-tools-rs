use std::sync::Arc;

use rand::RngExt;

use super::*;
use crate::race::solver::{Entrant, RaceSolver};
use crate::skill::{Skill, SkillType};
use crate::test_utils::*;
use crate::uma::Style;

#[test]
fn test_streams_are_reproducible() {
    let seed = RngSeed::new(42);
    let draw = |stream| seed.rng(stream).random::<u64>();
    assert_eq!(draw(Stream::Rush), draw(Stream::Rush));
    assert_eq!(draw(Stream::Skill(100101)), draw(Stream::Skill(100101)));
}

#[test]
fn test_streams_are_independent() {
    let seed = RngSeed::new(42);
    let draw = |seed: RngSeed, stream| seed.rng(stream).random::<u64>();
    // Different keys, values and seeds give different streams.
    assert_ne!(draw(seed, Stream::Rush), draw(seed, Stream::Positioning));
    assert_ne!(
        draw(seed, Stream::Skill(100101)),
        draw(seed, Stream::Skill(100102))
    );
    assert_ne!(
        draw(seed, Stream::WitCheck(100101)),
        draw(seed, Stream::Skill(100101))
    );
    assert_ne!(
        draw(seed, Stream::Rush),
        draw(RngSeed::new(43), Stream::Rush)
    );
    // So do the streams of different runners.
    let first = seed.derive(Stream::Runner(0));
    let second = seed.derive(Stream::Runner(1));
    assert_ne!(draw(first, Stream::Rush), draw(second, Stream::Rush));
}

/// Skill with a wit check, whose target speed effect activates on `condition`.
fn skill_with_wit_check(id: i32, condition: &str) -> Arc<Skill> {
    Arc::new(Skill {
        id,
        wit_check: true,
        ..synthetic_skill(vec![alternative(condition, SkillType::TargetSpeed, 0.35)])
    })
}

/// Finish times by gate of a race on a turf course between three runners with
/// random skills, the first one also having `extra` skills before her own.
fn finish_times(seed: u64, extra: &[Arc<Skill>]) -> Vec<(u8, f64)> {
    let mut solver = RaceSolver::new(load_course(10104), race_params(), seeded_config(seed));
    let first_skills = [
        skill_with_wit_check(1, "phase_random==1"),
        skill_with_wit_check(2, "corner_random==1"),
    ];
    let skills = extra.iter().chain(&first_skills).cloned().collect();
    solver.add_runner(Entrant::new(uma(Style::Pace), skills));
    for (style, id) in [(Style::Front, 3), (Style::Late, 4)] {
        let skills = vec![skill_with_wit_check(id, "phase_laterhalf_random==1")];
        solver.add_runner(Entrant::new(uma(style), skills));
    }
    let mut solver = solver.ready_runners();
    solver.run();

    let mut times: Vec<_> = solver
        .runners
        .iter()
        .map(|runner| (runner.gate_roll, runner.finish_time.unwrap()))
        .collect();
    times.sort_by_key(|&(gate, _)| gate);
    times
}

#[test]
fn test_skill_that_never_activates_keeps_race_identical() {
    // It rolls its random variable and wit check, but never activates on turf.
    let never = skill_with_wit_check(5, "phase_random==2&ground_type==2");
    for seed in 0..10 {
        assert_eq!(
            finish_times(seed, &[]),
            finish_times(seed, std::slice::from_ref(&never)),
            "seed {seed}"
        );
    }
}
