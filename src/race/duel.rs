//! Duel (Compete Fight, 追い比べ) formulas (1st Anniversary onward).
//!
//! On the final straight, two runners that stay close for a while may start
//! dueling, gaining target speed and acceleration based on their guts. A
//! runner keeps dueling until her HP runs low or she gets far from everyone
//! who has dueled during the race. Once she leaves, she can't duel again.

/// Maximum distance (m) between two runners to count as close.
pub const TRIGGER_DISTANCE: f64 = 3.0;
/// Maximum lane gap (in course widths) between two runners to count as close.
pub const TRIGGER_LANE_GAP: f64 = 0.25;
/// Time (s) two runners must stay close in a row before they can start dueling,
/// counted from the first frame they're close.
pub const TRIGGER_TIME: f64 = 2.0;
/// Maximum speed difference (m/s) between two runners to start dueling.
pub const MAX_SPEED_DIFF: f64 = 0.6;
/// Maximum order rate (top X%) at least one of the runners must be at to start
/// dueling.
pub const MAX_ORDER_RATE: u8 = 50;
/// Minimum HP percentage both runners must have to start dueling.
pub const MIN_START_HP_PERCENT: f64 = 15.0;
/// HP percentage below which a duelist exits.
pub const EXIT_HP_PERCENT: f64 = 5.0;
/// Distance (m) from every other duelist, ahead or behind, at which a duelist
/// exits.
pub const EXIT_DISTANCE: f64 = 5.0;

/// Tolerance for floating-point rounding errors in the trigger checks, so that
/// e.g. 30 frames at 15 FPS reach `TRIGGER_TIME`, and a speed difference a hair
/// over `MAX_SPEED_DIFF` still counts.
const TOLERANCE: f64 = 1e-6;

/// Duel state.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Duel {
    /// Not started yet.
    Pending,
    /// Dueling, with the given target speed (m/s) and acceleration (m/s²) bonuses.
    Active {
        speed_bonus: f64,
        acceleration_bonus: f64,
    },
    /// Left the duel. It can't start again, but the runner still counts for other
    /// duelists' exit checks.
    Ended,
}

impl Duel {
    /// Start dueling.
    pub fn start(guts: u16) -> Self {
        Duel::Active {
            speed_bonus: target_speed_bonus(guts),
            acceleration_bonus: acceleration_bonus(guts),
        }
    }

    /// Check if the runner is currently dueling.
    pub fn is_dueling(&self) -> bool {
        matches!(self, Duel::Active { .. })
    }

    /// Check if the runner has dueled at some point, even if she already left.
    pub fn has_dueled(&self) -> bool {
        !matches!(self, Duel::Pending)
    }

    /// Check if the runner has left a duel, and thus can't duel again.
    pub fn has_ended(&self) -> bool {
        matches!(self, Duel::Ended)
    }

    /// Target speed increase (m/s), zero if not dueling.
    pub fn speed_bonus(&self) -> f64 {
        match self {
            Duel::Active { speed_bonus, .. } => *speed_bonus,
            _ => 0.0,
        }
    }

    /// Acceleration increase (m/s²), zero if not dueling.
    pub fn acceleration_bonus(&self) -> f64 {
        match self {
            Duel::Active {
                acceleration_bonus, ..
            } => *acceleration_bonus,
            _ => 0.0,
        }
    }

    /// Leave the duel, if dueling.
    pub fn stop(&mut self) {
        if self.is_dueling() {
            *self = Duel::Ended;
        }
    }
}

/// Check if two runners `distance` meters and `lane_diff` course widths apart
/// are close enough to start counting towards a duel.
pub fn is_close(distance: f64, lane_diff: f64) -> bool {
    distance.abs() < TRIGGER_DISTANCE && lane_diff.abs() < TRIGGER_LANE_GAP
}

/// Check if two runners have been close for long enough to start dueling,
/// given how long (in seconds) they have been close in a row.
pub fn is_close_long_enough(close_time: f64) -> bool {
    close_time >= TRIGGER_TIME - TOLERANCE
}

/// Check if two runners' speeds (m/s) are similar enough to start dueling.
pub fn are_speeds_similar(speed: f64, other_speed: f64) -> bool {
    (speed - other_speed).abs() <= MAX_SPEED_DIFF + TOLERANCE
}

/// Check if a duelist `distance` meters away from another duelist is
/// separated from her, either ahead or behind.
pub fn is_separated(distance: f64) -> bool {
    distance.abs() >= EXIT_DISTANCE
}

/// Target speed increase (m/s) while dueling.
fn target_speed_bonus(guts: u16) -> f64 {
    (200.0 * guts as f64).powf(0.708) * 0.0001
}

/// Acceleration increase (m/s²) while dueling.
fn acceleration_bonus(guts: u16) -> f64 {
    (160.0 * guts as f64).powf(0.59) * 0.0001
}

#[cfg(test)]
mod tests;
