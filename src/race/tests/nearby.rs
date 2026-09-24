use crate::course::Course;
use crate::race::parameters::*;
use crate::race::runner::Runner;
use crate::race::solver::RaceSolver;
use crate::uma::*;

const DELTA_TIME: f64 = 1.0 / 15.0;

fn ready_runners(count: usize) -> Vec<Runner> {
    let course = Course::from_json_file("data/course_data.json", "10101")
        .expect("Failed to load course data");
    let params = RaceParameters {
        track_condition: TrackCondition::Firm,
        weather: Weather::Sunny,
        season: Season::Spring,
        time: Time::None,
        grade: Grade::G1,
        gates: 9,
        rng_seed: 42,
    };
    let uma = Uma {
        speed: 1200,
        stamina: 1000,
        power: 1000,
        guts: 800,
        wiz: 1000,
        style: Style::Pace,
        distance_aptitude: Aptitude::A,
        track_aptitude: Aptitude::A,
        style_aptitude: Aptitude::A,
    };
    let mut solver = RaceSolver::new(course, params);
    for _ in 0..count {
        solver.add_runner(uma, Motivation::Normal, vec![]);
    }
    solver.ready_runners().runners
}

/// Simulate one blocking update cycle between a runner ahead and a runner behind.
fn update_cycle(ahead: &mut Runner, behind: &mut Runner, is_near: bool) {
    ahead.reset_blocking_status();
    behind.reset_blocking_status();
    if is_near {
        behind.update_near_same_lane_behind(ahead);
        ahead.nearby.behind_set1_count += 1;
    }
    ahead.update_near_time(DELTA_TIME);
    behind.update_near_time(DELTA_TIME);
}

#[test]
fn test_near_counts_do_not_accumulate_across_cycles() {
    let mut runners = ready_runners(2);
    let (left, right) = runners.split_at_mut(1);
    let (ahead, behind) = (&mut left[0], &mut right[0]);

    // Well beyond u8::MAX cycles; would overflow if counts were never reset
    for _ in 0..1000 {
        update_cycle(ahead, behind, true);
        assert_eq!(behind.nearby.ahead_same_lane_count, 1);
        assert_eq!(ahead.nearby.behind_same_lane_count, 1);
        assert_eq!(ahead.nearby.behind_set1_count, 1);
    }
}

#[test]
fn test_near_times_reset_after_separation() {
    let mut runners = ready_runners(2);
    let (left, right) = runners.split_at_mut(1);
    let (ahead, behind) = (&mut left[0], &mut right[0]);

    for _ in 0..30 {
        update_cycle(ahead, behind, true);
    }
    assert!(behind.nearby.ahead_same_lane_time > 1.9);
    assert!(ahead.nearby.behind_same_lane_time > 1.9);
    assert!(ahead.nearby.behind_set1_time > 1.9);

    update_cycle(ahead, behind, false);
    assert_eq!(behind.nearby.ahead_same_lane_time, 0.0);
    assert_eq!(ahead.nearby.behind_same_lane_time, 0.0);
    assert_eq!(ahead.nearby.behind_set1_time, 0.0);
}
