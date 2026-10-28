//! # Trigger Scheduler
//!
//! This module holds the data structures the scheduler indexes triggers in, so
//! that only the triggers that can fire are looked at on each frame.
//!
//! ## Trigger Types
//!
//! Each trigger ([`TriggerCondition`]) is stored where it can be checked
//! cheaply:
//!
//! - [`Segments`] (random variables): one window per segment in the position
//!   [`WindowQueue`]. Fires every frame while inside a segment.
//! - [`Window`] (position, time): a [`WindowQueue`]. Fires every frame while
//!   inside the range.
//! - [`Window`] (HP): a list. Fires every frame while inside the range.
//! - [`LastStraightEntry`]: a list. Fires once, when entering the last
//!   straight.
//! - [`RaceStart`] (out-of-gate skills): a list. Fires once, on the first frame
//!   the runner moves.
//! - [`AllOutSpurtStart`]: a list. Converted to a segment window when the spurt
//!   starts.
//! - [`AnySkillActivation`] / [`HealSkillActivation`]: lists. Fire on frames
//!   where a (heal) skill activated.
//! - [`BuffSkillActivation`]: a list. Fires on frames where any runner
//!   activated a buff, or the most popular runner activated any skill, after
//!   every runner checked her skills.
//!
//! **Window queues.** Position and time only increase, so their windows wait in
//! a pending list sorted by start, are checked while active, and are dropped
//! once the value passes their end. HP can recover, so HP windows are scanned
//! every frame. Bounds are widened by a small epsilon, so a window is never
//! skipped because of floating point error.
//!
//! **Checked once vs. retried.** The last straight entry and the race start
//! happen once per race, so their logics are checked once: if the guards fail
//! at that moment, the skill doesn't activate. Windows (random segments
//! included) and skill-activation events retry every time they fire, until the
//! logic succeeds; afterwards, an activated logic waits for the cooldown of
//! its skill alternative (kept by the runner) to end, and an unlocking logic
//! is never checked again. A segment that is crossed in a single frame is
//! still checked, as long as the runner lands inside it; one that is jumped
//! over entirely is missed.
//!
//! [`TriggerCondition`]: crate::trigger::skill_activation::TriggerCondition
//! [`Segments`]:
//!     crate::trigger::skill_activation::TriggerCondition::Segments
//! [`Window`]:
//!     crate::trigger::skill_activation::TriggerCondition::Window
//! [`LastStraightEntry`]:
//!     crate::trigger::skill_activation::TriggerCondition::LastStraightEntry
//! [`RaceStart`]:
//!     crate::trigger::skill_activation::TriggerCondition::RaceStart
//! [`AllOutSpurtStart`]:
//!     crate::trigger::skill_activation::TriggerCondition::AllOutSpurtStart
//! [`AnySkillActivation`]:
//!     crate::trigger::skill_activation::TriggerCondition::AnySkillActivation
//! [`HealSkillActivation`]:
//!     crate::trigger::skill_activation::TriggerCondition::HealSkillActivation
//! [`BuffSkillActivation`]:
//!     crate::trigger::skill_activation::TriggerCondition::BuffSkillActivation

use crate::course::Phase;
use crate::trigger::skill_activation::SkillActivationLogic;

/// Main scheduler coordinating all trigger types.
///
/// This scheduler stores [`SkillActivationLogic`] which separates triggers
/// from guards. When a trigger fires, we evaluate only the guard conditions.
#[derive(Debug, Default, Clone, PartialEq)]
pub(crate) struct TriggerScheduler {
    pub(crate) skill_logics: Vec<SkillActivationLogic>,

    // Precondition tracking - one-time unlocks, indexed by the precondition ID
    // stored in `SkillActivationLogic::precondition`.
    pub(crate) precondition_satisfied: Vec<bool>,
    // Position windows of precondition logics. Kept apart from regular windows
    // because preconditions are checked before the triggers they unlock.
    pub(crate) precondition_windows: WindowQueue,

    // Window triggers - checked every frame while within bounds (including random
    // segments). Position and time only increase, so their windows are queued; HP
    // can recover, so HP windows are scanned every frame.
    pub(crate) position_windows: WindowQueue,
    pub(crate) time_windows: WindowQueue,
    pub(crate) hp_windows: Vec<WindowTrigger>,

    // Last straight entry triggers (is_last_straight_onetime).
    pub(crate) last_straight_triggers: Vec<usize>,

    // Race start triggers (out-of-gate skills), checked once.
    pub(crate) race_start_triggers: Vec<usize>,

    // Deferred random triggers - scheduled as distance triggers once
    // the all-out spurt starts.
    pub(crate) all_out_spurt_triggers: Vec<usize>,

    // Activation-dependent triggers - checked when skills activate.
    pub(crate) any_activation_triggers: Vec<usize>,
    pub(crate) buff_activation_triggers: Vec<usize>,
    pub(crate) heal_activation_triggers: Vec<usize>,

    // Activation tracking.
    pub(crate) activation_counts: ActivationCounts,
}

impl TriggerScheduler {
    /// Remove every trigger of the given logics, so they're never checked.
    pub(crate) fn remove_triggers(&mut self, logic_indices: &[usize]) {
        if logic_indices.is_empty() {
            return;
        }
        let removed = |logic_idx: &usize| logic_indices.contains(logic_idx);

        self.precondition_windows
            .remove_if(|window| removed(&window.logic_idx));
        self.position_windows
            .remove_if(|window| removed(&window.logic_idx));
        self.time_windows
            .remove_if(|window| removed(&window.logic_idx));
        self.hp_windows.retain(|window| !removed(&window.logic_idx));
        for list in [
            &mut self.last_straight_triggers,
            &mut self.race_start_triggers,
            &mut self.all_out_spurt_triggers,
            &mut self.any_activation_triggers,
            &mut self.buff_activation_triggers,
            &mut self.heal_activation_triggers,
        ] {
            list.retain(|logic_idx| !removed(logic_idx));
        }
    }
}

// ========== WINDOW TRIGGERS (Checked every frame within range) ==========

/// Trigger that signals a skill check every frame while a value is within a
/// range (`variable>=min & variable<=max`).
///
/// The variable is implied by the collection holding the trigger (see
/// [`TriggerScheduler`]).
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct WindowTrigger {
    pub(crate) logic_idx: usize,
    pub(crate) min_threshold: f64,
    pub(crate) max_threshold: f64,
}

/// Window triggers over a variable that never decreases (position, time).
///
/// Windows wait in `pending` until their range starts, stay in `active` while
/// inside it, and are dropped once the value moves past their range. This way,
/// windows are only looked at while they matter.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct WindowQueue {
    /// Windows that haven't started yet, sorted by `min_threshold` in descending
    /// order (the next window to start is last).
    pending: Vec<WindowTrigger>,
    /// Windows whose range has started.
    active: Vec<WindowTrigger>,
}

impl WindowQueue {
    /// Add a window, at build time or during the race (e.g. random segments rolled
    /// once the all-out spurt starts).
    pub(crate) fn push(&mut self, trigger: WindowTrigger) {
        let idx = self
            .pending
            .partition_point(|t| t.min_threshold > trigger.min_threshold);
        self.pending.insert(idx, trigger);
    }

    /// Advance the queue to `value` and return the logic indices whose window
    /// contains it. Windows left behind are dropped. Bounds are widened by
    /// `epsilon`.
    pub(crate) fn advance(&mut self, value: f64, epsilon: f64) -> Vec<usize> {
        while self
            .pending
            .last()
            .is_some_and(|t| value >= t.min_threshold - epsilon)
        {
            self.active.push(self.pending.pop().unwrap());
        }

        self.active.retain(|t| value <= t.max_threshold + epsilon);
        self.active.iter().map(|t| t.logic_idx).collect()
    }

    /// Remove every window matching `predicate`.
    pub(crate) fn remove_if(&mut self, predicate: impl Fn(&WindowTrigger) -> bool) {
        self.pending.retain(|t| !predicate(t));
        self.active.retain(|t| !predicate(t));
    }

    /// Iterate over all windows still in the queue.
    #[cfg(test)]
    pub(crate) fn iter(&self) -> impl Iterator<Item = &WindowTrigger> {
        self.active.iter().chain(self.pending.iter().rev())
    }

    /// Get the number of windows still in the queue.
    #[cfg(test)]
    pub(crate) fn len(&self) -> usize {
        self.pending.len() + self.active.len()
    }

    /// Check if no window is left in the queue.
    #[cfg(test)]
    pub(crate) fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

// ========== ACTIVATION TRACKING ==========

#[derive(Default, Debug, Clone, PartialEq)]
pub(crate) struct ActivationCounts {
    pub(crate) all: usize,
    // Phase::RunUp + Phase::Early (sections 0-4).
    pub(crate) start: usize,
    // Phase::Mid (sections 4-16).
    pub(crate) middle: usize,
    // After half course (section 12-24).
    pub(crate) later_half: usize,
    // Phase::Late + Phase::Last (sections 16-24).
    pub(crate) end_after: usize,
    pub(crate) heal: usize,
    // Green (stat boost) skills.
    pub(crate) green: usize,
}

impl ActivationCounts {
    /// Increment activation counts based on current phase and position, given
    /// the course's `halfway` point.
    pub(crate) fn increment(&mut self, phase: Phase, position: f64, halfway: f64) {
        self.all += 1;

        match phase {
            Phase::RunUp | Phase::Early => self.start += 1,
            Phase::Mid => {
                self.middle += 1;
                // Check if we're past the halfway point.
                if position >= halfway {
                    self.later_half += 1;
                }
            }
            Phase::Late | Phase::Last => {
                self.end_after += 1;
                // Late/Last is always past halfway.
                self.later_half += 1;
            }
        }
    }

    /// Increment heal skill count.
    pub(crate) fn increment_heal(&mut self) {
        self.heal += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::course::Phase;

    /// Scheduler with logics 0 and 1 in every trigger collection.
    fn scheduler_with_two_logics() -> TriggerScheduler {
        let mut scheduler = TriggerScheduler::default();
        for logic_idx in [0, 1] {
            let min = 100.0 * logic_idx as f64;
            let window = || window(logic_idx, min, min + 10.0);
            scheduler.precondition_windows.push(window());
            scheduler.position_windows.push(window());
            scheduler.time_windows.push(window());
            scheduler.hp_windows.push(window());
            scheduler.last_straight_triggers.push(logic_idx);
            scheduler.race_start_triggers.push(logic_idx);
            scheduler.all_out_spurt_triggers.push(logic_idx);
            scheduler.any_activation_triggers.push(logic_idx);
            scheduler.buff_activation_triggers.push(logic_idx);
            scheduler.heal_activation_triggers.push(logic_idx);
        }
        // An active window, so that both queue stages are checked.
        scheduler.position_windows.advance(0.0, 0.0);
        scheduler
    }

    /// Logic indices of every trigger collection, by name.
    fn logics_by_collection(scheduler: &TriggerScheduler) -> Vec<(&str, Vec<usize>)> {
        let windows = |queue: &WindowQueue| queue.iter().map(|t| t.logic_idx).collect();
        vec![
            (
                "precondition_windows",
                windows(&scheduler.precondition_windows),
            ),
            ("position_windows", windows(&scheduler.position_windows)),
            ("time_windows", windows(&scheduler.time_windows)),
            (
                "hp_windows",
                scheduler.hp_windows.iter().map(|t| t.logic_idx).collect(),
            ),
            (
                "last_straight_triggers",
                scheduler.last_straight_triggers.clone(),
            ),
            ("race_start_triggers", scheduler.race_start_triggers.clone()),
            (
                "all_out_spurt_triggers",
                scheduler.all_out_spurt_triggers.clone(),
            ),
            (
                "any_activation_triggers",
                scheduler.any_activation_triggers.clone(),
            ),
            (
                "buff_activation_triggers",
                scheduler.buff_activation_triggers.clone(),
            ),
            (
                "heal_activation_triggers",
                scheduler.heal_activation_triggers.clone(),
            ),
        ]
    }

    #[test]
    fn test_remove_triggers_from_every_collection() {
        let mut scheduler = scheduler_with_two_logics();
        scheduler.remove_triggers(&[1]);

        for (name, logics) in logics_by_collection(&scheduler) {
            assert_eq!(logics, [0], "{name}");
        }
    }

    #[test]
    fn test_remove_no_triggers() {
        let mut scheduler = scheduler_with_two_logics();
        scheduler.remove_triggers(&[]);

        for (name, logics) in logics_by_collection(&scheduler) {
            assert_eq!(logics, [0, 1], "{name}");
        }
    }

    #[test]
    fn test_activation_counts_early_phase() {
        let mut counts = ActivationCounts::default();
        counts.increment(Phase::Early, 50.0, 1200.0);

        assert_eq!(counts.all, 1);
        assert_eq!(counts.start, 1);
        assert_eq!(counts.middle, 0);
        assert_eq!(counts.later_half, 0);
        assert_eq!(counts.end_after, 0);
    }

    #[test]
    fn test_activation_counts_run_up_counts_as_start() {
        let mut counts = ActivationCounts::default();
        counts.increment(Phase::RunUp, 10.0, 1250.0);

        assert_eq!(counts.all, 1);
        assert_eq!(counts.start, 1);
    }

    #[test]
    fn test_activation_counts_mid_phase_before_section_12() {
        let mut counts = ActivationCounts::default();
        // Halfway at 1200m (section 12 of 100m sections).
        counts.increment(Phase::Mid, 1000.0, 1200.0);

        assert_eq!(counts.all, 1);
        assert_eq!(counts.start, 0);
        assert_eq!(counts.middle, 1);
        assert_eq!(counts.later_half, 0);
        assert_eq!(counts.end_after, 0);
    }

    #[test]
    fn test_activation_counts_mid_phase_after_section_12() {
        let mut counts = ActivationCounts::default();
        // Halfway at 1200m (section 12 of 100m sections).
        counts.increment(Phase::Mid, 1300.0, 1200.0);

        assert_eq!(counts.all, 1);
        assert_eq!(counts.start, 0);
        assert_eq!(counts.middle, 1);
        assert_eq!(counts.later_half, 1);
        assert_eq!(counts.end_after, 0);
    }

    #[test]
    fn test_activation_counts_late_phase() {
        let mut counts = ActivationCounts::default();
        counts.increment(Phase::Late, 1800.0, 1200.0);

        assert_eq!(counts.all, 1);
        assert_eq!(counts.start, 0);
        assert_eq!(counts.middle, 0);
        assert_eq!(counts.later_half, 1);
        assert_eq!(counts.end_after, 1);
    }

    #[test]
    fn test_activation_counts_last_phase() {
        let mut counts = ActivationCounts::default();
        counts.increment(Phase::Last, 2200.0, 1200.0);

        assert_eq!(counts.all, 1);
        assert_eq!(counts.start, 0);
        assert_eq!(counts.middle, 0);
        assert_eq!(counts.later_half, 1);
        assert_eq!(counts.end_after, 1);
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

        assert_eq!(queue.advance(50.0, 0.0), vec![1]);

        let mut inside = queue.advance(160.0, 0.0);
        inside.sort_unstable();
        assert_eq!(inside, vec![0, 1, 2]);

        let mut inside = queue.advance(250.0, 0.0);
        inside.sort_unstable();
        assert_eq!(inside, vec![1, 2]);

        // Exited windows are dropped.
        assert_eq!(queue.len(), 2);
    }

    #[test]
    fn test_window_queue_drops_skipped_windows() {
        let mut queue = WindowQueue::default();
        queue.push(window(0, 100.0, 105.0));

        assert!(queue.advance(110.0, 0.0).is_empty());
        assert!(queue.is_empty());
    }

    #[test]
    fn test_window_queue_widens_bounds_by_epsilon() {
        let mut queue = WindowQueue::default();
        queue.push(window(0, 100.0, 200.0));

        assert_eq!(queue.advance(99.95, 0.1), vec![0]);
        assert_eq!(queue.advance(200.05, 0.1), vec![0]);
        assert!(queue.advance(200.2, 0.1).is_empty());
        assert!(queue.is_empty());
    }

    #[test]
    fn test_window_queue_remove_if() {
        let mut queue = WindowQueue::default();
        queue.push(window(0, 0.0, 100.0));
        queue.push(window(1, 50.0, 100.0));
        queue.push(window(2, 0.0, 100.0));
        // 0 and 2 active, 1 pending.
        queue.advance(10.0, 0.0);

        queue.remove_if(|w| w.logic_idx != 2);
        let remaining: Vec<_> = queue.iter().map(|w| w.logic_idx).collect();
        assert_eq!(remaining, vec![2]);
    }
}
