use uma_tools::configuration::Language;
use uma_tools::course::*;
use uma_tools::race::parameters::*;
use uma_tools::race::solver::*;
use uma_tools::uma::skill::*;
use uma_tools::uma::*;

use rand::prelude::*;

fn main() {
    // TODO: load path from args
    let json_path = "uma_data.json";

    // TODO: do not use RNG
    let mut rng = rand::rng();

    let uma_data = UmaData::from_json_file(json_path).expect("Failed to parse JSON");
    let uma = uma_data.uma;
    let skill_ids = uma_data.skills;

    // TODO: load parameters from args
    let params = RaceParameters {
        condition: rng.random::<TrackCondition>(),
        weather: rng.random::<Weather>(),
        season: rng.random::<Season>(),
        time: rng.random::<Time>(),
        grade: rng.random::<Grade>(),
        gates: 18,
        rng_seed: rng.random(),
    };

    // TODO: load motivation from args
    let motivation = rng.random::<Motivation>();

    // TODO: load course ID from args
    let course_id = "10913";

    // Load course data from JSON file
    let course = Course::from_json_file("data/course_data.json", course_id)
        .expect("Failed to load course data");

    println!("Loaded course:");
    println!("  ID: {}", course.id);
    println!(
        "  Name: {}",
        course.track_name(Language::English).unwrap_or("Unknown")
    );
    println!("  Distance: {}m", course.distance);
    println!("  Surface: {:?}", course.surface);
    println!("  Course set status: {:?}", course.course_set_status);

    // Load all skills from JSON file
    let skills_map =
        Skill::load_skills_from_file("data/skill_data.json").expect("Failed to load skill data");

    println!("\nLoaded {} skills from JSON", skills_map.len());

    // Convert skill IDs to actual Skill objects
    let skills: Vec<Skill> = skill_ids
        .iter()
        .filter_map(|id| skills_map.get(&id.to_string()).cloned())
        .collect();

    println!("Uma has {} skills", skills.len());
    for skill in &skills {
        println!("  Skill ID: {}, Rarity: {:?}", skill.id, skill.rarity);
    }

    // Initialize the race state
    let mut solver = RaceSolver::new(course, params);
    solver.add_runner(uma, motivation, skills);

    // Initialize runners
    let mut solver = solver.ready_runners();

    {
        let runner = &solver.runners[0];

        println!("\nRace initialized successfully!");
        println!("Initial HP: {:.2}", runner.hp);
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

    // Simulate
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
        println!("Final HP: {:.2}", runner.hp);
        println!("Activated skills:");
        for skill in &runner.activated_skills {
            println!(
                "  Skill ID: {} at {:.2} meters",
                skill.skill_id, skill.distance
            );
        }
    }
}
