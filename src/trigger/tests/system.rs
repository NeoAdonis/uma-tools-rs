use super::super::system::*;
use crate::condition::parser::CompOp;
use crate::course::Phase;
use std::collections::BinaryHeap;

#[test]
fn test_activation_counts_early_phase() {
    let mut counts = ActivationCounts::default();
    counts.increment(Phase::Early, 50.0, 100.0);

    assert_eq!(counts.all, 1);
    assert_eq!(counts.start, 1);
    assert_eq!(counts.middle, 0);
    assert_eq!(counts.later_half, 0);
    assert_eq!(counts.end_after, 0);
}

#[test]
fn test_activation_counts_mid_phase_before_section_12() {
    let mut counts = ActivationCounts::default();
    // Section length 100, position 1000 = section 10 (before 12)
    counts.increment(Phase::Mid, 1000.0, 100.0);

    assert_eq!(counts.all, 1);
    assert_eq!(counts.start, 0);
    assert_eq!(counts.middle, 1);
    assert_eq!(counts.later_half, 0);
    assert_eq!(counts.end_after, 0);
}

#[test]
fn test_activation_counts_mid_phase_after_section_12() {
    let mut counts = ActivationCounts::default();
    // Section length 100, position 1300 = section 13 (after 12)
    counts.increment(Phase::Mid, 1300.0, 100.0);

    assert_eq!(counts.all, 1);
    assert_eq!(counts.start, 0);
    assert_eq!(counts.middle, 1);
    assert_eq!(counts.later_half, 1);
    assert_eq!(counts.end_after, 0);
}

#[test]
fn test_activation_counts_late_phase() {
    let mut counts = ActivationCounts::default();
    counts.increment(Phase::Late, 1800.0, 100.0);

    assert_eq!(counts.all, 1);
    assert_eq!(counts.start, 0);
    assert_eq!(counts.middle, 0);
    assert_eq!(counts.later_half, 1);
    assert_eq!(counts.end_after, 1);
}

#[test]
fn test_activation_counts_last_phase() {
    let mut counts = ActivationCounts::default();
    counts.increment(Phase::Last, 2200.0, 100.0);

    assert_eq!(counts.all, 1);
    assert_eq!(counts.start, 0);
    assert_eq!(counts.middle, 0);
    assert_eq!(counts.later_half, 1);
    assert_eq!(counts.end_after, 1);
}

#[test]
fn test_time_trigger_ordering() {
    let trigger1 = TimeTrigger {
        threshold_seconds: 10.0,
        logic_idx: 0,
        operator: CompOp::GreaterEqual,
    };
    let trigger2 = TimeTrigger {
        threshold_seconds: 5.0,
        logic_idx: 1,
        operator: CompOp::GreaterEqual,
    };

    let mut heap = BinaryHeap::new();
    heap.push(trigger1);
    heap.push(trigger2);

    // Should pop trigger2 first (5.0 < 10.0)
    let first = heap.pop().unwrap();
    assert_eq!(first.threshold_seconds, 5.0);
}

#[test]
fn test_distance_trigger_ordering_remain() {
    let trigger1 = DistanceTrigger {
        threshold_meters: 200.0,
        logic_idx: 0,
        operator: CompOp::LessEqual,
        trigger_type: DistanceTriggerType::RemainDistance,
    };
    let trigger2 = DistanceTrigger {
        threshold_meters: 500.0,
        logic_idx: 1,
        operator: CompOp::LessEqual,
        trigger_type: DistanceTriggerType::RemainDistance,
    };

    let mut heap = BinaryHeap::new();
    heap.push(trigger1);
    heap.push(trigger2);

    // Should pop trigger2 first (500.0m remaining happens BEFORE 200.0m remaining)
    let first = heap.pop().unwrap();
    assert_eq!(first.threshold_meters, 500.0);
}
