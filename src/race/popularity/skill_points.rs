//! Heuristic for the popularity points a skill adds to each stat.
//!
//! Used when the in-game values are missing from the skill data (outdated
//! skill data), estimating them from the skill rarity and effects.
//!
//! TODO: Remove this module once outdated skill data is no longer supported.

use super::SkillPopularity;
use crate::skill::{
    PopularityStat as Stat, Skill, SkillEffect, SkillRarity, SkillTarget, SkillType,
};

/// Get the stat a skill effect on oneself is associated with, and its strength
/// relative to a typical gold skill effect of the same type.
///
/// Stat-ups are compared against a +40 bonus. Effects with no meaningful
/// modifier get a fixed strength of 0.5.
fn own_effect_stat(effect: &SkillEffect) -> Option<(Stat, f64)> {
    use SkillType::*;
    let m = effect.modifier as f64;
    let (stat, strength) = match effect.skill_type {
        TargetSpeed => (Stat::Speed, m / 3500.0),
        CurrentSpeed => (Stat::Speed, m / 1500.0),
        SpeedUp => (Stat::Speed, m / 400000.0),
        MultiplyStartDelay | SetStartDelay => (Stat::Speed, 0.5),
        Acceleration | AccelerationFullThrottle => (Stat::Power, m / 4000.0),
        CurrentSpeedWithNaturalDeceleration => (Stat::Power, m / 3500.0),
        LaneMovementSpeed => (Stat::Power, m / 350.0),
        Fov => (Stat::Power, m / 100000.0),
        PowerUp => (Stat::Power, m / 400000.0),
        LaneChange => (Stat::Power, 0.5),
        Recovery => (Stat::Stamina, m / 550.0),
        StaminaUp => (Stat::Stamina, m / 400000.0),
        GutsUp => (Stat::Guts, m / 400000.0),
        WizUp => (Stat::Wiz, m / 400000.0),
        // Associated with speed for negative skills; handled separately
        // otherwise, as it affects several stats.
        AllStatsUp => (Stat::Speed, m / 400000.0),
        _ => return None,
    };
    Some((stat, strength.abs()))
}

/// Get the stat a debuff on other runners is associated with.
fn debuffed_stat(skill_type: SkillType) -> Option<Stat> {
    use SkillType::*;
    match skill_type {
        TargetSpeed | CurrentSpeed | CurrentSpeedWithNaturalDeceleration => Some(Stat::Speed),
        Recovery | RushTimeUp => Some(Stat::Stamina),
        Acceleration | Fov => Some(Stat::Power),
        _ => None,
    }
}

/// Get the modifier of a standard (○) white skill effect of the given type.
fn standard_white_modifier(skill_type: SkillType) -> Option<i32> {
    use SkillType::*;
    match skill_type {
        TargetSpeed | CurrentSpeed | CurrentSpeedWithNaturalDeceleration => Some(1500),
        Acceleration | AccelerationFullThrottle => Some(2000),
        Recovery => Some(150),
        LaneMovementSpeed => Some(250),
        Fov => Some(50000),
        _ => None,
    }
}

/// Get the total popularity points given by a skill with positive effects.
fn tier_points(skill: &Skill) -> f64 {
    match skill.rarity {
        SkillRarity::Unique => 30.0,
        // Inherited uniques are stored as white skills with an ID starting with 9.
        SkillRarity::White if skill.id.starts_with('9') => 15.0,
        SkillRarity::White => {
            let own_effects: Vec<_> = skill
                .alternatives
                .iter()
                .flat_map(|alternative| &alternative.effects)
                .filter(|effect| effect.target == SkillTarget::Oneself)
                .collect();
            if !own_effects.is_empty() && own_effects.iter().all(|e| e.skill_type.is_stat_boost()) {
                // Stat-ups give 0.75 points per stat point (rounded to 5), with a
                // minimum of 20.
                let bonus = own_effects.iter().map(|e| e.modifier).max().unwrap_or(0);
                ((bonus as f64 * 0.75 / 50000.0).round() * 5.0).max(20.0)
            } else if !skill.is_nerf()
                && own_effects
                    .iter()
                    .any(|e| standard_white_modifier(e.skill_type).is_some_and(|m| e.modifier > m))
            {
                // Stronger (◎) white skills have an effect stronger than the
                // standard (○) one, with no drawbacks (e.g. using stamina).
                30.0
            } else {
                20.0
            }
        }
        _ => 60.0,
    }
}

/// Estimate the popularity points a skill adds to each stat.
///
/// Points go to the stat of the strongest effect on oneself, and are split
/// evenly with a second stat when its strongest effect is at least half as
/// strong. Drawbacks (e.g. using stamina) are ignored. Skills with no positive
/// effect on oneself split their points between the stat affected by their
/// debuff on others and wisdom, or take 30 points from the stat they lower.
pub(super) fn estimate_popularity(skill: &Skill) -> SkillPopularity {
    let effects = || {
        skill
            .alternatives
            .iter()
            .flat_map(|alternative| &alternative.effects)
    };
    let popularity = SkillPopularity::default();

    // Strongest effect on oneself for each stat, in order of appearance.
    let mut ranked: Vec<(Stat, f64)> = Vec::new();
    for effect in effects() {
        if effect.target != SkillTarget::Oneself || !Skill::is_effect_positive(effect) {
            continue;
        }
        let Some((stat, strength)) = own_effect_stat(effect).filter(|&(_, s)| s > 0.0) else {
            continue;
        };
        let stats: &[Stat] = if effect.skill_type == SkillType::AllStatsUp {
            &[Stat::Speed, Stat::Stamina, Stat::Power]
        } else {
            &[stat]
        };
        for &stat in stats {
            match ranked.iter_mut().find(|(s, _)| *s == stat) {
                Some((_, s)) => *s = s.max(strength),
                None => ranked.push((stat, strength)),
            }
        }
    }
    // Stable sort, so ties keep the order of appearance.
    ranked.sort_by(|a, b| b.1.total_cmp(&a.1));

    match ranked.as_slice() {
        [] => {}
        [(first, _)] => return popularity.with(*first, tier_points(skill)),
        [(first, top), (second, next), ..] => {
            let points = tier_points(skill);
            if *next >= 0.5 * top {
                let half = (points / 2.0).floor();
                return popularity.with(*first, half).with(*second, half);
            }
            return popularity.with(*first, points);
        }
    }

    // Debuffs on others.
    if let Some(stat) = effects()
        .filter(|effect| {
            effect.target != SkillTarget::Oneself && !Skill::is_effect_positive(effect)
        })
        .find_map(|effect| debuffed_stat(effect.skill_type))
    {
        let half = (tier_points(skill) / 2.0).floor();
        return popularity.with(stat, half).with(Stat::Wiz, half);
    }

    // Negative skills.
    if let Some((stat, _)) = effects()
        .filter(|effect| effect.target == SkillTarget::Oneself)
        .find_map(own_effect_stat)
    {
        return popularity.with(stat, -30.0);
    }

    popularity
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::race::popularity::skill_popularity;
    use crate::race::popularity::tests::{points, skill};

    #[test]
    fn test_estimate_popularity_tiers() {
        use SkillRarity::*;
        use SkillType::*;

        let cases = [
            ("1", Gold, TargetSpeed, 3500, Stat::Speed, 60.0),
            ("1", Evolution, Acceleration, 4000, Stat::Power, 60.0),
            ("1", UniqueNatural, TargetSpeed, 3500, Stat::Speed, 60.0),
            ("1", Unique, TargetSpeed, 3500, Stat::Speed, 30.0),
            // Moving forward is associated with power, not speed.
            (
                "1",
                Gold,
                CurrentSpeedWithNaturalDeceleration,
                3500,
                Stat::Power,
                60.0,
            ),
            // Inherited unique.
            ("900011", White, TargetSpeed, 1500, Stat::Speed, 15.0),
            // Stronger (◎) and standard (○) white skills.
            ("200961", White, TargetSpeed, 2500, Stat::Speed, 30.0),
            ("200962", White, TargetSpeed, 1500, Stat::Speed, 20.0),
            ("200352", White, Recovery, 150, Stat::Stamina, 20.0),
            // Stat-ups.
            ("200012", White, SpeedUp, 400000, Stat::Speed, 30.0),
            ("200031", White, StaminaUp, 600000, Stat::Stamina, 45.0),
        ];
        for (id, rarity, skill_type, modifier, stat, expected) in cases {
            let skill = skill(id, rarity, &[(skill_type, SkillTarget::Oneself, modifier)]);
            assert_eq!(
                estimate_popularity(&skill),
                points(&[(stat, expected)]),
                "{id}"
            );
        }
    }

    #[test]
    fn test_estimate_popularity_split() {
        use SkillTarget::Oneself;
        use SkillType::*;

        let balanced = skill(
            "1",
            SkillRarity::Gold,
            &[(TargetSpeed, Oneself, 3500), (Acceleration, Oneself, 4000)],
        );
        assert_eq!(
            estimate_popularity(&balanced),
            points(&[(Stat::Speed, 30.0), (Stat::Power, 30.0)]),
        );

        // A secondary effect much weaker than the main one is ignored, as well as
        // drawbacks.
        let unbalanced = skill(
            "1",
            SkillRarity::Gold,
            &[
                (TargetSpeed, Oneself, 3500),
                (Acceleration, Oneself, 1000),
                (Recovery, Oneself, -200),
            ],
        );
        assert_eq!(
            estimate_popularity(&unbalanced),
            points(&[(Stat::Speed, 60.0)])
        );

        // Drawbacks also prevent a white skill from being in the stronger tier.
        let costly = skill(
            "202152",
            SkillRarity::White,
            &[(TargetSpeed, Oneself, 2500), (Recovery, Oneself, -200)],
        );
        assert_eq!(estimate_popularity(&costly), points(&[(Stat::Speed, 20.0)]));

        let inherited = skill(
            "900011",
            SkillRarity::White,
            &[(TargetSpeed, Oneself, 1500), (Acceleration, Oneself, 2000)],
        );
        assert_eq!(
            estimate_popularity(&inherited),
            points(&[(Stat::Speed, 7.0), (Stat::Power, 7.0)]),
        );
    }

    #[test]
    fn test_estimate_popularity_debuff_and_negative() {
        use SkillTarget::*;
        use SkillType::*;

        let debuff = skill(
            "200851",
            SkillRarity::White,
            &[(CurrentSpeed, BehindSelf, -1500)],
        );
        assert_eq!(
            estimate_popularity(&debuff),
            points(&[(Stat::Speed, 10.0), (Stat::Wiz, 10.0)]),
        );

        let negative = skill("200013", SkillRarity::White, &[(SpeedUp, Oneself, -400000)]);
        assert_eq!(
            estimate_popularity(&negative),
            points(&[(Stat::Speed, -30.0)])
        );

        // A bad start is associated with speed.
        let late_start = skill(
            "200433",
            SkillRarity::White,
            &[(MultiplyStartDelay, Oneself, 15000)],
        );
        assert_eq!(
            estimate_popularity(&late_start),
            points(&[(Stat::Speed, -30.0)])
        );
    }

    #[test]
    fn test_estimate_popularity_matches_data() {
        let skills = Skill::load_skills_from_file("data/skill_data.json")
            .expect("Failed to load skill data");

        let ids = [
            "200961", "200962", "200011", "200342", "200433", "200851", "202521", "10071",
            "900011", "900091",
        ];
        for id in ids {
            assert_eq!(
                estimate_popularity(&skills[id]),
                skill_popularity(&skills[id]),
                "skill {id}"
            );
        }
    }
}
