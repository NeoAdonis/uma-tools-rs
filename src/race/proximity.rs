//! Proximity checks between two runners.
//!
//! Each check takes the `distance` (m) from the runner behind to the runner
//! ahead, which must be non-negative, and their `lane_diff` (in course widths),
//! whose sign doesn't matter.

use crate::configuration::GameVersion;
use crate::units::CourseWidths;

/// Checks if a runner is blocked by another runner from the front based on
/// distance and lane difference.
///
/// When front-blocked, the runner's speed is capped. Runners at the same
/// position (e.g. at the start) don't block each other.
pub(crate) fn check_front_blocked(distance: f64, lane_diff: CourseWidths) -> bool {
    if !(distance > 0.0 && distance < 2.0) {
        return false;
    }
    // Lane difference threshold decreases linearly with distance between runners.
    // At 0m, threshold is 0.75 lanes.
    // At 2m, threshold is 0.3 lanes.
    lane_diff.abs() <= CourseWidths::from_horse_lanes((1.0 - 0.3 * distance) * 0.75)
}

/// Checks if a runner is blocking another runner from the side based on
/// distance and lane difference.
///
/// When side-blocked, the runner cannot change lanes.
pub(crate) fn check_side_blocked(distance: f64, lane_diff: CourseWidths) -> bool {
    is_near_by(distance, lane_diff, 1.05, 2.0)
}

/// Checks if a runner is near another runner based on distance and lane
/// difference.
///
/// Two runners are near if they are less than 3m apart and less than 3 lanes to
/// either side. Before the 1st anniversary update, both thresholds were 1.5
/// (1.5m and 1.5 lanes).
///
/// This is checked by certain skills (`near_count`).
pub(crate) fn is_near(distance: f64, lane_diff: CourseWidths, game_version: GameVersion) -> bool {
    let threshold = if game_version >= GameVersion::FirstAnniversary {
        3.0
    } else {
        1.5
    };
    is_near_by(distance, lane_diff, threshold, threshold)
}

/// Checks if a runner is near another runner and in the same lane based on
/// distance and lane difference.
///
/// This is checked by certain skills.
pub(crate) fn is_near_same_lane(distance: f64, lane_diff: CourseWidths) -> bool {
    is_near_by(distance, lane_diff, 2.5, 1.0)
}

/// Checks if a runner is behind another runner within a reasonable distance and
/// lane difference.
///
/// This is checked by certain skills.
pub(crate) fn is_near_behind_set1(distance: f64, lane_diff: CourseWidths) -> bool {
    is_near_by(distance, lane_diff, 5.0, 2.7)
}

/// Checks if a runner is surrounding another runner from the front (and being
/// surrounded by from behind) based on distance and lane difference.
///
/// This is checked by certain skills.
pub(crate) fn is_front_surrounded(distance: f64, lane_diff: CourseWidths) -> bool {
    is_near_by(distance, lane_diff, 3.0, 1.5)
}

/// Checks if a runner is surrounding (and being surrounded by) another runner
/// from the side based on distance and lane difference.
///
/// This is checked by certain skills.
pub(crate) fn is_side_surrounded(distance: f64, lane_diff: CourseWidths) -> bool {
    is_near_by(distance, lane_diff, 1.5, 3.0)
}

/// Distance (m) below which two runners may overlap.
pub(crate) const OVERLAP_DISTANCE: f64 = 0.4;
/// Lane difference (in horse lanes) below which two runners may overlap. An
/// overlapping runner is bumped to exactly this far from the other one.
pub(crate) const OVERLAP_LANE_GAP: f64 = 0.4;

/// Checks if a runner is overlapping with another runner based on position and
/// lane difference.
pub(crate) fn is_overlapping(distance: f64, lane_diff: CourseWidths) -> bool {
    is_near_by(distance, lane_diff, OVERLAP_DISTANCE, OVERLAP_LANE_GAP)
}

/// Rounding margin for lane differences, so that runners exactly at a threshold
/// (e.g. from their gates) count as at it.
const LANE_DIFF_TOLERANCE: CourseWidths = CourseWidths(1e-9);

/// Lane difference (in horse lanes) of the loosest lane threshold of the
/// proximity checks used when updating blocking status.
const MAX_CHECK_LANE_DIFF: f64 = 3.0;

/// Returns the lane difference at or beyond which no proximity check holds,
/// whatever the distance and game version.
pub(crate) fn lane_diff_limit() -> CourseWidths {
    CourseWidths::from_horse_lanes(MAX_CHECK_LANE_DIFF) - LANE_DIFF_TOLERANCE
}

/// Checks if a runner is near another runner within a specified distance and
/// lane difference.
///
/// `distance` and `max_distance` are in meters, `lane_diff` is in course widths
/// and `max_lane_diff` is in horse lanes. A lane difference equal to
/// `max_lane_diff` isn't near (see [`LANE_DIFF_TOLERANCE`]).
pub(crate) fn is_near_by(
    distance: f64,
    lane_diff: CourseWidths,
    max_distance: f64,
    max_lane_diff: f64,
) -> bool {
    if !(0.0..max_distance).contains(&distance) {
        return false;
    }
    lane_diff.abs() < CourseWidths::from_horse_lanes(max_lane_diff) - LANE_DIFF_TOLERANCE
}

#[cfg(test)]
mod tests;
