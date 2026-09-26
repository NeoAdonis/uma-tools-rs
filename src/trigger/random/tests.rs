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
        vec![Straight {
            start: 0.0,
            end: 100.0,
            front_type: FrontType::Audience,
        }],
    )
}

fn create_test_course_with_straights(straights: Vec<Straight>) -> Course {
    create_test_course_with(create_test_course().corners.clone(), straights)
}

fn create_test_course_with(corners: Vec<Corner>, straights: Vec<Straight>) -> Course {
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
            slopes: vec![Slope::new(800.0, 100.0, 500)],
            straights,
            race_track_id: 1,
            lane_max: 10000,
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

    assert!(result.is_some());
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
    assert!(result.is_some());
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
        let result = roll_condition(&condition, &course, &mut rng);
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
        let activation = roll_condition(&condition, &course, &mut rng).unwrap();
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

    assert!(roll_condition(&condition, &course, &mut rng).is_none());
}

#[test]
fn test_phase_first_half_straight_random_invalid_phase() {
    let course = create_test_course();
    let condition = parse_condition("phase_first_half_straight_random==4").unwrap();
    let mut rng = rand::rng();

    assert!(roll_condition(&condition, &course, &mut rng).is_none());
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
                let dist = roll_condition(&condition, &course, &mut rng).unwrap()[0];
                assert!(
                    (start..end).contains(&dist),
                    "{var}=={phase}: distance {dist} not in [{start}, {end})"
                );
            }
        }

        // Invalid phase fails the roll
        let condition = parse_condition(&format!("{var}==4")).unwrap();
        assert!(
            roll_condition(&condition, &course, &mut rng).is_none(),
            "{var}==4"
        );
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
    );
    let mut rng = rand::rngs::StdRng::seed_from_u64(0);

    for (phase, (start, end)) in [(0, (300.0, 400.0)), (1, (400.0, 700.0))] {
        let condition = parse_condition(&format!("phase_corner_random=={phase}")).unwrap();
        for _ in 0..50 {
            let dist = roll_condition(&condition, &course, &mut rng).unwrap()[0];
            assert!(
                (start..=end).contains(&dist),
                "phase {phase}: distance {dist} not in [{start}, {end}]"
            );
        }
    }

    // No corner in Late phase fails the roll
    let condition = parse_condition("phase_corner_random==2").unwrap();
    assert!(roll_condition(&condition, &course, &mut rng).is_none());
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
        let dist = roll_condition(&condition, &course, &mut rng).unwrap()[0];
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
    assert!(roll_condition(&condition, &course, &mut rng).is_none());
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
        let dist = roll_condition(&condition, &course, &mut rng).unwrap()[0];
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
    assert!(roll_condition(&condition, &course, &mut rng).is_none());
}
