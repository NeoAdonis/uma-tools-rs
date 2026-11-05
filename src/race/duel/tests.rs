use super::*;
use crate::test_utils::assert_close;

#[test]
fn test_formulas() {
    // (guts, target speed bonus, acceleration bonus)
    let cases = [(400, 0.2961, 0.0685), (1200, 0.6445, 0.1310)];
    for (guts, speed, acceleration) in cases {
        assert_close!(
            target_speed_bonus(guts),
            speed,
            tolerance = 1e-4,
            "guts {guts}"
        );
        assert_close!(
            acceleration_bonus(guts),
            acceleration,
            tolerance = 1e-4,
            "guts {guts}"
        );
    }
}

#[test]
fn test_trigger_proximity_in_course_widths() {
    assert!(is_close(2.9, CourseWidths(0.249)));
    assert!(is_close(-2.9, CourseWidths(-0.249)));
    assert!(!is_close(3.0, CourseWidths(0.0)));
    assert!(!is_close(-3.0, CourseWidths(0.0)));
    assert!(!is_close(0.0, CourseWidths(0.251)));
    assert!(!is_close(0.0, CourseWidths(-0.251)));
}

#[test]
fn test_speed_difference_is_inclusive() {
    assert!(are_speeds_similar(0.6, 0.0));
    // Rounding errors still count as 0.6.
    assert!(are_speeds_similar(20.0, 19.4));
    assert!(are_speeds_similar(0.60000001, 0.0));
    // In either order.
    assert!(!are_speeds_similar(19.3, 20.0));
}

#[test]
fn test_separation_in_both_directions() {
    assert!(!is_separated(4.99));
    assert!(!is_separated(-4.99));
    assert!(is_separated(5.0));
    assert!(is_separated(-5.0));
}

#[test]
fn test_pending_has_no_effect() {
    let duel = Duel::Pending;
    assert!(!duel.is_dueling());
    assert!(!duel.has_dueled());
    assert!(!duel.has_ended());
    assert_eq!(duel.speed_bonus(), 0.0);
    assert_eq!(duel.acceleration_bonus(), 0.0);
}

#[test]
fn test_active_applies_bonuses() {
    let duel = Duel::start(1000);
    assert!(duel.is_dueling());
    assert!(duel.has_dueled());
    assert!(!duel.has_ended());
    assert_eq!(duel.speed_bonus(), target_speed_bonus(1000));
    assert_eq!(duel.acceleration_bonus(), acceleration_bonus(1000));
}

#[test]
fn test_stop_ends_only_active_duels() {
    let mut duel = Duel::Pending;
    duel.stop();
    assert_eq!(duel, Duel::Pending);

    let mut duel = Duel::start(1000);
    duel.stop();
    assert_eq!(duel, Duel::Ended);
    assert!(duel.has_dueled());
    assert!(duel.has_ended());
    assert_eq!(duel.speed_bonus(), 0.0);
    assert_eq!(duel.acceleration_bonus(), 0.0);
}
