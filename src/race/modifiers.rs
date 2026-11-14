//! Stat and speed modifiers shared by several race mechanics.
//!
//! Multipliers and offsets that depend on the runner's aptitudes and style, the
//! race phase, the track and its condition, the course's stat bonuses, slopes
//! and limit break buffs, along with the adjusted stats derived from them.

use rand::{Rng, RngExt};

use crate::configuration::GameVersion;
use crate::course::{Phase, ThresholdStat, Track};
use crate::race::parameters::TrackCondition;
use crate::uma::{Aptitude, EffectiveStyle, Stats};

/// Calculates acceleration strategy phase multiplier based on running style.
pub(crate) fn acceleration_strategy_phase_multiplier(style: EffectiveStyle, phase: Phase) -> f64 {
    match (style, phase) {
        // Early-race.
        (EffectiveStyle::Front, Phase::RunUp | Phase::Early) => 1.0,
        (EffectiveStyle::Pace, Phase::RunUp | Phase::Early) => 0.985,
        (EffectiveStyle::Late, Phase::RunUp | Phase::Early) => 0.975,
        (EffectiveStyle::End, Phase::RunUp | Phase::Early) => 0.945,
        (EffectiveStyle::Runaway, Phase::RunUp | Phase::Early) => 1.17,
        // Mid-race.
        (EffectiveStyle::Runaway, Phase::Mid) => 0.94,
        (_, Phase::Mid) => 1.0, // All styles except Runaway
        // Late-race and Last spurt.
        (EffectiveStyle::Front, Phase::Late | Phase::Last) => 0.996,
        (EffectiveStyle::Pace, Phase::Late | Phase::Last) => 0.996,
        (EffectiveStyle::Late, Phase::Late | Phase::Last) => 1.0,
        (EffectiveStyle::End, Phase::Late | Phase::Last) => 0.997,
        (EffectiveStyle::Runaway, Phase::Late | Phase::Last) => 0.956,
    }
}

/// Returns the wit multiplier for the wit limit break buff (5th Anniv. onward).
///
/// Zero at 1200 wit or less; it then grows in brackets up to a maximum of 1.12.
pub(crate) fn wit_limit_break_multiplier(wit: f64) -> f64 {
    let brackets = |from: f64, step: f64| ((wit - from) / step).ceil();
    if wit <= 1200.0 {
        0.0
    } else if wit <= 1400.0 {
        brackets(1200.0, 20.0) * 0.02
    } else if wit <= 1600.0 {
        0.2 + brackets(1400.0, 20.0) * 0.06
    } else if wit <= 2000.0 {
        0.8 + brackets(1600.0, 20.0) * 0.01
    } else {
        (1.0 + brackets(2000.0, 100.0) * 0.01).min(1.12)
    }
}

/// Returns the phase strategy coefficient for the wit limit break buff (5th
/// Anniv. onward).
pub(crate) fn wit_limit_break_phase_coefficient(style: EffectiveStyle, phase: Phase) -> f64 {
    match (style, phase) {
        (EffectiveStyle::Front | EffectiveStyle::Runaway, Phase::RunUp | Phase::Early) => 0.26,
        (EffectiveStyle::Front | EffectiveStyle::Runaway, Phase::Mid) => 0.23,
        (EffectiveStyle::Front | EffectiveStyle::Runaway, Phase::Late) => 0.19,
        (EffectiveStyle::Front | EffectiveStyle::Runaway, Phase::Last) => 0.16,
        (EffectiveStyle::Pace, _) => 0.21,
        (EffectiveStyle::Late, Phase::RunUp | Phase::Early) => 0.19,
        (EffectiveStyle::Late, Phase::Mid) => 0.18,
        (EffectiveStyle::Late, Phase::Late) => 0.23,
        (EffectiveStyle::Late, Phase::Last) => 0.24,
        (EffectiveStyle::End, Phase::RunUp | Phase::Early) => 0.15,
        (EffectiveStyle::End, Phase::Mid) => 0.17,
        (EffectiveStyle::End, Phase::Late) => 0.25,
        (EffectiveStyle::End, Phase::Last) => 0.27,
    }
}

/// Returns the distance factor for the Stamina Limit Break buff (2.5 Anniv.
/// onward).
pub(crate) fn stamina_limit_break_distance_factor(distance: i32, game_version: GameVersion) -> f64 {
    let is_mecha_uma = game_version >= GameVersion::MechaUmaUpdate;
    match distance {
        ..=2100 => 0.0,
        2101..=2200 => 0.5,
        2201..=2400 => 1.0,
        2401..=2600 if is_mecha_uma => 1.5,
        2401..=2600 => 1.2,
        _ if is_mecha_uma => 1.8,
        _ => 1.5,
    }
}

/// Returns the Stamina Limit Break target speed buff in m/s, without its random
/// factor (2.5 Anniv. onward).
///
/// Zero at 1200 stamina or below, or on courses up to 2100m.
pub(crate) fn stamina_limit_break_base_buff(
    stamina: f64,
    distance: i32,
    game_version: GameVersion,
) -> f64 {
    if game_version < GameVersion::TwoPointFiveAnniversary || stamina <= 1200.0 {
        return 0.0;
    }
    (stamina - 1200.0).sqrt() * 0.0085 * stamina_limit_break_distance_factor(distance, game_version)
}

/// Returns the random factor of the Stamina Limit Break buff.
///
/// The game's parameter file lists a weighted random table and a parameter that
/// suggests power is involved, but observed values stay between 0.98 and
/// 1.02. The variation is small enough that other simulators and sources treat
/// the coefficient as always 1.0, so the same is done here. The RNG is still
/// taken so call sites stay the same if the roll is modeled in the future.
pub(crate) fn stamina_limit_break_random_factor<R: Rng + ?Sized>(_rng: &mut R) -> f64 {
    1.0
}

/// Calculates acceleration multiplier based on track aptitude.
pub(crate) fn ground_type_proficiency_multiplier(track_aptitude: Aptitude) -> f64 {
    match track_aptitude {
        Aptitude::S => 1.05,
        Aptitude::A => 1.0,
        Aptitude::B => 0.9,
        Aptitude::C => 0.8,
        Aptitude::D => 0.7,
        Aptitude::E => 0.5,
        Aptitude::F => 0.3,
        Aptitude::G => 0.1,
    }
}

/// Calculates acceleration multiplier based on distance aptitude.
pub(crate) fn acceleration_distance_proficiency_multiplier(distance_aptitude: Aptitude) -> f64 {
    match distance_aptitude {
        Aptitude::S | Aptitude::A | Aptitude::B | Aptitude::C | Aptitude::D => 1.0,
        Aptitude::E => 0.6,
        Aptitude::F => 0.5,
        Aptitude::G => 0.4,
    }
}

/// Calculates strategy phase multiplier based on running style.
pub(crate) fn strategy_phase_multiplier(style: EffectiveStyle, phase: Phase) -> f64 {
    match (style, phase) {
        // Early-race.
        (EffectiveStyle::Front, Phase::RunUp | Phase::Early) => 1.0,
        (EffectiveStyle::Pace, Phase::RunUp | Phase::Early) => 0.978,
        (EffectiveStyle::Late, Phase::RunUp | Phase::Early) => 0.938,
        (EffectiveStyle::End, Phase::RunUp | Phase::Early) => 0.931,
        (EffectiveStyle::Runaway, Phase::RunUp | Phase::Early) => 1.063,
        // Mid-race.
        (EffectiveStyle::Front, Phase::Mid) => 0.98,
        (EffectiveStyle::Pace, Phase::Mid) => 0.991,
        (EffectiveStyle::Late, Phase::Mid) => 0.998,
        (EffectiveStyle::End, Phase::Mid) => 1.0,
        (EffectiveStyle::Runaway, Phase::Mid) => 0.962,
        // Late-race and Last spurt.
        (EffectiveStyle::Front, Phase::Late | Phase::Last) => 0.962,
        (EffectiveStyle::Pace, Phase::Late | Phase::Last) => 0.975,
        (EffectiveStyle::Late, Phase::Late | Phase::Last) => 0.994,
        (EffectiveStyle::End, Phase::Late | Phase::Last) => 1.0,
        (EffectiveStyle::Runaway, Phase::Late | Phase::Last) => 0.95,
    }
}

/// Calculates speed multiplier based on distance aptitude.
pub(crate) fn distance_proficiency_multiplier(distance_aptitude: Aptitude) -> f64 {
    match distance_aptitude {
        Aptitude::S => 1.05,
        Aptitude::A => 1.0,
        Aptitude::B => 0.9,
        Aptitude::C => 0.8,
        Aptitude::D => 0.6,
        Aptitude::E => 0.4,
        Aptitude::F => 0.2,
        Aptitude::G => 0.1,
    }
}

/// Calculates base target speed based on various factors.
pub(crate) fn base_target_speed(
    base_speed: f64,
    style: EffectiveStyle,
    phase: Phase,
    speed_stat: f64,
    distance_aptitude: Aptitude,
) -> f64 {
    let base = base_speed * strategy_phase_multiplier(style, phase);
    // Speed stat only affects target speed during late-race and last spurt.
    match phase {
        Phase::RunUp | Phase::Early | Phase::Mid => base,
        Phase::Late | Phase::Last => {
            let speed_modifier = (500.0 * speed_stat).sqrt()
                * distance_proficiency_multiplier(distance_aptitude)
                * 0.002;
            base + speed_modifier
        }
    }
}

/// Returns the target speed modifier (m/s) in downhill mode on a `slope`
/// (percent; negative downhill).
pub(crate) fn downhill_speed_modifier(slope: f64) -> f64 {
    debug_assert!(slope < 0.0, "Not a downhill slope");
    -slope / 10.0 + 0.3
}

/// Returns the target speed modifier (m/s) on an uphill `slope` (percent),
/// given the power stat.
///
/// This modifier is negative, as uphills reduce speed.
pub(crate) fn uphill_speed_modifier(slope: f64, power: f64) -> f64 {
    -slope * 200.0 / power
}

/// Rolls section speed modifier range randomly based on wit stat.
pub(crate) fn roll_section_speed_modifier<R: Rng + ?Sized>(
    rng: &mut R,
    wit_stat: f64,
    base_speed: f64,
) -> f64 {
    let max = wit_stat * (wit_stat * 0.1).log10() / 550000.0;
    let min = max - 0.0065;
    rng.random_range(min..=max) * base_speed
}

/// Calculates track speed modifier based on track condition.
pub(crate) fn track_speed_modifier(track_condition: TrackCondition) -> f64 {
    match track_condition {
        TrackCondition::Heavy => 50.0,
        _ => 0.0,
    }
}

/// Calculates track power modifier based on track type and condition.
pub(crate) fn track_power_modifier(track: Track, track_condition: TrackCondition) -> f64 {
    match (track, track_condition) {
        (Track::Turf, TrackCondition::Firm) => 0.0,
        (Track::Turf, TrackCondition::Good) => 50.0,
        (Track::Turf, TrackCondition::Soft) => 50.0,
        (Track::Turf, TrackCondition::Heavy) => 50.0,
        (Track::Dirt, TrackCondition::Firm) => 100.0,
        (Track::Dirt, TrackCondition::Good) => 50.0,
        (Track::Dirt, TrackCondition::Soft) => 100.0,
        (Track::Dirt, TrackCondition::Heavy) => 100.0,
    }
}

/// Returns the style proficiency multiplier based on style aptitude.
///
/// Applied to the wit stat and to the spot struggle duration.
pub(crate) fn strategy_proficiency_multiplier(style_aptitude: Aptitude) -> f64 {
    match style_aptitude {
        Aptitude::S => 1.1,
        Aptitude::A => 1.0,
        Aptitude::B => 0.85,
        Aptitude::C => 0.75,
        Aptitude::D => 0.6,
        Aptitude::E => 0.4,
        Aptitude::F => 0.2,
        Aptitude::G => 0.1,
    }
}

/// Calculates adjusted speed after applying course and track modifiers, before
/// the career bonus and the clamp to at least 1.
///
/// Takes all base stats, since the course speed modifier depends on the
/// course's threshold stats, not just on speed.
pub(crate) fn calculate_adjusted_speed(
    base: &Stats,
    track_condition: TrackCondition,
    course_set_status: &[ThresholdStat],
) -> f64 {
    let course_modifier = course_speed_modifier(base, course_set_status);
    base.speed * course_modifier - track_speed_modifier(track_condition)
}

/// Calculates adjusted power after applying track modifier, before the career
/// bonus and the clamp to at least 1.
pub(crate) fn calculate_adjusted_power(
    base_power: f64,
    track: Track,
    track_condition: TrackCondition,
) -> f64 {
    base_power - track_power_modifier(track, track_condition)
}

/// Calculates adjusted wit after applying style aptitude modifier, before the
/// career bonus and the clamp to at least 1.
pub(crate) fn calculate_adjusted_wit(base_wit: f64, style_aptitude: Aptitude) -> f64 {
    base_wit * strategy_proficiency_multiplier(style_aptitude)
}

/// Calculates course speed modifier based on course set status.
///
/// Each course has a set of stats that provide bonuses if the corresponding
/// base stats exceed certain thresholds. If multiple stats are
/// present, their bonuses are averaged.
fn course_speed_modifier(base: &Stats, course_set_status: &[ThresholdStat]) -> f64 {
    if course_set_status.is_empty() {
        return 1.0;
    }

    let total_bonus: f64 = course_set_status
        .iter()
        .map(|&stat_type| {
            let stat_value = base.stat(stat_type);
            threshold_stat_bonus(stat_value)
        })
        .sum();

    // Return average bonus as multiplier (1.0 + average bonus).
    1.0 + (total_bonus / course_set_status.len() as f64)
}

/// Calculates bonus multiplier based on threshold stat value.
fn threshold_stat_bonus(stat_value: f64) -> f64 {
    if stat_value <= 300.0 {
        0.05
    } else if stat_value <= 600.0 {
        0.1
    } else if stat_value <= 900.0 {
        0.15
    } else {
        0.2
    }
}

#[cfg(test)]
mod tests;
