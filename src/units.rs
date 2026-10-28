//! Unit conversion utilities for race measurements.

use derive_more::{Add, AddAssign, Div, Mul, Neg, Sub, SubAssign};
use serde::{Deserialize, Deserializer};

/// Convert a value from 10000ths while deserializing, as most fractional values
/// are stored in the game data.
pub(crate) fn deserialize_ten_thousandths<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<f64, D::Error> {
    Ok(f64::deserialize(deserializer)? / 10000.0)
}

/// Course width to meters conversion unit. 1 course width = 11.25 meters.
pub const COURSE_WIDTH_TO_METERS: f64 = 11.25;

/// Horse lanes per course width. 1 horse lane = 1/18 course width = 0.625
/// meters.
pub const HORSE_LANES_PER_COURSE_WIDTH: f64 = 18.0;

/// Lateral position or distance across the course, in course widths.
///
/// Lane positions are measured from the inner fence. For example, a turf course
/// in Tokyo is 1.5 course widths wide, which is 16.875 meters.
///
/// Game formulas often use horse lanes instead (see
/// [`HORSE_LANES_PER_COURSE_WIDTH`]); they're plain numbers, converted with
/// [`CourseWidths::from_horse_lanes`], so that both units can't be mixed up.
///
/// # Examples
///
/// Check whether two runners are within a lane threshold that a game formula
/// gives in horse lanes:
///
/// ```
/// use uma_tools::units::CourseWidths;
///
/// let runner = CourseWidths(0.5);
/// let other = CourseWidths(0.375);
/// let max_lane_diff = CourseWidths::from_horse_lanes(3.0);
///
/// let gap = (runner - other).abs();
/// assert!(gap <= max_lane_diff);
/// assert_eq!(gap.horse_lanes(), 2.25);
/// assert_eq!(gap.meters(), 1.40625);
/// ```
#[derive(
    Debug,
    Clone,
    Copy,
    Default,
    PartialEq,
    PartialOrd,
    Add,
    AddAssign,
    Sub,
    SubAssign,
    Neg,
    Mul,
    Div,
)]
pub struct CourseWidths(pub f64);

impl CourseWidths {
    /// Convert horse lanes to course widths.
    pub const fn from_horse_lanes(lanes: f64) -> Self {
        Self(lanes / HORSE_LANES_PER_COURSE_WIDTH)
    }

    /// Convert to horse lanes.
    pub const fn horse_lanes(self) -> f64 {
        self.0 * HORSE_LANES_PER_COURSE_WIDTH
    }

    /// Convert to meters.
    pub const fn meters(self) -> f64 {
        self.0 * COURSE_WIDTH_TO_METERS
    }

    /// Absolute value, e.g. of a lane difference.
    pub fn abs(self) -> Self {
        Self(self.0.abs())
    }

    /// Minimum of both values.
    pub fn min(self, other: Self) -> Self {
        Self(self.0.min(other.0))
    }

    /// Maximum of both values.
    pub fn max(self, other: Self) -> Self {
        Self(self.0.max(other.0))
    }

    /// Restrict the value to `min..=max`.
    pub fn clamp(self, min: Self, max: Self) -> Self {
        Self(self.0.clamp(min.0, max.0))
    }
}

/// Ratio between both values.
impl std::ops::Div for CourseWidths {
    type Output = f64;

    fn div(self, divisor: Self) -> f64 {
        self.0 / divisor.0
    }
}

/// Bashin (バ身, horse-length) to meters conversion unit. 1 bashin = 2.5 meters.
pub const BASHIN_TO_METERS: f64 = 2.5;

/// Convert bashin to meters.
///
/// Inverse of [`meters_to_bashin`], which has an example.
pub fn bashin_to_meters(bashin: f64) -> f64 {
    bashin * BASHIN_TO_METERS
}

/// Convert meters to bashin.
///
/// # Examples
///
/// Evaluate the `bashin_diff_infront` skill condition variable, which counts
/// whole bashin to the runner ahead:
///
/// ```
/// use uma_tools::units::{bashin_to_meters, meters_to_bashin};
///
/// let runner = 1200.0;
/// let runner_ahead = 1206.0;
/// let bashin_diff_infront = meters_to_bashin(runner_ahead - runner).floor();
/// assert_eq!(bashin_diff_infront, 2.0);
///
/// // `bashin_diff_infront<=1` holds until the gap reaches 2 bashin.
/// assert_eq!(bashin_to_meters(2.0), 5.0);
/// ```
pub fn meters_to_bashin(meters: f64) -> f64 {
    meters / BASHIN_TO_METERS
}

/// Furlong to meters conversion unit. 1 furlong = 201.168 meters.
// ? The standard definition of furlong is presumably used.
pub const FURLONG_TO_METERS: f64 = 201.168;

/// Convert furlongs to meters.
///
/// Inverse of [`meters_to_furlongs`], which has an example.
pub fn furlongs_to_meters(furlongs: f64) -> f64 {
    furlongs * FURLONG_TO_METERS
}

/// Convert meters to furlongs.
///
/// # Examples
///
/// Evaluate the `furlong` skill condition variable, which counts whole
/// furlongs covered from the official start line:
///
/// ```
/// use uma_tools::units::{furlongs_to_meters, meters_to_furlongs};
///
/// let furlong = meters_to_furlongs(1500.0).floor();
/// assert_eq!(furlong, 7.0);
///
/// // `furlong>=8` starts to hold about 1609 meters in.
/// assert_eq!(furlongs_to_meters(8.0).round(), 1609.0);
/// ```
pub fn meters_to_furlongs(meters: f64) -> f64 {
    meters / FURLONG_TO_METERS
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::assert_close;

    #[test]
    fn test_horse_lane_conversion() {
        assert_eq!(CourseWidths::from_horse_lanes(1.0).meters(), 0.625);
        assert_eq!(CourseWidths::from_horse_lanes(18.0), CourseWidths(1.0));
        assert_eq!(CourseWidths(0.5).horse_lanes(), 9.0);
    }

    #[test]
    fn test_bashin_conversion() {
        assert_eq!(bashin_to_meters(1.0), 2.5);
        assert_eq!(bashin_to_meters(4.0), 10.0);
        assert_eq!(meters_to_bashin(2.5), 1.0);
        assert_eq!(meters_to_bashin(10.0), 4.0);
    }

    #[test]
    fn test_furlong_conversion() {
        assert_close!(furlongs_to_meters(1.0), 201.168, tolerance = 0.001);
        assert_close!(meters_to_furlongs(201.168), 1.0, tolerance = 0.001);
    }

    #[test]
    fn test_bashin_roundtrip() {
        let original = 5.0;
        let converted = bashin_to_meters(meters_to_bashin(original));
        assert_close!(converted, original, tolerance = 0.0001);
    }

    #[test]
    fn test_furlong_roundtrip() {
        let original = 1000.0;
        let converted = furlongs_to_meters(meters_to_furlongs(original));
        assert_close!(converted, original, tolerance = 0.0001);
    }
}
