use std::cell::RefCell;
use std::collections::HashMap;
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
    /// Count of existing skill IDs among all runners.
    pub existing_skills: HashMap<String, u8>,
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
            existing_skills: HashMap::new(),
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
            *self
                .state
                .existing_skills
                .entry(skill.id.clone())
                .or_insert(0) += 1;
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

        self.runners
            .sort_by(|a, b| b.position.partial_cmp(&a.position).unwrap());

        self.update_order_values();
        self.update_blocking_status(delta_time);

        if matches!(GAME_BEHAVIOR, GameBehavior::FirstAnniversary) {
            // TODO: Implement spot struggle and dueling checks.
        }
    }

    /// Update order (rank) and distance differences for all runners.
    /// Runners must be sorted by position in descending order before calling this method.
    fn update_order_values(&mut self) {
        #[cfg(debug_assertions)]
        {
            // Debug: check sorting
            for i in 1..self.runners.len() {
                debug_assert!(self.runners[i - 1].position >= self.runners[i].position);
            }
        }

        let total_runners = self.runners.len() as u8;
        let should_update_range = self.state.elapsed_time > 5.0;
        let position_first = self.runners.first().unwrap().position;
        let position_last = self.runners.last().unwrap().position;
        let position_range = position_first - position_last;
        let mut position_next = position_first;
        let mut rush_counter = 0;

        // From first to last, update order and distance differences from runners ahead
        for (index, runner) in self.runners.iter_mut().enumerate() {
            if runner.has_finished() {
                continue;
            }
            runner.update_order((index + 1) as u8, &self.state.course);
            runner.update_order_rate(total_runners, should_update_range);
            runner.update_diff_from_ahead(
                position_first,
                position_next,
                position_range,
                rush_counter,
            );
            position_next = runner.position;
            rush_counter += runner.is_rushed() as u8;
        }

        // From last to first, update distance differences from runners behind
        let mut position_prev = position_last;
        let mut rush_counter = 0;
        for runner in self.runners.iter_mut().rev() {
            runner.update_diff_from_behind(position_prev, rush_counter);
            position_prev = runner.position;
            rush_counter += runner.is_rushed() as u8;
        }
    }

    /// Update blocking and overlapping status for all runners.
    /// Runners must be sorted by position in descending order before calling this method.
    fn update_blocking_status(&mut self, delta_time: f64) {
        #[cfg(debug_assertions)]
        {
            // Debug: check sorting
            for i in 1..self.runners.len() {
                debug_assert!(self.runners[i - 1].position >= self.runners[i].position);
            }
        }

        for runner in self.runners.iter_mut() {
            runner.reset_blocking_status();
        }

        let num_runners = self.runners.len();
        for i in 0..num_runners {
            let (left, right) = self.runners.split_at_mut(i + 1);
            let runner_i = &mut left[i];
            runner_i.update_blocked_time(delta_time);
            for runner_j in right {
                let distance = runner_i.position - runner_j.position;
                if distance > 20.0 {
                    break; // Too far to even consider
                }
                if runner_j.is_visible(runner_i, &self.state.course) {
                    // let speed_diff = runner_j.speed - runner_i.speed;
                    // j can overtake i in less than 15 seconds
                    // if distance > 0.001
                    //     && speed_diff / distance < 15.0
                    //     && (runner_i.base_target_speed < runner_j.base_target_speed
                    //         || runner_i.is_front_blocked() && runner_i.speed < runner_j.base_target_speed)
                    // {
                    //     // add i as overtake target for j
                    // }
                }
                if distance > 3.0 {
                    continue; // Not close enough to consider blocking
                }
                if runner_i.is_near(runner_j, &self.state.course) {
                    runner_i.nearby_count += 1;
                    runner_j.nearby_count += 1;
                    // Runner i is also in front of runner j; update front count
                    runner_j.nearby_front_count += 1;
                }
                if runner_j.is_front_blocked_by(runner_i, &self.state.course) {
                    runner_j.blocked.speed_limit =
                        Some((0.988 + 0.012 * distance) * runner_j.speed);
                }
                if runner_j.is_side_blocked_by(runner_i, &self.state.course) {
                    runner_j.blocked.side = true;
                }
            }
        }
    }
}
