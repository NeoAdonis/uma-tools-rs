//! Duel (Compete Fight, 追い比べ) formulas (1st Anniversary onward).
//!
//! On the final straight, two runners that stay close for a while may start
//! dueling, gaining target speed and acceleration based on their guts. A
//! runner keeps dueling until her HP runs low or she gets far from everyone
//! who has dueled during the race. Once she leaves, she can't duel again.

use crate::race::runner::OrderState;
use crate::units::CourseWidths;

/// Maximum distance (m) between two runners to count as close.
pub(crate) const TRIGGER_DISTANCE: f64 = 3.0;
/// Maximum lane gap (in course widths) between two runners to count as close.
pub(crate) const TRIGGER_LANE_GAP: CourseWidths = CourseWidths(0.25);
/// Time (s) two runners must stay close in a row before they can start dueling,
/// counted from the first frame they're close.
pub(crate) const TRIGGER_TIME: f64 = 2.0;
/// Maximum speed difference (m/s) between two runners to start dueling.
pub(crate) const MAX_SPEED_DIFF: f64 = 0.6;
/// Maximum order rate (top X%, exact, not rounded) at least one of the runners
/// must be at to start dueling.
pub(crate) const MAX_ORDER_RATE: u8 = 50;
/// Minimum HP percentage both runners must have to start dueling.
pub(crate) const MIN_START_HP_PERCENT: f64 = 15.0;
/// HP percentage below which a duelist exits.
pub(crate) const EXIT_HP_PERCENT: f64 = 5.0;
/// Distance (m) from every other duelist, ahead or behind, at which a duelist
/// exits.
pub(crate) const EXIT_DISTANCE: f64 = 5.0;

/// Tolerance for floating-point rounding errors in the trigger checks.
///
/// It keeps accumulated frame times from being thrown off by them, and lets a
/// speed difference a hair over `MAX_SPEED_DIFF` still count.
const TOLERANCE: f64 = 1e-6;

/// Duel state.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Duel {
    /// Not started yet.
    Pending,
    /// Dueling, with the given target speed (m/s) and acceleration (m/s²) bonuses.
    Active {
        speed_bonus: f64,
        acceleration_bonus: f64,
    },
    /// Left the duel. It can't start again, but the runner still counts for other
    /// duelists' exit checks.
    Ended,
}

impl Duel {
    /// Starts dueling.
    pub(crate) fn start(guts: f64) -> Self {
        Duel::Active {
            speed_bonus: target_speed_bonus(guts),
            acceleration_bonus: acceleration_bonus(guts),
        }
    }

    /// Checks if the runner is currently dueling.
    pub fn is_dueling(&self) -> bool {
        matches!(self, Duel::Active { .. })
    }

    /// Checks if the runner has dueled at some point, even if she already left.
    pub fn has_dueled(&self) -> bool {
        !matches!(self, Duel::Pending)
    }

    /// Checks if the runner has left a duel, and thus can't duel again.
    pub fn has_ended(&self) -> bool {
        matches!(self, Duel::Ended)
    }

    /// Returns the target speed increase (m/s), zero if not dueling.
    pub(crate) fn speed_bonus(&self) -> f64 {
        match self {
            Duel::Active { speed_bonus, .. } => *speed_bonus,
            _ => 0.0,
        }
    }

    /// Returns the acceleration increase (m/s²), zero if not dueling.
    pub(crate) fn acceleration_bonus(&self) -> f64 {
        match self {
            Duel::Active {
                acceleration_bonus, ..
            } => *acceleration_bonus,
            _ => 0.0,
        }
    }

    /// Leaves the duel, if dueling.
    pub(crate) fn stop(&mut self) {
        if self.is_dueling() {
            *self = Duel::Ended;
        }
    }
}

/// Checks if two runners `distance` meters and `lane_diff` course widths apart
/// are close enough to start counting towards a duel.
pub(crate) fn is_close(distance: f64, lane_diff: CourseWidths) -> bool {
    distance.abs() < TRIGGER_DISTANCE && lane_diff.abs() < TRIGGER_LANE_GAP
}

/// Checks if two runners have been close for long enough to start dueling,
/// given how long (in seconds) they have been close in a row.
pub(crate) fn is_close_long_enough(close_time: f64) -> bool {
    close_time >= TRIGGER_TIME - TOLERANCE
}

/// Checks if two runners' speeds (m/s) are similar enough to start dueling.
pub(crate) fn are_speeds_similar(speed: f64, other_speed: f64) -> bool {
    (speed - other_speed).abs() <= MAX_SPEED_DIFF + TOLERANCE
}

/// Checks if a runner's order is high enough (at most `MAX_ORDER_RATE` percent
/// of the runners) for her to start dueling.
pub(crate) fn is_order_high_enough(order: &OrderState) -> bool {
    100 * u16::from(order.order) <= u16::from(MAX_ORDER_RATE) * u16::from(order.runner_count)
}

/// Checks if a duelist `distance` meters away from another duelist is separated
/// from her, either ahead or behind.
pub(crate) fn is_separated(distance: f64) -> bool {
    distance.abs() >= EXIT_DISTANCE
}

/// Returns the target speed increase (m/s) while dueling.
fn target_speed_bonus(guts: f64) -> f64 {
    (200.0 * guts).powf(0.708) * 0.0001
}

/// Returns the acceleration increase (m/s²) while dueling.
fn acceleration_bonus(guts: f64) -> f64 {
    (160.0 * guts).powf(0.59) * 0.0001
}

#[cfg(test)]
mod tests;
