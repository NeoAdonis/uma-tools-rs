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
        Style::Oonige => 0.86,
        Style::Front => 0.95,
        Style::Pace => 0.89,
        Style::Late => 1.0,
        Style::End => 0.995,
    }
}

pub fn calculate_hp_consumption_per_second(
    current_speed: f64,
    state: &RaceState,
    phase: Phase,
    guts_stat: u16,
    is_rushed: bool,
    is_pace_down: bool,
    is_downhill_accel: bool,
) -> f64 {
    let speed_diff = current_speed - state.course.base_speed() + 12.0;
    let base_consumption = 20.0 * speed_diff.powi(2) / 144.0;

    let mut status_modifier = 1.0;
    if is_rushed {
        status_modifier *= 1.6;
    }
    if is_pace_down {
        status_modifier *= 0.6;
    }
    if is_downhill_accel {
        status_modifier *= 0.4;
    }

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
        false, // is_rushed
        false, // is_pace_down
        false, // is_downhill_accel
    );

    hp_per_second * time
}
