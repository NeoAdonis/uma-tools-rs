//! Short target speed boosts checked periodically, followed by a cooldown.
//!
//! Shared by secure lead and compete before spurt: on each check, a runner
//! either waits for the next one or speeds up for a short time, after which she
//! must wait for a cooldown before the next check.

/// Timed boost state.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum TimedBoost {
    /// Waiting for the next check at `next_check` (s).
    Idle { next_check: f64 },
    /// Boosting target speed by `speed_bonus` (m/s) until `expires_at` (s).
    /// The next check is at `next_check` (s), after the cooldown.
    Active {
        speed_bonus: f64,
        expires_at: f64,
        next_check: f64,
    },
}

impl TimedBoost {
    /// Starts boosting target speed by `speed_bonus` at `elapsed_time`, for
    /// `duration` seconds followed by a `cooldown` in seconds.
    pub(crate) fn start(speed_bonus: f64, elapsed_time: f64, duration: f64, cooldown: f64) -> Self {
        let expires_at = elapsed_time + duration;
        TimedBoost::Active {
            speed_bonus,
            expires_at,
            next_check: expires_at + cooldown,
        }
    }

    /// Checks if the boost is idle and its next check is due at `elapsed_time`.
    pub(crate) fn is_check_due(&self, elapsed_time: f64) -> bool {
        matches!(self, TimedBoost::Idle { next_check } if elapsed_time >= *next_check)
    }

    /// Returns the target speed increase (m/s); zero if idle.
    pub(crate) fn speed_bonus(&self) -> f64 {
        match self {
            TimedBoost::Active { speed_bonus, .. } => *speed_bonus,
            TimedBoost::Idle { .. } => 0.0,
        }
    }

    /// Stops the boost once it expires at `elapsed_time`, waiting for the next
    /// check after the cooldown. Returns whether it stopped.
    pub(crate) fn expire(&mut self, elapsed_time: f64) -> bool {
        if let TimedBoost::Active {
            expires_at,
            next_check,
            ..
        } = *self
            && elapsed_time >= expires_at
        {
            *self = TimedBoost::Idle { next_check };
            return true;
        }
        false
    }
}

#[cfg(test)]
mod tests;
