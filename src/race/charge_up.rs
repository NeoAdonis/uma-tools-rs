//! Charge Up (足を貯める) and Fully Charged (脚色十分) formulas (2nd Anniv. onward).
//!
//! A runner with more than 1200 charge power conserves power during the first
//! 10 sections while keeping her position calmly. Upon entering her last spurt,
//! the conserved power is released as a temporary acceleration boost.

use crate::configuration::GameVersion;
use crate::course::DistanceType;
use crate::uma::Style;

/// Charge power that must be exceeded to be eligible.
pub const POWER_THRESHOLD: f64 = 1200.0;
/// Time in seconds after leaving a blocking state before power can be conserved
/// again.
pub const COOLDOWN: f64 = 1.5;
/// Minimum release duration in seconds. Shorter releases don't happen at all.
pub const MIN_DURATION: f64 = 0.2;
/// Conserved power multiplier upon entering a spot struggle.
pub const STRUGGLE_PENALTY: f64 = 0.95;
/// Conserved power multiplier upon entering rush.
pub const RUSH_PENALTY: f64 = 0.8;
/// Activity coefficient factor if the runner entered a spot struggle.
pub const STRUGGLE_ACTIVITY: f64 = 0.98;
/// Activity coefficient factor if the runner rushed.
pub const RUSH_ACTIVITY: f64 = 0.8;

/// Charge Up state. It is kept after the race as a record of the mechanic.
#[derive(Clone, Debug, PartialEq)]
pub struct ChargeUp {
    /// Power stat used by this mechanic. Fixed at the start of the race.
    pub power: f64,
    /// Power conserved so far.
    pub conserved: f64,
    /// Power can't be conserved before this time, in seconds.
    pub cooldown_until: f64,
    /// Whether the conserved power has been released.
    pub outcome: ChargeUpOutcome,
}

/// Outcome of the Charge Up upon entering the last spurt.
#[derive(Clone, Debug, PartialEq)]
pub enum ChargeUpOutcome {
    /// The last spurt has not been entered yet.
    Pending,
    /// The conserved power was released (Fully Charged, 脚色十分).
    Released(ChargeUpRelease),
    /// Not enough power was conserved; nothing was released.
    Insufficient,
}

/// Record of a Charge Up release.
#[derive(Clone, Debug, PartialEq)]
pub struct ChargeUpRelease {
    /// Power conserved at the time of release.
    pub conserved: f64,
    /// Acceleration bonus in m/s².
    pub acceleration: f64,
    /// Duration of the acceleration bonus in seconds.
    pub duration: f64,
    /// Time of release in seconds.
    pub start_time: f64,
    /// Position of release in meters.
    pub start_position: f64,
    /// Time when the acceleration bonus ends, in seconds.
    pub expires_at: f64,
}

impl ChargeUp {
    /// Create the Charge Up state, or `None` if the runner is not eligible.
    pub fn new(power: f64, game_version: GameVersion) -> Option<Self> {
        if game_version < GameVersion::SecondAnniversary || power <= POWER_THRESHOLD {
            return None;
        }
        Some(ChargeUp {
            power,
            conserved: 0.0,
            cooldown_until: 0.0,
            outcome: ChargeUpOutcome::Pending,
        })
    }

    /// Acceleration bonus at the given time in m/s² (zero if not active).
    pub fn acceleration(&self, elapsed_time: f64) -> f64 {
        match &self.outcome {
            ChargeUpOutcome::Released(release)
                if (release.start_time..release.expires_at).contains(&elapsed_time) =>
            {
                release.acceleration
            }
            _ => 0.0,
        }
    }
}

/// Power stat used by this mechanic.
///
/// Unlike the regular base stat, only the part up to 1200 is affected by
/// motivation: `base_power` is the halved stat after motivation, and the halved
/// excess of `raw_power` over 1200 is added back without it.
pub fn charge_power(base_power: u16, raw_power: u16, skill_bonus: i32) -> f64 {
    base_power as f64 + 0.5 * (raw_power as f64 - POWER_THRESHOLD).max(0.0) + skill_bonus as f64
}

/// Position keep coefficient of the conservation rate.
pub fn position_keep_coefficient(pace_down: bool) -> f64 {
    if pace_down { 6.7 } else { 4.2 }
}

/// Style coefficient of the conservation rate.
// ? Unknown for Runaway; assumed to be the same as Front.
pub fn style_coefficient(style: Style) -> f64 {
    match style.base_style() {
        Style::Pace => 0.75,
        Style::End => 0.7,
        _ => 1.0,
    }
}

/// Strategy-distance coefficient of the release acceleration.
// ? Unknown for Runaway; assumed to be the same as Front.
pub fn strategy_distance_coefficient(style: Style, distance_type: DistanceType) -> f64 {
    use DistanceType::*;
    match (style.base_style(), distance_type) {
        (Style::Pace, Short) => 0.7,
        (Style::Pace, Mile) => 0.8,
        (Style::Pace, Medium | Long) => 0.9,
        (Style::Late, Short) => 0.75,
        (Style::Late, Mile) => 0.7,
        (Style::Late, Medium) => 0.875,
        (Style::Late, Long) => 1.0,
        (Style::End, Short) => 0.7,
        (Style::End, Mile) => 0.75,
        (Style::End, Medium) => 0.86,
        (Style::End, Long) => 0.9,
        _ => 1.0,
    }
}

/// Distance type coefficient of the release duration.
pub fn distance_type_coefficient(distance_type: DistanceType) -> f64 {
    match distance_type {
        DistanceType::Short => 0.45,
        DistanceType::Mile => 1.0,
        DistanceType::Medium => 0.875,
        DistanceType::Long => 0.8,
    }
}

/// Activity coefficient of the release acceleration. Penalties stack
/// multiplicatively.
pub fn activity_coefficient(struggled: bool, rushed: bool) -> f64 {
    let mut coefficient = 1.0;
    if struggled {
        coefficient *= STRUGGLE_ACTIVITY;
    }
    if rushed {
        coefficient *= RUSH_ACTIVITY;
    }
    coefficient
}

/// Release acceleration bonus in m/s².
pub fn release_acceleration(
    power: f64,
    style: Style,
    distance_type: DistanceType,
    activity: f64,
) -> f64 {
    ((power - POWER_THRESHOLD) * 130.0).sqrt()
        * 0.001
        * strategy_distance_coefficient(style, distance_type)
        * activity
}

/// Release duration in seconds.
pub fn release_duration(conserved: f64, course_distance: i32, distance_type: DistanceType) -> f64 {
    conserved * (course_distance as f64).sqrt() * distance_type_coefficient(distance_type) / 1450.0
}

#[cfg(test)]
mod tests;
