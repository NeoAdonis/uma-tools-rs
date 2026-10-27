//! Skill-related data structures and logic for race simulation.

use num_enum::TryFromPrimitive;
use serde::{Deserialize, Deserializer, de::Error};
use serde_repr::Deserialize_repr;
use serde_with::{BoolFromInt, serde_as};
use std::collections::HashMap;
use std::path::Path;
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
    /// Modify wit stat.
    WitUp = 5,
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
    /// Modify acceleration during the all-out spurt (5th Anniversary onward).
    AccelerationAllOutSpurt = 48,
    /// (Re-)activate a unique skill.
    ActivateUnique = 49,
    /// Satisfy every popularity requirement: comparisons on `popularity` are
    /// always true. The modifier is ignored.
    IgnorePopularity = 50,
}

impl SkillType {
    /// Get the skill type with the given ID, if it's implemented.
    pub fn from_id(id: i32) -> Option<Self> {
        u8::try_from(id).ok()?.try_into().ok()
    }

    /// Check if this effect type is a stat boost (applied permanently).
    pub fn is_stat_boost(self) -> bool {
        use SkillType::*;
        matches!(
            self,
            SpeedUp | StaminaUp | PowerUp | GutsUp | WitUp | AllStatsUp
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
            || matches!(
                self,
                SkillType::ActivateSympathy | SkillType::IgnorePopularity
            )
    }

    /// Check if this effect type is a timed effect.
    pub fn is_timed_effect(self) -> bool {
        use SkillType::*;
        matches!(
            self,
            Acceleration
                | AccelerationAllOutSpurt
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
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Deserialize_repr)]
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
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Deserialize)]
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
    /// Skill affects a random team ally, oneself included.
    RandomAlly,
}

impl SkillTarget {
    /// Check if this target can only select opponents (i.e. allies are excluded
    /// after selection).
    ///
    /// Every target other than oneself, all runners and team members is a debuff
    /// target. Note that a debuff isn't always a negative modifier (e.g. rush time
    /// extension).
    pub fn is_opponent_only(self) -> bool {
        !matches!(
            self,
            SkillTarget::Oneself
                | SkillTarget::All
                | SkillTarget::AllAllies
                | SkillTarget::RandomAlly
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
            24 => SkillTarget::RandomAlly,
            _ => return Err(format!("unknown target type {target_type}")),
        })
    }
}

/// Represents how the value of a skill effect scales with the runner's
/// situation when the skill activates (see [`crate::race::skill_scaling`]).
///
/// Each variant's discriminant is its ID in the skill data.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Deserialize_repr, TryFromPrimitive)]
#[repr(u8)]
pub enum ValueScaling {
    /// No scaling.
    Direct = 1,
    /// Scales with the number of skills the runner has.
    MultiplySkillNum = 2,
    /// Unity Cup skills; scales with the team's total speed in the scenario.
    UnityCupSpeed = 3,
    /// Unity Cup skills; scales with the team's total stamina in the scenario.
    UnityCupStamina = 4,
    /// Unity Cup skills; scales with the team's total power in the scenario.
    UnityCupPower = 5,
    /// Unity Cup skills; scales with the team's total guts in the scenario.
    UnityCupGuts = 6,
    /// Unity Cup skills; scales with the team's total wit in the scenario.
    UnityCupWit = 7,
    /// Random multiplier, first variant.
    ///
    /// Both variants currently have the same multipliers, but they are separate
    /// IDs in the skill data, so they're kept apart in case an update changes
    /// either of them.
    MultiplyRandom1 = 8,
    /// Random multiplier, second variant. See [`ValueScaling::MultiplyRandom1`].
    MultiplyRandom2 = 9,
    /// Climax skills; scales with the number of races won during training.
    Climax = 10,
    /// Scales with the number of overtakes in corners during the late race or
    /// later.
    CornerOvertakeCount = 11,
    /// Scales with the runner's fan count.
    FanCount = 12,
    /// Scales with the highest of the runner's raw stats.
    MultiplyMaximumRawStatus = 13,
    /// Scales with the number of green skills activated by the runner.
    MultiplyActivateSpecificTagSkillCount = 14,
    /// Adds a flat amount when far enough from the first place.
    AddDistanceDiffTop = 19,
    /// Scales with the longest time the runner was side-blocked during the Mid
    /// race.
    MultiplyBlockedSideMaxTimeMidRace = 20,
    /// Scales with the runner's adjusted speed, first variant.
    MultiplySpeed1 = 22,
    /// Scales with the runner's adjusted speed, second variant.
    MultiplySpeed2 = 23,
    /// Scales with the total level of global potential in the L'Arc scenario.
    MultiplyArcGlobalPotentialLevel = 24,
    /// Scales with the largest lead the runner took before the late race.
    MultiplyTopLeadAmount = 25,
    /// Scales with the number of U.A.F. tournament wins.
    UafWinCount = 26,
    /// Scales with the cooking points earned in the Great Food Festival scenario.
    CookingPoints = 27,
    /// Scales with the total research level earned in the Mecha Umamusume
    /// scenario.
    ResearchLevel = 28,
    /// Scales with the love received from fans, up to 3x.
    FanLove = 30,
    /// Scales with the total development points earned in the Design Your Island
    /// scenario.
    DevelopmentPoints = 31,
    /// Scales with whether the Legendary Inn status was achieved in the Yukoma
    /// Onsen scenario.
    OnsenLegendaryInn = 32,
    /// Adds a flat amount when far enough (in bashin) from the first place.
    AddBashinDiffTop = 34,
    /// "Cinderella Gray" Oguri Cap's unique skill; adds a flat amount in Mile
    /// races, or on the Arima Kinen course with enough fans.
    CinderellaGray = 35,
    /// Adds a flat amount in Medium races.
    AddDistanceTypeMedium = 36,
    /// Scales with the team rank.
    TeamRank = 37,
    /// Scales with the Excitement Points earned in the Tracen-ken scenario.
    ExcitementPoints = 40,
}

/// Represents when a skill effect applies: on activation, or later, each time
/// an event happens while the skill's effects are active (追加効果).
///
/// Additional effects apply their value once per event, up to a maximum number
/// of times (see [`AdditionalActivation::max_triggers`]), and expire with the
/// skill's other effects.
///
/// Each variant's discriminant is its ID in the skill data.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash, Deserialize_repr, TryFromPrimitive)]
#[repr(u8)]
pub enum AdditionalActivation {
    /// Applies on activation.
    #[default]
    Regular = 0,
    /// Applies each time the runner overtakes.
    OrderUp = 1,
    /// Applies each time the runner activates another skill, first variant.
    ActivateAnySkill1 = 2,
    /// Applies each time the runner activates another skill, second variant.
    ActivateAnySkill2 = 3,
}

impl AdditionalActivation {
    /// Maximum number of times an additional effect applies, or 0 for regular
    /// effects.
    pub fn max_triggers(self) -> u8 {
        match self {
            AdditionalActivation::Regular => 0,
            AdditionalActivation::OrderUp => 3,
            AdditionalActivation::ActivateAnySkill1 => 3,
            AdditionalActivation::ActivateAnySkill2 => 2,
        }
    }
}

/// Represents a single effect of a skill.
#[derive(Clone, Debug, Deserialize, PartialEq)]
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
    pub value_scaling: ValueScaling,
    /// When the effect applies. Regular (on activation) if missing.
    #[serde(default)]
    pub additional: AdditionalActivation,
}

impl SkillEffect {
    /// Determine whether `self` is positive or negative.
    pub fn is_positive(&self) -> bool {
        match self.skill_type {
            // Purely positive effects.
            SkillType::ActivateRandomGold
            | SkillType::ActivateUnique
            | SkillType::ActivateSympathy
            | SkillType::ChangeStyleRunaway => true,
            // Less start delay is good.
            SkillType::AddStartDelay => self.modifier <= 0.0,
            // Multiplier of less than or equal to 1.0 is a reduction in delay.
            SkillType::MultiplyStartDelay => self.modifier <= 1.0,
            // Less chance or shorter rush time is good.
            SkillType::RushChanceUp | SkillType::RushTimeUp => self.modifier <= 0.0,
            // Positive modifier is good for other effects.
            _ => self.modifier >= 0.0,
        }
    }
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
pub const SYMPATHY_ID: i32 = 201631;

fn default_cooldown() -> f64 {
    DEFAULT_COOLDOWN
}

/// Represents how the duration of a skill scales with the runner's situation
/// when the skill activates (see [`crate::race::skill_scaling`]).
///
/// The scaling is applied on top of the course distance scaling.
///
/// Each variant's discriminant is its ID in the skill data.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Deserialize_repr, TryFromPrimitive)]
#[repr(u8)]
pub enum DurationScaling {
    /// No scaling.
    Direct = 1,
    /// Scales with the distance behind the first place.
    MultiplyDistanceDiffTop = 2,
    /// Scales with the remaining HP, first variant.
    MultiplyRemainHp1 = 3,
    /// Extended each time the runner overtakes while the effects are active, up
    /// to a few times.
    IncrementOrderUp = 4,
    /// Scales with the longest time the runner was side-blocked during the Mid
    /// race.
    MultiplyBlockedSideMaxContinueTimePhaseMiddleRun = 5,
    /// Scales with the remaining HP, second variant.
    MultiplyRemainHp2 = 7,
    /// Extended for each uphill climbed before activation, up to a few times.
    IncrementUphillCount = 8,
}

/// Represents an alternative version of a skill, with its own conditions and
/// effects.
#[derive(Clone, Debug, Deserialize, PartialEq)]
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
    pub duration_scaling: DurationScaling,
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
            condition.branches.iter().flatten().any(|comp| {
                comp.var == VarType::Activation(ActivationVar::IsActivateOtherSkillDetail)
            })
        };
        requires_other(&self.condition) || self.precondition.as_ref().is_some_and(requires_other)
    }

    /// Check if this alternative activates out of gate (before the race begins).
    ///
    /// That is, if any of its effects modifies a parameter that is needed before
    /// the race starts (see [`SkillType::is_pre_race_effect`]). Other alternatives
    /// of the same skill (e.g. continuations) activate during the race.
    pub fn is_out_of_gate(&self) -> bool {
        self.effects
            .iter()
            .any(|effect| effect.skill_type.is_pre_race_effect())
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
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Deserialize_repr)]
#[repr(u8)]
pub enum PopularityStat {
    None = 0,
    Speed = 1,
    Stamina = 2,
    Power = 3,
    Guts = 4,
    Wit = 5,
}

/// Represents the popularity points granted by a skill to a specific stat.
#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SkillPopularity {
    /// Stat that receives popularity points.
    pub stat: PopularityStat,
    /// Amount of popularity points granted (negative for negative skills).
    pub value: i8,
}

/// Represents a skill with its ID and data.
///
/// Skills are shared as `Arc<Skill>`, so that cloning them for every runner
/// of every race is cheap.
#[serde_as]
#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Skill {
    /// Skill ID.
    ///
    /// Not part of each skill's data, but the key of the skill in the skill
    /// data. Set when loading the skill.
    #[serde(skip)]
    pub id: i32,
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
    /// Whether this skill requires a wit check to activate.
    #[serde_as(as = "BoolFromInt")]
    pub wit_check: bool,
}

impl Skill {
    /// Check if `self` is a unique skill (i.e. only available to a specific Uma).
    pub fn is_unique(&self) -> bool {
        matches!(
            self.rarity,
            SkillRarity::Unique | SkillRarity::UniqueUpgraded | SkillRarity::UniqueNatural
        )
    }

    /// Check if `self` is a gold skill, including evolved ones.
    pub fn is_gold(&self) -> bool {
        matches!(self.rarity, SkillRarity::Gold | SkillRarity::Evolution)
    }

    /// Check if `self` is the inherited version of a unique skill (継承), whose ID
    /// is the unique's ID with its first digit replaced by 9 (e.g. 900011 for
    /// 100011).
    pub fn is_inherited_unique(&self) -> bool {
        (900_000..1_000_000).contains(&self.id)
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

    /// Check if this skill benefits from the wit limit break buff (5th Anniversary
    /// onward).
    ///
    /// Only unique, evolved and gold skills are affected. Inherited uniques
    /// are stored as white skills, so they are excluded as well.
    pub fn is_wit_limit_break_eligible(&self) -> bool {
        !matches!(self.rarity, SkillRarity::White)
    }

    /// Chance that this skill passes its pre-race activation check.
    ///
    /// Skills that don't require a wit check always pass; otherwise the chance
    /// depends on the runner's base wit (minimum 20%).
    pub fn activation_chance(&self, base_wit: u16) -> f64 {
        if !self.wit_check {
            return 1.0;
        }
        (1.0 - 90.0 / base_wit as f64).max(0.2)
    }

    /// Check if this skill should activate out of gate (before race begins).
    ///
    /// That is, if any of its alternatives does (see
    /// [`SkillAlternative::is_out_of_gate`]).
    pub fn should_activate_out_of_gate(&self) -> bool {
        self.alternatives
            .iter()
            .any(SkillAlternative::is_out_of_gate)
    }

    /// Check if any alternative of this skill satisfies every popularity
    /// requirement (see [`SkillType::IgnorePopularity`]).
    pub fn ignores_popularity(&self) -> bool {
        self.alternatives
            .iter()
            .flat_map(|alternative| &alternative.effects)
            .any(|effect| matches!(effect.skill_type, SkillType::IgnorePopularity))
    }

    /// Load all skills from a JSON file into a map of skill ID to [`Skill`] object.
    ///
    /// # Errors
    ///
    /// Returns an error if the file can't be read or the JSON is invalid.
    pub fn load_skills_from_file(
        json_path: impl AsRef<Path>,
    ) -> Result<HashMap<i32, Arc<Skill>>, Box<dyn std::error::Error + Send + Sync>> {
        use std::fs;

        let json_content = fs::read_to_string(json_path)?;
        // JSON keys are strings, but serde parses them as integers.
        let skills: HashMap<i32, Skill> = serde_json::from_str(&json_content)?;

        Ok(skills
            .into_iter()
            .map(|(id, skill)| (id, Arc::new(Skill { id, ..skill })))
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::{self, assert_close, synthetic_skill};

    fn skill_with_wit_check(wit_check: bool) -> Arc<Skill> {
        Arc::new(Skill {
            wit_check,
            ..synthetic_skill(vec![])
        })
    }

    #[test]
    fn test_activation_chance_without_wit_check() {
        let skill = skill_with_wit_check(false);
        assert_eq!(skill.activation_chance(1), 1.0);
        assert_eq!(skill.activation_chance(1200), 1.0);
    }

    #[test]
    fn test_activation_chance_with_wit_check() {
        let skill = skill_with_wit_check(true);
        assert_close!(skill.activation_chance(900), 0.9);
        // Low wit is clamped to the minimum chance.
        assert_eq!(skill.activation_chance(100), 0.2);
        assert_eq!(skill.activation_chance(0), 0.2);
    }

    #[test]
    fn test_wit_limit_break_eligibility() {
        let skill = |rarity| {
            Arc::new(Skill {
                rarity,
                ..synthetic_skill(vec![])
            })
        };
        assert!(!skill(SkillRarity::White).is_wit_limit_break_eligible());
        assert!(skill(SkillRarity::Gold).is_wit_limit_break_eligible());
        assert!(skill(SkillRarity::Unique).is_wit_limit_break_eligible());
        assert!(skill(SkillRarity::UniqueUpgraded).is_wit_limit_break_eligible());
        assert!(skill(SkillRarity::UniqueNatural).is_wit_limit_break_eligible());
        assert!(skill(SkillRarity::Evolution).is_wit_limit_break_eligible());
    }

    /// Deserialize an alternative with the given condition and precondition.
    fn alternative(condition: &str, precondition: &str) -> serde_json::Result<SkillAlternative> {
        let json = format!(
            r#"{{"baseDuration": 30000, "condition": "{condition}",
                "effects": [{{"type": 27, "target": {{"type": 1, "value": 0}}, "modifier": 3500,
                    "valueScaling": 1}}],
                "precondition": "{precondition}", "durationScaling": 1}}"#
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
            "effects": [], "precondition": "", "durationScaling": 1}"#;
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
        assert_eq!(target(24, 0).unwrap(), SkillTarget::RandomAlly);
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
    fn test_effect_is_positive() {
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
            let actual = test_utils::effect(skill_type, modifier).is_positive();
            assert_eq!(actual, expected, "{skill_type:?} with {modifier}");
        }
    }

    /// Deserialize a value scaling from its ID.
    fn value_scaling(id: u8) -> serde_json::Result<ValueScaling> {
        serde_json::from_str(&id.to_string())
    }

    #[test]
    fn test_value_scaling_ids() {
        assert_eq!(value_scaling(1).unwrap(), ValueScaling::Direct);
        assert_eq!(value_scaling(2).unwrap(), ValueScaling::MultiplySkillNum);
        assert_eq!(value_scaling(3).unwrap(), ValueScaling::UnityCupSpeed);
        assert_eq!(value_scaling(7).unwrap(), ValueScaling::UnityCupWit);
        assert_eq!(value_scaling(8).unwrap(), ValueScaling::MultiplyRandom1);
        assert_eq!(value_scaling(9).unwrap(), ValueScaling::MultiplyRandom2);
        assert_eq!(value_scaling(22).unwrap(), ValueScaling::MultiplySpeed1);
        assert_eq!(value_scaling(23).unwrap(), ValueScaling::MultiplySpeed2);
        assert_eq!(
            value_scaling(25).unwrap(),
            ValueScaling::MultiplyTopLeadAmount
        );
        // IDs are preserved.
        assert_eq!(ValueScaling::MultiplyRandom1 as u8, 8);
        assert_eq!(
            ValueScaling::try_from(31).unwrap(),
            ValueScaling::DevelopmentPoints
        );
        assert_eq!(value_scaling(32).unwrap(), ValueScaling::OnsenLegendaryInn);
        assert_eq!(value_scaling(35).unwrap(), ValueScaling::CinderellaGray);
        assert_eq!(
            value_scaling(36).unwrap(),
            ValueScaling::AddDistanceTypeMedium
        );
        assert_eq!(value_scaling(37).unwrap(), ValueScaling::TeamRank);
        assert_eq!(value_scaling(40).unwrap(), ValueScaling::ExcitementPoints);
        // Unknown IDs.
        for id in [0, 15, 21, 29, 33, 38, 41] {
            assert!(value_scaling(id).is_err(), "ID {id}");
        }
    }

    /// Deserialize a duration scaling from its ID.
    fn duration_scaling(id: u8) -> serde_json::Result<DurationScaling> {
        serde_json::from_str(&id.to_string())
    }

    #[test]
    fn test_duration_scaling_ids() {
        let cases = [
            (1, DurationScaling::Direct),
            (2, DurationScaling::MultiplyDistanceDiffTop),
            (3, DurationScaling::MultiplyRemainHp1),
            (4, DurationScaling::IncrementOrderUp),
            (
                5,
                DurationScaling::MultiplyBlockedSideMaxContinueTimePhaseMiddleRun,
            ),
            (7, DurationScaling::MultiplyRemainHp2),
            (8, DurationScaling::IncrementUphillCount),
        ];
        for (id, expected) in cases {
            assert_eq!(duration_scaling(id).unwrap(), expected, "ID {id}");
        }
        // Unknown IDs.
        for id in [0, 6, 9] {
            assert!(duration_scaling(id).is_err(), "ID {id}");
        }
    }

    #[test]
    fn test_additional_activation_ids() {
        let effect = |additional: &str| {
            let json = format!(
                r#"{{"type": 27, "target": {{"type": 1, "value": 0}}, "modifier": 500,
                    "valueScaling": 1{additional}}}"#
            );
            serde_json::from_str::<SkillEffect>(&json).map(|effect| effect.additional)
        };
        // Regular when missing.
        assert_eq!(effect("").unwrap(), AdditionalActivation::Regular);
        let cases = [
            (0, AdditionalActivation::Regular),
            (1, AdditionalActivation::OrderUp),
            (2, AdditionalActivation::ActivateAnySkill1),
            (3, AdditionalActivation::ActivateAnySkill2),
        ];
        for (id, expected) in cases {
            let parsed = effect(&format!(r#", "additional": {id}"#)).unwrap();
            assert_eq!(parsed, expected, "ID {id}");
        }
        assert!(effect(r#", "additional": 4"#).is_err());
    }

    #[test]
    fn test_is_green() {
        let skill = |skill_type| {
            let alternatives = vec![test_utils::alternative("always==1", skill_type, 40.0)];
            Arc::new(synthetic_skill(alternatives))
        };
        assert!(skill(SkillType::SpeedUp).is_green());
        assert!(skill(SkillType::AllStatsUp).is_green());
        assert!(!skill(SkillType::TargetSpeed).is_green());
        assert!(!skill(SkillType::MultiplyStartDelay).is_green());
    }

    #[test]
    fn test_is_gold() {
        let skill = |rarity| {
            let alternatives = vec![test_utils::alternative(
                "always==1",
                SkillType::TargetSpeed,
                0.15,
            )];
            Arc::new(Skill {
                rarity,
                ..synthetic_skill(alternatives)
            })
        };
        assert!(skill(SkillRarity::Gold).is_gold());
        assert!(skill(SkillRarity::Evolution).is_gold());
        assert!(!skill(SkillRarity::White).is_gold());
        assert!(!skill(SkillRarity::UniqueNatural).is_gold());
    }

    #[test]
    fn test_skill_ids_must_be_numeric() {
        let path = std::env::temp_dir().join(format!("skills-{}.json", std::process::id()));
        let load = |id: &str| {
            let json = format!(r#"{{"{id}": {{"rarity": 1, "alternatives": [], "witCheck": 0}}}}"#);
            std::fs::write(&path, json).unwrap();
            let result = Skill::load_skills_from_file(&path);
            std::fs::remove_file(&path).unwrap();
            result
        };
        assert_eq!(load("200012").unwrap()[&200012].id, 200012);
        assert!(load("speed").is_err());
    }

    #[test]
    fn test_skill_data_loads() {
        // Every condition in the skill data must parse.
        let skills = Skill::load_skills_from_file("data/skill_data.json").unwrap();
        assert!(!skills.is_empty());

        // Activated alternatives are tracked as bits of a mask (see
        // `Runner::activated_alternatives`).
        for (id, skill) in &skills {
            assert!(skill.alternatives.len() <= u8::BITS as usize, "{id}");
        }

        // Pre-race effects are applied before the race, where values aren't
        // scaled.
        let effects = skills
            .values()
            .flat_map(|skill| &skill.alternatives)
            .flat_map(|alternative| &alternative.effects);
        for effect in effects {
            if effect.skill_type.is_pre_race_effect() {
                assert_eq!(effect.value_scaling, ValueScaling::Direct, "{effect:?}");
            }
            // Additional effects stack their values while the skill's timed effects
            // are active, so only additive timed effects on oneself are supported.
            if effect.additional != AdditionalActivation::Regular {
                let skill_type = effect.skill_type;
                assert!(skill_type.is_timed_effect(), "{effect:?}");
                assert!(
                    !matches!(
                        skill_type,
                        SkillType::LaneChange | SkillType::ExtendEvolvedDuration
                    ),
                    "{effect:?}"
                );
                assert_eq!(effect.target, SkillTarget::Oneself, "{effect:?}");
            }
        }
    }

    #[test]
    fn test_pre_race_effects_activate_out_of_gate() {
        let skill = |skill_type| {
            let alternatives = vec![test_utils::alternative("running_style==1", skill_type, 0.0)];
            Arc::new(synthetic_skill(alternatives))
        };
        assert!(skill(SkillType::SpeedUp).should_activate_out_of_gate());
        assert!(skill(SkillType::MultiplyStartDelay).should_activate_out_of_gate());
        assert!(skill(SkillType::ChangeStyleRunaway).should_activate_out_of_gate());
        assert!(skill(SkillType::RushChanceUp).should_activate_out_of_gate());
        assert!(skill(SkillType::ActivateSympathy).should_activate_out_of_gate());
        assert!(skill(SkillType::IgnorePopularity).should_activate_out_of_gate());
        // The condition doesn't matter, only the effects.
        assert!(!skill(SkillType::TargetSpeed).should_activate_out_of_gate());
        assert!(!skill(SkillType::Acceleration).should_activate_out_of_gate());
    }
}
