//! Runners during the race: their state and how it's updated on each frame.
//!
//! A [`Runner`] is created from an [`UnreadyRunner`] once the race is ready to
//! start, and is updated by the solver on every frame. Her methods are split
//! across several `impl Runner` blocks by mechanic:
//!
//! - This module: core state (speed, acceleration, position, phase, HP...) and
//!   the glue to mechanics implemented in their own modules, such as rushing,
//!   spot struggle, Charge Up or dueling.
//! - `lane`: lane movement, overtake target tracking and target lane
//!   decisions.
//! - `positioning`: positioning strategies during the first 10 sections.
//! - `surroundings`: field of view and blocking, surrounding and nearby state.
//! - `skills`: skill trigger checks, activation and active effects.
//!
//! Submodules are private; their public items are re-exported from here.
//!
//! # Examples
//!
//! Show the race results, with each runner's finish margin as displayed in
//! the game:
//!
//! ```
//! use uma_tools::race::margin::Margin;
//! use uma_tools::{Configuration, RaceSolver, Runner, Uma};
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
//! # use uma_tools::{Course, RaceParametersData, Skill, UmaData};
//! # let parameters = RaceParametersData::from_json(
//! #     r#"{
//! #         "trackCondition": "Firm", "weather": "Sunny", "season": "Spring",
//! #         "time": "Noon", "grade": "G1", "gates": 18, "courseId": "10913"
//! #     }"#,
//! # )?;
//! # let uma = UmaData::from_json(
//! #     r#"{
//! #         "speed": 1500, "stamina": 800, "power": 1200, "guts": 700,
//! #         "wisdom": 1200, "strategy": "Oikomi", "distanceAptitude": "S",
//! #         "surfaceAptitude": "A", "strategyAptitude": "A",
//! #         "motivation": "Great", "skills": [200331]
//! #     }"#,
//! # )?;
//! # let path = "data/course_data.json";
//! # let course = Course::from_json_file(path, &parameters.course_id)?;
//! # let all_skills = Skill::load_all_from_json_file("data/skill_data.json")?;
//! # let skills = uma.resolve_skills(&all_skills)?;
//! let config = Configuration::default();
//! let mut solver = RaceSolver::new(course, parameters.race, config);
//! for speed in [1300, 1400, 1500] {
//!     let variant = Uma { speed, ..uma.uma };
//!     solver.add_runner(variant, uma.mood, skills.clone());
//! }
//! let mut solver = solver.ready_runners();
//! solver.run();
//!
//! let finish_time =
//!     |runner: &Runner| runner.finish_time.unwrap_or(f64::INFINITY);
//! let mut runners = solver.into_runners();
//! runners.sort_by(|a, b| finish_time(a).total_cmp(&finish_time(b)));
//! for runner in &runners {
//!     let margin = runner.finish_margin.map(Margin::from_meters);
//!     println!("Speed {}: {:?}", runner.raw.speed, margin);
//! }
//! // The winner has no runner ahead.
//! assert_eq!(runners[0].finish_margin, None);
//! # Ok(())
//! # }
//! ```

use std::ops::RangeInclusive;
use std::sync::Arc;

use rand::prelude::*;

use crate::configuration::GameVersion;
use crate::course::geometry::Slope;
use crate::course::*;
use crate::race::all_out_spurt::{self, AllOutSpurt};
use crate::race::charge_up::{self, ChargeUp};
use crate::race::compete_before_spurt::{self, StaminaKeep};
use crate::race::duel::Duel;
use crate::race::hp::*;
use crate::race::last_spurt::*;
use crate::race::modifiers::*;
use crate::race::popularity::JudgeScores;
use crate::race::rng::{RunnerRng, Stream};
use crate::race::rush::{self, RushState};
use crate::race::secure_lead;
use crate::race::solver::RaceState;
use crate::race::spot_struggle::{self, SpotStruggle};
use crate::race::timed_boost::TimedBoost;
use crate::race::transitions::*;
use crate::race::unready_runner::UnreadyRunner;
use crate::race::world_distance::{self, LastMove};
use crate::skill::*;
use crate::trigger::skill_activation::{ActiveTimedEffect, PendingEffect, SkillActivationRecord};
use crate::trigger::system::TriggerScheduler;
use crate::uma::*;
use crate::units::CourseWidths;

mod lane;
mod positioning;
mod skills;
mod surroundings;

pub use lane::LaneState;
pub(crate) use lane::OvertakeState;
pub(crate) use positioning::{POSITIONING_END_SECTION, PositioningStrategy};
pub use skills::SkillActivationFlags;
pub(crate) use skills::SkillModifiers;
pub(crate) use surroundings::{BlockedState, NearbyState, SurroundState};

/// Maximum target speed a runner can reach in m/s.
pub(crate) const MAX_TARGET_SPEED: f64 = 30.0;

/// Distance from the inner fence above which the force-in modifier applies.
pub(crate) const FORCE_IN_MIN_LANE: CourseWidths = CourseWidths(0.12);

/// Time (s) after which the best and worst orders of a runner are tracked.
pub(crate) const ORDER_RANGE_START_TIME: f64 = 5.0;

/// Tolerance for floating-point comparisons in skill triggers.
const EPSILON: f64 = 0.0001;

#[derive(Debug, Clone, PartialEq)]
pub struct InitialState {
    /// Runner mood level. It affects initial stat adjustments and is also used to
    /// evaluate some skill conditions.
    pub mood: Mood,
    /// Popularity rank (favorite rank) in the race (1-indexed).
    pub popularity_rank: u8,
    /// Scores and marks from the three judges used for popularity calculation.
    pub judge_scores: JudgeScores,
    /// Gate bracket (1-8, from innermost to outermost).
    pub gate_block: u8,
    /// Number of fans.
    pub fan_count: u32,
    /// Whether every popularity requirement is satisfied (see
    /// [`SkillType::IgnorePopularity`]).
    pub ignores_popularity: bool,
    /// Team index (0-based). In individual races, each runner has its own team.
    ///
    /// Runners on the same team are allies; everyone else is an opponent.
    pub team: u8,
}

/// State related to order (rank) in the race.
#[derive(Debug, Clone, PartialEq)]
pub struct OrderState {
    /// Current order (rank), 1-based.
    pub order: u8,
    /// Number of runners in the race, i.e. its gates, finished ones included.
    pub runner_count: u8,
    /// Current order rate (percentile; "you are in the top X%"), rounded up.
    pub rate: u8,
    /// Order rates that round to the current order (see `rates_at_order`).
    pub rates: RangeInclusive<i32>,
    /// Best (lowest) order reached so far.
    /// N.B. This value is tracked only after 5 seconds into the race.
    pub best: u8,
    /// Worst (highest) order reached so far.
    /// N.B. This value is tracked only after 5 seconds into the race.
    pub worst: u8,
    /// Order rates that round to the best order (see `rates_at_order`).
    pub best_rates: RangeInclusive<i32>,
    /// Order rates that round to the worst order (see `rates_at_order`).
    pub worst_rates: RangeInclusive<i32>,
    /// Change in order since last update.
    pub delta: i8,
    /// Number of overtakes since entering Mid phase.
    pub overtakes_since_mid: u16,
    /// Number of overtakes since entering Late phase.
    pub overtakes_since_late: u16,
    /// Number of overtakes since entering last corner.
    pub overtakes_since_last_corner: u16,
    /// Number of overtakes in corners during the Late race or later.
    pub overtakes_in_late_corners: u16,
}

impl Default for OrderState {
    fn default() -> Self {
        OrderState {
            order: 1,
            runner_count: 1,
            rate: 0,
            rates: rates_at_order(1, 1),
            best: u8::MAX,
            worst: 0,
            best_rates: rates_at_order(u8::MAX, 1),
            worst_rates: rates_at_order(0, 1),
            delta: 0,
            overtakes_since_mid: 0,
            overtakes_since_late: 0,
            overtakes_since_last_corner: 0,
            overtakes_in_late_corners: 0,
        }
    }
}

/// Order rates (top X%) that round to `order` in a race with `runner_count`
/// runners.
///
/// Order rate conditions convert their rate into an order, rounding to the
/// nearest (half up): e.g. 50% in a 9-runner race is 4.5, rounded to 5th
/// place, and so are 50% to 61%.
pub(crate) fn rates_at_order(order: u8, runner_count: u8) -> RangeInclusive<i32> {
    debug_assert!(runner_count > 0, "no runners");
    let (order, runner_count) = (order as i32, runner_count as i32);
    // `(runner_count * rate + 50) / 100 == order` holds for
    // `100 * order - 50 <= runner_count * rate < 100 * order + 50`.
    // Rounded up below zero too (order 0, before orders are tracked).
    let ceil_div = |value: i32| -(-value).div_euclid(runner_count);
    ceil_div(100 * order - 50)..=ceil_div(100 * order + 50) - 1
}

/// Distance differences from other runners.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct PositionDiff {
    /// Distance difference from the first-place runner in meters.
    pub from_first: f64,
    /// Distance difference from the next runner ahead in meters, or infinity
    /// if there's nobody ahead.
    pub from_next: f64,
    /// Distance difference from the previous runner behind in meters, or
    /// infinity if there's nobody behind.
    pub from_prev: f64,
    /// Distance difference rate from the first-place runner relative to the
    /// distance between first and last place, in percent.
    pub rate_from_first: f64,
    /// Largest lead over the second-place runner taken before the Late race, in
    /// meters. A runner with nobody behind has no lead.
    pub max_lead: f64,
}

/// Rush counters.
#[derive(Debug, Default, Clone, PartialEq)]
pub(crate) struct RushCounter {
    /// Number of runners currently in rush state ahead.
    pub(crate) ahead: u8,
    /// Number of runners currently in rush state behind.
    pub(crate) behind: u8,
    /// Number of opponents currently in rush state ahead.
    pub(crate) opponents_ahead: u8,
    /// Number of opponents currently in rush state behind.
    pub(crate) opponents_behind: u8,
}

/// An Uma with additional data related to racing, that is, an Uma who is
/// running.
///
/// N.B. This structure should be created from an [`UnreadyRunner`] via
/// [`RaceSolver`].
///
/// [`UnreadyRunner`]: crate::race::unready_runner::UnreadyRunner
/// [`RaceSolver`]: crate::race::solver::RaceSolver
#[derive(Debug, PartialEq)]
pub struct Runner {
    /// Gate number (1-indexed). It determines starting lane position.
    /// It is also used as a unique identifier for the runner.
    pub gate_roll: u8,
    /// Base stats before adjustments. After calculating the adjusted stats,
    /// the base stats are only used to evaluate some skill conditions.
    pub base: Uma,
    /// Raw stats, before halving and mood multiplier. Used for skill value scaling.
    pub raw: Uma,
    /// Adjusted stats after applying modifiers and out-of-gate skills.
    /// For the most part, these are the stats used for race calculations.
    pub adjusted: Uma,
    /// Set of static values related to the runner at the start of the race.
    pub initial_state: InitialState,
    /// Start delay (i.e. time taken to leave the gate) in seconds.
    pub start_delay: f64,
    /// Current speed in m/s. Besides position updates, it also affects HP
    /// consumption.
    pub speed: f64,
    /// Current acceleration in m/s².
    pub acceleration: f64,
    /// Current position in the course in meters.
    pub position: f64,
    /// Current HP state.
    pub hp: HpState,
    /// Minimum speed in m/s. It is based on course base speed and guts stat.
    pub(crate) min_speed: f64,
    /// Target speed in m/s.
    pub target_speed: f64,
    /// Speed modifier applied early in the race when away from the inner fence and
    /// blocked on the inside.
    ///
    /// Its value is rolled before the race begins based on running style.
    pub(crate) force_in_modifier: f64,
    /// Lane state including current and target lane positions in course widths.
    pub lane: LaneState,
    /// Speed modifier for the current section in m/s.
    pub(crate) section_speed_modifier: f64,
    /// List of skills of the runner: those that passed the wit check first, then
    /// those that failed it (see [`Self::available_skills`] and
    /// [`Self::failed_skills`]).
    pub skills: Vec<Arc<Skill>>,
    /// Number of skills that passed the wit check, at the start of `skills`.
    pub(crate) available_skill_count: usize,
    /// Elapsed time when the runner finished the race, in seconds.
    pub finish_time: Option<f64>,
    /// Distance in meters behind the runner that placed immediately ahead.
    /// `None` for first place or until the race is over.
    pub finish_margin: Option<f64>,
    /// List of skills that have been activated during the race.
    pub activated_skills: Vec<SkillActivationRecord>,
    /// Activated alternatives of each skill in `skills`, as a mask (bit `i` set
    /// when alternative `i` activated).
    pub activated_alternatives: Vec<u8>,
    /// Race time (in seconds) until which each alternative of each skill in
    /// `skills` can't activate again, shared by all its OR branches.
    pub(crate) cooldown_until: Vec<Vec<f64>>,
    /// Modifiers from skills applied to this runner's stats.
    pub(crate) skill_modifiers: SkillModifiers,
    /// Effects from skills that are currently active for a limited duration.
    pub(crate) active_timed_effects: Vec<ActiveTimedEffect>,
    /// Effects from this runner's skills that target other runners, applied by the
    /// solver once every runner has checked its skills.
    pub(crate) pending_effects: Vec<PendingEffect>,
    /// Chance of rushing as a percentage (see `rush::chance`).
    ///
    /// This is determined at the start of the race along with deciding whether the
    /// runner will rush or not.
    pub rush_chance: f64,
    /// State related to rushing.
    ///
    /// `None` means that the runner won't rush at all. Otherwise it tells when and
    /// how the rush will happen.
    pub rush_state: Option<RushState>,
    /// Flag indicating if the runner is currently in start dash mode.
    pub is_start_dash: bool,
    /// State related to spot struggle (位置取り争い).
    pub spot_struggle: SpotStruggle,
    /// State related to dueling (追い比べ).
    pub duel: Duel,
    /// State related to securing the lead (リード確保).
    pub secure_lead: TimedBoost,
    /// State related to competing before spurt (位置取り調整). Stamina Keep is
    /// also checked on the same schedule.
    pub compete_before_spurt: TimedBoost,
    /// State related to Stamina Keep (持久力温存).
    pub stamina_keep: StaminaKeep,
    /// Overtake targets being tracked. If `None`, no overtake mode active.
    pub(crate) overtake_state: Option<OvertakeState>,
    /// Time spent having at least one overtake target in seconds.
    pub(crate) overtake_target_time: f64,
    /// Time spent being an overtake target of someone else in seconds.
    pub(crate) being_overtake_target_time: f64,
    /// Extra move lane position.
    ///
    /// It's set during early/mid phase based on furthest visible runner. Used as
    /// fallback in overtake mode lane selection.
    pub(crate) extra_move_lane: Option<CourseWidths>,
    /// State related to order (rank) in the race.
    pub order: OrderState,
    /// Distance differences between this runner and others.
    pub position_diff: PositionDiff,
    /// Number of visible runners in this runner's field of view.
    pub(crate) visible_count: u8,
    /// Information about nearby runners.
    pub(crate) nearby: NearbyState,
    /// Blocking state information.
    pub(crate) blocked: BlockedState,
    /// Surrounding state information.
    pub(crate) surrounded: SurroundState,
    /// Number of runners ahead and behind currently rushing.
    pub(crate) rush_counter: RushCounter,
    /// Current positioning strategy during the first 10 sections of the race.
    pub(crate) positioning_strategy: PositioningStrategy,
    /// Next time to check positioning strategy conditions, in seconds.
    pub(crate) next_positioning_check: f64,
    /// Current slope target speed modifier in m/s, if any.
    ///
    /// Positive in downhill mode, negative when going uphill.
    pub(crate) slope_speed_modifier: Option<f64>,
    /// Time since the last downhill mode check in seconds.
    ///
    /// Downhill mode is checked upon entering a downhill and then every second.
    pub(crate) downhill_check_timer: f64,
    /// Number of uphills climbed to the end.
    pub(crate) uphills_climbed: u8,
    /// Current section index (0-23).
    pub section: u8,
    /// Current race phase.
    pub phase: Phase,
    /// Information about the last spurt phase.
    ///
    /// It indicates when the last spurt can start and its max speed, and it's
    /// calculated when entering the Mid phase.
    pub last_spurt: Option<LastSpurt>,
    /// Stamina Limit Break (スタミナ勝負) buff before its random factor, in m/s.
    ///
    /// Calculated before race begins; zero if the runner is not eligible.
    pub(crate) stamina_limit_break_base: f64,
    /// Active Stamina Limit Break target speed buff in m/s.
    ///
    /// It's set once the runner reaches its last spurt speed and lasts until the
    /// end of the race.
    pub stamina_limit_break: f64,
    /// All-out spurt (全開スパート) acceleration in m/s² before skill modifiers.
    ///
    /// Calculated before race begins; zero if the runner is not eligible.
    pub(crate) all_out_spurt_base: f64,
    /// All-out spurt (全開スパート) state.
    ///
    /// It is activated once the runner reaches its last spurt speed, and keeps
    /// increasing target speed until the end of the race.
    pub all_out_spurt: AllOutSpurt,
    /// Charge Up (足を貯める) state.
    ///
    /// It records the power conserved during the first 10 sections and its release
    /// upon entering the last spurt. `None` if the runner is not eligible.
    pub charge_up: Option<ChargeUp>,
    /// Random number streams of the runner (see [`crate::race::rng`]).
    pub(crate) rng: RunnerRng,
    /// Course segments (corners, straights and slopes) the runner is in.
    pub(crate) segments: SegmentState,
    /// Last move, which slows down the next one if it covered more world
    /// distance than course distance (see [`world_distance`]).
    pub(crate) last_move: LastMove,
    /// Scheduler for skill triggers. It contains many queues for different trigger
    /// types, each with their own criteria (distance, time, etc).
    pub(crate) trigger_scheduler: TriggerScheduler,
    /// Skill activation flags for the current update cycle.
    pub activation_flags: SkillActivationFlags,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum StartType {
    Good,
    Normal,
    Late,
}

impl StartType {
    /// Type of start given the start delay in seconds:
    ///
    /// - Good Start: start delay < 0.02s; gives bonus points in Team Trials.
    /// - Normal Start: 0.02s <= start delay <= 0.08s.
    /// - Late Start: start delay > 0.08s; used in some skill conditions.
    pub fn from_delay(start_delay: f64) -> Self {
        match start_delay {
            d if d < 0.02 => StartType::Good,
            d if d > 0.08 => StartType::Late,
            _ => StartType::Normal,
        }
    }
}

impl Runner {
    /// Create a new Runner from an UnreadyRunner, whose out-of-gate skills have
    /// already been applied, and RaceState.
    pub(crate) fn new(builder: UnreadyRunner, state: &RaceState) -> Self {
        let mut runner = builder.build(state);
        runner.resolve_static_guards(state);
        runner
    }

    /// Get the base style, treating Runaway as a variant of Front.
    pub fn base_style(&self) -> Style {
        self.adjusted.style.base_style()
    }

    /// Number of skills the runner has, including those that failed the wit check.
    pub fn skill_count(&self) -> usize {
        self.skills.len()
    }

    /// Skills that passed the wit check, so they can activate on their own.
    pub fn available_skills(&self) -> &[Arc<Skill>] {
        &self.skills[..self.available_skill_count]
    }

    /// Skills that failed the wit check, so they can only be activated by other
    /// skills (see [`SkillType::ActivateRandomGold`]).
    pub fn failed_skills(&self) -> &[Arc<Skill>] {
        &self.skills[self.available_skill_count..]
    }

    /// Determine the lane type based on the current lane position.
    pub fn lane_type(&self) -> LaneType {
        LaneType::from(self.lane.current)
    }

    /// Determine the type of start (see [`StartType::from_delay`]).
    pub fn start_type(&self) -> StartType {
        StartType::from_delay(self.start_delay)
    }

    /// Update acceleration based on current speed and target speed.
    pub(crate) fn update_acceleration(&mut self, elapsed_time: f64) {
        if self.speed < self.target_speed {
            self.accelerate(elapsed_time);
        } else {
            self.decelerate();
        }
    }

    /// Calculate increasing acceleration.
    fn accelerate(&mut self, elapsed_time: f64) {
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

        let dueling_modifier = self.duel.acceleration_bonus();

        let charge_up_modifier = self
            .charge_up
            .as_ref()
            .map_or(0.0, |c| c.acceleration(elapsed_time));

        self.acceleration = base_acceleration
            + self.skill_modifiers.acceleration
            + start_dash_modifier
            + dueling_modifier
            + charge_up_modifier;
    }

    /// Calculate decreasing acceleration (i.e., deceleration).
    fn decelerate(&mut self) {
        self.acceleration = if self.hp.is_empty() {
            -1.2
        } else if self.is_pace_down() {
            -0.5
        } else {
            match self.phase {
                Phase::RunUp | Phase::Early => -1.2,
                Phase::Mid => -0.8,
                Phase::Late | Phase::Last => -1.0,
            }
        };
    }

    /// Check if the runner is in downhill mode (i.e. not only going downhill but
    /// also experiencing a speed gain).
    pub fn is_in_downhill_mode(&self) -> bool {
        self.slope_speed_modifier.is_some_and(|s| s > 0.0)
    }

    /// Check if the runner is going uphill, thus experiencing a speed reduction.
    pub fn is_going_uphill(&self) -> bool {
        self.slope_speed_modifier.is_some_and(|s| s < 0.0)
    }

    /// Get the effective speed including skill modifiers.
    /// This is the actual speed used for position calculations.
    fn effective_speed(&self) -> f64 {
        self.speed + self.skill_modifiers.current_speed
    }

    /// Update the runner's speed based on acceleration and constraints.
    pub(crate) fn update_speed(&mut self, delta_time: f64, course_base_speed: f64) {
        // Update speed based on acceleration.
        self.speed += self.acceleration * delta_time;

        // Handle start dash end and min speed.
        if self.is_start_dash && self.speed >= 0.85 * course_base_speed {
            self.is_start_dash = false;
        } else if !self.is_start_dash && self.speed < self.min_speed {
            self.speed = self.min_speed;
        }
        // Cap speed along with target speed to avoid overshooting due to acceleration.
        // ? Is overshooting allowed? Cap it for now to avoid spikes.
        self.speed = self.speed.min(MAX_TARGET_SPEED);
        // Front blocking caps speed, even below the minimum speed.
        // ? Unknown whether the minimum speed takes precedence.
        if let Some(speed_limit) = self.blocked.speed_limit {
            self.speed = self.speed.min(speed_limit);
        }

        // The all-out spurt keeps increasing target speed.
        // It ignores the usual acceleration formula and acceleration skills.
        if let AllOutSpurt::Active { speed_bonus } = &mut self.all_out_spurt {
            let acceleration = self.all_out_spurt_base
                + all_out_spurt::SKILL_SCALE * self.skill_modifiers.all_out_spurt_acceleration;
            *speed_bonus += acceleration * delta_time;
        }
    }

    /// Last spurt of the runner, if she has entered it (i.e. it has been
    /// calculated and she has reached its target distance).
    pub fn active_last_spurt(&self) -> Option<&LastSpurt> {
        self.last_spurt
            .as_ref()
            .filter(|last_spurt| self.position >= last_spurt.target_distance)
    }

    /// Check if the runner is in her last spurt and has reached its speed.
    fn has_reached_last_spurt_speed(&self) -> bool {
        self.active_last_spurt()
            .is_some_and(|last_spurt| self.speed >= last_spurt.max_speed)
    }

    /// Update target speed based on various factors.
    pub(crate) fn update_target_speed(&mut self, state: &RaceState) {
        let game_version = state.config.game_version;
        let target_speed = if self.hp.is_empty() {
            self.min_speed
        } else if let Some(last_spurt) = self.active_last_spurt() {
            last_spurt.max_speed
        } else {
            let position_multiplier = self
                .positioning_strategy
                .speed_multiplier(self.phase, game_version);
            let base_target_speed = base_target_speed(
                state.course.base_speed(),
                self.adjusted.style,
                self.phase,
                self.adjusted.speed,
                self.adjusted.distance_aptitude,
            );

            base_target_speed * position_multiplier
        };

        // Stamina Limit Break is activated upon reaching last spurt speed (2.5 Anniv.).
        if self.stamina_limit_break == 0.0
            && self.stamina_limit_break_base > 0.0
            && self.has_reached_last_spurt_speed()
        {
            self.stamina_limit_break = self.stamina_limit_break_base
                * stamina_limit_break_random_factor(&mut self.rng.last_spurt);
        }

        // All-out spurt is activated upon reaching last spurt speed (from 5th Anniv.).
        if self.all_out_spurt == AllOutSpurt::Pending
            && self.all_out_spurt_base > 0.0
            && !self.hp.is_empty()
            && self.has_reached_last_spurt_speed()
        {
            self.all_out_spurt = AllOutSpurt::Active { speed_bonus: 0.0 };
            self.schedule_all_out_spurt_triggers(state);
        }

        // Lane state is updated after target speed within a frame, so `speed` still
        // holds the lane movement from the previous frame.
        let move_lane_modifier = if game_version >= GameVersion::FirstAnniversary
            && self.skill_modifiers.lane_speed > 0.0
            && self.lane.speed != 0.0
        {
            (0.0002 * self.adjusted.power as f64).sqrt()
        } else {
            0.0
        };

        let force_in_modifier = if self.phase <= Phase::Early
            && self.lane.current > FORCE_IN_MIN_LANE
            && self.blocked.side_in
        {
            self.force_in_modifier
        } else {
            0.0
        };

        self.target_speed = (target_speed
            + self.slope_speed_modifier.unwrap_or(0.0)
            + self.skill_modifiers.target_speed
            + self.section_speed_modifier
            + self.duel.speed_bonus()
            + move_lane_modifier
            + force_in_modifier
            + self.spot_struggle.speed_bonus(state.elapsed_time)
            + self.secure_lead.speed_bonus()
            + self.compete_before_spurt.speed_bonus()
            + self.stamina_limit_break
            + self.all_out_spurt.speed_bonus())
        .min(MAX_TARGET_SPEED)
    }

    /// Update the runner's position based on effective current speed.
    pub(crate) fn update_position(&mut self, delta_time: f64, state: &RaceState) {
        // Runner only moves for the part of the frame after her start delay.
        // The start delay may span several frames, so it's measured from the race
        // start. Clamped to avoid negative values from float drift in elapsed time.
        let move_time = delta_time
            .min(state.elapsed_time - self.start_delay)
            .max(0.0);
        let prev_position = self.position;
        // Use effective_speed which includes current_speed modifiers, slowed down
        // along the course by the world distance covered on the last move.
        let course_speed = self.effective_speed() / self.world_distance_ratio(&state.course);
        self.position += course_speed * move_time;
        // The lane changes after the move, so the next move ends at the new lane.
        self.last_move = LastMove {
            distance: self.position - prev_position,
            lane: self.lane.current,
        };
        if self.finish_time.is_none() && self.position >= state.course.length() {
            // Calculate exact finish time.
            let remaining_distance = state.course.length() - prev_position;
            let time_to_finish = remaining_distance / course_speed;
            self.finish_time = Some(state.elapsed_time - delta_time + time_to_finish);
        }
    }

    /// Ratio of world distance to course distance of the last move, at least 1.
    ///
    /// The move ends at the current lane and position, so the current corner
    /// (if any) gives its curvature.
    fn world_distance_ratio(&self, course: &Course) -> f64 {
        let curvature = self
            .segments
            .corner()
            .map_or(0.0, |idx| course.corners[idx].curvature());
        world_distance::ratio(
            self.last_move.distance,
            self.last_move.lane,
            self.lane.current,
            curvature,
        )
    }

    /// Check if `other` is an opponent (i.e. on a different team).
    pub fn is_opponent(&self, other: &Runner) -> bool {
        self.initial_state.team != other.initial_state.team
    }

    /// Check if the runner has finished the race.
    pub fn has_finished(&self) -> bool {
        self.finish_time.is_some()
    }

    /// Update the current section and phase based on position.
    pub(crate) fn update_phase(&mut self, state: &mut RaceState) {
        let prev_section = self.section;
        self.section = state.course.section_from_position(self.position);
        if self.section != prev_section {
            // Update section speed modifier.
            self.section_speed_modifier = roll_section_speed_modifier(
                &mut self.rng.keyed(Stream::SectionSpeed(self.section)),
                self.adjusted.wit,
                state.course.base_speed(),
            );

            // Start rush if conditions are met (a runner can only rush once).
            if let Some(rush_state) = &self.rush_state
                && rush_state.section == self.section
                && !self.has_rushed()
            {
                self.start_rush(state);
            }

            if self.section == spot_struggle::END_SECTION {
                self.spot_struggle.stop();
            }
        }

        // Phase can only advance at section boundaries, or at the end of the
        // run-up (which is part of section 0).
        if self.section != prev_section || self.phase == Phase::RunUp {
            let prev_phase = self.phase;
            self.phase = Phase::from_position(self.position, &state.course);
            if prev_phase != self.phase && self.phase >= Phase::Late && self.last_spurt.is_none() {
                self.last_spurt = Some(self.calculate_last_spurt(state));
            }
        }

        self.try_release_charge_up(state);
    }

    /// Release conserved power (脚色十分) upon entering the last spurt.
    ///
    /// Called after the position update, so the acceleration bonus starts on the
    /// next frame.
    fn try_release_charge_up(&mut self, state: &RaceState) {
        if self.active_last_spurt().is_none() {
            return;
        }

        let activity =
            charge_up::activity_coefficient(self.spot_struggle.has_struggled(), self.has_rushed());
        if let Some(charge_up) = &mut self.charge_up {
            charge_up.release(
                self.adjusted.style,
                &state.course,
                activity,
                state.elapsed_time,
                self.position,
            );
        }
    }

    /// Conserve power (足を貯める) during the first 10 sections.
    ///
    /// Power is conserved while keeping position calmly. Speeding up, rushing or
    /// struggling for the lead prevent it, and impose a cooldown upon exit.
    pub(crate) fn update_charge_up(&mut self, delta_time: f64, elapsed_time: f64) {
        if self.section >= POSITIONING_END_SECTION {
            return;
        }
        let blocked = self.is_rushed()
            || self.spot_struggle.is_active(elapsed_time)
            || matches!(
                self.positioning_strategy,
                PositioningStrategy::FrontSpeedUp
                    | PositioningStrategy::FrontOvertake
                    | PositioningStrategy::PaceUp(_)
                    | PositioningStrategy::PaceUpEx
            );
        let rate = charge_up::conservation_rate(self.is_pace_down(), self.adjusted.style);
        if let Some(charge_up) = &mut self.charge_up {
            charge_up.conserve(rate, blocked, delta_time, elapsed_time);
        }
    }

    /// Update HP based on current speed, phase, and modifiers.
    pub(crate) fn update_hp(&mut self, delta_time: f64, state: &RaceState) {
        let mut multiplier = if self.spot_struggle.is_active(state.elapsed_time) {
            spot_struggle::hp_multiplier(self.adjusted.style, self.is_rushed())
        } else if self.is_rushed() {
            rush::HP_MULTIPLIER
        } else {
            1.0
        };
        if self.is_pace_down() {
            multiplier *= 0.6;
        }
        if self.is_in_downhill_mode() {
            multiplier *= 0.4;
        }
        self.hp.consume(
            calculate_hp_per_second(
                self.speed,
                &state.course,
                state.params.track_condition,
                self.phase,
                self.adjusted.guts,
                multiplier,
            ) * delta_time,
        );
        if self.hp.is_empty() {
            // Running out of HP during the all-out spurt drops speed to minimum
            // immediately instead of decelerating (confirmed game bug).
            if self.all_out_spurt.is_active() {
                self.speed = self.min_speed;
                self.all_out_spurt = AllOutSpurt::Ended;
            }
        }
    }

    /// Start rushing, unless the runner is already rushed.
    ///
    /// The runner will move ahead of the position dictated by her style, losing HP
    /// at an increased rate.
    fn start_rush(&mut self, state: &mut RaceState) {
        if let Some(rush_state) = &mut self.rush_state
            && !rush_state.is_rushed
        {
            rush_state.start(state.elapsed_time);
            self.on_rush_change(state);
            if let Some(charge) = &mut self.charge_up {
                charge.penalize(charge_up::RUSH_PENALTY);
            }
        }
    }

    /// Stop rushing, if the runner is rushed.
    fn end_rush(&mut self, state: &mut RaceState) {
        if let Some(rush_state) = &mut self.rush_state
            && rush_state.is_rushed
        {
            rush_state.end(state.elapsed_time);
            self.on_rush_change(state);
        }
    }

    /// Update the race's count of rushed runners after this runner started or
    /// stopped rushing, and force a positioning strategy change immediately.
    fn on_rush_change(&mut self, state: &mut RaceState) {
        let is_rushed = self.is_rushed();
        let count =
            &mut state.rushed_by_team[self.initial_state.team as usize][self.base_style() as usize];
        if is_rushed {
            *count += 1;
        } else {
            *count -= 1;
        }
        self.reset_positioning(state.elapsed_time);
    }

    /// End rush once it reaches its maximum duration or the runner snaps out of it.
    pub(crate) fn update_rush_state(&mut self, state: &mut RaceState) {
        if let Some(rush_state) = &mut self.rush_state
            && rush_state.is_rushed
            && rush_state.should_end(state.elapsed_time, &mut self.rng.rush)
        {
            self.end_rush(state);
        }
    }

    /// Check if the runner is currently in a rushed state.
    pub fn is_rushed(&self) -> bool {
        self.rush_state.as_ref().is_some_and(|s| s.is_rushed)
    }

    /// Check if the runner has rushed at some point.
    pub fn has_rushed(&self) -> bool {
        self.rush_state.as_ref().is_some_and(RushState::has_rushed)
    }

    /// Start a spot struggle (位置取り争い), unless the runner already struggled.
    pub(crate) fn start_spot_struggle(&mut self, elapsed_time: f64) {
        if self.spot_struggle.has_struggled() {
            return;
        }
        self.spot_struggle = SpotStruggle::start(
            self.adjusted.guts,
            self.adjusted.style_aptitude,
            elapsed_time,
        );
        if let Some(charge) = &mut self.charge_up {
            charge.penalize(charge_up::STRUGGLE_PENALTY);
        }
    }

    /// Check if the runner should check whether to secure the lead on this frame.
    pub(crate) fn is_secure_lead_check_due(&self, state: &RaceState) -> bool {
        self.adjusted.style != Style::End
            && self.is_boost_check_due(&self.secure_lead, &secure_lead::SECTIONS, state)
    }

    /// Attempt to secure the lead (リード確保).
    ///
    /// If the lead over another runner with a later strategy is less than
    /// desirable, there is a chance for the runner to speed up for a short time at
    /// the cost of HP, followed by a cooldown.
    ///
    /// `is_alone` is whether no other runner with the same style is within 10m.
    pub(crate) fn try_secure_lead(
        &mut self,
        lead_insufficient: bool,
        is_alone: bool,
        state: &RaceState,
    ) {
        let elapsed_time = state.elapsed_time;
        if !lead_insufficient || !self.rng.secure_lead.random_bool(secure_lead::CHANCE) {
            self.secure_lead = TimedBoost::Idle {
                next_check: elapsed_time + secure_lead::CHECK_INTERVAL,
            };
            return;
        }

        let style = self.adjusted.style;
        let speed_bonus = secure_lead::target_speed_bonus(
            style,
            self.adjusted.guts,
            is_alone,
            state.config.game_version,
        );
        self.secure_lead = TimedBoost::start(
            speed_bonus,
            elapsed_time,
            secure_lead::DURATION,
            secure_lead::COOLDOWN,
        );
        self.hp
            .consume(secure_lead::hp_cost(style, state.course.distance));
    }

    /// Returns true if `self` should check whether to compete before spurt (位置取り調整)
    /// on this frame.
    pub(crate) fn is_compete_check_due(&self, state: &RaceState) -> bool {
        self.is_boost_check_due(
            &self.compete_before_spurt,
            &compete_before_spurt::SECTIONS,
            state,
        )
    }

    /// Check if a check for `boost`, which can only happen in `sections`, is due
    /// on this frame (2.5 Anniv. onward).
    fn is_boost_check_due(
        &self,
        boost: &TimedBoost,
        sections: &RangeInclusive<u8>,
        state: &RaceState,
    ) -> bool {
        state.config.game_version >= GameVersion::TwoPointFiveAnniversary
            && !self.has_finished()
            && !self.hp.is_empty()
            && sections.contains(&self.section)
            && boost.is_check_due(state.elapsed_time)
    }

    /// Attempt to compete before spurt (位置取り調整).
    ///
    /// First, if the runner lacks the HP she wants to keep to finish the race, she
    /// may notice and enter Stamina Keep (持久力温存). Then, unless in Stamina Keep, she
    /// competes if she is too far from the lead or has other runners nearby,
    /// speeding up for a short time at the cost of HP, followed by a cooldown.
    ///
    /// `is_alone` is whether no other runner with the same style is within
    /// `compete_before_spurt::ALONE_DISTANCE`.
    pub(crate) fn try_compete_before_spurt(
        &mut self,
        far_from_lead: bool,
        is_alone: bool,
        state: &RaceState,
    ) {
        let elapsed_time = state.elapsed_time;
        if !self.stamina_keep.is_active
            && self.hp.current() < self.hp_required_to_finish(state) * self.stamina_keep.ratio
            && self
                .rng
                .stamina_keep
                .random_bool(compete_before_spurt::stamina_keep_chance(self.adjusted.wit))
        {
            self.stamina_keep.is_active = true;
        }
        // ? Chance formula is unknown; set to always trigger for now.
        if self.stamina_keep.is_active || !(far_from_lead || self.nearby.count > 0) {
            self.compete_before_spurt = TimedBoost::Idle {
                next_check: elapsed_time + compete_before_spurt::CHECK_INTERVAL,
            };
            return;
        }

        let style = self.adjusted.style;
        let speed_bonus = compete_before_spurt::target_speed_bonus(
            style,
            self.adjusted.power,
            self.adjusted.guts,
            is_alone,
        );
        self.compete_before_spurt = TimedBoost::start(
            speed_bonus,
            elapsed_time,
            compete_before_spurt::DURATION,
            compete_before_spurt::COOLDOWN,
        );
        self.hp.consume(compete_before_spurt::hp_cost(
            style,
            state.course.distance,
            far_from_lead,
        ));
    }

    /// Start dueling (追い比べ), unless the runner already is.
    pub(crate) fn start_duel(&mut self) {
        if self.duel.is_dueling() {
            return;
        }
        self.duel = Duel::start(self.adjusted.guts);
    }

    /// Apply slope effects after a segment update with `transition` (see
    /// [`SegmentState::update`]).
    ///
    /// Uphills reduce the runner's target speed while on them. On downhills, the
    /// runner may enter downhill mode (Wit * 0.04% chance) or exit it (20% chance)
    /// upon entering them and then every second. Each slope has its own random
    /// stream, so that the checks on a slope don't depend on those before it.
    pub(crate) fn update_slope(
        &mut self,
        slopes: &[Slope],
        transition: &Transition,
        delta_time: f64,
    ) {
        if let Some(idx) = transition.exited {
            self.slope_speed_modifier = None;
            if slopes[idx].is_uphill() {
                self.uphills_climbed += 1;
            }
        }

        let Some(slope_idx) = self.segments.slope() else {
            return;
        };
        let slope = &slopes[slope_idx];
        if slope.is_uphill() {
            if transition.entered.is_some() {
                self.slope_speed_modifier =
                    Some(uphill_speed_modifier(slope.grade, self.adjusted.power));
            }
            return;
        }
        if !slope.is_downhill() {
            return;
        }

        if transition.entered.is_some() {
            self.downhill_check_timer = 0.0;
            let rng = &mut self.rng;
            rng.slope = rng.keyed(Stream::Slope(slope_idx));
        } else {
            self.downhill_check_timer += delta_time;
            // Accumulated frame times fall slightly short of whole seconds.
            if self.downhill_check_timer < 1.0 - EPSILON {
                return;
            }
            self.downhill_check_timer -= 1.0;
        }

        let in_downhill_mode = self.is_in_downhill_mode();
        let rng = &mut self.rng.slope;
        if in_downhill_mode {
            if rng.random_bool(0.2) {
                self.slope_speed_modifier = None;
            }
        } else {
            let chance = (self.adjusted.wit as f64 * 0.0004).min(1.0);
            if rng.random_bool(chance) {
                self.slope_speed_modifier = Some(downhill_speed_modifier(slope.grade));
            }
        }
    }

    /// Update order (rank) and overtakes based on current order.
    ///
    /// The order change is negative when overtaking, and positive when being
    /// overtaken.
    pub(crate) fn update_order(&mut self, current_order: u8, course: &Course) {
        let prev_order = self.order.order;
        self.order.delta = current_order as i8 - prev_order as i8;
        self.order.order = current_order;
        if self.order.delta < 0 {
            let overtakes = self.order.delta.unsigned_abs() as u16;
            if self.phase >= Phase::Mid {
                self.order.overtakes_since_mid += overtakes;
            }
            if self.phase >= Phase::Late {
                self.order.overtakes_since_late += overtakes;
                if self.segments.corner().is_some() {
                    self.order.overtakes_in_late_corners += overtakes;
                }
            }
            self.extend_effects_on_overtake(overtakes);
            self.trigger_additional_effects(&[AdditionalActivation::OrderUp], overtakes);
            if course
                .corners
                .last()
                .is_some_and(|x| self.position >= x.start)
            {
                self.order.overtakes_since_last_corner += overtakes;
            }
        }
    }

    /// Update order rate (percentage rank) based on total runners.
    ///
    /// After the first [`ORDER_RANGE_START_TIME`] seconds (`elapsed_time`),
    /// also update the best/worst orders.
    pub(crate) fn update_order_rate(&mut self, total_runners: u8, elapsed_time: f64) {
        let order = self.order.order;
        self.order.runner_count = total_runners;
        self.order.rate = ((order as f64 / total_runners as f64) * 100.0).ceil() as u8;
        self.order.rates = rates_at_order(order, total_runners);
        if elapsed_time > ORDER_RANGE_START_TIME {
            self.order.best = self.order.best.min(order);
            self.order.worst = self.order.worst.max(order);
        }
        self.order.best_rates = rates_at_order(self.order.best, total_runners);
        self.order.worst_rates = rates_at_order(self.order.worst, total_runners);
    }

    /// Update position differences from other runners ahead.
    ///
    /// `position_first`: position of the first-place runner.
    /// `position_next`: position of the next runner ahead, or infinity if
    /// there's nobody ahead.
    /// `position_range`: distance between first and last place.
    pub(crate) fn update_diff_from_ahead(
        &mut self,
        position_first: f64,
        position_next: f64,
        position_range: f64,
    ) {
        self.position_diff.from_first = position_first - self.position;
        self.position_diff.from_next = position_next - self.position;
        self.position_diff.rate_from_first = self.position_diff.from_first / position_range * 100.0;
    }

    /// Update position difference from the runner behind (`position_prev`, or
    /// negative infinity if there's nobody behind), and the largest lead taken
    /// before the Late race.
    ///
    /// The order must be updated first.
    pub(crate) fn update_diff_from_behind(&mut self, position_prev: f64) {
        self.position_diff.from_prev = self.position - position_prev;
        if self.order.order == 1
            && self.phase < Phase::Late
            && self.position_diff.from_prev.is_finite()
        {
            self.position_diff.max_lead = self
                .position_diff
                .max_lead
                .max(self.position_diff.from_prev);
        }
    }
}

#[cfg(test)]
mod tests;
