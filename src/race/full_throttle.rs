//! Full-Throttle Sprint (全開スパート) formulas (5th Anniversary onward).
//!
//! A runner with more than 2000 speed stat keeps gaining target speed after
//! reaching her last spurt speed, until she finishes the race or runs out of HP.

use crate::configuration::GameVersion;

/// Adjusted speed stat that must be exceeded to be eligible.
pub const SPEED_THRESHOLD: u16 = 2000;
/// Fraction of the listed value of Full-Throttle acceleration skills (type 48)
/// that is actually applied.
// ? Observed to be close to 10%; the reason is unknown.
pub const SKILL_SCALE: f64 = 0.1;

/// Full-Throttle Sprint state.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum FullThrottle {
    /// Not activated yet (or the runner is not eligible).
    Pending,
    /// Active. `speed_bonus` is the target speed gained so far in m/s.
    Active { speed_bonus: f64 },
    /// Ended by running out of HP. It can't be activated again.
    Ended,
}

impl FullThrottle {
    /// Check if the Full-Throttle Sprint is currently active.
    pub fn is_active(&self) -> bool {
        matches!(self, FullThrottle::Active { .. })
    }

    /// Target speed gained so far in m/s (zero if not active).
    pub fn speed_bonus(&self) -> f64 {
        match self {
            FullThrottle::Active { speed_bonus } => *speed_bonus,
            _ => 0.0,
        }
    }
}

/// Full-Throttle Sprint acceleration in m/s² before skill modifiers.
///
/// Zero if the runner is not eligible, i.e. adjusted speed stat of 2000 or below.
// ? Empirical formula; exact values are under investigation.
// ? The maximum gain should scale with speed, but it seems to never be reached,
// ? so it is not modeled (the global max speed still applies).
pub fn base_acceleration(
    speed: u16,
    power: u16,
    course_distance: i32,
    game_version: GameVersion,
) -> f64 {
    if game_version < GameVersion::FifthAnniversary || speed <= SPEED_THRESHOLD {
        return 0.0;
    }
    let acceleration_from_power = 0.0006 * (500.0 * power as f64).sqrt();
    0.068 * acceleration_from_power / (course_distance as f64 / 1000.0).powf(1.5)
}
