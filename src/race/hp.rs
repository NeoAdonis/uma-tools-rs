//! Health Points (HP) state and calculations.

use crate::course::*;
use crate::race::parameters::TrackCondition;
use crate::uma::EffectiveStyle;

/// Health Points (HP) state.
///
/// HP determines if the runner can sustain her target speed, decelerating
/// if it reaches 0.
///
/// It's also used to get the distance at which her last spurt can start.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HpState {
    /// Current HP.
    ///
    /// HP is set to its maximum at the start of the race and decreases over time.
    /// It can be restored by skills but cannot exceed `maximum`.
    ///
    /// If HP reaches 0, the runner's maximum speed is greatly reduced.
    current: f64,
    /// Maximum HP.
    ///
    /// Calculated from the stamina stat at the start of the race.
    maximum: f64,
    /// Flag indicating if the runner has emptied her HP at least once.
    has_emptied: bool,
}

impl HpState {
    /// Create a new HP state with given maximum HP.
    pub(crate) fn new(maximum: f64) -> Self {
        Self {
            current: maximum,
            maximum,
            has_emptied: false,
        }
    }

    /// Get the current HP.
    pub fn current(&self) -> f64 {
        self.current
    }

    /// Get the maximum HP.
    pub fn maximum(&self) -> f64 {
        self.maximum
    }

    /// Check if HP has been emptied at least once.
    pub fn has_emptied(&self) -> bool {
        self.has_emptied
    }

    /// Check if HP is empty (i.e., current HP <= 0).
    pub fn is_empty(&self) -> bool {
        self.current <= 0.0
    }

    /// Get current HP percentage.
    pub fn percent(&self) -> f64 {
        self.current / self.maximum * 100.0
    }

    /// Consume HP by a given amount, recording whether it has been emptied.
    pub(crate) fn consume(&mut self, amount: f64) {
        self.current -= amount;
        if self.is_empty() {
            self.has_emptied = true;
        }
    }

    /// Recover HP by a given amount, clamped to maximum.
    pub(crate) fn recover(&mut self, amount: f64) {
        self.current = (self.current + amount).min(self.maximum);
    }

    /// Set the current HP to `value`, recording whether it has been emptied.
    ///
    /// Unlike [`recover`](Self::recover), `value` isn't clamped to the maximum;
    /// exceeding it fails in debug builds instead.
    #[cfg(test)]
    pub(crate) fn set_current(&mut self, value: f64) {
        debug_assert!(
            value <= self.maximum,
            "HP {value} exceeds maximum {}",
            self.maximum
        );
        self.current = value;
        self.has_emptied |= self.is_empty();
    }
}

fn hp_ground_modifier(track: Track, track_condition: TrackCondition) -> f64 {
    match (track, track_condition) {
        (Track::Turf, TrackCondition::Firm) => 1.0,
        (Track::Turf, TrackCondition::Good) => 1.0,
        (Track::Turf, TrackCondition::Soft) => 1.02,
        (Track::Turf, TrackCondition::Heavy) => 1.02,
        (Track::Dirt, TrackCondition::Firm) => 1.0,
        (Track::Dirt, TrackCondition::Good) => 1.0,
        (Track::Dirt, TrackCondition::Soft) => 1.01,
        (Track::Dirt, TrackCondition::Heavy) => 1.02,
    }
}

fn hp_guts_modifier(guts_stat: f64) -> f64 {
    1.0 + (200.0 / (600.0 * guts_stat).sqrt())
}

fn hp_style_multiplier(style: EffectiveStyle) -> f64 {
    match style {
        EffectiveStyle::Runaway => 0.86,
        EffectiveStyle::Front => 0.95,
        EffectiveStyle::Pace => 0.89,
        EffectiveStyle::Late => 1.0,
        EffectiveStyle::End => 0.995,
    }
}

/// Calculate maximum HP from the stamina stat, running style and course.
///
/// Stats should already include any adjustments applied before the race starts.
pub(crate) fn calculate_max_hp(style: EffectiveStyle, stamina_stat: f64, course: &Course) -> f64 {
    0.8 * hp_style_multiplier(style) * stamina_stat + course.distance as f64
}

/// Calculate HP consumption per second.
///
/// `status_modifier` is a multiplicative coefficient based on current status
/// (e.g., rushed, pace-down, downhill). Should be 1.0 if no modifiers apply.
///
/// Returns the HP consumed per second traveled at the given speed.
pub(crate) fn calculate_hp_per_second(
    current_speed: f64,
    course: &Course,
    track_condition: TrackCondition,
    phase: Phase,
    guts_stat: f64,
    status_modifier: f64,
) -> f64 {
    let speed_diff = current_speed - course.base_speed() + 12.0;
    let base_consumption = 20.0 * speed_diff.powi(2) / 144.0;
    let ground_modifier = hp_ground_modifier(course.surface, track_condition);

    let consumption = base_consumption * status_modifier * ground_modifier;

    // In late-race and last spurt, HP consumption is multiplied by guts modifier.
    match phase {
        Phase::Late | Phase::Last => consumption * hp_guts_modifier(guts_stat),
        _ => consumption,
    }
}

/// Calculate HP consumption per meter at a given speed during last spurt.
///
/// This is useful for analytical calculations where you need to solve for
/// distances based on available HP.
///
/// Returns the HP consumed per meter traveled at the given speed.
pub(crate) fn estimate_hp_per_meter_last_spurt(
    speed: f64,
    course: &Course,
    track_condition: TrackCondition,
    guts_stat: f64,
) -> f64 {
    let hp_per_second = calculate_hp_per_second(
        speed,
        course,
        track_condition,
        Phase::Late,
        guts_stat,
        // No special status modifier (not rushed, not pace-down, not downhill).
        1.0,
    );
    hp_per_second / speed
}

/// Estimate HP consumption at a given speed over a distance during last spurt.
///
/// Returns the HP consumed during last spurt.
pub(crate) fn estimate_hp_last_spurt(
    speed: f64,
    course: &Course,
    track_condition: TrackCondition,
    distance: f64,
    guts_stat: f64,
) -> f64 {
    estimate_hp_per_meter_last_spurt(speed, course, track_condition, guts_stat) * distance
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::{assert_close, synthetic_course};

    #[test]
    fn test_max_hp() {
        // 2000m course.
        let max_hp = calculate_max_hp(EffectiveStyle::Pace, 1000.0, &synthetic_course());
        assert_close!(max_hp, 0.8 * 0.89 * 1000.0 + 2000.0);
    }

    #[test]
    fn test_hp_per_second() {
        use Phase::*;
        use Track::*;
        use TrackCondition::*;

        // At the base speed of the 2000m course (20 m/s) on firm turf, before
        // the Late race and with no status: 20 HP/s.
        let cases = [
            ("base", 20.0, Turf, Firm, Mid, 1.0, 20.0),
            (
                "faster",
                26.0,
                Turf,
                Firm,
                Mid,
                1.0,
                20.0 * 18.0 * 18.0 / 144.0,
            ),
            // 600 guts: 1 + 200 / 600.
            (
                "guts",
                20.0,
                Turf,
                Firm,
                Late,
                1.0,
                20.0 * (1.0 + 1.0 / 3.0),
            ),
            ("ground", 20.0, Dirt, Soft, Mid, 1.0, 20.0 * 1.01),
            ("status", 20.0, Turf, Firm, Mid, 1.6, 20.0 * 1.6),
        ];
        for (case, speed, surface, condition, phase, status, expected) in cases {
            let course = Course {
                surface,
                ..synthetic_course()
            };
            let actual = calculate_hp_per_second(speed, &course, condition, phase, 600.0, status);
            assert_close!(actual, expected, "{case}");
        }
    }

    #[test]
    fn test_recover_is_capped_at_maximum() {
        let mut hp = HpState::new(1000.0);
        hp.consume(300.0);
        hp.recover(500.0);
        assert_eq!(hp.current(), 1000.0);
    }

    #[test]
    fn test_has_emptied_stays_after_recovery() {
        let mut hp = HpState::new(1000.0);
        hp.consume(999.0);
        assert!(!hp.has_emptied());
        hp.consume(1.0);
        assert!(hp.is_empty() && hp.has_emptied());
        hp.recover(100.0);
        assert!(!hp.is_empty() && hp.has_emptied());
    }
}
