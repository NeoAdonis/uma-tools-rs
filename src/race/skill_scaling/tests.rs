use super::*;
use crate::test_utils::assert_close;

#[test]
fn test_skill_count_multiplier() {
    assert_close!(skill_count_multiplier(10), 1.1);
    // Capped at 20 skills.
    assert_close!(skill_count_multiplier(20), 1.2);
    assert_close!(skill_count_multiplier(21), 1.2);
}

#[test]
fn test_random_multiplier() {
    let cases = [(0.59, 0.0), (0.6, 0.02), (0.89, 0.02), (0.9, 0.04)];
    for (roll, expected) in cases {
        assert_eq!(random_multiplier(roll), expected, "roll {roll}");
    }
}

#[test]
fn test_max_raw_stat_multiplier() {
    let cases = [
        (599, 0.8),
        (600, 0.9),
        (799, 0.9),
        (800, 1.0),
        (999, 1.0),
        (1000, 1.1),
        (1099, 1.1),
        (1100, 1.2),
    ];
    for (stat, expected) in cases {
        assert_eq!(max_raw_stat_multiplier(stat), expected, "stat {stat}");
    }
}

#[test]
fn test_green_skill_count_multiplier() {
    let cases = [(2, 0.0), (3, 1.0), (4, 1.0), (5, 2.0), (6, 3.0)];
    for (count, expected) in cases {
        assert_eq!(
            green_skill_count_multiplier(count),
            expected,
            "count {count}"
        );
    }
}

#[test]
fn test_medium_distance_bonus() {
    assert_eq!(medium_distance_bonus(DistanceType::Mile), 0.0);
    assert_eq!(medium_distance_bonus(DistanceType::Medium), 0.2);
}

#[test]
fn test_bashin_from_first_bonus() {
    assert_eq!(bashin_from_first_bonus(7.9), 0.0);
    assert_eq!(bashin_from_first_bonus(8.0), 0.2);
}

#[test]
fn test_distance_from_first_bonus() {
    assert_eq!(distance_from_first_bonus(19.9), 0.0);
    assert_eq!(distance_from_first_bonus(20.0), 0.1);
}

#[test]
fn test_mid_side_blocked_time_multiplier() {
    let cases = [
        (1.9, 1.0),
        (2.0, 2.0),
        (3.9, 2.0),
        (4.0, 3.0),
        (5.9, 3.0),
        (6.0, 4.0),
    ];
    for (time, expected) in cases {
        assert_eq!(
            mid_side_blocked_time_multiplier(time),
            expected,
            "time {time}"
        );
    }
}

#[test]
fn test_speed_multipliers() {
    let cases = [
        (1699, 0.0),
        (1700, 1.0),
        (1799, 1.0),
        (1800, 2.0),
        (1899, 2.0),
        (1900, 3.0),
        (1999, 3.0),
        (2000, 4.0),
    ];
    for (speed, expected) in cases {
        assert_eq!(speed_multiplier_1(speed), expected, "speed {speed}");
    }

    let cases = [(1399, 1.0), (1400, 2.0), (1599, 2.0), (1600, 3.0)];
    for (speed, expected) in cases {
        assert_eq!(speed_multiplier_2(speed), expected, "speed {speed}");
    }
}

#[test]
fn test_lead_multiplier() {
    let cases = [(9.9, 1.0), (10.0, 1.4), (24.9, 1.4), (25.0, 1.8)];
    for (lead, expected) in cases {
        assert_eq!(lead_multiplier(lead), expected, "lead {lead}");
    }
}

#[test]
fn test_late_corner_overtake_multiplier() {
    let cases = [(1, 1.0), (2, 1.1), (3, 1.2), (4, 1.25)];
    for (overtakes, expected) in cases {
        assert_eq!(
            late_corner_overtake_multiplier(overtakes),
            expected,
            "overtakes {overtakes}"
        );
    }
}

#[test]
fn test_fan_count_multiplier() {
    let cases = [
        (19999, 0.8),
        (20000, 0.9),
        (49999, 0.9),
        (50000, 1.0),
        (99999, 1.0),
        (100000, 1.1),
        (159999, 1.1),
        (160000, 1.2),
    ];
    for (fan_count, expected) in cases {
        assert_eq!(
            fan_count_multiplier(fan_count),
            expected,
            "fans {fan_count}"
        );
    }
}

#[test]
fn test_distance_from_first_duration_multiplier() {
    assert_close!(distance_from_first_duration_multiplier(25.0), 1.2);
    // Capped at 50m.
    assert_close!(distance_from_first_duration_multiplier(50.0), 1.6);
    assert_close!(distance_from_first_duration_multiplier(100.0), 1.6);
}

#[test]
fn test_remaining_hp_duration_multipliers() {
    let cases = [
        (1999.0, 1.0),
        (2000.0, 1.5),
        (2400.0, 2.0),
        (2600.0, 2.2),
        (2800.0, 2.5),
        (3000.0, 3.0),
        (3200.0, 3.5),
        (3499.0, 3.5),
        (3500.0, 4.0),
    ];
    for (hp, expected) in cases {
        assert_eq!(remaining_hp_duration_multiplier_1(hp), expected, "HP {hp}");
    }

    let cases = [
        (1499.0, 1.0),
        (1500.0, 1.5),
        (1800.0, 2.0),
        (2000.0, 2.5),
        (2099.0, 2.5),
        (2100.0, 3.0),
    ];
    for (hp, expected) in cases {
        assert_eq!(remaining_hp_duration_multiplier_2(hp), expected, "HP {hp}");
    }
}

#[test]
fn test_uphill_duration_bonus() {
    let cases = [(1, 0.5), (4, 2.0), (5, 2.0)];
    for (uphills, expected) in cases {
        assert_eq!(
            uphill_duration_bonus(uphills),
            expected,
            "uphills {uphills}"
        );
    }
}

#[test]
fn test_level_multiplier() {
    use SkillType::*;
    let cases = [
        (SpeedUp, 1, 1.0),
        (SpeedUp, 10, 1.1),
        (TargetSpeed, 2, 1.01),
        (TargetSpeed, 10, 1.25),
        (Acceleration, 7, 1.125),
        // Clamped to the game's levels.
        (TargetSpeed, 0, 1.0),
        (TargetSpeed, 11, 1.25),
        // Missing from the game's table.
        (ActivateRandomGold, 10, 1.0),
    ];
    for (skill_type, level, expected) in cases {
        assert_eq!(
            level_multiplier(skill_type, level),
            expected,
            "{skill_type:?} level {level}"
        );
    }
}

#[test]
fn test_level_scaled_modifier() {
    use crate::test_utils::effect;
    assert_close!(
        level_scaled_modifier(&effect(SkillType::TargetSpeed, 0.35), 10),
        0.4375
    );
    assert_close!(
        level_scaled_modifier(&effect(SkillType::AddStartDelay, -0.1), 10),
        -0.118
    );
    // Negative effects don't scale.
    for (skill_type, modifier) in [
        (SkillType::CurrentSpeed, -0.25),
        (SkillType::Recovery, -0.04),
        (SkillType::MultiplyStartDelay, 1.5),
    ] {
        let modifier_at_10 = level_scaled_modifier(&effect(skill_type, modifier), 10);
        assert_eq!(modifier_at_10, modifier, "{skill_type:?}");
    }
}

#[test]
fn test_level_multipliers_are_1x_at_level_1() {
    // Skills loaded from the skill data (level 1) must keep their modifiers.
    for id in 0..=u8::MAX {
        let Ok(skill_type) = SkillType::try_from(id) else {
            continue;
        };
        assert_eq!(level_multiplier(skill_type, 1), 1.0, "{skill_type:?}");
    }
}
