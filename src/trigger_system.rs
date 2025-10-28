use crate::condition_parser::CompOp;
use crate::course::{Corner, Phase, Slope};
use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashMap, HashSet};

/// Main scheduler coordinating all trigger types
///
/// This scheduler stores `SkillActivationLogic` which separates triggers
/// from guards. When a trigger fires, we evaluate only the guard conditions.
pub struct TriggerScheduler {
    pub skill_logics: Vec<crate::skill_activation::SkillActivationLogic>,

    // Monotonic triggers - use priority queues for O(log n) activation
    pub time_triggers: BinaryHeap<TimeTrigger>,
    pub distance_triggers: BinaryHeap<DistanceTrigger>,
    pub hp_triggers: BinaryHeap<HpTrigger>,
    pub phase_triggers: BinaryHeap<PhaseTransitionTrigger>,

    // Spatial triggers - tied to course geometry
    pub corner_entry_triggers: Vec<CornerTransitionTrigger>,
    pub corner_exit_triggers: Vec<CornerTransitionTrigger>,
    pub slope_entry_triggers: Vec<SlopeTransitionTrigger>,
    pub slope_exit_triggers: Vec<SlopeTransitionTrigger>,
    pub straight_triggers: Vec<StraightTrigger>,

    // One-time triggers - fire once then mark as exhausted
    pub onetime_triggers: Vec<OnetimeTrigger>,

    // Continuous triggers - checked every step (minimized)
    pub continuous_triggers: Vec<usize>,

    // Random triggers - pre-rolled at race start
    pub random_activations: HashMap<usize, RandomActivation>,

    // Activation tracking
    pub activation_counts: ActivationCounts,
    pub activated_skill_ids: HashSet<u32>,
}

impl TriggerScheduler {
    pub fn new() -> Self {
        TriggerScheduler {
            skill_logics: Vec::new(),
            time_triggers: BinaryHeap::new(),
            distance_triggers: BinaryHeap::new(),
            hp_triggers: BinaryHeap::new(),
            phase_triggers: BinaryHeap::new(),
            corner_entry_triggers: Vec::new(),
            corner_exit_triggers: Vec::new(),
            slope_entry_triggers: Vec::new(),
            slope_exit_triggers: Vec::new(),
            straight_triggers: Vec::new(),
            onetime_triggers: Vec::new(),
            continuous_triggers: Vec::new(),
            random_activations: HashMap::new(),
            activation_counts: ActivationCounts::default(),
            activated_skill_ids: HashSet::new(),
        }
    }
}

impl Default for TriggerScheduler {
    fn default() -> Self {
        Self::new()
    }
}

// ========== MONOTONIC TRIGGERS (Priority Queue Based) ==========

/// Triggers based on elapsed time (monotonic increasing)
#[derive(Clone, Debug)]
pub struct TimeTrigger {
    pub threshold_seconds: f64,
    pub skill_index: usize,
    pub operator: CompOp,
}

impl Ord for TimeTrigger {
    fn cmp(&self, other: &Self) -> Ordering {
        // Min-heap: trigger earliest time first
        other
            .threshold_seconds
            .partial_cmp(&self.threshold_seconds)
            .unwrap_or(Ordering::Equal)
    }
}

impl PartialOrd for TimeTrigger {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl PartialEq for TimeTrigger {
    fn eq(&self, other: &Self) -> bool {
        self.threshold_seconds == other.threshold_seconds
    }
}

impl Eq for TimeTrigger {}

/// Triggers based on distance (both remaining and covered)
#[derive(Clone, Debug)]
pub struct DistanceTrigger {
    pub threshold_meters: f64,
    pub skill_index: usize,
    pub operator: CompOp,
    pub trigger_type: DistanceTriggerType,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum DistanceTriggerType {
    RemainDistance,  // Decreasing: triggers when remain_distance <= threshold
    DistanceRate,    // Increasing: triggers when position >= threshold
    Furlong,         // Increasing: triggers when position >= threshold (in furlongs)
}

impl Ord for DistanceTrigger {
    fn cmp(&self, other: &Self) -> Ordering {
        match (self.trigger_type, other.trigger_type) {
            (DistanceTriggerType::RemainDistance, DistanceTriggerType::RemainDistance) => {
                // For remaining distance, LARGER values happen earlier in race
                // 500m remaining happens BEFORE 200m remaining
                // BinaryHeap is max-heap, so we want normal ordering (larger pops first)
                self.threshold_meters
                    .partial_cmp(&other.threshold_meters)
                    .unwrap_or(Ordering::Equal)
            }
            (DistanceTriggerType::DistanceRate, DistanceTriggerType::DistanceRate)
            | (DistanceTriggerType::Furlong, DistanceTriggerType::Furlong) => {
                // For distance covered, SMALLER values happen earlier in race
                // 100m covered happens BEFORE 500m covered
                // BinaryHeap is max-heap, so we want reverse ordering (smaller pops first)
                other
                    .threshold_meters
                    .partial_cmp(&self.threshold_meters)
                    .unwrap_or(Ordering::Equal)
            }
            _ => Ordering::Equal,
        }
    }
}

impl PartialOrd for DistanceTrigger {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl PartialEq for DistanceTrigger {
    fn eq(&self, other: &Self) -> bool {
        self.threshold_meters == other.threshold_meters && self.trigger_type == other.trigger_type
    }
}

impl Eq for DistanceTrigger {}

/// Triggers based on HP percentage (generally decreasing)
#[derive(Clone, Debug)]
pub struct HpTrigger {
    pub threshold_percent: f64,
    pub skill_index: usize,
    pub operator: CompOp,
}

impl Ord for HpTrigger {
    fn cmp(&self, other: &Self) -> Ordering {
        // Min-heap: check higher HP thresholds first (100% before 50%)
        self.threshold_percent
            .partial_cmp(&other.threshold_percent)
            .unwrap_or(Ordering::Equal)
    }
}

impl PartialOrd for HpTrigger {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl PartialEq for HpTrigger {
    fn eq(&self, other: &Self) -> bool {
        self.threshold_percent == other.threshold_percent
    }
}

impl Eq for HpTrigger {}

/// Triggers based on phase transitions (monotonic increasing)
#[derive(Clone, Debug)]
pub struct PhaseTransitionTrigger {
    pub phase: Phase,
    pub boundary_distance: f64, // Pre-computed from section boundaries
    pub skill_indices: Vec<usize>,
}

impl Ord for PhaseTransitionTrigger {
    fn cmp(&self, other: &Self) -> Ordering {
        // Min-heap: trigger earlier phases first
        other
            .boundary_distance
            .partial_cmp(&self.boundary_distance)
            .unwrap_or(Ordering::Equal)
    }
}

impl PartialOrd for PhaseTransitionTrigger {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl PartialEq for PhaseTransitionTrigger {
    fn eq(&self, other: &Self) -> bool {
        self.boundary_distance == other.boundary_distance
    }
}

impl Eq for PhaseTransitionTrigger {}

// ========== SPATIAL TRIGGERS (Course Geometry Based) ==========

/// Triggers when entering/exiting a corner
#[derive(Clone, Debug)]
pub struct CornerTransitionTrigger {
    pub corner_index: usize,
    pub corner: Corner, // Reference to actual corner geometry
    pub skill_indices: Vec<usize>,
}

/// Triggers when entering/exiting a slope
#[derive(Clone, Debug)]
pub struct SlopeTransitionTrigger {
    pub slope_index: usize,
    pub slope: Slope, // Reference to actual slope geometry
    pub slope_type: SlopeType,
    pub skill_indices: Vec<usize>,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SlopeType {
    Uphill,
    Downhill,
    Any,
}

/// Triggers when on specific straights
#[derive(Clone, Debug)]
pub struct StraightTrigger {
    pub straight_index: usize,
    pub is_final: bool,
    pub skill_indices: Vec<usize>,
}

// ========== ONE-TIME TRIGGERS ==========

/// One-time triggers that fire once and mark as activated
#[derive(Clone, Debug)]
pub struct OnetimeTrigger {
    pub skill_index: usize, // Index into TriggerScheduler.skill_logics
    pub trigger_type: OnetimeType,
    pub activated: bool,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum OnetimeType {
    LastStraightEntry,   // is_last_straight_onetime
    PositionChange(i32), // change_order_onetime (delta)
    HpEmpty,             // is_hp_empty_onetime
}

// ========== RANDOM TRIGGERS ==========

#[derive(Clone, Debug)]
pub struct RandomActivation {
    pub activation_distance: f64,
    pub phase_restriction: Option<Phase>,
    pub corner_restriction: Option<usize>,
    pub slope_restriction: Option<SlopeType>,
    pub activated: bool,
}

// ========== ACTIVATION TRACKING ==========

#[derive(Default, Debug, Clone)]
pub struct ActivationCounts {
    pub all: usize,
    pub start: usize,      // Phase::Early (sections 0-4)
    pub middle: usize,     // Phase::Mid (sections 4-16)
    pub later_half: usize, // After half course (section 12-24)
    pub end_after: usize,  // Phase::Late + Phase::Last (sections 16-24)
    pub heal: usize,
    pub all_team: usize, // TODO: Multi-uma stub
}

impl ActivationCounts {
    /// Increment activation counts based on current phase and position
    pub fn increment(&mut self, phase: Phase, position: f64, section_length: f64) {
        self.all += 1;

        match phase {
            Phase::Early => self.start += 1,
            Phase::Mid => {
                self.middle += 1;
                // Check if we're past section 12 (later half)
                if position >= section_length * 12.0 {
                    self.later_half += 1;
                }
            }
            Phase::Late | Phase::Last => {
                self.end_after += 1;
                self.later_half += 1; // Late/Last is always in later half
            }
        }
    }

    /// Increment heal skill count
    pub fn increment_heal(&mut self) {
        self.heal += 1;
    }

    /// Increment team skill count (multi-uma stub)
    pub fn increment_team(&mut self) {
        self.all_team += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
            skill_index: 0,
            operator: CompOp::GreaterEqual,
        };
        let trigger2 = TimeTrigger {
            threshold_seconds: 5.0,
            skill_index: 1,
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
            skill_index: 0,
            operator: CompOp::LessEqual,
            trigger_type: DistanceTriggerType::RemainDistance,
        };
        let trigger2 = DistanceTrigger {
            threshold_meters: 500.0,
            skill_index: 1,
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
}
