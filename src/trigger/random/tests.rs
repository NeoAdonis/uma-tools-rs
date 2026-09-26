use super::*;
use crate::course::geometry::*;
use crate::course::*;

fn create_test_course() -> Course {
    create_test_course_with(
        vec![
            Corner {
                start: 100.0,
                length: 200.0,
            },
            Corner {
                start: 500.0,
                length: 150.0,
            },
        ],
        vec![Slope::new(800.0, 100.0, 500)],
        vec![Straight {
            start: 0.0,
            end: 100.0,
            front_type: FrontType::Audience,
        }],
    )
}

fn create_test_course_with_straights(straights: Vec<Straight>) -> Course {
    let course = create_test_course();
    create_test_course_with(course.corners.clone(), course.slopes.clone(), straights)
}

fn create_test_course_with_slopes(slopes: Vec<Slope>) -> Course {
    let course = create_test_course();
    create_test_course_with(course.corners.clone(), slopes, course.straights.clone())
}

fn create_test_course_with(
    corners: Vec<Corner>,
    slopes: Vec<Slope>,
    straights: Vec<Straight>,
) -> Course {
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
            corners,
            slopes,
            straights,
            race_track_id: 1,
            lane_max: 10000,
        },
    )
}

/// Roll a single-branch condition.
fn roll(condition: &Condition, course: &Course, rng: &mut impl Rng) -> Option<Vec<f64>> {
    let mut rolls = roll_condition(condition, course, rng);
    assert_eq!(rolls.len(), 1, "expected a single-branch condition");
    rolls.remove(0)
}

#[test]
fn test_random_lot_success() {
    let course = create_test_course();
    let condition = parse_condition("random_lot==100").unwrap(); // 100% chance

    let mut rng = rand::rng();
    let result = roll(&condition, &course, &mut rng);

    assert!(result.is_some());
}

#[test]
fn test_phase_random() {
    let course = create_test_course();
    let condition = parse_condition("phase_random==1").unwrap(); // Mid phase

    let mut rng = rand::rng();
    let result = roll(&condition, &course, &mut rng);

    assert!(result.is_some());
    let activation = result.unwrap();

    // Check activation distance is within Mid phase boundaries
    let section = course.section_length();
    assert!(activation[0] >= section * 4.0);
    assert!(activation[0] < section * 16.0);
}

/// Roll a single-branch `condition` many times and return the first points rolled.
fn rolled_points(condition: &str, course: &Course) -> Vec<f64> {
    let condition = parse_condition(condition).unwrap();
    let mut rng = rand::rngs::StdRng::seed_from_u64(0);
    (0..500)
        .map(|_| roll(&condition, course, &mut rng).unwrap()[0])
        .collect()
}

/// Assert that all `points` are within `[min, max]`, and that they cover it
/// (some are within 5% of the range from both ends).
fn assert_covers(points: &[f64], min: f64, max: f64, context: &str) {
    let lowest = points.iter().copied().fold(f64::INFINITY, f64::min);
    let highest = points.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let tolerance = (max - min) * 0.05;
    assert!(
        lowest >= min && highest <= max,
        "{context}: [{lowest}, {highest}] not in [{min}, {max}]"
    );
    assert!(
        lowest < min + tolerance && highest > max - tolerance,
        "{context}: [{lowest}, {highest}] doesn't cover [{min}, {max}]"
    );
}

#[test]
fn test_random_segment() {
    let mut rng = rand::rngs::StdRng::seed_from_u64(0);
    // Segment must fit: starts from 100 to 115
    let points: Vec<f64> = (0..500)
        .map(|_| random_segment(100.0, 125.0, &mut rng))
        .collect();
    assert_covers(&points, 100.0, 115.0, "[100-125]");

    // Intervals of 10m or less only have one segment
    assert_eq!(random_segment(100.0, 110.0, &mut rng), 100.0);
    assert_eq!(random_segment(100.0, 105.0, &mut rng), 100.0);
}

#[test]
fn test_random_segment_in_intervals() {
    let mut rng = rand::rngs::StdRng::seed_from_u64(0);
    // Stitched together: [0-15] + [100-125] = 40m, so offsets 0 to 30, which fall
    // on [0-15) and [100-115]
    let intervals = [(0.0, 15.0), (100.0, 125.0)];
    let (first, second): (Vec<f64>, Vec<f64>) = (0..500)
        .map(|_| random_segment_in_intervals(&intervals, &mut rng).unwrap())
        .partition(|&point| point < 15.0);
    assert_covers(&first, 0.0, 15.0, "first interval");
    assert_covers(&second, 100.0, 115.0, "second interval");

    assert!(random_segment_in_intervals(&[], &mut rng).is_none());
}

#[test]
fn test_corner_random() {
    // Two corners on course, which are the third and fourth
    let course = create_test_course();
    assert_covers(
        &rolled_points("corner_random==3", &course),
        100.0,
        290.0,
        "corner 3",
    );
    assert_covers(
        &rolled_points("corner_random==4", &course),
        500.0,
        640.0,
        "corner 4",
    );

    // First and second corners, and invalid corner numbers fail the roll
    let mut rng = rand::rng();
    for value in [0, 1, 2, 5] {
        let condition = parse_condition(&format!("corner_random=={value}")).unwrap();
        assert!(
            roll(&condition, &course, &mut rng).is_none(),
            "corner_random=={value}"
        );
    }
}

#[test]
fn test_corner_random_last_lap() {
    // Two laps: corners 1-4 at [0-100], [200-300], ..., then again at [800-900], ...
    let corners = (0..8)
        .map(|i| Corner {
            start: i as f64 * 200.0,
            length: 100.0,
        })
        .collect();
    let course = create_test_course_with(corners, vec![], vec![]);

    for number in 1..=4 {
        // Only the last lap's corner counts
        let start = 800.0 + (number - 1) as f64 * 200.0;
        let points = rolled_points(&format!("corner_random=={number}"), &course);
        assert_covers(
            &points,
            start,
            start + 90.0,
            &format!("corner_random=={number}"),
        );
    }
}

#[test]
fn test_corner_random_per_branch() {
    let course = create_test_course();
    let condition =
        parse_condition("corner_random==1@corner_random==2@corner_random==3@corner_random==4")
            .unwrap();

    let mut rng = rand::rng();
    let rolls = roll_condition(&condition, &course, &mut rng);

    // Corners 1 and 2 don't exist, so only the last two branches roll successfully,
    // each with its own point
    assert_eq!(rolls.len(), 4);
    assert!(rolls[0].is_none() && rolls[1].is_none());
    let third = rolls[2].as_ref().unwrap();
    let fourth = rolls[3].as_ref().unwrap();
    assert_eq!(third.len(), 1);
    assert!((100.0..=290.0).contains(&third[0]));
    assert_eq!(fourth.len(), 1);
    assert!((500.0..=640.0).contains(&fourth[0]));
}

#[test]
fn test_straight_random() {
    // Short straight [0-20] and long straight [100-1100]
    let course = create_test_course_with_straights(vec![
        Straight {
            start: 0.0,
            end: 20.0,
            front_type: FrontType::Audience,
        },
        Straight {
            start: 100.0,
            end: 1100.0,
            front_type: FrontType::Opposite,
        },
    ]);
    let (short, long): (Vec<f64>, Vec<f64>) = rolled_points("straight_random==1", &course)
        .into_iter()
        .partition(|&point| point < 20.0);

    assert_covers(&short, 0.0, 10.0, "short straight");
    assert_covers(&long, 100.0, 1090.0, "long straight");
    // Both straights have the same chance regardless of length
    assert!(
        (200..=300).contains(&short.len()),
        "short straight picked {} times",
        short.len()
    );
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
        let result = roll(&condition, &course, &mut rng);
        assert!(result.is_some());

        let activation = result.unwrap();
        let dist = activation[0];

        // Should be in Mid phase (400-1600)
        assert!(
            (400.0..=1600.0).contains(&dist),
            "Distance {} not in Mid phase range [400, 1600]",
            dist
        );

        // Should be in corner2 (500-650), which is the only corner overlapping with Mid
        assert!(
            (500.0..=650.0).contains(&dist),
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
        let result = roll(&condition, &course, &mut rng);
        assert!(result.is_some());

        let activation = result.unwrap();
        let dist = activation[0];

        // Should be in Early phase (0-400)
        assert!(
            (0.0..=400.0).contains(&dist),
            "Distance {} not in Early phase range [0, 400]",
            dist
        );

        // Should be in straight segment [0-100] ∩ [0-400] = [0-100]
        assert!(
            (0.0..=100.0).contains(&dist),
            "Distance {} not in straight segment [0, 100]",
            dist
        );
    }
}

#[test]
fn test_phase_first_half_straight_random() {
    // Course has 2400m, section = 100m
    // Phase::Late first half is sections 16-18, so 1600m to 1800m
    // Straights: [1500-1700] and [1750-2000]
    // Overlaps with Late first half: [1600-1700] and [1750-1800]
    let course = create_test_course_with_straights(vec![
        Straight {
            start: 1500.0,
            end: 1700.0,
            front_type: FrontType::Audience,
        },
        Straight {
            start: 1750.0,
            end: 2000.0,
            front_type: FrontType::Opposite,
        },
    ]);

    let condition = parse_condition("phase_first_half_straight_random==2").unwrap(); // Late phase
    let mut rng = rand::rngs::StdRng::seed_from_u64(0);
    let (mut hit_first, mut hit_second) = (false, false);

    for _ in 0..200 {
        let activation = roll(&condition, &course, &mut rng).unwrap();
        let dist = activation[0];

        if (1600.0..=1700.0).contains(&dist) {
            hit_first = true;
        } else if (1750.0..=1800.0).contains(&dist) {
            hit_second = true;
        } else {
            panic!("Distance {dist} not in [1600, 1700] or [1750, 1800]");
        }
    }

    // Points should be picked from both overlaps
    assert!(hit_first && hit_second);
}

#[test]
fn test_phase_first_half_straight_random_no_straight() {
    // Only straight is [0-100], which doesn't overlap with Late first half (1600-1800)
    let course = create_test_course();
    let condition = parse_condition("phase_first_half_straight_random==2").unwrap();
    let mut rng = rand::rng();

    assert!(roll(&condition, &course, &mut rng).is_none());
}

#[test]
fn test_phase_first_half_straight_random_invalid_phase() {
    let course = create_test_course();
    let condition = parse_condition("phase_first_half_straight_random==4").unwrap();
    let mut rng = rand::rng();

    assert!(roll(&condition, &course, &mut rng).is_none());
}

#[test]
fn test_phase_subdivision_random_all_phases() {
    // Course has 2400m, section = 100m
    let course = create_test_course();
    // (variable, expected range in meters for phases 0-3)
    let cases = [
        (
            "phase_random",
            [
                (0.0, 400.0),
                (400.0, 1600.0),
                (1600.0, 2000.0),
                (2000.0, 2400.0),
            ],
        ),
        (
            "phase_firstquarter_random",
            [
                (0.0, 100.0),
                (400.0, 700.0),
                (1600.0, 1700.0),
                (2000.0, 2100.0),
            ],
        ),
        (
            "phase_firsthalf_random",
            [
                (0.0, 200.0),
                (400.0, 1000.0),
                (1600.0, 1800.0),
                (2000.0, 2200.0),
            ],
        ),
        (
            "phase_laterhalf_random",
            [
                (200.0, 400.0),
                (1000.0, 1600.0),
                (1800.0, 2000.0),
                (2200.0, 2400.0),
            ],
        ),
    ];
    let mut rng = rand::rngs::StdRng::seed_from_u64(0);

    for (var, ranges) in cases {
        for (phase, (start, end)) in ranges.into_iter().enumerate() {
            let condition = parse_condition(&format!("{var}=={phase}")).unwrap();
            for _ in 0..50 {
                let dist = roll(&condition, &course, &mut rng).unwrap()[0];
                assert!(
                    (start..end).contains(&dist),
                    "{var}=={phase}: distance {dist} not in [{start}, {end})"
                );
            }
        }

        // Invalid phase fails the roll
        let condition = parse_condition(&format!("{var}==4")).unwrap();
        assert!(roll(&condition, &course, &mut rng).is_none(), "{var}==4");
    }
}

#[test]
fn test_phase_corner_random_partial_overlap() {
    // Corner [300-700] straddles Early (0-400) and Mid (400-1600)
    let course = create_test_course_with(
        vec![Corner {
            start: 300.0,
            length: 400.0,
        }],
        vec![],
        vec![],
    );
    let mut rng = rand::rngs::StdRng::seed_from_u64(0);

    for (phase, (start, end)) in [(0, (300.0, 400.0)), (1, (400.0, 700.0))] {
        let condition = parse_condition(&format!("phase_corner_random=={phase}")).unwrap();
        for _ in 0..50 {
            let dist = roll(&condition, &course, &mut rng).unwrap()[0];
            assert!(
                (start..=end).contains(&dist),
                "phase {phase}: distance {dist} not in [{start}, {end}]"
            );
        }
    }

    // No corner in Late phase fails the roll
    let condition = parse_condition("phase_corner_random==2").unwrap();
    assert!(roll(&condition, &course, &mut rng).is_none());
}

#[test]
fn test_phase_straight_random_multiple_straights() {
    // Straights [300-500] and [1500-1700] both overlap Mid (400-1600)
    // Overlaps: [400-500] and [1500-1600]
    let course = create_test_course_with_straights(vec![
        Straight {
            start: 300.0,
            end: 500.0,
            front_type: FrontType::Audience,
        },
        Straight {
            start: 1500.0,
            end: 1700.0,
            front_type: FrontType::Opposite,
        },
    ]);
    let condition = parse_condition("phase_straight_random==1").unwrap();
    let mut rng = rand::rngs::StdRng::seed_from_u64(0);
    let (mut hit_first, mut hit_second) = (false, false);

    for _ in 0..200 {
        let dist = roll(&condition, &course, &mut rng).unwrap()[0];
        if (400.0..=500.0).contains(&dist) {
            hit_first = true;
        } else if (1500.0..=1600.0).contains(&dist) {
            hit_second = true;
        } else {
            panic!("Distance {dist} not in [400, 500] or [1500, 1600]");
        }
    }
    assert!(hit_first && hit_second);

    // No straight in Last phase fails the roll
    let condition = parse_condition("phase_straight_random==3").unwrap();
    assert!(roll(&condition, &course, &mut rng).is_none());
}

#[test]
fn test_phase_latter_half_straight_random() {
    // Mid second half is sections 10-16, so 1000m to 1600m
    // Straights: [900-1100] and [1500-1700]
    // Overlaps with Mid second half: [1000-1100] and [1500-1600]
    let course = create_test_course_with_straights(vec![
        Straight {
            start: 900.0,
            end: 1100.0,
            front_type: FrontType::Audience,
        },
        Straight {
            start: 1500.0,
            end: 1700.0,
            front_type: FrontType::Opposite,
        },
    ]);
    let condition = parse_condition("phase_latter_half_straight_random==1").unwrap();
    let mut rng = rand::rngs::StdRng::seed_from_u64(0);
    let (mut hit_first, mut hit_second) = (false, false);

    for _ in 0..200 {
        let dist = roll(&condition, &course, &mut rng).unwrap()[0];
        if (1000.0..=1100.0).contains(&dist) {
            hit_first = true;
        } else if (1500.0..=1600.0).contains(&dist) {
            hit_second = true;
        } else {
            panic!("Distance {dist} not in [1000, 1100] or [1500, 1600]");
        }
    }
    assert!(hit_first && hit_second);

    // Only straight in Early is [0-100], which isn't in its second half (200-400)
    let course = create_test_course();
    let condition = parse_condition("phase_latter_half_straight_random==0").unwrap();
    assert!(roll(&condition, &course, &mut rng).is_none());
}

#[test]
fn test_up_slope_random() {
    // Uphills [800-900] and [1500-1550], downhill [1000-1200] (never picked)
    let course = create_test_course_with_slopes(vec![
        Slope::new(800.0, 100.0, 500),
        Slope::new(1000.0, 200.0, -500),
        Slope::new(1500.0, 50.0, 200),
    ]);
    let condition = parse_condition("up_slope_random==1").unwrap();
    let mut rng = rand::rngs::StdRng::seed_from_u64(0);
    let (mut hit_first, mut hit_second) = (false, false);

    for _ in 0..200 {
        let dist = roll(&condition, &course, &mut rng).unwrap()[0];
        if (800.0..=900.0).contains(&dist) {
            hit_first = true;
        } else if (1500.0..=1550.0).contains(&dist) {
            hit_second = true;
        } else {
            panic!("Distance {dist} not in [800, 900] or [1500, 1550]");
        }
    }
    assert!(hit_first && hit_second);

    // Only downhills on course fails the roll
    let course = create_test_course_with_slopes(vec![Slope::new(1000.0, 200.0, -500)]);
    assert!(roll(&condition, &course, &mut rng).is_none());
}

#[test]
fn test_up_slope_random_later_half() {
    // Course has 2400m, halfway = 1200m
    // Uphills: [800-900] (first half), [1100-1300] (crosses halfway), [1800-1900]
    // Downhill [2000-2100] is never picked
    // Valid points: [1200-1300] and [1800-1900]
    let course = create_test_course_with_slopes(vec![
        Slope::new(800.0, 100.0, 500),
        Slope::new(1100.0, 200.0, 300),
        Slope::new(1800.0, 100.0, 500),
        Slope::new(2000.0, 100.0, -500),
    ]);
    let condition = parse_condition("up_slope_random_later_half==1").unwrap();
    let mut rng = rand::rngs::StdRng::seed_from_u64(0);
    let (mut hit_crossing, mut hit_late) = (false, false);

    for _ in 0..200 {
        let dist = roll(&condition, &course, &mut rng).unwrap()[0];
        if (1200.0..=1300.0).contains(&dist) {
            hit_crossing = true;
        } else if (1800.0..=1900.0).contains(&dist) {
            hit_late = true;
        } else {
            panic!("Distance {dist} not in [1200, 1300] or [1800, 1900]");
        }
    }
    assert!(hit_crossing && hit_late);

    // Only uphill [800-900] is in the first half, which fails the roll
    let course = create_test_course();
    assert!(roll(&condition, &course, &mut rng).is_none());
}

#[test]
fn test_phase_corner_random_stitched_segments() {
    // Mid phase is 400m to 1600m
    // Corners [390-425] and [500-530] clipped to Mid: [400-425] and [500-530]
    // Stitched together: 55m, so offsets 0 to 45, which fall on [400-425) and
    // [500-520]
    let course = create_test_course_with(
        vec![
            Corner {
                start: 390.0,
                length: 35.0,
            },
            Corner {
                start: 500.0,
                length: 30.0,
            },
        ],
        vec![],
        vec![],
    );
    let (first, second): (Vec<f64>, Vec<f64>) = rolled_points("phase_corner_random==1", &course)
        .into_iter()
        .partition(|&point| point < 425.0);
    assert_covers(&first, 400.0, 425.0, "first corner");
    assert_covers(&second, 500.0, 520.0, "second corner");
}

#[test]
fn test_all_corner_random() {
    // Corners [100-300] and [500-650]
    let course = create_test_course();
    let condition = parse_condition("all_corner_random==1").unwrap();
    let mut rng = rand::rngs::StdRng::seed_from_u64(0);
    let mut counts = [0; 5];

    for _ in 0..500 {
        let points = roll(&condition, &course, &mut rng).unwrap();
        counts[points.len()] += 1;

        for point in &points {
            assert!(
                (100.0..=290.0).contains(point) || (500.0..=640.0).contains(point),
                "{point} not a segment on a corner: {points:?}"
            );
        }
        // In race order, and segments never overlap
        for pair in points.windows(2) {
            assert!(
                pair[1] >= pair[0] + 10.0,
                "overlapping or unordered: {points:?}"
            );
        }
    }

    // Always at least one segment, and up to 4
    assert_eq!(counts[0], 0);
    assert!(
        counts[1..].iter().all(|&count| count > 0),
        "segment counts: {counts:?}"
    );
}

#[test]
fn test_all_corner_random_short_corners() {
    let mut rng = rand::rngs::StdRng::seed_from_u64(0);
    let condition = parse_condition("all_corner_random==1").unwrap();

    // A 10m corner only fits one segment
    let course = create_test_course_with(
        vec![Corner {
            start: 500.0,
            length: 10.0,
        }],
        vec![],
        vec![],
    );
    assert_eq!(roll(&condition, &course, &mut rng), Some(vec![500.0]));

    // A 25m corner fits a second segment only if the first one starts within 5m
    // of the corner start, leaving at least 10m after it
    let course = create_test_course_with(
        vec![Corner {
            start: 500.0,
            length: 25.0,
        }],
        vec![],
        vec![],
    );
    let (mut one, mut two) = (0, 0);
    for _ in 0..500 {
        let points = roll(&condition, &course, &mut rng).unwrap();
        match points[..] {
            [first] => {
                assert!(first > 505.0 && first <= 515.0, "{points:?}");
                one += 1;
            }
            [first, second] => {
                assert!((500.0..=505.0).contains(&first), "{points:?}");
                assert!(second >= first + 10.0 && second <= 515.0, "{points:?}");
                two += 1;
            }
            _ => panic!("unexpected segments: {points:?}"),
        }
    }
    // First segment starts uniformly in [500-515], so a third of the time within 5m
    assert!((100..=230).contains(&two), "one: {one}, two: {two}");
}

#[test]
fn test_all_corner_random_no_corners() {
    let course = create_test_course_with(vec![], vec![], vec![]);
    let condition = parse_condition("all_corner_random==1").unwrap();
    assert!(roll(&condition, &course, &mut rand::rng()).is_none());
}

#[test]
fn test_all_corner_random_per_branch() {
    // Each branch gets its own segments, never more than 4
    let course = create_test_course();
    let condition = parse_condition(
        "distance_type==1&all_corner_random==1@distance_type==2&all_corner_random==1",
    )
    .unwrap();
    let mut rng = rand::rngs::StdRng::seed_from_u64(0);

    for _ in 0..100 {
        let rolls = roll_condition(&condition, &course, &mut rng);
        assert_eq!(rolls.len(), 2);
        for points in rolls {
            assert!((1..=4).contains(&points.unwrap().len()));
        }
    }
}

#[test]
fn test_random_lot_has_no_points() {
    let course = create_test_course();
    let condition = parse_condition("random_lot==100&phase_random==1").unwrap();
    let points = roll(&condition, &course, &mut rand::rng()).unwrap();

    // Only the phase point, nothing for the random lot
    assert_eq!(points.len(), 1);
    assert!((400.0..1600.0).contains(&points[0]));
}
