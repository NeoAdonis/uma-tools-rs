include!("track_names.rs");

use serde::Deserialize;

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]

pub enum Phase {
    Early,
    Mid,
    Late,
    Last, // Last spurt
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Track {
    Turf = 1, // 芝
    Dirt,     // ダート
}

pub enum Direction {
    Left = 1, // Clockwise
    Right,    // Counterclockwise
    Unused,
    Straight, // No curves
}

pub enum DistanceType {
    Short = 1, // less than 1600m
    Mile,      // less than 2000m
    Medium,    // less than 2400m
    Long,
}

pub enum ThresholdStat {
    Speed = 1,
    Stamina,
    Power,
    Guts,
    Wit,
}

impl TryFrom<i32> for ThresholdStat {
    type Error = ();

    fn try_from(value: i32) -> Result<Self, Self::Error> {
        match value {
            1 => Ok(ThresholdStat::Speed),
            2 => Ok(ThresholdStat::Stamina),
            3 => Ok(ThresholdStat::Power),
            4 => Ok(ThresholdStat::Guts),
            5 => Ok(ThresholdStat::Wit),
            _ => Err(()),
        }
    }
}

pub struct Corner {
    pub start: u32,  // in meters
    pub length: u32, // in meters
}

pub struct Slope {
    pub start: u32,  // in meters
    pub length: u32, // in meters
    pub slope: i32,
}

pub enum FrontType {
    Audience = 1,
    Opposite,
}

pub struct Straight {
    pub start: u32, // in meters
    pub end: u32,   // in meters
    pub front_type: FrontType,
}

pub struct Course {
    pub id: u16,
    pub distance: i32, // in meters
    pub distance_type: DistanceType,
    pub surface: Track,
    pub turn: Direction,
    pub course_set_status: Vec<i32>,
    pub finish_time_min: u32,
    pub finish_time_max: u32,
    pub lane_max: u32,
    pub corners: Vec<Corner>,
    pub slopes: Vec<Slope>,
    pub straights: Vec<Straight>,
    pub race_track_id: i32,
}

impl Course {
    // return distance type from distance
    // fn distance_type(&self) -> DistanceType {
    //     match self.distance {
    //         d if d < 1600 => DistanceType::Short,
    //         d if d < 2000 => DistanceType::Mile,
    //         d if d < 2400 => DistanceType::Medium,
    //         _ => DistanceType::Long,
    //     }
    // }
    pub fn section_length(&self) -> f64 {
        self.distance as f64 / 24.0
    }
    // in m/s
    pub fn base_speed(&self) -> f64 {
        20.0 - (self.distance as f64 - 2000.0) / 1000.0
    }
    pub fn track_name(&self, language: Language) -> Option<&'static str> {
        get_track_name(self.id, language)
    }
}
