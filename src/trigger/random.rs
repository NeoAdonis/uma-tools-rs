//! # Random Segments
//!
//! This module rolls the random variables of skill conditions when the
//! scheduler is built.
//!
//! Random variables don't pick a single point: they pick a random **10m
//! segment** ([`SEGMENT_LENGTH`]) inside the area they describe (a phase, a
//! corner, the last straight...), and the skill can activate anywhere in it, as
//! long as its guards hold. [`roll_condition`] returns the segment starts for
//! each OR branch (usually one; up to 4 for `all_corner_random`), or `None` if
//! the roll failed (a `random_lot` miss, or no matching feature on the course,
//! e.g. `corner_random==4` without a 4th corner).
//!
//! ## Shared Rolls
//!
//! Each distinct random comparison (same variable, operator and value) is
//! rolled **once per skill** ([`RandomRolls`]), and every branch containing it
//! shares the result: the same segments, or the same failure. For example, in
//! `distance_type==3&phase_random==1 @ distance_type==4&phase_random==1`, both
//! branches get the same segment, and `random_lot==50` in two branches hits or
//! misses for both at once.
//! 
//! Within a branch, rolling stops at the first failure; a comparison skipped
//! that way is rolled when another branch first needs it.
//!
//! This includes the branches of every alternative of the skill. Alternatives
//! often share a random variable, one of them adding requirements to the other
//! (e.g. `phase_straight_random==1&track_id==10005` vs
//! `phase_straight_random==1`). Rolled separately, the less specific one could
//! come first and prevent the other from activating, since a skill activates
//! through a single alternative; rolled once, the first one satisfied in order
//! wins at the shared segment.
//!
//! ## From Segments to Triggers
//!
//! When compiling (see [`skill_activation`](crate::trigger::skill_activation)),
//! each segment `[start, start + 10]` is intersected with the branch's position
//! range, and the non-empty parts become its trigger. If none remain, the
//! branch is dropped. Segments are position windows: the guards are tested
//! every frame while the runner is inside one, until they pass.
//!
//! `run_at_full_speed_random` can't be rolled in advance: its logics wait until
//! the all-out spurt starts, and then get a segment between the current
//! position and the goal. Unlike other random variables, it's rolled once per
//! branch; in practice, at most one branch per alternative is left by then,
//! since branches sharing a random variable are split by static conditions,
//! which are resolved when the runner is created.

use std::vec;

use crate::condition::parser::*;
use crate::condition::variables::*;
use crate::course::Course;
use crate::course::Phase as CoursePhase;
use crate::course::geometry::Segment;
use rand::prelude::*;

/// Results of the comparisons already rolled, so that they're rolled only once.
///
/// It should be shared by every condition of a skill: its alternatives often
/// share a random variable (e.g. `phase_laterhalf_random==1`, where one of them
/// adds requirements), and must get the same points, or the same failure.
#[derive(Default)]
pub struct RandomRolls<'a> {
    rolled: Vec<(&'a Comparison, Option<Vec<f64>>)>,
}

/// Roll the random variables of every OR branch of a condition.
///
/// Returns one entry per branch (in the order of
/// [`Condition::branches`]), holding the points where the branch should
/// be checked, or `None` if a roll failed and the branch can never be
/// satisfied.
///
/// Each distinct comparison is rolled once per `rolls`: branches containing the
/// same one (e.g. `phase_random==1` in
/// `distance_type==3&phase_random==1 @ distance_type==4&phase_random==1`) share
/// its points, or its failure.
pub fn roll_condition<'a>(
    condition: &'a Condition,
    course: &Course,
    rolls: &mut RandomRolls<'a>,
    rng: &mut impl Rng,
) -> Vec<Option<Vec<f64>>> {
    condition
        .branches
        .iter()
        .map(|comps| roll_branch(comps, course, rolls, rng))
        .collect()
}

/// Roll the random variables of an AND chain, reusing the results in `rolls`
/// for comparisons already rolled by another branch.
///
/// Returns the points where the branch should be checked, or `None` if a roll
/// failed (e.g. a random lot, or no corners for `corner_random`).
fn roll_branch<'a>(
    comps: &'a [Comparison],
    course: &Course,
    rolls: &mut RandomRolls<'a>,
    rng: &mut impl Rng,
) -> Option<Vec<f64>> {
    let mut points = Vec::new();
    for comp in comps {
        let result = match rolls.rolled.iter().find(|(prev, _)| *prev == comp) {
            Some((_, result)) => result.clone(),
            None => {
                let result = process_random_comp(comp, course, rng);
                rolls.rolled.push((comp, result.clone()));
                result
            }
        };
        points.extend(result?);
    }
    Some(points)
}

/// Process a single comparison that might contain a random variable.
///
/// Returns `Some(points)` with the points where the skill should be checked
/// (empty for non-positional rolls), or `None` if the roll failed or the
/// condition is invalid.
fn process_random_comp(comp: &Comparison, course: &Course, rng: &mut impl Rng) -> Option<Vec<f64>> {
    use VarType::*;

    match &comp.var {
        // Pure probability roll.
        // N.B. Both are only used by out-of-gate skills, which are evaluated before
        // the race instead (see `UnreadyRunner::apply_out_of_gate_skills`). Here,
        // they're shared by comparison, like any other random variable.
        Random(RandomVar::RandomLot | RandomVar::RandomLotShared) => {
            // "random_lot==X" means X% chance of activation.
            let chance = comp.value as f64 / 100.0;
            if rng.random_bool(chance) {
                // Success, no activation point.
                Some(vec![])
            } else {
                // Failed roll.
                None
            }
        }

        // Random point during a phase.
        RandomPhase(RandomPhaseVar::PhaseRandom) => {
            // "phase_random==X" where X is phase (0=Early, 1=Mid, 2=Late, 3=Last).
            let (start, end) = CoursePhase::try_from(comp.value as u8)
                .ok()?
                .boundaries(course);

            Some(vec![random_segment(start, end, rng)])
        }

        RandomPhase(RandomPhaseVar::PhaseFirsthalfRandom) => {
            // "phase_firsthalf_random==X" where X is phase.
            // Activates at random point during first half of specified phase.
            let (start, end) = CoursePhase::try_from(comp.value as u8)
                .ok()?
                .boundaries_first_half(course);

            Some(vec![random_segment(start, end, rng)])
        }

        RandomPhase(RandomPhaseVar::PhaseFirstquarterRandom) => {
            // "phase_firstquarter_random==X" where X is phase.
            // Activates at random point during first quarter of specified phase.
            let (start, end) = CoursePhase::try_from(comp.value as u8)
                .ok()?
                .boundaries_first_quarter(course);

            Some(vec![random_segment(start, end, rng)])
        }

        RandomPhase(RandomPhaseVar::PhaseLaterhalfRandom) => {
            // "phase_laterhalf_random==X" where X is phase.
            // Activates at random point during second half of specified phase.
            let (start, end) = CoursePhase::try_from(comp.value as u8)
                .ok()?
                .boundaries_second_half(course);

            Some(vec![random_segment(start, end, rng)])
        }

        RandomPhase(RandomPhaseVar::PhaseCornerRandom) => {
            // "phase_corner_random==X" where X is phase.
            // Picks random point during a corner in specified phase.
            let (start, end) = CoursePhase::try_from(comp.value as u8)
                .ok()?
                .boundaries(course);

            // Find all corner segments that intersect with this phase.
            let corner_intervals: Vec<(f64, f64)> = course
                .corners
                .iter()
                .filter_map(|corner| intersect_intervals(corner.start, corner.end(), start, end))
                .collect();

            // No corners in this phase fails the roll.
            random_segment_in_intervals(&corner_intervals, rng)
                .map(|activation_distance| vec![activation_distance])
        }

        RandomPhase(RandomPhaseVar::PhaseStraightRandom) => {
            // "phase_straight_random==X" where X is phase.
            // Picks random point during a straight in specified phase.
            let (start, end) = CoursePhase::try_from(comp.value as u8)
                .ok()?
                .boundaries(course);

            // Find all straight segments that intersect with this phase.
            let straight_intervals: Vec<(f64, f64)> = course
                .straights
                .iter()
                .filter_map(|straight| {
                    intersect_intervals(straight.start, straight.end, start, end)
                })
                .collect();

            // No straights in this phase fails the roll.
            random_segment_in_intervals(&straight_intervals, rng)
                .map(|activation_distance| vec![activation_distance])
        }

        RandomPhase(RandomPhaseVar::PhaseFirstHalfStraightRandom) => {
            // "phase_first_half_straight_random==X" where X is phase.
            // Picks random point during a straight in first half of set phase.
            let (start, end) = CoursePhase::try_from(comp.value as u8)
                .ok()?
                .boundaries_first_half(course);

            // Find all straight segments that intersect with the first half of the phase.
            let straight_intervals: Vec<(f64, f64)> = course
                .straights
                .iter()
                .filter_map(|straight| {
                    intersect_intervals(straight.start, straight.end, start, end)
                })
                .collect();

            // No straights found fails the roll.
            random_segment_in_intervals(&straight_intervals, rng)
                .map(|activation_distance| vec![activation_distance])
        }

        RandomPhase(RandomPhaseVar::PhaseLatterHalfStraightRandom) => {
            // "phase_latter_half_straight_random==X" where X is phase.
            // Picks random point during a straight in second half of set phase.
            let (start, end) = CoursePhase::try_from(comp.value as u8)
                .ok()?
                .boundaries_second_half(course);

            // Find all straight segments that intersect with the second half of the phase.
            let straight_intervals: Vec<(f64, f64)> = course
                .straights
                .iter()
                .filter_map(|straight| {
                    intersect_intervals(straight.start, straight.end, start, end)
                })
                .collect();

            // No straights found fails the roll.
            random_segment_in_intervals(&straight_intervals, rng)
                .map(|activation_distance| vec![activation_distance])
        }

        RandomSpatial(RandomSpatialVar::RunAtFullSpeedRandom) => {
            // "run_at_full_speed_random==1".
            // Rolled when the all-out spurt starts, since that can't be known in advance.
            Some(vec![])
        }

        RandomSpatial(RandomSpatialVar::IsFinalcornerRandom) => {
            // "is_finalcorner_random==1".
            // Picks random segment on final corner.
            // No corners on course fails the roll.
            course
                .corners
                .last()
                .map(|last_corner| vec![random_segment(last_corner.start, last_corner.end(), rng)])
        }

        RandomSpatial(RandomSpatialVar::CornerRandom) => {
            // "corner_random==X" where X is corner number (1-4).
            // If the race goes through corner X multiple times, only the last occurrence
            // counts.
            // Invalid corner number or corner not on course fails the roll.
            let corner_idx = (0..course.corners.len())
                .rev()
                .find(|&idx| course.corner_number(idx) as i32 == comp.value)?;
            let corner = &course.corners[corner_idx];
            Some(vec![random_segment(corner.start, corner.end(), rng)])
        }

        RandomSpatial(RandomSpatialVar::AllCornerRandom) => {
            // "all_corner_random==1".
            // Picks up to 4 random segments on corners (see `random_all_corner_segments`).
            // No corners on course fails the roll.
            let points = random_all_corner_segments(course, rng);
            (!points.is_empty()).then_some(points)
        }

        RandomSpatial(RandomSpatialVar::StraightRandom) => {
            // "straight_random==1".
            // Picks random straight, then random segment on it.
            // All straights have the same chance to be picked regardless of length.
            // No straights on course fails the roll (should be rare/unlikely).
            course
                .straights
                .choose(rng)
                .map(|straight| vec![random_segment(straight.start, straight.end, rng)])
        }

        RandomSpatial(RandomSpatialVar::LastStraightRandom) => {
            // "last_straight_random==1".
            // Picks random segment on final straight.
            // No straights on course fails the roll (should be rare/unlikely).
            course.straights.last().map(|last_straight| {
                vec![random_segment(last_straight.start, last_straight.end, rng)]
            })
        }

        RandomSpatial(RandomSpatialVar::UpSlopeRandom) => {
            // "up_slope_random==1".
            // Picks random segment on any uphill.
            let uphills: Vec<_> = course.slopes.iter().filter(|s| s.is_uphill()).collect();

            // No uphills on course fails the roll.
            uphills
                .choose(rng)
                .map(|slope| vec![random_segment(slope.start, slope.end(), rng)])
        }

        RandomSpatial(RandomSpatialVar::UpSlopeRandomLaterHalf) => {
            // "up_slope_random_later_half==1".
            // Picks random segment on any uphill in second half of race.
            // Uphills crossing the halfway point only count from there onwards.
            let halfway = course.halfway();
            let late_uphills: Vec<_> = course
                .slopes
                .iter()
                .filter(|s| s.is_uphill())
                .filter_map(|s| {
                    intersect_intervals(s.start, s.end(), halfway, course.distance as f64)
                })
                .collect();

            // No uphills after halfway on course fails the roll.
            late_uphills
                .choose(rng)
                .map(|&(start, end)| vec![random_segment(start, end, rng)])
        }

        RandomSpatial(RandomSpatialVar::DownSlopeRandom) => {
            // "down_slope_random==1".
            // Picks random segment on any downhill.
            let downhills: Vec<_> = course.slopes.iter().filter(|s| s.is_downhill()).collect();

            // No downhills on course fails the roll.
            downhills
                .choose(rng)
                .map(|slope| vec![random_segment(slope.start, slope.end(), rng)])
        }

        RandomSpatial(RandomSpatialVar::DistanceRateAfterRandom) => {
            // "distance_rate_after_random==X" where X is percentage.
            // Picks random segment after X% of the race.
            let threshold_distance = (comp.value as f64 / 100.0) * course.distance as f64;
            let end_distance = course.distance as f64;
            Some(vec![random_segment(threshold_distance, end_distance, rng)])
        }

        _ => {
            // Not a random variable, skip.
            Some(vec![])
        }
    }
}

/// Calculate intersection of two intervals `[a_start, a_end]` and
/// `[b_start, b_end]`.
///
/// Returns `Some((start, end))` if there's an intersection, `None` otherwise.
fn intersect_intervals(a_start: f64, a_end: f64, b_start: f64, b_end: f64) -> Option<(f64, f64)> {
    let intersection_start = a_start.max(b_start);
    let intersection_end = a_end.min(b_end);

    (intersection_start < intersection_end).then_some((intersection_start, intersection_end))
}

/// Length in meters of the segments picked by random conditions.
pub const SEGMENT_LENGTH: f64 = 10.0;

/// Pick a random 10m segment within `[start, end]` and return its start. The
/// start is uniform in `[start, end - 10]`, so the segment always fits.
/// Intervals shorter than 10m only have one segment, starting at `start`.
///
/// The skill can activate anywhere in the segment, as long as the other
/// conditions are met.
pub fn random_segment(start: f64, end: f64, rng: &mut impl Rng) -> f64 {
    let latest_start = end - SEGMENT_LENGTH;
    if latest_start <= start {
        start
    } else {
        rng.random_range(start..=latest_start)
    }
}

/// Pick a random 10m segment from a collection of intervals stitched together
/// end to end, and return its start (see [`random_segment`]).
///
/// Intervals distances must be positive (i.e., `start < end`), otherwise the
/// behavior is undefined. Returns `None` if the intervals are empty.
fn random_segment_in_intervals(intervals: &[(f64, f64)], rng: &mut impl Rng) -> Option<f64> {
    #[cfg(debug_assertions)]
    {
        for &(start, end) in intervals {
            assert!(start < end, "Interval start must be less than end");
        }
    }

    let (last, rest) = intervals.split_last()?;
    let total_length: f64 = intervals.iter().map(|(start, end)| end - start).sum();
    let mut offset = random_segment(0.0, total_length, rng);

    // Find which interval contains this offset.
    for &(start, end) in rest {
        if offset < end - start {
            return Some(start + offset);
        }
        offset -= end - start;
    }
    Some(last.0 + offset)
}

/// Pick the segments of `all_corner_random`, in race order.
///
/// Returns up to 4 points, or none if the course has no corners.
///
/// 1. All corners start as candidates.
/// 2. Pick a candidate, all with the same chance regardless of their length.
/// 3. Pick a random 10m segment on it (see [`random_segment`]).
/// 4. If at least 10m remain from the end of the segment to the end of the
///    candidate, the candidate is replaced by that remaining part. Otherwise,
///    it's discarded.
/// 5. Discard all candidates before the picked one.
/// 6. Repeat from 2 until 4 segments are picked or no candidates remain.
fn random_all_corner_segments(course: &Course, rng: &mut impl Rng) -> Vec<f64> {
    const MAX_SEGMENTS: usize = 4;

    let mut candidates: Vec<(f64, f64)> = course.corners.iter().map(Segment::range).collect();
    let mut points = Vec::with_capacity(MAX_SEGMENTS);

    while points.len() < MAX_SEGMENTS && !candidates.is_empty() {
        let idx = rng.random_range(0..candidates.len());
        let (start, end) = candidates[idx];
        let point = random_segment(start, end, rng);
        points.push(point);

        // The picked candidate is now the first one.
        candidates.drain(..idx);
        let remaining_start = point + SEGMENT_LENGTH;
        if end - remaining_start >= SEGMENT_LENGTH {
            candidates[0] = (remaining_start, end);
        } else {
            candidates.remove(0);
        }
    }

    points
}

#[cfg(test)]
mod tests;
