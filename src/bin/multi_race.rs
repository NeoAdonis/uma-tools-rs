//! Simulates the same race many times with different seeds and summarizes the
//! finish time of every runner.
//!
//! Races are split across `--threads` threads (one per logical CPU by
//! default); pass `--threads 1` to simulate them all in a single thread, e.g.
//! for profiling.

use std::ops::Range;
use std::path::PathBuf;
use std::thread;
use std::time::Instant;

use uma_tools::import::umaextractor::UmaExtractorData;
use uma_tools::{Configuration, Course, Mood, RaceParametersData, RaceSolver, Skill};

/// Mood of every runner, since UmaExtractor exports don't include it.
const MOOD: Mood = Mood::Great;

fn main() {
    let mut config_path = PathBuf::from("config.json");
    let mut uma_path = PathBuf::from("uma.json");
    let mut parameters_path = PathBuf::from("parameters.json");
    let mut races: u64 = 1000;
    // One thread per logical CPU by default, or a single one if unknown.
    let mut threads: u64 = thread::available_parallelism().map_or(1, |n| n.get() as u64);

    // Parse command-line arguments.
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--config" => {
                let value = args.next().expect("--config requires a file path");
                config_path = PathBuf::from(value);
            }
            "--uma" => {
                let value = args.next().expect("--uma requires a file path");
                uma_path = PathBuf::from(value);
            }
            "--parameters" => {
                let value = args.next().expect("--parameters requires a file path");
                parameters_path = PathBuf::from(value);
            }
            "--races" => {
                let value = args.next().expect("--races requires a number");
                races = value.parse().expect("--races requires a number");
            }
            "--threads" => {
                let value = args.next().expect("--threads requires a number");
                threads = value.parse().expect("--threads requires a number");
            }
            other => panic!("Unrecognized argument: {other}"),
        }
    }
    assert!(races > 0, "--races must be at least 1");
    assert!(threads > 0, "--threads must be at least 1");
    // Don't spawn threads that would have no races to simulate.
    let threads = threads.min(races);

    // Load everything once, so that every race only builds its solver.
    let config = Configuration::from_json_file(&config_path).expect("Failed to parse config JSON");
    let language = config.language;
    let parameters = RaceParametersData::from_json_file(&parameters_path)
        .expect("Failed to parse parameters JSON");
    let race_parameters = parameters.race;
    let course = Course::from_json_file("data/course_data.json", &parameters.course_id)
        .expect("Failed to load course data");
    let skills_map =
        Skill::load_all_from_json_file("data/skill_data.json").expect("Failed to load skill data");
    let all_umas =
        UmaExtractorData::all_from_json_file(&uma_path).expect("Failed to parse uma JSON");

    let distance_type = course.distance_type();
    let surface = course.surface;
    let entries: Vec<_> = all_umas
        .iter()
        .take(race_parameters.gates as usize)
        .map(|data| {
            let uma_data = data.to_uma_data(distance_type, surface, MOOD);
            let skills = uma_data
                .resolve_skills(&skills_map)
                .expect("Failed to resolve uma skills");
            (uma_data.uma, uma_data.mood, skills)
        })
        .collect();

    // Race N uses the base seed plus N, so that any race can be reproduced on its
    // own with that seed.
    let base_seed = config.seed.unwrap_or_else(rand::random);
    println!(
        "Course {} ({}), {} runners, {races} races, base seed {base_seed}, \
         {threads} threads",
        course.id,
        course.track_name(language).unwrap_or("Unknown"),
        entries.len()
    );

    // Simulate a range of races, returning their finish times by entry.
    let run_races = |races: Range<u64>| {
        let count = (races.end - races.start) as usize;
        let mut finish_times = vec![Vec::with_capacity(count); entries.len()];
        for race in races {
            let config = Configuration {
                seed: Some(base_seed.wrapping_add(race)),
                ..config
            };
            let mut solver = RaceSolver::new(course.clone(), race_parameters, config);
            for (uma, mood, skills) in &entries {
                solver.add_runner(*uma, *mood, skills.clone());
            }
            // Runners get reordered once ready, so map their gates back to
            // entries.
            let mut entry_by_gate = [0; u8::MAX as usize + 1];
            for (entry, runner) in solver.runners().iter().enumerate() {
                entry_by_gate[runner.gate_roll as usize] = entry;
            }

            let mut solver = solver.ready_runners();
            solver.run();
            for runner in solver.runners() {
                let entry = entry_by_gate[runner.gate_roll as usize];
                finish_times[entry].push(runner.finish_time.expect("Runner didn't finish"));
            }
        }
        finish_times
    };

    // Split the races into one contiguous range per thread, then gather the
    // finish times of every race, by entry.
    let start = Instant::now();
    let mut finish_times = thread::scope(|scope| {
        let run_races = &run_races;
        let handles: Vec<_> = (0..threads)
            .map(|thread| {
                let first = races * thread / threads;
                let last = races * (thread + 1) / threads;
                scope.spawn(move || run_races(first..last))
            })
            .collect();
        let mut finish_times = vec![Vec::with_capacity(races as usize); entries.len()];
        for handle in handles {
            let thread_times = handle.join().expect("Race thread panicked");
            for (times, thread_times) in finish_times.iter_mut().zip(thread_times) {
                times.extend(thread_times);
            }
        }
        finish_times
    });
    let elapsed = start.elapsed().as_secs_f64();
    println!(
        "Simulated in {elapsed:.3}s ({:.1} races/s, {:.3} ms/race)",
        races as f64 / elapsed,
        elapsed * 1000.0 / races as f64
    );

    println!("\nFinish times (s):");
    println!(
        "  {:<60} {:>8} {:>8} {:>8} {:>8} {:>8}",
        "Runner", "Mean", "Median", "Std dev", "Min", "Max"
    );
    let mut summaries: Vec<_> = entries
        .iter()
        .zip(&mut finish_times)
        .map(|((uma, _, _), times)| (uma, Summary::new(times)))
        .collect();
    summaries.sort_by(|(_, a), (_, b)| a.mean.total_cmp(&b.mean));
    for (uma, summary) in summaries {
        println!(
            "  {:<60} {:>8.3} {:>8.3} {:>8.3} {:>8.3} {:>8.3}",
            uma.name(language).unwrap_or("Unknown"),
            summary.mean,
            summary.median,
            summary.std_dev,
            summary.min,
            summary.max
        );
    }
}

/// Summary statistics of a sample.
struct Summary {
    mean: f64,
    median: f64,
    std_dev: f64,
    min: f64,
    max: f64,
}

impl Summary {
    /// Summarize a non-empty sample, sorting it in place.
    fn new(values: &mut [f64]) -> Self {
        values.sort_by(f64::total_cmp);
        let n = values.len();
        let mean = values.iter().sum::<f64>() / n as f64;
        let variance = values.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / n as f64;
        let median = if n.is_multiple_of(2) {
            (values[n / 2 - 1] + values[n / 2]) / 2.0
        } else {
            values[n / 2]
        };
        Summary {
            mean,
            median,
            std_dev: variance.sqrt(),
            min: values[0],
            max: values[n - 1],
        }
    }
}
