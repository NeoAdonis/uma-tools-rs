use super::*;

fn create_test_corner(start: f64, length: f64) -> Corner {
    Corner { start, length }
}

fn create_test_slope(start: f64, length: f64, slope: i32) -> Slope {
    Slope::new(start, length, slope)
}

fn create_test_straight(start: f64, end: f64) -> Straight {
    Straight {
        start,
        end,
        front_type: FrontType::Audience,
    }
}

#[test]
fn test_corner_entry() {
    let mut manager = CornerManager::new();
    let corners = vec![create_test_corner(100.0, 50.0)];

    // Before corner
    assert_eq!(manager.check_entry(50.0, &corners), None);

    // At corner start
    assert_eq!(manager.check_entry(100.0, &corners), Some(0));

    // Already in corner
    assert_eq!(manager.check_entry(125.0, &corners), None);
}

#[test]
fn test_corner_exit() {
    let mut manager = CornerManager::new();
    let corners = vec![create_test_corner(100.0, 50.0)];

    // Enter corner
    manager.check_entry(100.0, &corners);

    // Still in corner
    assert_eq!(manager.check_exit(125.0, &corners), None);

    // At corner end
    assert_eq!(manager.check_exit(150.0, &corners), Some(0));

    // Already exited
    assert_eq!(manager.check_exit(175.0, &corners), None);
}

#[test]
fn test_multiple_corners() {
    let mut manager = CornerManager::new();
    let corners = vec![
        create_test_corner(100.0, 50.0),
        create_test_corner(200.0, 30.0),
    ];

    // Enter first corner
    assert_eq!(manager.check_entry(100.0, &corners), Some(0));

    // Exit first corner
    assert_eq!(manager.check_exit(150.0, &corners), Some(0));

    // Enter second corner
    assert_eq!(manager.check_entry(200.0, &corners), Some(1));

    // Exit second corner
    assert_eq!(manager.check_exit(230.0, &corners), Some(1));
}

#[test]
fn test_slope_entry_uphill() {
    let mut manager = SlopeManager::new();
    let slopes = vec![create_test_slope(100.0, 50.0, 500)]; // Positive = uphill

    // Before slope
    assert_eq!(manager.check_entry(50.0, &slopes), None);

    // At slope start
    let entry = manager.check_entry(100.0, &slopes);
    assert!(entry.is_some());
    let (idx, slope_type) = entry.unwrap();
    assert_eq!(idx, 0);
    assert_eq!(slope_type, SlopeType::Uphill);
}

#[test]
fn test_slope_entry_downhill() {
    let mut manager = SlopeManager::new();
    let slopes = vec![create_test_slope(100.0, 50.0, -500)]; // Negative = downhill

    let entry = manager.check_entry(100.0, &slopes);
    assert!(entry.is_some());
    let (idx, slope_type) = entry.unwrap();
    assert_eq!(idx, 0);
    assert_eq!(slope_type, SlopeType::Downhill);
}

#[test]
fn test_slope_exit() {
    let mut manager = SlopeManager::new();
    let slopes = vec![create_test_slope(100.0, 50.0, 500)];

    // Enter slope
    manager.check_entry(100.0, &slopes);

    // Still on slope
    assert_eq!(manager.check_exit(125.0, &slopes), None);

    // At slope end
    assert_eq!(manager.check_exit(150.0, &slopes), Some(0));

    // Already exited
    assert_eq!(manager.check_exit(175.0, &slopes), None);
}

#[test]
fn test_straight_entry() {
    let mut manager = StraightManager::new();
    let straights = vec![create_test_straight(100.0, 150.0)];

    // Before straight
    assert_eq!(manager.check_entry(50.0, &straights), None);

    // At straight start
    assert_eq!(manager.check_entry(100.0, &straights), Some(0));

    // Already in straight
    assert_eq!(manager.check_entry(125.0, &straights), None);
}

#[test]
fn test_straight_exit() {
    let mut manager = StraightManager::new();
    let straights = vec![create_test_straight(100.0, 150.0)];

    // Enter straight
    manager.check_entry(100.0, &straights);

    // Still in straight
    assert_eq!(manager.check_exit(125.0, &straights), None);

    // At straight end
    assert_eq!(manager.check_exit(150.0, &straights), Some(0));

    // Already exited
    assert_eq!(manager.check_exit(175.0, &straights), None);
}

#[test]
fn test_multiple_straights() {
    let mut manager = StraightManager::new();
    let straights = vec![
        create_test_straight(100.0, 150.0),
        create_test_straight(200.0, 230.0),
    ];

    // Enter first straight
    assert_eq!(manager.check_entry(100.0, &straights), Some(0));

    // Exit first straight
    assert_eq!(manager.check_exit(150.0, &straights), Some(0));

    // Enter second straight
    assert_eq!(manager.check_entry(200.0, &straights), Some(1));

    // Exit second straight
    assert_eq!(manager.check_exit(230.0, &straights), Some(1));
}
