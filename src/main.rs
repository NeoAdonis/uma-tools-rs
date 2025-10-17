//mod condition_parser;
mod course;
mod race_parameters;
mod runner;
mod skill;
mod solver;
mod uma;

use course::*;
use race_parameters::*;
use serde_json;
use skill::*;
use solver::*;
use uma::*;

fn main() {
    // TODO: load from file
    let json_data = r#"{
        "speed": 1500,
        "stamina": 800,
        "power": 1200,
        "guts": 700,
        "wisdom": 1200,
        "strategy": "Oikomi",
        "distanceAptitude": "S",
        "surfaceAptitude": "A",
        "strategyAptitude": "A",
        "skills": [200642]
    }"#;

    let uma_data = serde_json::from_str::<UmaData>(json_data).expect("Failed to parse JSON");
    let uma = uma_data.uma;
    let skill_ids = uma_data.skills;

    // Set sample parameters
    let params = RaceParameters {
        condition: TrackCondition::Good,
        weather: Weather::Sunny,
        season: Season::Spring,
        time: Time::None,
        grade: Grade::G3,
        popularity: 0,
    };

    // Set mood to good condition
    let mood = Mood::Good;

    // TODO: load course ID from args
    let course_id = "10913";

    // Load course data from JSON file
    let course = Course::from_json_file("data/course_data.json", course_id)
        .expect("Failed to load course data");

    println!("Loaded course:");
    println!("  ID: {}", course.id);
    println!("  Distance: {}m", course.distance);
    println!("  Surface: {:?}", course.surface);
    println!("  Course set status: {:?}", course.course_set_status);

    // Create sample skills (empty for now, can be expanded later)
    let skills: Vec<Skill> = vec![];

    // Initialize the race state (RaceSolver)
    let race_state = RaceState::new(uma, mood, course, params, skills);

    println!("\nRace initialized successfully!");
    println!("Initial HP: {:.2}", race_state.runner.hp);
}
