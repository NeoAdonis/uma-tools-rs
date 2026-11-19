//! Pins: random outcomes of a runner fixed instead of rolled, e.g. to replay a
//! race or to control some variables while others are left to chance.
//!
//! A pinned outcome is still rolled, then replaced, so that pinning an outcome
//! never changes another one rolled from the same seed. A pin takes precedence
//! over any [`Configuration`] setting for the same outcome.
//!
//! Pinned values are only rejected when they would break the simulation or
//! never take effect. Values the mechanics would never roll are kept, so that
//! pins can express what happened in a real race even where the simulator
//! disagrees.
//!
//! [`Configuration`]: crate::Configuration
use std::collections::HashMap;
use std::sync::Arc;

use crate::course::SECTION_COUNT;
use crate::course::geometry::Slope;
use crate::race::solver::RunnerId;
use crate::skill::Skill;
use crate::uma::EffectiveStyle;

/// Random outcomes of a runner fixed instead of rolled.
///
/// Unset ones are rolled as usual. See the [module docs](self) for how pins
/// apply.
///
/// # Examples
///
/// Pin the start delay of a runner, leaving everything else to chance:
///
/// ```
/// use uma_tools::race::pins::Pins;
///
/// let pins = Pins {
///     start_delay: Some(0.035),
///     ..Pins::default()
/// };
/// assert_eq!(pins.start_delay, Some(0.035));
/// ```
///
/// Make a skill fail its wit check, whatever her wit:
///
/// ```
/// use uma_tools::race::pins::{Pins, SkillPin};
///
/// let mut pins = Pins::default();
/// pins.skills.insert(100011, SkillPin::WitCheck(false));
/// ```
#[derive(Debug, Default, Clone, PartialEq)]
pub struct Pins {
    /// Start delay in seconds, as the runner leaves the gate.
    ///
    /// It replaces the rolled delay after out-of-gate skills change it.
    pub start_delay: Option<f64>,
    /// Rushing outcomes.
    pub rush: RushPins,
    /// Outcomes of her skills, by skill ID.
    pub skills: HashMap<i32, SkillPin>,
    /// Section speed modifiers in m/s, by section (0-indexed).
    ///
    /// Each is the random target speed modifier she rolls from her wit when
    /// the race starts (section 0) or upon entering the section (others).
    pub section_speed: [Option<f64>; SECTION_COUNT as usize],
    /// Force-in modifier in m/s.
    ///
    /// It's the target speed increase she rolls when the race starts, which
    /// applies in the Early race while she's away from the inner rail and
    /// blocked toward the inside. It includes her style's share, and applies
    /// before the 1st anniversary too.
    pub force_in: Option<f64>,
    /// Downhill mode outcomes, by slope index (see [`Course::slopes`]).
    ///
    /// Each list says whether she's in downhill mode after each check on that
    /// downhill, in order: the check upon entering it, then one on the first
    /// frame that begins a full second after the previous check, 17 frames
    /// later. Checks past the end of the list are rolled as usual.
    ///
    /// [`Course::slopes`]: crate::Course::slopes
    pub downhill: HashMap<usize, Vec<bool>>,
    /// Outcomes of her position keep wit checks, sorted by time.
    ///
    /// They're the checks that roll the chance of entering Speed Up or
    /// Overtake (front runners) or Pace Up (others). Each pin fixes the first
    /// such check at or after its time and is used at most once. Rushed
    /// runners don't roll these checks, so they use no pins. Pace Down and
    /// Pace Up Ex have no check to pin. [`Pins::positioning_thresholds`] pins
    /// the exit thresholds of Pace Up and Pace Down.
    pub positioning: Vec<PinnedWitCheck>,
    /// Exit thresholds of her Pace Up and Pace Down, sorted by time.
    ///
    /// Each pin fixes the first threshold rolled at or after its time and is
    /// used at most once. Rushed runners still use them.
    pub positioning_thresholds: Vec<PinnedThreshold>,
}

/// Outcome of a position keep wit check fixed instead of rolled (see
/// [`Pins::positioning`]).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PinnedWitCheck {
    /// Time since the race started, in seconds.
    ///
    /// The pin fixes the first check that rolls at or after it.
    pub time: f64,
    /// Whether the check passes, whatever her wit.
    pub passes: bool,
}

/// Exit threshold of a Pace Up or Pace Down fixed instead of rolled (see
/// [`Pins::positioning_thresholds`]).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PinnedThreshold {
    /// Time since the race started, in seconds.
    ///
    /// The pin fixes the first threshold rolled at or after it.
    pub time: f64,
    /// Distance behind the pacemaker at which the strategy ends, in meters.
    ///
    /// Pace Up ends once she's closer than it, Pace Down once she's farther.
    pub distance: f64,
}

/// Rushing outcomes of a runner fixed instead of rolled.
///
/// Apart from whether she rushes, they only apply if she does.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct RushPins {
    /// Whether she rushes at some point, whatever her chance of rushing.
    pub rushes: Option<bool>,
    /// Style she positions as while rushed.
    pub style: Option<EffectiveStyle>,
    /// Section (0-indexed) upon whose entry she starts rushing.
    ///
    /// It goes from 1 to 23, since runners start in section 0 rather than
    /// entering it.
    pub section: Option<u8>,
    /// Snap-out checks she fails before snapping out of the rush.
    ///
    /// Reaching its maximum duration still ends it.
    pub failed_checks: Option<u8>,
}

/// Outcome of a skill fixed instead of rolled.
#[derive(Debug, Clone, PartialEq)]
pub enum SkillPin {
    /// Whether the skill passes its wit check.
    ///
    /// It applies even if the skill has none or [`Configuration::wit_checks`]
    /// says otherwise. Her conditions still decide when it activates.
    ///
    /// [`Configuration::wit_checks`]: crate::Configuration::wit_checks
    WitCheck(bool),
    /// Every activation of the skill, fixed instead of triggered.
    ///
    /// They replace its conditions, wit check and trigger rolls: an empty list
    /// means it never activates. They go through the usual activation code
    /// (effects, cooldowns, activation counts...), even if the skill is in
    /// cooldown or its effects are still active.
    ///
    /// Skills that activate others (e.g. [`SkillType::ActivateRandomGold`])
    /// never pick a skill whose activations are pinned: its picks are pinned
    /// activations of their own.
    ///
    /// The skill's conditions are still checked, without activating it, to
    /// report where they disagree with the pins: pinned activations the
    /// conditions didn't allow (see [`SkillActivationRecord::conditions_met`]),
    /// and frames where the conditions say it should have activated but no
    /// activation is pinned (see [`Runner::missing_activations`]).
    ///
    /// [`SkillActivationRecord::conditions_met`]:
    ///     crate::trigger::SkillActivationRecord::conditions_met
    /// [`Runner::missing_activations`]:
    ///     crate::race::runner::Runner::missing_activations
    /// [`SkillType::ActivateRandomGold`]:
    ///     crate::skill::SkillType::ActivateRandomGold
    Activations(Vec<PinnedActivation>),
}

/// Activation of a skill fixed instead of triggered (see
/// [`SkillPin::Activations`]).
#[derive(Debug, Clone, PartialEq)]
pub struct PinnedActivation {
    /// Time since the race started, in seconds.
    ///
    /// Skills are checked at the end of each frame, and the skill activates on
    /// the first check at or after that time, once she has left the gate and
    /// before she finishes. Within that check, it goes in skill ID order among
    /// the other skills' activations, like the skill would by its conditions.
    /// Out-of-gate alternatives activate before the race, so their time is 0.
    pub time: f64,
    /// Index of the alternative that activates.
    pub alternative: usize,
    /// Runners her effects on others reach, instead of the usual targets.
    ///
    /// Her effects on herself ignore it. `None` picks targets as usual.
    pub targets: Option<Vec<RunnerId>>,
}

/// Pins taken when due: each is used at most once, by the first roll at or
/// after its time.
///
/// Pins with the same time are taken in the order they were given.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct PinQueue<T> {
    /// Pins by descending time, the next one last.
    items: Vec<(f64, T)>,
}

impl<T> Default for PinQueue<T> {
    fn default() -> Self {
        Self { items: Vec::new() }
    }
}

impl<T> PinQueue<T> {
    /// Returns a queue of the `(time, pin)` pairs in `items`, in any order.
    pub(crate) fn new(items: impl IntoIterator<Item = (f64, T)>) -> Self {
        let mut items: Vec<_> = items.into_iter().collect();
        // Stable and reversed, so that popping takes ties in order.
        items.sort_by(|a, b| a.0.total_cmp(&b.0));
        items.reverse();
        Self { items }
    }

    /// Takes the next pin if it's due at `time`.
    pub(crate) fn take_due(&mut self, time: f64) -> Option<T> {
        if self.items.last().is_some_and(|&(due, _)| due <= time) {
            self.items.pop().map(|(_, item)| item)
        } else {
            None
        }
    }

    /// Returns the pins not taken yet, in no particular order.
    pub(crate) fn iter(&self) -> impl Iterator<Item = &T> {
        self.items.iter().map(|(_, item)| item)
    }
}

impl Pins {
    /// Returns the pinned activations of the skill with ID `id`, if any.
    pub(crate) fn activations(&self, id: i32) -> Option<&[PinnedActivation]> {
        match self.skills.get(&id) {
            Some(SkillPin::Activations(activations)) => Some(activations),
            _ => None,
        }
    }

    /// Returns whether a failed wit check is ruled out for `skill`.
    ///
    /// It is if the skill has at least one pinned activation, or no wit check
    /// to fail.
    pub(crate) fn rules_out_failed_wit_check(&self, skill: &Skill) -> bool {
        self.activations(skill.id)
            .is_some_and(|activations| !activations.is_empty())
            || !skill.wit_check
    }

    /// Panics if a pinned value would break the simulation or never take effect.
    ///
    /// The pins are those of a runner with `skills` on a course with `slopes`.
    pub(crate) fn validate(&self, skills: &[Arc<Skill>], slopes: &[Slope]) {
        if let Some(delay) = self.start_delay {
            assert!(
                delay.is_finite() && delay >= 0.0,
                "Invalid start delay {delay}: expected a finite, non-negative number"
            );
        }
        if let Some(force_in) = self.force_in {
            assert!(
                force_in.is_finite(),
                "Invalid force-in modifier {force_in}: expected a finite number"
            );
        }
        for (section, speed) in self.section_speed.iter().enumerate() {
            if let Some(speed) = speed {
                assert!(
                    speed.is_finite(),
                    "Invalid section speed {speed} for section {section}: expected a finite number"
                );
            }
        }
        if let Some(section) = self.rush.section {
            let last = SECTION_COUNT - 1;
            assert!(
                (1..=last).contains(&section),
                "Invalid rush section {section}: expected 1 to {last}"
            );
        }
        for pin in &self.positioning {
            assert!(
                pin.time.is_finite() && pin.time >= 0.0,
                "Invalid positioning time {}: expected a finite, non-negative number",
                pin.time
            );
        }
        for pair in self.positioning.windows(2) {
            assert!(
                pair[0].time <= pair[1].time,
                "Positioning pins out of order: {} before {}",
                pair[0].time,
                pair[1].time
            );
        }
        for pin in &self.positioning_thresholds {
            assert!(
                pin.time.is_finite() && pin.time >= 0.0,
                "Invalid threshold time {}: expected a finite, non-negative number",
                pin.time
            );
            assert!(
                pin.distance.is_finite(),
                "Invalid threshold distance {}: expected a finite number",
                pin.distance
            );
        }
        for pair in self.positioning_thresholds.windows(2) {
            assert!(
                pair[0].time <= pair[1].time,
                "Threshold pins out of order: {} before {}",
                pair[0].time,
                pair[1].time
            );
        }
        for &slope in self.downhill.keys() {
            let is_downhill = slopes.get(slope).is_some_and(Slope::is_downhill);
            assert!(is_downhill, "No downhill {slope} to pin");
        }
        for (&id, pin) in &self.skills {
            let skill = skills.iter().find(|skill| skill.id == id);
            let skill = skill.unwrap_or_else(|| panic!("No skill {id} to pin"));
            if let SkillPin::Activations(activations) = pin {
                validate_activations(skill, activations);
            }
        }
    }

    /// Panics if a pinned target isn't among the `runner_count` runners of the
    /// race.
    pub(crate) fn validate_targets(&self, runner_count: usize) {
        let activations = self.skills.values().filter_map(|pin| match pin {
            SkillPin::Activations(activations) => Some(activations),
            SkillPin::WitCheck(_) => None,
        });
        let targets = activations.flatten().filter_map(|a| a.targets.as_ref());
        for &id in targets.flatten() {
            assert!(id.index() < runner_count, "No runner with {id:?} to target");
        }
    }
}

/// Panics if a pinned activation of `skill` would break the simulation or never
/// take effect.
fn validate_activations(skill: &Skill, activations: &[PinnedActivation]) {
    let id = skill.id;
    let mut out_of_gate = 0;
    for &PinnedActivation {
        time, alternative, ..
    } in activations
    {
        let count = skill.alternatives.len();
        assert!(
            alternative < count,
            "Invalid alternative {alternative} for skill {id}: expected 0 to {}",
            count - 1
        );
        if skill.alternatives[alternative].is_out_of_gate() {
            assert!(
                time == 0.0,
                "Invalid time {time} for skill {id}: out-of-gate alternatives activate at 0"
            );
            out_of_gate += 1;
        } else {
            assert!(
                time.is_finite() && time >= 0.0,
                "Invalid time {time} for skill {id}: expected a finite, non-negative number"
            );
        }
    }
    assert!(
        out_of_gate <= 1,
        "Skill {id} activates out of gate {out_of_gate} times: expected at most once"
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pin_queue_takes_each_pin_once_in_time_then_given_order() {
        let mut queue = PinQueue::new([(2.0, 'c'), (1.0, 'a'), (2.0, 'd'), (1.0, 'b')]);
        assert_eq!(queue.take_due(0.5), None);
        // Pins due at the same time, whenever they're taken, come in order.
        assert_eq!(queue.take_due(2.0), Some('a'));
        assert_eq!(queue.take_due(2.0), Some('b'));
        assert_eq!(queue.iter().count(), 2);
        assert_eq!(queue.take_due(2.0), Some('c'));
        assert_eq!(queue.take_due(2.0), Some('d'));
        assert_eq!(queue.take_due(f64::INFINITY), None);
    }
}
