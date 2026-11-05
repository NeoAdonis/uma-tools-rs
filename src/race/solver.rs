//! Race setup and frame-by-frame simulation.
//!
//! A [`RaceSolver`] holds the shared [`RaceState`] and every runner of the
//! race. Runners are added while unready, then made ready all at once, after
//! which [`RaceSolver::run`] advances the race by [`DELTA_TIME`] per frame
//! until every runner finishes.
//!
//! Besides updating each runner, the solver handles everything that involves
//! several runners: order, blocking and overlaps, positioning strategy, skill
//! effects that target other runners, and mechanics such as spot struggle,
//! dueling, secure lead and compete before spurt.
//!
//! See [`RaceSolver`] for examples.

use std::collections::HashMap;
use std::sync::Arc;
use std::vec;

use rand::prelude::*;

use crate::configuration::{Assignment, Configuration, DefaultGate, DefaultMood, GameVersion};
use crate::course::Course;
use crate::race::compete_before_spurt;
use crate::race::duel;
use crate::race::parameters::{MAX_GATES, RaceParameters};
use crate::race::popularity::calculate_popularity;
use crate::race::proximity;
use crate::race::rng::{RngSeed, Stream};
use crate::race::runner::{
    POSITIONING_END_SECTION, PositioningStrategy, Runner, SkillActivationFlags,
};
use crate::race::secure_lead;
use crate::race::spot_struggle;
use crate::race::unready_runner::UnreadyRunner;
use crate::skill::*;
use crate::uma::*;
use crate::units::CourseWidths;

/// Scale applied to raw finish times to get displayed times.
pub const FINISH_TIME_SCALE: f64 = 1.18;

/// Duration of a single frame in seconds (race state is updated 15 times per
/// second).
pub const DELTA_TIME: f64 = 1.0 / 15.0;

/// Speed used to convert scaled finish time differences into margins, in m/s.
const MARGIN_SPEED: f64 = 15.0;

/// Maximum number of runners in a single team.
pub const MAX_TEAM_SIZE: usize = 3;

#[derive(Debug, PartialEq)]
pub struct RaceState {
    pub course: Course,
    pub params: RaceParameters,
    /// Runtime configuration. Shared by all runners so that they all follow the
    /// same game behavior.
    pub config: Configuration,
    /// Seed from which every random number stream of the race derives (see
    /// [`crate::race::rng`]).
    pub(crate) seed: RngSeed,
    /// Elapsed time in seconds. This value is shared by all runners.
    pub elapsed_time: f64,
    /// Time when the first runner finished, in seconds. This might be different
    /// from displayed finish time due to scoreboard conversion.
    pub finish_time: Option<f64>,
    /// Number of teams in the race. Team indices go from 0 to `team_count - 1`.
    pub(crate) team_count: u8,
    /// Count of runners by base style (Runaways count as Front runners).
    /// Indexed as `[style as usize]`.
    pub(crate) runners_by_style: [u8; Style::COUNT],
    /// Base style of the most popular runner (Runaway counts as Front).
    pub(crate) style_most_popular: Style,
    /// Count of runners currently rushed, by team and base style (Runaways count
    /// as Front runners). Indexed as `[team][style as usize]`.
    pub(crate) rushed_by_team: Vec<[u8; Style::COUNT]>,
    /// Skill activations by team (out-of-gate skills included), as of the start
    /// of the current frame. Indexed by team.
    pub(crate) team_activations: Vec<usize>,
    /// Count of existing skill IDs among all runners.
    pub(crate) existing_skills: HashMap<i32, u8>,
    /// Number of runners that activated a buff (advantage) skill in the current
    /// frame, by effect type.
    pub(crate) buffs_activated: HashMap<SkillType, u8>,
    /// Skill activations of the most popular runner in the current frame.
    pub(crate) popularity_top_activations: SkillActivationFlags,
}

/// Identifier of a runner in a race, returned when she's added to the
/// [`RaceSolver`] (see [`RaceSolver::add_runner`]).
///
/// It identifies her throughout the race, even after runners get reordered
/// (see [`RaceSolver::runner`]). It's only meaningful for the solver that
/// returned it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RunnerId(u8);

/// A runner as entered in a race: what she's added to the [`RaceSolver`]
/// with, before she gets ready (see [`RaceSolver::ready_runners`]).
///
/// # Examples
///
/// Enter a runner with a mood and a gate, leaving the rest to the solver:
///
/// ```
/// use uma_tools::{Aptitude, Entrant, Mood, Style, Uma};
///
/// let uma = Uma {
///     speed: 1200,
///     stamina: 800,
///     power: 1000,
///     guts: 600,
///     wit: 1100,
///     style: Style::Late,
///     distance_aptitude: Aptitude::A,
///     track_aptitude: Aptitude::A,
///     style_aptitude: Aptitude::A,
///     fan_count: None,
///     character_id: None,
/// };
/// let entrant = Entrant {
///     mood: Some(Mood::Good),
///     gate: Some(3),
///     ..Entrant::new(uma, vec![])
/// };
/// assert_eq!(entrant.uma.style, Style::Late);
/// ```
///
/// [`RaceSolver`]: crate::RaceSolver
/// [`RaceSolver::ready_runners`]: crate::RaceSolver::ready_runners
#[derive(Debug, Clone)]
pub struct Entrant {
    /// Raw stats, before halving and mood multiplier.
    pub uma: Uma,
    /// Mood, if set. Whether she keeps it depends on
    /// [`Configuration::mood_assignment`].
    ///
    /// [`Configuration::mood_assignment`]: crate::Configuration::mood_assignment
    pub mood: Option<Mood>,
    /// Gate number (1-indexed), if set. Whether she keeps it depends on
    /// [`Configuration::gate_assignment`].
    ///
    /// [`Configuration::gate_assignment`]: crate::Configuration::gate_assignment
    pub gate: Option<u8>,
    /// Skills, before wit checks.
    pub skills: Vec<Arc<Skill>>,
}

impl Entrant {
    /// Entrant with the given raw stats and skills, with no mood nor gate set.
    pub fn new(uma: Uma, skills: Vec<Arc<Skill>>) -> Self {
        Entrant {
            uma,
            mood: None,
            gate: None,
            skills,
        }
    }
}

/// Race solver that manages the race simulation.
///
/// It is managed in two phases:
///
/// 1. Before runners are ready (`RaceSolver<Entrant>`): Runners can be
///    added, but their gates, moods and stats are not set yet.
/// 2. After runners are ready (`RaceSolver<Runner>`): Runners have their gates
///    and moods, their stats are calculated and out-of-gate skills are
///    applied. The simulation can begin.
///
/// # Examples
///
/// The [crate-level example](crate) simulates a single race. Since every skill
/// draws from its own random streams, races with the same seed stay paired
/// when a skill is added, so running the same seeds with and without it
/// measures how much time it saves:
///
/// ```
/// use std::sync::Arc;
/// use uma_tools::{
///     Configuration, Course, Entrant, RaceParametersData, RaceSolver, Skill,
///     SkillData, UmaData,
/// };
///
/// # fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
/// let parameters = RaceParametersData::from_json(
///     r#"{
///         "trackCondition": "Firm", "weather": "Sunny", "season": "Spring",
///         "time": "Noon", "grade": "G1", "gates": 18, "courseId": "10913"
///     }"#,
/// )?;
/// let uma = UmaData::from_json(
///     r#"{
///         "speed": 1500, "stamina": 800, "power": 1200, "guts": 700,
///         "wisdom": 1200, "strategy": "Oikomi", "distanceAptitude": "S",
///         "surfaceAptitude": "A", "strategyAptitude": "A",
///         "motivation": "Great", "skills": [200331]
///     }"#,
/// )?;
/// let (courses, rails) = ("data/course_data.json", "data/rails");
/// let course =
///     Course::from_json_files(courses, rails, &parameters.course_id)?;
/// let all_skills = SkillData::from_json_file("data/skill_data.json")?;
/// let skills = uma.resolve_skills(&all_skills)?;
///
/// let finish_time = |seed, skills: &[Arc<Skill>]| {
///     let config = Configuration {
///         seed: Some(seed),
///         ..Configuration::default()
///     };
///     let course = course.clone();
///     let mut solver = RaceSolver::new(course, parameters.race, config);
///     let entrant = Entrant {
///         mood: uma.mood,
///         ..Entrant::new(uma.uma, skills.to_vec())
///     };
///     solver.add_runner(entrant);
///     let mut solver = solver.ready_runners();
///     solver.run();
///     solver.runners()[0].finish_time
/// };
///
/// let seeds = 20;
/// let saved: f64 = (0..seeds)
///     .filter_map(|seed| {
///         Some(finish_time(seed, &[])? - finish_time(seed, &skills)?)
///     })
///     .sum();
/// // Professor of Curvature saves about 0.07 seconds on average, wit check
/// // failures included.
/// assert!(saved / seeds as f64 > 0.05);
/// # Ok(())
/// # }
/// ```
#[derive(Debug)]
pub struct RaceSolver<T> {
    /// Runners of the current phase (see [`Self::runners`]).
    pub(crate) runners: Vec<T>,
    /// Team of each runner, indexed by [`RunnerId`].
    teams_by_id: Vec<u8>,
    /// Gate of each runner, indexed by [`RunnerId`]. Empty until runners are
    /// ready.
    gates_by_id: Vec<u8>,
    pub state: RaceState,
    /// Time (s) each pair of runners has been close in a row, indexed by their
    /// gates (see [`Self::duel_pair_index`]). Only pairs that may still start a
    /// duel are tracked.
    duel_close_times: Vec<Option<f64>>,
    /// Close times of the previous frame, kept to reuse their memory.
    prev_duel_close_times: Vec<Option<f64>>,
}

/// Methods available in both phases.
impl<T> RaceSolver<T> {
    /// Runners of the race.
    ///
    /// Before they're ready, they're in the order they were added. Once ready,
    /// they're ordered by gate, then by position (leader first) after every
    /// frame of the race.
    pub fn runners(&self) -> &[T] {
        &self.runners
    }

    /// Take the runners out of the solver, in the order of [`Self::runners`].
    pub fn into_runners(self) -> Vec<T> {
        self.runners
    }
}

/// Methods available BEFORE runners are ready (before stats have been
/// calculated and out-of-gate skills have been applied).
impl RaceSolver<Entrant> {
    /// Create a new RaceSolver for a given course and parameters.
    ///
    /// If the configuration has no seed, seed 0 is used.
    ///
    /// # Panics
    ///
    /// Panics if `params.gates` isn't between 1 and [`MAX_GATES`].
    pub fn new(course: Course, params: RaceParameters, config: Configuration) -> Self {
        let gates = params.gates;
        assert!(
            (1..=MAX_GATES).contains(&gates),
            "Invalid gates {gates}: expected 1 to {MAX_GATES}"
        );
        let seed = RngSeed::new(config.seed.unwrap_or(0));

        let state = RaceState {
            course,
            params,
            config,
            seed,
            elapsed_time: 0.0,
            finish_time: None,
            team_count: 0,
            style_most_popular: Style::Front,
            runners_by_style: [0; Style::COUNT],
            rushed_by_team: Vec::new(),
            team_activations: Vec::new(),
            existing_skills: HashMap::new(),
            buffs_activated: HashMap::new(),
            popularity_top_activations: SkillActivationFlags::default(),
        };

        RaceSolver {
            runners: Vec::new(),
            teams_by_id: Vec::new(),
            gates_by_id: Vec::new(),
            state,
            duel_close_times: Vec::new(),
            prev_duel_close_times: Vec::new(),
        }
    }

    /// Add an individual runner to the race BEFORE it starts.
    ///
    /// The runner gets a team of its own, so every other runner is an opponent.
    /// See [`RaceSolver::add_team`] to add allies instead.
    ///
    /// Her mood and gate may be unset, or replaced by default ones when she
    /// gets ready (see [`Configuration::mood_assignment`] and
    /// [`Configuration::gate_assignment`]).
    ///
    /// Returns the runner's ID, to find her once runners are ready (see
    /// [`RaceSolver::runner`]).
    ///
    /// # Examples
    ///
    /// Start the third runner from the innermost gate, and the rest from the
    /// next ones in the order they were added:
    ///
    /// ```
    /// # use uma_tools::configuration::DefaultGate;
    /// # use uma_tools::{Configuration, Course, Entrant, RaceParametersData};
    /// # use uma_tools::{RaceSolver, SkillData, UmaData};
    /// # fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    /// # let parameters = RaceParametersData::from_json(
    /// #     r#"{
    /// #         "trackCondition": "Firm", "weather": "Sunny", "season": "Spring",
    /// #         "time": "Noon", "grade": "G1", "gates": 18, "courseId": "10913"
    /// #     }"#,
    /// # )?;
    /// # let uma = UmaData::from_json(
    /// #     r#"{
    /// #         "speed": 1500, "stamina": 800, "power": 1200, "guts": 700,
    /// #         "wisdom": 1200, "strategy": "Oikomi", "distanceAptitude": "S",
    /// #         "surfaceAptitude": "A", "strategyAptitude": "A",
    /// #         "motivation": "Great", "skills": [200331]
    /// #     }"#,
    /// # )?;
    /// # let (courses, rails) = ("data/course_data.json", "data/rails");
    /// # let course =
    /// #     Course::from_json_files(courses, rails, &parameters.course_id)?;
    /// # let all_skills = SkillData::from_json_file("data/skill_data.json")?;
    /// let config = Configuration {
    ///     default_gate: DefaultGate::InOrder,
    ///     ..Configuration::default()
    /// };
    /// let mut solver = RaceSolver::new(course, parameters.race, config);
    /// let entrant = uma.to_entrant(&all_skills)?;
    /// let innermost = Entrant {
    ///     gate: Some(1),
    ///     ..entrant.clone()
    /// };
    /// let ids = [
    ///     solver.add_runner(entrant.clone()),
    ///     solver.add_runner(entrant),
    ///     solver.add_runner(innermost),
    /// ];
    /// let solver = solver.ready_runners();
    ///
    /// // Runners are ordered by gate once ready, but IDs still find them.
    /// let gate = |id| solver.runner(id).gate_roll;
    /// assert_eq!(ids.map(gate), [2, 3, 1]);
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Panics
    ///
    /// Panics if every gate is already taken (see [`RaceParameters::gates`]),
    /// or if her gate is set but isn't between 1 and [`RaceParameters::gates`],
    /// or another runner already has it.
    pub fn add_runner(&mut self, entrant: Entrant) -> RunnerId {
        if self.runners.len() >= self.state.params.gates as usize {
            panic!("Cannot add more runners than available gates");
        }

        let team = self.new_team();
        self.add_member(entrant, team)
    }

    /// Add a team of 1 to [`MAX_TEAM_SIZE`] runners to the race BEFORE it starts.
    ///
    /// Members are allies of each other and opponents of everyone else.
    ///
    /// Returns the members' IDs, in the same order as `members` (see
    /// [`RaceSolver::add_runner`]).
    ///
    /// # Examples
    ///
    /// Race a team of three, as in Team Trials or Champions Meeting, against a
    /// single rival. Skills that target opponents never affect allies:
    ///
    /// ```
    /// # use uma_tools::{Configuration, Course, RaceParametersData, RaceSolver};
    /// # use uma_tools::{SkillData, UmaData};
    /// # fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    /// # let parameters = RaceParametersData::from_json(
    /// #     r#"{
    /// #         "trackCondition": "Firm", "weather": "Sunny", "season": "Spring",
    /// #         "time": "Noon", "grade": "G1", "gates": 18, "courseId": "10913"
    /// #     }"#,
    /// # )?;
    /// # let uma = UmaData::from_json(
    /// #     r#"{
    /// #         "speed": 1500, "stamina": 800, "power": 1200, "guts": 700,
    /// #         "wisdom": 1200, "strategy": "Oikomi", "distanceAptitude": "S",
    /// #         "surfaceAptitude": "A", "strategyAptitude": "A",
    /// #         "motivation": "Great", "skills": [200331]
    /// #     }"#,
    /// # )?;
    /// # let (courses, rails) = ("data/course_data.json", "data/rails");
    /// # let course =
    /// #     Course::from_json_files(courses, rails, &parameters.course_id)?;
    /// # let all_skills = SkillData::from_json_file("data/skill_data.json")?;
    /// # let entrant = uma.to_entrant(&all_skills)?;
    /// let config = Configuration::default();
    /// let mut solver = RaceSolver::new(course, parameters.race, config);
    /// solver.add_team(vec![entrant.clone(); 3]);
    /// solver.add_runner(entrant);
    /// let solver = solver.ready_runners();
    ///
    /// let runners = solver.runners();
    /// for runner in runners {
    ///     let opponents = runners.iter().filter(|o| runner.is_opponent(o));
    ///     let expected = if runner.initial_state.team == 0 { 1 } else { 3 };
    ///     assert_eq!(opponents.count(), expected);
    /// }
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Panics
    ///
    /// Panics if the team has no members or more than [`MAX_TEAM_SIZE`], if
    /// there aren't enough free gates for all of them (see
    /// [`RaceParameters::gates`]), or if a member's gate can't be set (see
    /// [`RaceSolver::add_runner`]).
    pub fn add_team(&mut self, members: Vec<Entrant>) -> Vec<RunnerId> {
        if members.is_empty() || members.len() > MAX_TEAM_SIZE {
            panic!("Team must have between 1 and {MAX_TEAM_SIZE} runners");
        }
        // Check the whole team up front so that it is never added partially.
        if self.runners.len() + members.len() > self.state.params.gates as usize {
            panic!("Cannot add more runners than available gates");
        }

        let team = self.new_team();
        members
            .into_iter()
            .map(|member| self.add_member(member, team))
            .collect()
    }

    /// Allocate the next team index.
    fn new_team(&mut self) -> u8 {
        let team = self.state.team_count;
        self.state.team_count += 1;
        self.state.rushed_by_team.push([0; Style::COUNT]);
        self.state.team_activations.push(0);
        team
    }

    /// Add `entrant` as a member of `team`, returning her ID. Gate capacity
    /// must be checked by the caller.
    fn add_member(&mut self, entrant: Entrant, team: u8) -> RunnerId {
        if let Some(gate) = entrant.gate {
            let gates = self.state.params.gates;
            assert!(
                (1..=gates).contains(&gate),
                "Invalid gate {gate}: expected 1 to {gates}"
            );
            assert!(
                !self.runners.iter().any(|other| other.gate == Some(gate)),
                "Gate {gate} is already set for another runner"
            );
        }
        for skill in &entrant.skills {
            *self.state.existing_skills.entry(skill.id).or_insert(0) += 1;
        }
        self.state.runners_by_style[entrant.uma.style.base_style() as usize] += 1;

        let id = RunnerId(self.runners.len() as u8);
        self.runners.push(entrant);
        self.teams_by_id.push(team);
        id
    }

    /// Get all runners ready: assign their gates and moods (see
    /// [`Configuration`]), calculate their stats and activate out-of-gate
    /// skills.
    ///
    /// Returns a new [`RaceSolver`] with ready runners to run the simulation.
    ///
    /// # Examples
    ///
    /// Inspect the runners before the race, e.g. to see which skills failed
    /// their wit check and can't activate:
    ///
    /// ```
    /// # use uma_tools::{Configuration, Course, RaceParametersData, RaceSolver};
    /// # use uma_tools::{SkillData, UmaData};
    /// # fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    /// # let parameters = RaceParametersData::from_json(
    /// #     r#"{
    /// #         "trackCondition": "Firm", "weather": "Sunny", "season": "Spring",
    /// #         "time": "Noon", "grade": "G1", "gates": 18, "courseId": "10913"
    /// #     }"#,
    /// # )?;
    /// # let uma = UmaData::from_json(
    /// #     r#"{
    /// #         "speed": 1500, "stamina": 800, "power": 1200, "guts": 700,
    /// #         "wisdom": 1200, "strategy": "Oikomi", "distanceAptitude": "S",
    /// #         "surfaceAptitude": "A", "strategyAptitude": "A",
    /// #         "motivation": "Great", "skills": [200331]
    /// #     }"#,
    /// # )?;
    /// # let (courses, rails) = ("data/course_data.json", "data/rails");
    /// # let course =
    /// #     Course::from_json_files(courses, rails, &parameters.course_id)?;
    /// # let all_skills = SkillData::from_json_file("data/skill_data.json")?;
    /// # let entrant = uma.to_entrant(&all_skills)?;
    /// let config = Configuration::default();
    /// let mut solver = RaceSolver::new(course, parameters.race, config);
    /// solver.add_runner(entrant);
    /// let solver = solver.ready_runners();
    ///
    /// let runner = &solver.runners()[0];
    /// assert_eq!(runner.initial_state.popularity_rank, 1);
    /// for skill in runner.failed_skills() {
    ///     println!("Skill {} failed its wit check", skill.id);
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub fn ready_runners(self) -> RaceSolver<Runner> {
        let entrants = self.runners;
        let mut state = self.state;
        // Entrants are in the order they were added, i.e. by ID. Their random
        // streams derive from their IDs, so that they don't depend on skills.
        let seeds: Vec<_> = (0..entrants.len())
            .map(|index| state.seed.derive(Stream::Runner(index as u8)))
            .collect();
        let gates_by_id = assign_gates(&entrants, &seeds, &state);
        let moods = assign_moods(&entrants, &seeds, &state);
        let mut builders: Vec<_> = entrants
            .into_iter()
            .zip(seeds)
            .zip(&self.teams_by_id)
            .zip(&gates_by_id)
            .zip(moods)
            .map(|((((entrant, seed), &team), &gate), mood)| {
                let mut builder =
                    UnreadyRunner::new(entrant.uma, mood, &state, entrant.skills, seed);
                builder.initial_state.team = team;
                builder.assign_gate(gate, &state);
                builder
            })
            .collect();
        // Ready runners by gate, which settles ties (e.g. between popularity marks).
        builders.sort_by_key(|builder| builder.gate);

        // Calculate popularity for all runners.
        let popularity = calculate_popularity(&builders);

        // Assign popularity to each runner.
        for (builder, (rank, judge_scores)) in builders.iter_mut().zip(popularity) {
            builder.initial_state.popularity_rank = rank;
            builder.initial_state.judge_scores = judge_scores;
            if builder.initial_state.popularity_rank == 1 {
                state.style_most_popular = builder.base.style.base_style();
            }
        }

        // Apply out-of-gate skills to every runner first, as some of them (e.g.
        // "Connect") activate "Sympathy" for every runner.
        for builder in &mut builders {
            builder.apply_out_of_gate_skills(&state);
        }
        if builders.iter().any(|builder| builder.casts_sympathy) {
            for builder in &mut builders {
                builder.activate_sympathy();
            }
        }

        let runners = builders
            .into_iter()
            .map(|builder| Runner::new(builder, &state))
            .collect();
        let gates = state.params.gates as usize;
        RaceSolver {
            runners,
            teams_by_id: self.teams_by_id,
            gates_by_id,
            state,
            duel_close_times: vec![None; gates * gates],
            prev_duel_close_times: vec![None; gates * gates],
        }
    }
}

/// Gate of every entrant, in the same order, according to
/// [`Configuration::gate_assignment`] and [`Configuration::default_gate`].
///
/// Random gates are rolled in order, each runner from her own stream (derived
/// from her seed in `seeds`) until she gets a free one.
fn assign_gates(entrants: &[Entrant], seeds: &[RngSeed], state: &RaceState) -> Vec<u8> {
    let config = &state.config;
    let set_gate = |entrant: &Entrant| match config.gate_assignment {
        Assignment::Unset => entrant.gate,
        Assignment::All => None,
    };
    let mut taken = [false; MAX_GATES as usize + 1];
    for gate in entrants.iter().filter_map(set_gate) {
        taken[gate as usize] = true;
    }

    // Lowest gate that may still be free, for gates in order.
    let mut next_gate = 1;
    entrants
        .iter()
        .zip(seeds)
        .map(|(entrant, seed)| {
            let gate = match (set_gate(entrant), config.default_gate) {
                (Some(gate), _) => gate,
                (None, DefaultGate::InOrder) => {
                    while taken[next_gate as usize] {
                        next_gate += 1;
                    }
                    next_gate
                }
                (None, DefaultGate::Random) => {
                    let mut rng = seed.rng(Stream::Gate);
                    loop {
                        let gate = rng.random_range(1..=state.params.gates);
                        if !taken[gate as usize] {
                            break gate;
                        }
                    }
                }
            };
            taken[gate as usize] = true;
            gate
        })
        .collect()
}

/// Mood of every entrant, in the same order, according to
/// [`Configuration::mood_assignment`] and [`Configuration::default_mood`].
///
/// Random moods are rolled from each runner's own stream (derived from her seed
/// in `seeds`).
fn assign_moods(entrants: &[Entrant], seeds: &[RngSeed], state: &RaceState) -> Vec<Mood> {
    let config = &state.config;
    entrants
        .iter()
        .zip(seeds)
        .map(|(entrant, seed)| {
            let set_mood = match config.mood_assignment {
                Assignment::Unset => entrant.mood,
                Assignment::All => None,
            };
            set_mood.unwrap_or_else(|| match config.default_mood {
                DefaultMood::Fixed(mood) => mood,
                DefaultMood::Random => {
                    let mut rng = seed.rng(Stream::Mood);
                    let mood = rng.random_range(Mood::Awful as i8..=Mood::Great as i8);
                    Mood::try_from(mood).expect("moods are contiguous")
                }
            })
        })
        .collect()
}

/// Methods available AFTER runners are ready (stats have been calculated,
/// out-of-gate skills have been applied...).
impl RaceSolver<Runner> {
    /// The runner with ID `id`, wherever she is in [`Self::runners`].
    ///
    /// # Panics
    ///
    /// Panics if `id` isn't in this race.
    pub fn runner(&self, id: RunnerId) -> &Runner {
        let gate = self.gates_by_id.get(id.0 as usize);
        let gate = *gate.unwrap_or_else(|| panic!("No runner with {id:?} in this race"));
        self.runners
            .iter()
            .find(|runner| runner.gate_roll == gate)
            .expect("every ID has a runner")
    }

    /// Get the time as displayed on the scoreboard after a race finishes.
    ///
    /// Simulated times are scaled to match displayed ones, and clamped to the
    /// course's range (see [`Course::finish_time_min`]), so this is the time
    /// to compare with in-game results.
    ///
    /// # Examples
    ///
    /// ```
    /// # use uma_tools::{Configuration, Course, RaceParametersData, RaceSolver};
    /// # use uma_tools::{SkillData, UmaData};
    /// # fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    /// # let parameters = RaceParametersData::from_json(
    /// #     r#"{
    /// #         "trackCondition": "Firm", "weather": "Sunny", "season": "Spring",
    /// #         "time": "Noon", "grade": "G1", "gates": 18, "courseId": "10913"
    /// #     }"#,
    /// # )?;
    /// # let uma = UmaData::from_json(
    /// #     r#"{
    /// #         "speed": 1500, "stamina": 800, "power": 1200, "guts": 700,
    /// #         "wisdom": 1200, "strategy": "Oikomi", "distanceAptitude": "S",
    /// #         "surfaceAptitude": "A", "strategyAptitude": "A",
    /// #         "motivation": "Great", "skills": [200331]
    /// #     }"#,
    /// # )?;
    /// # let (courses, rails) = ("data/course_data.json", "data/rails");
    /// # let course =
    /// #     Course::from_json_files(courses, rails, &parameters.course_id)?;
    /// # let all_skills = SkillData::from_json_file("data/skill_data.json")?;
    /// # let entrant = uma.to_entrant(&all_skills)?;
    /// let config = Configuration::default();
    /// let mut solver = RaceSolver::new(course, parameters.race, config);
    /// solver.add_runner(entrant);
    /// let mut solver = solver.ready_runners();
    /// solver.run();
    ///
    /// if let (Some(simulated), Some(displayed)) =
    ///     (solver.state.finish_time, solver.scoreboard_time())
    /// {
    ///     println!("Simulated {simulated:.2}s, displayed {displayed:.2}s");
    ///     assert!(displayed > simulated);
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub fn scoreboard_time(&self) -> Option<f64> {
        let converted = self.state.finish_time? * FINISH_TIME_SCALE;
        let min = self.state.course.finish_time_min;
        let max = self.state.course.finish_time_max;
        let clamped = converted.clamp(min, max);
        if !(min..=max).contains(&converted) {
            // Add random offset between 0.0 and 1.0 seconds.
            let offset = self
                .state
                .seed
                .rng(Stream::Scoreboard)
                .random_range(0.0..1.0);
            Some(clamped + offset)
        } else {
            Some(converted)
        }
    }

    /// Run simulation until every runner finishes, then record finish margins.
    pub fn run(&mut self) {
        while !self.step_frame() {}
    }

    /// Advance the race by one frame ([`DELTA_TIME`]), e.g. to observe it
    /// frame by frame, unless every runner has already finished.
    ///
    /// Returns whether every runner has finished. Finish margins are recorded
    /// on the frame the last runner finishes, as [`Self::run`] does.
    ///
    /// # Examples
    ///
    /// Check who leads halfway through the race, which [`Self::run`] can't
    /// tell, as it only leaves the final results:
    ///
    /// ```
    /// use uma_tools::{
    ///     Configuration, Course, RaceParametersData, RaceSolver, SkillData,
    ///     UmaData,
    /// };
    ///
    /// # fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    /// # let parameters = RaceParametersData::from_json(
    /// #     r#"{
    /// #         "trackCondition": "Firm", "weather": "Sunny", "season": "Spring",
    /// #         "time": "Noon", "grade": "G1", "gates": 18, "courseId": "10913"
    /// #     }"#,
    /// # )?;
    /// # let (courses, rails) = ("data/course_data.json", "data/rails");
    /// # let course_id = &parameters.course_id;
    /// # let course = Course::from_json_files(courses, rails, course_id)?;
    /// # let all_skills = SkillData::from_json_file("data/skill_data.json")?;
    /// // Same stats, but a front runner and an end closer.
    /// let uma = |strategy| {
    ///     let json = format!(
    ///         r#"{{
    ///             "speed": 1500, "stamina": 800, "power": 1200, "guts": 700,
    ///             "wisdom": 1200, "strategy": "{strategy}",
    ///             "distanceAptitude": "S", "surfaceAptitude": "A",
    ///             "strategyAptitude": "A", "skills": []
    ///         }}"#
    ///     );
    ///     UmaData::from_json(&json)
    /// };
    /// let halfway = course.halfway();
    /// let config = Configuration {
    ///     seed: Some(42),
    ///     ..Configuration::default()
    /// };
    /// let mut solver = RaceSolver::new(course, parameters.race, config);
    /// let front = solver.add_runner(uma("Nige")?.to_entrant(&all_skills)?);
    /// let closer = solver.add_runner(uma("Oikomi")?.to_entrant(&all_skills)?);
    /// let mut solver = solver.ready_runners();
    ///
    /// // Runners are ordered by position after every frame, leader first.
    /// while solver.runners()[0].position < halfway {
    ///     solver.step_frame();
    /// }
    /// assert!(solver.runner(front).position > solver.runner(closer).position);
    ///
    /// // The rest of the race can still be run at once.
    /// solver.run();
    /// # Ok(())
    /// # }
    /// ```
    pub fn step_frame(&mut self) -> bool {
        if self.runners.iter().all(Runner::has_finished) {
            return true;
        }
        self.step(DELTA_TIME);
        let finished = self.runners.iter().all(Runner::has_finished);
        if finished {
            self.update_finish_margins();
        }
        finished
    }

    /// Set each runner's margin behind the runner that placed immediately ahead.
    ///
    /// Margin distance is calculated by scaling the difference in *displayed*
    /// finish times between runners by a set multiplier (15 m/s). All runners must
    /// have finished before calling this function.
    fn update_finish_margins(&mut self) {
        // Order must come from finish times.
        let mut by_finish: Vec<&mut Runner> = self.runners.iter_mut().collect();
        by_finish.sort_by(|a, b| a.finish_time.unwrap().total_cmp(&b.finish_time.unwrap()));

        let mut prev_time = None;
        for runner in by_finish {
            let time = runner.finish_time.expect("all runners must have finished");
            runner.finish_margin =
                prev_time.map(|prev| (time - prev) * FINISH_TIME_SCALE * MARGIN_SPEED);
            prev_time = Some(time);
        }
    }

    /// Perform a single simulation step.
    ///
    /// Each step has two main stages: skill activation and state update.
    ///
    /// For **skills**, every runner that moves during this step (i.e. leaves the
    /// gate by its end, so that her first frame of movement is checked too)
    /// [checks her skills](Runner::check_skills) before time advances. Nothing is
    /// updated in between, so every check sees the same state: the one reached at
    /// the end of the previous step, including multi-runner values (order,
    /// blocking...).
    ///
    /// Once every runner has checked her skills, the buff (advantage) skill
    /// types activated this frame are counted, along with the activations of the
    /// most popular runner, and the `is_other_character_activate_advantage_skill`
    /// (and `is_popularity_top_character_activate_advantage_skill`) triggers of
    /// every runner are checked, so that the result doesn't depend on which
    /// runner checked first. Then, effects targeting other runners are applied.
    ///
    /// After that, the **state is updated**: time advances, then each runner moves
    /// and updates everything derived from her position (phase, corners, straights,
    /// slopes), her lane and HP, and her active effects. Multi-runner values are
    /// updated last.
    ///
    /// Events detected during the update (e.g. entering a straight) are stored
    /// and handled by the next skill check. Update functions never check skills
    /// themselves; at most, they schedule new windows (e.g.
    /// `run_at_full_speed_random` segments, rolled when the all-out spurt starts
    /// during the target speed update).
    fn step(&mut self, delta_time: f64) {
        self.update_team_activations();
        self.state.buffs_activated.clear();
        self.state.popularity_top_activations = SkillActivationFlags::default();
        for runner in &mut self.runners {
            // Runners that stay in the gate during this step haven't reached any state
            // worth checking, and skills can't activate after the goal.
            if !runner.can_activate_skills(self.state.elapsed_time + delta_time) {
                continue;
            }
            runner.check_skills(&mut self.state);
            for skill_type in runner.activation_flags.buff_activated.iter() {
                *self.state.buffs_activated.entry(skill_type).or_insert(0) += 1;
            }
            if runner.initial_state.popularity_rank == 1 {
                self.state.popularity_top_activations = runner.activation_flags;
            }
        }
        self.check_advantage_triggers(delta_time);
        self.apply_pending_effects();

        self.state.elapsed_time += delta_time;

        for runner in &mut self.runners {
            if runner.start_delay > self.state.elapsed_time {
                continue;
            }
            runner.update_rush_state(&mut self.state);
            runner.update_target_speed(&self.state);
            runner.update_acceleration(self.state.elapsed_time);
            runner.update_speed(delta_time, self.state.course.base_speed());
            runner.update_position(delta_time, &self.state);
            if runner.has_finished() && self.state.finish_time.is_none() {
                self.state.finish_time = Some(self.state.elapsed_time);
            }

            // Update everything derived from the new position, so that skills see a
            // consistent state.
            runner.update_phase(&mut self.state);
            let transitions = runner.segments.update(runner.position, &self.state.course);
            runner.update_final_extra_move_lane(
                &transitions,
                &self.state.course,
                self.state.config.game_version,
            );
            runner.update_slope(&self.state.course.slopes, &transitions.slope, delta_time);

            runner.update_lane(&self.state.course, delta_time);
            runner.update_hp(delta_time, &self.state);
            runner.update_charge_up(delta_time, self.state.elapsed_time);
            runner.update_skill_durations(self.state.elapsed_time);
        }

        self.runners
            .sort_by(|a, b| b.position.total_cmp(&a.position));

        self.update_order_values();
        self.resolve_overlaps();
        self.update_blocking_status(delta_time);
        self.update_spot_struggle();
        self.update_duels(delta_time);
        self.update_positioning_strategy();
        self.update_secure_lead();
        self.update_compete_before_spurt();
    }

    /// Check `is_other_character_activate_advantage_skill` and
    /// `is_popularity_top_character_activate_advantage_skill` triggers of every
    /// runner, now that this frame's activations are known.
    ///
    /// Skills activated here aren't counted as advantage skills for other runners.
    fn check_advantage_triggers(&mut self, delta_time: f64) {
        // ? Assume activations don't chain within a frame.
        // Any activation of the most popular runner counts (see
        // `is_popularity_top_character_activate_advantage_skill==-1`).
        if self.state.buffs_activated.is_empty()
            && !self.state.popularity_top_activations.any_activated
        {
            return;
        }
        for runner in &mut self.runners {
            if runner.can_activate_skills(self.state.elapsed_time + delta_time) {
                runner.check_advantage_triggers(&self.state);
            }
        }
    }

    /// Apply skill effects that target other runners to their targets.
    ///
    /// Targets are selected from the state reached at the end of the previous step,
    /// so the result doesn't depend on which runner activated her skills first.
    ///
    /// Runners must be sorted by position in descending order before calling this.
    fn apply_pending_effects(&mut self) {
        // ? Assume received effects never activate further skills.
        for caster in 0..self.runners.len() {
            let pending = std::mem::take(&mut self.runners[caster].pending_effects);
            for pending_effect in &pending {
                let target = pending_effect.effect.target;
                for target in self.select_targets(caster, target, pending_effect.target_roll) {
                    self.runners[target].receive_skill_effect(pending_effect, &self.state);
                }
            }
        }
    }

    /// Select the indices of the runners targeted by a skill effect casted by the
    /// runner at index `caster`.
    ///
    /// Allies count towards the maximum number of targets, but they are excluded
    /// from debuffs afterwards. Runners that have finished are never targeted.
    ///
    /// `target_roll` (from 0 to 1) picks random targets, like the one drawn for
    /// each pending effect.
    ///
    /// Runners must be sorted by position in descending order before calling this.
    fn select_targets(&self, caster: usize, target: SkillTarget, target_roll: f64) -> Vec<usize> {
        let runners = &self.runners;
        let source = &runners[caster];
        // Finished runners are skipped before counting targets. Nearest runners first.
        // Debuff targets may include the caster and her allies at first, but they're
        // dropped at the end.
        let is_running = |i: &usize| !runners[*i].has_finished();
        let running = || (0..runners.len()).filter(is_running);
        let ahead = (0..caster).rev().filter(is_running);
        let behind = (caster + 1..runners.len()).filter(is_running);
        let is_rushed = |i: &usize| runners[*i].is_rushed();

        let selected: Vec<usize> = match target {
            SkillTarget::Oneself => vec![caster],
            SkillTarget::All => running().collect(),
            // ? Assume field of view only covers runners ahead, as for visible counts.
            SkillTarget::InFov { max } => ahead
                .filter(|&i| source.is_visible(&runners[i]))
                .take(max as usize)
                .collect(),
            // Index `position - 1` is the runner placed at `position`.
            SkillTarget::AheadOfPosition { position } => {
                let places = (position.saturating_sub(1) as usize).min(runners.len());
                (0..places).filter(is_running).collect()
            }
            SkillTarget::AheadOfSelf { count } => ahead.take(count as usize).collect(),
            SkillTarget::BehindSelf { count } => behind.take(count as usize).collect(),
            SkillTarget::AllAllies => running()
                .filter(|&i| i != caster && !source.is_opponent(&runners[i]))
                .collect(),
            SkillTarget::EnemyStrategy { style } => running()
                .filter(|&i| runners[i].base_style() == style)
                .collect(),
            SkillTarget::RushedAhead { max } => {
                ahead.filter(is_rushed).take(max as usize).collect()
            }
            SkillTarget::RushedBehind { max } => {
                behind.filter(is_rushed).take(max as usize).collect()
            }
            SkillTarget::RushedStrategy { style } => running()
                .filter(|&i| is_rushed(&i) && runners[i].base_style() == style)
                .collect(),
            // Runners that activated a recovery skill this frame, matching
            // `is_other_character_activate_advantage_skill==9`.
            SkillTarget::UsedRecovery { .. } => running()
                .filter(|&i| {
                    runners[i]
                        .activation_flags
                        .buff_activated
                        .contains(SkillType::Recovery)
                })
                .collect(),
            // Not used by any skill.
            SkillTarget::UmaId { .. } => Vec::new(),
            SkillTarget::RandomAlly => {
                let team = || running().filter(|&i| !source.is_opponent(&runners[i]));
                let team_size = team().count();
                let pick = (target_roll * team_size as f64) as usize;
                team().nth(pick.min(team_size - 1)).into_iter().collect()
            }
        };

        selected
            .into_iter()
            .filter(|&i| !target.is_opponent_only() || source.is_opponent(&runners[i]))
            .collect()
    }

    /// Recount skill activations by team from each runner's own counts.
    fn update_team_activations(&mut self) {
        let team_activations = &mut self.state.team_activations;
        team_activations.fill(0);
        for runner in &self.runners {
            let team = runner.initial_state.team as usize;
            team_activations[team] += runner.trigger_scheduler.activation_counts.all;
        }
    }

    /// Update order (rank) and distance differences for all runners.
    ///
    /// Runners must be sorted by position in descending order before calling this.
    fn update_order_values(&mut self) {
        // Debug: check sorting.
        self.assert_runners_sorted();

        // Every gate counts, even if fewer runners are simulated.
        let total_runners = self.state.params.gates;
        let position_first = self.runners.first().unwrap().position;
        let position_last = self.runners.last().unwrap().position;
        let position_range = position_first - position_last;
        // Nobody ahead of the leader or behind the last runner counts as an infinite
        // gap, so that skills meant to break away from the pack (`bashin_diff_*<=1`)
        // don't fire for them.
        // ? Might require verification in game.
        let mut position_next = f64::INFINITY;
        // Rushed runners counted so far, in total and by team.
        let mut rushed = 0;
        // There can't be more teams than gates.
        let mut rushed_by_team = [0; MAX_GATES as usize];

        // From first to last, update order and distance differences from runners ahead.
        for (index, runner) in self.runners.iter_mut().enumerate() {
            if runner.has_finished() {
                continue;
            }
            let team = runner.initial_state.team as usize;
            runner.update_order((index + 1) as u8, &self.state.course);
            runner.update_order_rate(total_runners, self.state.elapsed_time);
            runner.update_diff_from_ahead(position_first, position_next, position_range);
            runner.rush_counter.ahead = rushed;
            runner.rush_counter.opponents_ahead = rushed - rushed_by_team[team];
            position_next = runner.position;
            if runner.is_rushed() {
                rushed += 1;
                rushed_by_team[team] += 1;
            }
        }

        // From last to first, update distance differences from runners behind.
        let mut position_prev = f64::NEG_INFINITY;
        rushed = 0;
        rushed_by_team.fill(0);
        for runner in self.runners.iter_mut().rev() {
            let team = runner.initial_state.team as usize;
            runner.update_diff_from_behind(position_prev);
            runner.rush_counter.behind = rushed;
            runner.rush_counter.opponents_behind = rushed - rushed_by_team[team];
            position_prev = runner.position;
            if runner.is_rushed() {
                rushed += 1;
                rushed_by_team[team] += 1;
            }
        }
    }

    /// Update blocking and overlapping status for all runners.
    ///
    /// Runners must be sorted by position in descending order before calling this.
    fn update_blocking_status(&mut self, delta_time: f64) {
        // Debug: check sorting.
        self.assert_runners_sorted();

        for runner in self.runners.iter_mut() {
            runner.reset_blocking_status();
            runner.reset_overtake_checks();
        }

        let game_version = self.state.config.game_version;
        let num_runners = self.runners.len();
        // Furthest distance at which any check can apply: the longest field of
        // view, or the range of the proximity checks.
        let max_distance = self
            .runners
            .iter()
            .map(Runner::visible_distance)
            .fold(5.0, f64::max);
        for i in 0..num_runners {
            let (left, right) = self.runners.split_at_mut(i + 1);
            let (ahead, runner_i) = left.split_at_mut(i);
            let runner_i = &mut runner_i[0];
            let mut was_runner_behind_checked = false;

            for runner_j in right {
                // Only check runner immediately behind.
                if !was_runner_behind_checked && runner_j.lane.current < runner_i.lane.current {
                    runner_i.nearby.behind_in = true;
                }
                was_runner_behind_checked = true;

                let distance = runner_i.position - runner_j.position;
                let lane_diff = runner_i.lane.current - runner_j.lane.current;

                if distance > max_distance {
                    // We have gone too far behind; move to next i.
                    break;
                }

                if runner_j.is_visible(runner_i) {
                    runner_j.visible_count += 1;
                    // Check if runner_i is an overtake target for runner_j.
                    if runner_j.check_overtake_target(runner_i, delta_time) {
                        runner_i.being_overtake_target_time += delta_time;
                    }
                }

                if distance > 5.0 {
                    // Too far for the following checks.
                    continue;
                }

                // Update surrounding and blocking status.
                if proximity::is_front_surrounded(distance, lane_diff) {
                    runner_j.update_front_surrounded_by(runner_i);
                }
                if proximity::is_side_surrounded(distance, lane_diff) {
                    runner_j.update_side_surrounded_by(runner_i);
                }
                if proximity::is_near(distance, lane_diff, game_version) {
                    runner_j.update_near_by(runner_i);
                }
                if proximity::is_near_same_lane(distance, lane_diff) {
                    runner_j.update_near_same_lane_by(runner_i);
                }
                if proximity::is_near_behind_set1(distance, lane_diff) {
                    runner_i.nearby.behind_set1_count += 1;
                }
                if proximity::check_front_blocked(distance, lane_diff) {
                    runner_j.update_front_blocked_by(runner_i);
                }
                if proximity::check_side_blocked(distance, lane_diff) {
                    runner_j.update_side_blocked_by(runner_i, lane_diff);
                }
            }

            // Checked all runners behind i within a reasonable distance, and
            // runner_i was checked against all runners ahead before. Her block and
            // near statuses can be updated now.
            runner_i.update_blocked_time(delta_time);
            runner_i.update_near_time(delta_time);
            if let Some(gate_roll) = runner_i.track_front_blocker_target(delta_time)
                && let Some(blocker) = ahead.iter_mut().find(|r| r.gate_roll == gate_roll)
            {
                blocker.being_overtake_target_time += delta_time;
            }
            runner_i.finalize_overtake_state(delta_time, self.state.elapsed_time);
        }

        // Calculate lane targets.
        for i in 0..num_runners {
            // Borrow all runners along with a single runner in an immutable state first.
            let all_runners = &self.runners;
            let Some(decision) =
                all_runners[i].target_lane_decision(&self.state.course, all_runners)
            else {
                continue;
            };

            // Shared borrow of all runners ends here; result can be written back.
            let runner = &mut self.runners[i];
            runner.lane.target = decision.target;
            runner.extra_move_lane = decision.extra_move_lane;
        }
    }

    /// Bump the outer runner of each overlapping pair to `OVERLAP_LANE_GAP` horse
    /// lanes away from the inner one, and force her to update her target lane.
    ///
    /// Runners must be sorted by position in descending order before calling this.
    fn resolve_overlaps(&mut self) {
        // Debug: check sorting.
        self.assert_runners_sorted();

        let course_width = self.state.course.width;
        let num_runners = self.runners.len();
        // ? What if a bump causes a new overlap with a third runner?
        // ? For now, resolve all overlaps on the same frame.
        loop {
            let mut bumps = Vec::new();
            for i in 0..num_runners {
                let ahead = &self.runners[i];
                for (j, behind) in self.runners.iter().enumerate().skip(i + 1) {
                    let distance = ahead.position - behind.position;
                    if distance >= proximity::OVERLAP_DISTANCE {
                        // Too far behind; move to next i.
                        break;
                    }
                    let lane_diff = ahead.lane.current - behind.lane.current;
                    if !proximity::is_overlapping(distance, lane_diff) {
                        continue;
                    }

                    // ? Assume the runner behind is bumped if both are in the same line.
                    let (inner_lane, outer) = if lane_diff > CourseWidths(0.0) {
                        (behind.lane.current, i)
                    } else {
                        (ahead.lane.current, j)
                    };
                    let bumped_lane =
                        inner_lane + CourseWidths::from_horse_lanes(proximity::OVERLAP_LANE_GAP);
                    bumps.push((outer, bumped_lane.min(course_width)));
                }
            }

            let mut moved = false;
            for (outer, bumped_lane) in bumps {
                let runner = &mut self.runners[outer];
                // A runner overlapping several others ends up away from the outermost.
                // Nobody moves if already at the outer edge, which ends the loop.
                if bumped_lane > runner.lane.current {
                    runner.lane.current = bumped_lane;
                    moved = true;
                }
                // Keep the new lane unless the target lane update decides otherwise;
                // the old target may lead right back into the other runner. Being on
                // the target lane also forces that update, even if side-blocked.
                runner.lane.target = runner.lane.current;
            }
            if !moved {
                return;
            }
        }
    }

    /// Update positioning strategy for all runners. Runners must be sorted by
    /// position in descending order before calling this method.
    fn update_positioning_strategy(&mut self) {
        // Debug: check sorting.
        self.assert_runners_sorted();
        // A single runner has nobody to keep pace with, so she always runs at
        // regular pace. Otherwise she'd be her own pacemaker, and runners other
        // than front runners would pace down for the whole positioning window.
        if self.runners.len() == 1 {
            return;
        }

        // Front runners for the purpose of Speed Up mode distances: Runaways and
        // runners acting as front runners because they're rushed don't count.
        let front_runner_count = self
            .runners
            .iter()
            .filter(|runner| runner.adjusted.style == Style::Front)
            .count();

        // Find pacemaker.
        let mut pacemaker_position = self.runners[0].position;
        // After the 1.5 anniv. update, the pacemaker *seems* to be the most forward
        // runner overall. More parameters may be involved, but they are unknown.
        // In the meantime this is a reasonable approximation that works for most cases.
        if self.state.config.game_version < GameVersion::OnePointFiveAnniversary {
            // Prior to the 1.5 anniversary update, the pacemaker was the most forward
            // runner with the most aggressive style.
            let mut pacemaker_style = self.runners[0].positioning_style();
            for i in 0..self.runners.len() {
                let runner_i = &self.runners[i];
                let style = runner_i.positioning_style();
                if style < pacemaker_style {
                    pacemaker_style = style;
                    pacemaker_position = runner_i.position;
                }
                // Because runners are sorted by position descendingly, we only need to update
                // pacemaker when a more agressive style is found.
            }
        }

        let has_pace_up_ex = self.state.config.game_version >= GameVersion::OnePointFiveAnniversary;
        let pacemaker_style = self.runners[0].positioning_style();
        // Most passive style among runners ahead of the current one. Runaways
        // count as front runners, so neither should be behind the other.
        let mut max_style_ahead: Option<Style> = None;

        for i in 0..self.runners.len() {
            let style = self.runners[i].positioning_style();
            let base_style = style.base_style();
            let passive_ahead = max_style_ahead > Some(base_style);
            max_style_ahead = max_style_ahead.max(Some(base_style));
            if self.runners[i].section >= POSITIONING_END_SECTION {
                self.runners[i].positioning_strategy = PositioningStrategy::Normal;
                continue;
            }
            if has_pace_up_ex
                && self.runners[i].update_pace_up_ex(passive_ahead, pacemaker_style, &self.state)
            {
                continue;
            }

            let runner = &self.runners[i];
            if style.is_front_like() {
                // Lead over the runner behind if first overall (or alone).
                let lead = (i == 0).then(|| {
                    self.runners
                        .get(1)
                        .map_or(f64::INFINITY, |next| runner.position - next.position)
                });
                // Lead over the next runner with the same strategy if first among them.
                // Runaways count as front runners here too.
                let same_strategy =
                    |other: &Runner| other.positioning_style().base_style() == base_style;
                let is_first_in_strategy = !self.runners[..i].iter().any(same_strategy);
                let strategy_lead = is_first_in_strategy.then(|| {
                    self.runners[i + 1..]
                        .iter()
                        .find(|other| same_strategy(other))
                        .map_or(f64::INFINITY, |next| runner.position - next.position)
                });
                let runner = &mut self.runners[i];
                runner.update_positioning_front(
                    lead,
                    strategy_lead,
                    front_runner_count == 1,
                    &self.state,
                )
            } else {
                // Determine distance behind pacemaker.
                let distance_behind = pacemaker_position - runner.position;
                let runner = &mut self.runners[i];
                runner.update_positioning_non_front(distance_behind, &self.state)
            }
        }
    }

    /// Update secure lead (リード確保) state for all runners.
    fn update_secure_lead(&mut self) {
        let elapsed_time = self.state.elapsed_time;
        let course_distance = self.state.course.distance as f64;

        for i in 0..self.runners.len() {
            self.runners[i].secure_lead.expire(elapsed_time);
            if !self.runners[i].is_secure_lead_check_due(&self.state) {
                continue;
            }

            let runner = &self.runners[i];
            let style = runner.adjusted.style;
            let lead_insufficient = self.runners.iter().any(|other| {
                secure_lead::desirable_lead(style, other.adjusted.style, course_distance)
                    .is_some_and(|desirable| runner.position - other.position < desirable)
            });
            let is_alone = self.is_alone(i, 10.0);

            self.runners[i].try_secure_lead(lead_insufficient, is_alone, &self.state);
        }
    }

    /// Update spot struggle (位置取り争い) state for all runners.
    ///
    /// Runners must be sorted by position in descending order before calling this.
    fn update_spot_struggle(&mut self) {
        if self.state.config.game_version < GameVersion::FirstAnniversary {
            return;
        }
        for style in spot_struggle::STYLES {
            let has_struggled = self
                .runners
                .iter()
                .any(|r| r.adjusted.style == style && r.spot_struggle.has_struggled());
            if has_struggled {
                self.exit_spot_struggle(style);
            } else {
                self.try_start_spot_struggle(style);
            }
        }
    }

    /// Start a spot struggle between the frontmost runner with `style` and every
    /// runner with the same style close behind her, if any.
    fn try_start_spot_struggle(&mut self, style: Style) {
        let Some(leader_index) = self.runners.iter().position(|r| r.adjusted.style == style) else {
            return;
        };
        // Any runner past the minimum position will do; the first one is the furthest.
        if self.runners[0].position < spot_struggle::MIN_POSITION {
            return;
        }
        let leader = &self.runners[leader_index];
        // Can't start if a runner whose strategy should be behind is ahead.
        if self.runners[..leader_index]
            .iter()
            .any(|r| r.adjusted.style > style)
        {
            return;
        }

        let leader_in_sections = spot_struggle::SECTIONS.contains(&leader.section);
        let members: Vec<usize> = (leader_index + 1..self.runners.len())
            .filter(|&j| {
                let other = &self.runners[j];
                other.adjusted.style == style
                    && (leader_in_sections || spot_struggle::SECTIONS.contains(&other.section))
                    && spot_struggle::is_close_to_leader(
                        leader.position - other.position,
                        leader.lane.current - other.lane.current,
                    )
            })
            .collect();
        if members.is_empty() {
            return;
        }

        let elapsed_time = self.state.elapsed_time;
        self.runners[leader_index].start_spot_struggle(elapsed_time);
        for j in members {
            self.runners[j].start_spot_struggle(elapsed_time);
        }
    }

    /// Make strugglers with `style` exit the spot struggle if they're too far from
    /// every other struggler.
    ///
    /// Strugglers whose effects have expired still count (and can exit too), but
    /// those who have already exited don't. Exits cascade within the same frame,
    /// e.g. the last remaining struggler exits right away.
    fn exit_spot_struggle(&mut self, style: Style) {
        loop {
            let strugglers: Vec<usize> = (0..self.runners.len())
                .filter(|&i| {
                    let runner = &self.runners[i];
                    runner.adjusted.style == style && runner.spot_struggle.is_struggler()
                })
                .collect();
            // Decide every exit before applying them, so that the order doesn't matter.
            let exiting: Vec<usize> = strugglers
                .iter()
                .copied()
                .filter(|&i| {
                    let runner = &self.runners[i];
                    strugglers.iter().all(|&j| {
                        let other = &self.runners[j];
                        i == j
                            || spot_struggle::is_too_far(
                                runner.position - other.position,
                                runner.lane.current - other.lane.current,
                            )
                    })
                })
                .collect();
            if exiting.is_empty() {
                return;
            }
            for i in exiting {
                self.runners[i].spot_struggle.stop();
            }
        }
    }

    /// Update duel (追い比べ) state for all runners.
    ///
    /// Runners must be sorted by position in descending order before calling this.
    fn update_duels(&mut self, delta_time: f64) {
        if self.state.config.game_version < GameVersion::FirstAnniversary {
            return;
        }
        self.exit_duels();
        self.try_start_duels(delta_time);
    }

    /// Make duelists exit if their HP runs low or they're separated from every
    /// other runner who has dueled, ahead or behind.
    ///
    /// Runners who already exited still count, so exits never cascade.
    fn exit_duels(&mut self) {
        let exiting: Vec<usize> = (0..self.runners.len())
            .filter(|&i| {
                let runner = &self.runners[i];
                runner.duel.is_dueling()
                    && (runner.hp.percent() < duel::EXIT_HP_PERCENT
                        || self.runners.iter().enumerate().all(|(j, other)| {
                            i == j
                                || !other.duel.has_dueled()
                                || duel::is_separated(runner.position - other.position)
                        }))
            })
            .collect();
        for i in exiting {
            self.runners[i].duel.stop();
        }
    }

    /// Track how long each pair of runners has been close, and start a duel
    /// between those close for long enough that also meet the other conditions.
    ///
    /// Runners who already exited a duel can't start another one, nor be dueled.
    ///
    /// Runners must be sorted by position in descending order before calling this.
    fn try_start_duels(&mut self, delta_time: f64) {
        // Pairs no longer close aren't written again, which resets their time.
        std::mem::swap(&mut self.duel_close_times, &mut self.prev_duel_close_times);
        self.duel_close_times.fill(None);
        let mut starting = Vec::new();
        for i in 0..self.runners.len() {
            let ahead = &self.runners[i];
            if ahead.duel.has_ended() {
                continue;
            }
            for (j, behind) in self.runners.iter().enumerate().skip(i + 1) {
                let distance = ahead.position - behind.position;
                if distance >= duel::TRIGGER_DISTANCE {
                    // Too far behind; move to next i.
                    break;
                }
                let lane_diff = ahead.lane.current - behind.lane.current;
                if behind.duel.has_ended() || !duel::is_close(distance, lane_diff) {
                    continue;
                }

                let pair = self.duel_pair_index(ahead.gate_roll, behind.gate_roll);
                // Time starts counting from the first frame they're close.
                let close_time =
                    self.prev_duel_close_times[pair].map_or(0.0, |time| time + delta_time);
                self.duel_close_times[pair] = Some(close_time);
                if duel::is_close_long_enough(close_time) && self.can_start_duel(i, j) {
                    starting.extend([i, j]);
                }
            }
        }
        for i in starting {
            self.runners[i].start_duel();
        }
    }

    /// Index of the pair of runners with gates `a` and `b` in the duel close
    /// times, the same in either order.
    fn duel_pair_index(&self, a: u8, b: u8) -> usize {
        let gates = self.state.params.gates as usize;
        let (low, high) = (a.min(b) as usize, a.max(b) as usize);
        (low - 1) * gates + (high - 1)
    }

    /// Check if runners `i` and `j`, already close for long enough, meet the
    /// remaining conditions to start dueling. These only need to hold for a single
    /// frame.
    fn can_start_duel(&self, i: usize, j: usize) -> bool {
        let course = &self.state.course;
        let can_duel = |runner: &Runner| {
            !runner.has_finished()
                && runner.segments.is_on_final_straight(course)
                && runner.hp.percent() >= duel::MIN_START_HP_PERCENT
        };
        let (a, b) = (&self.runners[i], &self.runners[j]);
        can_duel(a)
            && can_duel(b)
            && duel::are_speeds_similar(a.speed, b.speed)
            && (a.order.rate <= duel::MAX_ORDER_RATE || b.order.rate <= duel::MAX_ORDER_RATE)
    }

    /// Update compete before spurt (位置取り調整) state for all runners.
    fn update_compete_before_spurt(&mut self) {
        let elapsed_time = self.state.elapsed_time;
        let course_distance = self.state.course.distance as f64;

        for i in 0..self.runners.len() {
            self.runners[i].compete_before_spurt.expire(elapsed_time);
            if !self.runners[i].is_compete_check_due(&self.state) {
                continue;
            }

            let runner = &self.runners[i];
            let threshold =
                compete_before_spurt::lead_threshold(runner.adjusted.style, course_distance);
            let far_from_lead = runner.position_diff.from_first > threshold;
            let is_alone = self.is_alone(i, compete_before_spurt::ALONE_DISTANCE);

            self.runners[i].try_compete_before_spurt(far_from_lead, is_alone, &self.state);
        }
    }

    /// Check if there's no other runner with the same style as runner `i` within
    /// `distance`.
    fn is_alone(&self, i: usize, distance: f64) -> bool {
        let runner = &self.runners[i];
        self.runners.iter().enumerate().all(|(j, other)| {
            i == j
                || other.adjusted.style != runner.adjusted.style
                || (runner.position - other.position).abs() > distance
        })
    }

    fn assert_runners_sorted(&self) {
        #[cfg(debug_assertions)]
        for i in 1..self.runners.len() {
            debug_assert!(self.runners[i - 1].position >= self.runners[i].position);
        }
    }
}

#[cfg(test)]
mod tests;
