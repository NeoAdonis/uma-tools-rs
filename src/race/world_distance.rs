//! Extra world distance covered by running wide through corners or changing
//! lanes, which slows down progress along the course.
//!
//! Each frame, the game takes the ratio of world distance to course distance
//! covered on the last frame, and divides the course distance covered on the
//! next one by its ratio to the base ratio of the course when above 1 (see
//! [`Rail`]).
//!
//! The world distance of a move is the distance on the horizontal plane
//! between the runner's world positions at its start and end, each moved
//! outwards from the course's inner rail (lane 0) by the lane offset in
//! meters. Off the rail, a runner covers more world distance where the course
//! turns towards the inner rail, and less where it turns away from it. The
//! rail itself also covers slightly more or less world distance than the base
//! ratio in places, which changes the ratio even on the rail.

use crate::course::rail::Rail;

/// Course distance and world position at the start of a runner's last move.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct LastMove {
    /// Course distance covered in meters.
    pub(crate) distance: f64,
    /// World position on the horizontal plane at the start of the move, kept
    /// from the previous frame so that each frame looks up the rail only once.
    pub(crate) world: [f64; 2],
}

/// Inverse of the ratio of world distance to course distance of `last_move`,
/// relative to the base ratio of `rail`, and at most 1.
///
/// The move ends at world position `end` on the horizontal plane. Its inverse
/// takes a single division to slow down the next move, and is 1 if the move
/// covered no world distance.
pub(crate) fn inverse_ratio(rail: &Rail, last_move: &LastMove, end: [f64; 2]) -> f64 {
    let LastMove {
        distance,
        world: start,
    } = *last_move;
    if distance <= 0.0 {
        return 1.0;
    }
    let (dx, dz) = (end[0] - start[0], end[1] - start[1]);
    // `hypot` is much slower and more precise than needed here.
    let inverse = (distance * rail.base_ratio) / (dx * dx + dz * dz).sqrt();
    // Ratio can be less than 1 where the rail covers less than at the base ratio,
    // or where the course turns the other way while off the rail.
    inverse.min(1.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::course::Course;
    use crate::course::rail::{Keyframe, rail_from_turns};
    use crate::test_utils::{assert_close, synthetic_course};
    use crate::units::CourseWidths;

    /// Tolerance of ratios against the expected values of [`course`], which
    /// assume lanes turning evenly rather than between keyframes.
    const TOLERANCE: f64 = 1e-5;

    /// Ratio (the inverse of [`inverse_ratio`]) of a move along `course` from
    /// `start` to `end` (positions in meters), and from `start_lane` to
    /// `end_lane`.
    fn ratio_of_move(
        course: &Course,
        start: f64,
        end: f64,
        start_lane: CourseWidths,
        end_lane: CourseWidths,
    ) -> f64 {
        let rail = &course.rail;
        let last_move = LastMove {
            distance: end - start,
            world: rail.world_position(start, start_lane),
        };
        1.0 / inverse_ratio(rail, &last_move, rail.world_position(end, end_lane))
    }

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
            rail: rail_from_turns(base_ratio, 2000.0, &per_segment(excess), &per_segment(turn)),
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
        assert_close!(
            ratio_of_move(&course, 100.0, 101.5, CourseWidths(0.5), CourseWidths(0.5)),
            1.0
        );
    }

    #[test]
    fn test_ratio_without_move() {
        let course = course(1.0, 0.0);
        assert_eq!(
            ratio_of_move(&course, 600.0, 600.0, CourseWidths(0.0), CourseWidths(0.1)),
            1.0
        );
    }

    #[test]
    fn test_ratio_on_corner_grows_with_lane() {
        let course = course(1.0, 0.0);
        let at_lane = |lane| {
            ratio_of_move(
                &course,
                600.0,
                601.5,
                CourseWidths(lane),
                CourseWidths(lane),
            )
        };
        assert_eq!(at_lane(0.0), 1.0);
        assert_close!(at_lane(1.0), 1.0 + CURVATURE * 11.25, tolerance = TOLERANCE);
    }

    #[test]
    fn test_ratio_counts_only_turn_within_move() {
        // Half of the move is on the corner, which ends at 800m.
        let course = course(1.0, 0.0);
        let lane = CourseWidths(1.0);
        let expected = 1.0 + CURVATURE * 11.25 / 2.0;
        let ratio = ratio_of_move(&course, 799.0, 801.0, lane, lane);
        assert_close!(ratio, expected, tolerance = TOLERANCE);
    }

    #[test]
    fn test_ratio_is_relative_to_base_ratio() {
        let course = course(1.05, 0.0);
        let lane = CourseWidths(1.0);
        let expected = 1.0 + CURVATURE * 11.25 / 1.05;
        let ratio = ratio_of_move(&course, 600.0, 601.5, lane, lane);
        assert_close!(ratio, expected, tolerance = TOLERANCE);
    }

    #[test]
    fn test_ratio_with_rail_excess() {
        // 1mm more per 2m segment on the corner, or 1mm less.
        let lane = CourseWidths(0.0);
        let longer = course(1.05, 0.001);
        let expected = 1.0 + 0.0005 / 1.05;
        assert_close!(ratio_of_move(&longer, 600.0, 601.5, lane, lane), expected);
        let shorter = course(1.05, -0.001);
        assert_eq!(ratio_of_move(&shorter, 600.0, 601.5, lane, lane), 1.0);
        // Running wide makes up for a shorter rail.
        let lane = CourseWidths(1.0);
        let expected = 1.0 + (CURVATURE * 11.25 - 0.0005) / 1.05;
        let ratio = ratio_of_move(&shorter, 600.0, 601.5, lane, lane);
        assert_close!(ratio, expected, tolerance = TOLERANCE);
    }

    #[test]
    fn test_ratio_with_lane_change() {
        // 3m forward and 0.4 course widths (4.5m) sideways.
        let course = course(1.0, 0.0);
        let expected = 3f64.hypot(4.5) / 3.0;
        let ratio = |start, end| {
            ratio_of_move(
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
        let ratio = ratio_of_move(&course, 600.0, 602.0, CourseWidths(0.4), CourseWidths(0.6));
        assert_close!(ratio, expected, tolerance = TOLERANCE);
    }

    #[test]
    fn test_ratio_with_lane_change_on_skewed_lanes() {
        // Lanes tilted backwards by `skew` (sine of the angle) on a straight.
        let skew: f64 = -0.05;
        let outward = [skew, -(1.0 - skew * skew).sqrt()];
        let keyframe = |x| Keyframe {
            position: [x, 0.0],
            outward,
        };
        let rail = Rail::new(1.0, 2.0, vec![keyframe(0.0), keyframe(2.0)]);
        let lateral = CourseWidths(0.2).meters();
        let ratio = |start: f64, end: f64| {
            let last_move = LastMove {
                distance: 2.0,
                world: rail.world_position(0.0, CourseWidths(start)),
            };
            1.0 / inverse_ratio(
                &rail,
                &last_move,
                rail.world_position(2.0, CourseWidths(end)),
            )
        };
        // Moving outwards goes backwards, and inwards forwards.
        let sideways = lateral * outward[1];
        let outwards = (2.0 + lateral * skew).hypot(sideways);
        assert_close!(ratio(0.4, 0.6), outwards / 2.0);
        let inwards = (2.0 - lateral * skew).hypot(sideways);
        assert_close!(ratio(0.6, 0.4), inwards / 2.0);
    }

    #[test]
    fn test_ratio_ignores_turns_away_from_inner_rail() {
        // The corner turns the other way, so running wide covers less.
        let course = course_with(1.0, 0.0, -CURVATURE * 2.0);
        let lane = CourseWidths(1.0);
        assert_eq!(ratio_of_move(&course, 600.0, 601.5, lane, lane), 1.0);
    }
}
