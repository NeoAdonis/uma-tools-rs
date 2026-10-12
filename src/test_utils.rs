//! Shared helpers for unit tests.

use crate::configuration::Configuration;
use crate::course::*;
use crate::race::parameters::*;
use crate::race::runner::Runner;
use crate::race::solver::RaceSolver;
use crate::skill::*;
use crate::uma::*;

/// Assert that two floats are equal, or within `1e-9` of each other.
///
/// Takes an optional `tolerance = ...` and an optional format string with
/// arguments, which is prepended to the failure message as context.
macro_rules! assert_close {
    ($actual:expr, $expected:expr, tolerance = $tolerance:expr $(, $($arg:tt)+)?) => {{
        let (actual, expected): (f64, f64) = ($actual, $expected);
        let message = format!("expected {expected}, got {actual}");
        $(let message = format!("{}: {message}", format_args!($($arg)+));)?
        assert!(
            actual == expected || (actual - expected).abs() < $tolerance,
            "{message}"
        );
    }};
    ($actual:expr, $expected:expr $(, $($arg:tt)+)?) => {
        $crate::test_utils::assert_close!($actual, $expected, tolerance = 1e-9 $(, $($arg)+)?)
    };
}
pub(crate) use assert_close;

/// Load a course from `data/course_data.json`.
pub(crate) fn load_course(course_id: u32) -> Course {
    Course::from_json_file("data/course_data.json", &course_id.to_string())
        .unwrap_or_else(|_| panic!("Failed to load course {course_id}"))
}

/// Synthetic 2000m (Medium) turf course data at Tokyo, without corners, slopes
/// or straights.
///
/// Override fields with `CourseData { distance: 2400, ..course_data() }`.
pub(crate) fn course_data() -> CourseData {
    CourseData {
        distance: 2000,
        distance_type: DistanceType::Medium,
        surface: Track::Turf,
        turn: Direction::Left,
        course_set_status: vec![],
        corners: vec![],
        slopes: vec![],
        straights: vec![],
        race_track_id: 10006,
        finish_time_min: 117.1,
        finish_time_max: 123.0,
        width: 1.0,
    }
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
        fan_count: None,
    }
}

/// Ready race with standard parameters and a single runner with normal mood.
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

/// Regular skill effect on oneself with a direct value.
///
/// Override fields with `SkillEffect { target, ..effect(...) }`.
pub(crate) fn effect(skill_type: SkillType, modifier: f64) -> SkillEffect {
    SkillEffect {
        skill_type,
        target: SkillTarget::Oneself,
        modifier,
        value_usage: ValueUsage::Direct,
        additional: AdditionalActivation::Regular,
    }
}

/// Skill alternative with a single [`effect`], a base duration of 3 seconds and
/// no precondition.
pub(crate) fn alternative(
    condition: &str,
    skill_type: SkillType,
    modifier: f64,
) -> SkillAlternative {
    SkillAlternative {
        base_duration: 3.0,
        base_cooldown: DEFAULT_COOLDOWN,
        condition: condition.parse().unwrap(),
        precondition: None,
        effects: vec![effect(skill_type, modifier)],
        time_usage: TimeUsage::Direct,
    }
}

/// White skill data without popularity data or wit check, so the skill is
/// always available.
///
/// Override fields with `SkillData { rarity, ..skill_data(...) }`.
pub(crate) fn skill_data(alternatives: Vec<SkillAlternative>) -> SkillData {
    SkillData {
        rarity: SkillRarity::White,
        alternatives,
        popularity: None,
        wisdom_check: false,
    }
}

/// Always available white skill with a single [`alternative`].
pub(crate) fn skill(id: &str, condition: &str, skill_type: SkillType, modifier: f64) -> Skill {
    Skill::new(
        id.to_string(),
        skill_data(vec![alternative(condition, skill_type, modifier)]),
    )
}
