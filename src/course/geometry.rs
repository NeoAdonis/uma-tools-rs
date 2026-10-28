//! Course segments: corners, straights and slopes.
//!
//! Each kind of segment is deserialized from the course data and implements
//! [`Segment`], which gives a common way to check the range it spans along the
//! course.
//!
//! # Examples
//!
//! Check which segments a position falls in, e.g. whether a runner is already
//! on the uphill of the final straight:
//!
//! ```
//! use uma_tools::Course;
//! use uma_tools::course::geometry::Segment;
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
//! // Tokyo, turf 2400m.
//! let course = Course::from_json_file("data/course_data.json", "10606")?;
//! let position = 2000.0;
//!
//! let straight = course.straights.iter().find(|s| s.contains(position));
//! assert_eq!(straight, course.straights.last());
//! let slope = course.slopes.iter().find(|s| s.contains(position));
//! assert!(slope.is_some_and(|slope| slope.is_uphill()));
//! # Ok(())
//! # }
//! ```

use serde::Deserialize;
use serde_repr::{Deserialize_repr, Serialize_repr};

use crate::units::deserialize_ten_thousandths;

/// Course segment (corner, slope or straight) spanning from `start()`
/// (inclusive) to `end()` (exclusive), in meters.
///
/// This trait is sealed: it's only implemented by [`Corner`], [`Slope`] and
/// [`Straight`], and can't be implemented outside this crate.
pub trait Segment: private::Sealed {
    fn start(&self) -> f64;
    fn end(&self) -> f64;

    /// Get the `(start, end)` range of the segment.
    fn range(&self) -> (f64, f64) {
        (self.start(), self.end())
    }

    /// Check if `position` is within the segment.
    fn contains(&self, position: f64) -> bool {
        position >= self.start() && position < self.end()
    }
}

mod private {
    /// Supertrait that keeps [`Segment`](super::Segment) from being implemented
    /// outside this crate.
    pub trait Sealed {}

    impl Sealed for super::Corner {}
    impl Sealed for super::Slope {}
    impl Sealed for super::Straight {}
}

/// Represents a corner on the course.
#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Corner {
    /// Start position of the corner in meters.
    pub start: f64,
    /// Length of the corner in meters.
    pub length: f64,
    /// Total angle the course turns through within the corner in degrees,
    /// always positive (the turning side is given by the course direction).
    pub turn_angle: f64,
}

impl Corner {
    /// Curvature in radians per meter, as if the corner turned evenly.
    ///
    /// # Examples
    ///
    /// Estimate the extra distance a runner covers by taking a corner further
    /// out, which is its curvature times the lane offset over its length:
    ///
    /// ```
    /// use uma_tools::course::geometry::Corner;
    /// use uma_tools::units::CourseWidths;
    ///
    /// let corner = Corner { start: 500.0, length: 150.0, turn_angle: 90.0 };
    /// let lane = CourseWidths(0.5);
    /// let extra = corner.curvature() * lane.meters() * corner.length;
    /// assert_eq!(extra.round(), 9.0);
    /// ```
    pub fn curvature(&self) -> f64 {
        self.turn_angle.to_radians() / self.length
    }
}

impl Segment for Corner {
    fn start(&self) -> f64 {
        self.start
    }

    fn end(&self) -> f64 {
        self.start + self.length
    }
}

/// Represents the slope type of a course segment.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum SlopeType {
    Flat = 0,
    Uphill = 1,
    Downhill = 2,
}

/// Represents a slope segment on the course.
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct Slope {
    /// Start position of the slope in meters.
    pub start: f64,
    /// Length of the slope in meters.
    pub length: f64,
    /// Slope grade as a percentage (positive uphill, negative downhill).
    ///
    /// Stored in 10000ths of a percent in the course data, and converted when
    /// deserializing.
    #[serde(deserialize_with = "deserialize_ten_thousandths")]
    pub grade: f64,
}

impl Segment for Slope {
    fn start(&self) -> f64 {
        self.start
    }

    fn end(&self) -> f64 {
        self.start + self.length
    }
}

impl Slope {
    /// Create a new Slope for testing purposes.
    #[cfg(test)]
    pub fn new(start: f64, length: f64, grade: f64) -> Self {
        Slope {
            start,
            length,
            grade,
        }
    }

    /// Check if the slope is uphill.
    pub fn is_uphill(&self) -> bool {
        self.grade > 0.0
    }

    /// Check if the slope is downhill.
    pub fn is_downhill(&self) -> bool {
        self.grade < 0.0
    }

    /// Get the slope type classification.
    pub fn slope_type(&self) -> SlopeType {
        if self.grade > 0.0 {
            SlopeType::Uphill
        } else if self.grade < 0.0 {
            SlopeType::Downhill
        } else {
            SlopeType::Flat
        }
    }
}

/// Represents whether a straight segment is facing the audience or not.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Serialize_repr, Deserialize_repr)]
#[repr(u8)]
pub enum FrontType {
    /// Towards the audience.
    Audience = 1,
    /// Away from the audience.
    Opposite = 2,
    Unknown = 3,
}

/// Represents a straight segment on the course.
#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Straight {
    /// Start position of the straight in meters.
    pub start: f64,
    /// End position of the straight in meters.
    pub end: f64,
    /// Whether the straight is facing the audience or not.
    pub front_type: FrontType,
}

impl Segment for Straight {
    fn start(&self) -> f64 {
        self.start
    }

    fn end(&self) -> f64 {
        self.end
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_segment_bounds() {
        let corner = Corner {
            start: 100.0,
            length: 50.0,
            turn_angle: 90.0,
        };
        assert_eq!(corner.range(), (100.0, 150.0));

        let slope = Slope::new(200.0, 30.0, 1.5);
        assert_eq!(slope.range(), (200.0, 230.0));

        let straight = Straight {
            start: 300.0,
            end: 400.0,
            front_type: FrontType::Audience,
        };
        assert_eq!(straight.range(), (300.0, 400.0));

        // Start is inclusive, end is exclusive.
        assert!(corner.contains(100.0));
        assert!(corner.contains(149.9));
        assert!(!corner.contains(150.0));
        assert!(!corner.contains(99.9));
    }

    #[test]
    fn test_slope_type() {
        // Positive is uphill, negative is downhill.
        let cases = [
            (1.5, SlopeType::Uphill),
            (0.0, SlopeType::Flat),
            (-1.5, SlopeType::Downhill),
        ];
        for (gradient, expected) in cases {
            let slope = Slope::new(100.0, 50.0, gradient);
            assert_eq!(slope.slope_type(), expected, "gradient {gradient}");
        }
    }
}
