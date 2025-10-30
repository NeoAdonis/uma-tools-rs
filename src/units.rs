//! Unit conversion utilities for race measurements

/// Bashin (バ身, horse-length) to meters conversion
/// 1 bashin = 2.5 meters
pub const BASHIN_TO_METERS: f64 = 2.5;

/// Convert bashin to meters
///
/// # Arguments
/// * `bashin` - Distance in bashin
///
/// # Returns
/// Distance in meters
pub fn bashin_to_meters(bashin: f64) -> f64 {
    bashin * BASHIN_TO_METERS
}

/// Convert meters to bashin
///
/// # Arguments
/// * `meters` - Distance in meters
///
/// # Returns
/// Distance in bashin
pub fn meters_to_bashin(meters: f64) -> f64 {
    meters / BASHIN_TO_METERS
}

/// Furlong to meters conversion
/// TODO: Verify exact furlong conversion
pub const FURLONG_TO_METERS: f64 = 201.168;

/// Convert furlongs to meters
///
/// # Arguments
/// * `furlongs` - Distance in furlongs
///
/// # Returns
/// Distance in meters
pub fn furlongs_to_meters(furlongs: f64) -> f64 {
    furlongs * FURLONG_TO_METERS
}

/// Convert meters to furlongs
///
/// # Arguments
/// * `meters` - Distance in meters
///
/// # Returns
/// Distance in furlongs
pub fn meters_to_furlongs(meters: f64) -> f64 {
    meters / FURLONG_TO_METERS
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bashin_conversion() {
        assert_eq!(bashin_to_meters(1.0), 2.5);
        assert_eq!(bashin_to_meters(4.0), 10.0);
        assert_eq!(meters_to_bashin(2.5), 1.0);
        assert_eq!(meters_to_bashin(10.0), 4.0);
    }

    #[test]
    fn test_furlong_conversion() {
        assert!((furlongs_to_meters(1.0) - 201.168).abs() < 0.001);
        assert!((meters_to_furlongs(201.168) - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_bashin_roundtrip() {
        let original = 5.0;
        let converted = bashin_to_meters(meters_to_bashin(original));
        assert!((converted - original).abs() < 0.0001);
    }

    #[test]
    fn test_furlong_roundtrip() {
        let original = 1000.0;
        let converted = furlongs_to_meters(meters_to_furlongs(original));
        assert!((converted - original).abs() < 0.0001);
    }
}
