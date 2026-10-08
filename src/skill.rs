//! Skill-related data structures and logic for race simulation.

use num_enum::TryFromPrimitive;
use serde::{Deserialize, Deserializer, de::Error};
use serde_repr::Deserialize_repr;
use serde_with::{BoolFromInt, serde_as};
use std::collections::HashMap;
use std::sync::Arc;

use crate::condition::parser::Condition;
use crate::condition::variables::{ActivationVar, VarType};
use crate::uma::Style;
use crate::units::deserialize_ten_thousandths;

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
    /// (Re-)activate a unique skill.
    ActivateUnique = 49,
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

    /// Check if this effect type modifies a parameter that is needed before the
    /// race starts (stats, start delay, running style, rush chance, etc.).
    pub fn is_pre_race_effect(self) -> bool {
        self.is_stat_boost()
            || self.is_start_delay_modification()
            || self.style_change().is_some()
            || self.is_rush_chance_effect()
            || matches!(self, SkillType::ActivateSympathy)
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

/// Represents the target of a skill effect, along with the parameter that
/// narrows it down (number of runners, running style...).
///
/// As far as targeting is concerned, skills don't distinguish allies from
/// opponents: allies can be selected and count towards the maximum number of
/// targets. However, allies are excluded from debuffs afterwards (see
/// [`SkillTarget::is_opponent_only`]).
#[derive(Copy, Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(try_from = "RawSkillTarget")]
pub enum SkillTarget {
    /// Skill affects oneself.
    Oneself,
    /// Skill affects all runners, oneself included.
    All,
    /// Skill affects runners inside field of view, up to `max` of them.
    InFov { max: u8 },
    /// Skill affects runners placed ahead of `position` (e.g. 1st to 4th place
    /// for position 5).
    AheadOfPosition { position: u8 },
    /// Skill affects the `count` nearest runners ahead.
    AheadOfSelf { count: u8 },
    /// Skill affects the `count` nearest runners behind.
    BehindSelf { count: u8 },
    /// Skill affects all team allies, oneself excluded.
    AllAllies,
    /// Skill affects all opponents with a specific running style.
    EnemyStrategy { style: Style },
    /// Skill affects the `max` nearest rushed runners ahead.
    RushedAhead { max: u8 },
    /// Skill affects the `max` nearest rushed runners behind.
    RushedBehind { max: u8 },
    /// Skill affects rushed runners with a specific running style.
    RushedStrategy { style: Style },
    /// Unknown. Not used by any skill.
    UmaId { value: i32 },
    /// Skill affects runners that have activated recovery skills.
    ///
    /// The meaning of `value` is unknown (always 1).
    UsedRecovery { value: i32 },
}

impl SkillTarget {
    /// Check if this target can only select opponents (i.e. allies are excluded
    /// after selection).
    ///
    /// Every target other than oneself, all runners and team allies is a debuff
    /// target. Note that a debuff isn't always a negative modifier (e.g. rush time
    /// extension).
    pub fn is_opponent_only(self) -> bool {
        !matches!(
            self,
            SkillTarget::Oneself | SkillTarget::All | SkillTarget::AllAllies
        )
    }
}

/// Skill target as stored in the skill data, before being converted to a
/// [`SkillTarget`].
#[derive(Deserialize)]
struct RawSkillTarget {
    #[serde(rename = "type")]
    target_type: u8,
    value: i32,
}

impl TryFrom<RawSkillTarget> for SkillTarget {
    type Error = String;

    fn try_from(raw: RawSkillTarget) -> Result<Self, Self::Error> {
        let RawSkillTarget { target_type, value } = raw;
        let count = || u8::try_from(value).map_err(|_| format!("invalid target count {value}"));
        let style = || match value {
            1 => Ok(Style::Front),
            2 => Ok(Style::Pace),
            3 => Ok(Style::Late),
            4 => Ok(Style::End),
            _ => Err(format!("invalid target style {value}")),
        };
        Ok(match target_type {
            1 => SkillTarget::Oneself,
            2 => SkillTarget::All,
            4 => SkillTarget::InFov { max: count()? },
            7 => SkillTarget::AheadOfPosition { position: count()? },
            9 => SkillTarget::AheadOfSelf { count: count()? },
            10 => SkillTarget::BehindSelf { count: count()? },
            11 => SkillTarget::AllAllies,
            18 => SkillTarget::EnemyStrategy { style: style()? },
            19 => SkillTarget::RushedAhead { max: count()? },
            20 => SkillTarget::RushedBehind { max: count()? },
            21 => SkillTarget::RushedStrategy { style: style()? },
            22 => SkillTarget::UmaId { value },
            23 => SkillTarget::UsedRecovery { value },
            _ => return Err(format!("unknown target type {target_type}")),
        })
    }
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
    ///
    /// Stored in 10000ths in the skill data, and converted when deserializing.
    #[serde(deserialize_with = "deserialize_ten_thousandths")]
    pub modifier: f64,
}

/// Convert a base duration from 10000ths of a second while deserializing. A
/// negative base duration (-1) means that effects last until the end.
fn deserialize_base_duration<'de, D: Deserializer<'de>>(deserializer: D) -> Result<f64, D::Error> {
    let duration = deserialize_ten_thousandths(deserializer)?;
    Ok(if duration < 0.0 {
        f64::INFINITY
    } else {
        duration
    })
}

/// Base cooldown in seconds used by most skills, long enough that they never
/// reactivate.
pub const DEFAULT_COOLDOWN: f64 = 500.0;

/// ID of the "Sympathy" skill, which skills with an
/// [`SkillType::ActivateSympathy`] effect activate for every runner.
pub const SYMPATHY_ID: &str = "201631";

fn default_cooldown() -> f64 {
    DEFAULT_COOLDOWN
}

/// Represents an alternative version of a skill, with its own conditions and
/// effects.
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillAlternative {
    /// Base cooldown duration in seconds.
    ///
    /// Stored in 10000ths of a second in the skill data, and converted when
    /// deserializing.
    #[serde(
        default = "default_cooldown",
        deserialize_with = "deserialize_ten_thousandths"
    )]
    pub base_cooldown: f64,
    /// Base duration in seconds before any modifiers, or infinity if effects last
    /// until the end of the race.
    ///
    /// Stored in 10000ths of a second in the skill data (-1 for the end of the
    /// race), and converted when deserializing.
    #[serde(deserialize_with = "deserialize_base_duration")]
    pub base_duration: f64,
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

impl SkillAlternative {
    /// Check if this alternative is a continuation of another one of the same
    /// skill, i.e. it requires another alternative to have activated
    /// (`is_activate_other_skill_detail`).
    ///
    /// Unlike other alternatives, a continuation can still activate after another
    /// alternative did.
    pub fn is_continuation(&self) -> bool {
        let requires_other = |condition: &Condition| {
            condition
                .comp_tree
                .or_branches()
                .iter()
                .flatten()
                .any(|comp| {
                    comp.var == VarType::Activation(ActivationVar::IsActivateOtherSkillDetail)
                })
        };
        requires_other(&self.condition) || self.precondition.as_ref().is_some_and(requires_other)
    }
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

    /// Determine whether a skill effect is positive or negative.
    pub fn is_effect_positive(effect: &SkillEffect) -> bool {
        match effect.skill_type {
            // Purely positive effects.
            SkillType::ActivateRandomGold
            | SkillType::ActivateUnique
            | SkillType::ActivateSympathy
            | SkillType::ChangeStyleRunaway => true,
            // When a start delay is set, it's always to a bad start.
            SkillType::SetStartDelay => false,
            // Multiplier of less than or equal to 1.0 is a reduction in delay.
            SkillType::MultiplyStartDelay => effect.modifier <= 1.0,
            // Less chance or shorter rush time is good.
            SkillType::RushChanceUp | SkillType::RushTimeUp => effect.modifier <= 0.0,
            // Positive modifier is good for other effects.
            _ => effect.modifier >= 0.0,
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
    ///
    /// That is, if any of its effects modifies a parameter that is needed before
    /// the race starts (see [`SkillType::is_pre_race_effect`]).
    pub fn should_activate_out_of_gate(&self) -> bool {
        self.alternatives
            .iter()
            .flat_map(|alternative| &alternative.effects)
            .any(|effect| effect.skill_type.is_pre_race_effect())
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
    use crate::test_utils::{self, assert_close, skill_data};

    fn skill_with_wisdom_check(wisdom_check: bool) -> Skill {
        let data = SkillData {
            wisdom_check,
            ..skill_data(vec![])
        };
        Skill::new("test".to_string(), data)
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
        assert_close!(skill.activation_chance(900), 0.9);
        // Low wisdom is clamped to the minimum chance.
        assert_eq!(skill.activation_chance(100), 0.2);
        assert_eq!(skill.activation_chance(0), 0.2);
    }

    #[test]
    fn test_wiz_limit_break_eligibility() {
        let skill = |rarity| {
            let data = SkillData {
                rarity,
                ..skill_data(vec![])
            };
            Skill::new("test".to_string(), data)
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
                "effects": [{{"type": 27, "target": {{"type": 1, "value": 0}}, "modifier": 3500}}],
                "precondition": "{precondition}"}}"#
        );
        serde_json::from_str(&json)
    }

    #[test]
    fn test_alternative_values_are_converted_from_ten_thousandths() {
        let parsed = alternative("phase>=2", "").unwrap();
        assert_eq!(parsed.base_duration, 3.0);
        assert_eq!(parsed.effects[0].modifier, 0.35);
        // Missing cooldown.
        assert_eq!(parsed.base_cooldown, DEFAULT_COOLDOWN);

        let json = r#"{"baseCooldown": 300000, "baseDuration": -1, "condition": "always==1",
            "effects": [], "precondition": ""}"#;
        let parsed: SkillAlternative = serde_json::from_str(json).unwrap();
        assert_eq!(parsed.base_cooldown, 30.0);
        // Until the end of the race.
        assert_eq!(parsed.base_duration, f64::INFINITY);
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
    fn test_continuation_requires_other_alternative() {
        assert!(!alternative("phase>=2", "").unwrap().is_continuation());
        assert!(
            alternative("phase>=2@is_activate_other_skill_detail==1", "")
                .unwrap()
                .is_continuation()
        );
        // Also in the precondition.
        assert!(
            alternative("phase>=2", "is_activate_other_skill_detail==1")
                .unwrap()
                .is_continuation()
        );
    }

    /// Deserialize a skill target from its type and value.
    fn target(target_type: u8, value: i32) -> serde_json::Result<SkillTarget> {
        serde_json::from_str(&format!(r#"{{"type": {target_type}, "value": {value}}}"#))
    }

    #[test]
    fn test_target_encodes_value() {
        assert_eq!(target(1, 0).unwrap(), SkillTarget::Oneself);
        assert_eq!(target(4, 18).unwrap(), SkillTarget::InFov { max: 18 });
        assert_eq!(
            target(7, 5).unwrap(),
            SkillTarget::AheadOfPosition { position: 5 }
        );
        assert_eq!(target(9, 3).unwrap(), SkillTarget::AheadOfSelf { count: 3 });
        assert_eq!(target(10, 5).unwrap(), SkillTarget::BehindSelf { count: 5 });
        assert_eq!(
            target(18, 1).unwrap(),
            SkillTarget::EnemyStrategy {
                style: Style::Front
            }
        );
        assert_eq!(
            target(21, 4).unwrap(),
            SkillTarget::RushedStrategy { style: Style::End }
        );
        assert_eq!(
            target(20, 10).unwrap(),
            SkillTarget::RushedBehind { max: 10 }
        );
    }

    #[test]
    fn test_invalid_targets_are_errors() {
        // Unknown target type.
        assert!(target(3, 0).is_err());
        // Runaway (0) isn't a valid target style.
        assert!(target(18, 0).is_err());
        // Negative count.
        assert!(target(9, -1).is_err());
    }

    #[test]
    fn test_skill_data_loads() {
        // Every condition in the skill data must parse.
        let skills = Skill::load_skills_from_file("data/skill_data.json").unwrap();
        assert!(!skills.is_empty());
    }

    #[test]
    fn test_pre_race_effects_activate_out_of_gate() {
        let skill = |skill_type| {
            let alternatives = vec![test_utils::alternative("running_style==1", skill_type, 0.0)];
            Skill::new("test".to_string(), skill_data(alternatives))
        };
        assert!(skill(SkillType::SpeedUp).should_activate_out_of_gate());
        assert!(skill(SkillType::MultiplyStartDelay).should_activate_out_of_gate());
        assert!(skill(SkillType::ChangeStyleRunaway).should_activate_out_of_gate());
        assert!(skill(SkillType::RushChanceUp).should_activate_out_of_gate());
        assert!(skill(SkillType::ActivateSympathy).should_activate_out_of_gate());
        // The condition doesn't matter, only the effects.
        assert!(!skill(SkillType::TargetSpeed).should_activate_out_of_gate());
        assert!(!skill(SkillType::Acceleration).should_activate_out_of_gate());
    }
}
