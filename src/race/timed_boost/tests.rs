use super::*;

#[test]
fn test_idle() {
    let idle = TimedBoost::Idle { next_check: 5.0 };
    assert_eq!(idle.speed_bonus(), 0.0);
    assert!(!idle.is_check_due(4.99));
    assert!(idle.is_check_due(5.0));
}

#[test]
fn test_start_and_expire() {
    let mut boost = TimedBoost::start(0.3, 10.0, 2.0, 1.0);
    assert_eq!(
        boost,
        TimedBoost::Active {
            speed_bonus: 0.3,
            expires_at: 12.0,
            next_check: 13.0,
        }
    );
    assert_eq!(boost.speed_bonus(), 0.3);
    // Never due while active, even past the next check.
    assert!(!boost.is_check_due(20.0));

    boost.expire(11.99);
    assert_eq!(boost.speed_bonus(), 0.3);
    boost.expire(12.0);
    assert_eq!(boost, TimedBoost::Idle { next_check: 13.0 });
}
