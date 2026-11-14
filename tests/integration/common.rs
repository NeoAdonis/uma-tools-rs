//! Shared helpers for integration tests.
//!
//! These mirror the crate's internal `test_utils`, which isn't available here,
//! so tests can be moved between both with few changes.

use std::sync::{Arc, OnceLock};
use uma_tools::configuration::{Assignment, Configuration, DefaultMood};
use uma_tools::course::Course;
use uma_tools::race::parameters::*;
use uma_tools::race::runner::Runner;
use uma_tools::race::solver::{Entrant, RaceSolver};
use uma_tools::skill::*;
use uma_tools::uma::*;

/// Load a course from `data/course_data.json` and `data/jp/rails/`.
pub(crate) fn load_course(course_id: u32) -> Course {
    let (courses, rails) = ("data/course_data.json", "data/jp/rails");
    Course::from_json_files(courses, rails, &course_id.to_string())
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
        career: false,
    }
}

/// Default configuration with the given RNG seed, keeping the moods set for
/// runners and giving Normal mood to the rest (see
/// [`Configuration::mood_assignment`]).
pub(crate) fn seeded_config(seed: u64) -> Configuration {
    Configuration {
        seed: Some(seed),
        mood_assignment: Assignment::Unset,
        default_mood: DefaultMood::Fixed(Mood::Normal),
        ..Default::default()
    }
}

/// Baseline runner with the given style and A aptitudes.
///
/// Override single stats with `Uma { wit: 100, ..uma(BaseStyle::Late) }`.
pub(crate) fn uma(style: BaseStyle) -> Uma {
    Uma {
        speed: 1200,
        stamina: 1000,
        power: 1000,
        guts: 800,
        wit: 1000,
        style,
        distance_aptitude: Aptitude::A,
        track_aptitude: Aptitude::A,
        style_aptitude: Aptitude::A,
        fan_count: None,
        character_id: None,
    }
}

/// The Runaway skill, from the game data.
///
/// Loaded once, since parsing the skill data is slow in unoptimized builds.
///
/// # Panics
///
/// Panics if the skill data can't be loaded.
pub(crate) fn runaway_skill() -> Arc<Skill> {
    static SKILL: OnceLock<Arc<Skill>> = OnceLock::new();
    SKILL
        .get_or_init(|| {
            SkillData::from_json_file("data/jp/skill_data.json")
                .unwrap()
                .get(202051)
                .unwrap()
                .clone()
        })
        .clone()
}

/// Ready race with standard parameters and a single runner with normal mood.
pub(crate) fn ready_solver(
    course_id: u32,
    uma: Uma,
    skills: Vec<Arc<Skill>>,
    config: Configuration,
) -> RaceSolver<Runner> {
    let mut solver = RaceSolver::new(load_course(course_id), race_params(), config);
    solver.add_runner(Entrant::new(uma, skills));
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
        value_scaling: ValueScaling::Direct,
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
        duration_scaling: DurationScaling::Direct,
    }
}

/// White skill with ID 0, without popularity data or wit check, so it's
/// always available.
///
/// Override fields with `Skill { id, rarity, ..synthetic_skill(...) }`.
pub(crate) fn synthetic_skill(alternatives: Vec<SkillAlternative>) -> Skill {
    Skill {
        id: 0,
        rarity: SkillRarity::White,
        alternatives,
        popularity: None,
        wit_check: false,
        group: 0,
        group_rate: 1,
        level: 1,
    }
}

/// Always available white skill with a single [`alternative`].
pub(crate) fn skill(id: i32, condition: &str, skill_type: SkillType, modifier: f64) -> Arc<Skill> {
    Arc::new(Skill {
        id,
        ..synthetic_skill(vec![alternative(condition, skill_type, modifier)])
    })
}

/// Always available skill whose target speed effect is too small to change the
/// race.
pub(crate) fn negligible_skill(condition: &str) -> Arc<Skill> {
    skill(999999, condition, SkillType::TargetSpeed, 0.0001)
}
