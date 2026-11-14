//! Rolling of the random variables of skill conditions.
//!
//! Random variables don't pick a single point: they pick a random **10m
//! segment** ([`SEGMENT_LENGTH`]) inside the area they describe (a phase, a
//! corner, the last straight...), and the skill can activate anywhere in it, as
//! long as its guards hold. [`RandomRolls::roll`] returns the segment starts
//! for each OR branch (usually one; up to 4 for `all_corner_random`), or `None`
//! if the roll failed (a `random_lot` miss, or no matching feature on the
//! course, e.g. `corner_random==4` without a 4th corner).
//!
//! # Shared Rolls
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
//! # From Segments to Triggers
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
#[derive(Debug, Default, Clone)]
pub(crate) struct RandomRolls<'a> {
    rolled: Vec<(&'a Comparison, Option<Vec<f64>>)>,
}

impl<'a> RandomRolls<'a> {
    /// Rolls the random variables of every OR branch of a condition.
    ///
    /// Returns one entry per branch (in the order of
    /// [`Condition::branches`]), holding the points where the branch should
    /// be checked, or `None` if a roll failed and the branch can never be
    /// satisfied.
    ///
    /// Each distinct comparison is rolled once per `self`: branches containing
    /// the same one (e.g. `phase_random==1` in
    /// `distance_type==3&phase_random==1 @ distance_type==4&phase_random==1`)
    /// share its points, or its failure.
    pub(crate) fn roll(
        &mut self,
        condition: &'a Condition,
        course: &Course,
        rng: &mut impl Rng,
    ) -> Vec<Option<Vec<f64>>> {
        condition
            .branches
            .iter()
            .map(|comps| self.roll_branch(comps, course, rng))
            .collect()
    }

    /// Rolls the random variables of an AND chain, reusing the results of
    /// comparisons already rolled by another branch.
    ///
    /// Returns the points where the branch should be checked, or `None` if a
    /// roll failed (e.g. a random lot, or no corners for `corner_random`).
    fn roll_branch(
        &mut self,
        comps: &'a [Comparison],
        course: &Course,
        rng: &mut impl Rng,
    ) -> Option<Vec<f64>> {
        let mut points = Vec::new();
        for comp in comps {
            let index = match self.rolled.iter().position(|(prev, _)| *prev == comp) {
                Some(index) => index,
                None => {
                    let result = process_random_comp(comp, course, rng);
                    self.rolled.push((comp, result));
                    self.rolled.len() - 1
                }
            };
            points.extend_from_slice(self.rolled[index].1.as_deref()?);
        }
        Some(points)
    }
}

/// Returns the areas of the random variables of every OR branch of a condition,
/// without rolling them.
///
/// The areas hold every segment they could roll, and so where the branch could
/// be true whatever the rolls. Random lots always hit.
///
/// Returns one entry per branch (in the order of [`Condition::branches`]),
/// holding the intervals `(start, end)` of its random variables, or `None` if
/// one of them has an empty area (e.g. `corner_random==4` without a 4th
/// corner).
pub(crate) fn random_areas(condition: &Condition, course: &Course) -> Vec<Option<Vec<(f64, f64)>>> {
    let branch_areas = |comps: &ComparisonList| {
        let mut intervals = Vec::new();
        for comp in comps {
            intervals.extend(random_area(comp, course)?.intervals());
        }
        Some(intervals)
    };
    condition.branches.iter().map(branch_areas).collect()
}

/// Processes a single comparison that might contain a random variable.
///
/// Returns `Some(points)` with the points where the skill should be checked
/// (empty for non-positional rolls), or `None` if the roll failed or the
/// condition is invalid.
fn process_random_comp(comp: &Comparison, course: &Course, rng: &mut impl Rng) -> Option<Vec<f64>> {
    match &comp.var {
        // Pure probability roll.
        // N.B. Both are only used by out-of-gate skills, which are evaluated before
        // the race instead (see `UnreadyRunner::apply_out_of_gate_skills`). Here,
        // they're shared by comparison, like any other random variable.
        VarType::Random(RandomVar::RandomLot | RandomVar::RandomLotShared) => {
            // "random_lot==X" means X% chance of activation.
            let chance = comp.value as f64 / 100.0;
            // Success has no activation point.
            rng.random_bool(chance).then(Vec::new)
        }
        _ => match random_area(comp, course)? {
            Area::Anywhere => Some(vec![]),
            Area::Interval(start, end) => Some(vec![random_segment(start, end, rng)]),
            Area::Stitched(intervals) => {
                random_segment_in_intervals(&intervals, rng).map(|p| vec![p])
            }
            Area::OneOf(intervals) => intervals
                .choose(rng)
                .map(|&(start, end)| vec![random_segment(start, end, rng)]),
            Area::AllCorners(corners) => Some(random_all_corner_segments(corners, rng)),
        },
    }
}

/// Where the segments of a random variable can be, and how they're picked.
#[derive(Debug, Clone, PartialEq)]
enum Area {
    /// No position constraint: non-random variables, random lots, and
    /// `run_at_full_speed_random`, rolled when the all-out spurt starts since
    /// that can't be known in advance.
    Anywhere,
    /// A segment anywhere in `(start, end)`.
    Interval(f64, f64),
    /// A segment anywhere in the intervals, stitched together end to end (see
    /// [`random_segment_in_intervals`]).
    Stitched(Vec<(f64, f64)>),
    /// A segment in one of the intervals, all with the same chance regardless
    /// of their length.
    OneOf(Vec<(f64, f64)>),
    /// Up to 4 segments on the corners (see [`random_all_corner_segments`]).
    AllCorners(Vec<(f64, f64)>),
}

impl Area {
    /// Returns the intervals the segments can be in. Empty means no constraint.
    fn intervals(self) -> Vec<(f64, f64)> {
        match self {
            Self::Anywhere => vec![],
            Self::Interval(start, end) => vec![(start, end)],
            Self::Stitched(intervals) | Self::OneOf(intervals) | Self::AllCorners(intervals) => {
                intervals
            }
        }
    }
}

/// Returns the area of a comparison that might contain a random variable.
///
/// Returns `None` if the area is empty (e.g. `corner_random==4` without a 4th
/// corner) or the condition is invalid.
fn random_area(comp: &Comparison, course: &Course) -> Option<Area> {
    use VarType::*;

    // Parts of `features` (as `(start, end)`) within `(start, end)`, or `None` if
    // there are none.
    let within = |features: Vec<(f64, f64)>, (start, end)| {
        let parts: Vec<_> = features
            .into_iter()
            .filter_map(|feature| intersect_intervals(feature, (start, end)))
            .collect();
        (!parts.is_empty()).then_some(parts)
    };
    let phase = || CoursePhase::try_from(comp.value as i8).ok();
    let corners = || {
        course
            .corners
            .iter()
            .map(Segment::range)
            .collect::<Vec<_>>()
    };
    let straights = || course.straights.iter().map(|s| (s.start, s.end)).collect();
    let slopes = |uphill: bool| {
        course
            .slopes
            .iter()
            .filter(|slope| {
                if uphill {
                    slope.is_uphill()
                } else {
                    slope.is_downhill()
                }
            })
            .map(|slope| (slope.start, slope.end()))
            .collect::<Vec<_>>()
    };
    let non_empty = |intervals: Vec<(f64, f64)>| (!intervals.is_empty()).then_some(intervals);

    let area = match &comp.var {
        // Random point during a phase (0=Early, 1=Mid, 2=Late, 3=Last).
        RandomPhase(RandomPhaseVar::PhaseRandom) => {
            let (start, end) = phase()?.boundaries(course);
            Area::Interval(start, end)
        }
        RandomPhase(RandomPhaseVar::PhaseFirsthalfRandom) => {
            let (start, end) = phase()?.boundaries_first_half(course);
            Area::Interval(start, end)
        }
        RandomPhase(RandomPhaseVar::PhaseFirstquarterRandom) => {
            let (start, end) = phase()?.boundaries_first_quarter(course);
            Area::Interval(start, end)
        }
        RandomPhase(RandomPhaseVar::PhaseLaterhalfRandom) => {
            let (start, end) = phase()?.boundaries_second_half(course);
            Area::Interval(start, end)
        }
        // Random point on the corners or straights within a phase, or part of it.
        RandomPhase(RandomPhaseVar::PhaseCornerRandom) => {
            Area::Stitched(within(corners(), phase()?.boundaries(course))?)
        }
        RandomPhase(RandomPhaseVar::PhaseStraightRandom) => {
            Area::Stitched(within(straights(), phase()?.boundaries(course))?)
        }
        RandomPhase(RandomPhaseVar::PhaseFirstHalfStraightRandom) => {
            Area::Stitched(within(straights(), phase()?.boundaries_first_half(course))?)
        }
        RandomPhase(RandomPhaseVar::PhaseLatterHalfStraightRandom) => Area::Stitched(within(
            straights(),
            phase()?.boundaries_second_half(course),
        )?),

        RandomSpatial(RandomSpatialVar::IsFinalcornerRandom) => {
            let last_corner = course.corners.last()?;
            Area::Interval(last_corner.start, last_corner.end())
        }
        RandomSpatial(RandomSpatialVar::CornerRandom) => {
            // "corner_random==X" where X is corner number (1-4).
            // If the race goes through corner X multiple times, only the last occurrence
            // counts.
            let corner_idx = (0..course.corners.len())
                .rev()
                .find(|&idx| course.corner_number(idx) as i32 == comp.value)?;
            let corner = &course.corners[corner_idx];
            Area::Interval(corner.start, corner.end())
        }
        RandomSpatial(RandomSpatialVar::AllCornerRandom) => Area::AllCorners(non_empty(corners())?),
        RandomSpatial(RandomSpatialVar::StraightRandom) => Area::OneOf(non_empty(straights())?),
        RandomSpatial(RandomSpatialVar::LastStraightRandom) => {
            let last_straight = course.straights.last()?;
            Area::Interval(last_straight.start, last_straight.end)
        }
        RandomSpatial(RandomSpatialVar::UpSlopeRandom) => Area::OneOf(non_empty(slopes(true))?),
        RandomSpatial(RandomSpatialVar::UpSlopeRandomLaterHalf) => {
            // Uphills crossing the halfway point only count from there onwards.
            Area::OneOf(within(slopes(true), (course.halfway(), course.length()))?)
        }
        RandomSpatial(RandomSpatialVar::DownSlopeRandom) => Area::OneOf(non_empty(slopes(false))?),
        RandomSpatial(RandomSpatialVar::DistanceRateAfterRandom) => {
            // "distance_rate_after_random==X" where X is percentage.
            Area::Interval(course.position_at_rate(comp.value as f64), course.length())
        }

        _ => Area::Anywhere,
    };
    Some(area)
}

/// Calculates the intersection of two intervals `a` and `b`, each given as
/// `(start, end)`.
///
/// Returns `Some((start, end))` if there's an intersection, `None` otherwise.
fn intersect_intervals(a: (f64, f64), b: (f64, f64)) -> Option<(f64, f64)> {
    let intersection_start = a.0.max(b.0);
    let intersection_end = a.1.min(b.1);

    (intersection_start < intersection_end).then_some((intersection_start, intersection_end))
}

/// Length in meters of the segments picked by random conditions.
pub(crate) const SEGMENT_LENGTH: f64 = 10.0;

/// Picks a random 10m segment within `[start, end]` and returns its start.
///
/// The start is uniform in `[start, end - 10]`, so the segment always fits.
/// Intervals shorter than 10m only have one segment, starting at `start`.
///
/// The skill can activate anywhere in the segment, as long as the other
/// conditions are met.
pub(crate) fn random_segment(start: f64, end: f64, rng: &mut impl Rng) -> f64 {
    let latest_start = end - SEGMENT_LENGTH;
    if latest_start <= start {
        start
    } else {
        rng.random_range(start..=latest_start)
    }
}

/// Picks a random 10m segment from a collection of intervals stitched together
/// end to end, and return its start (see [`random_segment`]).
///
/// Interval distances must be positive (i.e., `start < end`), otherwise the
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

/// Picks the segments of `all_corner_random`, in race order.
///
/// Returns up to 4 points, from `corners` (as `(start, end)`, in race order).
///
/// 1. All `corners` start as candidates.
/// 2. Pick a candidate, all with the same chance regardless of their length.
/// 3. Pick a random 10m segment on it (see [`random_segment`]).
/// 4. If at least 10m remain from the end of the segment to the end of the
///    candidate, the candidate is replaced by that remaining part. Otherwise,
///    it's discarded.
/// 5. Discard all candidates before the picked one.
/// 6. Repeat from 2 until 4 segments are picked or no candidates remain.
fn random_all_corner_segments(corners: Vec<(f64, f64)>, rng: &mut impl Rng) -> Vec<f64> {
    const MAX_SEGMENTS: usize = 4;

    let mut candidates = corners;
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
