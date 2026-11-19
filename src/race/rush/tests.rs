use rand_pcg::Pcg64Mcg;

use super::*;

fn rng(seed: u64) -> Pcg64Mcg {
    Pcg64Mcg::seed_from_u64(seed)
}

/// A runner who started rushing at 5 seconds.
fn rushed() -> RushState {
    let mut rush_state = RushState::roll(EffectiveStyle::Pace, &mut rng(0));
    rush_state.start(5.0);
    rush_state
}

#[test]
fn test_chance() {
    // (base wit, modifier, chance): 19.00% and 9.74% before rounding up.
    let cases = [
        (300.0, 0.0, 19.0),
        (1200.0, 0.0, 10.0),
        (1200.0, -3.0, 7.0),
        // Clamped.
        (1.0, 0.0, 100.0),
        (1200.0, -20.0, 0.0),
    ];
    for (wit, modifier, expected) in cases {
        assert_eq!(chance(wit, modifier), expected, "{wit} wit, {modifier}");
    }
}

#[test]
fn test_roll() {
    use EffectiveStyle::*;
    // (style, shares of each rushed style: Runaway, Front, Pace, Late).
    let cases = [
        (Runaway, [1.0, 0.0, 0.0, 0.0]),
        (Front, [0.0, 1.0, 0.0, 0.0]),
        (Pace, [0.0, 1.0, 0.0, 0.0]),
        (Late, [0.0, 0.75, 0.25, 0.0]),
        (End, [0.0, 0.7, 0.2, 0.1]),
    ];
    const ROLLS: u64 = 2000;
    for (style, expected) in cases {
        let mut counts = [0; 4];
        for seed in 0..ROLLS {
            let rush_state = RushState::roll(style, &mut rng(seed));
            assert!(SECTIONS.contains(&rush_state.section), "{rush_state:?}");
            assert!(!rush_state.is_rushed && !rush_state.has_rushed());
            assert_eq!(rush_state.max_duration, MAX_DURATION);
            let index = [Runaway, Front, Pace, Late]
                .iter()
                .position(|&s| s == rush_state.style)
                .unwrap();
            counts[index] += 1;
        }
        let shares = counts.map(|count| count as f64 / ROLLS as f64);
        for (actual, expected) in shares.into_iter().zip(expected) {
            assert!((actual - expected).abs() < 0.04, "{style:?}: {shares:?}");
        }
    }
}

#[test]
fn test_start_and_end() {
    let mut rush_state = rushed();
    assert!(rush_state.is_rushed && rush_state.has_rushed());
    assert_eq!(rush_state.end_time, 5.0 + CHECK_INTERVAL);

    rush_state.end(9.0);
    assert!(!rush_state.is_rushed && rush_state.has_rushed());
    assert_eq!(rush_state.end_time, 9.0);
}

#[test]
fn test_roll_checks_to_fail() {
    // Each check fails with probability 0.45, so the count is geometric.
    const ROLLS: u64 = 4000;
    let mut counts = [0; 4];
    for seed in 0..ROLLS {
        let rush_state = RushState::roll(EffectiveStyle::Pace, &mut rng(seed));
        assert_eq!(rush_state.failed_checks, 0);
        counts[(rush_state.checks_to_fail as usize).min(3)] += 1;
    }
    let fail = 1.0 - SNAP_OUT_CHANCE;
    let expected = [
        1.0 - fail,
        fail * (1.0 - fail),
        fail * fail * (1.0 - fail),
        fail * fail * fail,
    ];
    for (count, expected) in counts.into_iter().zip(expected) {
        let share = count as f64 / ROLLS as f64;
        assert!((share - expected).abs() < 0.04, "{counts:?}");
    }
}

#[test]
fn test_should_end_waits_for_check() {
    let mut rush_state = rushed();
    let end_time = rush_state.end_time;
    assert!(!rush_state.should_end(end_time - 0.1));
    assert_eq!(rush_state.end_time, end_time);
}

#[test]
fn test_should_end_fails_the_settled_checks() {
    for checks_to_fail in 0..3 {
        let mut rush_state = rushed();
        rush_state.checks_to_fail = checks_to_fail;
        let mut check_time = rush_state.end_time;
        while !rush_state.should_end(check_time) {
            check_time = rush_state.end_time;
        }
        assert_eq!(rush_state.failed_checks, checks_to_fail);
        let expected = 5.0 + CHECK_INTERVAL * (checks_to_fail + 1) as f64;
        assert_eq!(check_time, expected);
    }
}

#[test]
fn test_should_end_after_max_duration() {
    let mut rush_state = rushed();
    rush_state.checks_to_fail = 10;
    rush_state.max_duration += 2.0;
    let max_end = rush_state.start_time + rush_state.max_duration;
    // Before the next check, so only the maximum duration can end it.
    rush_state.end_time = f64::MAX;
    assert!(!rush_state.should_end(max_end - 0.1));
    assert!(rush_state.should_end(max_end));
    // At a check, with failures left to go.
    rush_state.end_time = 0.0;
    assert!(rush_state.should_end(max_end));
}
