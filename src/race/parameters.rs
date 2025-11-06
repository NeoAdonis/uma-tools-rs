#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum TrackCondition {
    Firm = -1, // 良
    Good,      // 稍重
    Soft,      // 重
    Heavy,     // 不良
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Weather {
    Sunny = 1, // 晴
    Cloudy,    // 曇
    Rainy,     // 雨
    Snowy,     // 雪
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Season {
    Spring = 1, // 春
    Summer,     // 夏
    Autumn,     // 秋
    Winter,     // 冬
    Sakura,     // 桜, Cherry Blossom Season; a subset of Spring
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Time {
    None,
    Morning, // 午前
    Noon,    // 正午
    Evening, // 夕方
    Night,   // 夜
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Grade {
    // N.B. Besides G1 races, important races (e.g. URA Finals, ChamMe, etc.)
    // generally fall under this category as well.
    G1 = 100,
    G2 = 200,
    G3 = 300,
    OP = 400,
    PreOP = 700,
    Maiden = 800,
    Debut = 900,
    Daily = 999,
}

pub struct RaceParameters {
    pub condition: TrackCondition,
    pub weather: Weather,
    pub season: Season,
    pub time: Time,
    pub grade: Grade,
    pub gates: u8,
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
