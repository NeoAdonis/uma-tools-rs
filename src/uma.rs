//! Core data structures to represent an Umamusume (Uma).
//!
//! This module includes data structures for stats, aptitudes, running styles
//! and motivation, as well as functions to convert raw stats into base stats.

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
/// When required to treat both Front and Runaway the same way, [`Style::is_front_like`] can
/// be used. Otherwise, explicit matching on `Style` should be done to handle their
/// distinct behaviors.
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
    /// 差し, sashi - Stays in the middle, behind front runners and pace chasers,
    /// and makes a late surge.
    #[serde(rename = "Sasi")]
    Late = 3,
    /// 追込, oikomi - Stays at the back and sprints towards the end.
    #[serde(rename = "Oikomi")]
    End = 4,
}

impl Style {
    /// Returns `true` if `self` is a front-running style ([`Style::Front`] or
    /// [`Style::Runaway`]).
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

    /// Returns the base style, treating [`Style::Runaway`] as a variant of
    /// [`Style::Front`] while leaving all other styles unchanged.
    ///
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
    /// Stat that affects performance during struggles and last stride.
    ///
    /// Representing "the will to fight and overcome", it affects speed and
    /// acceleration during duels and spot struggles. Moreover, it affects HP
    /// consumption late-race and last spurt performance; the higher
    /// the Guts, the better the performance on the final stretch.
    pub guts: u16,
    /// a.k.a. wit, wisdom. Stat that affects skill activation rate and
    /// decision-making.
    ///
    /// It affects skill activation rate, chance of avoiding getting rushed,
    /// position keeping, last spurt planning, and more.
    #[serde(rename = "wisdom")]
    pub wiz: u16,
    /// Running style or strategy. Affects mainly positioning strategy.
    #[serde(rename = "strategy")]
    pub style: Style,
    /// Aptitude for the current distance. Affects speed performance depending
    /// on the course distance.
    pub distance_aptitude: Aptitude,
    /// Aptitude for the current track surface. Affects acceleration and power
    /// performance depending on the track type.
    #[serde(rename = "surfaceAptitude")]
    pub track_aptitude: Aptitude,
    /// Aptitude for the current running style. Affects wisdom performance
    /// depending on the running style.
    #[serde(rename = "strategyAptitude")]
    pub style_aptitude: Aptitude,
}

/// Helper struct for deserializing [`Uma`] with motivation and skills from
/// JSON.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UmaData {
    #[serde(flatten)]
    pub uma: Uma,
    /// Motivation, mood or condition of the Uma. Affects base stat multipliers.
    pub motivation: Motivation,
    /// List of [`skill`](skill::Skill)s by ID that the Uma has.
    pub skills: Vec<u32>,
}

impl UmaData {
    /// Parse `UmaData` from a JSON string.
    ///
    /// Returns a `Result` containing the parsed `UmaData` or an error if the
    /// JSON is invalid.
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
    /// Returns a `Result` containing the parsed `UmaData` or an error if the
    /// file cannot be read or the JSON is invalid.
    pub fn from_json_file(path: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let content = std::fs::read_to_string(path)?;
        Ok(Self::from_json(&content)?)
    }
}

/// Calculate base stat and apply motivation multiplier.
///
/// Any stat above 1200 has its excess halved before applying multiplier.
pub fn calculate_base_stat(stat: u16, multiplier: f64) -> u16 {
    let mut computed = stat;
    if computed > 1200 {
        computed = 1200 + (computed - 1200) / 2;
    }
    ((computed).max(1) as f64 * multiplier) as u16
}

/// Calculate base stats multiplier from motivation.
///
/// Returns `motivation_value × 0.02 + 1.0`, where motivation values range from
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

    #[test]
    fn test_uma_stat_calculations() {
        let raw_uma = Uma {
            speed: 1500,
            stamina: 800,
            power: 1200,
            guts: 700,
            wiz: 1200,
            style: Style::End,
            distance_aptitude: Aptitude::S,
            track_aptitude: Aptitude::A,
            style_aptitude: Aptitude::A,
        };

        // Test motivation effects
        let base_uma = Uma::raw_to_base(raw_uma, Motivation::Normal);

        // Normal motivation should not change stats significantly
        // (motivation multiplier is 1.0 for Normal)
        assert!(base_uma.speed > 0);
        assert!(base_uma.stamina > 0);

        // Test Great motivation (positive boost)
        let great_uma = Uma::raw_to_base(raw_uma, Motivation::Great);
        assert!(great_uma.speed > base_uma.speed);

        // Test Awful motivation (negative impact)
        let awful_uma = Uma::raw_to_base(raw_uma, Motivation::Awful);
        assert!(awful_uma.speed < base_uma.speed);
    }
}
