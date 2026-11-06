//! Secure lead (リード確保) formulas.
//!
//! Between sections 11 and 15, a runner whose lead over another runner with a
//! later strategy is less than desirable may speed up for a short time at the
//! cost of HP.

use std::ops::RangeInclusive;

use crate::configuration::GameVersion;
use crate::uma::Style;

/// Sections (0-indexed) in which secure lead can be attempted.
pub(crate) const SECTIONS: RangeInclusive<u8> = 10..=14;
/// Interval between secure lead checks in seconds.
pub(crate) const CHECK_INTERVAL: f64 = 2.0;
/// Duration of the secure lead speed boost in seconds.
pub(crate) const DURATION: f64 = 2.0;
/// Cooldown after the speed boost ends in seconds.
pub(crate) const COOLDOWN: f64 = 1.0;
/// Base chance of attempting to secure the lead on each check.
// ? Should scale with wit, but the formula is unknown.
pub(crate) const CHANCE: f64 = 0.2;

/// Desirable lead (in meters) of a runner with `style` over a runner with
/// `other` style, or `None` if `other` should not be behind `style` by
/// strategy.
pub(crate) fn desirable_lead(style: Style, other: Style, course_distance: f64) -> Option<f64> {
    use Style::*;
    let coef = match (style, other) {
        (Runaway, Front) => 2.0,
        (Runaway, Pace) => 7.0,
        (Runaway, Late | End) => 8.0,
        (Front, Pace) => 4.0,
        (Front, Late | End) => 8.0,
        (Pace, Late) => 5.0,
        (Pace, End) => 6.0,
        (Late, End) => 3.0,
        _ => return None,
    };
    Some(coef + 0.0003 * (course_distance + 1000.0))
}

/// Target speed increase (m/s) while securing the lead.
///
/// `is_alone` is whether no other runner with the same style is within 10m.
/// It only has an effect on front-like runners.
// ? There is an unexplained 20% threshold for the `is_alone` bonus; ignored.
pub(crate) fn target_speed_bonus(
    style: Style,
    guts: f64,
    is_alone: bool,
    game_version: GameVersion,
) -> f64 {
    let style_coef = match style {
        Style::Runaway => 0.2,
        Style::Front | Style::Pace => 1.0,
        Style::Late => 0.8,
        Style::End => 0.0,
    };
    let alone_coef = match (style, is_alone) {
        (Style::Runaway, true) => 7.0,
        (Style::Front, true) if game_version >= GameVersion::FifthAnniversary => 4.0,
        (Style::Front, true) => 2.0,
        _ => 1.0,
    };
    (guts / 2000.0).sqrt() * 0.3 * style_coef * alone_coef
}

/// HP consumed upon securing the lead.
pub(crate) fn hp_cost(style: Style, course_distance: i32) -> f64 {
    let style_coef = match style {
        Style::Runaway => 1.2,
        Style::Front => 1.0,
        Style::Pace | Style::Late => 0.8,
        Style::End => 0.0,
    };
    20.0 * style_coef * distance_coefficient(course_distance)
}

/// Course distance coefficient of the HP cost. Shared with compete before
/// spurt mechanics.
pub(crate) fn distance_coefficient(course_distance: i32) -> f64 {
    match course_distance {
        ..=1800 => 0.3,
        1801..=2100 => 0.5,
        2101..=2200 => 0.8,
        2201..=2400 => 1.0,
        2401..=2600 => 1.1,
        _ => 1.2,
    }
}

#[cfg(test)]
mod tests;
