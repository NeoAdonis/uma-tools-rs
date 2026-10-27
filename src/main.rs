use std::path::PathBuf;

use uma_tools::configuration::Configuration;
use uma_tools::course::*;
use uma_tools::race::charge_up::ChargeUpOutcome;
use uma_tools::race::parameters::*;
use uma_tools::race::solver::*;
use uma_tools::skill::*;
use uma_tools::uma::*;

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

    // Resolve the seed so that the race can be reproduced.
    let seed = *config.seed.get_or_insert_with(rand::random);
    println!("RNG seed: {seed}");

    // Load Uma data from JSON file.
    let uma_data = UmaData::from_json_file(&uma_path).expect("Failed to parse uma JSON");
    let uma = uma_data.uma;
    let mood = uma_data.mood;
    let skill_ids = uma_data.skills;

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
        course.track_name(config.language).unwrap_or("Unknown")
    );
    println!("  Distance: {}m", course.distance);
    if course.run_up > 0.0 {
        println!("  Run-up: {}m", course.run_up);
    }
    println!("  Surface: {:?}", course.surface);
    println!("  Course set status: {:?}", course.course_set_status);

    // Load all skills from JSON file.
    let skills_map =
        Skill::load_skills_from_file("data/skill_data.json").expect("Failed to load skill data");

    println!("\nLoaded {} skills from JSON", skills_map.len());

    // Convert skill IDs to actual Skill objects.
    let skills: Vec<Skill> = skill_ids
        .iter()
        .filter_map(|id| skills_map.get(id).cloned())
        .collect();

    println!("Uma has {} skills", skills.len());
    for skill in &skills {
        println!("  Skill ID: {}, Rarity: {:?}", skill.id, skill.rarity);
    }

    // Initialize the race state.
    let mut solver = RaceSolver::new(course, race_parameters, config);
    solver.add_runner(uma, mood, skills);

    // Initialize runners.
    let mut solver = solver.ready_runners();

    {
        let runner = &solver.runners[0];

        println!("\nRace initialized successfully!");
        println!("Initial HP: {:.2}", runner.hp.maximum);
        println!("Rush chance: {:.2}%", runner.rush_chance);
        if let Some(rush_state) = &runner.rush_state {
            println!("> Will rush in section {}", rush_state.section);
        } else {
            println!("> Will not rush");
        }
        println!(
            "Start delay: {:.3} seconds ({:?})",
            runner.start_delay,
            runner.start_type()
        );
    }

    // Simulate.
    solver.run();

    {
        let runner = &solver.runners[0];

        println!("\nRace finished!");
        println!(
            "Final elapsed time: {:.2} seconds",
            solver.state.elapsed_time
        );
        println!(
            "Scoreboard time: {:.1} seconds",
            solver.scoreboard_time().unwrap_or_default()
        );
        println!("Final HP: {:.2}", runner.hp.current);
        println!(
            "Last spurt started at {:.2} meters ({:?})",
            runner.last_spurt.as_ref().unwrap().target_distance,
            runner.last_spurt.as_ref().unwrap().category
        );
        match &runner.charge_up {
            None => println!("Charge up: not eligible"),
            Some(charge_up) => match &charge_up.outcome {
                ChargeUpOutcome::Released(release) => println!(
                    "Charge up: conserved {:.1}, released at {:.2} meters for {:.2}s (+{:.3} m/s²)",
                    release.conserved,
                    release.start_position,
                    release.duration,
                    release.acceleration
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
        for skill in &runner.activated_skills {
            println!(
                "  Skill ID: {} at {:.2} meters",
                skill.skill_id, skill.distance
            );
        }
        if !runner.failed_skills().is_empty() {
            println!("Failed wit check:");
            for skill in runner.failed_skills() {
                println!("  Skill ID: {}", skill.id);
            }
        }
    }
}
