//! Extra world distance covered by running wide through corners or changing
//! lanes, which slows down progress along the course.
//!
//! Course positions are measured along the inner rail (lane 0), whose world
//! length matches the course distance. A runner off the rail through a corner
//! follows a wider arc, and one changing lanes moves diagonally, so both
//! cover more world distance than course distance. Each frame, the game takes
//! the ratio of world distance to course distance covered on the last frame,
//! and divides the course distance covered on the next one by it when above
//! 1.
//!
//! Corners are modeled with flat curvature: their turning angle spread evenly
//! over their length. Over a corner taken at a constant lane, the extra
//! distance (lane distance times turning angle) is the same as the game's,
//! even though the game's curvature builds up and eases off within corners.
//! Slopes, which lower the game's ratio by a negligible amount, are ignored.

use crate::units::CourseWidths;

/// Course distance and starting lane of a runner's last move.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct LastMove {
    /// Course distance covered in meters.
    pub(crate) distance: f64,
    /// Lane at the start of the move.
    pub(crate) lane: CourseWidths,
}

/// Ratio of world distance to course distance of a move, at least 1.
///
/// The move covers `distance` meters along the course from `start_lane` to
/// `end_lane`, on a stretch with `curvature` in radians per
/// meter (0 outside corners).
pub(crate) fn ratio(
    distance: f64,
    start_lane: CourseWidths,
    end_lane: CourseWidths,
    curvature: f64,
) -> f64 {
    if distance <= 0.0 {
        return 1.0;
    }
    let mid_lane = ((start_lane + end_lane) / 2.0).meters();
    let forward = distance * (1.0 + curvature * mid_lane);
    let lateral = (end_lane - start_lane).meters();
    (forward.hypot(lateral) / distance).max(1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    const EPSILON: f64 = 1e-12;

    #[test]
    fn test_ratio_on_straight_without_lane_change() {
        assert_eq!(ratio(1.5, CourseWidths(0.5), CourseWidths(0.5), 0.0), 1.0);
    }

    #[test]
    fn test_ratio_without_move() {
        assert_eq!(ratio(0.0, CourseWidths(0.0), CourseWidths(0.1), 0.01), 1.0);
    }

    #[test]
    fn test_ratio_on_corner_grows_with_lane() {
        // 90 degrees over 150m, 1 course width (11.25m) off the rail.
        let curvature = 90f64.to_radians() / 150.0;
        let expected = 1.0 + curvature * 11.25;
        assert!(
            (ratio(1.5, CourseWidths(1.0), CourseWidths(1.0), curvature) - expected).abs()
                < EPSILON
        );
        assert_eq!(
            ratio(1.5, CourseWidths(0.0), CourseWidths(0.0), curvature),
            1.0
        );
    }

    #[test]
    fn test_ratio_with_lane_change() {
        // 3m forward and 0.4 course widths (4.5m) sideways.
        let expected = 3f64.hypot(4.5) / 3.0;
        assert!((ratio(3.0, CourseWidths(0.2), CourseWidths(0.6), 0.0) - expected).abs() < EPSILON);
        // Moving inward costs the same.
        assert!((ratio(3.0, CourseWidths(0.6), CourseWidths(0.2), 0.0) - expected).abs() < EPSILON);
    }

    #[test]
    fn test_ratio_on_corner_uses_mid_lane() {
        let curvature = 0.01;
        let forward = 2.0 * (1.0 + curvature * CourseWidths(0.5).meters());
        let expected = forward.hypot(CourseWidths(0.2).meters()) / 2.0;
        assert!(
            (ratio(2.0, CourseWidths(0.4), CourseWidths(0.6), curvature) - expected).abs()
                < EPSILON
        );
    }
}
