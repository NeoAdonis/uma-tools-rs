use std::cell::RefCell;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::marker::PhantomData;
use std::rc::Rc;
use std::vec;

use rand::prelude::*;

use crate::configuration::{Configuration, GameVersion};
use crate::course::{Course, Phase};
use crate::race::compete_before_spurt;
use crate::race::parameters::RaceParameters;
use crate::race::runner::{POSITIONING_END_SECTION, PositioningStrategy, Runner};
use crate::race::secure_lead;
use crate::race::spot_struggle;
use crate::race::unready_runner::UnreadyRunner;
use crate::skill::*;
use crate::uma::*;

/// Scale applied to raw finish times to get displayed times.
pub const FINISH_TIME_SCALE: f64 = 1.18;

/// Speed used to convert scaled finish time differences into margins, in m/s.
const MARGIN_SPEED: f64 = 15.0;

pub struct RaceState {
    pub course: Course,
    pub params: RaceParameters,
    /// Runtime configuration. Shared by all runners so that they all follow the
    /// same game behavior.
    pub config: Configuration,
    /// Random number generator shared by all runners.
    /// It is initialized with a seed to ensure reproducibility.
    pub rng: Rc<RefCell<rand_pcg::Pcg64Mcg>>,
    /// Elapsed time in seconds. This value is shared by all runners.
    pub elapsed_time: f64,
    /// Time when the first runner finished, in seconds. This might be different
    /// from displayed finish time due to scoreboard conversion.
    pub finish_time: Option<f64>,
    /// Count of runners by style.
    pub runners_by_style: HashMap<Style, u8>,
    /// Running style of the most popular runner.
    pub style_most_popular: Style,
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
/// 1. Before runners are ready (`RaceSolver<UnreadyRunner>`): Runners can be
///    added, but their stats are not calculated yet.
/// 2. After runners are ready (`RaceSolver<Runner>`): Runners' stats are
///    calculated and out-of-gate skills are applied. The simulation can begin.
pub struct RaceSolver<T> {
    pub runners: Vec<Runner>,
    // Ordered by gate so that runners are always built in the same order (which
    // determines how the shared RNG is consumed).
    builders: BTreeMap<u8, UnreadyRunner>,
    pub state: RaceState,
    _marker: PhantomData<T>,
}

/// Methods available BEFORE runners are ready (before stats have been
/// calculated and out-of-gate skills have been applied).
impl RaceSolver<UnreadyRunner> {
    /// Create a new RaceSolver for a given course and parameters.
    pub fn new(course: Course, params: RaceParameters, config: &Configuration) -> Self {
        let seed = params.rng_seed;
        let rng = Rc::new(RefCell::new(rand_pcg::Pcg64Mcg::seed_from_u64(seed)));

        let state = RaceState {
            course,
            params,
            config: *config,
            rng,
            elapsed_time: 0.0,
            finish_time: None,
            style_most_popular: Style::Front,
            runners_by_style: HashMap::new(),
            rushed_by_style: HashMap::new(),
            existing_skills: HashMap::new(),
            buffs_activated: HashSet::new(),
        };

        RaceSolver {
            runners: vec![],
            builders: BTreeMap::new(),
            state,
            _marker: PhantomData,
        }
    }

    /// Add a runner to the race BEFORE it starts.
    pub fn add_runner(&mut self, uma: Uma, motivation: Motivation, skills: Vec<Skill>) {
        if self.builders.len() as u8 >= self.state.params.gates {
            panic!("Cannot add more runners than available gates");
        }

        // Get existing skills before moving them into builder.
        for skill in &skills {
            *self
                .state
                .existing_skills
                .entry(skill.id.clone())
                .or_insert(0) += 1;
        }

        // Roll gate.
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

        // Create builder.
        let builder = UnreadyRunner::new(uma, gate_roll, motivation, &self.state, skills);

        // Update runnners-by-style count before moving builder.
        *self
            .state
            .runners_by_style
            .entry(builder.base.style)
            .or_insert(0) += 1;

        // Add to builders list.
        self.builders.insert(gate_roll, builder);
    }

    /// Get all runners ready by calculating their stats and activating out-of-gate
    /// skills.
    ///
    /// Returns a new [`RaceSolver`] with ready runners to run the simulation.
    pub fn ready_runners(self) -> RaceSolver<Runner> {
        // Collect all UnreadyRunners and move state out.
        let mut builders: Vec<UnreadyRunner> = self.builders.into_values().collect();
        let mut state = self.state;

        // Calculate popularity for all runners.
        let popularity_ranks = crate::race::popularity::calculate_popularity(&mut builders, &state);

        // Assign popularity ranks to each runner.
        for (i, builder) in builders.iter_mut().enumerate() {
            builder.initial_state.popularity_rank = popularity_ranks[i];
            if builder.initial_state.popularity_rank == 1 {
                state.style_most_popular = builder.base.style;
            }
        }

        // Build final runners.
        let mut runners: Vec<Runner> = Vec::new();
        for builder in builders {
            let runner = Runner::new(builder, &state);
            runners.push(runner);
        }

        RaceSolver {
            runners,
            builders: BTreeMap::new(),
            state,
            _marker: PhantomData,
        }
    }
}

/// Methods available AFTER runners are ready (stats have been calculated,
/// out-of-gate skills have been applied...).
impl RaceSolver<Runner> {
    /// Get the time as displayed on the scoreboard after a race finishes.
    pub fn scoreboard_time(&self) -> Option<f64> {
        let converted = self.state.finish_time? * FINISH_TIME_SCALE;
        let min = self.state.course.finish_time_min();
        let max = self.state.course.finish_time_max();
        let clamped = converted.clamp(min, max);
        if !(min..=max).contains(&converted) {
            // Add random offset between 0.0 and 1.0 seconds.
            Some(clamped + self.state.rng.borrow_mut().random_range(0.0..1.0))
        } else {
            Some(converted)
        }
    }

    /// Run simulation until every runner finishes, then record finish margins.
    ///
    /// Race state is updated 15 times per second.
    pub fn run(&mut self) {
        let dt = 1.0 / 15.0; // 15 FPS
        while !self.runners.iter().all(Runner::has_finished) {
            self.step(dt);
        }
        self.update_finish_margins();
    }

    /// Set each runner's margin behind the runner that placed immediately ahead.
    ///
    /// Margin distance is calculated by scaling the difference in *displayed*
    /// finish times between runners by a set multiplier (15 m/s). All runners must
    /// have finished before calling this function.
    fn update_finish_margins(&mut self) {
        // Order must come from finish times.
        let mut by_finish: Vec<&mut Runner> = self.runners.iter_mut().collect();
        by_finish.sort_by(|a, b| a.finish_time.partial_cmp(&b.finish_time).unwrap());

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
    /// Skills are checked first, against the state reached at the end of the
    /// previous step (including multi-runner values such as order). Then, time
    /// advances and every runner is updated.
    fn step(&mut self, delta_time: f64) {
        for runner in &mut self.runners {
            // Runners still in the gate haven't reached any state worth checking, and
            // skills can't activate after the goal.
            if runner.start_delay > self.state.elapsed_time || runner.has_finished() {
                continue;
            }
            runner.check_skills(&mut self.state);
        }

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
            runner.check_corner_end(&self.state.course.corners);
            runner.check_corner_start(&self.state.course.corners);
            runner.check_straight_end(&self.state.course.straights);
            runner.check_straight_start(&self.state.course.straights);
            runner.update_slope(
                &self.state.course.slopes,
                delta_time,
                &mut *self.state.rng.borrow_mut(),
            );

            runner.update_lane(&self.state.course, delta_time);
            runner.update_hp(delta_time, &self.state);
            runner.update_charge_up(delta_time, self.state.elapsed_time);
            runner.update_skill_durations(self.state.elapsed_time);
        }

        self.runners
            .sort_by(|a, b| b.position.partial_cmp(&a.position).unwrap());

        self.update_order_values();
        self.update_blocking_status(delta_time);
        self.update_spot_struggle();
        self.update_positioning_strategy();
        self.update_secure_lead();
        self.update_compete_before_spurt();
    }

    /// Update order (rank) and distance differences for all runners.
    ///
    /// Runners must be sorted by position in descending order before calling this.
    fn update_order_values(&mut self) {
        // Debug: check sorting.
        self.assert_runners_sorted();

        let total_runners = self.runners.len() as u8;
        let should_update_range = self.state.elapsed_time > 5.0;
        let position_first = self.runners.first().unwrap().position;
        let position_last = self.runners.last().unwrap().position;
        let position_range = position_first - position_last;
        let mut position_next = position_first;
        let mut rush_counter = 0;

        // From first to last, update order and distance differences from runners ahead.
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

        // From last to first, update distance differences from runners behind.
        let mut position_prev = position_last;
        let mut rush_counter = 0;
        for runner in self.runners.iter_mut().rev() {
            runner.update_diff_from_behind(position_prev, rush_counter);
            position_prev = runner.position;
            rush_counter += runner.is_rushed() as u8;
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
            runner.uncheck_duel_targets();
            runner.reset_overtake_checks();
        }

        let game_version = self.state.config.game_version;
        let num_runners = self.runners.len();
        for i in 0..num_runners {
            let (left, right) = self.runners.split_at_mut(i + 1);
            let runner_i = &mut left[i];
            let mut was_runner_behind_checked = false;

            for runner_j in right {
                // Only check runner immediately behind.
                if !was_runner_behind_checked && runner_j.lane.current < runner_i.lane.current {
                    runner_i.nearby.behind_in = true;
                }
                was_runner_behind_checked = true;

                let distance = runner_i.position - runner_j.position;
                let lane_diff = runner_i.lane.current - runner_j.lane.current;

                if distance > 20.0 {
                    // We have gone too far behind; move to next i.
                    break;
                }

                if runner_j.is_visible(runner_i, &self.state.course) {
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
                if self.state.course.is_front_surrounded(distance, lane_diff) {
                    runner_j.update_front_surrounded_by(runner_i);
                }
                if self.state.course.is_side_surrounded(distance, lane_diff) {
                    runner_j.update_side_surrounded_by(runner_i);
                }
                if self.state.course.is_near(distance, lane_diff, game_version) {
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

                // Update dueling status.
                if game_version >= GameVersion::FirstAnniversary
                    && self.state.course.check_dueling(distance, lane_diff)
                {
                    runner_i.update_duel_target(runner_j, delta_time);
                    runner_j.update_duel_target(runner_i, delta_time);
                }
            }

            // Checked all runners behind i within a reasonable distance.
            // runner_i's block, near and dueling statuses can be updated now.
            runner_i.update_blocked_time(delta_time);
            runner_i.update_near_time(delta_time);
            runner_i.filter_duel_targets();
            runner_i.finalize_overtake_state(delta_time, self.state.elapsed_time);
        }

        // STUB: Using first corner as move lane point; should use
        // course.move_lane_point when available.
        let move_lane_point = self
            .state
            .course
            .corners
            .first()
            .map_or(self.state.course.halfway(), |c| c.start);

        // Calculate lane targets.
        for i in 0..num_runners {
            // Borrow all runners along with a single runner in an immutable state first.
            let all_runners = &self.runners;
            let runner = &all_runners[i];

            // Skip if side-blocked (can't change lanes anyway).
            if runner.blocked.side {
                continue;
            }

            // Determine which mode to use: normal vs overtake.
            let distance_to_move_lane = move_lane_point - runner.position;
            let within_200m_of_move_lane = (0.0..=200.0).contains(&distance_to_move_lane);

            // Use normal mode if:
            // 1. Not in overtake mode, OR;
            // 2. In Early/Mid phase AND within 200m of move lane point.
            let use_normal_mode = !runner.is_in_overtake_mode()
                || (matches!(runner.phase, Phase::Early | Phase::Mid) && within_200m_of_move_lane);

            let decision = if use_normal_mode {
                runner.normal_mode_lane_decision(&self.state.course, all_runners)
            } else {
                runner.overtake_lane_decision(&self.state.course, all_runners)
            };

            // Shared borrow of all runners ends here; result can be written back.
            let runner = &mut self.runners[i];
            runner.lane.target = decision.target;
            runner.extra_move_lane = decision.extra_move_lane;
        }
    }

    /// Update positioning strategy for all runners. Runners must be sorted by
    /// position in descending order before calling this method.
    fn update_positioning_strategy(&mut self) {
        // Debug: check sorting.
        self.assert_runners_sorted();

        // Because we are counting how many runners are *currently* behaving as front
        // runners rather than *actual* front runners, we cannot use runners_by_style
        // from RaceState.
        let front_runner_count = self
            .runners
            .iter()
            .filter(|runner| runner.positioning_style().is_front_like())
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
        // Most passive style among runners ahead of the current one.
        let mut max_style_ahead: Option<Style> = None;

        for i in 0..self.runners.len() {
            let style = self.runners[i].positioning_style();
            let passive_ahead = max_style_ahead > Some(style);
            max_style_ahead = max_style_ahead.max(Some(style));
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
                // Determine if we are in first place, and by how much.
                let distance_ahead = if i == 0 && i + 1 < self.runners.len() {
                    runner.position - self.runners[i + 1].position
                } else {
                    0.0
                };
                let runner = &mut self.runners[i];
                runner.update_positioning_front(
                    distance_ahead,
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
            self.runners[i].expire_secure_lead(elapsed_time);
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

    /// Update compete before spurt (位置取り調整) state for all runners.
    fn update_compete_before_spurt(&mut self) {
        let elapsed_time = self.state.elapsed_time;
        let course_distance = self.state.course.distance as f64;

        for i in 0..self.runners.len() {
            self.runners[i].expire_compete_before_spurt(elapsed_time);
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
