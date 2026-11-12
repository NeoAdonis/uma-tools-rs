//! Skill-related data structures and logic for race simulation.
//!
//! # Examples
//!
//! Compare a gold skill with its white version, e.g. to judge whether the
//! upgrade is worth its cost:
//!
//! ```
//! use uma_tools::SkillData;
//! use uma_tools::skill::SkillType;
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
//! let all_skills = SkillData::from_json_file("data/skill_data_jp.json")?;
//! let target_speed = |id| {
//!     let skill = all_skills.get(id)?;
//!     let effect = skill.alternatives.first()?.effects.first()?;
//!     (effect.skill_type == SkillType::TargetSpeed).then_some(effect.modifier)
//! };
//! // Professor of Curvature and Corner Adept ○, in m/s.
//! assert_eq!(target_speed(200331), Some(0.35));
//! assert_eq!(target_speed(200332), Some(0.15));
//! # Ok(())
//! # }
//! ```

use num_enum::TryFromPrimitive;
use serde::{Deserialize, Deserializer, de::Error};
use serde_repr::Deserialize_repr;
use serde_with::{BoolFromInt, serde_as};
use std::collections::{BTreeMap, HashMap};
use std::fmt;
use std::path::Path;
use std::sync::Arc;

use crate::condition::parser::{Comparison, ComparisonList, Condition};
use crate::condition::variables::{ActivationVar, VarType};
use crate::configuration::Language;
use crate::uma::{BaseStyle, EffectiveStyle};
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
    /// Change running style to Runaway (see [`EffectiveStyle::Runaway`]).
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

/// Set of skill types, as a bitmask of their IDs.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SkillTypeSet(u64);

impl SkillTypeSet {
    /// Add `skill_type` to the set.
    pub(crate) fn insert(&mut self, skill_type: SkillType) {
        self.0 |= Self::bit(skill_type);
    }

    /// Check if `skill_type` is in the set.
    pub fn contains(self, skill_type: SkillType) -> bool {
        self.0 & Self::bit(skill_type) != 0
    }

    /// Iterate over the skill types in the set, in ID order.
    pub fn iter(self) -> impl Iterator<Item = SkillType> {
        let mut bits = self.0;
        std::iter::from_fn(move || {
            if bits == 0 {
                return None;
            }
            let id = bits.trailing_zeros() as u8;
            bits &= bits - 1;
            Some(SkillType::try_from(id).expect("only skill types are inserted"))
        })
    }

    fn bit(skill_type: SkillType) -> u64 {
        1 << skill_type as u8
    }
}

impl SkillType {
    /// Get the skill type with the given ID, if it's implemented.
    pub fn from_id(id: i32) -> Option<Self> {
        u8::try_from(id).ok()?.try_into().ok()
    }

    /// Check if this effect type is a stat boost (applied permanently).
    pub(crate) fn is_stat_boost(self) -> bool {
        use SkillType::*;
        matches!(
            self,
            SpeedUp | StaminaUp | PowerUp | GutsUp | WitUp | AllStatsUp
        )
    }

    /// Check if this effect type is a start delay modification.
    pub(crate) fn is_start_delay_modification(self) -> bool {
        matches!(
            self,
            SkillType::MultiplyStartDelay | SkillType::AddStartDelay
        )
    }

    /// Returns the running style this effect type changes to, or `None` if it isn't
    /// a style change.
    pub(crate) fn style_change(self) -> Option<EffectiveStyle> {
        match self {
            SkillType::ChangeStyleRunaway => Some(EffectiveStyle::Runaway),
            _ => None,
        }
    }

    /// Check if this effect type is a rush chance effect.
    pub(crate) fn is_rush_chance_effect(self) -> bool {
        matches!(self, SkillType::RushChanceUp)
    }

    /// Check if this effect type modifies a parameter that is needed before the
    /// race starts (stats, start delay, running style, rush chance, etc.).
    pub(crate) fn is_pre_race_effect(self) -> bool {
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
    pub(crate) fn is_timed_effect(self) -> bool {
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
    EnemyStrategy { style: BaseStyle },
    /// Skill affects the `max` nearest rushed runners ahead.
    RushedAhead { max: u8 },
    /// Skill affects the `max` nearest rushed runners behind.
    RushedBehind { max: u8 },
    /// Skill affects rushed runners with a specific running style.
    RushedStrategy { style: BaseStyle },
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
            1 => Ok(BaseStyle::Front),
            2 => Ok(BaseStyle::Pace),
            3 => Ok(BaseStyle::Late),
            4 => Ok(BaseStyle::End),
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
/// situation when the skill activates (see `race::skill_scaling`).
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
    /// Scales with the total performance level earned in the Shuntaisai scenario.
    PerformanceLevel = 41,
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
    pub(crate) fn is_positive(&self) -> bool {
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

/// Level of a skill when not given, e.g. for skills loaded from the skill data.
pub const DEFAULT_SKILL_LEVEL: u8 = 1;

/// [`DEFAULT_SKILL_LEVEL`], for serde defaults.
fn default_skill_level() -> u8 {
    DEFAULT_SKILL_LEVEL
}

/// ID of the "Sympathy" skill, which skills with an
/// [`SkillType::ActivateSympathy`] effect activate for every runner.
pub(crate) const SYMPATHY_ID: i32 = 201631;

fn default_cooldown() -> f64 {
    DEFAULT_COOLDOWN
}

/// Represents how the duration of a skill scales with the runner's situation
/// when the skill activates (see `race::skill_scaling`).
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
    pub(crate) fn is_continuation(&self) -> bool {
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
    pub(crate) fn is_out_of_gate(&self) -> bool {
        self.effects
            .iter()
            .any(|effect| effect.skill_type.is_pre_race_effect())
    }

    /// Check if any effect of this alternative targets the runner herself.
    pub(crate) fn affects_oneself(&self) -> bool {
        self.effects
            .iter()
            .any(|effect| effect.target == SkillTarget::Oneself)
    }
}

/// Parse a condition string while deserializing.
fn deserialize_condition<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<Condition, D::Error> {
    let condition = String::deserialize(deserializer)?;
    let invalid =
        |e: &dyn fmt::Display| D::Error::custom(format!("invalid condition '{condition}': {e}"));
    let parsed = condition.parse().map_err(|e| invalid(&e))?;
    check_events(&parsed).map_err(|e| invalid(&e))?;
    Ok(parsed)
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
    let invalid = |e: &dyn fmt::Display| {
        D::Error::custom(format!("invalid precondition '{precondition}': {e}"))
    };
    let parsed: Condition = precondition.parse().map_err(|e| invalid(&e))?;
    check_events(&parsed).map_err(|e| invalid(&e))?;
    // Preconditions aren't rolled (see `crate::trigger::skill_activation`).
    if parsed.branches.iter().flatten().any(Comparison::is_random) {
        return Err(invalid(
            &"random variables aren't supported in preconditions",
        ));
    }
    Ok(Some(parsed))
}

/// Check that no branch of a condition has more than one event (see
/// [`Comparison::is_event`]): the trigger system checks a branch at the points
/// of its event, so it can't tell which event they come from.
fn check_events(condition: &Condition) -> Result<(), &'static str> {
    let has_many_events =
        |branch: &ComparisonList| branch.iter().filter(|comp| comp.is_event()).count() > 1;
    if condition.branches.iter().any(has_many_events) {
        return Err("more than one event in a branch");
    }
    Ok(())
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
    /// the skill adds no points.
    #[serde(default)]
    pub popularity: Option<Vec<SkillPopularity>>,
    /// Whether this skill requires a wit check to activate.
    #[serde_as(as = "BoolFromInt")]
    pub wit_check: bool,
    /// Group of skills that are versions of the same skill (e.g. Right-Handed
    /// ×, ○, ◎ and its gold version). A runner can only have one skill of each
    /// group.
    pub group: i32,
    /// Tier of this skill within its group: higher tiers are upgrades of lower
    /// ones (e.g. ○ is 1, ◎ is 2 and the gold version is 3). Negative for
    /// negative skills (×).
    pub group_rate: i8,
    /// Skill level, e.g. a unique skill's level from the Uma's talent level.
    ///
    /// Not part of the skill data, but of the Uma that has the skill; see
    /// [`UmaData::resolve_skills`]. [`DEFAULT_SKILL_LEVEL`] when loading the
    /// skill. Higher levels multiply the modifiers of the skill's effects,
    /// depending on their type.
    ///
    /// [`UmaData::resolve_skills`]: crate::uma::UmaData::resolve_skills
    #[serde(skip, default = "default_skill_level")]
    pub level: u8,
}

impl Skill {
    /// Get the skill name in the specified language, or `None` if it isn't
    /// available in that language.
    pub fn name(&self, language: Language) -> Option<&'static str> {
        crate::names::skill_name(self.id, language)
    }

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
    pub(crate) fn is_wit_limit_break_eligible(&self) -> bool {
        !matches!(self.rarity, SkillRarity::White)
    }

    /// Chance that this skill passes its pre-race activation check.
    ///
    /// Skills that don't require a wit check always pass; otherwise the chance
    /// depends on the runner's base wit (minimum 20%).
    pub(crate) fn activation_chance(&self, base_wit: f64) -> f64 {
        if !self.wit_check {
            return 1.0;
        }
        (1.0 - 90.0 / base_wit).max(0.2)
    }

    /// Check if this skill should activate out of gate (before race begins).
    ///
    /// That is, if any of its alternatives does (see
    /// [`SkillAlternative::is_out_of_gate`]).
    pub(crate) fn should_activate_out_of_gate(&self) -> bool {
        self.alternatives
            .iter()
            .any(SkillAlternative::is_out_of_gate)
    }

    /// Check if any alternative of this skill satisfies every popularity
    /// requirement (see [`SkillType::IgnorePopularity`]).
    pub(crate) fn ignores_popularity(&self) -> bool {
        self.alternatives
            .iter()
            .flat_map(|alternative| &alternative.effects)
            .any(|effect| matches!(effect.skill_type, SkillType::IgnorePopularity))
    }
}

/// Skills by ID, as loaded from skill data, with the skills that couldn't be
/// loaded.
///
/// Skills are loaded one by one, so that a skill the simulator doesn't support
/// yet (e.g. one using a new condition variable) doesn't prevent using the
/// others. Resolving an unsupported skill for a runner fails instead (see
/// [`UmaData::resolve_skills`](crate::uma::UmaData::resolve_skills)).
///
/// Skills are shared through [`Arc`], so that every runner with the same skill
/// can hold it without copying its data.
#[derive(Clone, Debug, Default)]
pub struct SkillData {
    skills: HashMap<i32, Arc<Skill>>,
    unsupported: BTreeMap<i32, SkillDataError>,
}

impl SkillData {
    /// Load the skills in a JSON file.
    ///
    /// # Errors
    ///
    /// Returns an error if the file can't be read, or for the same reasons as
    /// [`SkillData::from_json`].
    ///
    /// See [`UmaData::resolve_skills`](crate::uma::UmaData::resolve_skills) for
    /// an example.
    pub fn from_json_file(
        json_path: impl AsRef<Path>,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let json = std::fs::read_to_string(json_path)?;
        Ok(Self::from_json(&json)?)
    }

    /// Load the skills in a JSON string, e.g. with skill data fetched by a web
    /// page.
    ///
    /// # Errors
    ///
    /// Returns an error if the JSON is invalid or a skill ID isn't a number.
    /// Skills that can't be loaded are reported by [`Self::unsupported`]
    /// instead.
    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        // JSON keys are strings, but serde parses them as integers.
        let raw: HashMap<i32, serde_json::Value> = serde_json::from_str(json)?;

        let mut data = Self::default();
        for (id, skill) in raw {
            match Skill::deserialize(skill) {
                Ok(skill) => drop(data.skills.insert(id, Arc::new(Skill { id, ..skill }))),
                Err(error) => drop(
                    data.unsupported
                        .insert(id, SkillDataError(error.to_string())),
                ),
            }
        }
        Ok(data)
    }

    /// Skill with the given ID, if it was loaded.
    pub fn get(&self, id: i32) -> Option<&Arc<Skill>> {
        self.skills.get(&id)
    }

    /// Every loaded skill.
    pub fn skills(&self) -> impl Iterator<Item = &Arc<Skill>> {
        self.skills.values()
    }

    /// Why each skill that couldn't be loaded was rejected, by ID.
    ///
    /// # Examples
    ///
    /// Report the skills that won't be simulated, e.g. skills from a newer
    /// game version whose conditions use variables that aren't supported yet:
    ///
    /// ```
    /// use uma_tools::SkillData;
    ///
    /// # fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    /// // Like Professor of Curvature, but with a new variable.
    /// let json = r#"{"900001": {
    ///     "alternatives": [{
    ///         "baseCooldown": 300000, "baseDuration": 24000,
    ///         "condition": "new_variable==1", "durationScaling": 1,
    ///         "effects": [{
    ///             "modifier": 3500, "target": {"type": 1, "value": 0},
    ///             "type": 27, "valueScaling": 1
    ///         }],
    ///         "precondition": ""
    ///     }],
    ///     "group": 90000, "groupRate": 1, "popularity": [],
    ///     "rarity": 2, "witCheck": 1
    /// }}"#;
    /// let all_skills = SkillData::from_json(json)?;
    /// for (id, reason) in all_skills.unsupported() {
    ///     println!("Skill {id} won't be simulated: {reason}");
    /// }
    /// assert!(all_skills.get(900001).is_none());
    /// let reason = all_skills.unsupported()[&900001].to_string();
    /// assert!(reason.contains("new_variable"));
    /// # Ok(())
    /// # }
    /// ```
    pub fn unsupported(&self) -> &BTreeMap<i32, SkillDataError> {
        &self.unsupported
    }
}

/// Why a skill in the skill data couldn't be loaded, e.g. a condition with an
/// unknown variable or more than one event in a branch, or an unknown effect
/// type (see [`SkillData::unsupported`]).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct SkillDataError(String);

impl fmt::Display for SkillDataError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for SkillDataError {}

/// Collect skills by their [`Skill::id`].
impl FromIterator<Arc<Skill>> for SkillData {
    fn from_iter<I: IntoIterator<Item = Arc<Skill>>>(skills: I) -> Self {
        let skills = skills.into_iter().map(|skill| (skill.id, skill)).collect();
        Self {
            skills,
            ..Self::default()
        }
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
    fn test_skill_name() {
        // Also the ID of a character card, with another name.
        let skill = Skill {
            id: 100101,
            ..synthetic_skill(vec![])
        };
        let name = skill.name(Language::English);
        assert_eq!(name, Some("Shooting for Victory!"));
    }

    #[test]
    fn test_skill_type_ids_fit_in_set() {
        for id in 0..=u8::MAX {
            if SkillType::try_from(id).is_ok() {
                assert!(id < u64::BITS as u8, "skill type {id}");
            }
        }
    }

    #[test]
    fn test_skill_type_set() {
        let mut set = SkillTypeSet::default();
        assert_eq!(set.iter().next(), None);
        set.insert(SkillType::IgnorePopularity);
        set.insert(SkillType::SpeedUp);
        set.insert(SkillType::IgnorePopularity);
        assert!(set.contains(SkillType::SpeedUp));
        assert!(!set.contains(SkillType::Recovery));
        let types: Vec<_> = set.iter().collect();
        assert_eq!(types, [SkillType::SpeedUp, SkillType::IgnorePopularity]);
    }

    #[test]
    fn test_activation_chance_without_wit_check() {
        let skill = skill_with_wit_check(false);
        assert_eq!(skill.activation_chance(1.0), 1.0);
    }

    #[test]
    fn test_activation_chance_with_wit_check() {
        let skill = skill_with_wit_check(true);
        assert_close!(skill.activation_chance(900.0), 0.9);
        // Low wit is clamped to the minimum chance.
        assert_eq!(skill.activation_chance(100.0), 0.2);
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
        let error = |condition, precondition| {
            alternative(condition, precondition)
                .unwrap_err()
                .to_string()
        };
        // Empty conditions are not allowed, unlike empty preconditions.
        assert!(error("", "").contains("invalid condition '': "));
        assert!(error("phase>=2", "order<=").contains("invalid precondition 'order<=': "));
    }

    #[test]
    fn test_alternatives_with_unsupported_events_are_errors() {
        const MANY_EVENTS: &str = "more than one event in a branch";
        // (condition, precondition, reason it's rejected, if it is).
        let cases = [
            // Events in separate branches.
            ("phase_random==1@is_last_straight_onetime==1", "", None),
            // Activation comparisons that aren't events.
            ("is_activate_any_skill==0&phase_random==1", "", None),
            (
                "is_activate_any_skill==1&phase_random==1",
                "",
                Some(MANY_EVENTS),
            ),
            // Also in the precondition.
            ("phase>=2", "phase>=2&is_last_straight_onetime==1", None),
            (
                "phase>=2",
                "is_activate_heal_skill==1&is_last_straight_onetime==1",
                Some(MANY_EVENTS),
            ),
            // Preconditions aren't rolled.
            (
                "phase>=2",
                "random_lot==50",
                Some("random variables aren't supported in preconditions"),
            ),
        ];
        for (condition, precondition, reason) in cases {
            let error = alternative(condition, precondition)
                .err()
                .map(|e| e.to_string());
            match (reason, error) {
                (None, None) => {}
                (Some(reason), Some(error)) => {
                    assert!(
                        error.contains(reason),
                        "{condition} / {precondition}: {error}"
                    );
                }
                (_, error) => panic!("{condition} / {precondition}: {error:?}"),
            }
        }
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
        // One target per kind of value: none, a count, a style, the raw value.
        assert_eq!(target(1, 0).unwrap(), SkillTarget::Oneself);
        assert_eq!(target(4, 18).unwrap(), SkillTarget::InFov { max: 18 });
        assert_eq!(
            target(18, 1).unwrap(),
            SkillTarget::EnemyStrategy {
                style: BaseStyle::Front
            }
        );
        assert_eq!(
            target(22, 1001).unwrap(),
            SkillTarget::UmaId { value: 1001 }
        );
    }

    #[test]
    fn test_invalid_targets_are_errors() {
        // (type, value, accepted).
        let cases = [
            // Unknown target type.
            (3, 0, false),
            // Counts fit in a byte.
            (4, -1, false),
            (4, 0, true),
            (4, 255, true),
            (4, 256, false),
            // Styles, except Runaway (0).
            (18, 0, false),
            (18, 1, true),
            (18, 4, true),
            (18, 5, false),
        ];
        for (target_type, value, accepted) in cases {
            let result = target(target_type, value);
            assert_eq!(
                result.is_ok(),
                accepted,
                "type {target_type}, value {value}"
            );
        }
    }

    #[test]
    fn test_effect_is_positive() {
        let cases = [
            // Purely positive.
            (SkillType::ActivateRandomGold, 0.0, true),
            // Less start delay is good.
            (SkillType::AddStartDelay, -0.1, true),
            (SkillType::AddStartDelay, 0.1, false),
            (SkillType::MultiplyStartDelay, 0.5, true),
            (SkillType::MultiplyStartDelay, 1.5, false),
            // Less rush chance or time is good.
            (SkillType::RushChanceUp, -0.1, true),
            (SkillType::RushChanceUp, 0.1, false),
            // Anything else is good when positive.
            (SkillType::TargetSpeed, 0.1, true),
            (SkillType::TargetSpeed, -0.1, false),
        ];
        for (skill_type, modifier, expected) in cases {
            let actual = test_utils::effect(skill_type, modifier).is_positive();
            assert_eq!(actual, expected, "{skill_type:?} with {modifier}");
        }
    }

    #[test]
    fn test_additional_activation_defaults_to_regular() {
        let json = r#"{"type": 27, "target": {"type": 1, "value": 0}, "modifier": 500,
            "valueScaling": 1}"#;
        let effect: SkillEffect = serde_json::from_str(json).unwrap();
        assert_eq!(effect.additional, AdditionalActivation::Regular);
    }

    #[test]
    fn test_is_green() {
        let skill = |skill_type| {
            let alternatives = vec![test_utils::alternative("always==1", skill_type, 40.0)];
            Arc::new(synthetic_skill(alternatives))
        };
        assert!(skill(SkillType::SpeedUp).is_green());
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
    fn test_skill_data_from_json() {
        let skill = |id: &str, rarity: i32| {
            format!(
                r#""{id}": {{"rarity": {rarity}, "alternatives": [],
                    "witCheck": 0, "group": 1, "groupRate": 1}}"#
            )
        };
        let load = |skills: &[String]| SkillData::from_json(&format!("{{{}}}", skills.join(",")));

        // Skills that can't be loaded are reported, and the others kept.
        let data = load(&[skill("200012", 1), skill("200013", 99)]).unwrap();
        assert_eq!(data.get(200012).unwrap().id, 200012);
        assert!(data.get(200013).is_none());
        let unsupported: Vec<_> = data.unsupported().keys().collect();
        assert_eq!(unsupported, [&200013]);
    }

    #[test]
    fn test_skill_data_is_supported() {
        // Every skill in the data of every version loads (e.g. its conditions
        // only use known variables). This is the test to check after a data
        // update: other tests only use the skills that load.
        for path in test_utils::SKILL_DATA_FILES {
            check_skill_data(&SkillData::from_json_file(path).unwrap(), path);
        }
    }

    fn check_skill_data(skills: &SkillData, path: &str) {
        let unsupported = skills.unsupported();
        let report: Vec<_> = unsupported
            .iter()
            .map(|(id, e)| format!("{id}: {e}"))
            .collect();
        assert!(unsupported.is_empty(), "{path}:\n{}", report.join("\n"));
        assert!(skills.skills().next().is_some(), "{path}");

        // Activated alternatives are tracked as bits of a mask (see
        // `Runner::activated_alternatives`).
        for skill in skills.skills() {
            assert!(
                skill.alternatives.len() <= u8::BITS as usize,
                "{path}: {}",
                skill.id
            );
        }

        // Pre-race effects are applied before the race, where values aren't
        // scaled.
        let effects = skills
            .skills()
            .flat_map(|skill| &skill.alternatives)
            .flat_map(|alternative| &alternative.effects);
        for effect in effects {
            if effect.skill_type.is_pre_race_effect() {
                assert_eq!(
                    effect.value_scaling,
                    ValueScaling::Direct,
                    "{path}: {effect:?}"
                );
            }
            // Additional effects stack their values while the skill's timed effects
            // are active, so only additive timed effects on oneself are supported.
            if effect.additional != AdditionalActivation::Regular {
                let skill_type = effect.skill_type;
                assert!(skill_type.is_timed_effect(), "{path}: {effect:?}");
                assert!(
                    !matches!(
                        skill_type,
                        SkillType::LaneChange | SkillType::ExtendEvolvedDuration
                    ),
                    "{path}: {effect:?}"
                );
                assert_eq!(effect.target, SkillTarget::Oneself, "{path}: {effect:?}");
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
    }
}
