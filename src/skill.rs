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
    /// Add seconds to start delay.
    AddStartDelay = 14,
    /// Modify current speed directly. The speed drops instantly on expiry.
    CurrentSpeed = 21,
    /// Modify current speed but allow for natural deceleration.
    ///
    /// When activated, it behaves like [`SkillType::CurrentSpeed`]. On expiry, the
    /// same amount is added once to the runner's speed, so that it decelerates
    /// naturally instead of dropping instantly.
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
            SkillType::MultiplyStartDelay | SkillType::AddStartDelay
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

/// Represents how the value of a skill effect scales with the runner's situation
/// when the skill activates (see [`crate::race::skill_scaling`]).
#[derive(Copy, Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(try_from = "u8")]
pub enum ValueUsage {
    /// No scaling (1).
    Direct,
    /// Scales with the number of skills the runner has (2).
    MultiplySkillNum,
    /// Aoharu (Unity Cup) skills; scales with the team's total base stats in the
    /// scenario (3-7).
    Aoharu,
    /// Random multiplier, first variant (8).
    ///
    /// Both variants currently have the same multipliers, but they are separate
    /// IDs in the game data, so they're kept apart in case an update changes
    /// either of them.
    MultiplyRandom1,
    /// Random multiplier, second variant (9). See [`ValueUsage::MultiplyRandom1`].
    MultiplyRandom2,
    /// Climax skills; scales with the number of races won during training (10).
    Climax,
    /// Scales with the highest of the runner's raw stats (13).
    MultiplyMaximumRawStatus,
    /// Scales with the number of green skills activated by the runner (14).
    MultiplyActivateSpecificTagSkillCount,
    /// Adds a flat amount when far enough from the first place (19).
    AddDistanceDiffTop,
    /// Scales with the longest time the runner was side-blocked during the Mid
    /// race (20).
    MultiplyBlockedSideMaxContinueTimePhaseMiddleRun,
    /// Scales with the runner's adjusted speed, first variant (22).
    MultiplySpeed1,
    /// Scales with the runner's adjusted speed, second variant (23).
    MultiplySpeed2,
    /// Scales with the total level of global potential in the L'Arc scenario
    /// (24).
    MultiplyArcGlobalPotentialLevel,
    /// Scales with the largest lead the runner took before the Late race (25).
    MultiplyTopLeadAmount,
    /// Scales with the number of overtakes in corners during the Late race or
    /// later (11).
    CornerOvertakeCount,
    /// Scales with the runner's fan count (12).
    FanCount,
    /// Scales with the number of U.A.F. tournament wins (26).
    UafWinCount,
    /// Scales with the cooking points earned in the Great Food Festival scenario
    /// (27).
    CookingPoints,
    /// Scales with the total research level earned in the Mecha Umamusume
    /// scenario (28).
    ResearchLevel,
    /// Scales with the love received from fans, up to 3x (30).
    FanLove,
    /// Scales with the total development points earned in the Design Your Island
    /// scenario (31).
    DevelopmentPoints,
}

impl TryFrom<u8> for ValueUsage {
    type Error = String;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        use ValueUsage::*;
        Ok(match value {
            1 => Direct,
            2 => MultiplySkillNum,
            3..=7 => Aoharu,
            8 => MultiplyRandom1,
            9 => MultiplyRandom2,
            10 => Climax,
            11 => CornerOvertakeCount,
            12 => FanCount,
            13 => MultiplyMaximumRawStatus,
            14 => MultiplyActivateSpecificTagSkillCount,
            19 => AddDistanceDiffTop,
            20 => MultiplyBlockedSideMaxContinueTimePhaseMiddleRun,
            22 => MultiplySpeed1,
            23 => MultiplySpeed2,
            24 => MultiplyArcGlobalPotentialLevel,
            25 => MultiplyTopLeadAmount,
            26 => UafWinCount,
            27 => CookingPoints,
            28 => ResearchLevel,
            30 => FanLove,
            31 => DevelopmentPoints,
            _ => return Err(format!("unknown value usage {value}")),
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
    /// How the modifier scales when the skill activates.
    pub value_usage: ValueUsage,
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

/// Represents how the duration of a skill scales with the runner's situation
/// when the skill activates (see [`crate::race::skill_scaling`]).
///
/// The scaling is applied on top of the course distance scaling.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(try_from = "u8")]
pub enum TimeUsage {
    /// No scaling (1).
    Direct,
    /// Scales with the distance behind the first place (2).
    MultiplyDistanceDiffTop,
    /// Scales with the remaining HP, first variant (3).
    MultiplyRemainHp1,
    /// Extended each time the runner overtakes while the effects are active, up
    /// to a few times (4).
    IncrementOrderUp,
    /// Scales with the longest time the runner was side-blocked during the Mid
    /// race (5).
    MultiplyBlockedSideMaxContinueTimePhaseMiddleRun,
    /// Scales with the remaining HP, second variant (7).
    MultiplyRemainHp2,
    /// Extended for each uphill climbed before activation, up to a few times (8).
    IncrementUphillCount,
}

impl TryFrom<u8> for TimeUsage {
    type Error = String;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        use TimeUsage::*;
        Ok(match value {
            1 => Direct,
            2 => MultiplyDistanceDiffTop,
            3 => MultiplyRemainHp1,
            4 => IncrementOrderUp,
            5 => MultiplyBlockedSideMaxContinueTimePhaseMiddleRun,
            7 => MultiplyRemainHp2,
            8 => IncrementUphillCount,
            _ => return Err(format!("unknown time usage {value}")),
        })
    }
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
    /// How the duration scales when the skill activates.
    pub time_usage: TimeUsage,
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
            // Less start delay is good.
            SkillType::AddStartDelay => effect.modifier <= 0.0,
            // Multiplier of less than or equal to 1.0 is a reduction in delay.
            SkillType::MultiplyStartDelay => effect.modifier <= 1.0,
            // Less chance or shorter rush time is good.
            SkillType::RushChanceUp | SkillType::RushTimeUp => effect.modifier <= 0.0,
            // Positive modifier is good for other effects.
            _ => effect.modifier >= 0.0,
        }
    }

    /// Check if `self` is a unique skill (i.e. only available to a specific Uma).
    pub fn is_unique(self: &Skill) -> bool {
        matches!(
            self.rarity,
            SkillRarity::Unique | SkillRarity::UniqueUpgraded | SkillRarity::UniqueNatural
        )
    }

    /// Check if `self` is a green skill, i.e. one that boosts stats.
    pub fn is_green(&self) -> bool {
        // TODO: The game defines green skills by tags (601-615), which aren't in the
        // skill data yet.
        self.alternatives
            .iter()
            .flat_map(|alternative| &alternative.effects)
            .any(|effect| effect.skill_type.is_stat_boost())
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
                "effects": [{{"type": 27, "target": {{"type": 1, "value": 0}}, "modifier": 3500,
                    "valueUsage": 1}}],
                "precondition": "{precondition}", "timeUsage": 1}}"#
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
            "effects": [], "precondition": "", "timeUsage": 1}"#;
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
        assert_eq!(target(2, 0).unwrap(), SkillTarget::All);
        assert_eq!(target(11, 0).unwrap(), SkillTarget::AllAllies);
        assert_eq!(target(19, 2).unwrap(), SkillTarget::RushedAhead { max: 2 });
        assert_eq!(
            target(22, 1001).unwrap(),
            SkillTarget::UmaId { value: 1001 }
        );
        assert_eq!(
            target(23, 9).unwrap(),
            SkillTarget::UsedRecovery { value: 9 }
        );
    }

    #[test]
    fn test_invalid_targets_are_errors() {
        // Unknown target type.
        assert!(target(3, 0).is_err());
        // Runaway (0) isn't a valid target style.
        assert!(target(18, 0).is_err());
        // Negative or too large counts.
        for target_type in [4, 7, 9, 10, 19, 20] {
            assert!(target(target_type, -1).is_err(), "type {target_type}");
            assert!(target(target_type, 256).is_err(), "type {target_type}");
        }
        // Unknown style.
        for target_type in [18, 21] {
            assert!(target(target_type, 5).is_err(), "type {target_type}");
        }
    }

    #[test]
    fn test_is_effect_positive() {
        let cases = [
            // Purely positive.
            (SkillType::ActivateRandomGold, 0.0, true),
            (SkillType::ChangeStyleRunaway, 0.0, true),
            // Less start delay is good.
            (SkillType::AddStartDelay, -0.1, true),
            (SkillType::AddStartDelay, 0.1, false),
            (SkillType::MultiplyStartDelay, 0.5, true),
            (SkillType::MultiplyStartDelay, 1.5, false),
            // Less rush chance or time is good.
            (SkillType::RushChanceUp, -0.1, true),
            (SkillType::RushChanceUp, 0.1, false),
            (SkillType::RushTimeUp, -0.1, true),
            (SkillType::RushTimeUp, 0.1, false),
            // Anything else is good when positive.
            (SkillType::TargetSpeed, 0.1, true),
            (SkillType::TargetSpeed, -0.1, false),
        ];
        for (skill_type, modifier, expected) in cases {
            let effect = SkillEffect {
                skill_type,
                target: SkillTarget::Oneself,
                modifier,
                value_usage: ValueUsage::Direct,
            };
            let actual = Skill::is_effect_positive(&effect);
            assert_eq!(actual, expected, "{skill_type:?} with {modifier}");
        }
    }

    /// Deserialize a value usage from its ID.
    fn value_usage(id: u8) -> serde_json::Result<ValueUsage> {
        serde_json::from_str(&id.to_string())
    }

    #[test]
    fn test_value_usage_ids() {
        assert_eq!(value_usage(1).unwrap(), ValueUsage::Direct);
        assert_eq!(value_usage(2).unwrap(), ValueUsage::MultiplySkillNum);
        // Several IDs share the same scaling.
        for id in 3..=7 {
            assert_eq!(value_usage(id).unwrap(), ValueUsage::Aoharu, "ID {id}");
        }
        assert_eq!(value_usage(8).unwrap(), ValueUsage::MultiplyRandom1);
        assert_eq!(value_usage(9).unwrap(), ValueUsage::MultiplyRandom2);
        assert_eq!(value_usage(22).unwrap(), ValueUsage::MultiplySpeed1);
        assert_eq!(value_usage(23).unwrap(), ValueUsage::MultiplySpeed2);
        assert_eq!(value_usage(25).unwrap(), ValueUsage::MultiplyTopLeadAmount);
        // Unknown IDs.
        for id in [0, 15, 21, 29, 32] {
            assert!(value_usage(id).is_err(), "ID {id}");
        }
    }

    /// Deserialize a time usage from its ID.
    fn time_usage(id: u8) -> serde_json::Result<TimeUsage> {
        serde_json::from_str(&id.to_string())
    }

    #[test]
    fn test_time_usage_ids() {
        let cases = [
            (1, TimeUsage::Direct),
            (2, TimeUsage::MultiplyDistanceDiffTop),
            (3, TimeUsage::MultiplyRemainHp1),
            (4, TimeUsage::IncrementOrderUp),
            (
                5,
                TimeUsage::MultiplyBlockedSideMaxContinueTimePhaseMiddleRun,
            ),
            (7, TimeUsage::MultiplyRemainHp2),
            (8, TimeUsage::IncrementUphillCount),
        ];
        for (id, expected) in cases {
            assert_eq!(time_usage(id).unwrap(), expected, "ID {id}");
        }
        // Unknown IDs.
        for id in [0, 6, 9] {
            assert!(time_usage(id).is_err(), "ID {id}");
        }
    }

    #[test]
    fn test_is_green() {
        let skill = |skill_type| {
            let alternatives = vec![test_utils::alternative("always==1", skill_type, 40.0)];
            Skill::new("test".to_string(), skill_data(alternatives))
        };
        assert!(skill(SkillType::SpeedUp).is_green());
        assert!(skill(SkillType::AllStatsUp).is_green());
        assert!(!skill(SkillType::TargetSpeed).is_green());
        assert!(!skill(SkillType::MultiplyStartDelay).is_green());
    }

    #[test]
    fn test_skill_data_loads() {
        // Every condition in the skill data must parse.
        let skills = Skill::load_skills_from_file("data/skill_data.json").unwrap();
        assert!(!skills.is_empty());

        // Pre-race effects are applied before the race, where values aren't
        // scaled.
        let effects = skills
            .values()
            .flat_map(|skill| &skill.alternatives)
            .flat_map(|alternative| &alternative.effects);
        for effect in effects {
            if effect.skill_type.is_pre_race_effect() {
                assert_eq!(effect.value_usage, ValueUsage::Direct, "{effect:?}");
            }
        }
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
