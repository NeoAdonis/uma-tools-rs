use std::vec;

use crate::condition::parser::*;
use crate::condition::variables::*;
use crate::course::Course;
use crate::course::Phase as CoursePhase;
use rand::prelude::*;

/// Evaluate all random variables in a condition.
pub fn roll_condition(
    condition: &Condition,
    course: &Course,
    rng: &mut impl Rng,
) -> Option<Vec<f64>> {
    visit_comps(&condition.comp_tree, course, rng)
}

/// Visit all comparisons in a list and process random variables.
///
/// Returns `Some(activation_distances)` indicating where the skill should activate,
/// or `None` if a random lot roll failed or a condition within the list is invalid.
fn visit_comps(
    comp_tree: &ComparisonTree,
    course: &Course,
    rng: &mut impl Rng,
) -> Option<Vec<f64>> {
    match comp_tree {
        ComparisonTree::Single(comp) => process_random_comp(comp, course, rng),
        ComparisonTree::Binary { left, op, right } => {
            match op {
                BinOp::And => {
                    // All comparisons must succeed
                    let mut left_vals = visit_comps(left, course, rng)?;
                    let right_vals = visit_comps(right, course, rng)?;
                    left_vals.extend(right_vals);
                    Some(left_vals)
                }
                BinOp::Or => {
                    // Will fail only if both comparisons fail
                    match visit_comps(left, course, rng) {
                        Some(mut left_vals) => match visit_comps(right, course, rng) {
                            Some(right_vals) => {
                                left_vals.extend(right_vals);
                                Some(left_vals)
                            }
                            None => Some(left_vals),
                        },
                        None => visit_comps(right, course, rng),
                    }
                }
            }
        }
    }
}

/// Process a single comparison that might contain a random variable.
///
/// Returns `Some(activation_distances)` indicating where the skill should activate,
/// or `None` if a random lot roll failed or a condition is invalid.
pub fn process_random_comp(
    comp: &Comparison,
    course: &Course,
    rng: &mut impl Rng,
) -> Option<Vec<f64>> {
    use VarType::*;

    match &comp.var {
        // Pure probability roll
        Random(RandomVar::RandomLot) => {
            // random_lot==X means X% chance of activation
            let chance = comp.value as f64 / 100.0;
            if rng.random_bool(chance) {
                Some(vec![0.0]) // Success, activation distance is 0 (immediate)
            } else {
                None // Failed roll
            }
        }

        // Random point during a phase
        RandomPhase(RandomPhaseVar::PhaseRandom) => {
            // phase_random==X where X is phase (0=Early, 1=Mid, 2=Late, 3=Last)
            let (start, end) = CoursePhase::try_from(comp.value as u8)
                .ok()?
                .boundaries(course);

            Some(vec![rng.random_range(start..end)])
        }

        RandomPhase(RandomPhaseVar::PhaseFirsthalfRandom) => {
            // phase_firsthalf_random==X where X is phase
            // Activates at random point during first half of specified phase
            let (start, end) = CoursePhase::try_from(comp.value as u8)
                .ok()?
                .boundaries_first_half(course);

            Some(vec![rng.random_range(start..end)])
        }

        RandomPhase(RandomPhaseVar::PhaseFirstquarterRandom) => {
            // phase_firstquarter_random==X where X is phase
            // Activates at random point during first quarter of specified phase
            let (start, end) = CoursePhase::try_from(comp.value as u8)
                .ok()?
                .boundaries_first_quarter(course);

            Some(vec![rng.random_range(start..end)])
        }

        RandomPhase(RandomPhaseVar::PhaseLaterhalfRandom) => {
            // phase_laterhalf_random==X where X is phase
            // Activates at random point during second half of specified phase
            let (start, end) = CoursePhase::try_from(comp.value as u8)
                .ok()?
                .boundaries_second_half(course);

            Some(vec![rng.random_range(start..end)])
        }

        RandomPhase(RandomPhaseVar::PhaseCornerRandom) => {
            // phase_corner_random==X where X is phase
            // Picks random point during a corner in specified phase
            let (start, end) = CoursePhase::try_from(comp.value as u8)
                .ok()?
                .boundaries(course);

            // Find all corner segments that intersect with this phase
            let corner_intervals: Vec<(f64, f64)> = course
                .corners
                .iter()
                .filter_map(|corner| {
                    let corner_start = corner.start;
                    let corner_end = corner.start + corner.length;
                    intersect_intervals(corner_start, corner_end, start, end)
                })
                .collect();

            // No corners in this phase fails the roll
            random_point_in_intervals(&corner_intervals, rng)
                .map(|activation_distance| vec![activation_distance])
        }

        RandomPhase(RandomPhaseVar::PhaseStraightRandom) => {
            // phase_straight_random==X where X is phase
            // Picks random point during a straight in specified phase
            let (start, end) = CoursePhase::try_from(comp.value as u8)
                .ok()?
                .boundaries(course);

            // Find all straight segments that intersect with this phase
            let straight_intervals: Vec<(f64, f64)> = course
                .straights
                .iter()
                .filter_map(|straight| {
                    intersect_intervals(straight.start, straight.end, start, end)
                })
                .collect();

            // No straights in this phase fails the roll
            random_point_in_intervals(&straight_intervals, rng)
                .map(|activation_distance| vec![activation_distance])
        }

        RandomPhase(RandomPhaseVar::PhaseFirstHalfStraightRandom) => {
            // phase_first_half_straight_random==X where X is phase
            // Picks random point during a straight in first half of specified phase
            let (start, end) = CoursePhase::try_from(comp.value as u8)
                .ok()?
                .boundaries_first_half(course);

            // Find all straight segments that intersect with the first half of this phase
            let straight_intervals: Vec<(f64, f64)> = course
                .straights
                .iter()
                .filter_map(|straight| {
                    intersect_intervals(straight.start, straight.end, start, end)
                })
                .collect();

            // No straights found fails the roll
            random_point_in_intervals(&straight_intervals, rng)
                .map(|activation_distance| vec![activation_distance])
        }

        RandomPhase(RandomPhaseVar::PhaseLatterHalfStraightRandom) => {
            // phase_latter_half_straight_random==X where X is phase
            // Picks random point during a straight in second half of specified phase
            let (start, end) = CoursePhase::try_from(comp.value as u8)
                .ok()?
                .boundaries_second_half(course);

            // Find all straight segments that intersect with the second half of this phase
            let straight_intervals: Vec<(f64, f64)> = course
                .straights
                .iter()
                .filter_map(|straight| {
                    intersect_intervals(straight.start, straight.end, start, end)
                })
                .collect();

            // No straights found fails the roll
            random_point_in_intervals(&straight_intervals, rng)
                .map(|activation_distance| vec![activation_distance])
        }

        RandomSpatial(RandomSpatialVar::IsFinalcornerRandom) => {
            // is_finalcorner_random==1
            // Picks random point on final corner
            // No corners on course fails the roll
            course.corners.last().map(|last_corner| {
                let offset: f64 = rng.random();
                vec![last_corner.start + offset * last_corner.length]
            })
        }

        RandomSpatial(RandomSpatialVar::CornerRandom) => {
            // corner_random==X where X is corner number (1-indexed)
            let corner_idx = (comp.value - 1) as usize;
            // Invalid corner index fails the roll
            course.corners.get(corner_idx).map(|corner| {
                let offset: f64 = rng.random();
                vec![corner.start + offset * corner.length]
            })
        }

        RandomSpatial(RandomSpatialVar::AllCornerRandom) => {
            // all_corner_random==1
            // Picks 4 random points (roll random corner, then random point on it, 4 times)
            // TODO: extend to multiple activations
            // No corners on course fails the roll
            course.corners.choose(rng).map(|corner| {
                let offset: f64 = rng.random();
                vec![corner.start + offset * corner.length]
            })
        }

        RandomSpatial(RandomSpatialVar::StraightRandom) => {
            // straight_random==1
            // Picks random straight, then random point on it
            // No straights on course fails the roll (this should be rare/unlikely)
            course
                .straights
                .choose(rng)
                .map(|straight| vec![rng.random_range(straight.start..straight.end)])
        }

        RandomSpatial(RandomSpatialVar::LastStraightRandom) => {
            // last_straight_random==1
            // Picks random point on final straight
            // No straights on course fails the roll (this should be rare/unlikely)
            course
                .straights
                .last()
                .map(|last_straight| vec![rng.random_range(last_straight.start..last_straight.end)])
        }

        RandomSpatial(RandomSpatialVar::UpSlopeRandom) => {
            // up_slope_random==1
            // Picks random point on any uphill
            let uphills: Vec<_> = course.slopes.iter().filter(|s| s.is_uphill()).collect();

            // No uphills on course fails the roll
            uphills.choose(rng).map(|slope| {
                let offset: f64 = rng.random();
                vec![slope.start + offset * slope.length]
            })
        }

        RandomSpatial(RandomSpatialVar::UpSlopeRandomLaterHalf) => {
            // up_slope_random_later_half==1
            // Picks random point on any uphill in second half of race
            let section_len = course.section_length();
            let halfway = section_len * 12.0;

            let late_uphills: Vec<_> = course
                .slopes
                .iter()
                .filter(|s| s.is_uphill() && s.start >= halfway)
                .collect();

            // No uphills after halfway on course fails the roll
            late_uphills.choose(rng).map(|slope| {
                let offset: f64 = rng.random();
                vec![slope.start + offset * slope.length]
            })
        }

        RandomSpatial(RandomSpatialVar::DownSlopeRandom) => {
            // down_slope_random==1
            // Picks random point on any downhill
            let downhills: Vec<_> = course.slopes.iter().filter(|s| s.is_downhill()).collect();

            // No downhills on course fails the roll
            downhills.choose(rng).map(|slope| {
                let offset: f64 = rng.random();
                vec![slope.start + offset * slope.length]
            })
        }

        RandomSpatial(RandomSpatialVar::DistanceRateAfterRandom) => {
            // distance_rate_after_random==X where X is percentage
            // Picks random point after X% of the race
            let threshold_distance = (comp.value as f64 / 100.0) * course.distance as f64;
            let end_distance = course.distance as f64;
            let activation_distance = rng.random_range(threshold_distance..end_distance);
            Some(vec![activation_distance])
        }

        _ => {
            // Not a random variable, skip
            Some(vec![])
        }
    }
}

/// Calculate intersection of two intervals `[a_start, a_end]` and `[b_start, b_end]`.
/// Returns `Some((start, end))` if there's an intersection, `None` otherwise.
fn intersect_intervals(a_start: f64, a_end: f64, b_start: f64, b_end: f64) -> Option<(f64, f64)> {
    let intersection_start = a_start.max(b_start);
    let intersection_end = a_end.min(b_end);

    (intersection_start < intersection_end).then_some((intersection_start, intersection_end))
}

/// Pick a random point from a collection of intervals. Intervals distances must be
/// positive (i.e., `start < end`), otherwise the behavior is undefined.
/// Returns `None` if the intervals are empty.
fn random_point_in_intervals(intervals: &[(f64, f64)], rng: &mut impl Rng) -> Option<f64> {
    if intervals.is_empty() {
        return None;
    }

    #[cfg(debug_assertions)]
    {
        for &(start, end) in intervals {
            assert!(start < end, "Interval start must be less than end");
        }
    }

    // Calculate total length of all intervals
    let total_length: f64 = intervals.iter().map(|(start, end)| end - start).sum();

    // Pick a random offset within the total length
    let random_offset: f64 = rng.random_range(0.0..total_length);

    // Find which interval contains this offset
    let mut accumulated = 0.0;
    for &(interval_start, interval_end) in intervals {
        let interval_length = interval_end - interval_start;
        if accumulated + interval_length >= random_offset {
            let offset_in_interval = random_offset - accumulated;
            return Some(interval_start + offset_in_interval);
        }
        accumulated += interval_length;
    }

    unreachable!("random_offset < total_length, so some interval must contain it");
}
