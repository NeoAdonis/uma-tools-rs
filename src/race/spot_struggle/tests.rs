use super::*;

const EPSILON: f64 = 1e-9;

#[test]
fn test_formulas() {
    // (guts, target speed bonus, duration with A aptitude)
    let cases = [
        (400, 0.1516, 6.3498),
        (1000, 0.2627, 10.0399),
        (1200, 0.2930, 10.9982),
    ];
    for (guts, bonus, secs) in cases {
        assert!(
            (target_speed_bonus(guts) - bonus).abs() < 1e-4,
            "guts {guts}"
        );
        assert!(
            (duration(guts, Aptitude::A) - secs).abs() < 1e-4,
            "guts {guts}"
        );
    }
}

#[test]
fn test_duration_scales_with_style_aptitude() {
    let base = duration(1000, Aptitude::A);
    let cases = [
        (Aptitude::S, 1.1),
        (Aptitude::A, 1.0),
        (Aptitude::B, 0.85),
        (Aptitude::C, 0.75),
        (Aptitude::D, 0.6),
        (Aptitude::E, 0.4),
        (Aptitude::F, 0.2),
        (Aptitude::G, 0.1),
    ];
    for (aptitude, coef) in cases {
        let actual = duration(1000, aptitude);
        assert!((actual - base * coef).abs() < EPSILON, "{aptitude:?}");
    }
}

#[test]
fn test_trigger_proximity_in_course_widths() {
    // 0.165 course widths = 1.85625 m.
    assert!(is_close_to_leader(3.7, 1.85));
    assert!(is_close_to_leader(0.0, -1.85));
    assert!(!is_close_to_leader(3.75, 0.0));
    assert!(!is_close_to_leader(0.0, 1.86));
}

#[test]
fn test_exit_proximity_in_course_widths() {
    // 0.416 course widths = 4.68 m.
    assert!(!is_too_far(-5.0, -4.67));
    assert!(!is_too_far(0.0, 4.67));
    assert!(is_too_far(-5.01, 0.0));
    assert!(is_too_far(0.0, 4.69));
    assert!(is_too_far(0.0, -4.69));
    assert!(is_too_far(5.0, 4.69));
}

#[test]
fn test_exit_only_when_behind() {
    assert!(is_too_far(-5.01, 0.0));
    assert!(!is_too_far(5.01, 0.0));
    assert!(!is_too_far(100.0, 0.0));
}

#[test]
fn test_hp_multiplier() {
    assert_eq!(hp_multiplier(Style::Runaway, false), 3.5);
    assert_eq!(hp_multiplier(Style::Runaway, true), 7.7);
    assert_eq!(hp_multiplier(Style::Front, false), 1.4);
    assert_eq!(hp_multiplier(Style::Front, true), 3.6);
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
    let guts = 800;
    let state = SpotStruggle::start(guts, Aptitude::A, 10.0);
    let expires_at = 10.0 + duration(guts, Aptitude::A);
    assert!(state.has_struggled());

    assert!(state.is_active(10.0));
    assert!((state.speed_bonus(10.0) - target_speed_bonus(guts)).abs() < EPSILON);
    assert!(state.is_active(expires_at - 0.01));

    assert!(!state.is_active(expires_at));
    assert_eq!(state.speed_bonus(expires_at), 0.0);
    // Still counts as a struggler after expiration.
    assert!(state.is_struggler());
    assert!(state.has_struggled());
}

#[test]
fn test_stop() {
    let mut state = SpotStruggle::start(800, Aptitude::A, 10.0);
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
