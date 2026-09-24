//! Tests for the uma module

use super::skill::{Skill, SkillData, SkillRarity};
use super::*;

#[test]
fn test_motivation_multiplier() {
    // Normal = 0; multiplier should be 1.0
    assert_eq!(motivation_multiplier(Motivation::Normal), 1.0);

    // Great = +2; multiplier should be 1.04
    assert!((motivation_multiplier(Motivation::Great) - 1.04).abs() < 0.001);

    // Awful = -2; multiplier should be 0.96
    assert!((motivation_multiplier(Motivation::Awful) - 0.96).abs() < 0.001);
}

fn skill_with_wisdom_check(wisdom_check: bool) -> Skill {
    Skill::new(
        "test".to_string(),
        SkillData {
            rarity: SkillRarity::White,
            alternatives: vec![],
            wisdom_check,
        },
    )
}

#[test]
fn test_activation_chance_without_wisdom_check() {
    let skill = skill_with_wisdom_check(false);
    assert_eq!(skill.activation_chance(1), 1.0);
    assert_eq!(skill.activation_chance(1200), 1.0);
}

#[test]
fn test_activation_chance_with_wisdom_check() {
    let skill = skill_with_wisdom_check(true);
    assert!((skill.activation_chance(900) - 0.9).abs() < 1e-9);
    // Low wisdom is clamped to the minimum chance
    assert_eq!(skill.activation_chance(100), 0.2);
    assert_eq!(skill.activation_chance(0), 0.2);
}

#[test]
fn test_wiz_limit_break_eligibility() {
    let skill = |rarity| {
        Skill::new(
            "test".to_string(),
            SkillData {
                rarity,
                alternatives: vec![],
                wisdom_check: true,
            },
        )
    };
    assert!(!skill(SkillRarity::White).is_wiz_limit_break_eligible());
    assert!(skill(SkillRarity::Gold).is_wiz_limit_break_eligible());
    assert!(skill(SkillRarity::Unique).is_wiz_limit_break_eligible());
    assert!(skill(SkillRarity::UniqueUpgraded).is_wiz_limit_break_eligible());
    assert!(skill(SkillRarity::UniqueNatural).is_wiz_limit_break_eligible());
    assert!(skill(SkillRarity::Evolution).is_wiz_limit_break_eligible());
}
