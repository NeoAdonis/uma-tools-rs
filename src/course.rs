include!("track_names.rs");

use serde::Deserialize;
use serde_repr::{Deserialize_repr, Serialize_repr};

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub enum Phase {
    Early,
    Mid,
    Late,
    Last, // Last spurt
}

impl Phase {
    /// Get the boundaries (start, end) in meters for this phase on a given course
    ///
    /// Phases are divided into 24 sections:
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

#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize_repr, Deserialize_repr)]
#[repr(u8)]
pub enum Track {
    Turf = 1, // 芝
    Dirt = 2, // ダート
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize_repr, Deserialize_repr)]
#[repr(u8)]
pub enum Direction {
    Left = 1,  // Clockwise
    Right = 2, // Counterclockwise
    Unused = 3,
    Straight = 4, // No curves
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize_repr, Deserialize_repr)]
#[repr(u8)]
pub enum DistanceType {
    Short = 1,  // less than 1600m
    Mile = 2,   // less than 2000m
    Medium = 3, // less than 2400m
    Long = 4,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize_repr, Deserialize_repr)]
#[repr(u8)]
pub enum ThresholdStat {
    Speed = 1,
    Stamina = 2,
    Power = 3,
    Guts = 4,
    Wiz = 5,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Corner {
    pub start: f64,  // in meters
    pub length: f64, // in meters
}

#[derive(Clone, Debug, Deserialize)]
pub struct Slope {
    pub start: f64,  // in meters
    pub length: f64, // in meters
    slope: i32,
}

impl Slope {
    /// Create a new Slope for testing purposes
    #[cfg(test)]
    pub fn new(start: f64, length: f64, slope: i32) -> Self {
        Slope { start, length, slope }
    }

    pub fn is_uphill(&self) -> bool {
        self.slope > 0
    }

    pub fn is_downhill(&self) -> bool {
        self.slope < 0
    }

    pub fn slope_percent(&self) -> f64 {
        self.slope as f64 / 10000.0
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize_repr, Deserialize_repr)]
#[repr(u8)]
pub enum FrontType {
    Audience = 1,
    Opposite = 2,
    Unknown = 3,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Straight {
    pub start: f64, // in meters
    pub end: f64,   // in meters
    pub front_type: FrontType,
}

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

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CourseData {
    pub distance: i32,
    pub distance_type: DistanceType,
    pub surface: Track,
    pub turn: Direction,
    pub course_set_status: Vec<ThresholdStat>,
    pub finish_time_min: u32,
    pub finish_time_max: u32,
    pub lane_max: u32,
    pub corners: Vec<Corner>,
    pub slopes: Vec<Slope>,
    pub straights: Vec<Straight>,
    pub race_track_id: u16,
    #[serde(skip)]
    #[cfg_attr(test, allow(dead_code))]
    pub(crate) _course: (), // unused in current implementation
}

impl Course {
    /// Create a new Course for testing purposes
    #[cfg(test)]
    pub fn new_test(id: u16, data: CourseData) -> Self {
        Course { id, data }
    }

    // return distance type from distance
    // fn distance_type(&self) -> DistanceType {
    //     match self.distance {
    //         d if d < 1600 => DistanceType::Short,
    //         d if d < 2000 => DistanceType::Mile,
    //         d if d < 2400 => DistanceType::Medium,
    //         _ => DistanceType::Long,
    //     }
    // }

    // in meters
    pub fn section_length(&self) -> f64 {
        self.distance as f64 / 24.0
    }

    // in m/s
    pub fn base_speed(&self) -> f64 {
        20.0 - (self.distance as f64 - 2000.0) / 1000.0
    }

    /// Get the track name in the specified language
    ///
    /// # Arguments
    /// * `language` - Desired language
    ///
    /// # Returns
    /// * `Some(&'static str)` - The track name if available in the specified language
    /// * `None` - If the track name is not available in the specified language
    pub fn track_name(&self, language: Language) -> Option<&'static str> {
        get_track_name(self.race_track_id, language)
    }

    /// Load a specific course by ID from a JSON file
    ///
    /// # Arguments
    /// * `json_path` - Path to the JSON file containing course data
    /// * `course_id` - The course ID to look up (e.g., "10101")
    ///
    /// # Returns
    /// * `Ok(Course)` - The course data with the ID field populated
    /// * `Err` - If the file can't be read, JSON is invalid, or course ID not found
    pub fn from_json_file(
        json_path: &str,
        course_id: &str,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        use std::fs;
        let json_content = fs::read_to_string(json_path)?;
        Self::from_json_str(&json_content, course_id)
    }

    /// Load a specific course by ID from a JSON string
    ///
    /// # Arguments
    /// * `json_str` - JSON string containing course data
    /// * `course_id` - The course ID to look up (e.g., "10101")
    ///
    /// # Returns
    /// * `Ok(Course)` - The course data with the ID field populated
    /// * `Err` - If the JSON is invalid or course ID not found
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
