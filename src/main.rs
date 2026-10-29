use std::path::PathBuf;

use uma_tools::import::umaextractor::UmaExtractorData;
use uma_tools::names;
use uma_tools::race::charge_up::ChargeUpOutcome;
use uma_tools::{Configuration, Course, Mood, RaceParametersData, RaceSolver, Skill};

/// Mood of every runner, since UmaExtractor exports don't include it.
const MOOD: Mood = Mood::Great;

fn main() {
    let mut config_path = PathBuf::from("config.json");
    let mut uma_path = PathBuf::from("uma.json");
    let mut parameters_path = PathBuf::from("parameters.json");

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
            other => panic!("Unrecognized argument: {other}"),
        }
    }

    // Load configuration from JSON file.
    let mut config =
        Configuration::from_json_file(&config_path).expect("Failed to parse config JSON");
    let language = config.language;

    // Resolve the seed so that the race can be reproduced.
    let seed = *config.seed.get_or_insert_with(rand::random);
    println!("RNG seed: {seed}");

    // Load race parameters from JSON file.
    let parameters = RaceParametersData::from_json_file(&parameters_path)
        .expect("Failed to parse parameters JSON");

    let race_parameters = parameters.race;
    let course_id = parameters.course_id;

    // Load course data from JSON file.
    let course = Course::from_json_file("data/course_data.json", &course_id)
        .expect("Failed to load course data");

    println!("Loaded course:");
    println!("  ID: {}", course.id);
    println!(
        "  Name: {}",
        course.track_name(language).unwrap_or("Unknown")
    );
    println!(
        "  Distance: {}m ({:?})",
        course.distance,
        course.distance_type()
    );
    if course.run_up > 0.0 {
        println!("  Run-up: {}m", course.run_up);
    }
    println!("  Surface: {:?}", course.surface);
    println!("  Course set status: {:?}", course.course_set_status);

    // Load all skills from JSON file.
    let skills_map =
        Skill::load_all_from_json_file("data/skill_data.json").expect("Failed to load skill data");

    println!("\nLoaded {} skills from JSON", skills_map.len());

    // Load the Umas from JSON file, one per gate.
    let all_umas =
        UmaExtractorData::all_from_json_file(&uma_path).expect("Failed to parse uma JSON");
    let gates = race_parameters.gates as usize;
    if all_umas.len() < gates {
        println!(
            "Only {} Umas for {gates} gates, some will be empty",
            all_umas.len()
        );
    }

    // Initialize the race state.
    let distance_type = course.distance_type();
    let surface = course.surface;
    let mut solver = RaceSolver::new(course, race_parameters, config);
    println!("\nRunners:");
    for data in all_umas.iter().take(gates) {
        let uma_data = data.to_uma_data(distance_type, surface, MOOD);
        let skills = uma_data
            .resolve_skills(&skills_map)
            .expect("Failed to resolve uma skills");
        let uma = &uma_data.uma;
        println!(
            "  {} ({:?}): {}/{}/{}/{}/{}, aptitudes {:?}/{:?}/{:?}, {} skills",
            uma.name(language).unwrap_or("Unknown"),
            uma.style,
            uma.speed,
            uma.stamina,
            uma.power,
            uma.guts,
            uma.wit,
            uma.distance_aptitude,
            uma.track_aptitude,
            uma.style_aptitude,
            skills.len()
        );
        solver.add_runner(uma_data.uma, uma_data.mood, skills);
    }

    // Initialize runners.
    let mut solver = solver.ready_runners();
    println!("\nRace initialized successfully!");

    // Simulate.
    solver.run();

    println!("\nRace finished!");
    println!(
        "Final elapsed time: {:.2} seconds",
        solver.state.elapsed_time
    );
    println!(
        "Scoreboard time: {:.1} seconds",
        solver.scoreboard_time().unwrap_or_default()
    );

    let mut by_finish: Vec<_> = solver.runners().iter().collect();
    by_finish.sort_by(|a, b| a.finish_time.unwrap().total_cmp(&b.finish_time.unwrap()));

    println!("\nResults:");
    for (place, runner) in by_finish.iter().enumerate() {
        println!(
            "  {:>2}. Gate {:>2} {:<60} {:.2}s (+{:.2}m), popularity {}, HP {:.0}/{:.0}, \
             {} skills activated",
            place + 1,
            runner.gate_roll,
            runner.base.name(language).unwrap_or("Unknown"),
            runner.finish_time.unwrap(),
            runner.finish_margin.unwrap_or_default(),
            runner.initial_state.popularity_rank,
            runner.hp.current(),
            runner.hp.maximum(),
            runner.activated_skills.len()
        );
    }

    // Detail the winner's race.
    let winner = by_finish[0];
    println!(
        "\nWinner: {}",
        winner.base.name(language).unwrap_or("Unknown")
    );
    println!(
        "Start delay: {:.3} seconds ({:?})",
        winner.start_delay,
        winner.start_type()
    );
    if let Some(rush_state) = &winner.rush_state {
        println!("Rushed in section {}", rush_state.section);
    }
    println!(
        "Last spurt started at {:.2} meters ({:?})",
        winner.last_spurt.as_ref().unwrap().target_distance,
        winner.last_spurt.as_ref().unwrap().category
    );
    match &winner.charge_up {
        None => println!("Charge up: not eligible"),
        Some(charge_up) => match &charge_up.outcome {
            ChargeUpOutcome::Released(release) => println!(
                "Charge up: conserved {:.1}, released at {:.2} meters for {:.2}s (+{:.3} m/s²)",
                release.conserved, release.start_position, release.duration, release.acceleration
            ),
            ChargeUpOutcome::Insufficient => println!(
                "Charge up: conserved {:.1}, not enough to release",
                charge_up.conserved
            ),
            ChargeUpOutcome::Pending => println!(
                "Charge up: conserved {:.1}, not released",
                charge_up.conserved
            ),
        },
    }
    println!("Activated skills:");
    for skill in &winner.activated_skills {
        println!(
            "  {} ({}) at {:.2} meters",
            names::skill_name(skill.skill_id, language).unwrap_or("Unknown"),
            skill.skill_id,
            skill.distance
        );
    }
    if !winner.failed_skills().is_empty() {
        println!("Failed wit check:");
        for skill in winner.failed_skills() {
            println!(
                "  {} ({})",
                skill.name(language).unwrap_or("Unknown"),
                skill.id
            );
        }
    }
}
