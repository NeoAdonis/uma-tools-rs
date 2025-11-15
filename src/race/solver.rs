use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::marker::PhantomData;
use std::rc::Rc;
use std::vec;

use rand::prelude::*;

use crate::configuration::{GAME_BEHAVIOR, GameBehavior};
use crate::course::{Course, Phase};
use crate::race::parameters::RaceParameters;
use crate::race::runner::Runner;
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
    /// Set of activated buff skill types among all runners.
    pub buffs_activated: HashSet<SkillType>,
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
    builders: HashMap<u8, UnreadyRunner>,
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
            buffs_activated: HashSet::new(),
        };

        RaceSolver {
            runners: vec![],
            builders: HashMap::new(),
            state,
            _marker: PhantomData,
        }
    }

    /// Add a runner to the race BEFORE it starts.
    pub fn add_runner(&mut self, uma: Uma, motivation: Motivation, skills: Vec<Skill>) {
        if self.builders.len() as u8 >= self.state.params.gates {
            panic!("Cannot add more runners than available gates");
        }

        // Get existing skills before moving them into builder
        for skill in &skills {
            *self
                .state
                .existing_skills
                .entry(skill.id.clone())
                .or_insert(0) += 1;
        }

        // Roll gate
        let gate_roll = loop {
            let roll = self
                .state
                .rng
                .borrow_mut()
                .random_range(1..=self.state.params.gates);
            if !self.builders.contains_key(&roll) {
                break roll;
            }
        };

        // Create builder
        let builder = UnreadyRunner::new(
            uma,
            gate_roll,
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
        self.builders.insert(gate_roll, builder);
    }

    /// Get all added runners ready by calculating their stats and activating
    /// out-of-gate skills. It returns a new RaceSolver with ready runners
    /// to run the simulation.
    pub fn ready_runners(self) -> RaceSolver<Runner> {
        let mut runners: Vec<Runner> = Vec::new();
        for builder in self.builders {
            let runner = Runner::new(builder.1, &self.state);
            runners.push(runner);
        }
        RaceSolver {
            runners,
            builders: HashMap::new(),
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
        if !(min..=max).contains(&converted) {
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
            runner.update_lane(&self.state.course, delta_time);
            runner.update_hp(delta_time, &self.state);
            runner.update_slope(&self.state.course.slopes, &mut *self.state.rng.borrow_mut());
            runner.check_window_triggers(&self.state); // Distance, time, HP windows
            runner.check_activation_dependent_triggers(&self.state);
            runner.extract_buff_activations(&mut self.state);
            // Post-Update
            runner.check_corner_start(&self.state.course.corners);
            runner.check_straight_start(&self.state.course.straights, &self.state);
        }

        for runner in &mut self.runners {
            runner.check_buff_dependent_triggers(&self.state);
            runner.update_skill_durations(self.state.elapsed_time);
        }

        self.runners
            .sort_by(|a, b| b.position.partial_cmp(&a.position).unwrap());

        self.update_order_values();
        self.update_blocking_status(delta_time);
        self.update_positioning_strategy();
    }

    /// Update order (rank) and distance differences for all runners.
    /// Runners must be sorted by position in descending order before calling this method.
    fn update_order_values(&mut self) {
        // Debug: check sorting
        self.assert_runners_sorted();

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
        // Debug: check sorting
        self.assert_runners_sorted();

        for runner in self.runners.iter_mut() {
            runner.reset_blocking_status();
            runner.uncheck_duel_targets();
            runner.reset_overtake_checks();
        }

        let num_runners = self.runners.len();
        for i in 0..num_runners {
            let (left, right) = self.runners.split_at_mut(i + 1);
            let runner_i = &mut left[i];
            let mut was_runner_behind_checked = false;

            for runner_j in right {
                // Only check runner immediately behind
                if !was_runner_behind_checked && runner_j.lane.current < runner_i.lane.current {
                    runner_i.nearby.behind_in = true;
                }
                was_runner_behind_checked = true;

                let distance = runner_i.position - runner_j.position;
                let lane_diff = runner_i.lane.current - runner_j.lane.current;

                if distance > 20.0 {
                    break; // We have gone too far behind; move to next i
                }

                if runner_j.is_visible(runner_i, &self.state.course) {
                    runner_j.visible_count += 1;
                    // Check if runner_i is an overtake target for runner_j
                    if runner_j.check_overtake_target(runner_i, delta_time) {
                        runner_i.being_overtake_target_time += delta_time;
                    }
                }

                if distance > 5.0 {
                    continue; // Too far for the following checks
                }

                // Update surrounding and blocking status
                if self.state.course.is_front_surrounded(distance, lane_diff) {
                    runner_j.update_front_surrounded_by(runner_i);
                }
                if self.state.course.is_side_surrounded(distance, lane_diff) {
                    runner_j.update_side_surrounded_by(runner_i);
                }
                if self.state.course.is_near(distance, lane_diff) {
                    runner_j.update_near_behind(runner_i);
                }
                if self.state.course.is_near_same_lane(distance, lane_diff) {
                    runner_j.update_near_same_lane_behind(runner_i);
                }
                if self.state.course.is_near_behind_set1(distance, lane_diff) {
                    runner_i.nearby.behind_set1_count += 1;
                }
                if self.state.course.check_front_blocked(distance, lane_diff) {
                    runner_j.update_front_blocked_by(runner_i);
                }
                if self.state.course.check_side_blocked(distance, lane_diff) {
                    runner_j.update_side_blocked_by(runner_i);
                }

                // Update spot struggle and dueling status
                if matches!(GAME_BEHAVIOR, GameBehavior::FirstAnniversary) {
                    if runner_j.can_spot_struggle(runner_i, distance, lane_diff, &self.state.course)
                    {
                        runner_i.start_spot_struggle(self.state.elapsed_time);
                        runner_j.start_spot_struggle(self.state.elapsed_time);
                    }
                    if self.state.course.check_dueling(distance, lane_diff) {
                        runner_i.update_duel_target(runner_j, delta_time);
                        runner_j.update_duel_target(runner_i, delta_time);
                    }
                }
            }

            // Checked all runners behind i within a reasonable distance
            // runner_i's block, near and dueling statuses can be updated now
            runner_i.update_blocked_time(delta_time);
            runner_i.update_near_time(delta_time);
            runner_i.filter_duel_targets();
            runner_i.finalize_overtake_state(delta_time, self.state.elapsed_time);
        }

        // Calculate lane targets (needs access to all runners)
        // Use unsafe pointer to access runners slice while iterating
        let runners_ptr = self.runners.as_ptr();
        let runners_len = self.runners.len();

        // STUB: Using first corner as move lane point; should use course.move_lane_point when available
        let move_lane_point = self
            .state
            .course
            .corners
            .first()
            .map_or(self.state.course.halfway(), |c| c.start);

        for i in 0..num_runners {
            let runner = &mut self.runners[i];

            // Skip if side-blocked (can't change lanes anyway)
            if runner.blocked.side {
                continue;
            }

            // Safety: We're only reading from other runners, not modifying them
            let all_runners = unsafe { std::slice::from_raw_parts(runners_ptr, runners_len) };

            // Determine which mode to use: normal vs overtake
            let distance_to_move_lane = move_lane_point - runner.position;
            let within_200m_of_move_lane =
                (0.0..=200.0).contains(&distance_to_move_lane);

            // Use normal mode if:
            // 1. Not in overtake mode, OR
            // 2. In Early/Mid phase AND within 200m of move lane point
            let use_normal_mode = !runner.is_in_overtake_mode()
                || (matches!(runner.phase, Phase::Early | Phase::Mid) && within_200m_of_move_lane);

            if use_normal_mode {
                runner.calculate_normal_mode_lane_target(&self.state.course, all_runners);
            } else {
                runner.calculate_overtake_lane_target(&self.state.course, all_runners);
            }
        }
    }

    /// Update positioning strategy for all runners.
    /// Runners must be sorted by position in descending order before calling this method.
    fn update_positioning_strategy(&mut self) {
        // Debug: check sorting
        self.assert_runners_sorted();

        let is_single_front_runner = self
            .state
            .runners_by_style
            .get(&Style::Front)
            .copied()
            .unwrap_or(0)
            == 1;

        // Find pacemaker (the most forward runner with the most aggressive style)
        // TODO: fix; this is valid only before the 1.5 anniversary update
        let mut pacemaker_style = self.runners[0].adjusted.style;
        let mut pacemaker_position = self.runners[0].position;
        for i in 0..self.runners.len() {
            let runner_i = &self.runners[i];
            if runner_i.adjusted.style < pacemaker_style {
                pacemaker_style = runner_i.adjusted.style;
                pacemaker_position = runner_i.position;
            }
            // Because runners are sorted by position descendingly, we only
            // need to update pacemaker when a more agressive style is found
        }

        for i in 0..self.runners.len() {
            let runner = &self.runners[i];
            match runner.adjusted.style {
                Style::Front | Style::Runaway => {
                    // Determine if we are in first place, and by how much
                    let distance_ahead = if i == 0 && i + 1 < self.runners.len() {
                        runner.position - self.runners[i + 1].position
                    } else {
                        0.0
                    };
                    let runner = &mut self.runners[i];
                    runner.update_positioning_front(
                        self.state.elapsed_time,
                        distance_ahead,
                        is_single_front_runner,
                    )
                }
                _ => {
                    // Determine distance behind pacemaker
                    let distance_behind = pacemaker_position - runner.position;
                    let runner = &mut self.runners[i];
                    runner.update_positioning_non_front(
                        self.state.elapsed_time,
                        self.state.course.distance as f64,
                        distance_behind,
                    )
                }
            }
        }

        // TODO: implement PaceUpEx strategy
    }

    #[inline]
    fn assert_runners_sorted(&self) {
        #[cfg(debug_assertions)]
        for i in 1..self.runners.len() {
            debug_assert!(self.runners[i - 1].position >= self.runners[i].position);
        }
    }
}
