use crate::condition::parser::CompOp;
use crate::course::Phase;
use crate::trigger::skill_activation::{SkillActivationLogic, WindowVariable};
use std::cmp::Ordering;
use std::collections::BinaryHeap;

/// Main scheduler coordinating all trigger types.
///
/// This scheduler stores `SkillActivationLogic` which separates triggers
/// from guards. When a trigger fires, we evaluate only the guard conditions.
pub struct TriggerScheduler {
    pub skill_logics: Vec<SkillActivationLogic>,

    // Precondition tracking - one-time unlocks for skills with preconditions
    pub precondition_satisfied: Vec<bool>,

    // Monotonic triggers - use priority queues for O(log n) activation
    pub time_triggers: BinaryHeap<TimeTrigger>,
    pub distance_triggers: BinaryHeap<DistanceTrigger>,
    pub hp_triggers: BinaryHeap<HpTrigger>,
    pub phase_triggers: BinaryHeap<PhaseTransitionTrigger>,

    // Window triggers - checked every frame while within bounds
    pub window_triggers: Vec<WindowTrigger>,

    // Spatial triggers - tied to course geometry
    // Note: Corners and slopes use Window triggers (SpatialPosition)
    // Only straights are here for is_last_straight_onetime support
    pub straight_triggers: Vec<StraightTrigger>,

    // Continuous triggers - checked every step (minimized)
    pub continuous_triggers: Vec<usize>,

    // Deferred random triggers - scheduled as distance triggers once
    // Full-Throttle Sprint starts
    pub full_throttle_triggers: Vec<usize>,

    // Activation-dependent triggers - checked when skills activate
    pub any_activation_triggers: Vec<usize>,
    pub buff_activation_triggers: Vec<usize>,
    pub heal_activation_triggers: Vec<usize>,

    // Activation tracking
    pub activation_counts: ActivationCounts,
}

impl TriggerScheduler {
    pub fn new() -> Self {
        TriggerScheduler {
            skill_logics: Vec::new(),
            precondition_satisfied: Vec::new(),
            time_triggers: BinaryHeap::new(),
            distance_triggers: BinaryHeap::new(),
            hp_triggers: BinaryHeap::new(),
            phase_triggers: BinaryHeap::new(),
            window_triggers: Vec::new(),
            straight_triggers: Vec::new(),
            continuous_triggers: Vec::new(),
            full_throttle_triggers: Vec::new(),
            any_activation_triggers: Vec::new(),
            buff_activation_triggers: Vec::new(),
            heal_activation_triggers: Vec::new(),
            activation_counts: ActivationCounts::default(),
        }
    }
}

impl Default for TriggerScheduler {
    fn default() -> Self {
        Self::new()
    }
}

// ========== MONOTONIC TRIGGERS (Priority Queue Based) ==========

/// Trigger based on elapsed time (monotonic increasing).
#[derive(Clone, Debug)]
pub struct TimeTrigger {
    pub threshold_seconds: f64,
    pub logic_idx: usize,
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

/// Trigger based on distance. It fires once the runner's position reaches
/// `threshold_meters`.
///
/// All distance-based conditions (distance rate, remaining distance, furlongs,
/// random points) are converted to a position from the start when indexed.
#[derive(Clone, Debug)]
pub struct DistanceTrigger {
    /// Position from the start in meters.
    pub threshold_meters: f64,
    pub logic_idx: usize,
}

impl Ord for DistanceTrigger {
    fn cmp(&self, other: &Self) -> Ordering {
        // Min-heap: trigger earliest position first
        other
            .threshold_meters
            .partial_cmp(&self.threshold_meters)
            .unwrap_or(Ordering::Equal)
    }
}

impl PartialOrd for DistanceTrigger {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl PartialEq for DistanceTrigger {
    fn eq(&self, other: &Self) -> bool {
        self.threshold_meters == other.threshold_meters
    }
}

impl Eq for DistanceTrigger {}

/// Trigger based on HP percentage (generally decreasing).
#[derive(Clone, Debug)]
pub struct HpTrigger {
    pub threshold_percent: f64,
    pub logic_idx: usize,
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

/// Trigger based on phase transitions (monotonic increasing).
#[derive(Clone, Debug)]
pub struct PhaseTransitionTrigger {
    pub phase: Phase,
    pub boundary_distance: f64, // Pre-computed from section boundaries
    pub logic_indices: Vec<usize>,
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

// ========== WINDOW TRIGGERS (Continuous Checking Within Range) ==========

/// Trigger that signals a continuous skill check within a value range
/// (`variable>=min & variable<=max`).
/// More efficient than continuous triggers when we know the valid range in advance.
#[derive(Clone, Debug)]
pub struct WindowTrigger {
    pub logic_idx: usize,
    pub variable: WindowVariable,
    pub min_threshold: f64,
    pub max_threshold: f64,
}

// ========== SPATIAL TRIGGERS (Course Geometry Based) ==========

/// Triggers on specific straights.
///
/// Note: Only used for `is_last_straight_onetime` (one-time event trigger).
/// Other spatial features (corners, slopes, regular straights) use [`WindowTrigger`]s.
#[derive(Clone, Debug)]
pub struct StraightTrigger {
    pub straight_index: usize,
    pub is_last: bool,
    pub logic_indices: Vec<usize>,
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
    pub _all_team: usize, // TODO: Multi-uma stub
}

impl ActivationCounts {
    /// Increment activation counts based on current phase and position.
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

    /// Increment heal skill count.
    pub fn increment_heal(&mut self) {
        self.heal += 1;
    }

    /// Increment team skill count (multi-uma stub).
    pub fn _increment_team(&mut self) {
        // TODO: update when multi-uma is implemented
        self._all_team += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
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
    fn test_distance_trigger_ordering() {
        let mut heap = BinaryHeap::new();
        for (logic_idx, threshold_meters) in [1700.0, 200.0, 1000.0].into_iter().enumerate() {
            heap.push(DistanceTrigger {
                threshold_meters,
                logic_idx,
            });
        }

        // Should pop in increasing position order
        let order: Vec<f64> = std::iter::from_fn(|| heap.pop())
            .map(|t| t.threshold_meters)
            .collect();
        assert_eq!(order, vec![200.0, 1000.0, 1700.0]);
    }
}
