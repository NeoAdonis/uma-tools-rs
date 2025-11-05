use super::super::random::*;
use crate::condition::parser::*;
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
        assert!(result.is_some());

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
