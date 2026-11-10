use super::*;
use crate::units::CourseWidths;

#[test]
fn test_is_near_by() {
    let lane = CourseWidths::from_horse_lanes;
    // Within range, on either side.
    assert!(is_near_by(0.0, CourseWidths(0.0), 3.0, 3.0));
    assert!(is_near_by(2.9, lane(2.9), 3.0, 3.0));
    assert!(is_near_by(2.9, -lane(2.9), 3.0, 3.0));
    // Out of distance range.
    assert!(!is_near_by(3.0, CourseWidths(0.0), 3.0, 3.0));
    assert!(!is_near_by(-0.1, CourseWidths(0.0), 3.0, 3.0));
    // Out of lane range, on either side.
    assert!(!is_near_by(1.0, lane(3.0), 3.0, 3.0));
    assert!(!is_near_by(1.0, -lane(3.0), 3.0, 3.0));
}

#[test]
fn test_is_near_by_whole_lanes_apart() {
    let lane = CourseWidths::from_horse_lanes;
    // Runners exactly 2 horse lanes apart, as gates place them (with or
    // without extra space), aren't near whatever the rounding of their lanes.
    for extra in [0.0, 0.6, 1.86] {
        for inner in 0..17 {
            let inner = inner as f64 + extra;
            let lane_diff = lane(inner + 2.0) - lane(inner);
            assert!(!is_near_by(0.5, lane_diff, 1.05, 2.0), "{inner} lanes");
        }
    }
    assert!(is_near_by(0.5, lane(1.999), 1.05, 2.0));
}

#[test]
fn test_is_near_by_version() {
    let lane = CourseWidths::from_horse_lanes;
    // (version, distance, whether near), with as many lanes as meters apart.
    let cases = [
        (GameVersion::Original, 1.4, true),
        (GameVersion::Original, 1.5, false),
        (GameVersion::LATEST, 2.9, true),
        (GameVersion::LATEST, 3.0, false),
    ];
    for (version, distance, expected) in cases {
        let actual = is_near(distance, lane(distance - 0.1), version);
        assert_eq!(actual, expected, "{version:?} {distance}m");
    }
}

#[test]
fn test_check_front_blocked() {
    let lane = CourseWidths::from_horse_lanes;
    // Lane threshold shrinks from 0.75 lanes (touching) to 0.3 lanes (2m).
    let cases = [
        (0.01, lane(0.7), true),
        (0.01, -lane(0.7), true),
        (0.01, lane(0.8), false),
        (1.0, lane(0.52), true),
        (1.0, lane(0.53), false),
        (1.99, lane(0.3), true),
        // Only strictly between 0m and 2m.
        (0.0, CourseWidths(0.0), false),
        (2.0, CourseWidths(0.0), false),
    ];
    for (distance, lane_diff, expected) in cases {
        let actual = check_front_blocked(distance, lane_diff);
        assert_eq!(actual, expected, "{distance}m, {lane_diff:?}");
    }
}
