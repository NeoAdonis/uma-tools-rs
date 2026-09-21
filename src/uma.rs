//! Module containing core data structures to represent an Umamusume (Uma).
//! This includes stats, aptitudes, running styles, and motivation.
//! It also provides functions to calculate adjusted stats based on
//! track conditions, course modifiers, and aptitudes.

pub mod skill;

use crate::course::{ThresholdStat, Track};
use crate::race::parameters::TrackCondition;
use serde::Deserialize;

/// Running style or strategy of an Uma. The values represent in order, from most to
/// least aggressive:
///
/// - Runaway (大逃げ): A front runner that tries to outpace others from the start.
/// - Front runner (逃げ): Aims to lead the race.
/// - Pace chaser (先行): Stays near the front, behind the front runners (if any).
/// - Late surger (差し): Stays in the middle, behind front runners and pace chasers,
///   and makes a late surge.
/// - End closer (追込): Stays at the back and sprints towards the end.
///
/// ## Notes on Runaway vs Front
///
/// Runaway is a variant of Front running with more aggressive pacing. In many
/// contexts, they share the same behavior (e.g., positioning strategy, rush
/// calculations), but in specific situations (e.g., HP multipliers, phase modifiers,
/// spot struggle), they have distinct values.
///
/// When required to treat both Front and Runaway the same way, `is_front_like()` can
/// be used. Otherwise, explicit matching on `Style` should be done to handle their
/// distinct behaviors.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Deserialize, Hash, PartialOrd, Ord)]
#[serde(rename_all = "PascalCase")]
pub enum Style {
    /// 大逃げ, ōnige - A front runner that tries to outpace others from the start.
    /// In many contexts, it is treated similarly to `Style::Front` (see `Style`).
    #[serde(rename = "Oonige")]
    Runaway = 0,
    /// 逃げ, nige - Aims to lead the race.
    #[serde(rename = "Nige")]
    Front = 1,
    /// 先行, senkō - Stays near the front, behind the front runners (if any).
    #[serde(rename = "Senkou")]
    Pace = 2,
    /// 差し, sashi - Stays in the middle, behind front runners and pace chasers,
    /// and makes a late surge.
    #[serde(rename = "Sasi")]
    Late = 3,
    /// 追込, oikomi - Stays at the back and sprints towards the end.
    #[serde(rename = "Oikomi")]
    End = 4,
}

impl Style {
    /// Returns `true` if this style is a front-running style (Front or Runaway).
    ///
    /// Used when treating both Front and Runaway the same way is desired,
    /// (e.g., in positioning strategy or rush detection).
    ///
    /// # Examples
    ///
    /// ```rust
    /// use uma_tools::uma::Style;
    ///
    /// assert!(Style::Front.is_front_like());
    /// assert!(Style::Runaway.is_front_like());
    /// assert!(!Style::Pace.is_front_like());
    /// ```
    #[inline]
    pub fn is_front_like(self) -> bool {
        matches!(self, Style::Front | Style::Runaway)
    }

    /// Returns the base style, treating Runaway as a variant of Front.
    ///
    /// This maps Runaway to Front and returns all other styles unchanged.
    /// Useful to group styles for condition evaluation or statistical purposes.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use uma_tools::uma::Style;
    ///
    /// assert_eq!(Style::Runaway.base_style(), Style::Front);
    /// assert_eq!(Style::Front.base_style(), Style::Front);
    /// assert_eq!(Style::Pace.base_style(), Style::Pace);
    /// ```
    #[inline]
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

/// Motivation, mood or condition of the Uma. Affects base stat multipliers.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Deserialize)]
pub enum Motivation {
    /// 絶不調 (Very bad)
    Awful = -2,
    /// 不調 (Bad)
    Bad,
    /// 普通 (Normal)
    Normal,
    /// 好調 (Favorable)
    Good,
    /// 絶好調 (In top shape)
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
    /// Stat that represents "the will to fight and overcome".
    /// Affects speed and acceleration during duels and spot struggles.
    /// Moreover, it affects HP consumption late-race and last spurt performance;
    /// in short, the higher the Guts, the better the performance on the final stretch.
    pub guts: u16,
    /// a.k.a. wit, wisdom. Stat that represents how skillful the Uma is.
    /// Affects skill activation rate, chance of avoiding getting rushed,
    /// position keeping, last spurt planning, and more.
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
    /// Aptitude for the current running style. Affects wisdom performance depending
    /// on the running style.
    #[serde(rename = "strategyAptitude")]
    pub style_aptitude: Aptitude,
}

/// Helper struct for deserializing Uma with motivation and skills from JSON.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UmaData {
    #[serde(flatten)]
    pub uma: Uma,
    pub motivation: Motivation,
    pub skills: Vec<u32>,
}

impl UmaData {
    /// Parse UmaData from a JSON string.
    ///
    /// # Arguments
    ///
    /// - `json` - JSON string containing uma stats and skills.
    ///
    /// # Returns
    ///
    /// - `Ok(UmaData)` - The parsed UmaData.
    /// - `Err` - If the JSON is invalid.
    ///
    /// # Example
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

    /// Parse UmaData from a JSON file.
    ///
    /// # Arguments
    ///
    /// - `path` - Path to the JSON file.
    ///
    /// # Returns
    ///
    /// - `Ok(UmaData)` - The parsed UmaData.
    /// - `Err` - If the file can't be read or JSON is invalid.
    pub fn from_json_file(path: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let content = std::fs::read_to_string(path)?;
        Ok(Self::from_json(&content)?)
    }
}

/// Calculate track speed modifier based on track condition.
pub fn track_speed_modifier(track_condition: TrackCondition) -> u16 {
    match track_condition {
        TrackCondition::Heavy => 50,
        _ => 0,
    }
}

/// Calculate track power modifier based on track type and condition.
pub fn track_power_modifier(track: Track, track_condition: TrackCondition) -> u16 {
    match (track, track_condition) {
        (Track::Turf, TrackCondition::Firm) => 0,
        (Track::Turf, TrackCondition::Good) => 50,
        (Track::Turf, TrackCondition::Soft) => 50,
        (Track::Turf, TrackCondition::Heavy) => 50,
        (Track::Dirt, TrackCondition::Firm) => 100,
        (Track::Dirt, TrackCondition::Good) => 50,
        (Track::Dirt, TrackCondition::Soft) => 100,
        (Track::Dirt, TrackCondition::Heavy) => 100,
    }
}

/// Calculate wiz multiplier based on style aptitude.
pub fn wiz_style_multiplier(style_aptitude: Aptitude) -> f64 {
    match style_aptitude {
        Aptitude::S => 1.1,
        Aptitude::A => 1.0,
        Aptitude::B => 0.85,
        Aptitude::C => 0.75,
        Aptitude::D => 0.6,
        Aptitude::E => 0.4,
        Aptitude::F => 0.2,
        Aptitude::G => 0.1,
    }
}

/// Calculate base stat and apply motivation multiplier.
/// Any stat above 1200 has its excess halved before applying multiplier.
pub fn calculate_base_stat(stat: u16, multiplier: f64) -> u16 {
    let mut computed = stat;
    if computed > 1200 {
        computed = 1200 + (computed - 1200) / 2;
    }
    ((computed).max(1) as f64 * multiplier) as u16
}

/// Calculate adjusted speed after applying course and track modifiers.
pub fn calculate_adjusted_speed(
    base_speed: u16,
    track_condition: TrackCondition,
    course_modifier: f64,
) -> u16 {
    let modified_speed = (base_speed as f64 * course_modifier) as u16;
    let track_modifier = track_speed_modifier(track_condition);
    modified_speed.saturating_sub(track_modifier).max(1)
}

/// Calculate adjusted power after applying track modifier.
pub fn calculate_adjusted_power(
    base_power: u16,
    track: Track,
    track_condition: TrackCondition,
) -> u16 {
    let modifier = track_power_modifier(track, track_condition);
    base_power.saturating_sub(modifier).max(1)
}

/// Calculate adjusted wiz after applying style aptitude modifier.
pub fn calculate_adjusted_wiz(base_wiz: u16, style_aptitude: Aptitude) -> u16 {
    let aptitude_mod = wiz_style_multiplier(style_aptitude);
    ((base_wiz as f64 * aptitude_mod) as u16).max(1)
}

/// Given a ThresholdStat, return the corresponding base stat from an Uma.
pub fn get_base_stat_by_type(uma: &Uma, stat_type: ThresholdStat) -> u16 {
    match stat_type {
        ThresholdStat::Speed => uma.speed,
        ThresholdStat::Stamina => uma.stamina,
        ThresholdStat::Power => uma.power,
        ThresholdStat::Guts => uma.guts,
        ThresholdStat::Wiz => uma.wiz,
    }
}

/// Calculate course speed modifier based on course set status.
///
/// Each course has a set of stats that provide bonuses if the Uma's corresponding
/// base stats exceed certain thresholds. If multiple stats are present, their
/// bonuses are averaged.
pub fn course_speed_modifier(base: &Uma, course_set_status: &[ThresholdStat]) -> f64 {
    if course_set_status.is_empty() {
        return 1.0;
    }

    let total_bonus: f64 = course_set_status
        .iter()
        .map(|&stat_type| {
            let stat_value = get_base_stat_by_type(base, stat_type);
            threshold_stat_bonus(stat_value)
        })
        .sum();

    // Return average bonus as multiplier (1.0 + average bonus)
    1.0 + (total_bonus / course_set_status.len() as f64)
}

/// Calculate bonus multiplier based on threshold stat value.
fn threshold_stat_bonus(stat_value: u16) -> f64 {
    if stat_value <= 300 {
        0.05
    } else if stat_value <= 600 {
        0.1
    } else if stat_value <= 900 {
        0.15
    } else {
        0.2
    }
}

/// Calculate base stats multiplier from motivation.
///
/// Returns motivation_value × 0.02 + 1.0, where motivation values range from
/// -2 (awful) to +2 (great).
pub fn motivation_multiplier(motivation: Motivation) -> f64 {
    1.0 + (motivation as i8 as f64) * 0.02
}

impl Uma {
    /// Convert raw Uma stats to base stats using motivation multiplier.
    pub fn raw_to_base(raw: Uma, motivation: Motivation) -> Self {
        // N.B. In the Unity Cup (アオハル杯) scenario, a bonus is applied before motivation
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
        }
    }
}

// Random distribution implementation for Motivation
use rand::distr::{Distribution, StandardUniform};
use rand::prelude::*;

impl Distribution<Motivation> for StandardUniform {
    fn sample<R: Rng + ?Sized>(&self, rng: &mut R) -> Motivation {
        match rng.random_range(0..5) {
            0 => Motivation::Awful,
            1 => Motivation::Bad,
            2 => Motivation::Normal,
            3 => Motivation::Good,
            _ => Motivation::Great,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_motivation_multiplier() {
        // Normal = 0; multiplier should be 1.0
        assert_eq!(motivation_multiplier(Motivation::Normal), 1.0);

        // Great = +2; multiplier should be 1.04
        assert!((motivation_multiplier(Motivation::Great) - 1.04).abs() < 0.001);

        // Awful = -2; multiplier should be 0.96
        assert!((motivation_multiplier(Motivation::Awful) - 0.96).abs() < 0.001);
    }
}
