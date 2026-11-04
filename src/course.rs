//! Course-related structures and functions.
//!
//! This module includes data structures for representing race courses and
//! their attributes. It also provides functions for calculating various
//! properties of the course, such as section lengths, phase boundaries
//! and lane widths. Additionally, it handles special cases for certain tracks.
//!
//! # Examples
//!
//! Look up a course's layout, e.g. to see how long its final straight is
//! before picking skills for it:
//!
//! ```
//! use uma_tools::Course;
//! use uma_tools::configuration::Language;
//! use uma_tools::course::DistanceType;
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
//! // Tokyo, turf 2400m.
//! let course = Course::from_json_files(
//!     "data/course_data.json",
//!     "data/rails",
//!     "10606",
//! )?;
//! assert_eq!(course.track_name(Language::English), Some("Tokyo"));
//! assert_eq!(course.distance_type(), DistanceType::Medium);
//!
//! let final_straight = course.straights.last().ok_or("no straights")?;
//! assert_eq!(course.length() - final_straight.start, 525.0);
//! # Ok(())
//! # }
//! ```

pub mod geometry;
pub mod rail;

use crate::configuration::Language;
use crate::course::geometry::*;
use crate::course::rail::Rail;
use crate::units::{CourseWidths, deserialize_ten_thousandths};
use num_enum::TryFromPrimitive;
use serde::{Deserialize, Deserializer};
use serde_repr::{Deserialize_repr, Serialize_repr};
use std::path::Path;

// Track IDs for special handling.
const TRACK_ID_OOI: u16 = 10101;
const TRACK_ID_KAWASAKI: u16 = 10103;
const TRACK_ID_FUNABASHI: u16 = 10104;
const TRACK_ID_LONGCHAMP: u16 = 10201;

/// Represents the phase of a race based on the position on the course.
///
/// Every phase but [`Phase::RunUp`] spans a fixed number of the 24 sections of
/// the nominal distance, after the run-up.
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Debug, TryFromPrimitive)]
#[repr(i8)]
pub enum Phase {
    /// Run-up, from the starting gate to the official start line.
    ///
    /// Only on courses with a run-up. Its value in conditions is -1, so
    /// `phase==N` and `phase>=N` never hold, while `phase<=N` always does.
    /// Mechanics otherwise treat it as [`Phase::Early`].
    RunUp = -1,
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
    /// Phases are defined by the following section ranges, offset by the
    /// run-up:
    ///
    /// - Run-up: from the gate to the end of the run-up.
    /// - Early: sections 0-3.
    /// - Mid: sections 4-15.
    /// - Late: sections 16-19.
    /// - Last: sections 20-23, up to the goal.
    ///
    /// # Examples
    ///
    /// Find the stretch of the course where a skill with `phase==3` can
    /// activate, and where one with `phase_firsthalf==3` can:
    ///
    /// ```
    /// use uma_tools::course::{Course, Phase};
    ///
    /// # fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    /// // Tokyo, turf 2400m.
    /// let course = Course::from_json_files(
    ///     "data/course_data.json",
    ///     "data/rails",
    ///     "10606",
    /// )?;
    /// assert_eq!(Phase::Last.boundaries(&course), (2000.0, 2400.0));
    /// assert_eq!(Phase::Last.boundaries_first_half(&course), (2000.0, 2200.0));
    /// # Ok(())
    /// # }
    /// ```
    pub fn boundaries(&self, course: &Course) -> (f64, f64) {
        match self {
            Phase::RunUp => (0.0, course.run_up),
            Phase::Early => (course.section_start(0), course.section_start(4)),
            Phase::Mid => (course.section_start(4), course.section_start(16)),
            Phase::Late => (course.section_start(16), course.section_start(20)),
            Phase::Last => (course.section_start(20), course.length()),
        }
    }

    /// Get the boundaries `(start, end)` in meters for `self`'s first quarter on a
    /// given course.
    ///
    /// See [`Phase::boundaries`] for an example.
    pub fn boundaries_first_quarter(&self, course: &Course) -> (f64, f64) {
        match self {
            Phase::RunUp => (0.0, course.run_up),
            Phase::Early => (course.section_start(0), course.section_start(1)),
            Phase::Mid => (course.section_start(4), course.section_start(7)),
            Phase::Late => (course.section_start(16), course.section_start(17)),
            Phase::Last => (course.section_start(20), course.section_start(21)),
        }
    }

    /// Get the boundaries `(start, end)` in meters for `self`'s first half
    /// on a given course.
    ///
    /// See [`Phase::boundaries`] for an example.
    pub fn boundaries_first_half(&self, course: &Course) -> (f64, f64) {
        match self {
            Phase::RunUp => (0.0, course.run_up),
            Phase::Early => (course.section_start(0), course.section_start(2)),
            Phase::Mid => (course.section_start(4), course.section_start(10)),
            Phase::Late => (course.section_start(16), course.section_start(18)),
            Phase::Last => (course.section_start(20), course.section_start(22)),
        }
    }

    /// Get the boundaries `(start, end)` in meters for `self`'s second half
    /// on a given course.
    ///
    /// See [`Phase::boundaries`] for an example.
    pub fn boundaries_second_half(&self, course: &Course) -> (f64, f64) {
        match self {
            Phase::RunUp => (0.0, course.run_up),
            Phase::Early => (course.section_start(2), course.section_start(4)),
            Phase::Mid => (course.section_start(10), course.section_start(16)),
            Phase::Late => (course.section_start(18), course.section_start(20)),
            Phase::Last => (course.section_start(22), course.length()),
        }
    }

    /// Determine which phase a given position falls into.
    ///
    /// # Examples
    ///
    /// Positions are measured from the gate, so on a course with a run-up the
    /// Early race starts after it, and `phase==0` doesn't hold before:
    ///
    /// ```
    /// use uma_tools::course::{Course, Phase};
    ///
    /// # fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    /// // Santa Anita Park, dirt 1600m, with a 50m run-up.
    /// let course = Course::from_json_files(
    ///     "data/course_data.json",
    ///     "data/rails",
    ///     "11619",
    /// )?;
    /// assert_eq!(Phase::from_position(30.0, &course), Phase::RunUp);
    /// assert_eq!(Phase::from_position(50.0, &course), Phase::Early);
    /// # Ok(())
    /// # }
    /// ```
    pub fn from_position(position: f64, course: &Course) -> Phase {
        if position < course.run_up {
            Phase::RunUp
        } else if position < course.section_start(4) {
            Phase::Early
        } else if position < course.section_start(16) {
            Phase::Mid
        } else if position < course.section_start(20) {
            Phase::Late
        } else {
            Phase::Last
        }
    }
}

/// Track surface type.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Serialize_repr, Deserialize_repr)]
#[repr(u8)]
pub enum Track {
    /// 芝
    Turf = 1,
    /// ダート
    Dirt = 2,
}

/// Course turn direction.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Serialize_repr, Deserialize_repr)]
#[repr(u8)]
pub enum Direction {
    /// Clockwise (右回り).
    Right = 1,
    /// Counterclockwise (左回り).
    Left = 2,
    /// No turns.
    Straight = 4,
}

/// Course distance type, inferred from the course distance.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Serialize_repr, Deserialize_repr)]
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
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Serialize_repr, Deserialize_repr)]
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
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum LaneType {
    Inner,
    Middle,
    Outer,
    Outside,
}

impl From<CourseWidths> for LaneType {
    /// Lane type of a runner at `lane`, in course widths from the inner fence.
    fn from(lane: CourseWidths) -> Self {
        if lane <= CourseWidths(0.2) {
            LaneType::Inner
        } else if lane <= CourseWidths(0.4) {
            LaneType::Middle
        } else if lane <= CourseWidths(0.6) {
            LaneType::Outer
        } else {
            LaneType::Outside
        }
    }
}

/// Structure representing a race course.
#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Course {
    /// Course ID.
    ///
    /// Not part of each course's data, but the key of the course in the course
    /// data. Set when loading the course.
    #[serde(skip)]
    pub id: u16,
    /// Nominal course distance in meters, measured from the official start line
    /// (i.e. after the run-up).
    ///
    /// Use [`Course::length`] for the distance runners actually cover.
    pub distance: i32,
    /// Run-up in meters, from the starting gate to the official start
    /// line, for the few courses that have one.
    ///
    /// Positions on the course, including those of its corners, slopes and
    /// straights, are measured from the gate, so the nominal distance starts at
    /// `run_up` and the goal is at [`Course::length`].
    #[serde(default)]
    pub run_up: f64,
    /// Course track surface type.
    pub surface: Track,
    /// Course turn direction.
    pub direction: Direction,
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
    /// Move lane point in meters, for the few courses that set it explicitly.
    ///
    /// Use [`Course::move_lane_point`] instead, which falls back to the first
    /// corner start.
    #[serde(default)]
    pub move_lane_point: Option<f64>,
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
    #[serde(rename = "laneMax", deserialize_with = "deserialize_width")]
    pub width: CourseWidths,
    /// Shape of the inner rail in the world.
    ///
    /// Not part of each course's data, but read from the rail data when
    /// loading the course.
    #[serde(skip)]
    pub rail: Rail,
}

/// Deserialize the course width from 10000ths of a course width.
fn deserialize_width<'de, D: Deserializer<'de>>(deserializer: D) -> Result<CourseWidths, D::Error> {
    deserialize_ten_thousandths(deserializer).map(CourseWidths)
}

impl Course {
    /// Get the distance in meters from the starting gate to the goal, i.e. the
    /// nominal distance plus the run-up.
    pub fn length(&self) -> f64 {
        self.distance as f64 + self.run_up
    }

    /// Get the distance in meters covered from the official start line at
    /// `position` (0 during the run-up).
    ///
    /// See [`Course::position_at_rate`] for an example.
    pub fn nominal_position(&self, position: f64) -> f64 {
        (position - self.run_up).max(0.0)
    }

    /// Get the position in meters where `rate` percent of the nominal distance
    /// has been covered.
    ///
    /// Inverse of [`Course::distance_rate`], but not rounded down.
    ///
    /// # Examples
    ///
    /// Find where a skill with `distance_rate>=50` can start to activate.
    /// Positions are measured from the gate, so on a course with a run-up they
    /// are ahead of the nominal distance covered:
    ///
    /// ```
    /// use uma_tools::Course;
    ///
    /// # fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    /// // Santa Anita Park, dirt 1600m, with a 50m run-up.
    /// let course = Course::from_json_files(
    ///     "data/course_data.json",
    ///     "data/rails",
    ///     "11619",
    /// )?;
    /// let position = course.position_at_rate(50.0);
    /// assert_eq!(position, 850.0);
    /// assert_eq!(course.nominal_position(position), 800.0);
    /// assert_eq!(course.distance_rate(position), 50.0);
    /// # Ok(())
    /// # }
    /// ```
    pub fn position_at_rate(&self, rate: f64) -> f64 {
        self.run_up + rate / 100.0 * self.distance as f64
    }

    /// Get the length of a single section in meters.
    ///
    /// There are 24 equally-sized sections in the nominal distance.
    pub fn section_length(&self) -> f64 {
        self.distance as f64 / 24.0
    }

    /// Get the start position in meters of section `section` (0-24, the latter
    /// being the goal).
    ///
    /// See [`Course::section_from_position`] for an example.
    pub fn section_start(&self, section: u8) -> f64 {
        debug_assert!(section <= 24, "section {section} is past the goal");
        self.run_up + self.section_length() * section as f64
    }

    /// Get the halfway point of the nominal distance in meters.
    pub fn halfway(&self) -> f64 {
        self.section_start(12)
    }

    /// Get the move lane point in meters, or `None` if the course has neither
    /// an explicit one nor corners.
    ///
    /// Courses without an explicit move lane point use the first corner start.
    pub fn move_lane_point(&self) -> Option<f64> {
        let first_corner = || self.corners.first().map(|c| c.start);
        self.move_lane_point.or_else(first_corner)
    }

    /// Get the corner number (1-4) of the corner at `idx` in the course corners.
    ///
    /// Corners are numbered backwards from the final corner, which is always the
    /// fourth. If the race goes through the same corner multiple times (i.e. more
    /// than one lap), its number is repeated.
    ///
    /// # Examples
    ///
    /// Find where a skill with `corner==3` can activate. On a course with more
    /// than four corners, it's more than one corner:
    ///
    /// ```
    /// use uma_tools::Course;
    ///
    /// # fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    /// // Tokyo, turf 3400m, with six corners.
    /// let course = Course::from_json_files(
    ///     "data/course_data.json",
    ///     "data/rails",
    ///     "10608",
    /// )?;
    /// let starts: Vec<f64> = (0..course.corners.len())
    ///     .filter(|&idx| course.corner_number(idx) == 3)
    ///     .map(|idx| course.corners[idx].start)
    ///     .collect();
    /// assert_eq!(starts, [292.0, 2350.0]);
    /// # Ok(())
    /// # }
    /// ```
    pub fn corner_number(&self, idx: usize) -> u8 {
        debug_assert!(idx < self.corners.len(), "no corner at index {idx}");
        4 - ((self.corners.len() - 1 - idx) % 4) as u8
    }

    /// Get the distance type of the course, inferred from its distance.
    pub fn distance_type(&self) -> DistanceType {
        match self.distance {
            ..=1400 => DistanceType::Short,
            1401..=1800 => DistanceType::Mile,
            1801..=2400 => DistanceType::Medium,
            _ => DistanceType::Long,
        }
    }

    /// Check if the course distance is a basis distance (multiple of 400m).
    pub fn is_basis_distance(&self) -> bool {
        self.distance % 400 == 0
    }

    /// Get the base speed for this course in m/s.
    ///
    /// The longer the distance, the lower the speed an Uma should run at.
    pub(crate) fn base_speed(&self) -> f64 {
        20.0 - (self.distance as f64 - 2000.0) / 1000.0
    }

    /// Get remaining distance in meters from a given position to the goal.
    pub fn remain_distance(&self, position: f64) -> f64 {
        (self.length() - position).max(0.0)
    }

    /// Get the distance rate (percentage of the nominal distance completed) from
    /// a given position. It's 0 during the run-up.
    ///
    /// See [`Course::position_at_rate`] for an example.
    pub fn distance_rate(&self, position: f64) -> f64 {
        (self.nominal_position(position) / self.distance as f64 * 100.0).floor()
    }

    /// Get the section index (0-23) from a given position in meters.
    ///
    /// The run-up counts as section 0.
    ///
    /// # Examples
    ///
    /// Sections split the nominal distance, so on a course with a run-up they
    /// are offset by it:
    ///
    /// ```
    /// use uma_tools::Course;
    ///
    /// # fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    /// // Santa Anita Park, dirt 1600m, with a 50m run-up.
    /// let course = Course::from_json_files(
    ///     "data/course_data.json",
    ///     "data/rails",
    ///     "11619",
    /// )?;
    /// let section = course.section_from_position(1200.0);
    /// assert_eq!(section, 17);
    /// assert!(course.section_start(section) <= 1200.0);
    /// assert!(course.section_start(section + 1) > 1200.0);
    /// # Ok(())
    /// # }
    /// ```
    pub fn section_from_position(&self, position: f64) -> u8 {
        (self.nominal_position(position) / self.section_length()).floor() as u8
    }

    /// Get the starting lane position in course widths based on the gate roll
    /// (1-indexed), with gate 1 on the rail.
    pub(crate) fn starting_lane(&self, gate_roll: u8) -> CourseWidths {
        let adjustor = match self.race_track_id {
            TRACK_ID_OOI | TRACK_ID_KAWASAKI | TRACK_ID_FUNABASHI => 0.0,
            TRACK_ID_LONGCHAMP => {
                if gate_roll >= 15 {
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
        CourseWidths::from_horse_lanes((gate_roll - 1) as f64 + adjustor)
    }

    /// Get the track name in the specified language, or `None` if the track name is
    /// not available in that language.
    pub fn track_name(&self, language: Language) -> Option<&'static str> {
        crate::names::track_name(self.race_track_id, language)
    }

    /// Check if this is an exchange race (dirt grade).
    ///
    /// Exchange races take place at Oi (10101), Kawasaki (10103), Funabashi
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

    /// Load a specific course by ID from the course data JSON file
    /// (`data/course_data.json`) and its rail's JSON file in `rail_dir`
    /// (`data/rails/`), named after the course ID.
    ///
    /// # Errors
    ///
    /// Returns an error if a file can't be read, the JSON is invalid, or the
    /// course ID isn't found in the course data or isn't a valid number.
    ///
    /// See [`Course::from_json`] for an example.
    pub fn from_json_files(
        course_path: impl AsRef<Path>,
        rail_dir: impl AsRef<Path>,
        course_id: &str,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        use std::fs;
        let course_json = fs::read_to_string(course_path)?;
        let rail_json = fs::read_to_string(rail_dir.as_ref().join(format!("{course_id}.json")))?;
        Self::from_json(&course_json, &rail_json, course_id)
    }

    /// Load a specific course by ID from a course data JSON string, and the
    /// JSON string of its rail.
    ///
    /// # Errors
    ///
    /// Returns an error if the JSON is invalid, or the course ID isn't found in
    /// the course data or isn't a valid number.
    ///
    /// # Examples
    ///
    /// Load several courses while reading the course data only once, e.g. to
    /// compare a runner across distances:
    ///
    /// ```
    /// use std::fs;
    /// use uma_tools::Course;
    ///
    /// # fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    /// let courses = fs::read_to_string("data/course_data.json")?;
    /// // Tokyo, turf 2000m and 2400m.
    /// let courses = ["10604", "10606"]
    ///     .into_iter()
    ///     .map(|id| {
    ///         let rail = fs::read_to_string(format!("data/rails/{id}.json"))?;
    ///         Course::from_json(&courses, &rail, id)
    ///     })
    ///     .collect::<Result<Vec<_>, _>>()?;
    /// assert_eq!(courses[1].distance - courses[0].distance, 400);
    /// # Ok(())
    /// # }
    /// ```
    pub fn from_json(
        course_json: &str,
        rail_json: &str,
        course_id: &str,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        use std::collections::HashMap;

        let mut courses: HashMap<String, Course> = serde_json::from_str(course_json)?;

        let mut course = courses
            .remove(course_id)
            .ok_or_else(|| format!("Course ID '{}' not found in JSON", course_id))?;

        course.id = course_id
            .parse::<u16>()
            .map_err(|_| format!("Invalid course ID format: '{}'", course_id))?;
        course.rail = serde_json::from_str(rail_json)?;

        Ok(course)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::synthetic_course;

    /// Create a minimal course at the given racetrack.
    ///
    /// The course ID is unrelated to the racetrack ID, so that checks on the
    /// wrong one fail.
    fn create_test_course(track_id: u16) -> Course {
        Course {
            race_track_id: track_id,
            ..synthetic_course()
        }
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
        assert_eq!(course.width, CourseWidths(1.47));
        assert_eq!(course.slopes[0].grade, 1.0);
    }

    /// Create a 2400m course with a 100m run-up (100m sections).
    fn create_run_up_course() -> Course {
        Course {
            distance: 2400,
            run_up: 100.0,
            ..synthetic_course()
        }
    }

    #[test]
    fn test_run_up_offsets_nominal_distance() {
        let course = create_run_up_course();
        assert_eq!(course.length(), 2500.0);
        assert_eq!(course.section_length(), 100.0);
        assert_eq!(course.section_start(0), 100.0);
        assert_eq!(course.section_start(24), 2500.0);
        assert_eq!(course.halfway(), 1300.0);
        assert_eq!(course.position_at_rate(50.0), 1300.0);

        // (position, nominal position, distance rate, remaining, section)
        let cases = [
            (0.0, 0.0, 0.0, 2500.0, 0),
            (99.0, 0.0, 0.0, 2401.0, 0),
            (100.0, 0.0, 0.0, 2400.0, 0),
            (200.0, 100.0, 4.0, 2300.0, 1),
            (1300.0, 1200.0, 50.0, 1200.0, 12),
            (2499.0, 2399.0, 99.0, 1.0, 23),
            (2600.0, 2500.0, 104.0, 0.0, 25),
        ];
        for (position, nominal, rate, remaining, section) in cases {
            assert_eq!(course.nominal_position(position), nominal, "{position}m");
            assert_eq!(course.distance_rate(position), rate, "{position}m");
            assert_eq!(course.remain_distance(position), remaining, "{position}m");
            assert_eq!(
                course.section_from_position(position),
                section,
                "{position}m"
            );
        }
    }

    #[test]
    fn test_phase_with_run_up() {
        let course = create_run_up_course();
        let cases = [
            (Phase::RunUp, (0.0, 100.0)),
            (Phase::Early, (100.0, 500.0)),
            (Phase::Mid, (500.0, 1700.0)),
            (Phase::Late, (1700.0, 2100.0)),
            (Phase::Last, (2100.0, 2500.0)),
        ];
        for (phase, (start, end)) in cases {
            assert_eq!(phase.boundaries(&course), (start, end), "{phase:?}");
            assert_eq!(Phase::from_position(start, &course), phase, "{phase:?}");
        }
        assert_eq!(Phase::Mid.boundaries_first_quarter(&course), (500.0, 800.0));
        assert_eq!(Phase::Mid.boundaries_first_half(&course), (500.0, 1100.0));
        assert_eq!(
            Phase::Last.boundaries_second_half(&course),
            (2300.0, 2500.0)
        );

        // The run-up precedes every other phase, with -1 as its value.
        assert!(Phase::RunUp < Phase::Early);
        assert_eq!(Phase::RunUp as i32, -1);
        assert_eq!(Phase::try_from(-1), Ok(Phase::RunUp));

        // Without a run-up, the race starts in the early phase.
        let course = synthetic_course();
        assert_eq!(Phase::from_position(0.0, &course), Phase::Early);
        assert_eq!(Phase::RunUp.boundaries(&course), (0.0, 0.0));
    }

    #[test]
    fn test_distance_type_from_distance() {
        let cases = [
            (1000, DistanceType::Short),
            (1400, DistanceType::Short),
            (1401, DistanceType::Mile),
            (1800, DistanceType::Mile),
            (1801, DistanceType::Medium),
            (2400, DistanceType::Medium),
            (2401, DistanceType::Long),
            (3600, DistanceType::Long),
        ];
        for (distance, expected) in cases {
            let course = Course {
                distance,
                ..synthetic_course()
            };
            assert_eq!(course.distance_type(), expected, "{distance}m");
        }
    }

    #[test]
    fn test_move_lane_point() {
        let corner = Corner {
            start: 400.0,
            length: 200.0,
        };
        let course = |move_lane_point, corners| Course {
            move_lane_point,
            corners,
            ..synthetic_course()
        };

        // An explicit move lane point takes precedence over the first corner.
        let explicit = course(Some(1.0), vec![corner.clone()]);
        assert_eq!(explicit.move_lane_point(), Some(1.0));
        assert_eq!(course(None, vec![corner]).move_lane_point(), Some(400.0));
        assert_eq!(course(None, vec![]).move_lane_point(), None);

        // Niigata 1000m straight course.
        let niigata = crate::test_utils::load_course(10301);
        assert_eq!(niigata.move_lane_point(), Some(1.0));
    }

    #[test]
    fn test_is_exchange_race() {
        // Oi, Kawasaki, Funabashi, Morioka.
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
    fn test_is_basis_distance() {
        for (distance, expected) in [(2000, true), (2400, true), (2100, false)] {
            let course = Course {
                distance,
                ..crate::test_utils::synthetic_course()
            };
            assert_eq!(course.is_basis_distance(), expected, "{distance}m");
        }
    }

    #[test]
    fn test_starting_lane_adjustment_by_racetrack() {
        let lane = CourseWidths::from_horse_lanes;
        // (racetrack, gate roll, expected lane in horse lanes)
        let cases = [
            // Gate 1 starts on the rail.
            (10001, 1, 0.0),
            // Oi, Kawasaki and Funabashi: never adjusted.
            (TRACK_ID_OOI, 9, 8.0),
            (TRACK_ID_OOI, 12, 11.0),
            (TRACK_ID_KAWASAKI, 12, 11.0),
            (TRACK_ID_FUNABASHI, 12, 11.0),
            // Longchamp: adjusted from gate roll 15.
            (TRACK_ID_LONGCHAMP, 14, 13.0),
            (TRACK_ID_LONGCHAMP, 15, 14.0 + 1.86),
            // Anywhere else: adjusted from gate roll 10.
            (10001, 9, 8.0),
            (10001, 10, 9.0 + 0.6),
        ];
        for (track_id, gate_roll, expected) in cases {
            assert_eq!(
                create_test_course(track_id).starting_lane(gate_roll),
                lane(expected),
                "track {track_id}, gate roll {gate_roll}"
            );
        }

        // Course 10101 is at Sapporo, not at Oi (racetrack 10101).
        let course = Course {
            id: 10101,
            race_track_id: 10001,
            ..synthetic_course()
        };
        assert_eq!(course.starting_lane(12), lane(11.0 + 0.6));
    }

    #[test]
    fn test_corner_number() {
        let course_with_corners = |count: usize| {
            let mut course = create_test_course(10001);
            course.corners = (0..count)
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
    fn test_from_json_errors() {
        // Course data of 10101 under its own ID and under a non-numeric one.
        let json = std::fs::read_to_string("data/course_data.json").unwrap();
        let courses: serde_json::Value = serde_json::from_str(&json).unwrap();
        let data = &courses["10101"];
        let json = serde_json::json!({ "10101": data, "abc": data }).to_string();
        let rail = std::fs::read_to_string("data/rails/10101.json").unwrap();

        assert_eq!(Course::from_json(&json, &rail, "10101").unwrap().id, 10101);
        let error = Course::from_json(&json, &rail, "10102").unwrap_err();
        assert_eq!(error.to_string(), "Course ID '10102' not found in JSON");
        let error = Course::from_json(&json, &rail, "abc").unwrap_err();
        assert_eq!(error.to_string(), "Invalid course ID format: 'abc'");
        assert!(Course::from_json(&json, "{}", "10101").is_err());
    }

    #[test]
    fn test_every_course_has_a_rail() {
        let json = std::fs::read_to_string("data/course_data.json").unwrap();
        let courses: std::collections::HashMap<String, Course> =
            serde_json::from_str(&json).unwrap();
        let mut course_ids: Vec<_> = courses.into_keys().collect();
        let mut rail_ids: Vec<_> = std::fs::read_dir("data/rails")
            .unwrap()
            .map(|entry| {
                let path = entry.unwrap().path();
                let json = std::fs::read_to_string(&path).unwrap();
                let rail = serde_json::from_str::<Rail>(&json);
                assert!(rail.is_ok(), "{}: {rail:?}", path.display());
                path.file_stem().unwrap().to_string_lossy().into_owned()
            })
            .collect();
        course_ids.sort();
        rail_ids.sort();
        assert_eq!(course_ids, rail_ids);
    }

    #[test]
    fn test_every_course_has_a_track_name() {
        let json = std::fs::read_to_string("data/course_data.json").unwrap();
        let courses: std::collections::HashMap<String, Course> =
            serde_json::from_str(&json).unwrap();
        for (id, course) in courses {
            for language in [Language::Japanese, Language::English] {
                let name = course.track_name(language);
                assert!(name.is_some(), "course {id} ({language:?})");
            }
        }
        assert_eq!(create_test_course(0).track_name(Language::English), None);
    }

    #[test]
    fn test_lane_type_from_course_widths() {
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
            assert_eq!(LaneType::from(CourseWidths(lane)), expected, "lane {lane}");
        }
    }
}
