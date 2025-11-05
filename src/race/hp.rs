use crate::course::*;
use crate::race::parameters::TrackCondition;
use crate::race::solver::RaceState;
use crate::uma::Style;

fn hp_ground_modifier(track: Track, condition: TrackCondition) -> f64 {
    match (track, condition) {
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

pub fn hp_style_coefficient(style: Style) -> f64 {
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
/// # Arguments
///
/// - `current_speed` - Current speed of the runner in m/s
/// - `state` - Reference to the race state
/// - `phase` - Current phase of the race
/// - `guts_stat` - Guts stat of the runner
/// - `status_modifier` - Multiplicative modifier based on current status (e.g.,
///   rushed, pace-down, downhill). Should be 1.0 if no modifiers apply.
///
/// # Returns
///
/// HP consumption per second.
pub fn calculate_hp_consumption_per_second(
    current_speed: f64,
    state: &RaceState,
    phase: Phase,
    guts_stat: u16,
    status_modifier: f64,
) -> f64 {
    let speed_diff = current_speed - state.course.base_speed() + 12.0;
    let base_consumption = 20.0 * speed_diff.powi(2) / 144.0;
    let ground_modifier = hp_ground_modifier(state.course.surface, state.params.condition);

    let consumption = base_consumption * status_modifier * ground_modifier;

    // In late-race and last spurt, HP consumption is multiplied by guts modifier
    match phase {
        Phase::Late | Phase::Last => consumption * hp_guts_modifier(guts_stat),
        _ => consumption,
    }
}

pub fn estimate_hp_consumption(
    speed: f64,
    state: &RaceState,
    distance: f64,
    guts_stat: u16,
) -> f64 {
    // Estimate HP consumption for running at constant speed over distance
    // Using the HP consumption formula with Phase::Last

    // Time = distance / speed
    let time = distance / speed;

    // Calculate HP consumption per second at this speed
    // Assuming no special status modifiers (not rushed, not pace-down, not downhill)
    let hp_per_second = calculate_hp_consumption_per_second(
        speed,
        state,
        Phase::Last, // Last spurt phase for guts modifier
        guts_stat,
        1.0, // No special status modifier
    );

    hp_per_second * time
}
