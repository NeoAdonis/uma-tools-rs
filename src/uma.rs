use crate::course::*;
use crate::race_parameters::*;
use serde::Deserialize;

#[derive(Copy, Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum Style {
    // a.k.a. Strategy
    #[serde(rename = "Nige")]
    Front = 1, // Front runner, 逃げ
    #[serde(rename = "Senkou")]
    Pace, // Pace chaser, 先行
    #[serde(rename = "Sasi")]
    Late, // Late surger, 差し
    #[serde(rename = "Oikomi")]
    End, // End closer, 追込
    #[serde(rename = "Oonige")]
    Oonige, // 大逃げ
}

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

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Motivation {
    // a.k.a. Mood
    Awful = -2, // 絶不調
    Bad,        // 不調
    Normal,     // 普通
    Good,       // 好調
    Great,      // 絶好調
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Uma {
    pub speed: u16,
    pub stamina: u16,
    pub power: u16,
    pub guts: u16,
    #[serde(rename = "wisdom")]
    pub wiz: u16, // a.k.a. Wit
    #[serde(rename = "strategy")]
    pub style: Style,
    pub distance_aptitude: Aptitude,
    #[serde(rename = "surfaceAptitude")]
    pub track_aptitude: Aptitude,
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

pub fn track_speed_modifier(track: Track, condition: TrackCondition) -> u16 {
    match (track, condition) {
        (Track::Turf, TrackCondition::Firm) => 0,
        (Track::Turf, TrackCondition::Good) => 0,
        (Track::Turf, TrackCondition::Soft) => 0,
        (Track::Turf, TrackCondition::Heavy) => 50,
        (Track::Dirt, TrackCondition::Firm) => 0,
        (Track::Dirt, TrackCondition::Good) => 0,
        (Track::Dirt, TrackCondition::Soft) => 0,
        (Track::Dirt, TrackCondition::Heavy) => 50,
    }
}

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

pub fn wiz_style_coefficient(aptitude: Aptitude) -> f64 {
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

pub fn calculate_base_stat(stat: u16, coefficient: f64) -> u16 {
    let mut computed = stat;
    if computed > 1200 {
        computed = 1200 + (computed - 1200) / 2;
    }
    ((computed).max(1) as f64 * coefficient) as u16
}

pub fn calculate_adjusted_speed(
    base_speed: u16,
    track: Track,
    condition: TrackCondition,
    course_modifier: f64,
) -> u16 {
    let modified_speed = (base_speed as f64 * course_modifier) as u16;
    let track_modifier = track_speed_modifier(track, condition);
    modified_speed.saturating_sub(track_modifier).max(1)
}

pub fn calculate_adjusted_power(base_power: u16, track: Track, condition: TrackCondition) -> u16 {
    let modifier = track_power_modifier(track, condition);
    base_power.saturating_sub(modifier).max(1)
}

pub fn calculate_adjusted_wiz(base_wiz: u16, track_aptitude: Aptitude) -> u16 {
    let aptitude_mod = wiz_style_coefficient(track_aptitude);
    ((base_wiz as f64 * aptitude_mod) as u16).max(1)
}

pub fn threshold_stat_bonus(stat_value: u16) -> f64 {
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

pub fn get_base_stat_by_type(uma: &Uma, stat_type: ThresholdStat) -> u16 {
    match stat_type {
        ThresholdStat::Speed => uma.speed,
        ThresholdStat::Stamina => uma.stamina,
        ThresholdStat::Power => uma.power,
        ThresholdStat::Guts => uma.guts,
        ThresholdStat::Wiz => uma.wiz,
    }
}

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

impl Uma {
    pub fn raw_to_base(raw: Uma, motivation: Motivation) -> Self {
        // n.b.: In the Unity Cup (アオハル杯) scenario, a bonus is applied before motivation
        // multiplier based on team rank. This is out of scope for this simulator.
        let motivation_coefficient = 1.0 + (motivation as i8 as f64) * 0.02;
        Uma {
            speed: calculate_base_stat(raw.speed, motivation_coefficient),
            stamina: calculate_base_stat(raw.stamina, motivation_coefficient),
            power: calculate_base_stat(raw.power, motivation_coefficient),
            guts: calculate_base_stat(raw.guts, motivation_coefficient),
            wiz: calculate_base_stat(raw.wiz, motivation_coefficient),
            style: raw.style,
            distance_aptitude: raw.distance_aptitude,
            track_aptitude: raw.track_aptitude,
            style_aptitude: raw.style_aptitude,
        }
    }
}
