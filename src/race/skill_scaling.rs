//! Formulas that scale a skill effect's value or a skill's duration with the
//! runner's situation (see [`ValueUsage`](crate::skill::ValueUsage) and
//! [`TimeUsage`](crate::skill::TimeUsage)).

/// Multiplier for Unity Cup (アオハル杯) skills.
///
/// It depends on the team's total of the stat the skill is tied to (e.g. Speed
/// for [`ValueUsage::AoharuSpeed`](crate::skill::ValueUsage::AoharuSpeed)):
/// 0.8x below 1200, 0.9x below 1800, 1.0x below 2600, 1.1x below 3600, and 1.2x
/// from 3600. The maximum is assumed for now.
pub const AOHARU_MULTIPLIER: f64 = 1.2;

/// Multiplier for Climax skills.
///
/// It depends on the number of races won during training: 0.8x below 6,
/// 0.9x below 14, 1.0x below 18, 1.1x below 25, and 1.2x from 25. The maximum
/// is assumed for now.
pub const CLIMAX_MULTIPLIER: f64 = 1.2;

/// Multiplier for skills scaling with the total level of global potential in
/// the L'Arc scenario.
///
/// 1.0x below level 10, 1.1x below 20, and 1.2x from 20. The maximum is assumed
/// for now.
pub const L_ARC_MULTIPLIER: f64 = 1.2;

/// Multiplier for skills scaling with the number of U.A.F. tournament wins.
///
/// 1.0x below 10 wins, 1.1x below 15, and 1.2x from 15. The maximum is assumed
/// for now.
pub const UAF_WIN_MULTIPLIER: f64 = 1.2;

/// Multiplier for skills scaling with the cooking points earned in the Great
/// Food Festival (大豊食祭) scenario.
///
/// 1.0x below 10000 points, 1.1x below 16000, and 1.2x from 16000. The maximum
/// is assumed for now.
pub const COOKING_MULTIPLIER: f64 = 1.2;

/// Multiplier for skills scaling with the total research level earned in the
/// Mecha Umamusume scenario.
///
/// 1.0x below level 1000, 1.1x below 2000, and 1.2x from 2000. The maximum is
/// assumed for now.
pub const RESEARCH_MULTIPLIER: f64 = 1.2;

/// Multiplier for skills scaling with the total development points earned in
/// the Design Your Island scenario.
///
/// 1.0x below 2500 points, 1.1x below 3500, and 1.2x from 3500. The maximum is
/// assumed for now.
pub const DEVELOPMENT_MULTIPLIER: f64 = 1.2;

/// Multiplier for skills scaling with the love received from fans.
///
/// Love is gained during the race from the start dash, position changes in the
/// Mid race and skill activations, each gain scaling with the fan count during
/// training. The multiplier ranges from 1.0x to 3.0x. The maximum is assumed for
/// now.
pub const FAN_LOVE_MULTIPLIER: f64 = 3.0;

/// Multiplier from the number of skills the runner has, up to 1.2x at 20
/// skills.
pub fn skill_count_multiplier(skill_count: usize) -> f64 {
    (1.0 + 0.01 * skill_count as f64).min(1.2)
}

/// Random multiplier given a uniform `roll` in `[0, 1)`: 0x (60%), 0.02x (30%)
/// or 0.04x (10%).
///
/// Shared by both random variants for now (see
/// [`ValueUsage::MultiplyRandom1`](crate::skill::ValueUsage::MultiplyRandom1)).
pub fn random_multiplier(roll: f64) -> f64 {
    if roll < 0.6 {
        0.0
    } else if roll < 0.9 {
        0.02
    } else {
        0.04
    }
}

/// Multiplier from the highest of the runner's raw stats.
pub fn max_raw_stat_multiplier(stat: u16) -> f64 {
    match stat {
        ..600 => 0.8,
        600..800 => 0.9,
        800..1000 => 1.0,
        1000..1100 => 1.1,
        1100.. => 1.2,
    }
}

/// Multiplier from the number of green skills activated by the runner.
pub fn green_skill_count_multiplier(count: usize) -> f64 {
    match count {
        ..3 => 0.0,
        3..5 => 1.0,
        5 => 2.0,
        6.. => 3.0,
    }
}

/// Bonus added to the value when the runner is at least 20 m behind the first
/// place.
pub fn distance_from_first_bonus(distance: f64) -> f64 {
    if distance >= 20.0 { 0.1 } else { 0.0 }
}

/// Multiplier from the longest time (in seconds) the runner was side-blocked
/// during the Mid race.
///
/// Used for both values and durations.
pub fn mid_side_blocked_time_multiplier(time: f64) -> f64 {
    if time < 2.0 {
        1.0
    } else if time < 4.0 {
        2.0
    } else if time < 6.0 {
        3.0
    } else {
        4.0
    }
}

/// Multiplier from the runner's adjusted speed, first variant.
pub fn speed_multiplier_1(speed: u16) -> f64 {
    match speed {
        ..1700 => 0.0,
        1700..1800 => 1.0,
        1800..1900 => 2.0,
        1900..2000 => 3.0,
        2000.. => 4.0,
    }
}

/// Multiplier from the runner's adjusted speed, second variant.
pub fn speed_multiplier_2(speed: u16) -> f64 {
    match speed {
        ..1400 => 1.0,
        1400..1600 => 2.0,
        1600.. => 3.0,
    }
}

/// Multiplier from the number of overtakes in corners during the Late race or
/// later.
pub fn late_corner_overtake_multiplier(overtakes: u16) -> f64 {
    match overtakes {
        ..2 => 1.0,
        2 => 1.1,
        3 => 1.2,
        4.. => 1.25,
    }
}

/// Multiplier from the runner's fan count.
pub fn fan_count_multiplier(fan_count: u32) -> f64 {
    match fan_count {
        ..20000 => 0.8,
        20000..50000 => 0.9,
        50000..100000 => 1.0,
        100000..160000 => 1.1,
        160000.. => 1.2,
    }
}

/// Multiplier from the largest lead (in meters) the runner took before the Late
/// race.
pub fn lead_multiplier(lead: f64) -> f64 {
    if lead < 10.0 {
        1.0
    } else if lead < 25.0 {
        1.4
    } else {
        1.8
    }
}

/// Duration multiplier from the distance (in meters) behind the first place, up
/// to 1.6x.
pub fn distance_from_first_duration_multiplier(distance: f64) -> f64 {
    (0.8 + distance / 62.5).min(1.6)
}

/// Duration multiplier from the runner's remaining HP, first variant.
pub fn remaining_hp_duration_multiplier_1(hp: f64) -> f64 {
    if hp < 2000.0 {
        1.0
    } else if hp < 2400.0 {
        1.5
    } else if hp < 2600.0 {
        2.0
    } else if hp < 2800.0 {
        2.2
    } else if hp < 3000.0 {
        2.5
    } else if hp < 3200.0 {
        3.0
    } else if hp < 3500.0 {
        3.5
    } else {
        4.0
    }
}

/// Duration multiplier from the runner's remaining HP, second variant.
pub fn remaining_hp_duration_multiplier_2(hp: f64) -> f64 {
    if hp < 1500.0 {
        1.0
    } else if hp < 1800.0 {
        1.5
    } else if hp < 2000.0 {
        2.0
    } else if hp < 2100.0 {
        2.5
    } else {
        3.0
    }
}

/// Base duration in seconds added for each uphill climbed before the skill
/// activates, up to [`MAX_UPHILL_EXTENSIONS`] times.
pub const UPHILL_EXTENSION: f64 = 0.5;
/// Maximum number of uphills that extend a skill's base duration.
pub const MAX_UPHILL_EXTENSIONS: u8 = 4;

/// Base duration in seconds added for the number of uphills climbed.
pub fn uphill_duration_bonus(uphills: u8) -> f64 {
    UPHILL_EXTENSION * uphills.min(MAX_UPHILL_EXTENSIONS) as f64
}

/// Base duration in seconds added to a skill's effects for each overtake while
/// they're active. It scales with the course distance like base durations.
pub const OVERTAKE_EXTENSION: f64 = 1.0;
/// Maximum number of times a skill's effects are extended by overtaking.
pub const MAX_OVERTAKE_EXTENSIONS: u8 = 3;

#[cfg(test)]
mod tests;
