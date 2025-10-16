//mod condition_parser;
mod course;
mod race_parameters;
mod runner;
mod skill;
mod solver;
mod uma;

use course::*;
use race_parameters::*;
use skill::*;
use solver::*;
use uma::*;

fn main() {
    // Create a sample Uma with balanced stats (A-rank stats around 1000)
    let sample_uma = Uma::new(
        1000,        // speed
        900,         // stamina
        950,         // power
        850,         // guts
        1050,        // wit
        Style::Late, // running style (late surger)
        Aptitude::A, // distance aptitude
        Aptitude::A, // track aptitude (turf/dirt)
        Aptitude::A, // style aptitude
    );

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

    // Create a sample 2000m turf course (Tokyo Racecourse - typical medium distance)
    let sample_course = Course {
        id: 10101,
        distance: 2000,
        distance_type: DistanceType::Mile,
        surface: Track::Turf,
        turn: Direction::Left,
        course_set_status: vec![1, 5], // Speed and Wit threshold bonuses
        finish_time_min: 114,
        finish_time_max: 126,
        lane_max: 8,
        corners: vec![
            Corner {
                start: 400,
                length: 200,
            },
            Corner {
                start: 1400,
                length: 300,
            },
        ],
        slopes: vec![Slope {
            start: 800,
            length: 100,
            slope: 2,
        }],
        straights: vec![
            Straight {
                start: 0,
                end: 400,
                front_type: FrontType::Audience,
            },
            Straight {
                start: 1700,
                end: 2000,
                front_type: FrontType::Audience,
            },
        ],
        race_track_id: 101,
    };

    // Create sample skills (empty for now, can be expanded later)
    let skills: Vec<Skill> = vec![];

    // Initialize the race state (RaceSolver)
    let race_state = RaceState::new(sample_uma, mood, sample_course, params, skills);

    println!("Race initialized successfully!");
    println!("Initial HP: {:.2}", race_state.runner.hp);
}
