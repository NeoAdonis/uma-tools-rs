use super::*;
use crate::course::geometry::Corner;

fn transition(exited: Option<usize>, entered: Option<usize>) -> Transition {
    Transition { exited, entered }
}

#[test]
fn test_segment_entry() {
    let mut manager = SegmentManager::default();
    let corners = vec![Corner {
        start: 100.0,
        length: 50.0,
    }];

    // Before segment.
    assert_eq!(manager.update(50.0, &corners), transition(None, None));
    assert_eq!(manager.current(), None);

    // At segment start.
    assert_eq!(manager.update(100.0, &corners), transition(None, Some(0)));
    assert_eq!(manager.current(), Some(0));

    // Already in segment.
    assert_eq!(manager.update(125.0, &corners), transition(None, None));
    assert_eq!(manager.current(), Some(0));
}

#[test]
fn test_segment_exit() {
    let mut manager = SegmentManager::default();
    let corners = vec![Corner {
        start: 100.0,
        length: 50.0,
    }];
    manager.update(100.0, &corners);

    // At segment end.
    assert_eq!(manager.update(150.0, &corners), transition(Some(0), None));
    assert_eq!(manager.current(), None);

    // Already exited.
    assert_eq!(manager.update(175.0, &corners), transition(None, None));
}

#[test]
fn test_adjacent_segments() {
    let mut manager = SegmentManager::default();
    let corners = vec![
        Corner {
            start: 100.0,
            length: 50.0,
        },
        Corner {
            start: 150.0,
            length: 50.0,
        },
    ];
    manager.update(100.0, &corners);

    // Exiting first lets the next one be entered in the same update.
    assert_eq!(
        manager.update(150.0, &corners),
        transition(Some(0), Some(1))
    );
    assert_eq!(manager.current(), Some(1));
}

#[test]
fn test_segments_jumped_over() {
    let mut manager = SegmentManager::default();
    let corners = vec![
        Corner {
            start: 100.0,
            length: 50.0,
        },
        Corner {
            start: 200.0,
            length: 50.0,
        },
        Corner {
            start: 300.0,
            length: 50.0,
        },
    ];

    // The first two are passed over entirely, so only the last is reported.
    assert_eq!(manager.update(310.0, &corners), transition(None, Some(2)));
    assert_eq!(manager.current(), Some(2));
    assert_eq!(manager.update(350.0, &corners), transition(Some(2), None));
    assert_eq!(manager.current(), None);
}

#[test]
fn test_segment_not_reentered() {
    let mut manager = SegmentManager::default();
    let corners = vec![
        Corner {
            start: 100.0,
            length: 50.0,
        },
        Corner {
            start: 200.0,
            length: 50.0,
        },
    ];
    manager.update(120.0, &corners);
    manager.update(160.0, &corners);

    // Only the segments ahead can be entered, and only once.
    assert_eq!(manager.update(210.0, &corners), transition(None, Some(1)));
    assert_eq!(manager.update(210.0, &corners), transition(None, None));
    assert_eq!(manager.update(250.0, &corners), transition(Some(1), None));
    assert_eq!(manager.update(260.0, &corners), transition(None, None));
}
