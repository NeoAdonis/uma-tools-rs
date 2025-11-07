use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::marker::PhantomData;
use std::rc::Rc;
use std::vec;

use rand::prelude::*;

use crate::course::Course;
use crate::race::parameters::RaceParameters;
use crate::race::runner::{GAME_BEHAVIOR, GameBehavior, Runner};
use crate::race::unready_runner::UnreadyRunner;
use crate::uma::skill::*;
use crate::uma::*;

pub struct RaceState {
    pub course: Course,
    pub params: RaceParameters,
    /// Random number generator shared by all runners.
    /// It is initialized with a seed to ensure reproducibility.
    pub rng: Rc<RefCell<rand_pcg::Pcg64Mcg>>,
    /// Elapsed time in seconds. This value is shared by all runners.
    pub elapsed_time: f64,
    /// Time when the first runner finished, in seconds. This might be different from the
    /// displayed finish time due to scoreboard conversion.
    pub finish_time: Option<f64>,
    /// Count of runners by style.
    pub runners_by_style: HashMap<Style, u8>,
    /// Count of runners currently rushed by style.
    pub rushed_by_style: HashMap<Style, u8>,
    /// Set of existing skill IDs among all runners.
    pub existing_skills: HashSet<String>,
}

/// Race solver that manages the race simulation.
///
/// It is managed in two phases:
/// 
/// 1. Before runners are ready (`RaceSolver<UnreadyRunner>`): Runners can be added,
///    but their stats are not calculated yet.
/// 2. After runners are ready (`RaceSolver<Runner>`): Runners' stats are calculated
///    and out-of-gate skills are applied. The simulation can begin.
pub struct RaceSolver<T> {
    pub runners: Vec<Runner>,
    builders: Vec<UnreadyRunner>,
    pub state: RaceState,
    _marker: PhantomData<T>,
}

/// Methods available BEFORE runners are ready, i.e., before their stats have been
/// calculated and out-of-gate skills have been applied.
impl RaceSolver<UnreadyRunner> {
    /// Create a new RaceSolver with the given course and race parameters.
    pub fn new(course: Course, params: RaceParameters) -> Self {
        let seed = params.rng_seed;
        let rng = Rc::new(RefCell::new(rand_pcg::Pcg64Mcg::seed_from_u64(seed)));

        let state = RaceState {
            course,
            params,
            rng,
            elapsed_time: 0.0,
            finish_time: None,
            runners_by_style: HashMap::new(),
            rushed_by_style: HashMap::new(),
            existing_skills: HashSet::new(),
        };

        RaceSolver {
            runners: vec![],
            builders: vec![],
            state,
            _marker: PhantomData,
        }
    }

    /// Add a runner to the race BEFORE it starts.
    pub fn add_runner(&mut self, uma: Uma, motivation: Motivation, skills: Vec<Skill>) {
        // Get existing skills before moving them into builder
        for skill in &skills {
            self.state.existing_skills.insert(skill.id.clone());
        }

        // Create builder
        let builder = UnreadyRunner::new(
            uma,
            motivation,
            &self.state.course,
            &self.state.params,
            skills,
            Rc::clone(&self.state.rng),
        );

        // Update runnners-by-style count before moving builder
        *self
            .state
            .runners_by_style
            .entry(builder.base.style)
            .or_insert(0) += 1;

        // Add to builders list
        self.builders.push(builder);
    }

    /// Get all added runners ready by calculating their stats and activating
    /// out-of-gate skills. It returns a new RaceSolver with ready runners
    /// to run the simulation.
    pub fn ready_runners(self) -> RaceSolver<Runner> {
        let mut runners: Vec<Runner> = Vec::new();
        for builder in self.builders {
            let runner = Runner::new(builder, &self.state);
            runners.push(runner);
        }
        RaceSolver {
            runners,
            builders: vec![],
            state: self.state,
            _marker: PhantomData,
        }
    }
}

/// Methods available AFTER runners are ready, i.e., after their stats have been
/// calculated and out-of-gate skills have been applied.
impl RaceSolver<Runner> {
    /// Get the time as displayed on the scoreboard after a race finishes.
    pub fn scoreboard_time(&self) -> Option<f64> {
        let converted = self.state.finish_time? * 1.18;
        let min = self.state.course.finish_time_min();
        let max = self.state.course.finish_time_max();
        let clamped = converted.clamp(min, max);
        if converted < min || converted > max {
            Some(clamped + self.state.rng.borrow_mut().random_range(0.0..1.0))
        } else {
            Some(converted)
        }
    }

    /// Run simulation until the first runner finishes.
    ///
    /// Race state is updated 15 times per second.
    pub fn run(&mut self) {
        let dt = 1.0 / 15.0; // 15 FPS
        while self.state.finish_time.is_none() {
            self.step(dt);
        }
    }

    /// Perform a single simulation step.
    fn step(&mut self, delta_time: f64) {
        // Pre-Update
        for runner in &mut self.runners {
            runner.check_corner_end(&self.state.course.corners);
            runner.check_straight_end(&self.state.course.straights);
        }

        self.state.elapsed_time += delta_time;

        for runner in &mut self.runners {
            // Update
            if runner.start_delay > self.state.elapsed_time {
                continue;
            }
            runner.reset_activation_flags();
            runner.update_rush_state(&mut self.state);
            runner.check_all_preconditions(&self.state); // Check all preconditions every frame
            runner.check_time_triggers(&self.state);
            runner.check_continuous_triggers(&self.state);
            runner.update_target_speed(self.state.course.base_speed()); // TODO: review
            runner.update_acceleration();
            runner.update_phase(&mut self.state);
            runner.update_speed(delta_time, self.state.course.base_speed());
            runner.update_position(delta_time, &self.state);
            if runner.has_finished() && self.state.finish_time.is_none() {
                self.state.finish_time = Some(self.state.elapsed_time);
                // TODO: calculate how many lenghts ahead the runner is from the next runner
            }
            runner.update_hp(delta_time, &self.state);
            runner.update_slope(&self.state.course.slopes, &mut *self.state.rng.borrow_mut());
            runner.check_window_triggers(&self.state); // Distance, time, HP windows
            runner.check_activation_dependent_triggers(&self.state);
            runner.update_skill_durations(self.state.elapsed_time);
            // Post-Update
            runner.check_corner_start(&self.state.course.corners);
            runner.check_straight_start(&self.state.course.straights, &self.state);
        }

        if matches!(GAME_BEHAVIOR, GameBehavior::FirstAnniversary) {
            // TODO: Implement spot struggle and dueling checks.
        }
    }
}
