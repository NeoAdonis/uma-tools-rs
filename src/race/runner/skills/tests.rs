use super::*;
use crate::race::modifiers::{wit_limit_break_multiplier, wit_limit_break_phase_coefficient};
use crate::race::runner::Runner;
use crate::race::solver::RaceSolver;
use crate::skill::{Skill, SkillRarity};
use crate::test_utils::{
    alternative, assert_close, effect, ready_solver, seeded_config, synthetic_skill, uma,
};
use crate::uma::{Style, Uma};
use std::sync::Arc;

fn timed(skill_type: SkillType, value: f64) -> ActiveTimedEffect {
    ActiveTimedEffect {
        effect: effect(skill_type, value),
        value,
        expires_at: 10.0,
        overtake_extension: OvertakeExtension::default(),
        additional_triggers: 0,
        source: None,
    }
}

#[test]
fn test_skill_modifiers_from_effects() {
    let mods = SkillModifiers::from_effects(&[]);
    assert_eq!(mods.target_speed, 0.0);
    assert_eq!(mods.target_lane, None);
    assert_eq!(mods.duration_scaling, 1.0);

    let effects = [
        timed(SkillType::TargetSpeed, 0.15),
        timed(SkillType::TargetSpeed, 0.25),
        timed(SkillType::CurrentSpeed, 0.1),
        timed(SkillType::CurrentSpeedWithNaturalDeceleration, 0.05),
        timed(SkillType::Acceleration, 0.2),
        timed(SkillType::AccelerationAllOutSpurt, 0.3),
        timed(SkillType::LaneMovementSpeed, 0.02),
        timed(SkillType::Fov, -5.0),
        timed(SkillType::LaneChange, 0.5),
        timed(SkillType::ExtendEvolvedDuration, 1.5),
        timed(SkillType::ExtendEvolvedDuration, 2.0),
        // Not a modifier.
        timed(SkillType::Recovery, 0.05),
    ];
    let mods = SkillModifiers::from_effects(&effects);
    assert_close!(mods.target_speed, 0.4);
    assert_close!(mods.current_speed, 0.15);
    assert_close!(mods.acceleration, 0.2);
    assert_close!(mods.all_out_spurt_acceleration, 0.3);
    assert_close!(mods.lane_speed, 0.02);
    assert_close!(mods.visible_distance, -5.0);
    assert_eq!(mods.target_lane, Some(CourseWidths(0.5)));
    // Multiplicative.
    assert_close!(mods.duration_scaling, 3.0);
}

/// Ready race with a single runner with high wit, whose only skill has an
/// effect of `skill_type` worth 0.35.
fn high_wit_race(rarity: SkillRarity, skill_type: SkillType) -> RaceSolver<Runner> {
    let skill = Arc::new(Skill {
        rarity,
        ..synthetic_skill(vec![alternative("phase==3", skill_type, 0.35)])
    });
    let uma = Uma {
        wit: 1500,
        ..uma(Style::Pace)
    };
    ready_solver(10101, uma, vec![skill], seeded_config(42))
}

#[test]
fn test_wit_limit_break_bonus_by_skill() {
    use SkillRarity::*;
    use SkillType::*;

    // (rarity, effect type, whether the bonus applies).
    let cases = [
        (Gold, TargetSpeed, true),
        (UniqueNatural, CurrentSpeedWithNaturalDeceleration, true),
        (White, TargetSpeed, false),
        (Gold, CurrentSpeed, false),
    ];
    for (rarity, skill_type, boosted) in cases {
        let mut solver = high_wit_race(rarity, skill_type);
        let runner = &mut solver.runners[0];
        let bonus = wit_limit_break_phase_coefficient(runner.adjusted.style, runner.phase)
            * wit_limit_break_multiplier(runner.adjusted.wit);
        assert!(bonus > 0.0);
        runner.activate_skill(0, 0, &solver.state);

        let expected = if boosted { 0.35 * (1.0 + bonus) } else { 0.35 };
        let value = runner.active_timed_effects[0].value;
        assert_close!(value, expected, "{rarity:?} {skill_type:?}");
    }
}

#[test]
fn test_current_speed_on_expiry() {
    // Only the modifier with natural deceleration is transferred into the speed.
    let cases = [
        (SkillType::CurrentSpeedWithNaturalDeceleration, 20.35),
        (SkillType::CurrentSpeed, 20.0),
    ];
    for (skill_type, expected) in cases {
        let mut solver = high_wit_race(SkillRarity::White, skill_type);
        let runner = &mut solver.runners[0];
        runner.speed = 20.0;
        runner.activate_skill(0, 0, &solver.state);
        let expires_at = runner.active_timed_effects[0].expires_at;

        // While active, only the modifier applies.
        runner.update_skill_durations(expires_at - 0.1);
        assert_close!(runner.speed, 20.0, "{skill_type:?}");
        assert_close!(runner.skill_modifiers.current_speed, 0.35, "{skill_type:?}");

        runner.update_skill_durations(expires_at);
        assert_close!(runner.speed, expected, "{skill_type:?}");
        assert_close!(runner.skill_modifiers.current_speed, 0.0, "{skill_type:?}");
        assert!(runner.active_timed_effects.is_empty(), "{skill_type:?}");
    }
}
