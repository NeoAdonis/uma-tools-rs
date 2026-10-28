//! Race parameters that define the conditions of a race.
//!
//! # Examples
//!
//! Set up the same race in different conditions, e.g. to see how much a
//! runner loses on heavy ground (see [`RaceSolver`] for how to compare
//! races):
//!
//! ```
//! use uma_tools::race::parameters::*;
//!
//! let firm = RaceParameters {
//!     track_condition: TrackCondition::Firm,
//!     weather: Weather::Sunny,
//!     season: Season::Spring,
//!     time: Time::Noon,
//!     grade: Grade::G1,
//!     gates: 18,
//! };
//! let heavy = RaceParameters {
//!     track_condition: TrackCondition::Heavy,
//!     weather: Weather::Rainy,
//!     ..firm
//! };
//! ```
//!
//! [`RaceSolver`]: crate::race::solver::RaceSolver

use serde::{Deserialize, Deserializer, de::Error};
use std::path::Path;

/// Track condition (a.k.a. ground condition) enumeration.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Deserialize)]
pub enum TrackCondition {
    /// 良
    Firm = 1,
    /// 稍重
    Good = 2,
    /// 重
    Soft = 3,
    /// 不良
    Heavy = 4,
}

/// Weather enumeration.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Deserialize)]
pub enum Weather {
    /// 晴
    Sunny = 1,
    /// 曇
    Cloudy = 2,
    /// 雨
    Rainy = 3,
    /// 雪
    Snowy = 4,
}

/// Season enumeration.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Deserialize)]
pub enum Season {
    /// 春
    Spring = 1,
    /// 夏
    Summer = 2,
    /// 秋
    Autumn = 3,
    /// 冬
    Winter = 4,
    /// 桜 - Cherry Blossom (Sakura). It is considered part of Spring, therefore
    /// skills that are supposed to activate in Spring will generally have another
    /// condition to check for Sakura season as well.
    Sakura = 5,
}

/// Time of day enumeration.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Deserialize)]
pub enum Time {
    None,
    /// 午前
    Morning,
    /// 正午
    Noon,
    /// 夕方
    Evening,
    /// 夜
    Night,
}

/// Race grade enumeration.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Deserialize)]
pub enum Grade {
    /// Grade 1 races, as well as other important races (e.g. URA Finals, Champions
    /// Meeting, etc.).
    G1 = 100,
    /// Grade 2 races.
    G2 = 200,
    /// Grade 3 races.
    G3 = 300,
    /// OP races.
    #[serde(rename = "OP")]
    Op = 400,
    /// Pre-OP races.
    #[serde(rename = "PreOP")]
    PreOp = 700,
    /// Maiden races; the only races that runners can enter before winning their
    /// first race.
    Maiden = 800,
    /// Make Debut races.
    Debut = 900,
    /// Daily races.
    Daily = 999,
}

/// Struct representing the parameters of a race.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RaceParameters {
    /// Track condition. Affects speed, power and HP consumption.
    pub track_condition: TrackCondition,
    pub weather: Weather,
    pub season: Season,
    pub time: Time,
    pub grade: Grade,
    /// Number of gates (participants) in the race, from 1 to [`MAX_GATES`].
    #[serde(deserialize_with = "deserialize_gates")]
    pub gates: u8,
}

/// Maximum number of gates (participants) in a race.
///
/// Popularity relies on it: with 26 or more runners, φ can reach zero or below,
/// which makes judge impacts infinite or NaN.
pub const MAX_GATES: u8 = 18;

fn deserialize_gates<'de, D: Deserializer<'de>>(deserializer: D) -> Result<u8, D::Error> {
    let gates = u8::deserialize(deserializer)?;
    if (1..=MAX_GATES).contains(&gates) {
        Ok(gates)
    } else {
        Err(D::Error::custom(format!(
            "invalid gates {gates}: expected 1 to {MAX_GATES}"
        )))
    }
}

/// On-disk representation of `parameters.json`.
///
/// Bundles [`RaceParameters`] together with a course ID.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RaceParametersData {
    #[serde(flatten)]
    pub race: RaceParameters,
    pub course_id: String,
}

impl RaceParametersData {
    /// Parse `RaceParametersData` from a JSON string.
    ///
    /// # Errors
    ///
    /// Returns an error if the JSON is invalid.
    ///
    /// See the [crate-level example](crate) for an example.
    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json)
    }

    /// Parse `RaceParametersData` from a JSON file.
    ///
    /// # Errors
    ///
    /// Returns an error if the file can't be read or the JSON is invalid.
    ///
    /// See the [crate-level example](crate) for an example.
    pub fn from_json_file(
        path: impl AsRef<Path>,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let content = std::fs::read_to_string(path)?;
        Ok(Self::from_json(&content)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// JSON for [`RaceParametersData`] with every field set, and the weather as
    /// given.
    fn parameters_json(weather: &str) -> String {
        format!(
            r#"{{
                "trackCondition": "Soft",
                "weather": "{weather}",
                "season": "Sakura",
                "time": "Evening",
                "grade": "PreOP",
                "gates": 12,
                "courseId": "10101"
            }}"#
        )
    }

    #[test]
    fn test_from_json() {
        let expected = RaceParametersData {
            race: RaceParameters {
                track_condition: TrackCondition::Soft,
                weather: Weather::Snowy,
                season: Season::Sakura,
                time: Time::Evening,
                grade: Grade::PreOp,
                gates: 12,
            },
            course_id: "10101".to_string(),
        };
        let actual = RaceParametersData::from_json(&parameters_json("Snowy")).unwrap();
        assert_eq!(actual, expected);
    }

    /// Parse a single enum value from its JSON name.
    fn parse<T: serde::de::DeserializeOwned>(name: &str) -> T {
        serde_json::from_str(&format!("\"{name}\"")).unwrap()
    }

    #[test]
    fn test_variant_names() {
        let track_conditions = [
            ("Firm", TrackCondition::Firm),
            ("Good", TrackCondition::Good),
            ("Soft", TrackCondition::Soft),
            ("Heavy", TrackCondition::Heavy),
        ];
        for (name, expected) in track_conditions {
            assert_eq!(parse::<TrackCondition>(name), expected, "{name}");
        }

        let weathers = [
            ("Sunny", Weather::Sunny),
            ("Cloudy", Weather::Cloudy),
            ("Rainy", Weather::Rainy),
            ("Snowy", Weather::Snowy),
        ];
        for (name, expected) in weathers {
            assert_eq!(parse::<Weather>(name), expected, "{name}");
        }

        let seasons = [
            ("Spring", Season::Spring),
            ("Summer", Season::Summer),
            ("Autumn", Season::Autumn),
            ("Winter", Season::Winter),
            ("Sakura", Season::Sakura),
        ];
        for (name, expected) in seasons {
            assert_eq!(parse::<Season>(name), expected, "{name}");
        }

        let times = [
            ("None", Time::None),
            ("Morning", Time::Morning),
            ("Noon", Time::Noon),
            ("Evening", Time::Evening),
            ("Night", Time::Night),
        ];
        for (name, expected) in times {
            assert_eq!(parse::<Time>(name), expected, "{name}");
        }

        let grades = [
            ("G1", Grade::G1),
            ("G2", Grade::G2),
            ("G3", Grade::G3),
            ("OP", Grade::Op),
            ("PreOP", Grade::PreOp),
            ("Maiden", Grade::Maiden),
            ("Debut", Grade::Debut),
            ("Daily", Grade::Daily),
        ];
        for (name, expected) in grades {
            assert_eq!(parse::<Grade>(name), expected, "{name}");
        }
    }

    #[test]
    fn test_from_invalid_json() {
        let valid = parameters_json("Sunny");
        let mut without_course_id: serde_json::Value = serde_json::from_str(&valid).unwrap();
        without_course_id
            .as_object_mut()
            .unwrap()
            .remove("courseId");

        let invalid = [
            // Unknown variant.
            parameters_json("Foggy"),
            // Missing course ID.
            without_course_id.to_string(),
            // Course ID as a number.
            valid.replace(r#""courseId": "10101""#, r#""courseId": 10101"#),
            // No gates.
            valid.replace(r#""gates": 12"#, r#""gates": 0"#),
            // Too many gates.
            valid.replace(r#""gates": 12"#, r#""gates": 19"#),
            // Too many gates for a `u8`.
            valid.replace(r#""gates": 12"#, r#""gates": 256"#),
            // Malformed.
            "{".to_string(),
        ];
        for json in invalid {
            assert!(RaceParametersData::from_json(&json).is_err(), "{json}");
        }
    }

    #[test]
    fn test_from_json_file() {
        let parameters = RaceParametersData::from_json_file("parameters.json").unwrap();
        assert_eq!(parameters.race.track_condition, TrackCondition::Firm);
        assert_eq!(parameters.race.grade, Grade::G1);
        assert_eq!(parameters.race.gates, 18);
        assert_eq!(parameters.course_id, "10913");
    }

    #[test]
    fn test_from_missing_file() {
        assert!(RaceParametersData::from_json_file("does_not_exist.json").is_err());
    }
}
