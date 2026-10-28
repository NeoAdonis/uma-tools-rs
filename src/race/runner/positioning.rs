//! Positioning strategies during the first 10 sections of the race.

use rand::prelude::*;

use crate::configuration::GameVersion;
use crate::course::*;
use crate::race::solver::RaceState;
use crate::uma::*;

use super::Runner;

/// First section where positioning strategies no longer apply.
pub(crate) const POSITIONING_END_SECTION: u8 = 10;

/// Positioning strategy during the first 10 sections (5/12) of the race.
///
/// For front-runners, it determines the pacing behavior to stay ahead or catch
/// up. For non-front-runners, it determines the pacing behavior to keep their
/// distance from the pacemaker.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum PositioningStrategy {
    /// Run at regular pace.
    Normal,
    /// *(Front-runners only.)*
    /// Try to stay ahead by pacing up to a certain distance.
    FrontSpeedUp,
    /// *(Front-runners only.)*
    /// Try to catch up with first place by pacing up to a certain distance.
    FrontOvertake,
    /// *(Non-front-runners only.)*
    /// Try to stay within a certain distance from pacemaker by pacing up.
    PaceUp(f64),
    /// *(Non-front-runners only.)*
    /// Try to fall back within a certain distance from pacemaker by pacing down.
    PaceDown(f64),
    /// *Introduced by 1.5 Anniversary update in Japan.* Speed up if there is
    /// another runner ahead whose strategy should actually position her behind.
    /// Front-runners enter it when any such runner is ahead; other runners enter it
    /// when the pacemaker is such a runner. Exits once no such runner is ahead.
    /// Takes priority over all other strategies.
    PaceUpEx,
}

impl PositioningStrategy {
    /// Base target speed multiplier while following this strategy.
    pub(crate) fn speed_multiplier(self, phase: Phase, game_version: GameVersion) -> f64 {
        match self {
            PositioningStrategy::Normal => 1.0,
            PositioningStrategy::FrontSpeedUp => 1.04,
            PositioningStrategy::FrontOvertake => 1.05,
            PositioningStrategy::PaceUp(_) => 1.04,
            PositioningStrategy::PaceDown(_) => {
                if game_version >= GameVersion::OnePointFiveAnniversary && phase >= Phase::Mid {
                    0.945
                } else {
                    0.915
                }
            }
            PositioningStrategy::PaceUpEx => 2.0,
        }
    }
}

impl Runner {
    /// Check if the runner is currently in position keep pace down mode.
    pub(crate) fn is_pace_down(&self) -> bool {
        matches!(self.positioning_strategy, PositioningStrategy::PaceDown(_))
    }

    /// Get the current style used for positioning strategy decisions.
    pub(crate) fn positioning_style(&self) -> Style {
        if self.is_rushed() {
            self.rush_state.as_ref().unwrap().style
        } else {
            self.adjusted.style
        }
    }

    /// Criteria for front-runners to speed up to stay ahead or catch up.
    fn can_speed_up_to_front(
        &self,
        distance_ahead: f64,
        is_single_front_runner: bool,
        game_version: GameVersion,
    ) -> bool {
        let distance_threshold = if self.positioning_style() == Style::Runaway {
            17.5
        } else if game_version >= GameVersion::OnePointFiveAnniversary && is_single_front_runner {
            12.5
        } else {
            4.5
        };

        distance_ahead < distance_threshold
    }

    /// Update positioning strategy for front-runners during the first 10 sections
    /// (5/12 of the race). The strategy determines whether to pace up to stay ahead
    /// (Speed Up) or to catch up with the first runner of the same strategy
    /// (Overtake).
    ///
    /// `lead` is the distance to the runner behind if this runner is first
    /// overall, or `None` otherwise. `strategy_lead` is the distance to the next
    /// runner with the same strategy if this runner is first among them, or `None`
    /// otherwise. Both are infinite if there's no such runner behind.
    pub(crate) fn update_positioning_front(
        &mut self,
        lead: Option<f64>,
        strategy_lead: Option<f64>,
        is_single_front_runner: bool,
        state: &RaceState,
    ) {
        let elapsed_time = state.elapsed_time;
        let game_version = state.config.game_version;
        let can_speed_up = |runner: &Self, lead| {
            runner.can_speed_up_to_front(lead, is_single_front_runner, game_version)
        };
        match self.positioning_strategy {
            PositioningStrategy::Normal => {
                if elapsed_time < self.next_positioning_check {
                    return;
                }

                self.next_positioning_check += 2.0;

                // Use original style to determine if runner was originally a front-runner that
                // should speed up.
                if self.is_rushed() && self.adjusted.style.is_front_like() {
                    self.positioning_strategy = PositioningStrategy::FrontSpeedUp;
                    return;
                }

                if !self.roll_positioning_wit_check(0.2) {
                    return;
                }

                if lead.is_some_and(|lead| can_speed_up(self, lead)) {
                    // First and not far enough ahead: try to stay ahead.
                    self.positioning_strategy = PositioningStrategy::FrontSpeedUp;
                } else if strategy_lead.is_none() {
                    // Not first in the same strategy: try to catch up.
                    self.positioning_strategy = PositioningStrategy::FrontOvertake;
                }
            }
            PositioningStrategy::FrontSpeedUp => {
                // Exit if sufficiently ahead (staying if overtaken).
                if lead.is_some_and(|lead| !can_speed_up(self, lead)) {
                    self.positioning_strategy = PositioningStrategy::Normal;
                }
            }
            PositioningStrategy::FrontOvertake => {
                // Exit if well ahead of the rest of the same strategy.
                let distance_threshold = if matches!(self.positioning_style(), Style::Runaway) {
                    27.5
                } else {
                    10.0
                };
                if strategy_lead.is_some_and(|lead| lead > distance_threshold) {
                    self.positioning_strategy = PositioningStrategy::Normal;
                }
            }
            _ => {
                debug_assert!(false, "BUG: Invalid positioning strategy for front runner")
            }
        }
    }

    /// Update positioning strategy for non-front-runners during the first 10
    /// sections (5/12 of the race).
    ///
    /// The strategy determines whether to pace up or down to keep distance from the
    /// pacemaker.
    ///
    /// `distance_behind` is the distance from the pacemaker in meters (0 if we are
    /// the pacemaker).
    pub(crate) fn update_positioning_non_front(&mut self, distance_behind: f64, state: &RaceState) {
        let elapsed_time = state.elapsed_time;
        let course_length = state.course.distance as f64;
        match self.positioning_strategy {
            PositioningStrategy::Normal => {
                if elapsed_time < self.next_positioning_check {
                    return;
                }

                self.next_positioning_check += 2.0;

                // Determine target distance range based on style and course length.
                let course_multiplier = 0.0008 * (course_length - 1000.0) + 1.0;
                let range = match self.positioning_style() {
                    Style::Pace => 3.0..(5.0 * course_multiplier),
                    Style::Late => (6.5 * course_multiplier)..(7.0 * course_multiplier),
                    Style::End => (7.5 * course_multiplier)..(8.0 * course_multiplier),
                    _ => 0.0..0.0,
                };
                if range.contains(&distance_behind) {
                    return;
                }

                if distance_behind > range.end {
                    // Catch up to a certain distance (if wit allows).
                    if !self.roll_positioning_wit_check(0.15) {
                        return;
                    }

                    let threshold = self.rng.borrow_mut().random_range(range);
                    self.positioning_strategy = PositioningStrategy::PaceUp(threshold);
                } else if distance_behind < range.start && !self.skill_modifiers.has_speed_effects()
                {
                    // Pace down to a certain distance (unless affected by speed-increasing skills).
                    // 1.5 anniv. patch: if in mid-race, the maximum distance is replaced with the
                    // midpoint between min and max before rolling.
                    let threshold = if state.config.game_version
                        >= GameVersion::OnePointFiveAnniversary
                        && self.phase >= Phase::Mid
                    {
                        let min = range.start;
                        let max = range.end;
                        let mid = (min + max) / 2.0;
                        let adjusted_range = min..mid;
                        self.rng.borrow_mut().random_range(adjusted_range)
                    } else {
                        self.rng.borrow_mut().random_range(range)
                    };
                    self.positioning_strategy = PositioningStrategy::PaceDown(threshold);
                }
            }
            PositioningStrategy::PaceUp(threshold) => {
                // Exit when caught up within threshold.
                if distance_behind < threshold {
                    self.reset_positioning(elapsed_time + 3.0);
                }
            }
            PositioningStrategy::PaceDown(threshold) => {
                // Exit when fallen back beyond threshold or if speed is increased by skills.
                if distance_behind > threshold || self.skill_modifiers.has_speed_effects() {
                    self.reset_positioning(elapsed_time + 3.0);
                }
            }
            _ => {
                debug_assert!(
                    false,
                    "BUG: Invalid positioning strategy for non-front runner"
                )
            }
        }
    }

    /// Go back to the normal positioning strategy, checking it again at
    /// `next_check` (s).
    pub(super) fn reset_positioning(&mut self, next_check: f64) {
        self.positioning_strategy = PositioningStrategy::Normal;
        self.next_positioning_check = next_check;
    }

    /// Roll a wit check to change positioning strategy, with a chance of
    /// `coef * log10(wit * 0.1)`.
    fn roll_positioning_wit_check(&self, coef: f64) -> bool {
        let chance = coef * (self.adjusted.wit as f64 * 0.1).log10();
        self.rng.borrow_mut().random_bool(chance)
    }

    /// Enter or exit [`PositioningStrategy::PaceUpEx`] (1.5 anniversary and later).
    ///
    /// Returns `true` if the runner is in Pace Up Ex, in which case the regular
    /// positioning update must be skipped.
    ///
    /// `passive_ahead` tells whether any runner ahead has a style that should
    /// position her behind this runner.
    pub(crate) fn update_pace_up_ex(
        &mut self,
        passive_ahead: bool,
        pacemaker_style: Style,
        state: &RaceState,
    ) -> bool {
        if matches!(self.positioning_strategy, PositioningStrategy::PaceUpEx) {
            if !passive_ahead {
                self.reset_positioning(state.elapsed_time);
                return false;
            }
            return true;
        }

        let style = self.positioning_style();
        let should_enter = if style.is_front_like() {
            passive_ahead
        } else {
            pacemaker_style > style
        };
        if should_enter {
            // Overrides any other strategy.
            self.positioning_strategy = PositioningStrategy::PaceUpEx;
        }
        should_enter
    }
}

#[cfg(test)]
mod tests;
