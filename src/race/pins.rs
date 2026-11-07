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
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SkillPin {
    /// Whether the skill passes its wit check, even if it has none or
    /// [`Configuration::wit_checks`] says otherwise. Her conditions still
    /// decide when it activates.
    ///
    /// [`Configuration::wit_checks`]: crate::Configuration::wit_checks
    WitCheck(bool),
}

impl Pins {
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
        for id in self.skills.keys() {
            assert!(
                skills.iter().any(|skill| skill.id == *id),
                "No skill {id} to pin"
            );
        }
    }
}
