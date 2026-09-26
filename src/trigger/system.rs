use crate::condition::parser::CompOp;
use crate::course::Phase;
use crate::trigger::skill_activation::SkillActivationLogic;
use std::cmp::Ordering;
use std::collections::BinaryHeap;

/// Main scheduler coordinating all trigger types.
///
/// This scheduler stores [`SkillActivationLogic`] which separates triggers
/// from guards. When a trigger fires, we evaluate only the guard conditions.
pub struct TriggerScheduler {
    pub skill_logics: Vec<SkillActivationLogic>,

    // Precondition tracking - one-time unlocks, indexed by the precondition ID
    // stored in `SkillActivationLogic::precondition`
    pub precondition_satisfied: Vec<bool>,
    // Position windows of precondition logics. Kept apart from regular windows
    // because preconditions are checked at the start of each frame, before any
    // other trigger
    pub precondition_windows: WindowQueue,

    // Monotonic triggers - use priority queues for O(log n) activation
    pub time_triggers: BinaryHeap<TimeTrigger>,
    pub distance_triggers: BinaryHeap<DistanceTrigger>,
    pub hp_triggers: BinaryHeap<HpTrigger>,
    pub phase_triggers: BinaryHeap<PhaseTransitionTrigger>,

    // Window triggers - checked every frame while within bounds
    // Position and time only increase, so their windows are queued; HP can
    // recover, so HP windows are scanned every frame
    pub position_windows: WindowQueue,
    pub time_windows: WindowQueue,
    pub hp_windows: Vec<WindowTrigger>,

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
            precondition_windows: WindowQueue::default(),
            time_triggers: BinaryHeap::new(),
            distance_triggers: BinaryHeap::new(),
            hp_triggers: BinaryHeap::new(),
            phase_triggers: BinaryHeap::new(),
            position_windows: WindowQueue::default(),
            time_windows: WindowQueue::default(),
            hp_windows: Vec::new(),
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
///
/// The variable is implied by the collection holding the trigger (see
/// [`TriggerScheduler`]).
#[derive(Clone, Debug)]
pub struct WindowTrigger {
    pub logic_idx: usize,
    pub min_threshold: f64,
    pub max_threshold: f64,
}

/// Window triggers over a variable that never decreases (position, time).
///
/// Windows wait in `pending` until their range starts, stay in `active` while
/// inside it, and are dropped once the value moves past their range. This way,
/// windows are only looked at while they matter.
#[derive(Clone, Debug, Default)]
pub struct WindowQueue {
    /// Windows that haven't started yet, sorted by `min_threshold` in descending
    /// order (the next window to start is last).
    pending: Vec<WindowTrigger>,
    /// Windows whose range has started.
    active: Vec<WindowTrigger>,
}

/// Result of advancing a [`WindowQueue`] to a new value.
#[derive(Debug, Default)]
pub struct WindowUpdate {
    /// Logic indices whose window contains the current value.
    pub inside: Vec<usize>,
    /// Logic indices whose window was left (or skipped over) in this update.
    pub exited: Vec<usize>,
}

impl WindowQueue {
    /// Add a window. Intended for build time.
    pub fn push(&mut self, trigger: WindowTrigger) {
        let idx = self
            .pending
            .partition_point(|t| t.min_threshold > trigger.min_threshold);
        self.pending.insert(idx, trigger);
    }

    /// Advance the queue to `value` and report which windows contain it and
    /// which ones were left behind. Bounds are widened by `epsilon`.
    pub fn advance(&mut self, value: f64, epsilon: f64) -> WindowUpdate {
        while self
            .pending
            .last()
            .is_some_and(|t| value >= t.min_threshold - epsilon)
        {
            self.active.push(self.pending.pop().unwrap());
        }

        let mut update = WindowUpdate::default();
        self.active.retain(|t| {
            if value > t.max_threshold + epsilon {
                update.exited.push(t.logic_idx);
                false
            } else {
                update.inside.push(t.logic_idx);
                true
            }
        });
        update
    }

    /// Remove every window matching `predicate`.
    pub fn remove_if(&mut self, predicate: impl Fn(&WindowTrigger) -> bool) {
        self.pending.retain(|t| !predicate(t));
        self.active.retain(|t| !predicate(t));
    }

    /// Iterate over all windows still in the queue.
    pub fn iter(&self) -> impl Iterator<Item = &WindowTrigger> {
        self.active.iter().chain(self.pending.iter().rev())
    }

    pub fn len(&self) -> usize {
        self.pending.len() + self.active.len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
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

    fn window(logic_idx: usize, min_threshold: f64, max_threshold: f64) -> WindowTrigger {
        WindowTrigger {
            logic_idx,
            min_threshold,
            max_threshold,
        }
    }

    #[test]
    fn test_window_queue_reports_windows_containing_value() {
        let mut queue = WindowQueue::default();
        queue.push(window(0, 100.0, 200.0));
        queue.push(window(1, 0.0, f64::INFINITY));
        queue.push(window(2, 150.0, 300.0));

        let update = queue.advance(50.0, 0.0);
        assert_eq!(update.inside, vec![1]);
        assert!(update.exited.is_empty());

        let mut update = queue.advance(160.0, 0.0);
        update.inside.sort_unstable();
        assert_eq!(update.inside, vec![0, 1, 2]);

        let mut update = queue.advance(250.0, 0.0);
        update.inside.sort_unstable();
        assert_eq!(update.inside, vec![1, 2]);
        assert_eq!(update.exited, vec![0]);

        // Exited windows are dropped
        assert_eq!(queue.len(), 2);
    }

    #[test]
    fn test_window_queue_reports_skipped_windows_as_exited() {
        let mut queue = WindowQueue::default();
        queue.push(window(0, 100.0, 105.0));

        let update = queue.advance(110.0, 0.0);
        assert!(update.inside.is_empty());
        assert_eq!(update.exited, vec![0]);
        assert!(queue.is_empty());
    }

    #[test]
    fn test_window_queue_widens_bounds_by_epsilon() {
        let mut queue = WindowQueue::default();
        queue.push(window(0, 100.0, 200.0));

        assert_eq!(queue.advance(99.95, 0.1).inside, vec![0]);
        assert_eq!(queue.advance(200.05, 0.1).inside, vec![0]);
        assert_eq!(queue.advance(200.2, 0.1).exited, vec![0]);
    }

    #[test]
    fn test_window_queue_remove_if() {
        let mut queue = WindowQueue::default();
        queue.push(window(0, 0.0, 100.0));
        queue.push(window(1, 50.0, 100.0));
        queue.push(window(2, 0.0, 100.0));
        queue.advance(10.0, 0.0); // 0 and 2 active, 1 pending

        queue.remove_if(|w| w.logic_idx != 2);
        let remaining: Vec<_> = queue.iter().map(|w| w.logic_idx).collect();
        assert_eq!(remaining, vec![2]);
    }
}
