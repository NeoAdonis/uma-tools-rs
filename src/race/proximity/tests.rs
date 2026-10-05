use super::*;
use crate::units::HORSE_LANE_TO_METERS;

#[test]
fn test_is_near_by() {
    let lane = HORSE_LANE_TO_METERS;
    // Within range, on either side.
    assert!(is_near_by(0.0, 0.0, 3.0, 3.0));
    assert!(is_near_by(2.9, 2.9 * lane, 3.0, 3.0));
    assert!(is_near_by(2.9, -2.9 * lane, 3.0, 3.0));
    // Out of distance range.
    assert!(!is_near_by(3.0, 0.0, 3.0, 3.0));
    assert!(!is_near_by(-0.1, 0.0, 3.0, 3.0));
    // Out of lane range, on either side.
    assert!(!is_near_by(1.0, 3.0 * lane, 3.0, 3.0));
    assert!(!is_near_by(1.0, -3.0 * lane, 3.0, 3.0));
}

#[test]
fn test_is_near() {
    let lane = HORSE_LANE_TO_METERS;
    let version = GameVersion::LATEST;
    assert!(is_near(2.9, 2.9 * lane, version));
    assert!(is_near(2.9, -2.9 * lane, version));
    assert!(!is_near(3.0, 0.0, version));
    assert!(!is_near(0.0, 3.0 * lane, version));
    assert!(!is_near(0.0, -3.0 * lane, version));
}

#[test]
fn test_check_dueling() {
    // 0.25 course widths = 2.8125m.
    assert!(check_dueling(0.0, 0.0));
    assert!(check_dueling(2.9, 2.81));
    assert!(check_dueling(2.9, -2.81));
    assert!(!check_dueling(3.0, 0.0));
    assert!(!check_dueling(-0.1, 0.0));
    assert!(!check_dueling(0.0, 2.82));
    assert!(!check_dueling(0.0, -2.82));
}

#[test]
fn test_check_front_blocked_lane_side() {
    let lane = HORSE_LANE_TO_METERS;
    assert!(check_front_blocked(0.0, 0.7 * lane));
    assert!(check_front_blocked(0.0, -0.7 * lane));
    assert!(!check_front_blocked(0.0, 0.8 * lane));
    assert!(!check_front_blocked(0.0, -0.8 * lane));
}
