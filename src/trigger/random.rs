use std::vec;

use crate::condition::parser::*;
use crate::condition::variables::*;
use crate::course::Course;
use crate::course::Phase as CoursePhase;
use rand::prelude::*;

/// Roll the random variables of every OR branch of a condition.
///
/// Returns one entry per branch (in the order of
/// [`ComparisonTree::or_branches`]), holding the points where the branch should
/// be checked, or `None` if a roll failed and the branch can never be
/// satisfied.
///
/// Each distinct comparison is rolled once per condition: branches containing
/// the same one (e.g. `phase_random==1` in
/// `distance_type==3&phase_random==1 @ distance_type==4&phase_random==1`) share
/// its points, or its failure.
pub fn roll_condition(
    condition: &Condition,
    course: &Course,
    rng: &mut impl Rng,
) -> Vec<Option<Vec<f64>>> {
    let mut rolled = Vec::new();
    condition
        .comp_tree
        .or_branches()
        .iter()
        .map(|comps| roll_branch(comps, course, &mut rolled, rng))
        .collect()
}

/// Results of the comparisons already rolled for a condition.
type RolledComps<'a> = Vec<(&'a Comparison, Option<Vec<f64>>)>;

/// Roll the random variables of an AND chain, reusing the results in `rolled`
/// for comparisons already rolled by another branch.
///
/// Returns the points where the branch should be checked, or `None` if a roll
/// failed (e.g. a random lot, or no corners for `corner_random`).
fn roll_branch<'a>(
    comps: &[&'a Comparison],
    course: &Course,
    rolled: &mut RolledComps<'a>,
    rng: &mut impl Rng,
) -> Option<Vec<f64>> {
    let mut points = Vec::new();
    for &comp in comps {
        let result = match rolled.iter().find(|(prev, _)| *prev == comp) {
            Some((_, result)) => result.clone(),
            None => {
                let result = process_random_comp(comp, course, rng);
                rolled.push((comp, result.clone()));
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
        Random(RandomVar::RandomLot) => {
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
                .filter_map(|corner| {
                    let corner_start = corner.start;
                    let corner_end = corner.start + corner.length;
                    intersect_intervals(corner_start, corner_end, start, end)
                })
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
            // Rolled when Full-Throttle starts, since that can't be known in advance.
            Some(vec![])
        }

        RandomSpatial(RandomSpatialVar::IsFinalcornerRandom) => {
            // "is_finalcorner_random==1".
            // Picks random segment on final corner.
            // No corners on course fails the roll.
            course.corners.last().map(|last_corner| {
                vec![random_segment(
                    last_corner.start,
                    last_corner.start + last_corner.length,
                    rng,
                )]
            })
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
            Some(vec![random_segment(
                corner.start,
                corner.start + corner.length,
                rng,
            )])
        }

        RandomSpatial(RandomSpatialVar::AllCornerRandom) => {
            // "all_corner_random==1".
            // Picks up to 4 random segments on corners (see `random_all_corner_segments`).
            // TODO: The skill can activate in each of them if it's off cooldown, but
            // cooldowns aren't implemented yet, so it only activates in the first one where
            // the other conditions are met.
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
                .map(|slope| vec![random_segment(slope.start, slope.start + slope.length, rng)])
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
                    intersect_intervals(
                        s.start,
                        s.start + s.length,
                        halfway,
                        course.distance as f64,
                    )
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
                .map(|slope| vec![random_segment(slope.start, slope.start + slope.length, rng)])
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

    let mut candidates: Vec<(f64, f64)> = course
        .corners
        .iter()
        .map(|corner| (corner.start, corner.start + corner.length))
        .collect();
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
