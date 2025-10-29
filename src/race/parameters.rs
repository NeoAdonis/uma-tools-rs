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
    // n.b. Besides G1 races, important races (e.g. URA Finals, ChamMe, etc.)
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
    pub popularity: i32,
    pub gates: u8,
    pub rng_seed: u64,
}
