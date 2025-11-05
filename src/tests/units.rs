use crate::units::*;

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
