use crate::course::*;
use crate::race::parameters::TrackCondition;
use crate::uma::Style;

/// Health Points (HP) state.
///
/// HP determines if the runner can sustain her target speed, decelerating
/// if it reaches 0.
/// It's also used to get the distance at which her last spurt can start.
pub struct HpState {
    /// Current HP. It is set to `max_hp` at the start of the race and decreases
    /// over time. It can be restored by skills but cannot exceed `max_hp`.
    ///
    /// If HP reaches 0, the runner's maximum speed is greatly reduced.
    pub current: f64,
    /// Maximum HP. Calculated from the runner's stamina stat at the start of the race.
    pub maximum: f64,
    /// Flag indicating if the runner has emptied her HP at least once.
    pub has_emptied: bool,
}

impl HpState {
    /// Create a new HP state with given maximum HP.
    pub fn new(maximum: f64) -> Self {
        Self {
            current: maximum,
            maximum,
            has_emptied: false,
        }
    }

    /// Check if HP is empty (i.e., current HP <= 0).
    pub fn is_empty(&self) -> bool {
        self.current <= 0.0
    }

    /// Get current HP percentage.
    pub fn percent(&self) -> f64 {
        self.current / self.maximum * 100.0
    }

    /// Recover HP by a given amount, clamped to maximum.
    pub fn recover(&mut self, amount: f64) {
        self.current = (self.current + amount).min(self.maximum);
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

fn hp_guts_modifier(guts_stat: u16) -> f64 {
    1.0 + (200.0 / (600.0 * guts_stat as f64).sqrt())
}

/// Calculate HP style multiplier based on running style.
pub fn hp_style_multiplier(style: Style) -> f64 {
    match style {
        Style::Runaway => 0.86,
        Style::Front => 0.95,
        Style::Pace => 0.89,
        Style::Late => 1.0,
        Style::End => 0.995,
    }
}

/// Calculate HP consumption per second.
///
/// ## Arguments
///
/// - `current_speed` - Current speed of the runner in m/s
/// - `state` - Reference to the race state
/// - `phase` - Current phase of the race
/// - `guts_stat` - Guts stat of the runner
/// - `status_modifier` - Multiplicative modifier based on current status (e.g.,
///   rushed, pace-down, downhill). Should be 1.0 if no modifiers apply.
///
/// ## Returns
///
/// HP consumed per second traveled at the given speed.
pub fn calculate_hp_per_second(
    current_speed: f64,
    course: &Course,
    track_condition: TrackCondition,
    phase: Phase,
    guts_stat: u16,
    status_modifier: f64,
) -> f64 {
    let speed_diff = current_speed - course.base_speed() + 12.0;
    let base_consumption = 20.0 * speed_diff.powi(2) / 144.0;
    let ground_modifier = hp_ground_modifier(course.surface, track_condition);

    let consumption = base_consumption * status_modifier * ground_modifier;

    // In late-race and last spurt, HP consumption is multiplied by guts modifier
    match phase {
        Phase::Late | Phase::Last => consumption * hp_guts_modifier(guts_stat),
        _ => consumption,
    }
}

/// Calculate HP consumption per meter at a given speed during last spurt.
///
/// This is useful for analytical calculations where you need to solve
/// for distances based on available HP.
///
/// ## Arguments
///
/// - `speed` - Speed in m/s
/// - `course` - Course information
/// - `track_condition` - Track condition
/// - `guts_stat` - Guts stat of the runner
///
/// ## Returns
///
/// HP consumed per meter traveled at the given speed.
pub fn estimate_hp_per_meter_last_spurt(
    speed: f64,
    course: &Course,
    track_condition: TrackCondition,
    guts_stat: u16,
) -> f64 {
    let hp_per_second = calculate_hp_per_second(
        speed,
        course,
        track_condition,
        Phase::Late,
        guts_stat,
        1.0, // No special status modifier (not rushed, not pace-down, not downhill)
    );
    hp_per_second / speed
}

/// Estimate HP consumption at a given speed over a distance during last spurt.
///
/// ## Arguments
///
/// - `speed` - Speed in m/s
/// - `course` - Course information
/// - `track_condition` - Track condition
/// - `distance` - Distance to be covered in meters
/// - `guts_stat` - Guts stat of the runner
///
/// ## Returns
///
/// HP consumed during last spurt.
pub fn estimate_hp_last_spurt(
    speed: f64,
    course: &Course,
    track_condition: TrackCondition,
    distance: f64,
    guts_stat: u16,
) -> f64 {
    let time = distance / speed;
    let hp_per_second = calculate_hp_per_second(
        speed,
        course,
        track_condition,
        Phase::Late,
        guts_stat,
        1.0, // No special status modifier (not rushed, not pace-down, not downhill)
    );
    hp_per_second * time
}
