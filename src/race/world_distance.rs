//! Extra world distance covered by running wide through corners or changing
//! lanes, which slows down progress along the course.
//!
//! Each frame, the game compares the world distance covered on the previous
//! move with the course distance the runner would cover on the next one at the
//! base ratio of the course (see [`Rail`]), and shortens the next move by the
//! quotient when it is below 1.
//!
//! The world distance of a move is the distance on the horizontal plane
//! between the runner's world positions at its start and end, each moved
//! outwards from the course's inner rail (lane 0) by the lane offset in
//! meters. Off the rail, a runner covers more world distance where the course
//! turns towards the inner rail, and less where it turns away from it. The
//! rail itself also covers slightly more or less world distance than the base
//! ratio in places, which changes the ratio even on the rail.
//!
//! A slowed down move covers the base world distance, so the move after it
//! isn't slowed down again, and at a steady speed the slowdown alternates
//! frame by frame. A runner losing speed is slowed down further, since the
//! next move is shorter than the previous one, while a runner gaining speed
//! (e.g. in the start dash) isn't sped up, because the quotient is capped at 1.

use crate::course::rail::Rail;

/// Returns the factor, at most 1, that slows down a move of `step` meters.
///
/// `step` is the uncorrected course distance of the move, which follows a move
/// from world position `start` to `end` on the horizontal plane. The factor is
/// the world distance that `step` covers at the base ratio of `rail`, over the
/// world distance of the previous move, so it takes a single division per
/// frame. It is 1 if the previous move covered no world distance.
pub(crate) fn inverse_ratio(rail: &Rail, start: [f64; 2], end: [f64; 2], step: f64) -> f64 {
    let (dx, dz) = (end[0] - start[0], end[1] - start[1]);
    // `hypot` is much slower and more precise than needed here.
    let world_distance = (dx * dx + dz * dz).sqrt();
    if world_distance <= 0.0 {
        return 1.0;
    }
    // Ratio can be less than 1 where the rail covers less than at the base ratio,
    // where the course turns the other way while off the rail, or where the
    // runner gains speed.
    (rail.base_ratio * step / world_distance).min(1.0)
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

    /// Inverse of [`inverse_ratio`] after a move along `course` from `start` to
    /// `end` (positions in meters), and from `start_lane` to `end_lane`, for a
    /// next move of `step` meters.
    fn ratio_before_step(
        course: &Course,
        start: f64,
        end: f64,
        start_lane: CourseWidths,
        end_lane: CourseWidths,
        step: f64,
    ) -> f64 {
        let rail = &course.rail;
        let start = rail.world_position(start, start_lane);
        1.0 / inverse_ratio(rail, start, rail.world_position(end, end_lane), step)
    }

    /// [`ratio_before_step`] with a next move as long as the move.
    fn ratio_of_move(
        course: &Course,
        start: f64,
        end: f64,
        start_lane: CourseWidths,
        end_lane: CourseWidths,
    ) -> f64 {
        ratio_before_step(course, start, end, start_lane, end_lane, end - start)
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
    fn test_ratio_without_move() {
        let course = course(1.0, 0.0);
        assert_eq!(
            ratio_of_move(&course, 600.0, 600.0, CourseWidths(0.1), CourseWidths(0.1)),
            1.0
        );
    }

    #[test]
    fn test_ratio_follows_speed_changes() {
        // Straight on the rail, after a 2m move.
        let course = course(1.0, 0.0);
        let lane = CourseWidths(0.0);
        let ratio = |step| ratio_before_step(&course, 100.0, 102.0, lane, lane, step);
        // Losing speed slows down, even where the course doesn't.
        assert_close!(ratio(1.9), 2.0 / 1.9);
        // Gaining speed doesn't speed up.
        assert_eq!(ratio(2.1), 1.0);
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
        // 1mm more per 2m segment on the corner.
        let course = course(1.05, 0.001);
        let lane = CourseWidths(0.0);
        let expected = 1.0 + 0.0005 / 1.05;
        assert_close!(ratio_of_move(&course, 600.0, 601.5, lane, lane), expected);
    }

    #[test]
    fn test_ratio_is_at_most_one() {
        let cases = [
            // 1mm less per 2m segment on the corner.
            ("shorter rail", course(1.05, -0.001), CourseWidths(0.0)),
            // Running wide where the corner turns away from the inner rail.
            (
                "turn away",
                course_with(1.0, 0.0, -CURVATURE * 2.0),
                CourseWidths(1.0),
            ),
        ];
        for (case, course, lane) in cases {
            let ratio = ratio_of_move(&course, 600.0, 601.5, lane, lane);
            assert_eq!(ratio, 1.0, "{case}");
        }
    }

    #[test]
    fn test_ratio_with_lane_change() {
        // 3m forward and 0.4 course widths (4.5m) sideways.
        let course = course(1.0, 0.0);
        let expected = 3f64.hypot(4.5) / 3.0;
        let ratio = ratio_of_move(&course, 100.0, 103.0, CourseWidths(0.2), CourseWidths(0.6));
        assert_close!(ratio, expected);
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
            let start = rail.world_position(0.0, CourseWidths(start));
            1.0 / inverse_ratio(
                &rail,
                start,
                rail.world_position(2.0, CourseWidths(end)),
                2.0,
            )
        };
        // Moving outwards goes backwards, and inwards forwards.
        let sideways = lateral * outward[1];
        let outwards = (2.0 + lateral * skew).hypot(sideways);
        assert_close!(ratio(0.4, 0.6), outwards / 2.0);
        let inwards = (2.0 - lateral * skew).hypot(sideways);
        assert_close!(ratio(0.6, 0.4), inwards / 2.0);
    }
}
