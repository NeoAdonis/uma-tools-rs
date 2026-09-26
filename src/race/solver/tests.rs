use super::*;
use crate::test_utils::race_params;

/// Index of the course feature containing `position`, given `(start, end)` ranges.
fn feature_at(position: f64, ranges: impl Iterator<Item = (f64, f64)>) -> Option<usize> {
    ranges
        .enumerate()
        .find(|(_, (start, end))| position >= *start && position < *end)
        .map(|(idx, _)| idx)
}

#[test]
fn test_position_derived_state_is_consistent_after_each_frame() {
    // Skills are checked against the runner's state; everything derived from the
    // position must match the position at the end of each frame
    for course_id in ["10101", "10104", "10606", "10913"] {
        let course = Course::from_json_file("data/course_data.json", course_id)
            .expect("Failed to load course data");
        let mut solver = RaceSolver::new(course, race_params(0), &Configuration::default());
        for style in [Style::Front, Style::Pace, Style::Late, Style::End] {
            let uma = Uma {
                speed: 1200,
                stamina: 1000,
                power: 1000,
                guts: 800,
                wiz: 1000,
                style,
                distance_aptitude: Aptitude::A,
                track_aptitude: Aptitude::A,
                style_aptitude: Aptitude::A,
            };
            solver.add_runner(uma, Motivation::Normal, vec![]);
        }
        let mut solver = solver.ready_runners();

        while solver.state.finish_time.is_none() {
            solver.step(1.0 / 15.0);

            let course = &solver.state.course;
            // Runners still in the gate aren't updated (nor are their skills checked)
            let started = solver
                .runners
                .iter()
                .filter(|runner| runner.start_delay <= solver.state.elapsed_time);
            for runner in started {
                let position = runner.position;
                let context = format!("course {course_id}, position {position}");
                assert_eq!(
                    runner.phase,
                    Phase::from_position(position, course),
                    "{context}"
                );
                assert_eq!(
                    runner.section,
                    course.section_from_position(position),
                    "{context}"
                );
                assert_eq!(
                    runner.current_corner(),
                    feature_at(
                        position,
                        course.corners.iter().map(|c| (c.start, c.start + c.length))
                    ),
                    "{context}"
                );
                assert_eq!(
                    runner.current_straight(),
                    feature_at(position, course.straights.iter().map(|s| (s.start, s.end))),
                    "{context}"
                );
                assert_eq!(
                    runner.current_slope(),
                    feature_at(
                        position,
                        course.slopes.iter().map(|s| (s.start, s.start + s.length))
                    ),
                    "{context}"
                );
            }
        }
    }
}
