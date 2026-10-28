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
fn test_is_near() {
    let lane = CourseWidths::from_horse_lanes;
    let version = GameVersion::LATEST;
    assert!(is_near(2.9, lane(2.9), version));
    assert!(is_near(2.9, -lane(2.9), version));
    assert!(!is_near(3.0, CourseWidths(0.0), version));
    assert!(!is_near(0.0, lane(3.0), version));
    assert!(!is_near(0.0, -lane(3.0), version));
}

#[test]
fn test_check_front_blocked_lane_side() {
    let lane = CourseWidths::from_horse_lanes;
    assert!(check_front_blocked(0.0, lane(0.7)));
    assert!(check_front_blocked(0.0, -lane(0.7)));
    assert!(!check_front_blocked(0.0, lane(0.8)));
    assert!(!check_front_blocked(0.0, -lane(0.8)));
}
