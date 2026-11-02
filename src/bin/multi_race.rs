//! Simulates the same race many times with different seeds and summarizes how
//! every runner performed: finish times, placings, last spurt and HP, race
//! behavior (positions, blocking, rushing, downhill mode, position fights,
//! duels) and, for one runner, how often each skill activates.
//!
//! Runners come from an UmaExtractor export (`--uma`, raced with the race
//! parameters of `--parameters`), or from an uma.moe build code (`--build`),
//! which has its own course, race conditions, teams and gates (its gate order
//! overrides the configured one, if it has one). `--focus` picks the runner
//! (by field index, 0 by default) whose skills are summarized.
//!
//! Races are split across `--threads` threads (one per logical CPU by
//! default); pass `--threads 1` to simulate them all in a single thread, e.g.
//! for profiling.

use std::ops::Range;
use std::path::PathBuf;
use std::sync::Arc;
use std::thread;
use std::time::Instant;

use uma_tools::course::Phase;
use uma_tools::import::umaextractor::UmaExtractorData;
use uma_tools::import::umamoe::UmaMoeBuild;
use uma_tools::race::last_spurt::LastSpurtCategory;
use uma_tools::race::parameters::RaceParameters;
use uma_tools::race::solver::{DELTA_TIME, FINISH_TIME_SCALE};
use uma_tools::{Configuration, Course, Mood, RaceParametersData, RaceSolver, Skill, Uma};

/// Mood of every runner from UmaExtractor, since its exports don't include it.
const MOOD: Mood = Mood::Great;

/// A runner of the field, in the order the runners are added to the race.
struct Entry {
    uma: Uma,
    mood: Mood,
    skills: Vec<Arc<Skill>>,
    /// Gate set for the runner, if any.
    gate: Option<u8>,
}

/// What a runner did in a single race.
#[derive(Default)]
struct RunnerRace {
    /// Simulated finish time (s).
    finish_time: f64,
    /// Finishing place, 1-based.
    place: usize,
    start_delay: f64,
    /// Order upon entering the Mid and Late race.
    mid_order: Option<u8>,
    late_order: Option<u8>,
    /// Time (s) spent front-blocked, and number of times it started.
    blocked_time: f64,
    blocked_count: u32,
    /// Time (s) spent rushed, if the runner rushed.
    rushed_time: Option<f64>,
    /// Time (s) spent in downhill mode.
    downhill_time: f64,
    /// Whether the runner took part in a position fight (spot struggle).
    struggled: bool,
    dueled: bool,
    full_spurt: bool,
    spurt_speed: f64,
    /// Distance (m) to the goal when HP first ran out, if it did.
    hp_out_to_go: Option<f64>,
    max_hp: f64,
    /// HP left at the goal, zero if it ran out.
    hp_left: f64,
    /// IDs of the skills that activated, without repetitions.
    activated: Vec<i32>,
}

fn main() {
    let mut config_path = PathBuf::from("config.json");
    let mut uma_path = PathBuf::from("uma.json");
    let mut parameters_path = PathBuf::from("parameters.json");
    let mut build_path = None;
    let mut focus = 0;
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
            "--build" => {
                let value = args.next().expect("--build requires a file path");
                build_path = Some(PathBuf::from(value));
            }
            "--focus" => {
                let value = args.next().expect("--focus requires a number");
                focus = value.parse().expect("--focus requires a number");
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
    let mut config =
        Configuration::from_json_file(&config_path).expect("Failed to parse config JSON");
    let language = config.language;
    let skills_map =
        Skill::load_all_from_json_file("data/skill_data.json").expect("Failed to load skill data");
    let (courses, rails) = ("data/course_data.json", "data/rails");
    let load_course = |parameters: &RaceParametersData| {
        Course::from_json_files(courses, rails, &parameters.course_id)
            .expect("Failed to load course data")
    };

    // Entries in the order they're added, the teams they're added in (as
    // entry indices), and each entry's index in the field, to report them in
    // field order.
    let (course, race_parameters, entries, teams, field_order): (
        Course,
        RaceParameters,
        Vec<Entry>,
        Vec<Range<usize>>,
        Vec<usize>,
    ) = if let Some(build_path) = &build_path {
        let build = UmaMoeBuild::from_build_code_file(build_path).expect("Failed to parse build");
        config.gate_order = build.gate_order().unwrap_or(config.gate_order);
        let parameters = build.race_parameters();
        let course = load_course(&parameters);
        let (distance, track) = (course.distance_type(), course.surface);
        let field = build.runners();
        let (mut entries, mut teams, mut field_order) = (Vec::new(), Vec::new(), Vec::new());
        for team in build.teams() {
            let start = entries.len();
            for runner in team {
                let uma_data = runner.to_uma_data(distance, track);
                let skills = uma_data
                    .resolve_skills(&skills_map)
                    .expect("Failed to resolve uma skills");
                let index = field.iter().position(|r| std::ptr::eq(r, runner));
                field_order.push(index.expect("Runner not in field"));
                entries.push(Entry {
                    uma: uma_data.uma,
                    mood: uma_data.mood,
                    skills,
                    gate: runner.gate(),
                });
            }
            teams.push(start..entries.len());
        }
        (course, parameters.race, entries, teams, field_order)
    } else {
        let parameters = RaceParametersData::from_json_file(&parameters_path)
            .expect("Failed to parse parameters JSON");
        let course = load_course(&parameters);
        let (distance, track) = (course.distance_type(), course.surface);
        let all_umas =
            UmaExtractorData::all_from_json_file(&uma_path).expect("Failed to parse uma JSON");
        let entries: Vec<_> = all_umas
            .iter()
            .take(parameters.race.gates as usize)
            .map(|data| {
                let uma_data = data.to_uma_data(distance, track, MOOD);
                let skills = uma_data
                    .resolve_skills(&skills_map)
                    .expect("Failed to resolve uma skills");
                Entry {
                    uma: uma_data.uma,
                    mood: uma_data.mood,
                    skills,
                    gate: None,
                }
            })
            .collect();
        let teams = (0..entries.len()).map(|i| i..i + 1).collect();
        let field_order = (0..entries.len()).collect();
        (course, parameters.race, entries, teams, field_order)
    };
    let focus = field_order
        .iter()
        .position(|&index| index == focus)
        .expect("--focus is out of the field");

    // Race N uses the base seed plus N, so that any race can be reproduced on its
    // own with that seed.
    let base_seed = config.seed.unwrap_or_else(rand::random);
    println!(
        "Course {} ({}, {} m), {:?}, {:?}, {:?}, {:?}, {} runners, gates {:?}, \
         {races} races, base seed {base_seed}, {threads} threads",
        course.id,
        course.track_name(language).unwrap_or("Unknown"),
        course.distance,
        race_parameters.track_condition,
        race_parameters.weather,
        race_parameters.season,
        race_parameters.time,
        entries.len(),
        config.gate_order
    );

    // Simulate a range of races, returning what every entry did in each.
    let run_races = |races: Range<u64>| {
        let count = (races.end - races.start) as usize;
        let mut results: Vec<Vec<RunnerRace>> = (0..entries.len())
            .map(|_| Vec::with_capacity(count))
            .collect();
        for race in races {
            let config = Configuration {
                seed: Some(base_seed.wrapping_add(race)),
                ..config
            };
            let mut solver = RaceSolver::new(course.clone(), race_parameters, config);
            for team in &teams {
                let members = entries[team.clone()]
                    .iter()
                    .map(|entry| (entry.uma, entry.mood, entry.skills.clone()))
                    .collect();
                solver.add_team(members);
            }
            for (index, entry) in entries.iter().enumerate() {
                if let Some(gate) = entry.gate {
                    solver.set_gate(index, gate);
                }
            }

            // Runners get reordered once ready, but keep the index of their entry.
            let mut solver = solver.ready_runners();
            let mut stats: Vec<RunnerRace> =
                (0..entries.len()).map(|_| Default::default()).collect();
            let mut was_blocked = vec![false; entries.len()];
            let mut finished = false;
            while !finished {
                finished = solver.step_frame();
                for runner in solver.runners() {
                    let entry = runner.index as usize;
                    let stats = &mut stats[entry];
                    if stats.place > 0 {
                        continue;
                    }
                    match runner.phase {
                        Phase::Mid if stats.mid_order.is_none() => {
                            stats.mid_order = Some(runner.order.order);
                        }
                        Phase::Late if stats.late_order.is_none() => {
                            stats.late_order = Some(runner.order.order);
                        }
                        _ => {}
                    }
                    let blocked = runner.is_front_blocked();
                    if blocked {
                        stats.blocked_time += DELTA_TIME;
                        if !was_blocked[entry] {
                            stats.blocked_count += 1;
                        }
                    }
                    was_blocked[entry] = blocked;
                    if runner.is_in_downhill_mode() {
                        stats.downhill_time += DELTA_TIME;
                    }
                    if runner.hp.has_emptied() && stats.hp_out_to_go.is_none() {
                        let to_go = course.length() - runner.position;
                        stats.hp_out_to_go = Some(to_go.max(0.0));
                    }
                    if let Some(finish_time) = runner.finish_time {
                        // Ranked below, once everyone has finished.
                        stats.place = usize::MAX;
                        stats.finish_time = finish_time;
                        stats.max_hp = runner.hp.maximum();
                        stats.hp_left = runner.hp.current().max(0.0);
                    }
                }
            }

            let mut by_finish: Vec<usize> = (0..entries.len()).collect();
            by_finish.sort_by(|&a, &b| stats[a].finish_time.total_cmp(&stats[b].finish_time));
            for (place, &entry) in by_finish.iter().enumerate() {
                stats[entry].place = place + 1;
            }
            for runner in solver.runners() {
                let stats = &mut stats[runner.index as usize];
                stats.start_delay = runner.start_delay;
                stats.rushed_time = runner
                    .rush_state
                    .as_ref()
                    .filter(|rush| rush.has_rushed())
                    .map(|rush| rush.end_time - rush.start_time);
                stats.struggled = runner.spot_struggle.has_struggled();
                stats.dueled = runner.duel.has_dueled();
                if let Some(last_spurt) = runner.last_spurt {
                    stats.full_spurt = last_spurt.category == LastSpurtCategory::Full;
                    stats.spurt_speed = last_spurt.max_speed;
                }
                stats.activated = runner.activated_skills.iter().map(|r| r.skill_id).collect();
                stats.activated.sort_unstable();
                stats.activated.dedup();
            }
            for (results, stats) in results.iter_mut().zip(stats) {
                results.push(stats);
            }
        }
        results
    };

    // Split the races into one contiguous range per thread, then gather the
    // results of every race, by entry.
    let start = Instant::now();
    let results = thread::scope(|scope| {
        let run_races = &run_races;
        let handles: Vec<_> = (0..threads)
            .map(|thread| {
                let first = races * thread / threads;
                let last = races * (thread + 1) / threads;
                scope.spawn(move || run_races(first..last))
            })
            .collect();
        let mut results: Vec<Vec<RunnerRace>> = (0..entries.len()).map(|_| Vec::new()).collect();
        for handle in handles {
            let thread_results = handle.join().expect("Race thread panicked");
            for (results, thread_results) in results.iter_mut().zip(thread_results) {
                results.extend(thread_results);
            }
        }
        results
    });
    let elapsed = start.elapsed().as_secs_f64();
    println!(
        "Simulated in {elapsed:.3}s ({:.1} races/s, {:.3} ms/race)",
        races as f64 / elapsed,
        elapsed * 1000.0 / races as f64
    );

    let name = |entry: usize| entries[entry].uma.name(language).unwrap_or("Unknown");
    // Wide enough for every name and the header.
    let width = (0..entries.len())
        .map(|entry| name(entry).chars().count())
        .fold("Runner".len(), usize::max);
    let mut by_field: Vec<usize> = (0..entries.len()).collect();
    by_field.sort_by_key(|&entry| field_order[entry]);
    let n = races as f64;
    let percent = |count: usize| 100.0 * count as f64 / n;

    println!("\nFinish times (s) and placings:");
    println!(
        "  {:<width$} {:>8} {:>8} {:>8} {:>8} {:>8} {:>8} {:>6} {:>6}",
        "Runner", "Mean", "Median", "Std dev", "Min", "Max", "Scaled", "Win%", "Top3%"
    );
    for &entry in &by_field {
        let results = &results[entry];
        let mut times: Vec<_> = results.iter().map(|r| r.finish_time).collect();
        let summary = Summary::new(&mut times);
        println!(
            "  {:<width$} {:>8.3} {:>8.3} {:>8.3} {:>8.3} {:>8.3} {:>8} {:>6.1} {:>6.1}",
            name(entry),
            summary.mean,
            summary.median,
            summary.std_dev,
            summary.min,
            summary.max,
            format_time(summary.mean * FINISH_TIME_SCALE),
            percent(results.iter().filter(|r| r.place == 1).count()),
            percent(results.iter().filter(|r| r.place <= 3).count()),
        );
    }

    println!("\nLast spurt and HP:");
    println!(
        "  {:<width$} {:>7} {:>9} {:>8} {:>8} {:>8} {:>10} {:>8}",
        "Runner", "Full%", "Spurt m/s", "Max HP", "HP last%", "HP out%", "Out to go", "HP left"
    );
    for &entry in &by_field {
        let results = &results[entry];
        let outs: Vec<_> = results.iter().filter_map(|r| r.hp_out_to_go).collect();
        println!(
            "  {:<width$} {:>7.1} {:>9.3} {:>8.0} {:>8.1} {:>8.1} {:>10.1} {:>8.1}",
            name(entry),
            percent(results.iter().filter(|r| r.full_spurt).count()),
            mean(results.iter().map(|r| r.spurt_speed)),
            mean(results.iter().map(|r| r.max_hp)),
            percent(results.len() - outs.len()),
            percent(outs.len()),
            mean(outs.iter().copied()),
            mean(results.iter().map(|r| r.hp_left)),
        );
    }

    println!("\nRace behavior (positions are mean orders; times in s):");
    println!(
        "  {:<width$} {:>6} {:>5} {:>5} {:>7} {:>7} {:>7} {:>7} {:>7} {:>8} {:>6} {:>6}",
        "Runner",
        "Start",
        "Mid",
        "Late",
        "Block%",
        "Block t",
        "Block n",
        "Rush%",
        "Rush t",
        "Downhill",
        "Fight%",
        "Duel%"
    );
    for &entry in &by_field {
        let results = &results[entry];
        let blocked: Vec<_> = results.iter().filter(|r| r.blocked_count > 0).collect();
        let rushed: Vec<_> = results.iter().filter_map(|r| r.rushed_time).collect();
        println!(
            "  {:<width$} {:>6.3} {:>5.2} {:>5.2} {:>7.1} {:>7.2} {:>7.2} {:>7.1} {:>7.2} {:>8.2} {:>6.1} {:>6.1}",
            name(entry),
            mean(results.iter().map(|r| r.start_delay)),
            mean(results.iter().filter_map(|r| r.mid_order).map(f64::from)),
            mean(results.iter().filter_map(|r| r.late_order).map(f64::from)),
            percent(blocked.len()),
            mean(blocked.iter().map(|r| r.blocked_time)),
            mean(blocked.iter().map(|r| r.blocked_count as f64)),
            percent(rushed.len()),
            mean(rushed.iter().copied()),
            mean(results.iter().map(|r| r.downhill_time)),
            percent(results.iter().filter(|r| r.struggled).count()),
            percent(results.iter().filter(|r| r.dueled).count()),
        );
    }

    println!("\nSkill activations of {} (% of races):", name(focus));
    let mut activations: Vec<_> = entries[focus]
        .skills
        .iter()
        .map(|skill| {
            let count = results[focus]
                .iter()
                .filter(|r| r.activated.binary_search(&skill.id).is_ok())
                .count();
            (skill, percent(count))
        })
        .collect();
    activations.sort_by(|(_, a), (_, b)| a.total_cmp(b));
    for (skill, rate) in activations {
        let skill_name = skill.name(language).unwrap_or("Unknown");
        println!("  {:>6} {:<40} {rate:>6.1}", skill.id, skill_name);
    }
}

/// Mean of `values`, or NaN if there are none.
fn mean(values: impl Iterator<Item = f64>) -> f64 {
    let (sum, count) = values.fold((0.0, 0), |(sum, count), v| (sum + v, count + 1));
    sum / count as f64
}

/// Format `seconds` as `m:ss.ss`.
fn format_time(seconds: f64) -> String {
    let minutes = (seconds / 60.0).floor();
    format!("{minutes}:{:05.2}", seconds - minutes * 60.0)
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
