//! Extra world distance covered by running wide through corners or changing
//! lanes, which slows down progress along the course.
//!
//! Each frame, the game takes the ratio of world distance to course distance
//! covered on the last frame, and divides the course distance covered on the
//! next one by its ratio to the base ratio of the course when above 1 (see
//! [`Rail`](crate::course::rail::Rail)).
//!
//! The world distance of a move is that of the course's inner rail (lane 0)
//! over the same stretch, plus the lane offset (in meters) times the rail's
//! turn, combined with the lateral distance of a lane change. Off the rail,
//! a runner covers more world distance where the course turns towards the
//! inner rail, and less where it turns away from it. The rail itself also
//! covers slightly more or less world distance than the base ratio in places,
//! which changes the ratio even on the rail.
//!
//! This follows the game as long as the lanes point straight outwards from
//! the rail, but the game's rotations can lag behind the heading of the rail
//! by a few degrees in corners, which changes how much world distance a lane
//! change covers there.

use crate::course::Course;
use crate::units::CourseWidths;

/// Course distance and starting lane of a runner's last move.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct LastMove {
    /// Course distance covered in meters.
    pub(crate) distance: f64,
    /// Lane at the start of the move.
    pub(crate) lane: CourseWidths,
}

/// Ratio of world distance to course distance of a move, relative to the
/// course's base ratio, and at least 1.
///
/// The move goes along `course` from `start` to `end` (positions in meters),
/// and from `start_lane` to `end_lane`.
pub(crate) fn ratio(
    course: &Course,
    start: f64,
    end: f64,
    start_lane: CourseWidths,
    end_lane: CourseWidths,
) -> f64 {
    let distance = end - start;
    if distance <= 0.0 {
        return 1.0;
    }
    let rail = &course.rail;
    let change = rail.change(start, end, course.length());
    let mid_lane = ((start_lane + end_lane) / 2.0).meters();
    // Calculate world distance of the move at the base ratio.
    let base_distance = distance * rail.base_ratio;
    let forward = base_distance + change.excess + mid_lane * change.turn;
    let ratio = if start_lane == end_lane {
        forward / base_distance
    } else {
        let lateral = (end_lane - start_lane).meters();
        // `hypot` is much slower and more precise than needed here.
        (forward * forward + lateral * lateral).sqrt() / base_distance
    };
    // Ratio can be less than 1 where the rail covers less than at the base ratio,
    // or where the course turns the other way while off the rail.
    ratio.max(1.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::course::rail::Rail;
    use crate::test_utils::{assert_close, synthetic_course};

    /// Synthetic 2000m course with base ratio `base_ratio`, whose 2m segments
    /// between keyframes from 500m to 800m (a corner) are `excess` meters
    /// longer and turn by `turn` radians.
    fn course_with(base_ratio: f64, excess: f64, turn: f64) -> Course {
        let in_corner = |segment| (250..400).contains(&segment);
        let per_segment = |value| -> Vec<f64> {
            (0..1000)
                .map(|segment| if in_corner(segment) { value } else { 0.0 })
                .collect()
        };
        Course {
            rail: Rail::new(base_ratio, &per_segment(excess), &per_segment(turn)),
            ..synthetic_course()
        }
    }

    /// [`course_with`] a corner turning by [`CURVATURE`].
    fn course(base_ratio: f64, excess: f64) -> Course {
        course_with(base_ratio, excess, CURVATURE * 2.0)
    }

    /// Curvature of the corner of [`course`] in radians per meter.
    const CURVATURE: f64 = 0.005;

    #[test]
    fn test_ratio_on_straight_without_lane_change() {
        let course = course(1.0, 0.0);
        assert_eq!(
            ratio(&course, 100.0, 101.5, CourseWidths(0.5), CourseWidths(0.5)),
            1.0
        );
    }

    #[test]
    fn test_ratio_without_move() {
        let course = course(1.0, 0.0);
        assert_eq!(
            ratio(&course, 600.0, 600.0, CourseWidths(0.0), CourseWidths(0.1)),
            1.0
        );
    }

    #[test]
    fn test_ratio_on_corner_grows_with_lane() {
        let course = course(1.0, 0.0);
        let at_lane = |lane| {
            ratio(
                &course,
                600.0,
                601.5,
                CourseWidths(lane),
                CourseWidths(lane),
            )
        };
        assert_eq!(at_lane(0.0), 1.0);
        assert_close!(at_lane(1.0), 1.0 + CURVATURE * 11.25);
    }

    #[test]
    fn test_ratio_counts_only_turn_within_move() {
        // Half of the move is on the corner, which ends at 800m.
        let course = course(1.0, 0.0);
        let lane = CourseWidths(1.0);
        let expected = 1.0 + CURVATURE * 11.25 / 2.0;
        assert_close!(ratio(&course, 799.0, 801.0, lane, lane), expected);
    }

    #[test]
    fn test_ratio_is_relative_to_base_ratio() {
        let course = course(1.05, 0.0);
        let lane = CourseWidths(1.0);
        let expected = 1.0 + CURVATURE * 11.25 / 1.05;
        assert_close!(ratio(&course, 600.0, 601.5, lane, lane), expected);
    }

    #[test]
    fn test_ratio_with_rail_excess() {
        // 1mm more per 2m segment on the corner, or 1mm less.
        let lane = CourseWidths(0.0);
        let longer = course(1.05, 0.001);
        let expected = 1.0 + 0.0005 / 1.05;
        assert_close!(ratio(&longer, 600.0, 601.5, lane, lane), expected);
        let shorter = course(1.05, -0.001);
        assert_eq!(ratio(&shorter, 600.0, 601.5, lane, lane), 1.0);
        // Running wide makes up for a shorter rail.
        let lane = CourseWidths(1.0);
        let expected = 1.0 + (CURVATURE * 11.25 - 0.0005) / 1.05;
        assert_close!(ratio(&shorter, 600.0, 601.5, lane, lane), expected);
    }

    #[test]
    fn test_ratio_with_lane_change() {
        // 3m forward and 0.4 course widths (4.5m) sideways.
        let course = course(1.0, 0.0);
        let expected = 3f64.hypot(4.5) / 3.0;
        let ratio = |start, end| {
            ratio(
                &course,
                100.0,
                103.0,
                CourseWidths(start),
                CourseWidths(end),
            )
        };
        assert_close!(ratio(0.2, 0.6), expected);
        // Moving inward costs the same.
        assert_close!(ratio(0.6, 0.2), expected);
    }

    #[test]
    fn test_ratio_on_corner_uses_mid_lane() {
        let course = course(1.05, 0.0);
        let forward = 2.0 * (1.0 + CURVATURE * CourseWidths(0.5).meters() / 1.05);
        let expected = forward.hypot(CourseWidths(0.2).meters() / 1.05) / 2.0;
        let ratio = ratio(&course, 600.0, 602.0, CourseWidths(0.4), CourseWidths(0.6));
        assert_close!(ratio, expected);
    }

    #[test]
    fn test_ratio_ignores_turns_away_from_inner_rail() {
        // The corner turns the other way, so running wide covers less.
        let course = course_with(1.0, 0.0, -CURVATURE * 2.0);
        let lane = CourseWidths(1.0);
        assert_eq!(ratio(&course, 600.0, 601.5, lane, lane), 1.0);
    }
}
