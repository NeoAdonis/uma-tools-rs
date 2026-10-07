//! Race parameters that define the conditions of a race.

use serde::Deserialize;

/// Track condition (a.k.a. ground condition) enumeration.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Deserialize)]
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
#[derive(Copy, Clone, Debug, PartialEq, Eq, Deserialize)]
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
#[derive(Copy, Clone, Debug, PartialEq, Eq, Deserialize)]
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
#[derive(Copy, Clone, Debug, PartialEq, Eq, Deserialize)]
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
#[derive(Copy, Clone, Debug, PartialEq, Eq, Deserialize)]
pub enum Grade {
    /// Grade 1 races, as well as other important races (e.g. URA Finals, Champions
    /// Meeting, etc.).
    G1 = 100,
    /// Grade 2 races.
    G2 = 200,
    /// Grade 3 races.
    G3 = 300,
    /// OP races.
    OP = 400,
    /// Pre-OP races.
    PreOP = 700,
    /// Maiden races; the only races that runners can enter before winning their
    /// first race.
    Maiden = 800,
    /// Make Debut races.
    Debut = 900,
    /// Daily races.
    Daily = 999,
}

/// Struct representing the parameters of a race.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RaceParameters {
    /// Track condition. Affects speed, power and HP consumption.
    pub track_condition: TrackCondition,
    pub weather: Weather,
    pub season: Season,
    pub time: Time,
    pub grade: Grade,
    /// Number of gates (participants) in the race.
    pub gates: u8,
}

/// On-disk representation of `parameters.json`.
///
/// Bundles `RaceParameters` together with `course_id`.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RaceParametersData {
    #[serde(flatten)]
    pub race: RaceParameters,
    pub course_id: String,
}

impl RaceParametersData {
    /// Parse `RaceParameters` from a JSON string.
    ///
    /// Returns an error if the JSON is invalid.
    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json)
    }

    /// Parse `RaceParameters` from a JSON file.
    ///
    /// Returns an error if the file can't be read or JSON is invalid.
    pub fn from_json_file(path: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let content = std::fs::read_to_string(path)?;
        Ok(Self::from_json(&content)?)
    }
}

// Random distribution implementations for race parameters.
use rand::distr::{Distribution, StandardUniform};
use rand::prelude::*;

impl Distribution<TrackCondition> for StandardUniform {
    fn sample<R: Rng + ?Sized>(&self, rng: &mut R) -> TrackCondition {
        match rng.random_range(0..4) {
            0 => TrackCondition::Firm,
            1 => TrackCondition::Good,
            2 => TrackCondition::Soft,
            _ => TrackCondition::Heavy,
        }
    }
}

impl Distribution<Weather> for StandardUniform {
    fn sample<R: Rng + ?Sized>(&self, rng: &mut R) -> Weather {
        match rng.random_range(0..4) {
            0 => Weather::Sunny,
            1 => Weather::Cloudy,
            2 => Weather::Rainy,
            _ => Weather::Snowy,
        }
    }
}

impl Distribution<Season> for StandardUniform {
    fn sample<R: Rng + ?Sized>(&self, rng: &mut R) -> Season {
        match rng.random_range(0..5) {
            0 => Season::Spring,
            1 => Season::Summer,
            2 => Season::Autumn,
            3 => Season::Winter,
            _ => Season::Sakura,
        }
    }
}

impl Distribution<Time> for StandardUniform {
    fn sample<R: Rng + ?Sized>(&self, rng: &mut R) -> Time {
        match rng.random_range(0..4) {
            0 => Time::Morning,
            1 => Time::Noon,
            2 => Time::Evening,
            _ => Time::Night,
        }
    }
}

impl Distribution<Grade> for StandardUniform {
    fn sample<R: Rng + ?Sized>(&self, rng: &mut R) -> Grade {
        match rng.random_range(0..7) {
            0 => Grade::G1,
            1 => Grade::G2,
            2 => Grade::G3,
            3 => Grade::OP,
            4 => Grade::PreOP,
            5 => Grade::Maiden,
            _ => Grade::Debut,
        }
    }
}
