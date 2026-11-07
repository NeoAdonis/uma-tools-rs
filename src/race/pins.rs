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
use crate::race::solver::RunnerId;
use crate::skill::Skill;
use crate::uma::Style;

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
}

/// Rushing outcomes of a runner fixed instead of rolled (see
/// [`crate::race::rush`]). Apart from whether she rushes, they only apply if
/// she does.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct RushPins {
    /// Whether she rushes at some point, whatever her chance of rushing.
    pub rushes: Option<bool>,
    /// Style she positions as while rushed.
    pub style: Option<Style>,
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
    /// [`SkillType::ActivateRandomGold`]:
    ///     crate::skill::SkillType::ActivateRandomGold
    Activations(Vec<PinnedActivation>),
}

/// Activation of a skill fixed instead of triggered (see
/// [`SkillPin::Activations`]).
#[derive(Debug, Clone, PartialEq)]
pub struct PinnedActivation {
    /// Time since the race started, in seconds. The skill activates on the
    /// first skill check of a frame ending after that time, once she has left
    /// the gate and before she finishes. Out-of-gate alternatives activate
    /// before the race, so their time is 0.
    pub time: f64,
    /// Index of the alternative that activates.
    pub alternative: usize,
    /// Runners her effects on others reach, instead of the usual targets. Her
    /// effects on herself ignore it. `None` picks targets as usual.
    pub targets: Option<Vec<RunnerId>>,
}

impl Pins {
    /// Pinned activations of the skill with ID `id`, if they are.
    pub(crate) fn activations(&self, id: i32) -> Option<&[PinnedActivation]> {
        match self.skills.get(&id) {
            Some(SkillPin::Activations(activations)) => Some(activations),
            _ => None,
        }
    }

    /// Panic if a pinned value would break the simulation or never take effect,
    /// for a runner with `skills`.
    pub(crate) fn validate(&self, skills: &[Arc<Skill>]) {
        if let Some(delay) = self.start_delay {
            assert!(
                delay.is_finite() && delay >= 0.0,
                "Invalid start delay {delay}: expected a finite, non-negative number"
            );
        }
        if let Some(section) = self.rush.section {
            let last = SECTION_COUNT - 1;
            assert!(
                (1..=last).contains(&section),
                "Invalid rush section {section}: expected 1 to {last}"
            );
        }
        for (&id, pin) in &self.skills {
            let skill = skills.iter().find(|skill| skill.id == id);
            let skill = skill.unwrap_or_else(|| panic!("No skill {id} to pin"));
            if let SkillPin::Activations(activations) = pin {
                validate_activations(skill, activations);
            }
        }
    }

    /// Panic if a pinned target isn't among the `runner_count` runners of the
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

/// Panic if a pinned activation of `skill` would break the simulation or never
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
