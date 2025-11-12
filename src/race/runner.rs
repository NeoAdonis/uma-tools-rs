use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

use rand::prelude::*;

use crate::condition::evaluator::{EvaluatorFull, evaluate_comparison_list, evaluate_condition};
use crate::condition::parser::CompOp;
use crate::course::geometry::{Corner, Slope, Straight};
use crate::course::*;
use crate::race::hp::*;
use crate::race::last_spurt::*;
use crate::race::modifiers::*;
use crate::race::parameters::TrackCondition;
use crate::race::solver::RaceState;
use crate::race::transitions::*;
use crate::race::unready_runner::UnreadyRunner;
use crate::trigger::skill_activation::{ActiveTimedEffect, SkillActivationRecord, WindowVariable};
use crate::trigger::system::TriggerScheduler;
use crate::uma::skill::*;
use crate::uma::*;

#[allow(dead_code)]
pub enum GameBehavior {
    Original,         // when the game was first released
    FirstAnniversary, // after the first anniversary update
}

pub static GAME_BEHAVIOR: GameBehavior = GameBehavior::Original;

/// Skill modifiers that are applied during race calculations.
#[derive(Debug, Clone)]
pub struct SkillModifiers {
    pub target_speed: f64,
    pub current_speed: f64,
    pub acceleration: f64,
    pub duration_scaling: f64, // Multiplicative, defaults to 1.0
}

impl Default for SkillModifiers {
    fn default() -> Self {
        Self {
            target_speed: 0.0,
            current_speed: 0.0,
            acceleration: 0.0,
            duration_scaling: 1.0, // Multiplicative default
        }
    }
}

/// Apply style change effects.
#[inline]
pub fn apply_style_change(style: &mut Style, effect_type: &SkillType) {
    #[allow(clippy::single_match)] // Account for possible future additions
    match effect_type {
        SkillType::ChangeStyleRunaway => {
            *style = Style::Runaway;
        }
        _ => {} // Not a style change effect
    }
}

/// Check if an effect type is a stat boost (applied permanently).
#[inline]
pub fn is_stat_boost(effect_type: &SkillType) -> bool {
    matches!(
        effect_type,
        SkillType::SpeedUp
            | SkillType::StaminaUp
            | SkillType::PowerUp
            | SkillType::GutsUp
            | SkillType::WizUp
            | SkillType::AllStatsUp
    )
}

/// Check if an effect type is a start delay modification.
#[inline]
pub fn is_start_delay_modification(effect_type: &SkillType) -> bool {
    matches!(
        effect_type,
        SkillType::MultiplyStartDelay | SkillType::SetStartDelay
    )
}

/// Check if an effect type is a style change.
#[inline]
pub fn is_style_change(effect_type: &SkillType) -> bool {
    matches!(effect_type, SkillType::ChangeStyleRunaway)
}

/// Check if an effect type is a timed effect.
#[inline]
pub fn is_timed_effect(effect_type: &SkillType) -> bool {
    matches!(
        effect_type,
        SkillType::Acceleration
            | SkillType::TargetSpeed
            | SkillType::CurrentSpeed
            | SkillType::CurrentSpeedWithNaturalDeceleration
            | SkillType::ExtendEvolvedDuration
    )
}

/// State related to rushing (temptation, 掛かり).
pub struct RushState {
    pub section: u8,
    pub start_time: f64,
    pub end_time: f64,
    pub max_duration: f64,
    pub is_rushed: bool,
}

/// Modifier applied during spot struggle and its expiration time.
pub struct StruggleState {
    modifier: f64,
    expires_at: f64,
}

/// Modifiers applied during dueling.
pub struct DuelingState {
    target_speed_modifier: f64,
    acceleration_modifier: f64,
}

/// Information about a duel target.
pub struct DuelTarget {
    /// Whether this target has been checked in the current update cycle.
    is_checked: bool,
    /// Whether the runner is currently dueling with this target.
    is_dueling: bool,
    /// Time spent engaging with this target in seconds.
    engagement_time: f64,
}

/// State related to order (rank) in the race.
pub struct OrderState {
    /// Current order (rank), 1-based.
    pub order: u8,
    /// Current order rate (percentile; "you are in the top X%").
    pub rate: u8,
    /// Maximum order rate achieved so far.
    /// N.B. This value is calculated only after 5 seconds into the race.
    pub rate_max: u8,
    /// Minimum order rate achieved so far.
    /// N.B. This value is calculated only after 5 seconds into the race.
    pub rate_min: u8,
    /// Change in order since last update.
    pub delta: i8,
    /// Number of overtakes since entering Mid phase.
    pub overtakes_since_mid: u16,
    /// Number of overtakes since entering Late phase.
    pub overtakes_since_late: u16,
    /// Number of overtakes since entering last corner.
    pub overtakes_since_last_corner: u16,
}

impl Default for OrderState {
    fn default() -> Self {
        OrderState {
            order: 1,
            rate: 0,
            rate_max: 0,
            rate_min: 100,
            delta: 0,
            overtakes_since_mid: 0,
            overtakes_since_late: 0,
            overtakes_since_last_corner: 0,
        }
    }
}

/// Distance differences from other runners.
#[derive(Default)]
pub struct PositionDiff {
    /// Distance difference from the first-place runner in meters.
    pub from_first: f64,
    /// Distance difference from the next runner ahead in meters.
    pub from_next: f64,
    /// Distance difference from the previous runner behind in meters.
    pub from_prev: f64,
    /// Distance difference rate from the first-place runner relative to the distance
    /// between first and last place, in percent.
    pub rate_from_first: f64,
}

/// Rush (temptation, 掛かり) counters.
#[derive(Default)]
pub struct RushCounter {
    /// Number of runners currently in rush state ahead.
    pub ahead: u8,
    /// Number of runners currently in rush state behind.
    pub behind: u8,
}

/// Lane state including current and target lane positions in meters.
pub struct LaneState {
    /// Current lane position in meters.
    pub current: f64,
    /// Target lane position in meters.
    pub target: f64,
}

impl LaneState {
    pub fn new(lane: f64) -> Self {
        Self {
            current: lane,
            target: lane,
        }
    }
}

/// Surrounding state information.
#[derive(Default)]
pub struct SurroundState {
    pub front: bool,
    pub back: bool,
    pub side: bool,
}

/// Blocking state information.
#[derive(Default)]
pub struct BlockedState {
    /// Speed limit imposed by front blocking, if any.
    /// This also determines whether the runner is front-blocked.
    pub speed_limit: Option<f64>,
    /// Whether the runner is currently side-blocked.
    pub side: bool,
    /// Time spent being front-blocked in seconds.
    pub front_time: f64,
    /// Time spent being side-blocked in seconds.
    pub side_time: f64,
}

/// Information about nearby runners.
#[derive(Default)]
pub struct NearbyState {
    /// Total number of nearby runners.
    pub count: u8,
    /// Number of nearby runners ahead.
    pub ahead_count: u8,
    /// Whether the runner behind is in a lane that's closer to the center.
    pub behind_in: bool,
    /// Number of nearby runners in the same lane ahead.
    pub ahead_same_lane_count: u8,
    /// Time spent near runners in the same lane ahead in seconds.
    pub ahead_same_lane_time: f64,
    /// Number of nearby runners in the same lane behind.
    pub behind_same_lane_count: u8,
    /// Time spent near runners in the same lane behind in seconds.
    pub behind_same_lane_time: f64,
    /// Number of runners behind within a reasonable distance.
    /// Used to calculate variable `behind_near_lane_time_set1`.
    pub behind_set1_count: u8,
    /// Time spent near runners behind within a reasonable distance in seconds.
    /// Maps to variable `behind_near_lane_time_set1`.
    pub behind_set1_time: f64,
}

/// Skill activation flags for the current update cycle.
#[derive(Default)]
pub struct SkillActivationFlags {
    /// Flag indicating if a skill was activated in the current update cycle.
    pub any_activated: bool,
    /// Flag indicating if a healing skill was activated in the current update cycle.
    pub heal_activated: bool,
    /// Flag indicating if a buff skill was activated in the current update cycle.
    pub buff_activated: HashSet<SkillType>,
}

/// An Uma with additional data related to racing, that is, an Uma who is running.
///
/// N.B. This structure should be created from an `UnreadyRunner` via `RaceSolver`.
pub struct Runner {
    /// Base stats before adjustments. After calculating the adjusted stats,
    /// the base stats are only used to evaluate some skill conditions.
    pub base: Uma,
    /// Adjusted stats after applying modifiers and out-of-gate skills.
    /// For the most part, these are the stats used for race calculations.
    pub adjusted: Uma,
    /// Runner motivation level. After applying motivation modifiers to stats,
    /// this field is only used to evaluate some skill conditions.
    pub motivation: Motivation,
    /// Gate number (0-indexed). It determines starting lane position.
    pub gate_roll: u8,
    /// Gate bracket (0-7, from innermost to outermost).
    /// Used to evaluate some skill conditions.
    pub gate_block: u8,
    /// Start delay (i.e. time taken to leave the gate) in seconds.
    pub start_delay: f64,
    /// Current speed in m/s. Besides position updates, it also affects HP consumption.
    pub speed: f64,
    /// Current acceleration in m/s².
    pub acceleration: f64,
    /// Current position in the course in meters.
    pub position: f64,
    /// Current HP. It is set to `max_hp` at the start of the race and decreases
    /// over time. It can be restored by skills but cannot exceed `max_hp`.
    ///
    /// If HP reaches 0, the runner's maximum speed is greatly reduced.
    pub hp: f64,
    /// Maximum HP. Calculated from the runner's stamina stat at the start of the race.
    pub max_hp: f64,
    /// Flag indicating if the runner has emptied her HP at least once.
    pub has_emptied_hp: bool,
    /// Minimum speed in m/s. It is based on course base speed and guts stat.
    pub min_speed: f64,
    /// Base target speed in m/s before modifiers.
    pub base_target_speed: f64,
    /// Lane state including current and target lane positions in meters.
    pub lane: LaneState,
    /// Speed modifier for the current section in m/s.
    pub section_speed_modifier: f64,
    /// List of skills available for activation.
    pub available_skills: Vec<Skill>,
    // Elapsed time when the runner finished the race, in seconds.
    pub finish_time: Option<f64>,
    /// List of skills that have been activated during the race.
    pub activated_skills: Vec<SkillActivationRecord>,
    /// Set of activated skill IDs.
    pub activated_skill_ids: HashSet<String>,
    /// Set of activated skill alternatives (skill_index, alternative_index).
    pub activated_alternatives: HashSet<(usize, usize)>,
    /// Modifiers from skills applied to this runner's stats.
    pub skill_modifiers: SkillModifiers,
    /// Effects from skills that are currently active for a limited duration.
    pub active_timed_effects: Vec<ActiveTimedEffect>,
    /// List of indices of golden skills in `available_skills`.
    /// These skills can be activated by other skills.
    pub golden_skill_indices: Vec<usize>,
    /// Chance of entering rush (temptation, 掛かり) state.
    /// This is determined at the start of the race along with deciding whether
    /// the runner will rush or not.
    pub rush_chance: f64,
    /// State related to rushing (temptation, 掛かり). If `None`, the runner is set to
    /// not rush at all. Otherwise, it tells when and how the rush will happen.
    pub rush_state: Option<RushState>,
    /// Flag indicating if the runner is currently in start dash mode.
    pub is_start_dash: bool,
    /// State related to spot struggle. If `None`, the runner is not in a struggle.
    pub spot_struggle: Option<StruggleState>,
    /// State related to dueling. If `None`, the runner is not currently dueling.
    pub dueling: Option<DuelingState>,
    /// Duel targets being tracked for potential dueling. Gate number is used as key.
    pub duel_targets: HashMap<u8, DuelTarget>,
    /// Count of duels the runner has engaged in.
    pub duel_count: u8,
    /// State related to order (rank) in the race.
    pub order: OrderState,
    /// Distance differences between this runner and others.
    pub position_diff: PositionDiff,
    /// Furthest distance at which the runner can see other runners, in meters.
    pub visible_distance: f64,
    /// Number of visible runners within `visible_distance`.
    pub visible_count: u8,
    /// Information about nearby runners.
    pub nearby: NearbyState,
    /// Blocking state information.
    pub blocked: BlockedState,
    /// Surrounding state information.
    pub surrounded: SurroundState,
    /// Number of runners currently in rush (temptation, 掛かり) state ahead and behind.
    pub rush_counter: RushCounter,
    /// Current slope acceleration in m/s², if any.
    pub slope_acceleration: Option<f64>,
    /// Current section index (0-23).
    pub section: u8,
    /// Current race phase.
    pub phase: Phase,
    /// Information about the last spurt phase. It is calculated when entering the
    /// Mid phase. It indicates when the last spurt can start and its max speed.
    pub last_spurt: Option<LastSpurt>,
    /// Reference to a `RaceSolver`'s Random Number Generator.
    /// It is shared with other runners.
    pub rng: Rc<RefCell<rand_pcg::Pcg64Mcg>>,
    /// Structure used to determine corner entries and exits, along with skills
    /// triggered by those events.
    pub corner_manager: CornerManager,
    /// Structure used to determine slope entries and exits, along with their
    /// acceleration/deceleration effects as well as skills triggered by slopes.
    pub slope_manager: SlopeManager,
    /// Structure used to determine straight entries and exits, along with skills
    /// triggered by those events.
    pub straight_manager: StraightManager,
    /// Scheduler for skill triggers. It contains many queues for different trigger
    /// types, each with their own criteria (distance, time, etc).
    pub trigger_scheduler: TriggerScheduler,
    /// Skill activation flags for the current update cycle.
    pub activation_flags: SkillActivationFlags,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum StartType {
    Good,
    Normal,
    Late,
}

impl Runner {
    /// Create a new Runner from an UnreadyRunner and RaceState.
    pub fn new(mut builder: UnreadyRunner, state: &RaceState) -> Self {
        // Apply out-of-gate skills with proper activation tracking
        builder.apply_out_of_gate_skills(state);

        // Build final Runner with all modifications applied
        builder.build(&state.course)
    }

    /// Determine the lane type based on target lane position.
    pub fn lane_type(&self, course: &Course) -> LaneType {
        let percent = self.lane.current / course.width();
        if percent <= 0.2 {
            LaneType::Inner
        } else if percent <= 0.4 {
            LaneType::Middle
        } else if percent <= 0.6 {
            LaneType::Outer
        } else {
            LaneType::Outside
        }
    }

    /// Determine the type of start based on start delay:
    ///
    /// - Good Start: start delay < 0.02s; gives bonus points in Team Trials.
    /// - Normal Start: 0.02s <= start delay <= 0.08s.
    /// - Late Start: start delay > 0.08s; used in some skill conditions.
    pub fn start_type(&self) -> StartType {
        match self.start_delay {
            d if d < 0.02 => StartType::Good,
            d if d > 0.08 => StartType::Late,
            _ => StartType::Normal,
        }
    }

    /// Check if the runner is currently in position keep pace down mode.
    pub fn is_pace_down(&self) -> bool {
        // TODO: implement
        false
    }

    /// Update acceleration based on current speed and target speed.
    pub fn update_acceleration(&mut self) {
        if self.speed < self.target_speed() {
            self.accelerate();
        } else {
            self.decelerate();
        }
    }

    /// Calculate increasing acceleration.
    fn accelerate(&mut self) {
        let base_accel = if self.is_going_uphill() {
            0.0004
        } else {
            0.0006
        };

        let power_component = (500.0 * self.adjusted.power as f64).sqrt();
        let strategy_multiplier =
            acceleration_strategy_phase_multiplier(self.adjusted.style, self.phase);
        let ground_multiplier = ground_type_proficiency_multiplier(self.adjusted.track_aptitude);
        let distance_multiplier =
            acceleration_distance_proficiency_multiplier(self.adjusted.distance_aptitude);

        let base_acceleration = base_accel
            * power_component
            * strategy_multiplier
            * ground_multiplier
            * distance_multiplier;

        let start_dash_modifier = if self.is_start_dash { 24.0 } else { 0.0 };

        let dueling_modifier = self
            .dueling
            .as_ref()
            .map_or(0.0, |d| d.acceleration_modifier);

        self.acceleration = base_acceleration
            + self.skill_modifiers.acceleration
            + start_dash_modifier
            + dueling_modifier;
    }

    /// Calculate decreasing acceleration (i.e., deceleration).
    fn decelerate(&mut self) {
        self.acceleration = if self.hp <= 0.0 {
            -1.2
        } else if self.is_pace_down() {
            -0.5
        } else {
            match self.phase {
                Phase::Early => -1.2,
                Phase::Mid => -0.8,
                Phase::Late | Phase::Last => -1.0,
            }
        };
    }

    /// Check if the runner is currently experiencing downhill acceleration.
    pub fn is_accelerating_downhill(&self) -> bool {
        self.slope_acceleration.is_some_and(|s| s < 0.0)
    }

    /// Check if the runner is going uphill, thus experiencing uphill deceleration.
    pub fn is_going_uphill(&self) -> bool {
        self.slope_acceleration.is_some_and(|s| s > 0.0)
    }

    /// Start downhill acceleration based on slope percent.
    /// N.B. Going downhill does not guarantee acceleration.
    fn start_downhill_acceleration(&mut self, slope: f64) {
        if slope > 0.0 {
            panic!("Not a downhill slope");
        }
        self.slope_acceleration = Some(-slope / 10.0 + 0.3);
    }

    /// Start uphill deceleration based on slope percent.
    /// This should always happen while on an uphill.
    fn start_uphill_deceleration(&mut self, slope: f64) {
        self.slope_acceleration = Some(-slope * 200.0 / self.adjusted.power as f64);
    }

    /// Stop any slope-based acceleration or deceleration.
    fn stop_slope_acceleration(&mut self) {
        self.slope_acceleration = None;
    }

    /// Get the effective speed including skill modifiers.
    /// This is the actual speed used for position calculations.
    fn effective_speed(&self) -> f64 {
        self.speed + self.skill_modifiers.current_speed + self.section_speed_modifier
    }

    /// Update the runner's speed based on acceleration and constraints.
    pub fn update_speed(&mut self, delta_time: f64, course_base_speed: f64) {
        // Update speed based on acceleration
        self.speed += self.acceleration * delta_time;

        // Clamp to target speed
        if self.speed > self.target_speed() {
            self.speed = self.target_speed();
        }

        // Handle start dash end and min speed
        if self.is_start_dash && self.speed >= 0.85 * course_base_speed {
            self.is_start_dash = false;
        } else if !self.is_start_dash && self.speed < self.min_speed {
            self.speed = self.min_speed;
        }
    }

    /// Calculate the target speed including all modifiers.
    fn target_speed(&self) -> f64 {
        let dueling_modifier = self
            .dueling
            .as_ref()
            .map_or(0.0, |d| d.target_speed_modifier);
        self.base_target_speed
            + self.slope_acceleration.unwrap_or(0.0)
            + self.skill_modifiers.target_speed
            + dueling_modifier
    }

    /// Update the base target speed based on current phase and conditions.
    pub fn update_target_speed(&mut self, course_base_speed: f64) {
        self.base_target_speed = if let Some(last_spurt) = &self.last_spurt
            && self.position >= last_spurt.target_distance
        {
            last_spurt.max_speed
        } else {
            let struggle_mod = self.spot_struggle.as_ref().map_or(0.0, |s| s.modifier);
            base_target_speed(
                course_base_speed,
                self.adjusted.style,
                self.phase,
                self.adjusted.speed,
                self.adjusted.distance_aptitude,
            ) + struggle_mod
        };
    }

    /// Update the runner's position based on effective current speed.
    pub fn update_position(&mut self, delta_time: f64, state: &RaceState) {
        // Adjust delta time if the start of the frame is right before start delay
        // (By calling this function, we are guaranteed that the runner should have started on this frame)
        let delta_time = if (state.elapsed_time - delta_time) < self.start_delay {
            delta_time - self.start_delay
        } else {
            delta_time
        };
        let prev_position = self.position;
        // Use effective_speed which includes current_speed modifiers
        self.position += self.effective_speed() * delta_time;
        if self.position >= state.course.distance as f64 {
            // Calculate exact finish time
            let remaining_distance = state.course.distance as f64 - prev_position;
            let time_to_finish = remaining_distance / self.effective_speed();
            self.finish_time = Some(state.elapsed_time - delta_time + time_to_finish);
        }
        self.check_distance_triggers(state);
    }

    /// Check if the runner has finished the race.
    pub fn has_finished(&self) -> bool {
        self.finish_time.is_some()
    }

    /// Update the current section and phase based on position.
    pub fn update_phase(&mut self, state: &mut RaceState) {
        let prev_section = self.section;
        self.section = state.course.section_from_position(self.position);
        if self.section != prev_section {
            // Update section speed modifier
            self.section_speed_modifier = roll_section_speed_modifier(
                &mut self.rng.borrow_mut(),
                self.adjusted.wiz,
                state.course.base_speed(),
            );

            // Start rush if conditions are met
            if let Some(rush_state) = &mut self.rush_state
                && rush_state.section == self.section
                && !rush_state.is_rushed
                && rush_state.end_time < state.elapsed_time
            {
                self.start_rush(state);
            }

            // Stop spot struggle if 9th section is entered
            if self.section == 8 {
                self.stop_spot_struggle(state.elapsed_time);
            }

            // Phase can only advance at section boundaries
            let prev_phase = self.phase;
            self.phase = Phase::from_position(self.position, &state.course);
            if prev_phase != self.phase {
                self.check_phase_triggers(state);
                if self.phase >= Phase::Mid && self.last_spurt.is_none() {
                    calculate_last_spurt(self, &state.course, state.params.condition);
                }
            }
        }
    }

    /// Get current HP percentage.
    ///
    /// HP determines if the runner can sustain her target speed, decelerating
    /// if it reaches 0.
    /// It's also used to get the distance at which her last spurt can start.
    pub fn hp_percent(&self) -> f64 {
        self.hp / self.max_hp * 100.0
    }

    /// Update HP based on current speed, phase, and modifiers.
    pub fn update_hp(&mut self, delta_time: f64, state: &RaceState) {
        let mut multiplier = if let Some(struggle) = &self.spot_struggle
            && state.elapsed_time < struggle.expires_at
        {
            if self.adjusted.style == Style::Runaway {
                if self.is_rushed() { 7.7 } else { 3.5 }
            } else if self.adjusted.style == Style::Front {
                if self.is_rushed() { 3.6 } else { 1.4 }
            } else {
                panic!("BUG: spot struggle should only apply to Runaway and Front styles");
            }
        } else if self.is_rushed() {
            1.5
        } else {
            1.0
        };
        if self.is_pace_down() {
            multiplier *= 0.6;
        }
        if self.is_accelerating_downhill() {
            multiplier *= 0.4;
        }
        self.hp -= calculate_hp_consumption_per_second(
            self.speed,
            &state.course,
            state.params.condition,
            self.phase,
            self.adjusted.guts,
            multiplier,
        ) * delta_time;
        if self.hp <= 0.0 {
            self.has_emptied_hp = true;
        }
        // Exit dueling state if HP is below 5%
        if self.hp_percent() < 5.0 {
            self.dueling = None;
        }
        self.check_hp_triggers(state);
    }

    /// Enter rush (temptation, 掛かり) state.
    ///
    /// The runner will move ahead of the position dictated by their style,
    /// losing HP at an increased rate.
    fn start_rush(&mut self, state: &mut RaceState) {
        if let Some(rush_state) = &mut self.rush_state
            && !rush_state.is_rushed
        {
            rush_state.is_rushed = true;
            rush_state.start_time = state.elapsed_time;
            // Rushed for at least 3 seconds
            rush_state.end_time = state.elapsed_time + 3.0;
            *state.rushed_by_style.entry(self.base.style).or_insert(0) += 1;
            // TODO: implement position strat change
        }
    }

    /// Exit rush (temptation, 掛かり) state.
    fn end_rush(&mut self, state: &mut RaceState) {
        if let Some(rush_state) = &mut self.rush_state
            && rush_state.is_rushed
        {
            rush_state.is_rushed = false;
            rush_state.end_time = state.elapsed_time; // Ensure end time is current
            state
                .rushed_by_style
                .entry(self.base.style)
                .and_modify(|e| *e -= 1);
        }
    }

    /// Update rush state based on elapsed time and random chance.
    pub fn update_rush_state(&mut self, state: &mut RaceState) {
        if let Some(rush_state) = &mut self.rush_state
            && rush_state.is_rushed
        {
            if state.elapsed_time >= rush_state.start_time + rush_state.max_duration {
                // End rush after max duration
                self.end_rush(state);
                return;
            }
            if state.elapsed_time >= rush_state.end_time {
                // 55% chance of snapping out
                let roll: f64 = self.rng.borrow_mut().random_range(0.0..100.0);
                if roll < 55.0 {
                    self.end_rush(state);
                } else {
                    // Extend rush by 3 seconds
                    rush_state.end_time = state.elapsed_time + 3.0;
                }
            }
        }
    }

    /// Check if the runner is currently in a rushed state.
    pub fn is_rushed(&self) -> bool {
        self.rush_state.as_ref().is_some_and(|s| s.is_rushed)
    }

    /// Check if the runner has rushed at some point.
    pub fn has_rushed(&self) -> bool {
        self.rush_state.as_ref().is_some_and(|s| s.start_time > 0.0)
    }

    /// Check if a spot struggle (位置取り争い) can occur with another runner.
    pub fn can_spot_struggle(
        &self,
        other: &Self,
        distance: f64,
        lane_diff: f64,
        course: &Course,
    ) -> bool {
        if self.position <= 150.0 || other.position <= 150.0 {
            return false;
        }
        if self.section > 6 || other.section > 6 {
            return false;
        }
        match self.adjusted.style {
            Style::Front => {
                matches!(other.adjusted.style, Style::Front)
                    && course.is_near_by(distance, lane_diff, 3.75, 0.165)
            }
            Style::Runaway => {
                matches!(other.adjusted.style, Style::Runaway)
                    && course.is_near_by(distance, lane_diff, 5.0, 0.416)
            }
            _ => false,
        }
    }

    /// Start a spot struggle (Lead competition, 位置取り争い).
    ///
    /// When there are 2 or more front runners early in the race,
    /// a spot struggle may begin.
    ///
    /// During a spot struggle, runners gain a speed boost based on their guts
    /// stat, but also consume HP at a higher rate.
    pub fn start_spot_struggle(&mut self, elapsed_time: f64) {
        if self.is_spot_struggling() {
            return;
        }
        let modifier = (500.0 * self.adjusted.guts as f64).powf(0.6) * 0.0001;
        let duration = (700.0 * self.adjusted.guts as f64).sqrt() * 0.012;
        self.spot_struggle = Some(StruggleState {
            modifier,
            expires_at: elapsed_time + duration,
        });
    }

    /// Stop the spot struggle (位置取り争い).
    pub fn stop_spot_struggle(&mut self, elapsed_time: f64) {
        if let Some(struggle) = &mut self.spot_struggle {
            struggle.expires_at = elapsed_time; // Set to expire immediately
        }
    }

    /// Check if the runner is currently in a spot struggle (位置取り争い).
    pub fn is_spot_struggling(&self) -> bool {
        self.spot_struggle.is_some()
    }

    /// Uncheck all duel targets at the start of an update cycle.
    pub fn uncheck_duel_targets(&mut self) {
        for target in self.duel_targets.values_mut() {
            target.is_checked = false;
        }
    }

    /// Update duel target engagement time and start dueling if conditions are met.
    pub fn update_duel_target(&mut self, target: &Self, delta_time: f64) {
        let target_entry = self
            .duel_targets
            .entry(target.gate_roll)
            .or_insert(DuelTarget {
                is_checked: true,
                is_dueling: false,
                engagement_time: 0.0,
            });
        target_entry.engagement_time += delta_time;

        if !target_entry.is_dueling
            && target_entry.engagement_time > 2.0
            && target.order.rate <= 50
            && (self.speed - target.speed).abs() < 0.6
        {
            target_entry.is_dueling = true;
            self.start_dueling();
        }
    }

    /// Remove duel targets that were not checked in the current update cycle.
    pub fn filter_duel_targets(&mut self) {
        self.duel_targets.retain(|_k, v| v.is_checked);
    }

    /// Start a duel (Compete Fight, 追い比べ).
    ///
    /// On the final straight, a duel may occur between multiple runners that are
    /// close together.
    ///
    /// During a duel, runners gain target speed and acceleration boosts based on
    /// their guts stat.
    pub fn start_dueling(&mut self) {
        let target_speed_modifier = (200.0 * self.adjusted.guts as f64).powf(0.708) * 0.0001;
        let acceleration_modifier = (160.0 * self.adjusted.guts as f64).powf(0.59) * 0.0001;
        self.dueling = Some(DuelingState {
            target_speed_modifier,
            acceleration_modifier,
        });
        self.duel_count += 1;
    }

    /// Check if the runner is currently dueling (追い比べ).
    pub fn is_dueling(&self) -> bool {
        self.dueling.is_some()
    }

    /// Get the current corner index if in a corner.
    pub fn current_corner(&self) -> Option<usize> {
        self.corner_manager.current_corner()
    }

    /// Check for corner entry and update state.
    pub fn check_corner_start(&mut self, corners: &[Corner]) {
        self.corner_manager.check_entry(self.position, corners);
    }

    /// Check for corner exit and update state.
    pub fn check_corner_end(&mut self, corners: &[Corner]) {
        self.corner_manager.check_exit(self.position, corners);
    }

    /// Get the current straight index if on a straight.
    pub fn current_straight(&self) -> Option<usize> {
        self.straight_manager.current_straight()
    }

    /// Check for straight entry and trigger skills if applicable.
    pub fn check_straight_start(&mut self, straights: &[Straight], state: &RaceState) {
        if let Some(straight_idx) = self.straight_manager.check_entry(self.position, straights) {
            // Trigger straight entry skills
            // Note: straight triggers are only for entry, not exit
            let skill_indices: Vec<usize> = self
                .trigger_scheduler
                .straight_triggers
                .iter()
                .filter(|t| t.straight_index == straight_idx)
                .flat_map(|t| t.logic_indices.iter().copied())
                .collect();

            for skill_index in skill_indices {
                self.try_activate_skill(skill_index, state);
            }
        }
    }

    /// Check for straight exit and update state.
    pub fn check_straight_end(&mut self, straights: &[Straight]) {
        // Only check; no triggers are fired by this event
        self.straight_manager.check_exit(self.position, straights);
    }

    /// Get the current slope index if on a slope.
    pub fn current_slope(&self) -> Option<usize> {
        self.slope_manager.current_slope()
    }

    /// Update slope state and apply slope-based acceleration/deceleration.
    pub fn update_slope(&mut self, slopes: &[Slope], rng: &mut impl Rng) {
        if let Some((slope_idx, _slope_type)) =
            self.slope_manager.check_entry(self.position, slopes)
        {
            if slopes[slope_idx].is_uphill() {
                self.start_uphill_deceleration(slopes[slope_idx].slope_percent());
            }
        } else if let Some(_slope_idx) = self.slope_manager.check_exit(self.position, slopes) {
            self.stop_slope_acceleration();
        } else if let Some(idx) = self.current_slope() {
            // On slope - check for downhill acceleration
            if slopes[idx].is_downhill() {
                let has_second_passed = false; // TODO: track time on slope
                if has_second_passed {
                    if self.is_accelerating_downhill() {
                        // 20% chance to stop each second
                        let roll: f64 = rng.random();
                        if roll < 0.2 {
                            self.stop_slope_acceleration();
                        }
                    } else {
                        let roll: f64 = rng.random();
                        let chance = self.adjusted.wiz as f64 * 0.0004;
                        if roll < chance {
                            self.start_downhill_acceleration(slopes[idx].slope_percent());
                        }
                    }
                }
            }
        }
    }

    /// Update order (rank) and overtakes based on current order.
    pub fn update_order(&mut self, current_order: u8, course: &Course) {
        let prev_order = self.order.order;
        self.order.delta = prev_order as i8 - current_order as i8;
        self.order.order = current_order;
        if self.order.delta < 0 {
            if self.phase >= Phase::Mid {
                self.order.overtakes_since_mid += self.order.delta.unsigned_abs() as u16;
            }
            if self.phase >= Phase::Late && self.order.delta < 0 {
                self.order.overtakes_since_late += self.order.delta.unsigned_abs() as u16;
            }
            if course
                .corners
                .last()
                .is_some_and(|x| self.position >= x.start)
            {
                self.order.overtakes_since_last_corner += 1;
            }
        }
    }

    /// Update order rate (percentage rank) based on total runners.
    /// If `should_update_range` is true, also update the min/max order rates.
    pub fn update_order_rate(&mut self, total_runners: u8, should_update_range: bool) {
        let rate = ((self.order.order as f64 / total_runners as f64) * 100.0).ceil() as u8;
        self.order.rate = rate;
        if should_update_range {
            self.order.rate_max = self.order.rate_max.max(rate);
            self.order.rate_min = self.order.rate_min.min(rate);
        }
    }

    /// Update position differences from other runners ahead.
    ///
    /// `position_first`: position of the first-place runner.
    /// `position_next`: position of the next runner ahead.
    /// `position_range`: distance between first and last place.
    pub fn update_diff_from_ahead(
        &mut self,
        position_first: f64,
        position_next: f64,
        position_range: f64,
        rushed_ahead: u8,
    ) {
        self.position_diff.from_first = position_first - self.position;
        self.position_diff.from_next = position_next - self.position;
        self.position_diff.rate_from_first = self.position_diff.from_first / position_range * 100.0;
        self.rush_counter.ahead = rushed_ahead;
    }

    /// Update position difference from the runner behind.
    pub fn update_diff_from_behind(&mut self, position_prev: f64, rushed_behind: u8) {
        self.position_diff.from_prev = self.position - position_prev;
        self.rush_counter.behind = rushed_behind;
    }

    /// Check if another runner is visible to this runner.
    pub fn is_visible(&self, other: &Self, course: &Course) -> bool {
        let distance = (self.position - other.position).abs();
        distance <= self.visible_distance
            && (self.lane.current - other.lane.current).abs()
                <= ((distance / self.visible_distance) * 11.5 + 2.0) * course.lane_width() / 2.0
    }

    /// Update front-blocked state for this runner based on another runner.
    pub fn update_front_blocked_by(&mut self, other: &Self) {
        let distance = (other.position - self.position).abs();
        self.blocked.speed_limit = Some((0.988 + 0.012 * distance) * other.speed);
    }

    /// Update side-blocked state for this runner and another.
    pub fn update_side_blocked_by(&mut self, other: &mut Self) {
        self.blocked.side = true;
        other.blocked.side = true;
    }

    /// Update front-surrounded state for this runner and another.
    pub fn update_front_surrounded_by(&mut self, other_front: &mut Self) {
        self.surrounded.front = true;
        other_front.surrounded.back = true;
    }

    /// Update back-surrounded state for this runner and another.
    pub fn update_side_surrounded_by(&mut self, other: &mut Self) {
        self.surrounded.side = true;
        other.surrounded.side = true;
    }

    /// Update nearby state for this runner and another in the same lane ahead.
    pub fn update_near_same_lane_behind(&mut self, other: &mut Self) {
        self.nearby.ahead_same_lane_count += 1;
        other.nearby.behind_same_lane_count += 1;
    }

    /// Update nearby state for this runner and another ahead.
    pub fn update_near_behind(&mut self, other: &mut Self) {
        // Both runners are near each other
        self.nearby.count += 1;
        other.nearby.count += 1;
        // Other runner is in front of this runner
        self.nearby.ahead_count += 1;
    }

    /// Check if this runner is front-blocked.
    pub fn is_front_blocked(&self) -> bool {
        self.blocked.speed_limit.is_some()
    }

    /// Check if this runner is surrounded on all sides.
    pub fn is_surrounded(&self) -> bool {
        self.surrounded.front && self.surrounded.back && self.surrounded.side
    }

    /// Reset blocking status for the next update cycle.
    pub fn reset_blocking_status(&mut self) {
        self.visible_count = 0;
        self.nearby.count = 0;
        self.nearby.ahead_count = 0;
        self.nearby.behind_in = false;
        self.blocked.speed_limit = None;
        self.blocked.side = false;
        self.surrounded = SurroundState::default();
    }

    /// Update nearby time counters based on current nearby counts.
    pub fn update_near_time(&mut self, delta_time: f64) {
        if self.nearby.ahead_same_lane_count > 0 {
            self.nearby.ahead_same_lane_time += delta_time;
        } else {
            self.nearby.ahead_same_lane_time = 0.0;
        }
        if self.nearby.behind_same_lane_count > 0 {
            self.nearby.behind_same_lane_time += delta_time;
        } else {
            self.nearby.behind_same_lane_time = 0.0;
        }
        if self.nearby.behind_set1_count > 0 {
            self.nearby.behind_set1_time += delta_time;
        } else {
            self.nearby.behind_set1_time = 0.0;
        }
    }

    /// Update blocked time counters based on current blocking status.
    pub fn update_blocked_time(&mut self, delta_time: f64) {
        if self.blocked.speed_limit.is_some() {
            self.blocked.front_time += delta_time;
        } else {
            self.blocked.front_time = 0.0;
        }
        if self.blocked.side {
            self.blocked.side_time += delta_time;
        } else {
            self.blocked.side_time = 0.0;
        }
    }

    /// Check all preconditions to detect satisfaction at any point during the race.
    /// This must be called every frame to implement proper "one-time unlock" semantics
    /// where preconditions can be satisfied at any time, not just when triggers fire.
    pub fn check_all_preconditions(&mut self, state: &RaceState) {
        // Iterate through all skills and check their preconditions
        let skill_count = self.trigger_scheduler.skill_logics.len();
        for trigger_index in 0..skill_count {
            self.check_precondition(trigger_index, state);
        }
    }

    /// Check if precondition is satisfied and mark as unlocked if true.
    fn check_precondition(&mut self, trigger_index: usize, state: &RaceState) {
        // Skip if precondition already satisfied
        if self.trigger_scheduler.precondition_satisfied[trigger_index] {
            return;
        }

        // Evaluate precondition
        let logic = &self.trigger_scheduler.skill_logics[trigger_index];
        let precondition = logic.precondition.clone();

        if let Some(precond) = precondition
            && evaluate_condition(&precond, &EvaluatorFull, Some(logic), state, self)
        {
            self.trigger_scheduler.precondition_satisfied[trigger_index] = true;
        }
    }

    /// Check continuous triggers every frame.
    pub fn check_continuous_triggers(&mut self, state: &RaceState) {
        // Clone indices to avoid borrow conflicts
        let continuous_indices: Vec<usize> = self.trigger_scheduler.continuous_triggers.to_vec();

        for trigger_index in continuous_indices {
            self.try_activate_skill(trigger_index, state);
        }
    }

    /// Reset activation flags at the start of each frame.
    pub fn reset_activation_flags(&mut self) {
        self.activation_flags = SkillActivationFlags::default();
    }

    /// Extract activated buff skills from a runner and update race state.
    pub fn extract_buff_activations(&mut self, state: &mut RaceState) {
        state
            .buffs_activated
            .extend(self.activation_flags.buff_activated.drain());
    }

    /// Check activation-dependent triggers.
    /// This should be called at the end of each frame after all other triggers
    /// have been checked.
    pub fn check_activation_dependent_triggers(&mut self, state: &RaceState) {
        // Pass 1: Check is_activate_any_skill triggers
        if self.activation_flags.any_activated {
            let any_activation_indices = self.trigger_scheduler.any_activation_triggers.to_vec();
            for trigger_index in any_activation_indices {
                self.try_activate_skill(trigger_index, state);
            }
        }
    }

    /// Check activation-dependent triggers.
    /// This should be called at the end of each frame after all other triggers
    /// (and any-skill acivation-dependent triggers) have been checked.
    pub fn check_buff_dependent_triggers(&mut self, state: &RaceState) {
        // Pass 2: Check is_other_character_activate_advantage_skill triggers
        if self.activation_flags.heal_activated {
            let buff_activation_indices = self.trigger_scheduler.buff_activation_triggers.to_vec();
            for trigger_index in buff_activation_indices {
                self.try_activate_skill(trigger_index, state);
            }
        }

        // Pass 3: Check is_activate_heal_skill triggers
        if self.activation_flags.heal_activated {
            let heal_activation_indices = self.trigger_scheduler.heal_activation_triggers.to_vec();
            for trigger_index in heal_activation_indices {
                self.try_activate_skill(trigger_index, state);
            }
        }
    }

    pub fn check_phase_triggers(&mut self, state: &RaceState) {
        while let Some(trigger) = self.trigger_scheduler.phase_triggers.peek() {
            if self.phase != trigger.phase {
                break; // Wrong phase
            }

            let trigger = self.trigger_scheduler.phase_triggers.pop().unwrap();
            for trigger_index in trigger.logic_indices {
                self.try_activate_skill(trigger_index, state);
            }
        }
    }

    pub fn check_distance_triggers(&mut self, state: &RaceState) {
        while let Some(trigger) = self.trigger_scheduler.distance_triggers.peek() {
            if !Self::should_fire_trigger(self.position, trigger.threshold_meters, trigger.operator)
            {
                break; // Threshold not reached yet, this and all subsequent triggers require further distance
            }

            let trigger = self.trigger_scheduler.distance_triggers.pop().unwrap();
            let trigger_index = trigger.logic_idx;
            self.try_activate_skill(trigger_index, state);
        }
    }

    pub fn check_hp_triggers(&mut self, state: &RaceState) {
        while let Some(trigger) = self.trigger_scheduler.hp_triggers.peek() {
            if !Self::should_fire_trigger(
                self.hp_percent(),
                trigger.threshold_percent,
                trigger.operator,
            ) {
                break; // Threshold not reached yet, this and all subsequent triggers require lower HP
            }

            let trigger = self.trigger_scheduler.hp_triggers.pop().unwrap();
            let trigger_index = trigger.logic_idx;
            self.try_activate_skill(trigger_index, state);
        }
    }

    pub fn check_time_triggers(&mut self, state: &RaceState) {
        while let Some(trigger) = self.trigger_scheduler.time_triggers.peek() {
            if !Self::should_fire_trigger(
                state.elapsed_time,
                trigger.threshold_seconds,
                trigger.operator,
            ) {
                break; // Threshold not reached yet, this and all subsequent triggers require more time
            }

            let trigger = self.trigger_scheduler.time_triggers.pop().unwrap();
            let trigger_index = trigger.logic_idx;
            self.try_activate_skill(trigger_index, state);
        }
    }

    pub fn check_window_triggers(&mut self, state: &RaceState) {
        // Collect trigger checks to avoid borrowing conflicts
        let mut triggers_to_pull = Vec::new();
        let mut triggers_to_mark_attempted = Vec::new();

        for trigger in &self.trigger_scheduler.window_triggers {
            // Get current value for the window variable
            let current_value = match trigger.variable {
                WindowVariable::AccumulateTime => state.elapsed_time,
                WindowVariable::DistanceRate => self.position, // Already in meters (converted at build time)
                WindowVariable::HpPercent => self.hp_percent(),
                WindowVariable::SpatialPosition => self.position, // Spatial features use position directly
            };

            // Check if we're within the window
            if current_value >= trigger.min_threshold && current_value <= trigger.max_threshold {
                // Within window - try activation
                triggers_to_pull.push(trigger.logic_idx);
            } else if current_value > trigger.max_threshold {
                // Exited window without activating - check if we should mark as attempted
                let logic = &self.trigger_scheduler.skill_logics[trigger.logic_idx];
                if !logic.state.has_activated && !logic.state.onetime_trigger_attempted {
                    triggers_to_mark_attempted.push(trigger.logic_idx);
                }
            }
            // If current_value < min_threshold, we haven't entered the window yet (do nothing)
        }

        // Pull triggers
        for trigger_index in triggers_to_pull {
            self.try_activate_skill(trigger_index, state);
        }

        // Mark attempted
        for trigger_index in triggers_to_mark_attempted {
            self.trigger_scheduler.skill_logics[trigger_index].mark_attempted();
        }
    }

    /// Check if a monotonic trigger should fire based on operator.
    fn should_fire_trigger(current_value: f64, threshold: f64, operator: CompOp) -> bool {
        const EPSILON: f64 = 0.0001; // Tolerance for equality comparisons
        match operator {
            CompOp::GreaterEqual => current_value >= threshold,
            CompOp::LessEqual => current_value <= threshold,
            // Most, if not all triggers use greater/less or equal comparisons
            // Check all other operators for completeness
            CompOp::Greater => current_value > threshold,
            CompOp::Less => current_value < threshold,
            CompOp::Equal => (current_value - threshold).abs() < EPSILON,
            CompOp::NotEqual => (current_value - threshold).abs() >= EPSILON,
        }
    }

    /// Activate a skill by its index and alternative index.
    pub fn activate_skill(
        &mut self,
        skill_index: usize,
        alternative_index: usize,
        elapsed_time: f64,
        course: &Course,
        condition: TrackCondition,
    ) {
        // Clone the skill data we need to avoid borrow checker issues
        let skill_id = self.available_skills[skill_index].id.clone();
        let alternative =
            self.available_skills[skill_index].alternatives[alternative_index].clone();

        // Calculate scaled duration
        let base_duration = alternative.base_duration as f64 / 10000.0; // Convert from 10000ths of a second
        let scaled_duration = base_duration * self.skill_modifiers.duration_scaling;

        // Apply each effect in the alternative
        for effect in &alternative.effects {
            match effect.target {
                SkillTarget::Oneself => self.apply_skill_effect(
                    effect,
                    elapsed_time,
                    scaled_duration,
                    course,
                    condition,
                ),
                _ => continue, // TODO: Handle other targets
            }
        }

        // Recalculate skill modifiers from active effects
        self.recalculate_skill_modifiers();

        // Track skill activation
        self.activated_skill_ids.insert(skill_id.clone());
        self.activated_alternatives
            .insert((skill_index, alternative_index));
        self.activated_skills.push(SkillActivationRecord {
            skill_id,
            distance: self.position,
        });

        // Increment activation counts for condition evaluation
        let section_length = course.distance as f64 / 24.0;
        self.trigger_scheduler.activation_counts.increment(
            self.phase,
            self.position,
            section_length,
        );

        // Set activation flags for activation-dependent triggers
        self.activation_flags.any_activated = true;
        if self.is_heal_skill_by_index(skill_index) {
            self.activation_flags.heal_activated = true;
        }
    }

    /// Given a trigger index, attempt to activate the associated skill.
    fn try_activate_skill(&mut self, trigger_index: usize, state: &RaceState) {
        // Check if precondition is satisfied; skip if not
        if !self.trigger_scheduler.precondition_satisfied[trigger_index] {
            return;
        }

        // Get skill logic and conditions
        let logic = &mut self.trigger_scheduler.skill_logics[trigger_index];
        if !logic.should_check() {
            return;
        }
        let skill_index = logic.skill_index;
        let alternative_index = logic.alternative_index;
        let guards = logic.guards.clone();
        let is_onetime = logic.triggers.iter().any(|t| t.is_onetime());

        let should_activate = if guards.is_empty() {
            true // No guards - always activate when trigger fires
        } else {
            let logic = &self.trigger_scheduler.skill_logics[trigger_index];
            evaluate_comparison_list(&guards, &EvaluatorFull, Some(logic), state, self)
        };

        if should_activate {
            self.activate_skill(
                skill_index,
                alternative_index,
                state.elapsed_time,
                &state.course,
                state.params.condition,
            );
            self.trigger_scheduler.skill_logics[trigger_index].mark_activated();
        } else if is_onetime {
            // Onetime trigger that failed guards - mark attempted so we don't retry
            self.trigger_scheduler.skill_logics[trigger_index].mark_attempted();
        }
        // Note: Monotonic triggers that fail guards are NOT marked as attempted
        // This allows window triggers to retry them continuously while in window
    }

    /// Apply a single skill effect to the runner.
    ///
    /// Effects can be categorized as follows:
    /// - Timed effects (speed, acceleration...): modify a stat for a certain duration
    ///   that is constantly tracked.
    /// - Recovery: instant effect that recovers HP.
    pub fn apply_skill_effect(
        &mut self,
        effect: &SkillEffect,
        elapsed_time: f64,
        scaled_duration: f64,
        course: &Course,
        condition: TrackCondition,
    ) {
        let effect_type = &effect.skill_type;

        // Stat boosts and start delay modifications should have been applied out of gate.
        // Therefore, these are gracefully ignored here.

        // Timed effects (tracked in active_timed_effects)
        if is_timed_effect(effect_type) {
            let expires_at = elapsed_time + scaled_duration;
            self.active_timed_effects.push(ActiveTimedEffect {
                effect: effect.clone(),
                expires_at,
            });
        }
        // Recovery (HP restoration)
        else if matches!(effect_type, SkillType::Recovery) {
            self.hp = (self.hp + effect.modifier as f64).min(self.max_hp);

            // Increment heal skill count
            self.trigger_scheduler.activation_counts.increment_heal();

            // Recalculate last spurt if in Mid phase or later (FirstAnniversary behavior)
            if !matches!(GAME_BEHAVIOR, GameBehavior::Original) && self.phase >= Phase::Mid {
                calculate_last_spurt(self, course, condition);
            }
        }
        // Activate random gold skill
        else if matches!(effect_type, SkillType::ActivateRandomGold) {
            if !self.golden_skill_indices.is_empty() {
                let idx = self
                    .rng
                    .borrow_mut()
                    .random_range(0..self.golden_skill_indices.len());
                let golden_skill_idx = self.golden_skill_indices[idx];
                // Recursively activate the golden skill (use first alternative)
                self.activate_skill(golden_skill_idx, 0, elapsed_time, course, condition);
            }
        }
        // Style change
        else if is_style_change(effect_type) {
            apply_style_change(&mut self.adjusted.style, effect_type);
        }
        // Rush time extension
        else if matches!(effect_type, SkillType::RushTimeUp) {
            // Extend only if the Uma is bound to rush at all
            // ? Does this apply as well if already rushing? Assume it does for now
            if let Some(rush_state) = &mut self.rush_state {
                rush_state.max_duration += effect.modifier as f64 / 10000.0;
            }
        }
        // FOV change
        else if matches!(effect_type, SkillType::Fov) {
            // TODO: implement FOV change
        }
        // Lane movement speed change
        else if matches!(effect_type, SkillType::LaneMovementSpeed) {
            // TODO: implement lane movement speed change
        }
        // Lane change
        else if matches!(effect_type, SkillType::LaneChange) {
            // Change target lane to (modifier/100)% of course width
            // TODO: review
            let lane_change_percent = effect.modifier as f64 / 10000.0;
            self.lane.target = course.width() * lane_change_percent;
        }
        // "Sympathy" skill activation
        else if matches!(effect_type, SkillType::ActivateSympathy) {
            // Force activation of skill with ID "201631" if available
            if let Some((skill_index, _)) = self
                .available_skills
                .iter()
                .enumerate()
                .find(|(_, skill)| skill.id == "201631")
            {
                self.activate_skill(skill_index, 0, elapsed_time, course, condition);
            }
        }

        if Skill::is_effect_positive(effect) {
            self.activation_flags.buff_activated.insert(*effect_type);
        }
    }

    /// Check if a skill at the given index is a heal skill
    /// (i.e. if any of its alternatives contain a Recovery effect).
    fn is_heal_skill_by_index(&self, skill_index: usize) -> bool {
        let skill = &self.available_skills[skill_index];
        skill.alternatives.iter().any(|alt| {
            alt.effects
                .iter()
                .any(|eff| matches!(eff.skill_type, SkillType::Recovery))
        })
    }

    /// Recalculate skill modifiers from active timed effects.
    fn recalculate_skill_modifiers(&mut self) {
        let mut mods = SkillModifiers::default();

        for effect in &self.active_timed_effects {
            match effect.effect.skill_type {
                SkillType::Acceleration => {
                    mods.acceleration += effect.effect.modifier as f64;
                }
                SkillType::TargetSpeed => {
                    mods.target_speed += effect.effect.modifier as f64;
                }
                SkillType::CurrentSpeed | SkillType::CurrentSpeedWithNaturalDeceleration => {
                    mods.current_speed += effect.effect.modifier as f64;
                }
                SkillType::ExtendEvolvedDuration => {
                    // Duration scaling is multiplicative
                    mods.duration_scaling *= effect.effect.modifier as f64 / 10000.0;
                }
                _ => {}
            }
        }

        self.skill_modifiers = mods;
    }

    /// Update skill durations and remove expired effects.
    pub fn update_skill_durations(&mut self, elapsed_time: f64) {
        let before_count = self.active_timed_effects.len();

        // Remove expired effects
        self.active_timed_effects
            .retain(|effect| effect.expires_at > elapsed_time);

        // Recalculate modifiers if any effects expired
        if self.active_timed_effects.len() != before_count {
            self.recalculate_skill_modifiers();
        }
    }
}
