//! Race parameters that define the conditions of a race.

/// Track condition enumeration.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum TrackCondition {
    /// 良
    Firm = -1,
    /// 稍重
    Good,
    /// 重
    Soft,
    /// 不良
    Heavy,
}

/// Weather enumeration.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Weather {
    /// 晴
    Sunny = 1,
    /// 曇
    Cloudy,
    /// 雨
    Rainy,
    /// 雪
    Snowy,
}

/// Season enumeration.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Season {
    /// 春
    Spring = 1,
    /// 夏
    Summer,
    /// 秋
    Autumn,
    /// 冬
    Winter,
    /// 桜 - Cherry Blossom Season. As a subset of Spring, any conditions
    /// that apply to Spring also apply to Cherry Blossom Season.
    Sakura,
}

/// Time of day enumeration.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
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
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Grade {
    /// G1 races. Other important races (e.g. URA Finals, Champions Meeting, etc.)
    /// generally fall under this category as well.
    G1 = 100,
    /// G2 races.
    G2 = 200,
    G3 = 300,
    /// OP races.
    OP = 400,
    /// Pre-OP races.
    PreOP = 700,
    /// Maiden races; the only races that runners can enter before
    /// winning their first race.
    Maiden = 800,
    /// Make Debut races.
    Debut = 900,
    /// Daily races.
    Daily = 999,
}

/// Struct representing the parameters of a race.
pub struct RaceParameters {
    pub condition: TrackCondition,
    pub weather: Weather,
    pub season: Season,
    pub time: Time,
    pub grade: Grade,
    /// Number of gates (participants) in the race.
    pub gates: u8,
    /// Seed for pseudo-random number generation. Ensures reproducibility.
    pub rng_seed: u64,
}

// Random distribution implementations for race parameters
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
