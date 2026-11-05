//! Core data structures to represent an Umamusume (Uma).
//!
//! This module includes data structures for stats, aptitudes, running styles
//! and mood, as well as functions to convert raw stats into base stats.
//!
//! # Examples
//!
//! Build Umas in code rather than from JSON, e.g. to sweep a stat and race
//! each variant. Stats above 1200 only count half, so the base speed grows
//! half as fast past that point:
//!
//! ```
//! use uma_tools::{Aptitude, Mood, Style, Uma};
//!
//! let uma = Uma {
//!     speed: 1000,
//!     stamina: 800,
//!     power: 1000,
//!     guts: 600,
//!     wit: 1200,
//!     style: Style::Pace,
//!     distance_aptitude: Aptitude::A,
//!     track_aptitude: Aptitude::A,
//!     style_aptitude: Aptitude::A,
//!     fan_count: None,
//!     character_id: None,
//! };
//! let base_speeds: Vec<u16> = (1000..=1500)
//!     .step_by(100)
//!     .map(|speed| Uma { speed, ..uma }.to_base(Mood::Normal).speed)
//!     .collect();
//! assert_eq!(base_speeds, [1000, 1100, 1200, 1250, 1300, 1350]);
//! ```

use num_enum::TryFromPrimitive;
use serde::{Deserialize, Deserializer, de::Error};

use crate::configuration::Language;
use crate::course::ThresholdStat;
use crate::race::solver::Entrant;
use crate::skill::{DEFAULT_SKILL_LEVEL, Skill, SkillData, SkillDataError};
use std::fmt;
use std::path::Path;
use std::sync::Arc;

/// Running style or strategy of an Uma. The values represent in order, from
/// most to least aggressive:
///
/// - Runaway (大逃げ): A front runner that tries to outpace others from the start.
/// - Front runner (逃げ): Aims to lead the race.
/// - Pace chaser (先行): Stays near the front, behind the front runners (if any).
/// - Late surger (差し): Stays in the middle, behind front runners and pace
///   chasers, and makes a late surge.
/// - End closer (追込): Stays at the back and sprints towards the end.
///
/// ## Notes on Runaway vs Front
///
/// Runaway is a variant of Front running with more aggressive pacing. In many
/// contexts, they share the same behavior (e.g., positioning strategy, rush
/// calculations), but in specific situations (e.g., HP multipliers, phase
/// modifiers, spot struggle), they have distinct values.
///
/// When required to treat both Front and Runaway the same way,
/// [`Style::is_front_like`] can be used. Otherwise, explicit matching on
/// `Style` should be done to handle their distinct behaviors.
///
/// ## Deserialization
///
/// Styles are deserialized from their romanized Japanese names: `Oonige`,
/// `Nige`, `Senkou` (or `Senko`), `Sasi` (or `Sashi`) and `Oikomi`.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Deserialize, Hash, PartialOrd, Ord)]
#[serde(rename_all = "PascalCase")]
pub enum Style {
    /// 大逃げ, ōnige - A front runner that tries to outpace others from the start.
    /// In many contexts, it is treated similarly to [`Style::Front`].
    #[serde(rename = "Oonige")]
    Runaway = 0,
    /// 逃げ, nige - Aims to lead the race.
    #[serde(rename = "Nige")]
    Front = 1,
    /// 先行, senkō - Stays near the front, behind the front runners (if any).
    #[serde(rename = "Senkou", alias = "Senko")]
    Pace = 2,
    /// 差し, sashi - Stays in the middle, behind front runners and pace chasers, and
    /// makes a late surge.
    #[serde(rename = "Sasi", alias = "Sashi")]
    Late = 3,
    /// End closer (追込, oikomi) - Stays at the back and sprints towards the end.
    #[serde(rename = "Oikomi")]
    End = 4,
}

impl Style {
    /// Number of running styles. Styles can be used as array indices
    /// (`style as usize`).
    pub const COUNT: usize = 5;

    /// Returns `true` if `self` is a front-running style ([`Style::Front`] or
    /// [`Style::Runaway`]).
    ///
    /// Used when treating both Front and Runaway the same way is desired, (e.g., in
    /// positioning strategy or rush detection).
    ///
    /// ```rust
    /// use uma_tools::uma::Style;
    ///
    /// assert!(Style::Front.is_front_like());
    /// assert!(Style::Runaway.is_front_like());
    /// assert!(!Style::Pace.is_front_like());
    /// ```
    pub fn is_front_like(self) -> bool {
        matches!(self, Style::Front | Style::Runaway)
    }

    /// Returns the base style, treating [`Style::Runaway`] as a variant of
    /// [`Style::Front`] while leaving all other styles unchanged.
    ///
    /// Useful to group styles for condition evaluation or statistical purposes.
    ///
    /// ```rust
    /// use uma_tools::uma::Style;
    ///
    /// assert_eq!(Style::Runaway.base_style(), Style::Front);
    /// assert_eq!(Style::Front.base_style(), Style::Front);
    /// assert_eq!(Style::Pace.base_style(), Style::Pace);
    /// ```
    pub fn base_style(self) -> Style {
        match self {
            Style::Runaway => Style::Front,
            _ => self,
        }
    }
}

/// Aptitude levels for distance, track surface, and running style, with the
/// game's values.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Deserialize, TryFromPrimitive)]
#[repr(u8)]
pub enum Aptitude {
    S = 8,
    A = 7,
    B = 6,
    C = 5,
    D = 4,
    E = 3,
    F = 2,
    G = 1,
}

/// Mood or condition of the Uma, with the game's values from -2 (Awful) to 2
/// (Great). Affects base stat multipliers.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Deserialize, TryFromPrimitive)]
#[repr(i8)]
pub enum Mood {
    /// 絶不調, very bad.
    Awful = -2,
    /// 不調, bad.
    Bad,
    /// 普通, normal.
    Normal,
    /// 好調, favorable.
    Good,
    /// 絶好調, in top shape.
    Great,
}

/// Core stats for an Umamusume.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Uma {
    /// Stat that affects max speed.
    #[serde(deserialize_with = "deserialize_stat")]
    pub speed: u16,
    /// Stat that affects max HP.
    #[serde(deserialize_with = "deserialize_stat")]
    pub stamina: u16,
    /// Stat that affects acceleration and lateral movement.
    #[serde(deserialize_with = "deserialize_stat")]
    pub power: u16,
    /// Stat that affects performance during struggles and last stride.
    ///
    /// Representing "the will to fight and overcome", it affects speed and
    /// acceleration during duels and spot struggles. Moreover, it affects HP
    /// consumption late-race and last spurt performance; the higher
    /// the Guts, the better the performance on the final stretch.
    #[serde(deserialize_with = "deserialize_stat")]
    pub guts: u16,
    /// Wit, also known as wisdom or wiz. Stat that affects skill activation rate
    /// and decision-making.
    ///
    /// Specifically, it affects skill activation rate, chance of avoiding getting
    /// rushed, position keeping, last spurt planning, and more.
    #[serde(rename = "wisdom", deserialize_with = "deserialize_stat")]
    pub wit: u16,
    /// Running style or strategy. Affects mainly positioning strategy.
    #[serde(rename = "strategy")]
    pub style: Style,
    /// Aptitude for the current distance. Affects speed performance depending on
    /// the course distance.
    pub distance_aptitude: Aptitude,
    /// Aptitude for the current track surface. Affects acceleration and power
    /// performance depending on the track type.
    #[serde(rename = "surfaceAptitude")]
    pub track_aptitude: Aptitude,
    /// Aptitude for the current running style. Affects wit performance depending
    /// on the running style.
    #[serde(rename = "strategyAptitude")]
    pub style_aptitude: Aptitude,
    /// Number of fans. Used by some skill conditions and effects.
    ///
    /// If `None`, the configured default is used (see
    /// [`Configuration::default_fan_count`]).
    ///
    /// [`Configuration::default_fan_count`]:
    ///     crate::configuration::Configuration::default_fan_count
    pub fan_count: Option<u32>,
    /// Six-digits ID of the character card identifying the character the Uma
    /// represents and its outfit. The character ID is its first 4 digits.
    ///
    /// `None` if unknown.
    pub character_id: Option<u32>,
}

/// Deserialize a stat, rejecting zero.
///
/// Stats are at least 1 in the game. There's no upper bound, as the cap keeps
/// rising with game updates.
pub(crate) fn deserialize_stat<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<u16, D::Error> {
    let stat = u16::deserialize(deserializer)?;
    if stat >= 1 {
        Ok(stat)
    } else {
        Err(D::Error::custom("invalid stat 0: expected at least 1"))
    }
}

/// Helper struct for deserializing [`Uma`] with mood and skills from JSON.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UmaData {
    #[serde(flatten)]
    pub uma: Uma,
    /// Mood, motivation or condition of the Uma, if set. Affects base stat
    /// multipliers.
    #[serde(rename = "motivation", default)]
    pub mood: Option<Mood>,
    /// [`Skill`]s that the Uma has. In JSON, a list of skill IDs, all with
    /// [`DEFAULT_SKILL_LEVEL`].
    #[serde(deserialize_with = "deserialize_skill_ids")]
    pub skills: Vec<UmaSkill>,
}

/// A skill that an Uma has, by ID, with its level.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct UmaSkill {
    /// Skill ID.
    pub id: i32,
    /// Skill level (see [`Skill::level`]).
    pub level: u8,
}

/// Deserialize a list of skill IDs into [`UmaSkill`]s with
/// [`DEFAULT_SKILL_LEVEL`].
fn deserialize_skill_ids<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<Vec<UmaSkill>, D::Error> {
    let ids = Vec::<i32>::deserialize(deserializer)?;
    Ok(ids
        .into_iter()
        .map(|id| UmaSkill {
            id,
            level: DEFAULT_SKILL_LEVEL,
        })
        .collect())
}

impl UmaData {
    /// Parse `UmaData` from a JSON string.
    ///
    /// # Errors
    ///
    /// Returns an error if the JSON is invalid, or if a stat is zero.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use uma_tools::uma::UmaData;
    /// # fn main() -> Result<(), serde_json::Error> {
    /// let json = r#"{
    ///     "speed": 1500,
    ///     "stamina": 800,
    ///     "power": 1200,
    ///     "guts": 700,
    ///     "wisdom": 1200,
    ///     "strategy": "Oikomi",
    ///     "distanceAptitude": "S",
    ///     "surfaceAptitude": "A",
    ///     "strategyAptitude": "A",
    ///     "motivation": "Great",
    ///     "skills": [200642]
    /// }"#;
    /// let uma_data = UmaData::from_json(json)?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json)
    }

    /// Parse `UmaData` from a JSON file.
    ///
    /// # Errors
    ///
    /// Returns an error if the file can't be read, the JSON is invalid, or a
    /// stat is zero.
    ///
    /// See [`UmaData::from_json`] for an example.
    pub fn from_json_file(
        path: impl AsRef<Path>,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let content = std::fs::read_to_string(path)?;
        Ok(Self::from_json(&content)?)
    }

    /// Look up the skills of `self` in `all_skills` by ID, keeping their order.
    ///
    /// Skills are shared with `all_skills`, except those whose level differs,
    /// which are copied with their level set.
    ///
    /// # Errors
    ///
    /// Returns, for the first skill that can't be resolved:
    ///
    /// - [`ResolveSkillsError::UnknownSkill`] if it isn't in `all_skills`.
    /// - [`ResolveSkillsError::UnsupportedSkill`] if it is, but couldn't be
    ///   loaded (see [`SkillData::unsupported`]), so the runner can't race
    ///   with it.
    ///
    /// Then returns [`ResolveSkillsError::SameGroup`] with the first two
    /// skills of the same group (see [`Skill::group`]), as a runner can only
    /// have one of them.
    ///
    /// # Examples
    ///
    /// Report a skill that's missing from the skill data, e.g. one added by a
    /// game update that the data doesn't include yet:
    ///
    /// ```
    /// use uma_tools::{ResolveSkillsError, SkillData, UmaData};
    ///
    /// # fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    /// let uma = UmaData::from_json(
    ///     r#"{
    ///         "speed": 1500, "stamina": 800, "power": 1200, "guts": 700,
    ///         "wisdom": 1200, "strategy": "Oikomi", "distanceAptitude": "S",
    ///         "surfaceAptitude": "A", "strategyAptitude": "A",
    ///         "motivation": "Great", "skills": [200331, 999999]
    ///     }"#,
    /// )?;
    /// let all_skills = SkillData::from_json_file("data/skill_data_jp.json")?;
    /// let error = uma.resolve_skills(&all_skills).err();
    /// assert_eq!(error, Some(ResolveSkillsError::UnknownSkill(999999)));
    /// # Ok(())
    /// # }
    /// ```
    pub fn resolve_skills(
        &self,
        all_skills: &SkillData,
    ) -> Result<Vec<Arc<Skill>>, ResolveSkillsError> {
        let skills =
            self.skills
                .iter()
                .map(|&UmaSkill { id, level }| {
                    let skill = all_skills.get(id).ok_or_else(|| {
                        match all_skills.unsupported().get(&id) {
                            Some(reason) => {
                                ResolveSkillsError::UnsupportedSkill(id, reason.clone())
                            }
                            None => ResolveSkillsError::UnknownSkill(id),
                        }
                    })?;
                    Ok(if skill.level == level {
                        skill.clone()
                    } else {
                        Arc::new(Skill {
                            level,
                            ..Skill::clone(skill)
                        })
                    })
                })
                .collect::<Result<Vec<_>, _>>()?;
        for (i, skill) in skills.iter().enumerate() {
            if let Some(other) = skills[..i].iter().find(|other| other.group == skill.group) {
                return Err(ResolveSkillsError::SameGroup(other.id, skill.id));
            }
        }
        Ok(skills)
    }

    /// Convert `self` to an [`Entrant`], to add it to a race, with its mood and
    /// its skills resolved from `all_skills` (see [`Self::resolve_skills`]).
    ///
    /// # Errors
    ///
    /// Returns an error if the skills can't be resolved.
    ///
    /// # Examples
    ///
    /// ```
    /// use uma_tools::{Mood, SkillData, UmaData};
    ///
    /// # fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    /// let uma = UmaData::from_json_file("tests/data/uma_data.json")?;
    /// let all_skills = SkillData::from_json_file("data/skill_data_jp.json")?;
    /// let entrant = uma.to_entrant(&all_skills)?;
    /// assert_eq!(entrant.mood, Some(Mood::Great));
    /// assert_eq!(entrant.gate, None);
    /// # Ok(())
    /// # }
    /// ```
    pub fn to_entrant(&self, all_skills: &SkillData) -> Result<Entrant, ResolveSkillsError> {
        Ok(Entrant {
            mood: self.mood,
            ..Entrant::new(self.uma, self.resolve_skills(all_skills)?)
        })
    }
}

/// Error resolving the skills of an Uma.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ResolveSkillsError {
    /// A skill ID isn't in the skill data.
    UnknownSkill(i32),
    /// A skill (by ID) is in the skill data, but couldn't be loaded, with the
    /// reason (see [`SkillData::unsupported`]), also given as the error's
    /// [`source`](std::error::Error::source).
    UnsupportedSkill(i32, SkillDataError),
    /// Two skills (by ID) are versions of the same skill, e.g. a gold skill and
    /// its white version, or the same skill twice.
    SameGroup(i32, i32),
}

impl fmt::Display for ResolveSkillsError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            ResolveSkillsError::UnknownSkill(id) => write!(f, "unknown skill ID {id}"),
            ResolveSkillsError::UnsupportedSkill(id, _) => write!(f, "skill {id} isn't supported"),
            ResolveSkillsError::SameGroup(first, second) => {
                write!(f, "skills {first} and {second} are from the same group")
            }
        }
    }
}

impl std::error::Error for ResolveSkillsError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            ResolveSkillsError::UnsupportedSkill(_, reason) => Some(reason),
            _ => None,
        }
    }
}

/// Calculate base stat and apply mood multiplier.
///
/// Any stat above 1200 has its excess halved before applying multiplier.
pub(crate) fn calculate_base_stat(stat: u16, multiplier: f64) -> u16 {
    let mut computed = stat;
    if computed > 1200 {
        computed = 1200 + (computed - 1200) / 2;
    }
    ((computed).max(1) as f64 * multiplier) as u16
}

impl Mood {
    /// Convert a motivation, the game's mood value in careers and other tools,
    /// from 1 (Awful) to 5 (Great), or `None` if out of range.
    pub(crate) fn from_motivation(motivation: i8) -> Option<Self> {
        Self::try_from(motivation.checked_sub(3)?).ok()
    }

    /// Calculate base stats multiplier from `self`.
    ///
    /// Returns `mood_value × 0.02 + 1.0`, where mood values range from -2
    /// (awful) to +2 (great).
    pub(crate) fn multiplier(self) -> f64 {
        1.0 + (self as i8 as f64) * 0.02
    }
}

impl Uma {
    /// Get the name of the character card (outfit and Uma) in the specified
    /// language, or `None` if the card or its name in that language is unknown.
    pub fn name(&self, language: Language) -> Option<&'static str> {
        crate::names::character_name(self.character_id?, language)
    }

    /// Convert `self`, holding raw stats, to base stats using the mood
    /// multiplier.
    ///
    /// Base stats are the ones that race formulas and course set status checks
    /// use. Stats above 1200 only count half of their excess.
    ///
    /// # Examples
    ///
    /// See how much 400 speed above 1200 is worth in a race, and how mood
    /// changes it:
    ///
    /// ```
    /// use uma_tools::course::ThresholdStat;
    /// use uma_tools::{Aptitude, Mood, Style, Uma};
    ///
    /// let raw = Uma {
    ///     speed: 1600,
    ///     stamina: 800,
    ///     power: 1000,
    ///     guts: 600,
    ///     wit: 1200,
    ///     style: Style::Pace,
    ///     distance_aptitude: Aptitude::A,
    ///     track_aptitude: Aptitude::A,
    ///     style_aptitude: Aptitude::A,
    ///     fan_count: None,
    ///     character_id: None,
    /// };
    /// assert_eq!(raw.to_base(Mood::Normal).speed, 1400);
    /// assert_eq!(raw.to_base(Mood::Great).speed, 1456);
    /// assert_eq!(raw.to_base(Mood::Awful).stat(ThresholdStat::Speed), 1344);
    /// ```
    pub fn to_base(self, mood: Mood) -> Self {
        // N.B. In the Unity Cup (アオハル杯) scenario, a bonus is applied before mood
        // multiplier based on team rank. This is out of scope for this simulator.
        let mood_multiplier = mood.multiplier();
        Uma {
            speed: calculate_base_stat(self.speed, mood_multiplier),
            stamina: calculate_base_stat(self.stamina, mood_multiplier),
            power: calculate_base_stat(self.power, mood_multiplier),
            guts: calculate_base_stat(self.guts, mood_multiplier),
            wit: calculate_base_stat(self.wit, mood_multiplier),
            ..self
        }
    }

    /// Get the stat of `self` corresponding to a [`ThresholdStat`].
    pub fn stat(&self, stat: ThresholdStat) -> u16 {
        match stat {
            ThresholdStat::Speed => self.speed,
            ThresholdStat::Stamina => self.stamina,
            ThresholdStat::Power => self.power,
            ThresholdStat::Guts => self.guts,
            ThresholdStat::Wit => self.wit,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::{data_skills, synthetic_skill};

    #[test]
    fn test_mood_from_motivation() {
        let cases = [
            (0, None),
            (1, Some(Mood::Awful)),
            (5, Some(Mood::Great)),
            (6, None),
        ];
        for (motivation, expected) in cases {
            assert_eq!(Mood::from_motivation(motivation), expected, "{motivation}");
        }
    }

    #[test]
    fn test_calculate_base_stat() {
        // (raw stat, mood, base stat).
        let cases = [
            (1200, Mood::Normal, 1200),
            // Excess above 1200 is halved, rounding down.
            (1201, Mood::Normal, 1200),
            // Multiplier is applied after halving, truncating.
            (1500, Mood::Great, 1404),
            (1199, Mood::Good, 1222),
            (1199, Mood::Bad, 1175),
            // Stats are at least 1.
            (0, Mood::Normal, 1),
        ];
        for (stat, mood, expected) in cases {
            let actual = calculate_base_stat(stat, mood.multiplier());
            assert_eq!(actual, expected, "{stat} with {mood:?}");
        }
    }

    #[test]
    fn test_to_base() {
        let raw = Uma {
            speed: 1500,
            stamina: 800,
            power: 1200,
            guts: 700,
            wit: 1300,
            style: Style::End,
            distance_aptitude: Aptitude::S,
            track_aptitude: Aptitude::B,
            style_aptitude: Aptitude::C,
            fan_count: Some(120000),
            character_id: Some(101901),
        };

        // Style, aptitudes and fan count are kept.
        let expected = Uma {
            speed: 1404,
            stamina: 832,
            power: 1248,
            guts: 728,
            wit: 1300,
            ..raw
        };
        assert_eq!(raw.to_base(Mood::Great), expected);
    }

    /// JSON for [`UmaData`] with every field set, and the style as given.
    fn uma_json(strategy: &str) -> String {
        format!(
            r#"{{
                "speed": 1501,
                "stamina": 802,
                "power": 1203,
                "guts": 704,
                "wisdom": 1205,
                "strategy": "{strategy}",
                "distanceAptitude": "S",
                "surfaceAptitude": "B",
                "strategyAptitude": "C",
                "fanCount": 98765,
                "motivation": "Bad",
                "skills": [200642, 201161]
            }}"#
        )
    }

    #[test]
    fn test_uma_data_from_json() {
        let expected = UmaData {
            uma: Uma {
                speed: 1501,
                stamina: 802,
                power: 1203,
                guts: 704,
                wit: 1205,
                style: Style::Pace,
                distance_aptitude: Aptitude::S,
                track_aptitude: Aptitude::B,
                style_aptitude: Aptitude::C,
                fan_count: Some(98765),
                character_id: None,
            },
            mood: Some(Mood::Bad),
            skills: [200642, 201161].map(skill_level_1).to_vec(),
        };
        assert_eq!(UmaData::from_json(&uma_json("Senkou")).unwrap(), expected);
    }

    #[test]
    fn test_uma_data_without_mood() {
        let json = uma_json("Nige").replace(r#""motivation": "Bad","#, "");
        assert_eq!(UmaData::from_json(&json).unwrap().mood, None);
    }

    #[test]
    fn test_uma_name() {
        use crate::test_utils::uma;
        // Also the ID of a skill, with another name.
        let card = Uma {
            character_id: Some(100101),
            ..uma(Style::Late)
        };
        let name = card.name(Language::English);
        assert_eq!(name, Some("[Special Dreamer] Special Week"));
        assert_eq!(uma(Style::Late).name(Language::English), None);
    }

    #[test]
    fn test_uma_data_from_json_file() {
        let data = UmaData::from_json_file("tests/data/uma_data.json").unwrap();
        let uma = data.uma;
        assert_eq!(
            (uma.speed, uma.stamina, uma.power, uma.guts, uma.wit),
            (1500, 800, 1200, 700, 1200)
        );
        assert_eq!(uma.style, Style::End);
        assert_eq!(data.mood, Some(Mood::Great));
        assert_eq!(data.skills, [skill_level_1(200642)]);
    }

    #[test]
    fn test_uma_data_style_names() {
        let cases = [
            ("Oonige", Style::Runaway),
            ("Nige", Style::Front),
            ("Senkou", Style::Pace),
            ("Senko", Style::Pace),
            ("Sasi", Style::Late),
            ("Sashi", Style::Late),
            ("Oikomi", Style::End),
        ];
        for (name, expected) in cases {
            let uma_data = UmaData::from_json(&uma_json(name)).unwrap();
            assert_eq!(uma_data.uma.style, expected, "{name}");
        }
    }

    #[test]
    fn test_zero_stat_is_rejected() {
        let json = uma_json("Nige").replace(r#""guts": 704"#, r#""guts": 0"#);
        let error = UmaData::from_json(&json).unwrap_err().to_string();
        assert!(
            error.contains("invalid stat 0: expected at least 1"),
            "{error}"
        );
    }

    /// [`UmaSkill`] with the given ID and level 1.
    fn skill_level_1(id: i32) -> UmaSkill {
        UmaSkill { id, level: 1 }
    }

    /// Skill data with a single synthetic skill with ID 1.
    fn skill_1() -> SkillData {
        SkillData::from_iter([Arc::new(Skill {
            id: 1,
            ..synthetic_skill(vec![])
        })])
    }

    /// Uma data with the given skill IDs, all with level 1.
    fn uma_with_skills(skills: Vec<i32>) -> UmaData {
        uma_with_skill_levels(skills.into_iter().map(skill_level_1).collect())
    }

    /// Uma data with the given skills.
    fn uma_with_skill_levels(skills: Vec<UmaSkill>) -> UmaData {
        UmaData {
            uma: crate::test_utils::uma(Style::Pace),
            mood: Some(Mood::Normal),
            skills,
        }
    }

    #[test]
    fn test_resolve_skills_keeps_order_and_shares_skills() {
        let all_skills = SkillData::from_iter([1, 2, 3].map(|id| {
            Arc::new(Skill {
                id,
                group: id,
                ..synthetic_skill(vec![])
            })
        }));
        let skills = uma_with_skills(vec![3, 1])
            .resolve_skills(&all_skills)
            .unwrap();
        assert_eq!(skills.len(), 2);
        assert!(Arc::ptr_eq(&skills[0], all_skills.get(3).unwrap()));
        assert!(Arc::ptr_eq(&skills[1], all_skills.get(1).unwrap()));
    }

    #[test]
    fn test_to_entrant() {
        let all_skills = skill_1();
        let uma_data = UmaData {
            mood: Some(Mood::Bad),
            ..uma_with_skills(vec![1])
        };
        let entrant = uma_data.to_entrant(&all_skills).unwrap();
        assert_eq!(entrant.uma, uma_data.uma);
        assert_eq!((entrant.mood, entrant.gate), (Some(Mood::Bad), None));
    }

    #[test]
    fn test_resolve_skills_sets_levels() {
        let all_skills = skill_1();
        let skills = uma_with_skill_levels(vec![UmaSkill { id: 1, level: 5 }])
            .resolve_skills(&all_skills)
            .unwrap();
        assert_eq!(skills[0].level, 5);
        // The loaded skill is left untouched.
        assert_eq!(all_skills.get(1).unwrap().level, 1);
    }

    #[test]
    fn test_resolve_skills_fails_on_missing_skills() {
        // Skill 2 is in the data, but with an unknown rarity.
        let all_skills = SkillData::from_json(
            r#"{
                "1": {"rarity": 1, "alternatives": [], "witCheck": 0, "group": 1, "groupRate": 1},
                "2": {"rarity": 99, "alternatives": [], "witCheck": 0, "group": 2, "groupRate": 1}
            }"#,
        )
        .unwrap();
        let resolve = |skills| uma_with_skills(skills).resolve_skills(&all_skills);

        let error = resolve(vec![1, 42, 2]).unwrap_err();
        assert_eq!(error, ResolveSkillsError::UnknownSkill(42));
        assert_eq!(error.to_string(), "unknown skill ID 42");

        // The reason is the source, not part of the message.
        let error = resolve(vec![1, 2, 42]).unwrap_err();
        let reason = &all_skills.unsupported()[&2];
        assert_eq!(
            error,
            ResolveSkillsError::UnsupportedSkill(2, reason.clone())
        );
        assert_eq!(error.to_string(), "skill 2 isn't supported");
        let source = std::error::Error::source(&error).unwrap();
        assert_eq!(source.downcast_ref::<SkillDataError>(), Some(reason));
        assert!(std::error::Error::source(&ResolveSkillsError::UnknownSkill(42)).is_none());
    }

    #[test]
    fn test_resolve_skills_fails_on_same_group() {
        // Right-Handed ○ and its gold version, with Corner Adept ○.
        let (white, gold, other) = (200012, 200014, 200332);
        let resolve = |skills| uma_with_skills(skills).resolve_skills(data_skills());
        assert!(resolve(vec![white, other]).is_ok());
        let error = ResolveSkillsError::SameGroup(white, gold);
        assert_eq!(resolve(vec![white, other, gold]), Err(error.clone()));
        assert_eq!(
            error.to_string(),
            "skills 200012 and 200014 are from the same group"
        );
        // The same skill twice.
        assert_eq!(
            resolve(vec![other, other]),
            Err(ResolveSkillsError::SameGroup(other, other))
        );
    }
}
