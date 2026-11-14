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

/// Random outcomes of a runner fixed instead of rolled (see the
/// [module docs](self)). Unset ones are rolled as usual.
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
    /// Start delay in seconds, as the runner leaves the gate: it replaces the
    /// rolled delay after out-of-gate skills change it.
    pub start_delay: Option<f64>,
    /// Rushing outcomes.
    pub rush: RushPins,
    /// Outcomes of her skills, by skill ID.
    pub skills: HashMap<i32, SkillPin>,
    /// Section speed modifier in m/s, by section (0-indexed): the random
    /// target speed modifier she rolls from her wit when the race starts
    /// (section 0) and upon entering each other section.
    pub section_speed: [Option<f64>; SECTION_COUNT as usize],
    /// Force-in modifier in m/s: the target speed increase she rolls when the
    /// race starts, which applies in the Early race while she's away from the
    /// inner rail and blocked towards the inside. It includes her style's
    /// share, and applies before the 1st anniversary too.
    pub force_in: Option<f64>,
    /// Downhill mode outcomes, by slope index (see [`Course::slopes`]): whether
    /// she's in downhill mode after each check on that downhill, in order (the
    /// check upon entering it, then one every second). Checks past the end of
    /// the list are rolled as usual.
    ///
    /// [`Course::slopes`]: crate::Course::slopes
    pub downhill: HashMap<usize, Vec<bool>>,
}

/// Rushing outcomes of a runner fixed instead of rolled (see
/// [`crate::race::rush`]). Apart from whether she rushes, they only apply if
/// she does.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct RushPins {
    /// Whether she rushes at some point, whatever her chance of rushing.
    pub rushes: Option<bool>,
    /// Style she positions as while rushed.
    pub style: Option<EffectiveStyle>,
    /// Section (0-indexed) upon whose entry she starts rushing, from 1 to 23:
    /// runners start in section 0 rather than entering it.
    pub section: Option<u8>,
    /// Snap-out checks she fails before snapping out of the rush. Reaching its
    /// maximum duration still ends it.
    pub failed_checks: Option<u8>,
}

/// Outcome of a skill fixed instead of rolled.
#[derive(Debug, Clone, PartialEq)]
pub enum SkillPin {
    /// Whether the skill passes its wit check, even if it has none or
    /// [`Configuration::wit_checks`] says otherwise. Her conditions still
    /// decide when it activates.
    ///
    /// [`Configuration::wit_checks`]: crate::Configuration::wit_checks
    WitCheck(bool),
    /// Every activation of the skill, in place of its conditions, wit check and
    /// trigger rolls (none means it never activates). Activations go through
    /// the usual activation code (effects, cooldowns, activation counts...),
    /// even if the skill is in cooldown or its effects are still active.
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
    /// Time since the race started, in seconds. Skills are checked at the end
    /// of each frame, and the skill activates on the first check at or after
    /// that time, once she has left the gate and before she finishes.
    /// Out-of-gate alternatives activate before the race, so their time is 0.
    pub time: f64,
    /// Index of the alternative that activates.
    pub alternative: usize,
    /// Runners her effects on others reach, instead of the usual targets. Her
    /// effects on herself ignore it. `None` picks targets as usual.
    pub targets: Option<Vec<RunnerId>>,
}

impl Pins {
    /// Returns the section speed modifier of `section`: the pinned one, if any, or
    /// `rolled`.
    ///
    /// Sections past the goal can't be pinned.
    pub(crate) fn section_speed(&self, section: u8, rolled: f64) -> f64 {
        let pinned = self.section_speed.get(section as usize).copied();
        pinned.flatten().unwrap_or(rolled)
    }

    /// Returns whether she's in downhill mode after `check` (0-indexed) on the
    /// slope with index `slope`: the pinned outcome, if any, or `rolled`.
    pub(crate) fn downhill_mode(&self, slope: usize, check: usize, rolled: bool) -> bool {
        let pinned = self.downhill.get(&slope).and_then(|c| c.get(check));
        pinned.copied().unwrap_or(rolled)
    }

    /// Returns the pinned activations of the skill with ID `id`, if any.
    pub(crate) fn activations(&self, id: i32) -> Option<&[PinnedActivation]> {
        match self.skills.get(&id) {
            Some(SkillPin::Activations(activations)) => Some(activations),
            _ => None,
        }
    }

    /// Returns whether a failed wit check is ruled out for `skill`: it has at least
    /// one pinned activation, or it has no wit check to fail.
    pub(crate) fn rules_out_failed_wit_check(&self, skill: &Skill) -> bool {
        self.activations(skill.id)
            .is_some_and(|activations| !activations.is_empty())
            || !skill.wit_check
    }

    /// Panics if a pinned value would break the simulation or never take effect,
    /// for a runner with `skills` on a course with `slopes`.
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
