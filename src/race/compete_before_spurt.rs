//! Compete before spurt (位置取り調整) and Stamina Keep (持久力温存) formulas.
//!
//! Between sections 11 and 15, a runner that is too far from the lead or has
//! other runners nearby may speed up for a short time at the cost of HP.
//! A runner that notices she lacks the HP to finish the race enters Stamina
//! Keep instead, and won't compete until a recovery skill resets it.

use std::ops::RangeInclusive;

use rand::prelude::*;

use crate::configuration::GameVersion;
use crate::race::secure_lead;
use crate::uma::Style;

/// Sections (0-indexed) in which the runner can compete before spurt.
pub(crate) const SECTIONS: RangeInclusive<u8> = 10..=14;
/// Interval between checks in seconds.
pub(crate) const CHECK_INTERVAL: f64 = 2.0;
/// Duration of the speed boost in seconds.
pub(crate) const DURATION: f64 = 2.0;
/// Cooldown after the speed boost ends in seconds.
pub(crate) const COOLDOWN: f64 = 1.0;
/// HP cost factor added when competing only because of nearby runners.
pub(crate) const NEAR_FACTOR: f64 = 0.5;
/// Distance (m) within which another runner with the same style prevents the
/// front-runner bonus.
pub(crate) const ALONE_DISTANCE: f64 = 10.0;
/// Range of the ratio of HP required to finish the race that the runner tries
/// to keep. Rolled once per race.
pub(crate) const STAMINA_KEEP_RATIO: RangeInclusive<f64> = 1.035..=1.04;

/// Stamina Keep (持久力温存) state (2.5 Anniv. onward).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StaminaKeep {
    /// Ratio of the HP required to finish the race that the runner tries to
    /// keep. Rolled before the race begins; zero before 2.5 Anniv.
    pub ratio: f64,
    /// Whether the runner is keeping stamina. While keeping it, she won't
    /// compete before spurt. Reset by recovery skills.
    pub is_active: bool,
}

impl StaminaKeep {
    /// Roll the HP ratio the runner tries to keep (see [`STAMINA_KEEP_RATIO`]).
    pub(crate) fn roll(game_version: GameVersion, rng: &mut impl Rng) -> Self {
        let ratio = if game_version < GameVersion::TwoPointFiveAnniversary {
            0.0
        } else {
            rng.random_range(STAMINA_KEEP_RATIO)
        };
        StaminaKeep {
            ratio,
            is_active: false,
        }
    }
}

/// Distance (m) from the first place above which the runner is considered too
/// far from the lead.
// ? Threshold should scale with course distance, but the formula is unknown.
// ? The distance term of secure lead is assumed.
pub(crate) fn lead_threshold(style: Style, course_distance: f64) -> f64 {
    let coef = match style {
        Style::Runaway | Style::Front => 0.0,
        Style::Pace => 2.5,
        Style::Late => 5.0,
        Style::End => 10.0,
    };
    coef + 0.0003 * (course_distance + 1000.0)
}

/// Target speed increase (m/s) while competing.
///
/// `is_alone` is whether no other runner with the same style is within
/// `ALONE_DISTANCE`. It only has an effect on front-like runners.
// ? There is an unexplained 20% threshold for the `is_alone` bonus; ignored.
pub(crate) fn target_speed_bonus(style: Style, power: u16, guts: u16, is_alone: bool) -> f64 {
    let style_coef = match style {
        Style::Runaway => 0.2,
        Style::Front => 0.8,
        _ => 1.0,
    };
    let alone_coef = match (style, is_alone) {
        (Style::Runaway, true) => 2.0,
        (Style::Front, true) => 1.1,
        _ => 1.0,
    };
    ((power as f64 / 1500.0).sqrt() * 2.0 + (guts as f64 / 3000.0).powf(0.2))
        * 0.1
        * style_coef
        * alone_coef
}

/// HP consumed upon competing.
///
/// `far_from_lead` is whether the runner competes because she is too far from
/// the lead. Otherwise, she competes only because of nearby runners.
pub(crate) fn hp_cost(style: Style, course_distance: i32, far_from_lead: bool) -> f64 {
    let style_coef = match style {
        Style::Runaway => 1.5,
        Style::Front => 1.2,
        _ => 1.0,
    };
    let near_factor = if far_from_lead { 0.0 } else { NEAR_FACTOR };
    20.0 * (style_coef * secure_lead::distance_coefficient(course_distance) + near_factor)
}

/// Chance of noticing the lack of HP and entering Stamina Keep on each check.
// ? Highly speculative.
pub(crate) fn stamina_keep_chance(wit: u16) -> f64 {
    let wit = wit as f64;
    (0.3 * (wit / 1000.0 + wit.powf(0.03))).min(1.0)
}

#[cfg(test)]
mod tests;
