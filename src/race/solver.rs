use crate::course::Course;
use crate::race::parameters::RaceParameters;
use crate::race::runner::{GAME_BEHAVIOR, GameBehavior, Runner, RunnerBuilder};
use crate::uma::skill::*;
use crate::uma::*;
use rand::prelude::*;
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;
use std::vec;

pub struct RaceState {
    pub course: Course,
    pub params: RaceParameters,
    pub rng: Rc<RefCell<rand_pcg::Pcg64Mcg>>,
    pub elapsed_time: f64,        // Race time in seconds (shared by all umas)
    pub finish_time: Option<f64>, // Time when first Uma finished
    pub runners_by_style: HashMap<Style, u8>, // Count of runners by style
    pub rushed_by_style: HashMap<Style, u8>, // Count of runners currently rushed by style
    pub existing_skills: HashSet<String>,
}

pub struct NoRunner;

pub struct RaceSolver<T> {
    pub runners: Vec<T>,
    pub builders: Vec<RunnerBuilder>,
    pub state: RaceState,
}

/// Methods available before runners have been built
impl RaceSolver<NoRunner> {
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
        }
    }

    pub fn add_runner(&mut self, uma: Uma, motivation: Motivation, skills: Vec<Skill>) {
        // Get existing skills before moving them into builder
        for skill in &skills {
            self.state.existing_skills.insert(skill.id.clone());
        }

        // Create builder
        let builder = RunnerBuilder::new(
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

    pub fn build_runners(self) -> RaceSolver<Runner> {
        let mut runners: Vec<Runner> = Vec::new();
        for builder in self.builders {
            let runner = Runner::new(builder, &self.state);
            runners.push(runner);
        }
        RaceSolver {
            runners,
            builders: vec![],
            state: self.state,
        }
    }
}

/// Methods available after runners have been built
impl RaceSolver<Runner> {
    /// Get the time as displayed on the scoreboard after a race finishes
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

    /// Perform a single simulation step.
    fn step(&mut self, delta_time: f64) {
        // Pre-Update
        for runner in &mut self.runners {
            runner.check_corner_end(&self.state.course.corners, &self.state);
            runner.check_straight_end(&self.state.course.straights, &self.state);
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
            runner.update_last_spurt_state(&self.state.course, self.state.params.condition);
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
            runner.check_window_triggers(&self.state); // Check window conditions (distance, time, HP)
            runner.update_slope(
                &self.state.course.slopes,
                &mut *self.state.rng.borrow_mut(),
                &self.state,
            );
            runner.check_activation_dependent_triggers(&self.state);
            runner.update_skill_durations(self.state.elapsed_time);
            // Post-Update
            runner.check_corner_start(&self.state.course.corners, &self.state);
            runner.check_straight_start(&self.state.course.straights, &self.state);
        }

        if matches!(GAME_BEHAVIOR, GameBehavior::FirstAnniversary) {
            // TODO: Implement spot struggle and dueling checks.
        }
    }

    pub fn run(&mut self) {
        let dt = 1.0 / 15.0; // 15 FPS
        while self.state.finish_time.is_none() {
            self.step(dt);
        }
    }
}
