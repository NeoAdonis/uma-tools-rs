use std::cell::RefCell;
use std::collections::{BTreeMap, HashMap};
use std::marker::PhantomData;
use std::rc::Rc;
use std::vec;

use rand::prelude::*;

use crate::configuration::{Configuration, GameVersion};
use crate::course::Course;
use crate::race::compete_before_spurt;
use crate::race::duel;
use crate::race::parameters::RaceParameters;
use crate::race::proximity;
use crate::race::runner::{POSITIONING_END_SECTION, PositioningStrategy, Runner};
use crate::race::secure_lead;
use crate::race::spot_struggle;
use crate::race::unready_runner::UnreadyRunner;
use crate::skill::*;
use crate::uma::*;
use crate::units::horse_lanes_to_course_widths;

/// Scale applied to raw finish times to get displayed times.
pub const FINISH_TIME_SCALE: f64 = 1.18;

/// Duration of a single frame in seconds (race state is updated 15 times per
/// second).
pub const DELTA_TIME: f64 = 1.0 / 15.0;

/// Speed used to convert scaled finish time differences into margins, in m/s.
const MARGIN_SPEED: f64 = 15.0;

/// Maximum number of runners in a single team.
pub const MAX_TEAM_SIZE: usize = 3;

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
    /// Number of teams in the race. Team indices go from 0 to `team_count - 1`.
    pub team_count: u8,
    /// Count of runners by base style (Runaways count as Front runners).
    pub runners_by_style: HashMap<Style, u8>,
    /// Base style of the most popular runner (Runaway counts as Front).
    pub style_most_popular: Style,
    /// Count of runners currently rushed, by team and base style (Runaways count
    /// as Front runners). Indexed as `[team][style as usize]`.
    pub rushed_by_team: Vec<[u8; Style::COUNT]>,
    /// Skill activations by team (out-of-gate skills included), as of the start
    /// of the current frame. Indexed by team.
    pub team_activations: Vec<usize>,
    /// Count of existing skill IDs among all runners.
    pub existing_skills: HashMap<String, u8>,
    /// Number of runners that activated a buff (advantage) skill in the current
    /// frame, by effect type.
    pub buffs_activated: HashMap<SkillType, u8>,
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
    /// Time (s) each pair of runners has been close in a row, keyed by their gates
    /// (lowest first). Only pairs that may still start a duel are tracked.
    duel_close_times: HashMap<(u8, u8), f64>,
    /// Marker to indicate whether the solver is in the "unready" or "ready" phase.
    _marker: PhantomData<T>,
}

/// Methods available BEFORE runners are ready (before stats have been
/// calculated and out-of-gate skills have been applied).
impl RaceSolver<UnreadyRunner> {
    /// Create a new RaceSolver for a given course and parameters.
    ///
    /// If the configuration has no seed, a random one is used.
    pub fn new(course: Course, params: RaceParameters, config: &Configuration) -> Self {
        let seed = config.seed.unwrap_or_else(rand::random);
        let rng = Rc::new(RefCell::new(rand_pcg::Pcg64Mcg::seed_from_u64(seed)));

        let state = RaceState {
            course,
            params,
            config: *config,
            rng,
            elapsed_time: 0.0,
            finish_time: None,
            team_count: 0,
            style_most_popular: Style::Front,
            runners_by_style: HashMap::new(),
            rushed_by_team: Vec::new(),
            team_activations: Vec::new(),
            existing_skills: HashMap::new(),
            buffs_activated: HashMap::new(),
        };

        RaceSolver {
            runners: vec![],
            builders: BTreeMap::new(),
            state,
            duel_close_times: HashMap::new(),
            _marker: PhantomData,
        }
    }

    /// Add an individual runner to the race BEFORE it starts.
    ///
    /// The runner gets a team of its own, so every other runner is an opponent.
    pub fn add_runner(&mut self, uma: Uma, mood: Mood, skills: Vec<Skill>) {
        if self.builders.len() >= self.state.params.gates as usize {
            panic!("Cannot add more runners than available gates");
        }

        let team = self.new_team();
        self.add_member(uma, mood, skills, team);
    }

    /// Add a team of 1 to [`MAX_TEAM_SIZE`] runners to the race BEFORE it starts.
    ///
    /// Members are allies of each other and opponents of everyone else.
    pub fn add_team(&mut self, members: Vec<(Uma, Mood, Vec<Skill>)>) {
        if members.is_empty() || members.len() > MAX_TEAM_SIZE {
            panic!("Team must have between 1 and {MAX_TEAM_SIZE} runners");
        }
        // Check the whole team up front so that it is never added partially.
        if self.builders.len() + members.len() > self.state.params.gates as usize {
            panic!("Cannot add more runners than available gates");
        }

        let team = self.new_team();
        for (uma, mood, skills) in members {
            self.add_member(uma, mood, skills, team);
        }
    }

    /// Allocate the next team index.
    fn new_team(&mut self) -> u8 {
        let team = self.state.team_count;
        self.state.team_count += 1;
        self.state.rushed_by_team.push([0; Style::COUNT]);
        self.state.team_activations.push(0);
        team
    }

    /// Add a runner as a member of `team`. Gate capacity must be checked by the
    /// caller.
    fn add_member(&mut self, uma: Uma, mood: Mood, skills: Vec<Skill>, team: u8) {
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
        let mut builder = UnreadyRunner::new(uma, gate_roll, mood, &self.state, skills);
        builder.initial_state.team = team;

        // Update runnners-by-style count before moving builder.
        *self
            .state
            .runners_by_style
            .entry(builder.base.style.base_style())
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
        let popularity_ranks = crate::race::popularity::calculate_popularity(&mut builders);

        // Assign popularity ranks to each runner.
        for (i, builder) in builders.iter_mut().enumerate() {
            builder.initial_state.popularity_rank = popularity_ranks[i];
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
            duel_close_times: HashMap::new(),
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
        let min = self.state.course.finish_time_min;
        let max = self.state.course.finish_time_max;
        let clamped = converted.clamp(min, max);
        if !(min..=max).contains(&converted) {
            // Add random offset between 0.0 and 1.0 seconds.
            Some(clamped + self.state.rng.borrow_mut().random_range(0.0..1.0))
        } else {
            Some(converted)
        }
    }

    /// Run simulation until every runner finishes, then record finish margins.
    pub fn run(&mut self) {
        while !self.runners.iter().all(Runner::has_finished) {
            self.step(DELTA_TIME);
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
    /// types activated this frame are counted, and the
    /// `is_other_character_activate_advantage_skill` triggers of every runner
    /// are checked, so that the result doesn't depend on which runner checked
    /// first. Then, effects targeting other runners are applied.
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
        for runner in &mut self.runners {
            // Runners that stay in the gate during this step haven't reached any state
            // worth checking, and skills can't activate after the goal.
            if !runner.can_activate_skills(self.state.elapsed_time + delta_time) {
                continue;
            }
            runner.check_skills(&mut self.state);
            for &skill_type in &runner.activation_flags.buff_activated {
                *self.state.buffs_activated.entry(skill_type).or_insert(0) += 1;
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
            runner.update_slope(
                &self.state.course.slopes,
                &transitions.slope,
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
        self.resolve_overlaps();
        self.update_blocking_status(delta_time);
        self.update_spot_struggle();
        self.update_duels(delta_time);
        self.update_positioning_strategy();
        self.update_secure_lead();
        self.update_compete_before_spurt();
    }

    /// Check `is_other_character_activate_advantage_skill` triggers of every runner,
    /// now that this frame's activations are known.
    ///
    /// Skills activated here aren't counted as advantage skills for other runners.
    fn check_advantage_triggers(&mut self, delta_time: f64) {
        // ? Assume activations don't chain within a frame.
        if self.state.buffs_activated.is_empty() {
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
                for target in self.select_targets(caster, pending_effect.effect.target) {
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
    /// Runners must be sorted by position in descending order before calling this.
    fn select_targets(&self, caster: usize, target: SkillTarget) -> Vec<usize> {
        let runners = &self.runners;
        let source = &runners[caster];
        // Finished runners are skipped before counting targets. Nearest runners first.
        let is_running = |i: &usize| !runners[*i].has_finished();
        let ahead = (0..caster).rev().filter(is_running);
        let behind = (caster + 1..runners.len()).filter(is_running);
        let others = (0..runners.len()).filter(|&i| i != caster && is_running(&i));
        let is_rushed = |i: &usize| runners[*i].is_rushed();

        let selected: Vec<usize> = match target {
            SkillTarget::Oneself => vec![caster],
            SkillTarget::All => (0..runners.len()).filter(is_running).collect(),
            // ? Assume field of view only covers runners ahead, as for visible counts.
            SkillTarget::InFov { max } => ahead
                .filter(|&i| source.is_visible(&runners[i]))
                .take(max as usize)
                .collect(),
            // Index `position - 1` is the runner placed at `position`.
            SkillTarget::AheadOfPosition { position } => (0..position.saturating_sub(1) as usize)
                .filter(|&i| i < runners.len() && i != caster && is_running(&i))
                .collect(),
            SkillTarget::AheadOfSelf { count } => ahead.take(count as usize).collect(),
            SkillTarget::BehindSelf { count } => behind.take(count as usize).collect(),
            SkillTarget::AllAllies => others
                .filter(|&i| !source.is_opponent(&runners[i]))
                .collect(),
            SkillTarget::EnemyStrategy { style } => others
                .filter(|&i| runners[i].base_style() == style)
                .collect(),
            SkillTarget::RushedAhead { max } => {
                ahead.filter(is_rushed).take(max as usize).collect()
            }
            SkillTarget::RushedBehind { max } => {
                behind.filter(is_rushed).take(max as usize).collect()
            }
            SkillTarget::RushedStrategy { style } => others
                .filter(|&i| is_rushed(&i) && runners[i].base_style() == style)
                .collect(),
            // Runners that activated a recovery skill this frame, matching
            // `is_other_character_activate_advantage_skill==9`.
            SkillTarget::UsedRecovery { .. } => others
                .filter(|&i| {
                    runners[i]
                        .activation_flags
                        .buff_activated
                        .contains(&SkillType::Recovery)
                })
                .collect(),
            // Not used by any skill.
            SkillTarget::UmaId { .. } => Vec::new(),
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

        let total_runners = self.runners.len() as u8;
        let should_update_range = self.state.elapsed_time > 5.0;
        let position_first = self.runners.first().unwrap().position;
        let position_last = self.runners.last().unwrap().position;
        let position_range = position_first - position_last;
        let mut position_next = position_first;
        // Rushed runners counted so far, in total and by team.
        let mut rushed = 0;
        let mut rushed_by_team = vec![0; self.state.team_count as usize];

        // From first to last, update order and distance differences from runners ahead.
        for (index, runner) in self.runners.iter_mut().enumerate() {
            if runner.has_finished() {
                continue;
            }
            let team = runner.initial_state.team as usize;
            runner.update_order((index + 1) as u8, &self.state.course);
            runner.update_order_rate(total_runners, should_update_range);
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
        let mut position_prev = position_last;
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
                    let (inner_lane, outer) = if lane_diff > 0.0 {
                        (behind.lane.current, i)
                    } else {
                        (ahead.lane.current, j)
                    };
                    let bumped_lane =
                        inner_lane + horse_lanes_to_course_widths(proximity::OVERLAP_LANE_GAP);
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
                // Lead over the runner behind if first overall (or alone).
                let lead = (i == 0).then(|| {
                    self.runners
                        .get(1)
                        .map_or(f64::INFINITY, |next| runner.position - next.position)
                });
                // Lead over the next runner with the same strategy if first among them.
                let same_strategy = |other: &Runner| other.positioning_style() == style;
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
        let mut close_times = HashMap::with_capacity(self.duel_close_times.len());
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

                let gates = (ahead.gate_roll, behind.gate_roll);
                let key = (gates.0.min(gates.1), gates.0.max(gates.1));
                // Time starts counting from the first frame they're close.
                let close_time = self
                    .duel_close_times
                    .get(&key)
                    .map_or(0.0, |time| time + delta_time);
                close_times.insert(key, close_time);
                if duel::is_close_long_enough(close_time) && self.can_start_duel(i, j) {
                    starting.extend([i, j]);
                }
            }
        }
        // Pairs no longer close are dropped, which resets their time.
        self.duel_close_times = close_times;
        for i in starting {
            self.runners[i].start_duel();
        }
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
