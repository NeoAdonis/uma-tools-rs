use crate::condition::parser::*;
use crate::condition::variables::*;
use crate::course::Course;
use crate::course::Phase as CoursePhase;
use crate::course::geometry::SlopeType;
use crate::skill::Skill;
use crate::trigger::system::RandomActivation;
use rand::prelude::*;
use std::collections::HashMap;

/// Pre-roll all random triggers before race starts
/// Returns a map from skill_index to RandomActivation
pub fn preroll_random_triggers(
    skills: &[Skill],
    course: &Course,
    rng: &mut impl Rng,
) -> HashMap<usize, RandomActivation> {
    let mut activations = HashMap::new();

    for (skill_idx, skill) in skills.iter().enumerate() {
        for alternative in &skill.alternatives {
            if let Ok(condition) = parse_condition(&alternative.condition)
                && let Some(activation) = roll_condition(&condition, course, rng)
            {
                activations.insert(skill_idx, activation);
                break; // Only one alternative per skill activates
            }
        }
    }

    activations
}

/// Roll a condition to determine if and where it activates
fn roll_condition(
    condition: &Condition,
    course: &Course,
    rng: &mut impl Rng,
) -> Option<RandomActivation> {
    let mut context = RollContext {
        has_random: false,
        activation_distance: 0.0,
        phase_restriction: None,
        corner_restriction: None,
        slope_restriction: None,
        failed: false,
    };

    // Walk the condition tree and process random variables
    visit_comps(&condition.comp_list, &mut context, course, rng);

    if context.failed {
        return None; // Random roll failed, skill never activates
    }

    if context.has_random {
        Some(RandomActivation {
            activation_distance: context.activation_distance,
            phase_restriction: context.phase_restriction,
            corner_restriction: context.corner_restriction,
            slope_restriction: context.slope_restriction,
            activated: false,
        })
    } else {
        None // No random variables in this condition
    }
}

struct RollContext {
    has_random: bool,
    activation_distance: f64,
    phase_restriction: Option<CoursePhase>,
    corner_restriction: Option<usize>,
    slope_restriction: Option<SlopeType>,
    failed: bool,
}

/// Visit all comparisons in a CompList and process random variables
fn visit_comps(
    comp_list: &CompList,
    context: &mut RollContext,
    course: &Course,
    rng: &mut impl Rng,
) {
    match comp_list {
        CompList::Single(comp) => {
            process_random_comp(comp, context, course, rng);
        }
        CompList::Binary { left, right, .. } => {
            visit_comps(left, context, course, rng);
            visit_comps(right, context, course, rng);
        }
    }
}

/// Process a single comparison that might contain a random variable
fn process_random_comp(
    comp: &Comp,
    context: &mut RollContext,
    course: &Course,
    rng: &mut impl Rng,
) {
    use VarType::*;

    match &comp.var {
        // Pure probability roll
        Trigger(TriggerVar::RandomLot) => {
            // random_lot==X means X% chance of activation
            let roll: f64 = rng.random();
            context.has_random = true;

            if roll * 100.0 >= comp.value as f64 {
                context.failed = true;
            }
            // If successful, activation_distance remains 0.0 (activates immediately when other conditions met)
        }

        // Random point during a phase
        Phase(PhaseVar::PhaseRandom) => {
            // phase_random==X where X is phase (0=Early, 1=Mid, 2=Late, 3=Last)
            let phase = match comp.value {
                0 => CoursePhase::Early,
                1 => CoursePhase::Mid,
                2 => CoursePhase::Late,
                3 => CoursePhase::Last,
                _ => return,
            };

            let (start, end) = phase.boundaries(course);
            context.activation_distance = rng.random_range(start..end);
            context.phase_restriction = Some(phase);
            context.has_random = true;
        }

        Phase(PhaseVar::PhaseFirsthalfRandom) => {
            // phase_firsthalf_random==X where X is phase
            // Activates at random point during first half of specified phase
            let phase = match comp.value {
                0 => CoursePhase::Early,
                1 => CoursePhase::Mid,
                2 => CoursePhase::Late,
                3 => CoursePhase::Last,
                _ => return,
            };

            let (start, end) = phase.boundaries(course);
            let midpoint = (start + end) / 2.0;
            context.activation_distance = rng.random_range(start..midpoint);
            context.phase_restriction = Some(phase);
            context.has_random = true;
        }

        Phase(PhaseVar::PhaseFirstquarterRandom) => {
            // phase_firstquarter_random==X where X is phase
            // Activates at random point during first quarter of specified phase
            let phase = match comp.value {
                0 => CoursePhase::Early,
                1 => CoursePhase::Mid,
                2 => CoursePhase::Late,
                3 => CoursePhase::Last,
                _ => return,
            };

            let (start, end) = phase.boundaries(course);
            let quarter_point = start + (end - start) * 0.25;
            context.activation_distance = rng.random_range(start..quarter_point);
            context.phase_restriction = Some(phase);
            context.has_random = true;
        }

        Phase(PhaseVar::PhaseLaterhalfRandom) => {
            // phase_laterhalf_random==X where X is phase
            // Activates at random point during second half of specified phase
            let phase = match comp.value {
                0 => CoursePhase::Early,
                1 => CoursePhase::Mid,
                2 => CoursePhase::Late,
                3 => CoursePhase::Last,
                _ => return,
            };

            let (start, end) = phase.boundaries(course);
            let midpoint = (start + end) / 2.0;
            context.activation_distance = rng.random_range(midpoint..end);
            context.phase_restriction = Some(phase);
            context.has_random = true;
        }

        Phase(PhaseVar::PhaseCornerRandom) => {
            // phase_corner_random==X where X is phase
            // Picks random point during a corner in specified phase
            let phase = match comp.value {
                0 => CoursePhase::Early,
                1 => CoursePhase::Mid,
                2 => CoursePhase::Late,
                3 => CoursePhase::Last,
                _ => return,
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

            if let Some(distance) = random_point_in_intervals(&corner_intervals, rng) {
                context.activation_distance = distance;
                context.phase_restriction = Some(phase);
                context.has_random = true;
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
                _ => return,
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

            if let Some(distance) = random_point_in_intervals(&straight_intervals, rng) {
                context.activation_distance = distance;
                context.phase_restriction = Some(phase);
                context.has_random = true;
            }
        }

        Phase(PhaseVar::PhaseFirstHalfStraightRandom) => {
            // TODO: Under investigation - straight in first half of race
            let section_len = course.section_length();
            let halfway = section_len * 12.0;

            if !course.straights.is_empty() {
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
                    context.activation_distance = rng.random_range(straight.start..end);
                    context.has_random = true;
                }
            }
        }

        Phase(PhaseVar::PhaseLatterHalfStraightRandom) => {
            // TODO: Under investigation - straight in second half of race
            let section_len = course.section_length();
            let halfway = section_len * 12.0;

            if !course.straights.is_empty() {
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
                    context.activation_distance = rng.random_range(start..straight.end);
                    context.has_random = true;
                }
            }
        }

        Phase(PhaseVar::IsFinalcornerRandom) => {
            // is_finalcorner_random==1
            // Picks random point on final corner
            if let Some(last_corner) = course.corners.last() {
                let offset: f64 = rng.random();
                context.activation_distance = last_corner.start + offset * last_corner.length;
                context.corner_restriction = Some(course.corners.len() - 1);
                context.has_random = true;
            }
        }

        Spatial(SpatialVar::CornerRandom) => {
            // corner_random==X where X is corner number (1-indexed)
            let corner_idx = (comp.value - 1) as usize;
            if corner_idx < course.corners.len() {
                let corner = &course.corners[corner_idx];
                let offset: f64 = rng.random();
                context.activation_distance = corner.start + offset * corner.length;
                context.corner_restriction = Some(corner_idx);
                context.has_random = true;
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
                context.activation_distance = corner.start + offset * corner.length;
                context.corner_restriction = Some(corner_idx);
                context.has_random = true;
            }
        }

        Spatial(SpatialVar::StraightRandom) => {
            // straight_random==1
            // Picks random straight, then random point on it
            if !course.straights.is_empty() {
                let straight_idx = rng.random_range(0..course.straights.len());
                let straight = &course.straights[straight_idx];
                context.activation_distance = rng.random_range(straight.start..straight.end);
                context.has_random = true;
            }
        }

        Spatial(SpatialVar::LastStraightRandom) => {
            // last_straight_random==1
            // Picks random point on final straight
            if let Some(last_straight) = course.straights.last() {
                context.activation_distance =
                    rng.random_range(last_straight.start..last_straight.end);
                context.has_random = true;
            }
        }

        Spatial(SpatialVar::UpSlopeRandom) => {
            // up_slope_random==1
            // Picks random point on any uphill
            let uphills: Vec<_> = course.slopes.iter().filter(|s| s.is_uphill()).collect();

            if !uphills.is_empty() {
                let slope = uphills[rng.random_range(0..uphills.len())];
                let offset: f64 = rng.random();
                context.activation_distance = slope.start + offset * slope.length;
                context.slope_restriction = Some(SlopeType::Uphill);
                context.has_random = true;
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
                context.activation_distance = slope.start + offset * slope.length;
                context.slope_restriction = Some(SlopeType::Uphill);
                context.has_random = true;
            }
        }

        Spatial(SpatialVar::DownSlopeRandom) => {
            // down_slope_random==1
            // Picks random point on any downhill
            let downhills: Vec<_> = course.slopes.iter().filter(|s| s.is_downhill()).collect();

            if !downhills.is_empty() {
                let slope = downhills[rng.random_range(0..downhills.len())];
                let offset: f64 = rng.random();
                context.activation_distance = slope.start + offset * slope.length;
                context.slope_restriction = Some(SlopeType::Downhill);
                context.has_random = true;
            }
        }

        Distance(DistanceVar::DistanceRateAfterRandom) => {
            // distance_rate_after_random==X where X is percentage
            // Picks random point after X% of the race
            let threshold_distance = (comp.value as f64 / 100.0) * course.distance as f64;
            let end_distance = course.distance as f64;
            context.activation_distance = rng.random_range(threshold_distance..end_distance);
            context.has_random = true;
        }

        _ => {
            // Not a random variable, skip
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
                lane_max: 18,
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

        assert!(result.is_some());
    }

    #[test]
    fn test_phase_random() {
        let course = create_test_course();
        let condition = parse_condition("phase_random==1").unwrap(); // Mid phase

        let mut rng = rand::rng();
        let result = roll_condition(&condition, &course, &mut rng);

        assert!(result.is_some());
        let activation = result.unwrap();
        assert_eq!(activation.phase_restriction, Some(Phase::Mid));

        // Check activation distance is within Mid phase boundaries
        let section = course.section_length();
        assert!(activation.activation_distance >= section * 4.0);
        assert!(activation.activation_distance < section * 16.0);
    }

    #[test]
    fn test_corner_random() {
        let course = create_test_course();
        let condition = parse_condition("corner_random==1").unwrap(); // First corner

        let mut rng = rand::rng();
        let result = roll_condition(&condition, &course, &mut rng);

        assert!(result.is_some());
        let activation = result.unwrap();
        assert_eq!(activation.corner_restriction, Some(0));

        // Check activation distance is within first corner
        assert!(activation.activation_distance >= 100.0);
        assert!(activation.activation_distance <= 300.0);
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
            assert!(result.is_some());

            let activation = result.unwrap();
            let dist = activation.activation_distance;

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

            // Verify phase restriction is set
            assert_eq!(activation.phase_restriction, Some(Phase::Mid));
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
            assert!(result.is_some());

            let activation = result.unwrap();
            let dist = activation.activation_distance;

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

            // Verify phase restriction is set
            assert_eq!(activation.phase_restriction, Some(Phase::Early));
        }
    }
}
