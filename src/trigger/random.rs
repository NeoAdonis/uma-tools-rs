use std::vec;

use crate::condition::parser::*;
use crate::condition::variables::*;
use crate::course::Course;
use crate::course::Phase as CoursePhase;
use rand::prelude::*;

/// Evaluate all random variables in a condition
pub fn roll_condition(
    condition: &Condition,
    course: &Course,
    rng: &mut impl Rng,
) -> Option<Vec<f64>> {
    visit_comps(&condition.comp_tree, course, rng)
}

/// Visit all comparisons in a list and process random variables
///
/// Returns Some(activation_distances) indicating where the skill should activate,
/// or None if a random lot roll failed or a condition within the list is invalid
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

/// Process a single comparison that might contain a random variable
///
/// Returns Some(activation_distances) indicating where the skill should activate,
/// or None if a random lot roll failed or a condition is invalid
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
            let roll: f64 = rng.random();
            if roll * 100.0 < comp.value as f64 {
                Some(vec![0.0]) // Success, activation distance is 0 (immediate)
            } else {
                None // Failed roll
            }
        }

        // Random point during a phase
        RandomPhase(RandomPhaseVar::PhaseRandom) => {
            // phase_random==X where X is phase (0=Early, 1=Mid, 2=Late, 3=Last)
            let phase = match comp.value {
                0 => CoursePhase::Early,
                1 => CoursePhase::Mid,
                2 => CoursePhase::Late,
                3 => CoursePhase::Last,
                _ => return None,
            };

            let (start, end) = phase.boundaries(course);
            let activation_distance = rng.random_range(start..end);
            Some(vec![activation_distance])
        }

        RandomPhase(RandomPhaseVar::PhaseFirsthalfRandom) => {
            // phase_firsthalf_random==X where X is phase
            // Activates at random point during first half of specified phase
            let phase = match comp.value {
                0 => CoursePhase::Early,
                1 => CoursePhase::Mid,
                2 => CoursePhase::Late,
                3 => CoursePhase::Last,
                _ => return None,
            };

            let (start, end) = phase.boundaries(course);
            let midpoint = (start + end) / 2.0;
            let activation_distance = rng.random_range(start..midpoint);
            Some(vec![activation_distance])
        }

        RandomPhase(RandomPhaseVar::PhaseFirstquarterRandom) => {
            // phase_firstquarter_random==X where X is phase
            // Activates at random point during first quarter of specified phase
            let phase = match comp.value {
                0 => CoursePhase::Early,
                1 => CoursePhase::Mid,
                2 => CoursePhase::Late,
                3 => CoursePhase::Last,
                _ => return None,
            };

            let (start, end) = phase.boundaries(course);
            let quarter_point = start + (end - start) * 0.25;
            let activation_distance = rng.random_range(start..quarter_point);
            Some(vec![activation_distance])
        }

        RandomPhase(RandomPhaseVar::PhaseLaterhalfRandom) => {
            // phase_laterhalf_random==X where X is phase
            // Activates at random point during second half of specified phase
            let phase = match comp.value {
                0 => CoursePhase::Early,
                1 => CoursePhase::Mid,
                2 => CoursePhase::Late,
                3 => CoursePhase::Last,
                _ => return None,
            };

            let (start, end) = phase.boundaries(course);
            let midpoint = start + (end - start) * 0.25;
            let activation_distance = rng.random_range(midpoint..end);
            Some(vec![activation_distance])
        }

        RandomPhase(RandomPhaseVar::PhaseCornerRandom) => {
            // phase_corner_random==X where X is phase
            // Picks random point during a corner in specified phase
            let phase = match comp.value {
                0 => CoursePhase::Early,
                1 => CoursePhase::Mid,
                2 => CoursePhase::Late,
                3 => CoursePhase::Last,
                _ => return None,
            };

            let (phase_start, phase_end) = phase.boundaries(course);

            // Find all corner segments that intersect with this phase
            let corner_intervals: Vec<(f64, f64)> = course
                .corners
                .iter()
                .filter_map(|corner| {
                    let corner_start = corner.start;
                    let corner_end = corner.start + corner.length;
                    intersect_intervals(corner_start, corner_end, phase_start, phase_end)
                })
                .collect();

            if let Some(activation_distance) = random_point_in_intervals(&corner_intervals, rng) {
                Some(vec![activation_distance])
            } else {
                // No corners in this phase, fail the roll
                None
            }
        }

        RandomPhase(RandomPhaseVar::PhaseStraightRandom) => {
            // phase_straight_random==X where X is phase
            // Picks random point during a straight in specified phase
            let phase = match comp.value {
                0 => CoursePhase::Early,
                1 => CoursePhase::Mid,
                2 => CoursePhase::Late,
                3 => CoursePhase::Last,
                _ => return None,
            };

            let (phase_start, phase_end) = phase.boundaries(course);

            // Find all straight segments that intersect with this phase
            let straight_intervals: Vec<(f64, f64)> = course
                .straights
                .iter()
                .filter_map(|straight| {
                    intersect_intervals(straight.start, straight.end, phase_start, phase_end)
                })
                .collect();

            if let Some(activation_distance) = random_point_in_intervals(&straight_intervals, rng) {
                Some(vec![activation_distance])
            } else {
                // No straights in this phase, fail the roll
                None
            }
        }

        RandomPhase(RandomPhaseVar::PhaseFirstHalfStraightRandom) => {
            // TODO: Under investigation - straight in first half of race
            let section_len = course.section_length();
            let halfway = section_len * 12.0;

            // Filter straights that are in first half
            let first_half_straights: Vec<_> = course
                .straights
                .iter()
                .filter(|s| s.start < halfway)
                .collect();

            if !first_half_straights.is_empty() {
                let straight =
                    first_half_straights[rng.random_range(0..first_half_straights.len())];
                let end = straight.end.min(halfway);
                let activation_distance = rng.random_range(straight.start..end);
                Some(vec![activation_distance])
            } else {
                // No straights in this phase, fail the roll
                None
            }
        }

        RandomPhase(RandomPhaseVar::PhaseLatterHalfStraightRandom) => {
            // TODO: Under investigation - straight in second half of race
            let section_len = course.section_length();
            let halfway = section_len * 12.0;

            // Filter straights that are in second half
            let second_half_straights: Vec<_> = course
                .straights
                .iter()
                .filter(|s| s.end > halfway)
                .collect();

            if !second_half_straights.is_empty() {
                let straight =
                    second_half_straights[rng.random_range(0..second_half_straights.len())];
                let start = straight.start.max(halfway);
                let activation_distance = rng.random_range(start..straight.end);
                Some(vec![activation_distance])
            } else {
                // No straights in this phase, fail the roll
                None
            }
        }

        RandomSpatial(RandomSpatialVar::IsFinalcornerRandom) => {
            // is_finalcorner_random==1
            // Picks random point on final corner
            if let Some(last_corner) = course.corners.last() {
                let offset: f64 = rng.random();
                let activation_distance = last_corner.start + offset * last_corner.length;
                Some(vec![activation_distance])
            } else {
                // No corners on course, fail the roll
                None
            }
        }

        RandomSpatial(RandomSpatialVar::CornerRandom) => {
            // corner_random==X where X is corner number (1-indexed)
            let corner_idx = (comp.value - 1) as usize;
            if corner_idx < course.corners.len() {
                let corner = &course.corners[corner_idx];
                let offset: f64 = rng.random();
                let activation_distance = corner.start + offset * corner.length;
                Some(vec![activation_distance])
            } else {
                // Invalid corner index, fail the roll
                None
            }
        }

        RandomSpatial(RandomSpatialVar::AllCornerRandom) => {
            // all_corner_random==1
            // Picks 4 random points (roll random corner, then random point on it, 4 times)
            // TODO: extend to multiple activations
            if !course.corners.is_empty() {
                let corner_idx = rng.random_range(0..course.corners.len());
                let corner = &course.corners[corner_idx];
                let offset: f64 = rng.random();
                let activation_distance = corner.start + offset * corner.length;
                Some(vec![activation_distance])
            } else {
                // No corners on course, fail the roll
                None
            }
        }

        RandomSpatial(RandomSpatialVar::StraightRandom) => {
            // straight_random==1
            // Picks random straight, then random point on it
            if !course.straights.is_empty() {
                let straight_idx = rng.random_range(0..course.straights.len());
                let straight = &course.straights[straight_idx];
                let activation_distance = rng.random_range(straight.start..straight.end);
                Some(vec![activation_distance])
            } else {
                // No straights on course, fail the roll
                // (This should be rare/unlikely)
                None
            }
        }

        RandomSpatial(RandomSpatialVar::LastStraightRandom) => {
            // last_straight_random==1
            // Picks random point on final straight
            if let Some(last_straight) = course.straights.last() {
                let activation_distance = rng.random_range(last_straight.start..last_straight.end);
                Some(vec![activation_distance])
            } else {
                // No straights on course, fail the roll
                // (This should be rare/unlikely)
                None
            }
        }

        RandomSpatial(RandomSpatialVar::UpSlopeRandom) => {
            // up_slope_random==1
            // Picks random point on any uphill
            let uphills: Vec<_> = course.slopes.iter().filter(|s| s.is_uphill()).collect();

            if !uphills.is_empty() {
                let slope = uphills[rng.random_range(0..uphills.len())];
                let offset: f64 = rng.random();
                let activation_distance = slope.start + offset * slope.length;
                Some(vec![activation_distance])
            } else {
                // No uphills on course, fail the roll
                None
            }
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

            if !late_uphills.is_empty() {
                let slope = late_uphills[rng.random_range(0..late_uphills.len())];
                let offset: f64 = rng.random();
                let activation_distance = slope.start + offset * slope.length;
                Some(vec![activation_distance])
            } else {
                // No uphills after halfway on course, fail the roll
                None
            }
        }

        RandomSpatial(RandomSpatialVar::DownSlopeRandom) => {
            // down_slope_random==1
            // Picks random point on any downhill
            let downhills: Vec<_> = course.slopes.iter().filter(|s| s.is_downhill()).collect();

            if !downhills.is_empty() {
                let slope = downhills[rng.random_range(0..downhills.len())];
                let offset: f64 = rng.random();
                let activation_distance = slope.start + offset * slope.length;
                Some(vec![activation_distance])
            } else {
                // No downhills on course, fail the roll
                None
            }
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

/// Calculate intersection of two intervals [a_start, a_end] and [b_start, b_end]
/// Returns Some((start, end)) if there's an intersection, None otherwise
fn intersect_intervals(a_start: f64, a_end: f64, b_start: f64, b_end: f64) -> Option<(f64, f64)> {
    let intersection_start = a_start.max(b_start);
    let intersection_end = a_end.min(b_end);

    if intersection_start < intersection_end {
        Some((intersection_start, intersection_end))
    } else {
        None
    }
}

/// Pick a random point from a collection of intervals
/// Returns the random point if intervals is non-empty
fn random_point_in_intervals(intervals: &[(f64, f64)], rng: &mut impl Rng) -> Option<f64> {
    if intervals.is_empty() {
        return None;
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

    None
}
