//! Spot struggle (位置取り争い) formulas (1st Anniversary onward).
//!
//! Early in the race, the frontmost runner of a front-like strategy (front
//! runners or runaways, separately) may struggle for the lead with others of
//! her strategy close behind, gaining target speed for a while at the cost of
//! HP. It happens at most once per strategy.

use std::ops::RangeInclusive;

use crate::race::modifiers::strategy_proficiency_multiplier;
use crate::uma::{Aptitude, Style};
use crate::units::CourseWidths;

/// Strategies that can struggle. Runners only struggle against others with
/// the same strategy.
pub(crate) const STYLES: [Style; 2] = [Style::Runaway, Style::Front];
/// Distance (m) any runner (not necessarily one of the strugglers) must have
/// run before a spot struggle can start.
pub(crate) const MIN_POSITION: f64 = 150.0;
/// Sections (0-indexed) in which a spot struggle can start. At least one of
/// the leader and the runner struggling with her must be in them.
pub(crate) const SECTIONS: RangeInclusive<u8> = 0..=5;
/// Section (0-indexed) upon whose entry any ongoing spot struggle stops.
pub(crate) const END_SECTION: u8 = 8;
/// Maximum distance (m) from the leader to start struggling with her.
pub(crate) const TRIGGER_DISTANCE: f64 = 3.75;
/// Maximum lane gap (in course widths) from the leader to start struggling
/// with her.
pub(crate) const TRIGGER_LANE_GAP: CourseWidths = CourseWidths(0.165);
/// Distance (m) behind every other struggler above which a runner exits.
pub(crate) const EXIT_DISTANCE: f64 = 5.0;
/// Lane gap (in course widths) from every other struggler above which a
/// runner exits.
pub(crate) const EXIT_LANE_GAP: CourseWidths = CourseWidths(0.416);

/// Spot struggle state.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SpotStruggle {
    /// Not started yet.
    Pending,
    /// Started. Effects apply until `expires_at` (in seconds), but the runner
    /// counts as a struggler until she exits or the struggle ends.
    Active { speed_bonus: f64, expires_at: f64 },
    /// Exited by drifting away from the other strugglers, or ended upon
    /// entering `END_SECTION`. It can't start again.
    Ended,
}

impl SpotStruggle {
    /// Start the spot struggle at `elapsed_time`.
    pub(crate) fn start(guts: f64, style_aptitude: Aptitude, elapsed_time: f64) -> Self {
        SpotStruggle::Active {
            speed_bonus: target_speed_bonus(guts),
            expires_at: elapsed_time + duration(guts, style_aptitude),
        }
    }

    /// Check if the spot struggle is in effect at `elapsed_time`.
    pub fn is_active(&self, elapsed_time: f64) -> bool {
        matches!(self, SpotStruggle::Active { expires_at, .. } if elapsed_time < *expires_at)
    }

    /// Check if the runner still counts as a struggler, even if the effects
    /// have expired.
    pub fn is_struggler(&self) -> bool {
        matches!(self, SpotStruggle::Active { .. })
    }

    /// Check if the runner has entered a spot struggle at some point.
    pub fn has_struggled(&self) -> bool {
        !matches!(self, SpotStruggle::Pending)
    }

    /// Target speed increase (m/s) at `elapsed_time` (zero if not active).
    pub(crate) fn speed_bonus(&self, elapsed_time: f64) -> f64 {
        match self {
            SpotStruggle::Active {
                speed_bonus,
                expires_at,
            } if elapsed_time < *expires_at => *speed_bonus,
            _ => 0.0,
        }
    }

    /// Stop the spot struggle, if it was started.
    pub(crate) fn stop(&mut self) {
        if self.has_struggled() {
            *self = SpotStruggle::Ended;
        }
    }
}

/// Check if a runner `distance` meters and `lane_diff` course widths away from
/// the frontmost runner of her strategy is close enough to struggle with her.
pub(crate) fn is_close_to_leader(distance: f64, lane_diff: CourseWidths) -> bool {
    distance.abs() < TRIGGER_DISTANCE && lane_diff.abs() < TRIGGER_LANE_GAP
}

/// Check if a struggler is too far from another struggler to keep struggling,
/// given her `distance` in meters (positive if ahead) and `lane_diff` in course
/// widths from the other.
///
/// Only falling behind counts; getting ahead doesn't.
pub(crate) fn is_too_far(distance: f64, lane_diff: CourseWidths) -> bool {
    distance < -EXIT_DISTANCE || lane_diff.abs() > EXIT_LANE_GAP
}

/// Target speed increase (m/s) during the spot struggle.
pub(crate) fn target_speed_bonus(guts: f64) -> f64 {
    (500.0 * guts).powf(0.6) * 0.0001
}

/// Duration of the spot struggle in seconds.
pub(crate) fn duration(guts: f64, style_aptitude: Aptitude) -> f64 {
    (700.0 * guts).sqrt() * 0.012 * strategy_proficiency_multiplier(style_aptitude)
}

/// HP consumption multiplier during the spot struggle.
///
/// # Panics
///
/// Panics if `style` is neither [`Style::Runaway`] nor [`Style::Front`], the
/// only styles that take part in spot struggles.
pub(crate) fn hp_multiplier(style: Style, rushed: bool) -> f64 {
    match (style, rushed) {
        (Style::Runaway, true) => 7.7,
        (Style::Runaway, false) => 3.5,
        (Style::Front, true) => 3.6,
        (Style::Front, false) => 1.4,
        _ => unreachable!("spot struggle should only apply to Runaway and Front styles"),
    }
}

#[cfg(test)]
mod tests;
