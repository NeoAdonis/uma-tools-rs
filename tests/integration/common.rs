//! Shared helpers for integration tests.

use uma_tools::course::Course;
use uma_tools::race::parameters::*;
use uma_tools::skill::*;
use uma_tools::trigger::skill_activation::{LogicAction, SkillActivationLogic};
use uma_tools::trigger::system::TriggerScheduler;

/// Load a course from `data/course_data.json`.
pub fn load_course(course_id: u32) -> Course {
    Course::from_json_file("data/course_data.json", &course_id.to_string())
        .unwrap_or_else(|_| panic!("Failed to load course {course_id}"))
}

/// Standard race parameters (firm turf, sunny spring G1 with 9 gates) with the
/// given RNG seed.
pub fn race_params(rng_seed: u64) -> RaceParameters {
    RaceParameters {
        track_condition: TrackCondition::Firm,
        weather: Weather::Sunny,
        season: Season::Spring,
        time: Time::None,
        grade: Grade::G1,
        gates: 9,
        rng_seed,
    }
}

/// Build a white target speed skill with one `(condition, precondition)` pair
/// per alternative.
pub fn make_skill_with_alternatives(id: &str, alternatives: Vec<(&str, &str)>) -> Skill {
    Skill::new(
        id.to_string(),
        SkillData {
            rarity: SkillRarity::White,
            alternatives: alternatives
                .iter()
                .map(|(condition, precondition)| SkillAlternative {
                    condition: condition.to_string(),
                    precondition: precondition.to_string(),
                    base_duration: 40000,
                    base_cooldown: DEFAULT_COOLDOWN,
                    effects: vec![SkillEffect {
                        skill_type: SkillType::TargetSpeed,
                        target: SkillTarget::Oneself,
                        modifier: 4000,
                    }],
                })
                .collect(),
            wisdom_check: true,
        },
    )
}

/// Build a white target speed skill with a single alternative.
pub fn make_skill(id: &str, condition: &str, precondition: &str) -> Skill {
    make_skill_with_alternatives(id, vec![(condition, precondition)])
}

/// Logics that activate skills, i.e. excluding the ones that unlock preconditions.
pub fn activation_logics(scheduler: &TriggerScheduler) -> Vec<&SkillActivationLogic> {
    scheduler
        .skill_logics
        .iter()
        .filter(|logic| logic.action == LogicAction::Activate)
        .collect()
}
