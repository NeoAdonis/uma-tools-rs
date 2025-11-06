include!("track_names.rs");

pub mod geometry;

#[cfg(test)]
mod tests;

use crate::course::geometry::*;
use crate::units::course_widths_to_meters;
use serde::Deserialize;
use serde_repr::{Deserialize_repr, Serialize_repr};

// Track IDs for special handling
const TRACK_ID_OOI: u16 = 10101;
const TRACK_ID_LONGCHAMP: u16 = 10201;

/// Represents the phase of a race based on the position on the course.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub enum Phase {
    /// Early race (sections 0-3).
    Early,
    /// Mid race (sections 4-15).
    Mid,
    /// Late race (sections 16-19).
    Late,
    /// Last spurt (not to be confused with the last spurt mechanic) (sections 20-23).
    Last,
}

impl Phase {
    /// Get the boundaries (start, end) in meters for this phase on a given course.
    ///
    /// Phases are divided into 24 sections:
    ///
    /// - Early: sections 0-3 (0.0 to section_length * 4.0)
    /// - Mid: sections 4-15 (section_length * 4.0 to section_length * 16.0)
    /// - Late: sections 16-19 (section_length * 16.0 to section_length * 20.0)
    /// - Last: sections 20-23 (section_length * 20.0 to course distance)
    pub fn boundaries(&self, course: &Course) -> (f64, f64) {
        let section = course.section_length();
        match self {
            Phase::Early => (0.0, section * 4.0),
            Phase::Mid => (section * 4.0, section * 16.0),
            Phase::Late => (section * 16.0, section * 20.0),
            Phase::Last => (section * 20.0, course.distance as f64),
        }
    }

    /// Determine which phase a given position falls into
    pub fn from_position(position: f64, course: &Course) -> Phase {
        let section_length = course.section_length();
        if position < section_length * 4.0 {
            Phase::Early
        } else if position < section_length * 16.0 {
            Phase::Mid
        } else if position < section_length * 20.0 {
            Phase::Late
        } else {
            Phase::Last
        }
    }
}

/// Track surface type.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize_repr, Deserialize_repr)]
#[repr(u8)]
pub enum Track {
    /// 芝
    Turf = 1,
    /// ダート
    Dirt = 2,
}

/// Course turn direction.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize_repr, Deserialize_repr)]
#[repr(u8)]
pub enum Direction {
    /// Clockwise (右回り).
    Right = 1,
    /// Counterclockwise (左回り).
    Left = 2,
    /// No turns.
    Straight = 4,
}

/// Course distance type.
/// N.B. Even though distance type can be inferred from distance, it is explicitly
/// stored in the data.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize_repr, Deserialize_repr)]
#[repr(u8)]
pub enum DistanceType {
    /// Less than 1600m.
    Short = 1,
    /// 1600m to 2000m.
    Mile = 2,
    /// 2000m to 2400m.
    Medium = 3,
    /// 2400m or longer.
    Long = 4,
}

/// Stats used for course set status checks.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize_repr, Deserialize_repr)]
#[repr(u8)]
pub enum ThresholdStat {
    Speed = 1,
    Stamina = 2,
    Power = 3,
    Guts = 4,
    Wiz = 5,
}

/// Lane type an Uma is running in. Used mainly by skill conditions.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum LaneType {
    Inner,
    Middle,
    Outer,
    Outside,
}

/// Structure representing a race course.
#[derive(Clone, Debug)]
pub struct Course {
    pub id: u16,
    data: CourseData,
}

impl std::ops::Deref for Course {
    type Target = CourseData;

    fn deref(&self) -> &Self::Target {
        &self.data
    }
}

/// Course data structure as deserialized from JSON.
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CourseData {
    pub distance: i32,
    pub distance_type: DistanceType,
    pub surface: Track,
    pub turn: Direction,
    pub course_set_status: Vec<ThresholdStat>,
    pub corners: Vec<Corner>,
    pub slopes: Vec<Slope>,
    pub straights: Vec<Straight>,
    pub race_track_id: u16,
    pub(crate) finish_time_min: u32,
    pub(crate) finish_time_max: u32,
    pub(crate) lane_max: u32,
}

impl Course {
    /// Create a new Course for testing purposes.
    #[cfg(test)]
    pub fn new_test(id: u16, data: CourseData) -> Self {
        Course { id, data }
    }

    /// Get the length of a single section in meters.
    /// There are 24 sections in a course.
    pub fn section_length(&self) -> f64 {
        self.distance as f64 / 24.0
    }

    /// Get quarterway point in meters.
    pub fn quarterway(&self) -> f64 {
        self.distance as f64 / 4.0
    }

    /// Get halfway point in meters.
    pub fn halfway(&self) -> f64 {
        self.distance as f64 / 2.0
    }

    /// Check if the course distance is a basis distance (multiple of 400m).
    pub fn is_basis_distance(&self) -> bool {
        self.distance % 400 == 0
    }

    /// Get the base speed for this course in m/s.
    /// In essence, the longer the distance, the lower the speed an Uma should run at.
    pub fn base_speed(&self) -> f64 {
        20.0 - (self.distance as f64 - 2000.0) / 1000.0
    }

    /// Get the maximum finish time that can be displayed on the scoreboard in seconds.
    pub fn finish_time_max(&self) -> f64 {
        self.finish_time_max as f64 / 10000.0
    }

    /// Get the minimum finish time that can be displayed on the scoreboard in seconds.
    pub fn finish_time_min(&self) -> f64 {
        self.finish_time_min as f64 / 10000.0
    }

    /// Get the section index (0-23) from a given position in meters.
    pub fn section_from_position(&self, position: f64) -> u8 {
        (position / self.section_length()).floor() as u8
    }

    pub fn starting_lane(&self, gate_roll: u8) -> f64 {
        let adjustor = match self.id {
            TRACK_ID_OOI => 0.0,
            TRACK_ID_LONGCHAMP => {
                if gate_roll >= 13 {
                    1.86
                } else {
                    0.0
                }
            }
            _ => {
                if gate_roll >= 10 {
                    0.6
                } else {
                    0.0
                }
            }
        };
        gate_roll as f64 * self.lane_width() + adjustor
    }

    /// Get the total course width in meters.
    pub fn width(&self) -> f64 {
        course_widths_to_meters(self.lane_max as f64 / 10000.0)
    }

    /// Get the width of a single lane in meters.
    pub fn lane_width(&self) -> f64 {
        self.width() / 18.0
    }

    /// Get the track name in the specified language.
    ///
    /// # Arguments
    ///
    /// - `language` - Desired language.
    ///
    /// # Returns
    ///
    /// - `Some(&'static str)` - The track name if available in the specified language.
    /// - `None` - If the track name is not available in the specified language.
    pub fn track_name(&self, language: Language) -> Option<&'static str> {
        get_track_name(self.race_track_id, language)
    }

    /// Check if this is an exchange race (dirt grade).
    /// Exchange races take place at Ooi (10101), Kawasaki (10103), Funabashi (10104),
    /// or Morioka (10105).
    pub fn is_exchange_race(&self) -> bool {
        matches!(self.race_track_id, 10101 | 10103 | 10104 | 10105)
    }

    /// Load a specific course by ID from a JSON file.
    ///
    /// # Arguments
    ///
    /// - `json_path` - Path to the JSON file containing course data.
    /// - `course_id` - The course ID to look up (e.g., "10101").
    ///
    /// # Returns
    ///
    /// - `Ok(Course)` - The course data with the ID field populated.
    /// - `Err` - If the file can't be read, JSON is invalid, or course ID not found.
    pub fn from_json_file(
        json_path: &str,
        course_id: &str,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        use std::fs;
        let json_content = fs::read_to_string(json_path)?;
        Self::from_json_str(&json_content, course_id)
    }

    /// Load a specific course by ID from a JSON string.
    ///
    /// # Arguments
    ///
    /// - `json_str` - JSON string containing course data.
    /// - `course_id` - The course ID to look up (e.g., "10101").
    ///
    /// # Returns
    ///
    /// - `Ok(Course)` - The course data with the ID field populated.
    /// - `Err` - If the JSON is invalid or course ID not found.
    pub fn from_json_str(
        json_str: &str,
        course_id: &str,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        use std::collections::HashMap;

        let courses: HashMap<String, CourseData> = serde_json::from_str(json_str)?;

        let data = courses
            .get(course_id)
            .ok_or_else(|| format!("Course ID '{}' not found in JSON", course_id))?
            .clone();

        let id = course_id
            .parse::<u16>()
            .map_err(|_| format!("Invalid course ID format: '{}'", course_id))?;

        Ok(Course { id, data })
    }
}
