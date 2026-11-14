//! A log of the race mechanics that a runner went through.
//!
//! Each runner records an [`Event`] whenever a mechanic starts and when it
//! ends, in chronological order of their start. It serves to show results and
//! to recreate game replays.
//!
//! Each start and end records the elapsed time and the runner's position at the
//! moment the mechanic's state changes, without adjusting them. So mechanics
//! updated before the runner moves in a frame (rushes, charge up releases, last
//! spurts, all-out spurts and stamina limit breaks, except the end of an
//! all-out spurt) record the position at the end of the previous frame, while
//! those updated after the move (spot struggles, duels, secure leads, competes
//! before spurt, stamina keeps and the end of timed effects) record the current
//! one.

use std::mem::discriminant;

use crate::race::runner::Runner;
use crate::uma::EffectiveStyle;

/// A race mechanic that a runner went through, such as a rush or a duel.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Event {
    /// The mechanic.
    pub kind: EventKind,
    /// Elapsed time when it started, in seconds.
    pub start_time: f64,
    /// Distance the runner had run when it started, in meters.
    pub start_distance: f64,
    /// When it ended, or `None` while ongoing, or if it lasted until the end of the
    /// race.
    pub end: Option<EventEnd>,
}

/// The end of an [`Event`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EventEnd {
    /// Elapsed time when it ended, in seconds.
    pub time: f64,
    /// Distance the runner had run when it ended, in meters.
    pub distance: f64,
}

/// A kind of [`Event`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum EventKind {
    /// Rushing (掛かり), which lasts until the runner snaps out of it.
    ///
    /// It happens at most once per race.
    Rush {
        /// Style the runner positions as while rushed.
        style: EffectiveStyle,
    },
    /// Spot struggle (位置取り争い), until its speed bonus stops.
    ///
    /// It happens at most once per race.
    SpotStruggle,
    /// Duel (追い比べ), until the runner exits it.
    ///
    /// It happens at most once per race.
    Duel,
    /// Release of the power conserved by charge up (脚色十分), until its acceleration
    /// bonus expires.
    ///
    /// It happens at most once per race, and only if enough power was conserved.
    ChargeUpRelease {
        /// Power conserved at the time of release.
        conserved: f64,
        /// Acceleration bonus in m/s².
        acceleration: f64,
    },
    /// Competing before the last spurt (位置取り調整), until its speed bonus expires. It
    /// can repeat after a cooldown.
    CompeteBeforeSpurt,
    /// Stamina keep (持久力温存), until a recovery skill resets it. It can repeat.
    StaminaKeep,
    /// Securing the lead (リード確保), until its speed bonus expires. It can repeat
    /// after a cooldown.
    SecureLead,
    /// Start of the last spurt (ラストスパート).
    ///
    /// It never ends, but its effects stop when the runner runs out of HP. It's
    /// recorded only once, even if the spurt is recalculated.
    LastSpurt,
    /// Start of the all-out spurt (全開スパート), until the runner runs out of HP, if she
    /// does.
    AllOutSpurt,
    /// Stamina Limit Break (スタミナ勝負) speed bonus, which starts when the runner
    /// reaches her last spurt speed.
    ///
    /// It never ends, but along with the last spurt, its effects stop when the
    /// runner runs out of HP.
    StaminaLimitBreak {
        /// Target speed bonus in m/s.
        speed_bonus: f64,
    },
}

impl Event {
    /// Returns an ongoing event of `kind` that started at `time` and `distance`.
    pub(crate) fn start(kind: EventKind, time: f64, distance: f64) -> Self {
        Event {
            kind,
            start_time: time,
            start_distance: distance,
            end: None,
        }
    }
}

impl Runner {
    /// Records the start of an event of `kind` at `time` and the runner's position.
    pub(crate) fn log_start(&mut self, kind: EventKind, time: f64) {
        self.events.push(Event::start(kind, time, self.position));
    }

    /// Records the end, at `time` and the runner's position, of the last ongoing
    /// event of the same kind as `kind`, ignoring its payload.
    ///
    /// Does nothing if there is none, so an event can't end twice.
    pub(crate) fn log_end(&mut self, kind: EventKind, time: f64) {
        let kind = discriminant(&kind);
        let distance = self.position;
        if let Some(event) = self
            .events
            .iter_mut()
            .rev()
            .find(|event| discriminant(&event.kind) == kind && event.end.is_none())
        {
            event.end = Some(EventEnd { time, distance });
        }
    }
}

#[cfg(test)]
mod tests;
