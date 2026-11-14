//! Glue between runners and the race mechanics in their own modules.
//!
//! Each mechanic's rules and calculations live in its own module under `race`,
//! which knows nothing about [`Runner`]. The methods here apply them to a
//! runner: they read and update her state, log her events and tell the race
//! state about changes. They cover rushing, spot struggle, secure lead, compete
//! before spurt (with Stamina Keep), dueling and Charge Up.

use std::ops::RangeInclusive;

use rand::prelude::*;

use crate::configuration::GameVersion;
use crate::race::charge_up::{self, ChargeUpOutcome};
use crate::race::compete_before_spurt;
use crate::race::duel::Duel;
use crate::race::events::{Event, EventKind};
use crate::race::rush::RushState;
use crate::race::secure_lead;
use crate::race::solver::RaceState;
use crate::race::spot_struggle::SpotStruggle;
use crate::race::timed_boost::TimedBoost;
use crate::uma::*;

use super::{POSITIONING_END_SECTION, PositioningStrategy, Runner};

impl Runner {
    /// Releases conserved power (脚色十分) once `position` reaches the start of the
    /// last spurt.
    ///
    /// Called after the position update, so the acceleration bonus starts on the
    /// next frame.
    pub(super) fn try_release_charge_up(&mut self, position: f64, state: &RaceState) {
        if !self
            .last_spurt
            .is_some_and(|last_spurt| last_spurt.has_started(position))
        {
            return;
        }

        let activity =
            charge_up::activity_coefficient(self.spot_struggle.has_struggled(), self.has_rushed());
        if let Some(charge_up) = &mut self.charge_up
            && charge_up.outcome == ChargeUpOutcome::Pending
        {
            charge_up.release(
                self.style,
                &state.course,
                activity,
                state.elapsed_time,
                position,
            );
            if let ChargeUpOutcome::Released(release) = &charge_up.outcome {
                self.events.push(Event::start(
                    EventKind::ChargeUpRelease {
                        conserved: release.conserved,
                        acceleration: release.acceleration,
                    },
                    release.start_time,
                    release.start_position,
                ));
            }
        }
    }

    /// Conserves power (足を貯める) during the first 10 sections.
    ///
    /// Power is conserved while keeping position calmly. Speeding up, rushing or
    /// struggling for the lead prevent it, and impose a cooldown upon exit.
    pub(crate) fn update_charge_up(&mut self, delta_time: f64, elapsed_time: f64) {
        if self.section >= POSITIONING_END_SECTION {
            return;
        }
        let blocked = self.is_rushed()
            || self.spot_struggle.is_active(elapsed_time)
            || matches!(
                self.positioning_strategy,
                PositioningStrategy::FrontSpeedUp
                    | PositioningStrategy::FrontOvertake
                    | PositioningStrategy::PaceUp(_)
                    | PositioningStrategy::PaceUpEx
            );
        let rate = charge_up::conservation_rate(self.is_pace_down(), self.style);
        if let Some(charge_up) = &mut self.charge_up {
            charge_up.conserve(rate, blocked, delta_time, elapsed_time);
        }
    }

    /// Starts rushing, if the runner is bound to rush.
    ///
    /// The runner will move ahead of the position dictated by her style, losing HP
    /// at an increased rate.
    pub(super) fn start_rush(&mut self, state: &mut RaceState) {
        if let Some(rush_state) = &mut self.rush_state {
            rush_state.start(state.elapsed_time);
            let style = rush_state.style;
            self.log_start(EventKind::Rush { style }, state.elapsed_time);
            self.on_rush_change(state);
            if let Some(charge) = &mut self.charge_up {
                charge.penalize(charge_up::RUSH_PENALTY);
            }
        }
    }

    /// Stops rushing, if the runner is rushed.
    pub(super) fn end_rush(&mut self, state: &mut RaceState) {
        if let Some(rush_state) = &mut self.rush_state
            && rush_state.is_rushed
        {
            rush_state.end(state.elapsed_time);
            let style = rush_state.style;
            self.log_end(EventKind::Rush { style }, state.elapsed_time);
            self.on_rush_change(state);
        }
    }

    /// Updates the race's count of rushed runners after this runner started or
    /// stopped rushing, and force a positioning strategy change immediately.
    fn on_rush_change(&mut self, state: &mut RaceState) {
        let is_rushed = self.is_rushed();
        let count =
            &mut state.rushed_by_team[self.initial_state.team as usize][self.base_style() as usize];
        if is_rushed {
            *count += 1;
        } else {
            *count -= 1;
        }
        self.reset_positioning(state.elapsed_time);
    }

    /// Ends the rush once it reaches its maximum duration or the runner snaps out
    /// of it.
    pub(crate) fn update_rush_state(&mut self, state: &mut RaceState) {
        if let Some(rush_state) = &mut self.rush_state
            && rush_state.is_rushed
            && rush_state.should_end(
                state.elapsed_time,
                &mut self.rng.rush,
                self.pins.rush.failed_checks,
            )
        {
            self.end_rush(state);
        }
    }

    /// Checks whether the runner has rushed at some point.
    pub(crate) fn has_rushed(&self) -> bool {
        self.rush_state.as_ref().is_some_and(RushState::has_rushed)
    }

    /// Starts a spot struggle (位置取り争い), unless the runner already struggled.
    pub(crate) fn start_spot_struggle(&mut self, elapsed_time: f64) {
        if self.spot_struggle.has_struggled() {
            return;
        }
        self.spot_struggle =
            SpotStruggle::start(self.adjusted.guts, self.raw.style_aptitude, elapsed_time);
        self.log_start(EventKind::SpotStruggle, elapsed_time);
        if let Some(charge) = &mut self.charge_up {
            charge.penalize(charge_up::STRUGGLE_PENALTY);
        }
    }

    /// Stops the spot struggle at `elapsed_time`, if it was started.
    pub(crate) fn stop_spot_struggle(&mut self, elapsed_time: f64) {
        self.spot_struggle.stop();
        self.log_end(EventKind::SpotStruggle, elapsed_time);
    }

    /// Checks whether the runner should try to secure the lead on this frame.
    pub(crate) fn is_secure_lead_check_due(&self, state: &RaceState) -> bool {
        self.style != EffectiveStyle::End
            && self.is_boost_check_due(&self.secure_lead, &secure_lead::SECTIONS, state)
    }

    /// Attempts to secure the lead (リード確保).
    ///
    /// If the lead over another runner with a later strategy is less than
    /// desirable, there is a chance for the runner to speed up for a short time at
    /// the cost of HP, followed by a cooldown.
    ///
    /// `is_alone` is whether no other runner with the same style is within 10m.
    pub(crate) fn try_secure_lead(
        &mut self,
        lead_insufficient: bool,
        is_alone: bool,
        state: &RaceState,
    ) {
        let elapsed_time = state.elapsed_time;
        if !lead_insufficient || !self.rng.secure_lead.random_bool(secure_lead::CHANCE) {
            self.secure_lead = TimedBoost::Idle {
                next_check: elapsed_time + secure_lead::CHECK_INTERVAL,
            };
            return;
        }

        let style = self.style;
        let speed_bonus = secure_lead::target_speed_bonus(
            style,
            self.adjusted.guts,
            is_alone,
            state.config.game_version,
        );
        self.secure_lead = TimedBoost::start(
            speed_bonus,
            elapsed_time,
            secure_lead::DURATION,
            secure_lead::COOLDOWN,
        );
        self.log_start(EventKind::SecureLead, elapsed_time);
        self.hp
            .consume(secure_lead::hp_cost(style, state.course.distance));
    }

    /// Checks whether the runner should try to compete before spurt (位置取り調整) on
    /// this frame.
    pub(crate) fn is_compete_check_due(&self, state: &RaceState) -> bool {
        self.is_boost_check_due(
            &self.compete_before_spurt,
            &compete_before_spurt::SECTIONS,
            state,
        )
    }

    /// Checks whether a check for `boost`, which can only happen in `sections`, is
    /// due on this frame (2.5 Anniv. onward).
    fn is_boost_check_due(
        &self,
        boost: &TimedBoost,
        sections: &RangeInclusive<u8>,
        state: &RaceState,
    ) -> bool {
        state.config.game_version >= GameVersion::TwoPointFiveAnniversary
            && !self.has_finished()
            && !self.hp.is_empty()
            && sections.contains(&self.section)
            && boost.is_check_due(state.elapsed_time)
    }

    /// Attempts to compete before spurt (位置取り調整).
    ///
    /// First, if the runner lacks the HP she wants to keep to finish the race, she
    /// may notice and enter Stamina Keep (持久力温存). Then, unless in Stamina Keep, she
    /// competes if she is too far from the lead or has other runners nearby,
    /// speeding up for a short time at the cost of HP, followed by a cooldown.
    ///
    /// `is_alone` is whether no other runner with the same style is within
    /// `compete_before_spurt::ALONE_DISTANCE`.
    pub(crate) fn try_compete_before_spurt(
        &mut self,
        far_from_lead: bool,
        is_alone: bool,
        state: &RaceState,
    ) {
        let elapsed_time = state.elapsed_time;
        if !self.stamina_keep.is_active
            && self.hp.current() < self.hp_required_to_finish(state) * self.stamina_keep.ratio
            && self
                .rng
                .stamina_keep
                .random_bool(compete_before_spurt::stamina_keep_chance(self.adjusted.wit))
        {
            self.stamina_keep.is_active = true;
            self.log_start(EventKind::StaminaKeep, elapsed_time);
        }
        // ? Chance formula is unknown; set to always trigger for now.
        if self.stamina_keep.is_active || !(far_from_lead || self.nearby.count > 0) {
            self.compete_before_spurt = TimedBoost::Idle {
                next_check: elapsed_time + compete_before_spurt::CHECK_INTERVAL,
            };
            return;
        }

        let style = self.style;
        let speed_bonus = compete_before_spurt::target_speed_bonus(
            style,
            self.adjusted.power,
            self.adjusted.guts,
            is_alone,
        );
        self.compete_before_spurt = TimedBoost::start(
            speed_bonus,
            elapsed_time,
            compete_before_spurt::DURATION,
            compete_before_spurt::COOLDOWN,
        );
        self.log_start(EventKind::CompeteBeforeSpurt, elapsed_time);
        self.hp.consume(compete_before_spurt::hp_cost(
            style,
            state.course.distance,
            far_from_lead,
        ));
    }

    /// Starts dueling (追い比べ), unless the runner already is.
    pub(crate) fn start_duel(&mut self, elapsed_time: f64) {
        if self.duel.is_dueling() {
            return;
        }
        self.duel = Duel::start(self.adjusted.guts);
        self.log_start(EventKind::Duel, elapsed_time);
    }

    /// Stops dueling at `elapsed_time`, if the runner is.
    pub(crate) fn stop_duel(&mut self, elapsed_time: f64) {
        self.duel.stop();
        self.log_end(EventKind::Duel, elapsed_time);
    }

    /// Ends the events whose effects have expired at `elapsed_time` but whose state
    /// doesn't change by itself: spot struggles and charge up releases.
    pub(crate) fn end_expired_events(&mut self, elapsed_time: f64) {
        let spot_struggle_expired = matches!(
            self.spot_struggle,
            SpotStruggle::Active { expires_at, .. } if elapsed_time >= expires_at
        );
        if spot_struggle_expired {
            self.log_end(EventKind::SpotStruggle, elapsed_time);
        }
        let release_expired = matches!(
            self.charge_up.as_ref().map(|charge_up| &charge_up.outcome),
            Some(ChargeUpOutcome::Released(release)) if elapsed_time >= release.expires_at
        );
        if release_expired {
            self.log_end(
                EventKind::ChargeUpRelease {
                    conserved: 0.0,
                    acceleration: 0.0,
                },
                elapsed_time,
            );
        }
    }
}
