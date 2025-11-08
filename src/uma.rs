pub mod skill;

use crate::course::{ThresholdStat, Track};
use crate::race::parameters::TrackCondition;
use serde::Deserialize;

/// Running style or strategy of the Uma. The values represent, in order:
///
/// - Front runner (逃げ): Leads the race.
/// - Pace chaser (先行): Stays near the front behind the leaders.
/// - Late surger (差し): Stays in the middle and makes a late charge.
/// - End closer (追込): Stays at the back and sprints at the end.
/// - Runaway (大逃げ): A front runner with even more aggressive pacing.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Deserialize, Hash)]
#[serde(rename_all = "PascalCase")]
pub enum Style {
    #[serde(rename = "Nige")]
    Front = 1,
    #[serde(rename = "Senkou")]
    Pace,
    #[serde(rename = "Sasi")]
    Late,
    #[serde(rename = "Oikomi")]
    End,
    #[serde(rename = "Oonige")]
    Runaway,
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
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
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
    /// It affects speed and acceleration during duels and spot struggles.
    /// Moreover, it affects HP consumption late-race and last spurt performance;
    /// in short, the higher the Guts, the better the performance on the final stretch.
    pub guts: u16,
    /// A.k.a. wit, wisdom. Stat that represents how skillful the Uma is.
    ///
    /// It affects skill activation rate, chance of avoiding getting rushed,
    /// position keeping, last spurt planning, and more.
    #[serde(rename = "wisdom")]
    pub wiz: u16,
    /// Running style of the Uma.
    #[serde(rename = "strategy")]
    pub style: Style,
    /// Aptitude for the current distance.
    pub distance_aptitude: Aptitude,
    /// Aptitude for the current track surface.
    #[serde(rename = "surfaceAptitude")]
    pub track_aptitude: Aptitude,
    /// Aptitude for the current running style.
    #[serde(rename = "strategyAptitude")]
    pub style_aptitude: Aptitude,
}

/// Helper struct for deserializing Uma with skills from JSON
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UmaData {
    #[serde(flatten)]
    pub uma: Uma,
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
    /// ```ignore
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
    ///     "skills": [200642]
    /// }"#;
    /// let uma_data = UmaData::from_json(json)?;
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
pub fn track_speed_modifier(condition: TrackCondition) -> u16 {
    match condition {
        TrackCondition::Heavy => 50,
        _ => 0,
    }
}

/// Calculate track power modifier based on track type and condition.
pub fn track_power_modifier(track: Track, condition: TrackCondition) -> u16 {
    match (track, condition) {
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

/// Calculate wiz style multiplier based on aptitude.
pub fn wiz_style_multiplier(aptitude: Aptitude) -> f64 {
    match aptitude {
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
    condition: TrackCondition,
    course_modifier: f64,
) -> u16 {
    let modified_speed = (base_speed as f64 * course_modifier) as u16;
    let track_modifier = track_speed_modifier(condition);
    modified_speed.saturating_sub(track_modifier).max(1)
}

/// Calculate adjusted power after applying track modifier.
pub fn calculate_adjusted_power(base_power: u16, track: Track, condition: TrackCondition) -> u16 {
    let modifier = track_power_modifier(track, condition);
    base_power.saturating_sub(modifier).max(1)
}

/// Calculate adjusted wiz after applying style aptitude modifier.
pub fn calculate_adjusted_wiz(base_wiz: u16, track_aptitude: Aptitude) -> u16 {
    let aptitude_mod = wiz_style_multiplier(track_aptitude);
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

impl Uma {
    /// Convert raw Uma stats to base stats using motivation multiplier.
    pub fn raw_to_base(raw: Uma, motivation: Motivation) -> Self {
        // N.B. In the Unity Cup (アオハル杯) scenario, a bonus is applied before motivation
        // multiplier based on team rank. This is out of scope for this simulator.
        let motivation_multiplier = 1.0 + (motivation as i8 as f64) * 0.02;
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
