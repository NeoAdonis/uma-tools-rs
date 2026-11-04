//! Import of Umas exported by umadump.
//!
//! umadump exports trained Umas as JSON objects using the game's field
//! names (`card_id`, `wiz`, `proper_distance_mile`, etc.), with aptitudes from
//! 1 (G) to 8 (S). [`UmaDumpData`] reads one of them, ignoring the fields
//! that the simulation doesn't use.
//!
//! umadump also exports the state of a career on each turn, with the Uma in
//! training under `single_mode_load_common.chara_info`, in the same format
//! except for its running style field (`race_running_style`) and an extra
//! mood (`motivation`). [`UmaDumpData::from_career_json`] reads it, so the
//! last turn of a career can race as if trained.
//!
//! An Uma has aptitudes for every distance and track surface, so importing it
//! takes the ones to use. Trained Umas don't include the mood, so it's left
//! unset for them.
//!
//! # Examples
//!
//! Import an Uma for a turf mile race:
//!
//! ```
//! use uma_tools::course::{DistanceType, Track};
//! use uma_tools::import::umadump::UmaDumpData;
//! use uma_tools::{Aptitude, Style};
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
//! let path = "tests/data/umadump_uma.json";
//! let data = UmaDumpData::from_json_file(path)?;
//! let uma_data = data.to_uma_data(DistanceType::Mile, Track::Turf);
//! assert_eq!(uma_data.uma.style, Style::Late);
//! assert_eq!(uma_data.uma.distance_aptitude, Aptitude::A);
//! assert_eq!(uma_data.uma.character_id, Some(101901));
//! # Ok(())
//! # }
//! ```

use serde::{Deserialize, Deserializer, de::Error};
use std::path::Path;

use crate::course::{DistanceType, Track};
use crate::uma::{Aptitude, Mood, Style, Uma, UmaData, UmaSkill, deserialize_stat};

/// A trained Uma, or one in training, as exported by umadump.
///
/// Convert it with [`UmaDumpData::to_uma_data`] to race with it.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Deserialize)]
pub struct UmaDumpData {
    /// See [`Uma::character_id`].
    card_id: u32,
    #[serde(deserialize_with = "deserialize_stat")]
    speed: u16,
    #[serde(deserialize_with = "deserialize_stat")]
    stamina: u16,
    #[serde(deserialize_with = "deserialize_stat")]
    power: u16,
    #[serde(deserialize_with = "deserialize_stat")]
    guts: u16,
    #[serde(rename = "wiz", deserialize_with = "deserialize_stat")]
    wit: u16,
    fans: u32,
    /// `race_running_style` in careers.
    #[serde(alias = "race_running_style", deserialize_with = "deserialize_style")]
    running_style: Style,
    /// Only in careers.
    #[serde(
        rename = "motivation",
        default,
        deserialize_with = "deserialize_motivation"
    )]
    mood: Option<Mood>,
    #[serde(
        rename = "proper_running_style_nige",
        deserialize_with = "deserialize_aptitude"
    )]
    front_aptitude: Aptitude,
    #[serde(
        rename = "proper_running_style_senko",
        deserialize_with = "deserialize_aptitude"
    )]
    pace_aptitude: Aptitude,
    #[serde(
        rename = "proper_running_style_sashi",
        deserialize_with = "deserialize_aptitude"
    )]
    late_aptitude: Aptitude,
    #[serde(
        rename = "proper_running_style_oikomi",
        deserialize_with = "deserialize_aptitude"
    )]
    end_aptitude: Aptitude,
    #[serde(
        rename = "proper_distance_short",
        deserialize_with = "deserialize_aptitude"
    )]
    short_aptitude: Aptitude,
    #[serde(
        rename = "proper_distance_mile",
        deserialize_with = "deserialize_aptitude"
    )]
    mile_aptitude: Aptitude,
    #[serde(
        rename = "proper_distance_middle",
        deserialize_with = "deserialize_aptitude"
    )]
    medium_aptitude: Aptitude,
    #[serde(
        rename = "proper_distance_long",
        deserialize_with = "deserialize_aptitude"
    )]
    long_aptitude: Aptitude,
    #[serde(
        rename = "proper_ground_turf",
        deserialize_with = "deserialize_aptitude"
    )]
    turf_aptitude: Aptitude,
    #[serde(
        rename = "proper_ground_dirt",
        deserialize_with = "deserialize_aptitude"
    )]
    dirt_aptitude: Aptitude,
    #[serde(rename = "skill_array", deserialize_with = "deserialize_skills")]
    skills: Vec<UmaSkill>,
}

/// Deserialize a running style from its game value, from 1 (Front) to 4
/// (End).
///
/// Runaway isn't a running style in the game data, but the effect of a skill.
fn deserialize_style<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Style, D::Error> {
    match u8::deserialize(deserializer)? {
        1 => Ok(Style::Front),
        2 => Ok(Style::Pace),
        3 => Ok(Style::Late),
        4 => Ok(Style::End),
        value => Err(D::Error::custom(format!(
            "invalid running style {value}: expected 1 to 4"
        ))),
    }
}

/// Deserialize a mood from its motivation, from 1 (Awful) to 5 (Great).
fn deserialize_motivation<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<Mood>, D::Error> {
    let value = i8::deserialize(deserializer)?;
    Mood::from_motivation(value)
        .map(Some)
        .ok_or_else(|| D::Error::custom(format!("invalid motivation {value}: expected 1 to 5")))
}

/// Deserialize an aptitude from its game value, from 1 (G) to 8 (S).
fn deserialize_aptitude<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Aptitude, D::Error> {
    let value = u8::deserialize(deserializer)?;
    Aptitude::try_from(value).map_err(D::Error::custom)
}

/// Deserialize skills from objects with `skill_id` and `level`.
fn deserialize_skills<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<Vec<UmaSkill>, D::Error> {
    #[derive(Deserialize)]
    struct RawSkill {
        skill_id: i32,
        level: u8,
    }

    let skills = Vec::<RawSkill>::deserialize(deserializer)?;
    Ok(skills
        .into_iter()
        .map(|skill| UmaSkill {
            id: skill.skill_id,
            level: skill.level,
        })
        .collect())
}

/// A career turn as exported by umadump, with only the Uma in training.
#[derive(Deserialize)]
struct Career {
    single_mode_load_common: CareerCommon,
}

/// See [`Career`].
#[derive(Deserialize)]
struct CareerCommon {
    chara_info: UmaDumpData,
}

impl UmaDumpData {
    /// Parse `UmaDumpData` from a JSON string.
    ///
    /// # Errors
    ///
    /// Returns an error if the JSON is invalid, a stat is zero, or a running
    /// style or aptitude is out of range.
    ///
    /// # Examples
    ///
    /// Only the fields used by the simulation are required:
    ///
    /// ```
    /// use uma_tools::import::umadump::UmaDumpData;
    ///
    /// # fn main() -> Result<(), serde_json::Error> {
    /// let data = UmaDumpData::from_json(
    ///     r#"{
    ///         "card_id": 101901, "speed": 1305, "stamina": 495,
    ///         "power": 1143, "guts": 688, "wiz": 898, "fans": 172229,
    ///         "running_style": 3,
    ///         "proper_running_style_nige": 2,
    ///         "proper_running_style_senko": 7,
    ///         "proper_running_style_sashi": 7,
    ///         "proper_running_style_oikomi": 6,
    ///         "proper_distance_short": 2, "proper_distance_mile": 7,
    ///         "proper_distance_middle": 7, "proper_distance_long": 1,
    ///         "proper_ground_turf": 7, "proper_ground_dirt": 7,
    ///         "skill_array": [{ "skill_id": 100191, "level": 5 }]
    ///     }"#,
    /// )?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json)
    }

    /// Parse `UmaDumpData` from a JSON file.
    ///
    /// # Errors
    ///
    /// Returns an error if the file can't be read, or for the same reasons as
    /// [`UmaDumpData::from_json`].
    ///
    /// See the [module docs](self) for an example.
    pub fn from_json_file(
        path: impl AsRef<Path>,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let content = std::fs::read_to_string(path)?;
        Ok(Self::from_json(&content)?)
    }

    /// Parse a list of `UmaDumpData` from a JSON array, as umadump
    /// exports every trained Uma at once.
    ///
    /// # Errors
    ///
    /// Returns an error if the JSON isn't an array, or for the same reasons as
    /// [`UmaDumpData::from_json`] for any of its elements.
    pub fn all_from_json(json: &str) -> Result<Vec<Self>, serde_json::Error> {
        serde_json::from_str(json)
    }

    /// Parse a list of `UmaDumpData` from a JSON file with an array.
    ///
    /// # Errors
    ///
    /// Returns an error if the file can't be read, or for the same reasons as
    /// [`UmaDumpData::all_from_json`].
    ///
    /// # Examples
    ///
    /// ```
    /// use uma_tools::import::umadump::UmaDumpData;
    ///
    /// # fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    /// let all = UmaDumpData::all_from_json_file("uma.json")?;
    /// assert_eq!(all.len(), 18);
    /// # Ok(())
    /// # }
    /// ```
    pub fn all_from_json_file(
        path: impl AsRef<Path>,
    ) -> Result<Vec<Self>, Box<dyn std::error::Error + Send + Sync>> {
        let content = std::fs::read_to_string(path)?;
        Ok(Self::all_from_json(&content)?)
    }

    /// Parse `UmaDumpData` from the Uma in training in a career turn, as a
    /// JSON string.
    ///
    /// # Errors
    ///
    /// Returns an error if the JSON is invalid, the Uma is missing, or for
    /// the same reasons as [`UmaDumpData::from_json`] for the Uma, also if
    /// its mood is out of range.
    ///
    /// # Examples
    ///
    /// ```
    /// use uma_tools::course::{DistanceType, Track};
    /// use uma_tools::import::umadump::UmaDumpData;
    /// use uma_tools::{Mood, Style};
    ///
    /// # fn main() -> Result<(), serde_json::Error> {
    /// let data = UmaDumpData::from_career_json(
    ///     r#"{ "single_mode_load_common": { "chara_info": {
    ///         "card_id": 104601, "speed": 1168, "stamina": 682,
    ///         "power": 1050, "guts": 567, "wiz": 679, "fans": 313947,
    ///         "race_running_style": 1, "motivation": 5,
    ///         "proper_running_style_nige": 7,
    ///         "proper_running_style_senko": 4,
    ///         "proper_running_style_sashi": 1,
    ///         "proper_running_style_oikomi": 1,
    ///         "proper_distance_short": 6, "proper_distance_mile": 7,
    ///         "proper_distance_middle": 7, "proper_distance_long": 6,
    ///         "proper_ground_turf": 4, "proper_ground_dirt": 7,
    ///         "skill_array": [{ "skill_id": 100461, "level": 4 }]
    ///     } } }"#,
    /// )?;
    /// let uma_data = data.to_uma_data(DistanceType::Mile, Track::Dirt);
    /// assert_eq!(uma_data.uma.style, Style::Front);
    /// assert_eq!(uma_data.mood, Some(Mood::Great));
    /// # Ok(())
    /// # }
    /// ```
    pub fn from_career_json(json: &str) -> Result<Self, serde_json::Error> {
        let career: Career = serde_json::from_str(json)?;
        Ok(career.single_mode_load_common.chara_info)
    }

    /// Parse `UmaDumpData` from the Uma in training in a career turn, from a
    /// JSON file.
    ///
    /// # Errors
    ///
    /// Returns an error if the file can't be read, or for the same reasons as
    /// [`UmaDumpData::from_career_json`].
    pub fn from_career_json_file(
        path: impl AsRef<Path>,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let content = std::fs::read_to_string(path)?;
        Ok(Self::from_career_json(&content)?)
    }

    /// Convert `self` to [`UmaData`] for a race of the given distance type and
    /// track surface, with the mood if known.
    ///
    /// The distance and track aptitudes are the ones for `distance` and
    /// `track`, and the style aptitude the one for the Uma's running style.
    pub fn to_uma_data(&self, distance: DistanceType, track: Track) -> UmaData {
        let style_aptitude = match self.running_style {
            Style::Runaway | Style::Front => self.front_aptitude,
            Style::Pace => self.pace_aptitude,
            Style::Late => self.late_aptitude,
            Style::End => self.end_aptitude,
        };
        let distance_aptitude = match distance {
            DistanceType::Short => self.short_aptitude,
            DistanceType::Mile => self.mile_aptitude,
            DistanceType::Medium => self.medium_aptitude,
            DistanceType::Long => self.long_aptitude,
        };
        let track_aptitude = match track {
            Track::Turf => self.turf_aptitude,
            Track::Dirt => self.dirt_aptitude,
        };
        UmaData {
            uma: Uma {
                speed: self.speed,
                stamina: self.stamina,
                power: self.power,
                guts: self.guts,
                wit: self.wit,
                style: self.running_style,
                distance_aptitude,
                track_aptitude,
                style_aptitude,
                fan_count: Some(self.fans),
                character_id: Some(self.card_id),
            },
            mood: self.mood,
            skills: self.skills.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Path to an Uma exported by umadump, with every field.
    const UMA_PATH: &str = "tests/data/umadump_uma.json";

    /// Path to the last turn of a career exported by umadump, with only the
    /// Uma in training.
    const CAREER_PATH: &str = "tests/data/umadump_career.json";

    /// JSON for [`UmaDumpData`] with only the required fields, and
    /// distinct aptitudes (S to G, then S and A) in field order.
    const UMA_JSON: &str = r#"{
        "card_id": 100101,
        "speed": 1201, "stamina": 802, "power": 1003, "guts": 604,
        "wiz": 1105, "fans": 98765, "running_style": 1,
        "proper_running_style_nige": 8,
        "proper_running_style_senko": 7,
        "proper_running_style_sashi": 6,
        "proper_running_style_oikomi": 5,
        "proper_distance_short": 4, "proper_distance_mile": 3,
        "proper_distance_middle": 2, "proper_distance_long": 1,
        "proper_ground_turf": 8, "proper_ground_dirt": 7,
        "skill_array": [
            { "skill_id": 100011, "level": 4 },
            { "skill_id": 200012, "level": 1 }
        ]
    }"#;

    #[test]
    fn test_to_uma_data() {
        let data = UmaDumpData::from_json(UMA_JSON).unwrap();
        let expected = UmaData {
            uma: Uma {
                speed: 1201,
                stamina: 802,
                power: 1003,
                guts: 604,
                wit: 1105,
                style: Style::Front,
                distance_aptitude: Aptitude::D,
                track_aptitude: Aptitude::S,
                style_aptitude: Aptitude::S,
                fan_count: Some(98765),
                character_id: Some(100101),
            },
            mood: None,
            skills: vec![
                UmaSkill {
                    id: 100011,
                    level: 4,
                },
                UmaSkill {
                    id: 200012,
                    level: 1,
                },
            ],
        };
        let uma_data = data.to_uma_data(DistanceType::Short, Track::Turf);
        assert_eq!(uma_data, expected);
    }

    #[test]
    fn test_to_uma_data_picks_aptitudes() {
        let data = UmaDumpData::from_json(UMA_JSON).unwrap();
        let cases = [
            (DistanceType::Short, Track::Turf, Aptitude::D, Aptitude::S),
            (DistanceType::Mile, Track::Dirt, Aptitude::E, Aptitude::A),
            (DistanceType::Medium, Track::Turf, Aptitude::F, Aptitude::S),
            (DistanceType::Long, Track::Dirt, Aptitude::G, Aptitude::A),
        ];
        for (distance, track, distance_aptitude, track_aptitude) in cases {
            let uma = data.to_uma_data(distance, track).uma;
            assert_eq!(uma.distance_aptitude, distance_aptitude, "{distance:?}");
            assert_eq!(uma.track_aptitude, track_aptitude, "{track:?}");
        }
    }

    #[test]
    fn test_style_aptitude_follows_running_style() {
        let cases = [
            (1, Style::Front, Aptitude::S),
            (2, Style::Pace, Aptitude::A),
            (3, Style::Late, Aptitude::B),
            (4, Style::End, Aptitude::C),
        ];
        for (value, style, aptitude) in cases {
            let json = UMA_JSON.replace(
                r#""running_style": 1"#,
                &format!(r#""running_style": {value}"#),
            );
            let uma = UmaDumpData::from_json(&json)
                .unwrap()
                .to_uma_data(DistanceType::Mile, Track::Turf)
                .uma;
            assert_eq!(
                (uma.style, uma.style_aptitude),
                (style, aptitude),
                "{value}"
            );
        }
    }

    #[test]
    fn test_from_invalid_json() {
        let invalid = |from: &str, to: &str| {
            assert_eq!(UMA_JSON.matches(from).count(), 1, "{from}");
            let json = UMA_JSON.replace(from, to);
            assert!(UmaDumpData::from_json(&json).is_err(), "{to}");
        };
        invalid(r#""running_style": 1"#, r#""running_style": 0"#);
        invalid(r#""running_style": 1"#, r#""running_style": 5"#);
        invalid(r#""proper_ground_turf": 8"#, r#""proper_ground_turf": 0"#);
        invalid(r#""guts": 604"#, r#""guts": 0"#);
    }

    #[test]
    fn test_from_json_file() {
        let uma_data = UmaDumpData::from_json_file(UMA_PATH)
            .unwrap()
            .to_uma_data(DistanceType::Long, Track::Dirt);
        let uma = uma_data.uma;
        assert_eq!(
            (uma.speed, uma.stamina, uma.power, uma.guts, uma.wit),
            (1305, 495, 1143, 688, 898)
        );
        assert_eq!(uma.style, Style::Late);
        assert_eq!(
            (
                uma.distance_aptitude,
                uma.track_aptitude,
                uma.style_aptitude
            ),
            (Aptitude::G, Aptitude::A, Aptitude::A)
        );
        assert_eq!(uma.fan_count, Some(172229));
        assert_eq!(uma.character_id, Some(101901));
        assert_eq!(uma_data.skills.len(), 18);
        assert_eq!(
            uma_data.skills[..2],
            [
                UmaSkill {
                    id: 100191,
                    level: 5
                },
                UmaSkill {
                    id: 200072,
                    level: 1
                },
            ]
        );
    }

    /// Wrap the Uma in `json` as a career turn, with its running style
    /// renamed and the mood value given.
    fn career_json(json: &str, mood: i32) -> String {
        let chara_info = json
            .replace(r#""running_style""#, r#""race_running_style""#)
            .replace(
                r#""card_id""#,
                &format!(r#""motivation": {mood}, "card_id""#),
            );
        format!(r#"{{ "single_mode_load_common": {{ "chara_info": {chara_info} }} }}"#)
    }

    #[test]
    fn test_from_career_json() {
        let data = UmaDumpData::from_career_json(&career_json(UMA_JSON, 2)).unwrap();
        let expected = UmaDumpData {
            mood: Some(Mood::Bad),
            ..UmaDumpData::from_json(UMA_JSON).unwrap()
        };
        assert_eq!(data, expected);
        assert!(UmaDumpData::from_career_json(&career_json(UMA_JSON, 6)).is_err());
    }

    #[test]
    fn test_from_career_json_file() {
        let uma_data = UmaDumpData::from_career_json_file(CAREER_PATH)
            .unwrap()
            .to_uma_data(DistanceType::Mile, Track::Turf);
        let uma = uma_data.uma;
        assert_eq!(
            (uma.speed, uma.stamina, uma.power, uma.guts, uma.wit),
            (1168, 682, 1050, 567, 679)
        );
        assert_eq!(uma.style, Style::Front);
        assert_eq!(
            (
                uma.distance_aptitude,
                uma.track_aptitude,
                uma.style_aptitude
            ),
            (Aptitude::A, Aptitude::D, Aptitude::A)
        );
        assert_eq!(uma.fan_count, Some(313947));
        assert_eq!(uma.character_id, Some(104601));
        assert_eq!(uma_data.mood, Some(Mood::Great));
        assert_eq!(uma_data.skills.len(), 18);
        assert_eq!(
            uma_data.skills[0],
            UmaSkill {
                id: 100461,
                level: 4
            }
        );
    }

    #[test]
    fn test_all_from_json() {
        let single = UmaDumpData::from_json(UMA_JSON).unwrap();
        let other = UMA_JSON.replace(r#""card_id": 100101"#, r#""card_id": 100201"#);
        let all = UmaDumpData::all_from_json(&format!("[{UMA_JSON}, {other}]")).unwrap();
        assert_eq!(all.len(), 2);
        assert_eq!(all[0], single);
        assert_eq!(all[1].card_id, 100201);
    }
}
