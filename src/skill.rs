//! Skill-related data structures and logic for race simulation.

use num_enum::TryFromPrimitive;
use serde::Deserialize;
use serde_repr::Deserialize_repr;
use serde_with::{BoolFromInt, serde_as};
use std::collections::HashMap;

/// Represents the type of skill effect.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Deserialize_repr, Hash, TryFromPrimitive)]
#[repr(u8)]
pub enum SkillType {
    /// Modify speed stat.
    SpeedUp = 1,
    /// Modify stamina stat.
    StaminaUp = 2,
    /// Modify power stat.
    PowerUp = 3,
    /// Modify guts stat.
    GutsUp = 4,
    /// Modify wisdom stat.
    WizUp = 5,
    /// Change running style to Runaway.
    ChangeStyleRunaway = 6,
    /// Modify field of view.
    Fov = 8,
    /// Recover (heal) some HP.
    Recovery = 9,
    /// Multiply start delay.
    MultiplyStartDelay = 10,
    /// Modify how long a runner can stay rushed.
    RushTimeUp = 13,
    /// Set start delay to a fixed value.
    SetStartDelay = 14,
    /// Modify current speed directly.
    CurrentSpeed = 21,
    /// Modify current speed with natural deceleration.
    CurrentSpeedWithNaturalDeceleration = 22,
    /// Modify target speed.
    TargetSpeed = 27,
    /// Modify lane movement speed.
    LaneMovementSpeed = 28,
    /// Modify chance of becoming rushed.
    RushChanceUp = 29,
    /// Modify acceleration.
    Acceleration = 31,
    /// Modify all stats at once.
    AllStatsUp = 32,
    /// Change target lane.
    LaneChange = 35,
    /// Activate a random gold skill.
    ActivateRandomGold = 37,
    /// Activate "Sympathy" skill.
    ActivateSympathy = 41,
    /// Extend evolved skill duration.
    ExtendEvolvedDuration = 42,
    /// Modify acceleration during Full-throttle spurt (5th Anniversary onward).
    AccelerationFullThrottle = 48,
}

/// Represents the rarity of a skill.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Deserialize_repr)]
#[repr(u8)]
pub enum SkillRarity {
    /// Common skill.
    White = 1,
    /// Non-unique gold skill; stronger than white skills.
    Gold = 2,
    /// Skill unique to a specific Uma (1 or 2 stars).
    Unique = 3,
    /// Skill unique to a specific Uma (3+ stars) after upgrade from a 1 or 2
    /// stars unique skill.
    UniqueUpgraded = 4,
    /// Skill unique to a specific Uma (3+ stars) with no 1 or 2 stars
    /// equivalent.
    UniqueNatural = 5,
    /// Skills evolved from gold skills.
    Evolution = 6,
}

/// Represents the target of a skill effect.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Deserialize_repr)]
#[repr(u8)]
pub enum SkillTarget {
    /// Skill affects oneself.
    Oneself = 1,
    /// Skill affects all runners.
    All = 2,
    /// Skill affects only runners inside field of view.
    InFov = 4,
    /// Skill affects only the next 4 runners ahead.
    AheadOfPosition = 7,
    /// Skill affects only the next 2 runners ahead.
    AheadOfSelf = 9,
    /// Skill affects only the previous 2 runners behind.
    BehindSelf = 10,
    /// Skill affects all team allies.
    AllAllies = 11,
    /// Skill affects all opponents with a specific running style.
    EnemyStrategy = 18,
    /// Skill affects runners ahead that are rushed.
    RushedAhead = 19,
    /// Skill affects runners behind that are rushed.
    RushedBehind = 20,
    /// Skill affects runners with a specific running style that are rushed.
    RushedStrategy = 21,
    /// (Unknown)
    UmaId = 22,
    /// Skill affects runners that have used recovery skills.
    UsedRecovery = 23,
}

/// Represents a single effect of a skill.
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillEffect {
    /// Type of the skill effect.
    #[serde(rename = "type")]
    pub skill_type: SkillType,
    /// Target of the skill effect.
    pub target: SkillTarget,
    /// Modifier value for the skill effect. Interpretation depends on the skill
    /// type.
    pub modifier: i32,
}

/// Represents an alternative version of a skill, with its own conditions and
/// effects.
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillAlternative {
    /// Base duration before any modifiers in 10000ths of a second.
    pub base_duration: i32,
    /// Condition string that determines when this alternative can be triggered.
    pub condition: String,
    /// List of effects that this alternative applies when triggered.
    pub effects: Vec<SkillEffect>,
    /// Precondition that must be satisfied before this alternative can be considered.
    pub precondition: String,
}

/// Represents a skill with its ID and data.
#[derive(Clone, Debug)]
pub struct Skill {
    pub id: String,
    data: SkillData,
}

impl std::ops::Deref for Skill {
    type Target = SkillData;

    fn deref(&self) -> &Self::Target {
        &self.data
    }
}

/// Represents the data of a skill.
#[serde_as]
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillData {
    /// Skill rarity.
    pub rarity: SkillRarity,
    /// Skill alternatives, each with its own conditions and effects.
    pub alternatives: Vec<SkillAlternative>,
    /// Whether this skill requires a wisdom check to activate.
    #[serde_as(as = "BoolFromInt")]
    pub wisdom_check: bool,
}

impl Skill {
    /// Create a new Skill from ID and SkillData.
    pub fn new(id: String, data: SkillData) -> Self {
        Skill { id, data }
    }

    /// Check if this skill is a "green" (stat boosting) skill.
    pub fn is_green(self: &Skill) -> bool {
        for alternative in &self.alternatives {
            for effect in &alternative.effects {
                match effect.skill_type {
                    SkillType::SpeedUp
                    | SkillType::StaminaUp
                    | SkillType::PowerUp
                    | SkillType::GutsUp
                    | SkillType::WizUp
                    | SkillType::AllStatsUp => {
                        return true;
                    }
                    _ => {}
                }
            }
        }
        false
    }

    /// Determine whether a skill effect is positive or negative.
    pub fn is_effect_positive(effect: &SkillEffect) -> bool {
        match effect.skill_type {
            // Purely positive effects
            SkillType::ActivateRandomGold
            | SkillType::ActivateSympathy
            | SkillType::ChangeStyleRunaway => true,
            // When a start delay is set, it's always to a bad start
            SkillType::SetStartDelay => false,
            // Multiplier of less than or equal to 1.0 is a reduction in delay
            SkillType::MultiplyStartDelay => effect.modifier <= 10000,
            // Less chance or shorter rush time is good
            SkillType::RushChanceUp | SkillType::RushTimeUp => effect.modifier <= 0,
            // Positive modifier is good for other effects
            _ => effect.modifier >= 0,
        }
    }

    /// Check if a skill affects you positively (buff).
    ///
    /// N.B. Because a skill can have both positive and negative effects,
    /// a skill can be both a buff and a nerf.
    pub fn is_buff(self: &Skill) -> bool {
        for alternative in &self.alternatives {
            for effect in &alternative.effects {
                if effect.target == SkillTarget::Oneself && Skill::is_effect_positive(effect) {
                    return true;
                }
            }
        }
        false
    }

    /// Check if a skill affects you negatively (nerf).
    ///
    /// N.B. Because a skill can have both positive and negative effects, a
    /// skill can be both a buff and a nerf.
    pub fn is_nerf(self: &Skill) -> bool {
        for alternative in &self.alternatives {
            for effect in &alternative.effects {
                if effect.target == SkillTarget::Oneself && !Skill::is_effect_positive(effect) {
                    return true;
                }
            }
        }
        false
    }

    /// Check if a skill affects others negatively (debuff).
    pub fn is_debuff(self: &Skill) -> bool {
        for alternative in &self.alternatives {
            for effect in &alternative.effects {
                if effect.target != SkillTarget::Oneself && !Skill::is_effect_positive(effect) {
                    return true;
                }
            }
        }
        false
    }

    /// Check if this skill is a unique skill (i.e. only available to a specific
    /// Uma).
    pub fn is_unique(self: &Skill) -> bool {
        matches!(
            self.rarity,
            SkillRarity::Unique | SkillRarity::UniqueUpgraded | SkillRarity::UniqueNatural
        )
    }

    /// Check if this skill benefits from the Wiz Limit Break buff (5th
    /// Anniversary onward).
    ///
    /// Only unique, evolved and gold skills are affected. Inherited uniques
    /// are stored as white skills, so they are excluded as well.
    pub fn is_wiz_limit_break_eligible(&self) -> bool {
        !matches!(self.rarity, SkillRarity::White)
    }

    /// Chance that this skill passes its pre-race activation check.
    ///
    /// Skills that don't require a wisdom check always pass; otherwise the
    /// chance depends on the runner's base wisdom (minimum 20%).
    pub fn activation_chance(&self, base_wiz: u16) -> f64 {
        if !self.wisdom_check {
            return 1.0;
        }
        (1.0 - 90.0 / base_wiz as f64).max(0.2)
    }

    /// Check if this skill should activate out of gate (before race begins).
    pub fn should_activate_out_of_gate(&self) -> bool {
        // Green skills (stat boosts) always activate at start
        if self.is_green() {
            return true;
        }

        // Skills with always==1 condition activate at start
        if let Some(alt) = self.alternatives.first() {
            if alt.condition == "always==1" {
                return true;
            }

            // Start delay modification skills activate at start
            if alt.effects.iter().any(|ef| {
                matches!(
                    ef.skill_type,
                    SkillType::MultiplyStartDelay | SkillType::SetStartDelay
                )
            }) {
                return true;
            }
        }

        false
    }

    /// Check if this skill can activate on a specific course (ignoring
    /// wisdom-based activation chance).
    ///
    /// This evaluates only course-static guards (ground type, distance type,
    /// etc.) and ignores runtime guards (hp percentage, phase, order, etc.)
    /// which are assumed to be potentially satisfiable during the race.
    ///
    /// Returns `true` if at least one alternative can activate on this course.
    pub fn is_activable_on_course(
        &self,
        state: &crate::race::solver::RaceState,
        runner: &crate::race::unready_runner::UnreadyRunner,
    ) -> bool {
        // A skill is activable if at least one of its alternatives can activate
        for alternative in &self.alternatives {
            if can_alternative_activate(alternative, state, runner) {
                return true;
            }
        }
        false
    }

    /// Load all skills from a JSON file into a hash map of skill ID to
    /// [`Skill`] object.
    pub fn load_skills_from_file(
        json_path: &str,
    ) -> Result<HashMap<String, Skill>, Box<dyn std::error::Error>> {
        use std::fs;

        let json_content = fs::read_to_string(json_path)?;
        let skills_data: HashMap<String, SkillData> = serde_json::from_str(&json_content)?;

        let mut skills_map = HashMap::new();
        for (id_str, data) in skills_data {
            skills_map.insert(id_str.clone(), Skill::new(id_str, data));
        }

        Ok(skills_map)
    }
}

/// Check if a specific skill alternative can activate on this course.
///
/// Parses the alternative's condition and evaluates only static guards.
/// Returns `true` if all static guards pass (or there are no static guards).
fn can_alternative_activate(
    alternative: &SkillAlternative,
    state: &crate::race::solver::RaceState,
    runner: &crate::race::unready_runner::UnreadyRunner,
) -> bool {
    use crate::condition::evaluator::check_static_guards;
    use crate::condition::parser::parse_condition;

    // Empty condition means always activable
    if alternative.condition.is_empty() {
        return true;
    }

    // Parse condition
    let condition = match parse_condition(&alternative.condition) {
        Ok(cond) => cond,
        Err(_) => {
            // If parsing fails, conservatively assume activable
            return true;
        }
    };

    // Check static guards only
    check_static_guards(&condition.comp_tree, state, runner)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn skill_with_wisdom_check(wisdom_check: bool) -> Skill {
        Skill::new(
            "test".to_string(),
            SkillData {
                rarity: SkillRarity::White,
                alternatives: vec![],
                wisdom_check,
            },
        )
    }

    #[test]
    fn test_activation_chance_without_wisdom_check() {
        let skill = skill_with_wisdom_check(false);
        assert_eq!(skill.activation_chance(1), 1.0);
        assert_eq!(skill.activation_chance(1200), 1.0);
    }

    #[test]
    fn test_activation_chance_with_wisdom_check() {
        let skill = skill_with_wisdom_check(true);
        assert!((skill.activation_chance(900) - 0.9).abs() < 1e-9);
        // Low wisdom is clamped to the minimum chance
        assert_eq!(skill.activation_chance(100), 0.2);
        assert_eq!(skill.activation_chance(0), 0.2);
    }

    #[test]
    fn test_wiz_limit_break_eligibility() {
        let skill = |rarity| {
            Skill::new(
                "test".to_string(),
                SkillData {
                    rarity,
                    alternatives: vec![],
                    wisdom_check: true,
                },
            )
        };
        assert!(!skill(SkillRarity::White).is_wiz_limit_break_eligible());
        assert!(skill(SkillRarity::Gold).is_wiz_limit_break_eligible());
        assert!(skill(SkillRarity::Unique).is_wiz_limit_break_eligible());
        assert!(skill(SkillRarity::UniqueUpgraded).is_wiz_limit_break_eligible());
        assert!(skill(SkillRarity::UniqueNatural).is_wiz_limit_break_eligible());
        assert!(skill(SkillRarity::Evolution).is_wiz_limit_break_eligible());
    }
}
