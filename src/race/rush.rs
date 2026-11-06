//! Rushing (temptation, kakari, 掛かり) formulas.
//!
//! Before the race, each runner rolls whether she will rush based on her wit.
//! If so, upon entering a random section early in the race, she rushes ahead of
//! the position dictated by her style (possibly positioning as a different
//! style), losing HP at an increased rate until she snaps out of it.

use std::ops::RangeInclusive;

use rand::prelude::*;

use crate::uma::Style;

/// Sections (0-indexed) in which a runner may start rushing.
pub(crate) const SECTIONS: RangeInclusive<u8> = 1..=8;
/// Time in seconds after which a rushed runner may snap out of it, either since
/// she started rushing or since her last failed attempt.
pub(crate) const CHECK_INTERVAL: f64 = 3.0;
/// Chance of snapping out of the rush on each check.
pub(crate) const SNAP_OUT_CHANCE: f64 = 0.55;
/// Maximum time in seconds a runner stays rushed, before skill effects.
pub(crate) const MAX_DURATION: f64 = 12.0;
/// HP consumption multiplier while rushed.
pub(crate) const HP_MULTIPLIER: f64 = 1.5;

/// Rush state of a runner bound to rush.
#[derive(Clone, Debug, PartialEq)]
pub struct RushState {
    /// Section (0-indexed) upon whose entry the runner starts rushing.
    pub section: u8,
    /// Time in seconds when the runner started rushing, or zero if she hasn't.
    pub start_time: f64,
    /// Time in seconds of the next snap out check while rushed, or when the
    /// rush ended once it's over.
    pub end_time: f64,
    /// Maximum time in seconds the runner stays rushed.
    pub max_duration: f64,
    /// Whether the runner is currently rushed.
    pub is_rushed: bool,
    /// Style used for positioning while rushed.
    pub style: Style,
}

impl RushState {
    /// Roll when and how a runner with `style` will rush.
    pub(crate) fn roll(style: Style, rng: &mut impl Rng) -> Self {
        let style = roll_style(style, rng);
        let section = rng.random_range(SECTIONS);
        RushState {
            section,
            start_time: 0.0,
            end_time: f64::MAX,
            max_duration: MAX_DURATION,
            is_rushed: false,
            style,
        }
    }

    /// Check if the runner has rushed at some point.
    pub fn has_rushed(&self) -> bool {
        self.start_time > 0.0
    }

    /// Start rushing at `elapsed_time`.
    pub(crate) fn start(&mut self, elapsed_time: f64) {
        self.is_rushed = true;
        self.start_time = elapsed_time;
        self.end_time = elapsed_time + CHECK_INTERVAL;
    }

    /// Stop rushing at `elapsed_time`.
    pub(crate) fn end(&mut self, elapsed_time: f64) {
        self.is_rushed = false;
        self.end_time = elapsed_time;
    }

    /// Check if the rush should end at `elapsed_time`, either because it reached
    /// its maximum duration or because the runner snapped out of it. A failed snap
    /// out check extends the rush until the next one.
    pub(crate) fn should_end(&mut self, elapsed_time: f64, rng: &mut impl Rng) -> bool {
        if elapsed_time >= self.start_time + self.max_duration {
            return true;
        }
        if elapsed_time < self.end_time {
            return false;
        }
        if rng.random_bool(SNAP_OUT_CHANCE) {
            return true;
        }
        self.end_time = elapsed_time + CHECK_INTERVAL;
        false
    }
}

/// Chance of rushing as a percentage, given the runner's base wit and the rush
/// chance modifier from skills (in percentage points).
///
/// The game rolls integer percentages, so it's rounded up. Very low wit and
/// rush chance effects can push it out of range, so it's clamped.
pub(crate) fn chance(wit: f64, modifier: f64) -> f64 {
    ((6.5 / (0.1 * wit + 1.0).log10()).powf(2.0) + modifier)
        .ceil()
        .clamp(0.0, 100.0)
}

/// Roll the style a runner with `style` positions as while rushed.
fn roll_style(style: Style, rng: &mut impl Rng) -> Style {
    match style {
        // Front-like styles keep their style.
        Style::Front | Style::Runaway => style,
        Style::Pace => Style::Front,
        // 75% Front, 25% Pace.
        Style::Late => {
            if rng.random_bool(0.75) {
                Style::Front
            } else {
                Style::Pace
            }
        }
        // 70% Front, 20% Pace, 10% Late.
        Style::End => {
            let roll: f64 = rng.random();
            if roll < 0.7 {
                Style::Front
            } else if roll < 0.9 {
                Style::Pace
            } else {
                Style::Late
            }
        }
    }
}

#[cfg(test)]
mod tests;
