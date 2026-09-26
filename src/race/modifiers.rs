use rand::{Rng, RngExt};

use crate::configuration::GameVersion;
use crate::course::Phase;
use crate::uma::{Aptitude, Style};

/// Calculate acceleration strategy phase multiplier based on running style.
pub fn acceleration_strategy_phase_multiplier(style: Style, phase: Phase) -> f64 {
    match (style, phase) {
        // Early-race
        (Style::Front, Phase::Early) => 1.0,
        (Style::Pace, Phase::Early) => 0.985,
        (Style::Late, Phase::Early) => 0.975,
        (Style::End, Phase::Early) => 0.945,
        (Style::Runaway, Phase::Early) => 1.17,
        // Mid-race
        (Style::Runaway, Phase::Mid) => 0.94,
        (_, Phase::Mid) => 1.0, // All styles except Runaway
        // Late-race and Last spurt
        (Style::Front, Phase::Late | Phase::Last) => 0.996,
        (Style::Pace, Phase::Late | Phase::Last) => 0.996,
        (Style::Late, Phase::Late | Phase::Last) => 1.0,
        (Style::End, Phase::Late | Phase::Last) => 0.997,
        (Style::Runaway, Phase::Late | Phase::Last) => 0.956,
    }
}

/// Wiz multiplier for the Wiz Limit Break buff (5th Anniversary onward).
///
/// Zero at 1200 wiz or below; it then grows in brackets up to a maximum of 1.12.
pub fn wiz_limit_break_multiplier(wiz: u16) -> f64 {
    let brackets = |from: u16, step: f64| ((wiz - from) as f64 / step).ceil();
    match wiz {
        0..=1200 => 0.0,
        1201..=1400 => brackets(1200, 20.0) * 0.02,
        1401..=1600 => 0.2 + brackets(1400, 20.0) * 0.06,
        1601..=2000 => 0.8 + brackets(1600, 20.0) * 0.01,
        _ => (1.0 + brackets(2000, 100.0) * 0.01).min(1.12),
    }
}

/// Phase strategy coefficient for the Wiz Limit Break buff (5th Anniversary onward).
pub fn wiz_limit_break_phase_coefficient(style: Style, phase: Phase) -> f64 {
    match (style, phase) {
        (Style::Front | Style::Runaway, Phase::Early) => 0.26,
        (Style::Front | Style::Runaway, Phase::Mid) => 0.23,
        (Style::Front | Style::Runaway, Phase::Late) => 0.19,
        (Style::Front | Style::Runaway, Phase::Last) => 0.16,
        (Style::Pace, _) => 0.21,
        (Style::Late, Phase::Early) => 0.19,
        (Style::Late, Phase::Mid) => 0.18,
        (Style::Late, Phase::Late) => 0.23,
        (Style::Late, Phase::Last) => 0.24,
        (Style::End, Phase::Early) => 0.15,
        (Style::End, Phase::Mid) => 0.17,
        (Style::End, Phase::Late) => 0.25,
        (Style::End, Phase::Last) => 0.27,
    }
}

/// Distance factor for the Stamina Limit Break buff (2.5 Anniversary onward).
pub fn stamina_limit_break_distance_factor(distance: i32, game_version: GameVersion) -> f64 {
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

/// Stamina Limit Break target speed buff in m/s, without its random factor
/// (2.5 Anniversary onward).
///
/// Zero at 1200 stamina or below, or on courses up to 2100m.
pub fn stamina_limit_break_base_buff(
    stamina: u16,
    distance: i32,
    game_version: GameVersion,
) -> f64 {
    if game_version < GameVersion::TwoPointFiveAnniversary || stamina <= 1200 {
        return 0.0;
    }
    ((stamina - 1200) as f64).sqrt()
        * 0.0085
        * stamina_limit_break_distance_factor(distance, game_version)
}

/// Random factor of the Stamina Limit Break buff.
///
/// The game's parameter file lists a weighted random table and a parameter that
/// suggests power is involved, but observed values stay between 0.98 and
/// 1.02. The variation is small enough that other simulators and sources treat
/// the coefficient as always 1.0, so the same is done here. The RNG is still
/// taken so call sites stay the same if the roll is modeled in the future.
pub fn stamina_limit_break_random_factor<R: Rng + ?Sized>(_rng: &mut R) -> f64 {
    1.0
}

/// Calculate acceleration multiplier based on track aptitude.
pub fn ground_type_proficiency_multiplier(track_aptitude: Aptitude) -> f64 {
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

/// Calculate acceleration multiplier based on distance aptitude.
pub fn acceleration_distance_proficiency_multiplier(distance_aptitude: Aptitude) -> f64 {
    match distance_aptitude {
        Aptitude::S | Aptitude::A | Aptitude::B | Aptitude::C | Aptitude::D => 1.0,
        Aptitude::E => 0.6,
        Aptitude::F => 0.5,
        Aptitude::G => 0.4,
    }
}

/// Calculate strategy phase multiplier based on running style.
pub fn strategy_phase_multiplier(style: Style, phase: Phase) -> f64 {
    match (style, phase) {
        // Early-race
        (Style::Front, Phase::Early) => 1.0,
        (Style::Pace, Phase::Early) => 0.978,
        (Style::Late, Phase::Early) => 0.938,
        (Style::End, Phase::Early) => 0.931,
        (Style::Runaway, Phase::Early) => 1.063,
        // Mid-race
        (Style::Front, Phase::Mid) => 0.98,
        (Style::Pace, Phase::Mid) => 0.991,
        (Style::Late, Phase::Mid) => 0.998,
        (Style::End, Phase::Mid) => 1.0,
        (Style::Runaway, Phase::Mid) => 0.962,
        // Late-race and Last spurt
        (Style::Front, Phase::Late | Phase::Last) => 0.962,
        (Style::Pace, Phase::Late | Phase::Last) => 0.975,
        (Style::Late, Phase::Late | Phase::Last) => 0.994,
        (Style::End, Phase::Late | Phase::Last) => 1.0,
        (Style::Runaway, Phase::Late | Phase::Last) => 0.95,
    }
}

/// Calculate speed multiplier based on distance aptitude.
pub fn distance_proficiency_multiplier(distance_aptitude: Aptitude) -> f64 {
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

/// Calculate base target speed based on various factors.
pub fn base_target_speed(
    base_speed: f64,
    style: Style,
    phase: Phase,
    speed_stat: u16,
    distance_aptitude: Aptitude,
) -> f64 {
    let base = base_speed * strategy_phase_multiplier(style, phase);
    // Speed stat only affects target speed during late-race and last spurt
    match phase {
        Phase::Early | Phase::Mid => base,
        Phase::Late | Phase::Last => {
            let speed_modifier = (500.0 * speed_stat as f64).sqrt()
                * distance_proficiency_multiplier(distance_aptitude)
                * 0.002;
            base + speed_modifier
        }
    }
}

/// Roll section speed modifier range randomly based on wiz stat.
pub fn roll_section_speed_modifier<R: Rng + ?Sized>(
    rng: &mut R,
    wiz_stat: u16,
    base_speed: f64,
) -> f64 {
    let max = wiz_stat as f64 * (wiz_stat as f64 * 0.1).log10() / 550000.0;
    let min = max - 0.0065;
    rng.random_range(min..=max) * base_speed
}
