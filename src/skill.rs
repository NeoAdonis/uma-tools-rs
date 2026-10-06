//! Skill-related data structures and logic for race simulation.

use num_enum::TryFromPrimitive;
use serde::{Deserialize, Deserializer, de::Error};
use serde_repr::Deserialize_repr;
use serde_with::{BoolFromInt, serde_as};
use std::collections::HashMap;
use std::sync::Arc;

use crate::condition::parser::{CompOp, Comparison, ComparisonTree, Condition};
use crate::condition::variables::VarType;
use crate::uma::Style;

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

impl SkillType {
    /// Check if this effect type is a stat boost (applied permanently).
    pub fn is_stat_boost(self) -> bool {
        use SkillType::*;
        matches!(
            self,
            SpeedUp | StaminaUp | PowerUp | GutsUp | WizUp | AllStatsUp
        )
    }

    /// Check if this effect type is a start delay modification.
    pub fn is_start_delay_modification(self) -> bool {
        matches!(
            self,
            SkillType::MultiplyStartDelay | SkillType::SetStartDelay
        )
    }

    /// Returns the running style this effect type changes to, or `None` if it isn't
    /// a style change.
    pub fn style_change(self) -> Option<Style> {
        match self {
            SkillType::ChangeStyleRunaway => Some(Style::Runaway),
            _ => None,
        }
    }

    /// Check if this effect type is a rush chance effect.
    pub fn is_rush_chance_effect(self) -> bool {
        matches!(self, SkillType::RushChanceUp)
    }

    /// Check if this effect type is a timed effect.
    pub fn is_timed_effect(self) -> bool {
        use SkillType::*;
        matches!(
            self,
            Acceleration
                | AccelerationFullThrottle
                | TargetSpeed
                | CurrentSpeed
                | CurrentSpeedWithNaturalDeceleration
                | Fov
                | ExtendEvolvedDuration
                | LaneMovementSpeed
                | LaneChange
        )
    }
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
    /// Skill unique to a specific Uma (3+ stars) after upgrade from a 1 or 2 stars
    /// unique skill.
    UniqueUpgraded = 4,
    /// Skill unique to a specific Uma (3+ stars) with no 1 or 2 stars equivalent.
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
    /// Unknown.
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

/// Base cooldown used by most skills, long enough that they never reactivate.
pub const DEFAULT_COOLDOWN: i32 = 500_0000; // 500 seconds.

fn default_cooldown() -> i32 {
    DEFAULT_COOLDOWN
}

/// Represents an alternative version of a skill, with its own conditions and
/// effects.
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillAlternative {
    /// Base cooldown duration in 10000ths of a second.
    #[serde(default = "default_cooldown")]
    pub base_cooldown: i32,
    /// Base duration before any modifiers in 10000ths of a second.
    pub base_duration: i32,
    /// Condition that determines when this alternative can be triggered.
    #[serde(deserialize_with = "deserialize_condition")]
    pub condition: Condition,
    /// List of effects that this alternative applies when triggered.
    pub effects: Vec<SkillEffect>,
    /// Precondition that must be satisfied before this alternative can be
    /// considered, if any.
    #[serde(deserialize_with = "deserialize_precondition")]
    pub precondition: Option<Condition>,
}

/// Parse a condition string while deserializing.
fn deserialize_condition<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<Condition, D::Error> {
    let condition = String::deserialize(deserializer)?;
    condition
        .parse()
        .map_err(|e| D::Error::custom(format!("invalid condition '{condition}': {e}")))
}

/// Parse a precondition string while deserializing. An empty string means
/// there's no precondition.
fn deserialize_precondition<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<Condition>, D::Error> {
    let precondition = String::deserialize(deserializer)?;
    if precondition.is_empty() {
        return Ok(None);
    }
    precondition
        .parse()
        .map(Some)
        .map_err(|e| D::Error::custom(format!("invalid precondition '{precondition}': {e}")))
}

/// Stat that can receive popularity points from a skill.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Deserialize_repr)]
#[repr(u8)]
pub enum PopularityStat {
    None = 0,
    Speed = 1,
    Stamina = 2,
    Power = 3,
    Guts = 4,
    Wiz = 5,
}

/// Represents the popularity points granted by a skill to a specific stat.
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillPopularity {
    /// Stat that receives popularity points.
    pub stat: PopularityStat,
    /// Amount of popularity points granted (negative for negative skills).
    pub value: i8,
}

/// Represents a skill with its ID and data.
///
/// The data is shared, so cloning a skill is cheap.
#[derive(Clone, Debug)]
pub struct Skill {
    pub id: String,
    data: Arc<SkillData>,
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
    /// Popularity points granted by this skill to specific stats.
    ///
    /// `None` when missing from the skill data (outdated data), in which case
    /// the points are estimated from the skill rarity and effects.
    #[serde(default)]
    pub popularity: Option<Vec<SkillPopularity>>,
    /// Whether this skill requires a wisdom check to activate.
    #[serde_as(as = "BoolFromInt")]
    pub wisdom_check: bool,
}

impl Skill {
    /// Create a new Skill from ID and SkillData.
    pub fn new(id: String, data: SkillData) -> Self {
        Skill {
            id,
            data: Arc::new(data),
        }
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
            // Purely positive effects.
            SkillType::ActivateRandomGold
            | SkillType::ActivateSympathy
            | SkillType::ChangeStyleRunaway => true,
            // When a start delay is set, it's always to a bad start.
            SkillType::SetStartDelay => false,
            // Multiplier of less than or equal to 1.0 is a reduction in delay.
            SkillType::MultiplyStartDelay => effect.modifier <= 10000,
            // Less chance or shorter rush time is good.
            SkillType::RushChanceUp | SkillType::RushTimeUp => effect.modifier <= 0,
            // Positive modifier is good for other effects.
            _ => effect.modifier >= 0,
        }
    }

    /// Check if a skill affects you positively (buff).
    ///
    /// N.B. Because a skill can have both positive and negative effects, a skill
    /// can be both a buff and a nerf.
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
    /// N.B. Because a skill can have both positive and negative effects, a skill
    /// can be both a buff and a nerf.
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

    /// Check if `self` is a unique skill (i.e. only available to a specific Uma).
    pub fn is_unique(self: &Skill) -> bool {
        matches!(
            self.rarity,
            SkillRarity::Unique | SkillRarity::UniqueUpgraded | SkillRarity::UniqueNatural
        )
    }

    /// Check if this skill benefits from the Wiz Limit Break buff (5th Anniversary
    /// onward).
    ///
    /// Only unique, evolved and gold skills are affected. Inherited uniques
    /// are stored as white skills, so they are excluded as well.
    pub fn is_wiz_limit_break_eligible(&self) -> bool {
        !matches!(self.rarity, SkillRarity::White)
    }

    /// Chance that this skill passes its pre-race activation check.
    ///
    /// Skills that don't require a wisdom check always pass; otherwise the chance
    /// depends on the runner's base wisdom (minimum 20%).
    pub fn activation_chance(&self, base_wiz: u16) -> f64 {
        if !self.wisdom_check {
            return 1.0;
        }
        (1.0 - 90.0 / base_wiz as f64).max(0.2)
    }

    /// Check if this skill should activate out of gate (before race begins).
    pub fn should_activate_out_of_gate(&self) -> bool {
        // Green skills (stat boosts) always activate at start.
        if self.is_green() {
            return true;
        }

        // Skills with always==1 condition activate at start.
        if let Some(alt) = self.alternatives.first() {
            if alt.condition.comp_tree
                == ComparisonTree::Single(Comparison {
                    var: VarType::Always,
                    op: CompOp::Equal,
                    value: 1,
                })
            {
                return true;
            }

            // Start delay modification skills activate at start.
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

    /// Check if this skill adds popularity points to a runner in a race.
    ///
    /// This evaluates only popularity guards (distance type, ground type,
    /// running style, etc.; see [`VarType::is_popularity_guard`]). Any other
    /// condition is assumed to be satisfiable.
    ///
    /// Returns `true` if the guards pass for at least one alternative.
    ///
    /// [`VarType::is_popularity_guard`]:
    /// crate::condition::variables::VarType::is_popularity_guard
    pub fn is_popularity_eligible(
        &self,
        state: &crate::race::solver::RaceState,
        runner: &crate::race::unready_runner::UnreadyRunner,
    ) -> bool {
        use crate::condition::evaluator::check_popularity_guards;

        self.alternatives.iter().any(|alternative| {
            check_popularity_guards(&alternative.condition.comp_tree, state, runner)
        })
    }

    /// Load all skills from a JSON file into a map of skill ID to [`Skill`] object.
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

#[cfg(test)]
mod tests {
    use super::*;

    fn skill_with_wisdom_check(wisdom_check: bool) -> Skill {
        Skill::new(
            "test".to_string(),
            SkillData {
                rarity: SkillRarity::White,
                alternatives: vec![],
                popularity: None,
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
        // Low wisdom is clamped to the minimum chance.
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
                    popularity: None,
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

    /// Deserialize an alternative with the given condition and precondition.
    fn alternative(condition: &str, precondition: &str) -> serde_json::Result<SkillAlternative> {
        let json = format!(
            r#"{{"baseDuration": 30000, "condition": "{condition}",
                "effects": [{{"type": 27, "target": 1, "modifier": 3500}}],
                "precondition": "{precondition}"}}"#
        );
        serde_json::from_str(&json)
    }

    #[test]
    fn test_alternative_conditions_are_parsed() {
        let parsed = alternative("phase>=2&order<=3", "").unwrap();
        assert_eq!(parsed.condition, "phase>=2&order<=3".parse().unwrap());
        assert_eq!(parsed.precondition, None);

        let parsed = alternative("phase>=2", "is_overtake==1").unwrap();
        assert_eq!(parsed.precondition, Some("is_overtake==1".parse().unwrap()));
    }

    #[test]
    fn test_invalid_alternative_conditions_are_errors() {
        // Empty conditions are not allowed.
        assert!(alternative("", "").is_err());
        assert!(alternative("phase>=", "").is_err());
        assert!(alternative("phase>=2&", "").is_err());
        assert!(alternative("phase>=2", "order<=").is_err());
    }

    #[test]
    fn test_skill_data_loads() {
        // Every condition in the skill data must parse.
        let skills = Skill::load_skills_from_file("data/skill_data.json").unwrap();
        assert!(!skills.is_empty());
    }

    #[test]
    fn test_always_condition_activates_out_of_gate() {
        let skill = |condition: &str| {
            Skill::new(
                "test".to_string(),
                SkillData {
                    rarity: SkillRarity::White,
                    alternatives: vec![alternative(condition, "").unwrap()],
                    popularity: None,
                    wisdom_check: false,
                },
            )
        };
        assert!(skill("always==1").should_activate_out_of_gate());
        assert!(!skill("always==1&phase>=2").should_activate_out_of_gate());
        assert!(!skill("phase>=2").should_activate_out_of_gate());
    }
}
