use super::*;

fn create_test_corner(start: f64, length: f64) -> Corner {
    Corner { start, length }
}

#[test]
fn test_segment_bounds() {
    let corner = create_test_corner(100.0, 50.0);
    assert_eq!((corner.start(), corner.end()), (100.0, 150.0));

    let slope = Slope::new(200.0, 30.0, 1.5);
    assert_eq!((slope.start(), slope.end()), (200.0, 230.0));

    let straight = Straight {
        start: 300.0,
        end: 400.0,
        front_type: FrontType::Audience,
    };
    assert_eq!((straight.start(), straight.end()), (300.0, 400.0));
}

#[test]
fn test_segment_entry() {
    let mut manager = SegmentManager::default();
    let corners = vec![create_test_corner(100.0, 50.0)];

    // Before segment.
    assert_eq!(manager.check_entry(50.0, &corners), None);
    assert_eq!(manager.current(), None);

    // At segment start.
    assert_eq!(manager.check_entry(100.0, &corners), Some(0));
    assert_eq!(manager.current(), Some(0));

    // Already in segment.
    assert_eq!(manager.check_entry(125.0, &corners), None);
    assert_eq!(manager.current(), Some(0));
}

#[test]
fn test_segment_exit() {
    let mut manager = SegmentManager::default();
    let corners = vec![create_test_corner(100.0, 50.0)];

    // Not in a segment.
    assert_eq!(manager.check_exit(125.0, &corners), None);

    // Enter segment.
    manager.check_entry(100.0, &corners);

    // Still in segment.
    assert_eq!(manager.check_exit(125.0, &corners), None);

    // At segment end.
    assert_eq!(manager.check_exit(150.0, &corners), Some(0));
    assert_eq!(manager.current(), None);

    // Already exited.
    assert_eq!(manager.check_exit(175.0, &corners), None);
}

#[test]
fn test_multiple_segments() {
    let mut manager = SegmentManager::default();
    let corners = vec![
        create_test_corner(100.0, 50.0),
        create_test_corner(200.0, 30.0),
    ];

    // Enter first segment.
    assert_eq!(manager.check_entry(100.0, &corners), Some(0));

    // Exit first segment.
    assert_eq!(manager.check_exit(150.0, &corners), Some(0));

    // Between segments.
    assert_eq!(manager.check_entry(175.0, &corners), None);

    // Enter second segment.
    assert_eq!(manager.check_entry(200.0, &corners), Some(1));

    // Exit second segment.
    assert_eq!(manager.check_exit(230.0, &corners), Some(1));
}

#[test]
fn test_adjacent_segments() {
    let mut manager = SegmentManager::default();
    let corners = vec![
        create_test_corner(100.0, 50.0),
        create_test_corner(150.0, 50.0),
    ];
    manager.check_entry(100.0, &corners);

    // Exiting first lets the next one be entered at the same position.
    assert_eq!(manager.check_exit(150.0, &corners), Some(0));
    assert_eq!(manager.check_entry(150.0, &corners), Some(1));
}
