//! Integration tests for race simulation
//!
//! These tests verify the public API of the race simulation system.

use uma_tools::race::parameters::*;
use uma_tools::uma::*;

#[test]
fn test_basic_race_simulation() {
    // Create a basic Uma
    let _uma = Uma {
        speed: 1200,
        stamina: 800,
        power: 1000,
        guts: 600,
        wiz: 1000,
        style: Style::End,
        distance_aptitude: Aptitude::A,
        track_aptitude: Aptitude::A,
        style_aptitude: Aptitude::A,
    };

    // Create race parameters
    let _params = RaceParameters {
        track_condition: TrackCondition::Good,
        weather: Weather::Sunny,
        season: Season::Spring,
        time: Time::Noon,
        grade: Grade::G1,
        gates: 18,
        rng_seed: 12345,
    };

    // Create a test course (this would normally be loaded from JSON)
    // For now, we'll skip the full course loading and just verify the API works

    // This test verifies that the public API is accessible and compiles correctly
}

#[test]
fn test_uma_stat_calculations() {
    let raw_uma = Uma {
        speed: 1500,
        stamina: 800,
        power: 1200,
        guts: 700,
        wiz: 1200,
        style: Style::End,
        distance_aptitude: Aptitude::S,
        track_aptitude: Aptitude::A,
        style_aptitude: Aptitude::A,
    };

    // Test motivation effects
    let base_uma = Uma::raw_to_base(raw_uma, Motivation::Normal);

    // Normal motivation should not change stats significantly
    // (motivation multiplier is 1.0 for Normal)
    assert!(base_uma.speed > 0);
    assert!(base_uma.stamina > 0);

    // Test Great motivation (positive boost)
    let great_uma = Uma::raw_to_base(raw_uma, Motivation::Great);
    assert!(great_uma.speed > base_uma.speed);

    // Test Awful motivation (negative impact)
    let awful_uma = Uma::raw_to_base(raw_uma, Motivation::Awful);
    assert!(awful_uma.speed < base_uma.speed);
}

#[test]
fn test_aptitude_multipliers() {
    // Test that aptitude multipliers are ordered correctly
    let s_coef = wiz_style_multiplier(Aptitude::S);
    let a_coef = wiz_style_multiplier(Aptitude::A);
    let b_coef = wiz_style_multiplier(Aptitude::B);
    let c_coef = wiz_style_multiplier(Aptitude::C);

    assert!(s_coef > a_coef);
    assert!(a_coef > b_coef);
    assert!(b_coef > c_coef);
}

#[test]
fn test_track_condition_modifiers() {
    // Test that track conditions apply appropriate modifiers
    let heavy_modifier = track_speed_modifier(TrackCondition::Heavy);
    let firm_modifier = track_speed_modifier(TrackCondition::Firm);

    // Heavy track should have more penalty than firm
    assert!(heavy_modifier > firm_modifier);
}
