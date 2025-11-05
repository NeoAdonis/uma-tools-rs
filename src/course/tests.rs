use super::*;

#[test]
fn test_is_exchange_race() {
    // Create a minimal CourseData for testing
    let create_test_course = |track_id: u16| -> Course {
        let data = CourseData {
            distance: 2000,
            distance_type: DistanceType::Mile,
            surface: Track::Dirt,
            turn: Direction::Left,
            course_set_status: vec![],
            finish_time_min: 120000,
            finish_time_max: 150000,
            lane_max: 10000,
            corners: vec![],
            slopes: vec![],
            straights: vec![],
            race_track_id: track_id,
            _course: (),
        };
        Course::new_test(track_id, data)
    };

    // Exchange race tracks (should return true)
    assert!(create_test_course(10101).is_exchange_race()); // Ooi
    assert!(create_test_course(10103).is_exchange_race()); // Kawasaki
    assert!(create_test_course(10104).is_exchange_race()); // Funabashi
    assert!(create_test_course(10105).is_exchange_race()); // Morioka

    // Non-exchange race tracks (should return false)
    assert!(!create_test_course(10001).is_exchange_race()); // Sapporo
    assert!(!create_test_course(10006).is_exchange_race()); // Tokyo
    assert!(!create_test_course(10008).is_exchange_race()); // Kyoto
    assert!(!create_test_course(10201).is_exchange_race()); // Longchamp
}
