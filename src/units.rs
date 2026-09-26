//! Unit conversion utilities for race measurements

/// Course width to meters conversion unit. 1 course width = 11.25 meters.
///
/// Used as unit to measure course width relatively. For example, a turf
/// course in Tokyo is 1.5 courses wide, which is 16.875 meters.
pub const COURSE_WIDTH_TO_METERS: f64 = 11.25;

/// Convert course widths to meters.
pub fn course_widths_to_meters(widths: f64) -> f64 {
    widths * COURSE_WIDTH_TO_METERS
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
pub const FURLONG_TO_METERS: f64 = 201.168; // ? The standard definition of furlong is presumably used

/// Convert furlongs to meters.
pub fn furlongs_to_meters(furlongs: f64) -> f64 {
    furlongs * FURLONG_TO_METERS
}

/// Convert meters to furlongs.
pub fn meters_to_furlongs(meters: f64) -> f64 {
    meters / FURLONG_TO_METERS
}
