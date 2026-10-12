//! Core data structures to represent an Umamusume (Uma).
//!
//! This module includes data structures for stats, aptitudes, running styles
//! and mood, as well as functions to convert raw stats into base stats.

use serde::Deserialize;

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
    #[serde(rename = "Senkou")]
    Pace = 2,
    /// 差し, sashi - Stays in the middle, behind front runners and pace chasers, and
    /// makes a late surge.
    #[serde(rename = "Sasi")]
    Late = 3,
    /// 追込, oikomi - Stays at the back and sprints towards the end.
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

/// Aptitude levels for distance, track surface, and running style.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Deserialize)]
pub enum Aptitude {
    S,
    A,
    B,
    C,
    D,
    E,
    F,
    G,
}

/// Mood or condition of the Uma. Affects base stat multipliers.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Deserialize)]
pub enum Motivation {
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
#[derive(Copy, Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Uma {
    /// Stat that affects max speed.
    pub speed: u16,
    /// Stat that affects max HP.
    pub stamina: u16,
    /// Stat that affects acceleration and lateral movement.
    pub power: u16,
    /// Stat that affects performance during struggles and last stride.
    ///
    /// Representing "the will to fight and overcome", it affects speed and
    /// acceleration during duels and spot struggles. Moreover, it affects HP
    /// consumption late-race and last spurt performance; the higher
    /// the Guts, the better the performance on the final stretch.
    pub guts: u16,
    /// Wit, also known as wisdom or wiz. Stat that affects skill activation rate
    /// and decision-making.
    ///
    /// Specifically, it affects skill activation rate, chance of avoiding getting
    /// rushed, position keeping, last spurt planning, and more.
    #[serde(rename = "wisdom")]
    pub wiz: u16,
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
    /// [`Configuration::default_fan_count`](crate::configuration::Configuration::default_fan_count)).
    pub fan_count: Option<u32>,
}

/// Helper struct for deserializing [`Uma`] with mood and skills from JSON.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UmaData {
    #[serde(flatten)]
    pub uma: Uma,
    /// Mood or condition of the Uma. Affects base stat multipliers.
    pub motivation: Motivation,
    /// List of [`skill`](crate::skill::Skill)s by ID that the Uma has.
    pub skills: Vec<u32>,
}

impl UmaData {
    /// Parse `UmaData` from a JSON string.
    ///
    /// Returns an error if the JSON is invalid.
    ///
    /// ```rust
    /// # use uma_tools::uma::UmaData;
    /// # fn try_main() -> Result<(), serde_json::Error> {
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
    /// # try_main().unwrap();
    /// ```
    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json)
    }

    /// Parse `UmaData` from a JSON file.
    ///
    /// Returns an error if the file cannot be read or the JSON is invalid.
    pub fn from_json_file(path: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let content = std::fs::read_to_string(path)?;
        Ok(Self::from_json(&content)?)
    }
}

/// Calculate base stat and apply mood multiplier.
///
/// Any stat above 1200 has its excess halved before applying multiplier.
pub fn calculate_base_stat(stat: u16, multiplier: f64) -> u16 {
    let mut computed = stat;
    if computed > 1200 {
        computed = 1200 + (computed - 1200) / 2;
    }
    ((computed).max(1) as f64 * multiplier) as u16
}

/// Calculate base stats multiplier from mood.
///
/// Returns `mood_value × 0.02 + 1.0`, where mood values range from -2 (awful)
/// to +2 (great).
pub fn motivation_multiplier(motivation: Motivation) -> f64 {
    1.0 + (motivation as i8 as f64) * 0.02
}

impl Uma {
    /// Convert raw Uma stats to base stats using mood multiplier.
    pub fn raw_to_base(raw: Uma, motivation: Motivation) -> Self {
        // N.B. In the Unity Cup (アオハル杯) scenario, a bonus is applied before mood
        // multiplier based on team rank. This is out of scope for this simulator.
        let motivation_multiplier = motivation_multiplier(motivation);
        Uma {
            speed: calculate_base_stat(raw.speed, motivation_multiplier),
            stamina: calculate_base_stat(raw.stamina, motivation_multiplier),
            power: calculate_base_stat(raw.power, motivation_multiplier),
            guts: calculate_base_stat(raw.guts, motivation_multiplier),
            wiz: calculate_base_stat(raw.wiz, motivation_multiplier),
            style: raw.style,
            distance_aptitude: raw.distance_aptitude,
            track_aptitude: raw.track_aptitude,
            style_aptitude: raw.style_aptitude,
            fan_count: raw.fan_count,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::assert_close;

    #[test]
    fn test_motivation_multiplier() {
        let cases = [
            (Motivation::Awful, 0.96),
            (Motivation::Bad, 0.98),
            (Motivation::Normal, 1.0),
            (Motivation::Good, 1.02),
            (Motivation::Great, 1.04),
        ];
        for (motivation, expected) in cases {
            assert_close!(
                motivation_multiplier(motivation),
                expected,
                "{motivation:?}"
            );
        }
    }

    #[test]
    fn test_calculate_base_stat() {
        // (raw stat, mood, base stat).
        let cases = [
            (1000, Motivation::Normal, 1000),
            (1200, Motivation::Normal, 1200),
            // Excess above 1200 is halved, rounding down.
            (1201, Motivation::Normal, 1200),
            (1500, Motivation::Normal, 1350),
            // Multiplier is applied after halving, truncating.
            (1000, Motivation::Awful, 960),
            (1500, Motivation::Great, 1404),
            (1199, Motivation::Good, 1222),
            // Stats are at least 1 before the multiplier.
            (0, Motivation::Normal, 1),
            (0, Motivation::Great, 1),
        ];
        for (stat, motivation, expected) in cases {
            let actual = calculate_base_stat(stat, motivation_multiplier(motivation));
            assert_eq!(actual, expected, "{stat} with {motivation:?}");
        }
    }

    #[test]
    fn test_raw_to_base() {
        let raw = Uma {
            speed: 1500,
            stamina: 800,
            power: 1200,
            guts: 700,
            wiz: 1300,
            style: Style::End,
            distance_aptitude: Aptitude::S,
            track_aptitude: Aptitude::B,
            style_aptitude: Aptitude::C,
            fan_count: Some(120000),
        };

        // Style, aptitudes and fan count are kept.
        let expected = Uma {
            speed: 1404,
            stamina: 832,
            power: 1248,
            guts: 728,
            wiz: 1300,
            ..raw
        };
        assert_eq!(Uma::raw_to_base(raw, Motivation::Great), expected);
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
                wiz: 1205,
                style: Style::Pace,
                distance_aptitude: Aptitude::S,
                track_aptitude: Aptitude::B,
                style_aptitude: Aptitude::C,
                fan_count: Some(98765),
            },
            motivation: Motivation::Bad,
            skills: vec![200642, 201161],
        };
        assert_eq!(UmaData::from_json(&uma_json("Senkou")).unwrap(), expected);
    }

    #[test]
    fn test_uma_data_style_names() {
        let cases = [
            ("Oonige", Style::Runaway),
            ("Nige", Style::Front),
            ("Senkou", Style::Pace),
            ("Sasi", Style::Late),
            ("Oikomi", Style::End),
        ];
        for (name, expected) in cases {
            let uma_data = UmaData::from_json(&uma_json(name)).unwrap();
            assert_eq!(uma_data.uma.style, expected, "{name}");
        }
    }

    #[test]
    fn test_uma_data_from_invalid_json() {
        // Unknown style.
        assert!(UmaData::from_json(&uma_json("Front")).is_err());
        // Missing field.
        let json = uma_json("Nige").replace(r#""motivation": "Bad","#, "");
        assert!(UmaData::from_json(&json).is_err());
        // Malformed.
        assert!(UmaData::from_json("{").is_err());
    }

    #[test]
    fn test_uma_data_from_json_file() {
        let uma_data = UmaData::from_json_file("uma.json").unwrap();
        assert_eq!(uma_data.uma.speed, 1500);
        assert_eq!(uma_data.uma.style, Style::End);
        assert_eq!(uma_data.motivation, Motivation::Great);
        assert_eq!(uma_data.skills, vec![200642]);
    }

    #[test]
    fn test_uma_data_from_missing_file() {
        assert!(UmaData::from_json_file("does_not_exist.json").is_err());
    }
}
