use rand_pcg::Pcg64Mcg;

use super::*;

fn rng(seed: u64) -> Pcg64Mcg {
    Pcg64Mcg::seed_from_u64(seed)
}

/// A runner who started rushing at 5 seconds.
fn rushed() -> RushState {
    let mut rush_state = RushState::roll(Style::Pace, &mut rng(0));
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
    use Style::*;
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
fn test_should_end_waits_for_check() {
    let mut rush_state = rushed();
    let end_time = rush_state.end_time;
    assert!(!rush_state.should_end(end_time - 0.1, &mut rng(0)));
    assert_eq!(rush_state.end_time, end_time);
}

#[test]
fn test_should_end_extends_on_failed_check() {
    let (mut snapped, mut extended) = (0, 0);
    for seed in 0..100 {
        let mut rush_state = rushed();
        let check_time = rush_state.end_time;
        if rush_state.should_end(check_time, &mut rng(seed)) {
            snapped += 1;
        } else {
            extended += 1;
            assert_eq!(rush_state.end_time, check_time + CHECK_INTERVAL);
        }
    }
    assert!(
        snapped > 0 && extended > 0,
        "{snapped} snapped, {extended} extended"
    );
}

#[test]
fn test_should_end_after_max_duration() {
    let mut rush_state = rushed();
    rush_state.max_duration += 2.0;
    let max_end = rush_state.start_time + rush_state.max_duration;
    // Before the next check, so only the maximum duration can end it.
    rush_state.end_time = f64::MAX;
    assert!(!rush_state.should_end(max_end - 0.1, &mut rng(0)));
    assert!(rush_state.should_end(max_end, &mut rng(0)));
}
