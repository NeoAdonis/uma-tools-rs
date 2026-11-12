use super::*;
use crate::test_utils::assert_close;

#[test]
fn test_formulas() {
    // (guts, target speed bonus, duration with A aptitude)
    let cases = [(400.0, 0.1516, 6.3498), (1200.0, 0.2930, 10.9982)];
    for (guts, bonus, secs) in cases {
        assert_close!(
            target_speed_bonus(guts),
            bonus,
            tolerance = 1e-4,
            "guts {guts}"
        );
        assert_close!(
            duration(guts, Aptitude::A),
            secs,
            tolerance = 1e-4,
            "guts {guts}"
        );
    }
}

#[test]
fn test_duration_scales_with_style_aptitude() {
    let base = duration(1000.0, Aptitude::A);
    let cases = [(Aptitude::S, 1.1), (Aptitude::B, 0.85), (Aptitude::G, 0.1)];
    for (aptitude, coef) in cases {
        let actual = duration(1000.0, aptitude);
        assert_close!(actual, base * coef, "{aptitude:?}");
    }
}

#[test]
fn test_trigger_proximity_in_course_widths() {
    assert!(is_close_to_leader(3.7, CourseWidths(0.164)));
    assert!(is_close_to_leader(0.0, CourseWidths(-0.164)));
    assert!(!is_close_to_leader(3.75, CourseWidths(0.0)));
    assert!(!is_close_to_leader(0.0, CourseWidths(0.166)));
}

#[test]
fn test_exit_proximity_in_course_widths() {
    // (distance, lane difference, whether too far).
    let cases = [
        (-5.0, CourseWidths(-0.415), false),
        (0.0, CourseWidths(0.415), false),
        (-5.01, CourseWidths(0.0), true),
        (0.0, CourseWidths(0.417), true),
        // Only when behind in distance, but on either side in lanes.
        (5.01, CourseWidths(0.0), false),
        (5.0, CourseWidths(4.69), true),
    ];
    for (distance, lane_diff, expected) in cases {
        let actual = is_too_far(distance, lane_diff);
        assert_eq!(actual, expected, "{distance}m, {lane_diff:?}");
    }
}

#[test]
fn test_hp_multiplier() {
    assert_eq!(hp_multiplier(EffectiveStyle::Runaway, false), 3.5);
    assert_eq!(hp_multiplier(EffectiveStyle::Runaway, true), 7.7);
    assert_eq!(hp_multiplier(EffectiveStyle::Front, false), 1.4);
    assert_eq!(hp_multiplier(EffectiveStyle::Front, true), 3.6);
}

#[test]
fn test_pending_has_no_effect() {
    let state = SpotStruggle::Pending;
    assert!(!state.has_struggled());
    assert!(!state.is_struggler());
    assert!(!state.is_active(0.0));
    assert_eq!(state.speed_bonus(0.0), 0.0);
}

#[test]
fn test_speed_bonus_expires() {
    let guts = 800.0;
    let state = SpotStruggle::start(guts, Aptitude::A, 10.0);
    let expires_at = 10.0 + duration(guts, Aptitude::A);
    assert!(state.has_struggled());

    assert!(state.is_active(10.0));
    assert_close!(state.speed_bonus(10.0), target_speed_bonus(guts));
    assert!(state.is_active(expires_at - 0.01));

    assert!(!state.is_active(expires_at));
    assert_eq!(state.speed_bonus(expires_at), 0.0);
    // Still counts as a struggler after expiration.
    assert!(state.is_struggler());
    assert!(state.has_struggled());
}

#[test]
fn test_stop() {
    let mut state = SpotStruggle::start(800.0, Aptitude::A, 10.0);
    state.stop();
    assert_eq!(state, SpotStruggle::Ended);
    assert!(state.has_struggled());
    assert!(!state.is_struggler());
    assert!(!state.is_active(10.0));
    assert_eq!(state.speed_bonus(10.0), 0.0);

    // Stopping a struggle that never started does nothing.
    let mut state = SpotStruggle::Pending;
    state.stop();
    assert_eq!(state, SpotStruggle::Pending);
}
