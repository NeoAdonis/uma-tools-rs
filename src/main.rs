mod condition;
mod course;
mod race;
mod trigger;
mod uma;
mod units;

use course::*;
use race::parameters::*;
use race::solver::*;
use rand::distr::*;
use rand::prelude::*;
use uma::skill::*;
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
    let mut solver = solver.build_runners();

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
