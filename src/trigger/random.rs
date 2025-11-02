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
) -> Result<Vec<f64>, ()> {
    visit_comps(&condition.comp_list, course, rng)
}

/// Visit all comparisons in a list and process random variables
///
/// Returns Ok(activation_distances) indicating where the skill should activate,
/// or Err(()) if a random lot roll failed or a condition within the list is invalid
fn visit_comps(comp_list: &CompList, course: &Course, rng: &mut impl Rng) -> Result<Vec<f64>, ()> {
    match comp_list {
        CompList::Single(comp) => process_random_comp(comp, course, rng),
        CompList::Binary { left, op, right } => {
            match op {
                BinOp::And => {
                    // All comparisons must succeed
                    let mut left_vals = visit_comps(left, course, rng)?;
                    let right_vals = visit_comps(right, course, rng)?;
                    left_vals.extend(right_vals);
                    Ok(left_vals)
                }
                BinOp::Or => {
                    // Will fail only if both comparisons fail
                    match visit_comps(left, course, rng) {
                        Ok(mut left_vals) => match visit_comps(right, course, rng) {
                            Ok(right_vals) => {
                                left_vals.extend(right_vals);
                                Ok(left_vals)
                            }
                            Err(()) => Ok(left_vals),
                        },
                        Err(()) => visit_comps(right, course, rng),
                    }
                }
            }
        }
    }
}

/// Process a single comparison that might contain a random variable
///
/// Returns Ok(activation_distances) indicating where the skill should activate,
/// or Err(()) if a random lot roll failed or a condition is invalid
pub fn process_random_comp(
    comp: &Comp,
    course: &Course,
    rng: &mut impl Rng,
) -> Result<Vec<f64>, ()> {
    use VarType::*;

    match &comp.var {
        // Pure probability roll
        Random(RandomVar::RandomLot) => {
            // random_lot==X means X% chance of activation
            let roll: f64 = rng.random();
            if roll * 100.0 < comp.value as f64 {
                Ok(vec![0.0]) // Success, activation distance is 0 (immediate)
            } else {
                Err(()) // Failed roll
            }
        }

        // Random point during a phase
        Phase(PhaseVar::PhaseRandom) => {
            // phase_random==X where X is phase (0=Early, 1=Mid, 2=Late, 3=Last)
            let phase = match comp.value {
                0 => CoursePhase::Early,
                1 => CoursePhase::Mid,
                2 => CoursePhase::Late,
                3 => CoursePhase::Last,
                _ => return Err(()),
            };

            let (start, end) = phase.boundaries(course);
            let activation_distance = rng.random_range(start..end);
            Ok(vec![activation_distance])
        }

        Phase(PhaseVar::PhaseFirsthalfRandom) => {
            // phase_firsthalf_random==X where X is phase
            // Activates at random point during first half of specified phase
            let phase = match comp.value {
                0 => CoursePhase::Early,
                1 => CoursePhase::Mid,
                2 => CoursePhase::Late,
                3 => CoursePhase::Last,
                _ => return Err(()),
            };

            let (start, end) = phase.boundaries(course);
            let midpoint = (start + end) / 2.0;
            let activation_distance = rng.random_range(start..midpoint);
            Ok(vec![activation_distance])
        }

        Phase(PhaseVar::PhaseFirstquarterRandom) => {
            // phase_firstquarter_random==X where X is phase
            // Activates at random point during first quarter of specified phase
            let phase = match comp.value {
                0 => CoursePhase::Early,
                1 => CoursePhase::Mid,
                2 => CoursePhase::Late,
                3 => CoursePhase::Last,
                _ => return Err(()),
            };

            let (start, end) = phase.boundaries(course);
            let quarter_point = start + (end - start) * 0.25;
            let activation_distance = rng.random_range(start..quarter_point);
            Ok(vec![activation_distance])
        }

        Phase(PhaseVar::PhaseLaterhalfRandom) => {
            // phase_laterhalf_random==X where X is phase
            // Activates at random point during second half of specified phase
            let phase = match comp.value {
                0 => CoursePhase::Early,
                1 => CoursePhase::Mid,
                2 => CoursePhase::Late,
                3 => CoursePhase::Last,
                _ => return Err(()),
            };

            let (start, end) = phase.boundaries(course);
            let midpoint = start + (end - start) * 0.25;
            let activation_distance = rng.random_range(midpoint..end);
            Ok(vec![activation_distance])
        }

        Phase(PhaseVar::PhaseCornerRandom) => {
            // phase_corner_random==X where X is phase
            // Picks random point during a corner in specified phase
            let phase = match comp.value {
                0 => CoursePhase::Early,
                1 => CoursePhase::Mid,
                2 => CoursePhase::Late,
                3 => CoursePhase::Last,
                _ => return Err(()),
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
                Ok(vec![activation_distance])
            } else {
                // No corners in this phase, fail the roll
                Err(())
            }
        }

        Phase(PhaseVar::PhaseStraightRandom) => {
            // phase_straight_random==X where X is phase
            // Picks random point during a straight in specified phase
            let phase = match comp.value {
                0 => CoursePhase::Early,
                1 => CoursePhase::Mid,
                2 => CoursePhase::Late,
                3 => CoursePhase::Last,
                _ => return Err(()),
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
                Ok(vec![activation_distance])
            } else {
                // No straights in this phase, fail the roll
                Err(())
            }
        }

        Phase(PhaseVar::PhaseFirstHalfStraightRandom) => {
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
                Ok(vec![activation_distance])
            } else {
                // No straights in this phase, fail the roll
                Err(())
            }
        }

        Phase(PhaseVar::PhaseLatterHalfStraightRandom) => {
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
                Ok(vec![activation_distance])
            } else {
                // No straights in this phase, fail the roll
                Err(())
            }
        }

        Spatial(SpatialVar::IsFinalcornerRandom) => {
            // is_finalcorner_random==1
            // Picks random point on final corner
            if let Some(last_corner) = course.corners.last() {
                let offset: f64 = rng.random();
                let activation_distance = last_corner.start + offset * last_corner.length;
                Ok(vec![activation_distance])
            } else {
                // No corners on course, fail the roll
                Err(())
            }
        }

        Spatial(SpatialVar::CornerRandom) => {
            // corner_random==X where X is corner number (1-indexed)
            let corner_idx = (comp.value - 1) as usize;
            if corner_idx < course.corners.len() {
                let corner = &course.corners[corner_idx];
                let offset: f64 = rng.random();
                let activation_distance = corner.start + offset * corner.length;
                Ok(vec![activation_distance])
            } else {
                // Invalid corner index, fail the roll
                Err(())
            }
        }

        Spatial(SpatialVar::AllCornerRandom) => {
            // all_corner_random==1
            // Picks 4 random points (roll random corner, then random point on it, 4 times)
            // TODO: extend to multiple activations
            if !course.corners.is_empty() {
                let corner_idx = rng.random_range(0..course.corners.len());
                let corner = &course.corners[corner_idx];
                let offset: f64 = rng.random();
                let activation_distance = corner.start + offset * corner.length;
                Ok(vec![activation_distance])
            } else {
                // No corners on course, fail the roll
                Err(())
            }
        }

        Spatial(SpatialVar::StraightRandom) => {
            // straight_random==1
            // Picks random straight, then random point on it
            if !course.straights.is_empty() {
                let straight_idx = rng.random_range(0..course.straights.len());
                let straight = &course.straights[straight_idx];
                let activation_distance = rng.random_range(straight.start..straight.end);
                Ok(vec![activation_distance])
            } else {
                // No straights on course, fail the roll
                // (This should be rare/unlikely)
                Err(())
            }
        }

        Spatial(SpatialVar::LastStraightRandom) => {
            // last_straight_random==1
            // Picks random point on final straight
            if let Some(last_straight) = course.straights.last() {
                let activation_distance = rng.random_range(last_straight.start..last_straight.end);
                Ok(vec![activation_distance])
            } else {
                // No straights on course, fail the roll
                // (This should be rare/unlikely)
                Err(())
            }
        }

        Spatial(SpatialVar::UpSlopeRandom) => {
            // up_slope_random==1
            // Picks random point on any uphill
            let uphills: Vec<_> = course.slopes.iter().filter(|s| s.is_uphill()).collect();

            if !uphills.is_empty() {
                let slope = uphills[rng.random_range(0..uphills.len())];
                let offset: f64 = rng.random();
                let activation_distance = slope.start + offset * slope.length;
                Ok(vec![activation_distance])
            } else {
                // No uphills on course, fail the roll
                Err(())
            }
        }

        Spatial(SpatialVar::UpSlopeRandomLaterHalf) => {
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
                Ok(vec![activation_distance])
            } else {
                // No uphills after halfway on course, fail the roll
                Err(())
            }
        }

        Spatial(SpatialVar::DownSlopeRandom) => {
            // down_slope_random==1
            // Picks random point on any downhill
            let downhills: Vec<_> = course.slopes.iter().filter(|s| s.is_downhill()).collect();

            if !downhills.is_empty() {
                let slope = downhills[rng.random_range(0..downhills.len())];
                let offset: f64 = rng.random();
                let activation_distance = slope.start + offset * slope.length;
                Ok(vec![activation_distance])
            } else {
                // No downhills on course, fail the roll
                Err(())
            }
        }

        Spatial(SpatialVar::DistanceRateAfterRandom) => {
            // distance_rate_after_random==X where X is percentage
            // Picks random point after X% of the race
            let threshold_distance = (comp.value as f64 / 100.0) * course.distance as f64;
            let end_distance = course.distance as f64;
            let activation_distance = rng.random_range(threshold_distance..end_distance);
            Ok(vec![activation_distance])
        }

        _ => {
            // Not a random variable, skip
            Ok(vec![])
        }
    }
}

/// Calculate intersection of two intervals [a_start, a_end] and [b_start, b_end]
/// Returns Some((start, end)) if there's an intersection, Err(()) otherwise
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::course::geometry::*;
    use crate::course::*;

    fn create_test_course() -> Course {
        Course::new_test(
            10001,
            CourseData {
                distance: 2400,
                distance_type: DistanceType::Medium,
                surface: Track::Turf,
                turn: Direction::Left,
                course_set_status: vec![],
                finish_time_min: 140000,
                finish_time_max: 160000,
                corners: vec![
                    Corner {
                        start: 100.0,
                        length: 200.0,
                    },
                    Corner {
                        start: 500.0,
                        length: 150.0,
                    },
                ],
                slopes: vec![Slope::new(800.0, 100.0, 500)],
                straights: vec![Straight {
                    start: 0.0,
                    end: 100.0,
                    front_type: FrontType::Audience,
                }],
                race_track_id: 1,
                _lane_max: (),
                _course: (),
            },
        )
    }

    #[test]
    fn test_random_lot_success() {
        let course = create_test_course();
        let condition = parse_condition("random_lot==100").unwrap(); // 100% chance

        let mut rng = rand::rng();
        let result = roll_condition(&condition, &course, &mut rng);

        assert!(result.is_ok());
    }

    #[test]
    fn test_phase_random() {
        let course = create_test_course();
        let condition = parse_condition("phase_random==1").unwrap(); // Mid phase

        let mut rng = rand::rng();
        let result = roll_condition(&condition, &course, &mut rng);

        assert!(result.is_ok());
        let activation = result.unwrap();

        // Check activation distance is within Mid phase boundaries
        let section = course.section_length();
        assert!(activation[0] >= section * 4.0);
        assert!(activation[0] < section * 16.0);
    }

    #[test]
    fn test_corner_random() {
        let course = create_test_course();
        let condition = parse_condition("corner_random==1").unwrap(); // First corner

        let mut rng = rand::rng();
        let result = roll_condition(&condition, &course, &mut rng);

        assert!(result.is_ok());
        let activation = result.unwrap();

        // Check activation distance is within first corner
        assert!(activation[0] >= 100.0);
        assert!(activation[0] <= 300.0);
    }

    #[test]
    fn test_corner_random_multiple() {
        let course = create_test_course();
        let condition =
            parse_condition("corner_random==1@corner_random==2@corner_random==3@corner_random==4")
                .unwrap(); // First corner

        let mut rng = rand::rng();
        let result = roll_condition(&condition, &course, &mut rng);

        // Even though corner 3 and 4 don't exist, the others should still roll successfully
        assert!(result.is_ok());
        let activation = result.unwrap();

        // Check activation distance is within first corner
        assert!(activation[0] >= 100.0);
        assert!(activation[0] <= 300.0);

        // Check activation distance is within second corner
        assert!(activation[1] >= 500.0);
        assert!(activation[1] <= 650.0);
    }

    #[test]
    fn test_phase_corner_random() {
        let course = create_test_course();
        // Course has 2400m, section = 100m
        // Phase::Mid is sections 4-16, so 400m to 1600m
        // Corners in test course:
        //   Corner 1: 100m-300m (no overlap with Mid 400-1600)
        //   Corner 2: 500m-650m (fully in Mid)

        let condition = parse_condition("phase_corner_random==1").unwrap(); // Mid phase
        let mut rng = rand::rng();

        // Run multiple times to ensure it's in valid range
        for _ in 0..20 {
            let result = roll_condition(&condition, &course, &mut rng);
            assert!(result.is_ok());

            let activation = result.unwrap();
            let dist = activation[0];

            // Should be in Mid phase (400-1600)
            assert!(
                dist >= 400.0 && dist <= 1600.0,
                "Distance {} not in Mid phase range [400, 1600]",
                dist
            );

            // Should be in corner2 (500-650), which is the only corner overlapping with Mid
            assert!(
                dist >= 500.0 && dist <= 650.0,
                "Distance {} not in corner2 segment [500, 650]",
                dist
            );
        }
    }

    #[test]
    fn test_phase_straight_random() {
        let course = create_test_course();
        // Straights in test course: [0-100]
        // Phase::Early is 0-400m (sections 0-4)
        // Straight [0-100] fully overlaps with Early

        let condition = parse_condition("phase_straight_random==0").unwrap(); // Early phase
        let mut rng = rand::rng();

        for _ in 0..20 {
            let result = roll_condition(&condition, &course, &mut rng);
            assert!(result.is_ok());

            let activation = result.unwrap();
            let dist = activation[0];

            // Should be in Early phase (0-400)
            assert!(
                dist >= 0.0 && dist <= 400.0,
                "Distance {} not in Early phase range [0, 400]",
                dist
            );

            // Should be in straight segment [0-100] ∩ [0-400] = [0-100]
            assert!(
                dist >= 0.0 && dist <= 100.0,
                "Distance {} not in straight segment [0, 100]",
                dist
            );
        }
    }
}
