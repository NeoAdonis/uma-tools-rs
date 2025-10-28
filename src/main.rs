mod condition_evaluator;
mod condition_parser;
mod condition_variables;
mod course;
mod hp;
mod race_parameters;
mod race_transitions;
mod random_triggers;
mod runner;
mod skill;
mod solver;
mod trigger_builder;
mod trigger_system;
mod uma;
mod units;

use course::*;
use race_parameters::*;
use rand::distr::*;
use rand::prelude::*;
use serde_json;
use skill::*;
use solver::*;
use uma::*;

// TODO: remove
impl Distribution<Motivation> for StandardUniform {
    fn sample<R: Rng + ?Sized>(&self, rng: &mut R) -> Motivation {
        match rng.random_range(0..5) {
            0 => Motivation::Awful,
            1 => Motivation::Bad,
            2 => Motivation::Normal,
            3 => Motivation::Good,
            _ => Motivation::Great,
        }
    }
}
impl Distribution<TrackCondition> for StandardUniform {
    fn sample<R: Rng + ?Sized>(&self, rng: &mut R) -> TrackCondition {
        match rng.random_range(0..4) {
            0 => TrackCondition::Firm,
            1 => TrackCondition::Good,
            2 => TrackCondition::Soft,
            _ => TrackCondition::Heavy,
        }
    }
}
impl Distribution<Weather> for StandardUniform {
    fn sample<R: Rng + ?Sized>(&self, rng: &mut R) -> Weather {
        match rng.random_range(0..4) {
            0 => Weather::Sunny,
            1 => Weather::Cloudy,
            2 => Weather::Rainy,
            _ => Weather::Snowy,
        }
    }
}
impl Distribution<Grade> for StandardUniform {
    fn sample<R: Rng + ?Sized>(&self, rng: &mut R) -> Grade {
        match rng.random_range(0..7) {
            0 => Grade::G1,
            1 => Grade::G2,
            2 => Grade::G3,
            3 => Grade::OP,
            4 => Grade::PreOP,
            5 => Grade::Maiden,
            _ => Grade::Debut,
        }
    }
}
impl Distribution<Time> for StandardUniform {
    fn sample<R: Rng + ?Sized>(&self, rng: &mut R) -> Time {
        match rng.random_range(0..4) {
            0 => Time::Morning,
            1 => Time::Noon,
            2 => Time::Evening,
            _ => Time::Night,
        }
    }
}
impl Distribution<Season> for StandardUniform {
    fn sample<R: Rng + ?Sized>(&self, rng: &mut R) -> Season {
        match rng.random_range(0..5) {
            0 => Season::Spring,
            1 => Season::Summer,
            2 => Season::Autumn,
            3 => Season::Winter,
            _ => Season::Sakura,
        }
    }
}

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

    // TODO: do not use RNG
    let mut rng = rand::rng();

    let uma_data = serde_json::from_str::<UmaData>(json_data).expect("Failed to parse JSON");
    let uma = uma_data.uma;
    let skill_ids = uma_data.skills;

    // TODO: load parameters from args
    let params = RaceParameters {
        condition: rng.random::<TrackCondition>(),
        weather: rng.random::<Weather>(),
        season: rng.random::<Season>(),
        time: rng.random::<Time>(),
        grade: rng.random::<Grade>(),
        popularity: 0,
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

    // Initialize the race state (RaceSolver)
    let mut race_state = RaceState::new(uma, motivation, course, params, skills);

    println!("\nRace initialized successfully!");
    println!("Initial HP: {:.2}", race_state.runner.hp);

    // Simulate
    race_state.run();
}
