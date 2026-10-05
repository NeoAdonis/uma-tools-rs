//! Proximity checks between two runners.
//!
//! Each check takes the `distance` (m) from the runner behind to the runner
//! ahead, which must be non-negative, and their `lane_diff` (m), whose sign
//! doesn't matter.

use crate::configuration::GameVersion;
use crate::units::{course_widths_to_meters, horse_lanes_to_meters};

/// Check if a runner is blocked by another runner from the front based on
/// distance and lane difference.
///
/// When front-blocked, the runner's speed is capped.
pub fn check_front_blocked(distance: f64, lane_diff: f64) -> bool {
    if !(0.0..2.0).contains(&distance) {
        return false;
    }
    // Lane difference threshold decreases linearly with distance between runners.
    // At 0m, threshold is 0.75 lanes.
    // At 2m, threshold is 0.3 lanes.
    lane_diff.abs() <= horse_lanes_to_meters((1.0 - 0.3 * distance) * 0.75)
}

/// Check if a runner is blocking another runner from the side based on distance
/// and lane difference.
///
/// When side-blocked, the runner cannot change lanes.
pub fn check_side_blocked(distance: f64, lane_diff: f64) -> bool {
    is_near_by(distance, lane_diff, 1.05, 2.0)
}

/// Check if a runner is near another runner based on distance and lane
/// difference.
///
/// Two runners are near if they are less than 3m apart and less than 3 lanes to
/// either side. Before the 1st anniversary update, both thresholds were 1.5
/// (1.5m and 1.5 lanes).
///
/// This is checked by certain skills (`near_count`).
pub fn is_near(distance: f64, lane_diff: f64, game_version: GameVersion) -> bool {
    let threshold = if game_version >= GameVersion::FirstAnniversary {
        3.0
    } else {
        1.5
    };
    is_near_by(distance, lane_diff, threshold, threshold)
}

/// Check if a runner is near another runner and in the same lane based on
/// distance and lane difference.
///
/// This is checked by certain skills.
pub fn is_near_same_lane(distance: f64, lane_diff: f64) -> bool {
    is_near_by(distance, lane_diff, 2.5, 1.0)
}

/// Check if a runner is behind another runner within a reasonable distance and
/// lane difference.
///
/// This is checked by certain skills.
pub fn is_near_behind_set1(distance: f64, lane_diff: f64) -> bool {
    is_near_by(distance, lane_diff, 5.0, 2.7)
}

/// Check if a runner is surrounding another runner from the front (and
/// conversely, being surrounded by the other runner from behind) based on
/// distance and lane difference.
///
/// This is checked by certain skills.
pub fn is_front_surrounded(distance: f64, lane_diff: f64) -> bool {
    is_near_by(distance, lane_diff, 3.0, 1.5)
}

/// Check if a runner is surrounding (and being surrounded by) another runner
/// from the side based on distance and lane difference.
///
/// This is checked by certain skills.
pub fn is_side_surrounded(distance: f64, lane_diff: f64) -> bool {
    is_near_by(distance, lane_diff, 1.5, 3.0)
}

/// Check if a runner is within dueling range of another runner based on
/// distance and lane difference.
///
/// Two runners are in range if they are less than 3m apart and less than
/// 0.25 course widths to either side.
pub fn check_dueling(distance: f64, lane_diff: f64) -> bool {
    (0.0..3.0).contains(&distance) && lane_diff.abs() < course_widths_to_meters(0.25)
}

/// Check if a runner is overlapping with another runner based on position and
/// lane difference.
pub fn is_overlapping(distance: f64, lane_diff: f64) -> bool {
    is_near_by(distance, lane_diff, 0.4, 0.4)
}

/// Check if a runner is near another runner within a specified distance
/// and lane difference.
///
/// `distance`, `lane_diff` and `max_distance` are in meters, while
/// `max_lane_diff` is in horse lanes.
pub fn is_near_by(distance: f64, lane_diff: f64, max_distance: f64, max_lane_diff: f64) -> bool {
    if !(0.0..max_distance).contains(&distance) {
        return false;
    }
    lane_diff.abs() < horse_lanes_to_meters(max_lane_diff)
}

#[cfg(test)]
mod tests;
