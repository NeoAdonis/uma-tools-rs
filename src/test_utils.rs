//! Shared helpers for unit tests.

use crate::configuration::Configuration;
use crate::course::Course;
use crate::race::parameters::*;
use crate::race::runner::Runner;
use crate::race::solver::RaceSolver;
use crate::skill::Skill;
use crate::uma::*;

/// Load a course from `data/course_data.json`.
pub(crate) fn load_course(course_id: u32) -> Course {
    Course::from_json_file("data/course_data.json", &course_id.to_string())
        .unwrap_or_else(|_| panic!("Failed to load course {course_id}"))
}

/// Standard race parameters (firm turf, sunny spring G1 with 9 gates).
pub(crate) fn race_params() -> RaceParameters {
    RaceParameters {
        track_condition: TrackCondition::Firm,
        weather: Weather::Sunny,
        season: Season::Spring,
        time: Time::None,
        grade: Grade::G1,
        gates: 9,
    }
}

/// Default configuration with the given RNG seed.
pub(crate) fn seeded_config(seed: u64) -> Configuration {
    Configuration {
        seed: Some(seed),
        ..Default::default()
    }
}

/// Baseline runner with the given style and A aptitudes.
///
/// Override single stats with `Uma { wiz: 100, ..uma(Style::Late) }`.
pub(crate) fn uma(style: Style) -> Uma {
    Uma {
        speed: 1200,
        stamina: 1000,
        power: 1000,
        guts: 800,
        wiz: 1000,
        style,
        distance_aptitude: Aptitude::A,
        track_aptitude: Aptitude::A,
        style_aptitude: Aptitude::A,
    }
}

/// Ready race with standard parameters and a single runner with normal
/// motivation.
pub(crate) fn ready_solver(
    course_id: u32,
    uma: Uma,
    skills: Vec<Skill>,
    config: &Configuration,
) -> RaceSolver<Runner> {
    let mut solver = RaceSolver::new(load_course(course_id), race_params(), config);
    solver.add_runner(uma, Motivation::Normal, skills);
    solver.ready_runners()
}
