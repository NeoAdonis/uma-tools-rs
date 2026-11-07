//! Pins: random outcomes of a runner fixed instead of rolled, e.g. to replay a
//! race or to control some variables while others are left to chance.
//!
//! A pinned outcome is still rolled, then replaced, so that pinning an outcome
//! never changes another one rolled from the same seed. A pin takes precedence
//! over any [`Configuration`] setting for the same outcome.
//!
//! Pinned values are only rejected when they would break the simulation.
//! Values the mechanics would never roll are kept, so that pins can express
//! what happened in a real race even where the simulator disagrees.
//!
//! [`Configuration`]: crate::Configuration

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
#[derive(Debug, Default, Clone, PartialEq)]
pub struct Pins {
    /// Start delay in seconds, as the runner leaves the gate: it replaces the
    /// rolled delay after out-of-gate skills change it.
    pub start_delay: Option<f64>,
}

impl Pins {
    /// Panic if a pinned value would break the simulation.
    pub(crate) fn validate(&self) {
        if let Some(delay) = self.start_delay {
            assert!(
                delay.is_finite() && delay >= 0.0,
                "Invalid start delay {delay}: expected a finite, non-negative number"
            );
        }
    }
}
