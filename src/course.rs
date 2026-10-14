//! Course-related structures and functions.
//!
//! This module includes data structures for representing race courses and
//! their attributes. It also provides functions for calculating various
//! properties of the course, such as section lengths, phase boundaries
//! and lane widths. Additionally, it handles special cases for certain tracks.

include!("course/track_names.rs");

pub mod geometry;

use crate::course::geometry::*;
use crate::units::{deserialize_ten_thousandths, horse_lanes_to_course_widths};
use num_enum::TryFromPrimitive;
use serde::Deserialize;
use serde_repr::{Deserialize_repr, Serialize_repr};

// Track IDs for special handling.
const TRACK_ID_OOI: u16 = 10101;
const TRACK_ID_LONGCHAMP: u16 = 10201;

/// Represents the phase of a race based on the position on the course.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug, TryFromPrimitive)]
#[repr(u8)]
pub enum Phase {
    /// Early race (sections 0-3).
    Early = 0,
    /// Mid race (sections 4-15).
    Mid = 1,
    /// Late race (sections 16-19).
    Late = 2,
    /// Last leg (sections 20-23). Not to be confused with the last spurt mechanic.
    Last = 3,
}

impl Phase {
    /// Get the boundaries `(start, end)` in meters for `self` on a given course.
    ///
    /// Phases are defined by the following section ranges:
    ///
    /// - Early: sections 0-3 (0.0 to `section_length * 4.0`).
    /// - Mid: sections 4-15 (`section_length * 4.0` to `section_length * 16.0`).
    /// - Late: sections 16-19 (`section_length * 16.0` to `section_length * 20.0`).
    /// - Last: sections 20-23 (`section_length * 20.0` to course distance).
    pub fn boundaries(&self, course: &Course) -> (f64, f64) {
        let section = course.section_length();
        match self {
            Phase::Early => (0.0, section * 4.0),
            Phase::Mid => (section * 4.0, section * 16.0),
            Phase::Late => (section * 16.0, section * 20.0),
            Phase::Last => (section * 20.0, course.distance as f64),
        }
    }

    /// Get the boundaries `(start, end)` in meters for `self`'s first quarter on a
    /// given course.
    pub fn boundaries_first_quarter(&self, course: &Course) -> (f64, f64) {
        let section = course.section_length();
        match self {
            Phase::Early => (0.0, section * 1.0),
            Phase::Mid => (section * 4.0, section * 7.0),
            Phase::Late => (section * 16.0, section * 17.0),
            Phase::Last => (section * 20.0, section * 21.0),
        }
    }

    /// Get the boundaries `(start, end)` in meters for `self`'s first half
    /// on a given course.
    pub fn boundaries_first_half(&self, course: &Course) -> (f64, f64) {
        let section = course.section_length();
        match self {
            Phase::Early => (0.0, section * 2.0),
            Phase::Mid => (section * 4.0, section * 10.0),
            Phase::Late => (section * 16.0, section * 18.0),
            Phase::Last => (section * 20.0, section * 22.0),
        }
    }

    /// Get the boundaries `(start, end)` in meters for `self`'s second half
    /// on a given course.
    pub fn boundaries_second_half(&self, course: &Course) -> (f64, f64) {
        let section = course.section_length();
        match self {
            Phase::Early => (section * 2.0, section * 4.0),
            Phase::Mid => (section * 10.0, section * 16.0),
            Phase::Late => (section * 18.0, section * 20.0),
            Phase::Last => (section * 22.0, course.distance as f64),
        }
    }

    /// Determine which phase a given position falls into.
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
///
/// N.B. Even though distance type can be inferred from distance, it is
/// explicitly stored in the data.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize_repr, Deserialize_repr)]
#[repr(u8)]
pub enum DistanceType {
    /// Also known as Sprint. Up to 1400m.
    Short = 1,
    /// 1401m up to 1800m.
    Mile = 2,
    /// 1801m up to 2400m.
    Medium = 3,
    /// 2401m or longer.
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
    Wit = 5,
}

/// Lane type an Uma is running in.
///
/// Used mainly by skill conditions.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum LaneType {
    Inner,
    Middle,
    Outer,
    Outside,
}

impl LaneType {
    /// Lane type of a runner at `lane`, in course widths from the inner fence.
    pub fn from_lane(lane: f64) -> Self {
        if lane <= 0.2 {
            LaneType::Inner
        } else if lane <= 0.4 {
            LaneType::Middle
        } else if lane <= 0.6 {
            LaneType::Outer
        } else {
            LaneType::Outside
        }
    }
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
    /// Course distance in meters.
    pub distance: i32,
    /// Course distance type.
    pub distance_type: DistanceType,
    /// Course track surface type.
    pub surface: Track,
    /// Course turn direction.
    pub turn: Direction,
    /// Stats used for course set status checks.
    ///
    /// If an Uma's base stat exceeds a threshold, they receive a speed bonus.
    pub course_set_status: Vec<ThresholdStat>,
    /// Course corners.
    pub corners: Vec<Corner>,
    /// Course slopes.
    pub slopes: Vec<Slope>,
    /// Course straights.
    pub straights: Vec<Straight>,
    /// Race track ID associated with this course.
    pub race_track_id: u16,
    /// Minimum finish time that can be displayed on the scoreboard in seconds.
    ///
    /// Stored in 10000ths of a second in the course data, and converted when
    /// deserializing.
    #[serde(deserialize_with = "deserialize_ten_thousandths")]
    pub finish_time_min: f64,
    /// Maximum finish time that can be displayed on the scoreboard in seconds.
    ///
    /// Stored in 10000ths of a second in the course data, and converted when
    /// deserializing.
    #[serde(deserialize_with = "deserialize_ten_thousandths")]
    pub finish_time_max: f64,
    /// Total course width in course widths (the maximum lane distance).
    ///
    /// Stored in 10000ths of a course width in the course data (as `laneMax`), and
    /// converted when deserializing.
    #[serde(rename = "laneMax", deserialize_with = "deserialize_ten_thousandths")]
    pub width: f64,
}

impl Course {
    /// Create a new Course for testing purposes.
    #[cfg(test)]
    pub fn new_test(id: u16, data: CourseData) -> Self {
        Course { id, data }
    }

    /// Get the length of a single section in meters.
    ///
    /// There are 24 equally-sized sections in a course.
    pub fn section_length(&self) -> f64 {
        self.distance as f64 / 24.0
    }

    /// Get halfway point in meters.
    pub fn halfway(&self) -> f64 {
        self.distance as f64 / 2.0
    }

    /// Get the move lane point in meters.
    ///
    /// STUB: Uses the first corner start (or halfway if there are no corners)
    /// until the actual move lane point is available.
    pub fn move_lane_point(&self) -> f64 {
        self.corners.first().map_or(self.halfway(), |c| c.start)
    }

    /// Get the corner number (1-4) of the corner at `idx` in the course corners.
    ///
    /// Corners are numbered backwards from the final corner, which is always the
    /// fourth. If the race goes through the same corner multiple times (i.e. more
    /// than one lap), its number is repeated.
    pub fn corner_number(&self, idx: usize) -> u8 {
        4 - ((self.corners.len() - 1 - idx) % 4) as u8
    }

    /// Check if the course distance is a basis distance (multiple of 400m).
    pub fn is_basis_distance(&self) -> bool {
        self.distance % 400 == 0
    }

    /// Get the base speed for this course in m/s.
    ///
    /// The longer the distance, the lower the speed an Uma should run at.
    pub fn base_speed(&self) -> f64 {
        20.0 - (self.distance as f64 - 2000.0) / 1000.0
    }

    /// Get remaining distance in meters from a given position.
    pub fn remain_distance(&self, position: f64) -> f64 {
        (self.distance as f64 - position).max(0.0)
    }

    /// Get the distance rate (percentage of course completed) from a given
    /// position.
    pub fn distance_rate(&self, position: f64) -> f64 {
        (position / self.distance as f64 * 100.0).floor()
    }

    /// Get the section index (0-23) from a given position in meters.
    pub fn section_from_position(&self, position: f64) -> u8 {
        (position / self.section_length()).floor() as u8
    }

    /// Get the starting lane position in course widths based on the gate roll.
    pub fn starting_lane(&self, gate_roll: u8) -> f64 {
        let adjustor = match self.race_track_id {
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
        horse_lanes_to_course_widths(gate_roll as f64 + adjustor)
    }

    /// Get the track name in the specified language, or `None` if the track name is
    /// not available in that language.
    pub fn track_name(&self, language: Language) -> Option<&'static str> {
        get_track_name(self.race_track_id, language)
    }

    /// Check if this is an exchange race (dirt grade).
    ///
    /// Exchange races take place at Ooi (10101), Kawasaki (10103), Funabashi
    /// (10104), or Morioka (10105).
    pub fn is_exchange_race(&self) -> bool {
        matches!(self.race_track_id, 10101 | 10103 | 10104 | 10105)
    }

    /// Check if the race is held at a racetrack outside Japan.
    ///
    /// Abroad tracks include Longchamp, Santa Anita Park and Del Mar.
    pub fn is_abroad(&self) -> bool {
        matches!(self.race_track_id, 10201..=10203)
    }

    /// Check if the race is held at a racetrack with tight turns.
    ///
    /// Tight tracks include Sapporo, Hakodate, Fukushima, Kokura, Kawasaki or
    /// Funabashi.
    pub fn is_tight_track(&self) -> bool {
        matches!(
            self.race_track_id,
            10001 | 10002 | 10004 | 10010 | 10103 | 10104
        )
    }

    /// Load a specific course by ID from a JSON file.
    ///
    /// Returns a `Result` containing the parsed `Course` or an error if the file
    /// cannot be read, the JSON is invalid or the course ID is not found.
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
    /// Returns a `Result` containing the parsed `Course` or an error if the JSON is
    /// invalid or the course ID is not found.
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::course_data;

    /// Create a minimal course at the given racetrack.
    ///
    /// The course ID is unrelated to the racetrack ID, so that checks on the
    /// wrong one fail.
    fn create_test_course(track_id: u16) -> Course {
        let data = CourseData {
            race_track_id: track_id,
            ..course_data()
        };
        Course::new_test(0, data)
    }

    /// Racetrack IDs of every course in the course data.
    const TRACK_IDS: [u16; 17] = [
        10001, 10002, 10003, 10004, 10005, 10006, 10007, 10008, 10009, 10010, 10101, 10103, 10104,
        10105, 10201, 10202, 10203,
    ];

    #[test]
    fn test_values_are_converted_from_ten_thousandths() {
        let course = crate::test_utils::load_course(10201);
        assert_eq!(course.finish_time_min, 54.7);
        assert_eq!(course.finish_time_max, 57.0);
        assert_eq!(course.width, 1.47);
        assert_eq!(course.slopes[0].slope, 1.0);
    }

    #[test]
    fn test_is_exchange_race() {
        // Ooi, Kawasaki, Funabashi, Morioka.
        let exchange = [10101, 10103, 10104, 10105];
        for track_id in TRACK_IDS {
            assert_eq!(
                create_test_course(track_id).is_exchange_race(),
                exchange.contains(&track_id),
                "track {track_id}"
            );
        }
    }

    #[test]
    fn test_is_abroad() {
        // Longchamp, Santa Anita Park, Del Mar.
        let abroad = [10201, 10202, 10203];
        for track_id in TRACK_IDS {
            assert_eq!(
                create_test_course(track_id).is_abroad(),
                abroad.contains(&track_id),
                "track {track_id}"
            );
        }
    }

    #[test]
    fn test_is_tight_track() {
        // Sapporo, Hakodate, Fukushima, Kokura, Kawasaki, Funabashi.
        let tight = [10001, 10002, 10004, 10010, 10103, 10104];
        for track_id in TRACK_IDS {
            assert_eq!(
                create_test_course(track_id).is_tight_track(),
                tight.contains(&track_id),
                "track {track_id}"
            );
        }
    }

    #[test]
    fn test_starting_lane_adjustment_by_racetrack() {
        let lane = horse_lanes_to_course_widths;
        // (racetrack, gate roll, expected lane in horse lanes)
        let cases = [
            // Ooi: never adjusted.
            (TRACK_ID_OOI, 9, 9.0),
            (TRACK_ID_OOI, 12, 12.0),
            // Longchamp: adjusted from gate roll 13.
            (TRACK_ID_LONGCHAMP, 12, 12.0),
            (TRACK_ID_LONGCHAMP, 13, 13.0 + 1.86),
            // Anywhere else: adjusted from gate roll 10.
            (10001, 9, 9.0),
            (10001, 10, 10.0 + 0.6),
        ];
        for (track_id, gate_roll, expected) in cases {
            assert_eq!(
                create_test_course(track_id).starting_lane(gate_roll),
                lane(expected),
                "track {track_id}, gate roll {gate_roll}"
            );
        }

        // Course 10101 is at Sapporo, not at Ooi (racetrack 10101).
        let sapporo = CourseData {
            race_track_id: 10001,
            ..course_data()
        };
        let course = Course::new_test(10101, sapporo);
        assert_eq!(course.starting_lane(12), lane(12.0 + 0.6));
    }

    #[test]
    fn test_corner_number() {
        let course_with_corners = |count: usize| {
            let mut course = create_test_course(10001);
            course.data.corners = (0..count)
                .map(|i| Corner {
                    start: i as f64 * 200.0,
                    length: 100.0,
                })
                .collect();
            course
        };

        // Numbered backwards from the final corner, which is always the fourth.
        for (count, expected) in [
            (2, vec![3, 4]),
            (3, vec![2, 3, 4]),
            (4, vec![1, 2, 3, 4]),
            (5, vec![4, 1, 2, 3, 4]),
            (6, vec![3, 4, 1, 2, 3, 4]),
            (8, vec![1, 2, 3, 4, 1, 2, 3, 4]),
        ] {
            let course = course_with_corners(count);
            let numbers: Vec<u8> = (0..count).map(|idx| course.corner_number(idx)).collect();
            assert_eq!(numbers, expected, "{count} corners");
        }
    }

    #[test]
    fn test_from_json_str_errors() {
        // Course data of 10101 under its own ID and under a non-numeric one.
        let json = std::fs::read_to_string("data/course_data.json").unwrap();
        let courses: serde_json::Value = serde_json::from_str(&json).unwrap();
        let data = &courses["10101"];
        let json = serde_json::json!({ "10101": data, "abc": data }).to_string();

        assert_eq!(Course::from_json_str(&json, "10101").unwrap().id, 10101);
        let error = Course::from_json_str(&json, "10102").unwrap_err();
        assert_eq!(error.to_string(), "Course ID '10102' not found in JSON");
        let error = Course::from_json_str(&json, "abc").unwrap_err();
        assert_eq!(error.to_string(), "Invalid course ID format: 'abc'");
    }

    #[test]
    fn test_every_course_has_a_track_name() {
        let json = std::fs::read_to_string("data/course_data.json").unwrap();
        let courses: std::collections::HashMap<String, CourseData> =
            serde_json::from_str(&json).unwrap();
        for (id, data) in courses {
            let course = Course::new_test(id.parse().unwrap(), data);
            for language in [Language::Japanese, Language::English] {
                let name = course.track_name(language);
                assert!(name.is_some(), "course {id} ({language:?})");
            }
        }
        assert_eq!(create_test_course(0).track_name(Language::English), None);
    }

    #[test]
    fn test_lane_type_from_lane() {
        // Thresholds are in course widths, regardless of the course's total width.
        let cases = [
            (0.0, LaneType::Inner),
            (0.2, LaneType::Inner),
            (0.21, LaneType::Middle),
            (0.4, LaneType::Middle),
            (0.41, LaneType::Outer),
            (0.6, LaneType::Outer),
            (0.61, LaneType::Outside),
        ];
        for (lane, expected) in cases {
            assert_eq!(LaneType::from_lane(lane), expected, "lane {lane}");
        }
    }
}
