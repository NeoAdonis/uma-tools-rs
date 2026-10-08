//! Unit conversion utilities for race measurements.

use serde::{Deserialize, Deserializer};

/// Convert a value from 10000ths while deserializing, as most fractional values
/// are stored in the game data.
pub(crate) fn deserialize_ten_thousandths<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<f64, D::Error> {
    Ok(f64::deserialize(deserializer)? / 10000.0)
}

/// Course width to meters conversion unit. 1 course width = 11.25 meters.
///
/// Lane positions are measured in course widths. For example, a turf course in
/// Tokyo is 1.5 courses wide, which is 16.875 meters.
pub const COURSE_WIDTH_TO_METERS: f64 = 11.25;

/// Convert course widths to meters.
pub fn course_widths_to_meters(widths: f64) -> f64 {
    widths * COURSE_WIDTH_TO_METERS
}

/// Horse lanes per course width. 1 horse lane = 1/18 course width = 0.625
/// meters.
pub const HORSE_LANES_PER_COURSE_WIDTH: f64 = 18.0;

/// Convert horse lanes to course widths.
pub fn horse_lanes_to_course_widths(lanes: f64) -> f64 {
    lanes / HORSE_LANES_PER_COURSE_WIDTH
}

/// Convert course widths to horse lanes.
pub fn course_widths_to_horse_lanes(widths: f64) -> f64 {
    widths * HORSE_LANES_PER_COURSE_WIDTH
}

/// Bashin (バ身, horse-length) to meters conversion unit. 1 bashin = 2.5 meters.
pub const BASHIN_TO_METERS: f64 = 2.5;

/// Convert bashin to meters.
pub fn bashin_to_meters(bashin: f64) -> f64 {
    bashin * BASHIN_TO_METERS
}

/// Convert meters to bashin.
pub fn meters_to_bashin(meters: f64) -> f64 {
    meters / BASHIN_TO_METERS
}

/// Furlong to meters conversion unit. 1 furlong = 201.168 meters.
// ? The standard definition of furlong is presumably used.
pub const FURLONG_TO_METERS: f64 = 201.168;

/// Convert furlongs to meters.
pub fn furlongs_to_meters(furlongs: f64) -> f64 {
    furlongs * FURLONG_TO_METERS
}

/// Convert meters to furlongs.
pub fn meters_to_furlongs(meters: f64) -> f64 {
    meters / FURLONG_TO_METERS
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::assert_close;

    #[test]
    fn test_horse_lane_conversion() {
        assert_eq!(
            course_widths_to_meters(horse_lanes_to_course_widths(1.0)),
            0.625
        );
        assert_eq!(horse_lanes_to_course_widths(18.0), 1.0);
        assert_eq!(course_widths_to_horse_lanes(0.5), 9.0);
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
