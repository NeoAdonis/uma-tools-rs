use std::cell::RefCell;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::rc::Rc;

use rand::prelude::*;

use crate::condition::evaluator::{CompEvaluator, EvaluatorFull, evaluate_comparison_list};
use crate::condition::parser::{Comparison, ComparisonList};
use crate::configuration::GameVersion;
use crate::course::geometry::{Corner, Slope, Straight};
use crate::course::*;
use crate::race::all_out_spurt::{self, AllOutSpurt};
use crate::race::charge_up::{self, ChargeUp, ChargeUpOutcome, ChargeUpRelease};
use crate::race::compete_before_spurt;
use crate::race::duel::Duel;
use crate::race::hp::*;
use crate::race::last_spurt::*;
use crate::race::modifiers::*;
use crate::race::popularity::JudgeScores;
use crate::race::proximity;
use crate::race::secure_lead;
use crate::race::skill_scaling;
use crate::race::solver::RaceState;
use crate::race::spot_struggle::{self, SpotStruggle};
use crate::race::transitions::*;
use crate::race::unready_runner::UnreadyRunner;
use crate::skill::*;
use crate::trigger::random::{SEGMENT_LENGTH, random_segment};
use crate::trigger::skill_activation::{
    ActiveTimedEffect, LogicAction, OvertakeExtension, PendingEffect, SkillActivationRecord,
};
use crate::trigger::system::{TriggerScheduler, WindowTrigger};
use crate::uma::*;
use crate::units::*;

/// Skill modifiers that are applied during race calculations.
#[derive(Debug, Clone)]
pub struct SkillModifiers {
    /// Modifies target speed. Positive values increase it.
    pub target_speed: f64,
    /// Modifies current speed directly. Positive values increase it.
    pub current_speed: f64,
    /// Modifies acceleration. Positive values increase it.
    pub acceleration: f64,
    /// Modifies all-out spurt acceleration (listed value, before scaling).
    pub all_out_spurt_acceleration: f64,
    /// Modifies lane movement speed. Positive values increase lane change speed.
    pub lane_speed: f64,
    /// Modifies visible distance. Positive values increase it.
    pub visible_distance: f64,
    /// Sets the target lane directly in course widths, or `None` if no change.
    pub target_lane: Option<f64>,
    /// Affects duration of timed effects. Multiplicative - 1.0 means no change.
    pub duration_scaling: f64,
}

impl Default for SkillModifiers {
    fn default() -> Self {
        Self {
            target_speed: 0.0,
            current_speed: 0.0,
            acceleration: 0.0,
            all_out_spurt_acceleration: 0.0,
            lane_speed: 0.0,
            visible_distance: 0.0,
            target_lane: None,
            duration_scaling: 1.0, // Multiplicative default.
        }
    }
}

/// Maximum target speed a runner can reach in m/s.
pub const MAX_TARGET_SPEED: f64 = 30.0;

/// Distance from the inner fence (in course widths) above which the force-in
/// modifier applies.
pub const FORCE_IN_MIN_LANE: f64 = 0.12;

/// Tolerance for floating-point comparisons in skill triggers.
const EPSILON: f64 = 0.0001;

pub struct InitialState {
    /// Runner motivation level. It affects initial stat adjustments,
    /// and is also used to evaluate some skill conditions.
    pub motivation: Motivation,
    /// Popularity rank (favorite rank) in the race (1-indexed).
    pub popularity_rank: u8,
    /// Scores and marks from the three judges used for popularity calculation.
    pub judge_scores: JudgeScores,
    /// Gate bracket (1-8, from innermost to outermost).
    pub gate_block: u8,
    /// Number of fans.
    pub fan_count: u32,
    /// Team index (0-based). In individual races, each runner has its own team.
    ///
    /// Runners on the same team are allies; everyone else is an opponent.
    pub team: u8,
}

/// State related to rushing (temptation, 掛かり).
pub struct RushState {
    pub section: u8,
    pub start_time: f64,
    pub end_time: f64,
    pub max_duration: f64,
    pub is_rushed: bool,
    pub style: Style,
}

/// Speed boost applied while securing the lead (リード確保) and its expiration time.
pub struct SecureLeadState {
    pub modifier: f64,
    pub expires_at: f64,
}

/// Speed boost applied while competing before spurt (位置取り調整) and its expiration
/// time.
pub struct CompeteState {
    pub modifier: f64,
    pub expires_at: f64,
}

/// Information about an overtake target ahead of this runner.
pub struct OvertakeTarget {
    /// Whether this target has been checked in the current update cycle.
    is_checked: bool,
    /// Time spent being an overtake target in seconds.
    engagement_time: f64,
    /// Whether this target is the closest blocker.
    is_closest_blocker: bool,
}

/// Overtake mode state including persistence timer.
pub struct OvertakeState {
    /// Ordered map of overtake targets by gate_roll.
    ///
    /// Lane candidates are always considered in the same order.
    targets: BTreeMap<u8, OvertakeTarget>,
    /// Time when overtake mode should expire (None if active with targets).
    expires_at: Option<f64>,
}

/// Temporary candidate lane during overtake mode lane selection.
struct OvertakeLaneCandidate {
    lane: f64,
    score: f64,
}

/// Result of a lane target calculation to be written back to the runner.
///
/// Both fields hold the runner's current values when unchanged.
pub struct LaneDecision {
    pub target: f64,
    pub extra_move_lane: Option<f64>,
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
    /// Number of overtakes in corners during the Late race or later.
    pub overtakes_in_late_corners: u16,
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
            overtakes_in_late_corners: 0,
        }
    }
}

/// First section where positioning strategies no longer apply.
pub const POSITIONING_END_SECTION: u8 = 10;

/// Positioning strategy during the first 10 sections (5/12) of the race.
///
/// For front-runners, it determines the pacing behavior to stay ahead or catch
/// up. For non-front-runners, it determines the pacing behavior to keep their
/// distance from the pacemaker.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PositioningStrategy {
    /// Run at regular pace.
    Normal,
    /// *(Front-runners only.)*
    /// Try to stay ahead by pacing up to a certain distance.
    FrontSpeedUp,
    /// *(Front-runners only.)*
    /// Try to catch up with first place by pacing up to a certain distance.
    FrontOvertake,
    /// *(Non-front-runners only.)*
    /// Try to stay within a certain distance from pacemaker by pacing up.
    PaceUp(f64),
    /// *(Non-front-runners only.)*
    /// Try to fall back within a certain distance from pacemaker by pacing down.
    PaceDown(f64),
    /// *Introduced by 1.5 Anniversary update in Japan.* Speed up if there is
    /// another runner ahead whose strategy should actually position her behind.
    /// Front-runners enter it when any such runner is ahead; other runners enter it
    /// when the pacemaker is such a runner. Exits once no such runner is ahead.
    /// Takes priority over all other strategies.
    PaceUpEx,
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
    /// Distance difference rate from the first-place runner relative to the
    /// distance between first and last place, in percent.
    pub rate_from_first: f64,
    /// Largest lead over the second-place runner taken before the Late race, in
    /// meters.
    pub max_lead: f64,
}

/// Rush (temptation, 掛かり) counters.
#[derive(Default)]
pub struct RushCounter {
    /// Number of runners currently in rush state ahead.
    pub ahead: u8,
    /// Number of runners currently in rush state behind.
    pub behind: u8,
    /// Number of opponents currently in rush state ahead.
    pub opponents_ahead: u8,
    /// Number of opponents currently in rush state behind.
    pub opponents_behind: u8,
}

/// Lane state including current and target lane positions in course widths.
///
/// Use [`course_widths_to_meters`] to convert them to meters for reporting.
pub struct LaneState {
    /// Current lane position in course widths.
    pub current: f64,
    /// Target lane position in course widths.
    pub target: f64,
    /// Lateral speed applied on the last lane update in course widths per second.
    ///
    /// Positive values move outward, negative values move inward and zero means the
    /// runner didn't change lanes on the last frame.
    pub speed: f64,
    /// Non-directional lane change speed in course widths per second.
    ///
    /// It changes towards the lane change target speed, before skills and direction
    /// are accounted for, and resets when the target lane is reached.
    pub current_speed: f64,
}

impl LaneState {
    pub fn new(lane: f64) -> Self {
        Self {
            current: lane,
            target: lane,
            speed: 0.0,
            current_speed: 0.0,
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
    /// Whether the runner is currently side-blocked towards the inside.
    pub side_in: bool,
    /// Whether the runner is currently side-blocked towards the outside.
    pub side_out: bool,
    /// Time spent being front-blocked in seconds.
    pub front_time: f64,
    /// Time spent being side-blocked in seconds.
    pub side_time: f64,
    /// Time spent being side-blocked during the Mid race in seconds. Unlike
    /// [`Self::side_time`], blocking before the Mid race doesn't count.
    pub mid_side_time: f64,
    /// Longest time spent being side-blocked during the Mid race in seconds.
    pub max_mid_side_time: f64,
}

impl BlockedState {
    /// Whether the runner is currently side-blocked in either direction.
    pub fn side(&self) -> bool {
        self.side_in || self.side_out
    }
}

/// Information about nearby runners.
#[derive(Default)]
pub struct NearbyState {
    /// Total number of nearby runners.
    pub count: u8,
    /// Whether the runner behind is in a lane that's closer to the center.
    pub behind_in: bool,
    /// Number of nearby runners in the same lane ahead.
    /// Maps to variable `near_infront_count`.
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
    /// Indicates if a skill was activated in the current update cycle.
    pub any_activated: bool,
    /// Indicates if a healing skill was activated in the current update cycle.
    pub heal_activated: bool,
    /// Effect types of the buff (advantage) skills activated in the current update
    /// cycle. Only effects on oneself count.
    pub buff_activated: HashSet<SkillType>,
}

/// An Uma with additional data related to racing, that is, an Uma who is
/// running.
///
/// N.B. This structure should be created from an `UnreadyRunner` via
/// `RaceSolver`.
pub struct Runner {
    /// Gate number (1-indexed). It determines starting lane position.
    /// It is also used as a unique identifier for the runner.
    pub gate_roll: u8,
    /// Base stats before adjustments. After calculating the adjusted stats,
    /// the base stats are only used to evaluate some skill conditions.
    pub base: Uma,
    /// Raw stats, before halving and motivation. Used to scale some skill values.
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
    pub min_speed: f64,
    /// Target speed in m/s.
    pub target_speed: f64,
    /// Speed modifier applied early in the race when away from the inner fence and
    /// blocked on the inside.
    ///
    /// Its value is rolled before the race begins based on running style.
    pub force_in_modifier: f64,
    /// Lane state including current and target lane positions in course widths.
    pub lane: LaneState,
    /// Speed modifier for the current section in m/s.
    pub section_speed_modifier: f64,
    /// List of skills available for activation.
    pub available_skills: Vec<Skill>,
    /// List of skills that failed the wisdom check, so they can't activate.
    pub failed_skills: Vec<Skill>,
    // Elapsed time when the runner finished the race, in seconds.
    pub finish_time: Option<f64>,
    /// Distance in meters behind the runner that placed immediately ahead.
    /// `None` for first place or until the race is over.
    pub finish_margin: Option<f64>,
    /// List of skills that have been activated during the race.
    pub activated_skills: Vec<SkillActivationRecord>,
    /// Activated skill IDs, each with a mask of its activated alternatives (bit
    /// `i` set when alternative `i` activated).
    pub activated_skill_ids: HashMap<String, u32>,
    /// Set of activated skill alternatives (skill_index, alternative_index).
    pub activated_alternatives: HashSet<(usize, usize)>,
    /// Modifiers from skills applied to this runner's stats.
    pub skill_modifiers: SkillModifiers,
    /// Effects from skills that are currently active for a limited duration.
    pub active_timed_effects: Vec<ActiveTimedEffect>,
    /// Effects from this runner's skills that target other runners, applied by the
    /// solver once every runner has checked its skills.
    pub pending_effects: Vec<PendingEffect>,
    /// List of indices of golden skills in `available_skills`.
    /// These skills can be activated by other skills.
    pub golden_skill_indices: Vec<usize>,
    /// Chance of entering rush (temptation, 掛かり) state as a percentage.
    /// This is determined at the start of the race along with deciding whether the
    /// runner will rush or not.
    pub rush_chance: f64,
    /// State related to rushing (temptation, 掛かり). `None` means that the runner
    /// won't rush at all. Otherwise it tells when and how the rush will happen.
    pub rush_state: Option<RushState>,
    /// Flag indicating if the runner is currently in start dash mode.
    pub is_start_dash: bool,
    /// State related to spot struggle (位置取り争い).
    pub spot_struggle: SpotStruggle,
    /// State related to dueling (追い比べ).
    pub duel: Duel,
    /// State related to secure lead mechanics. If `None`, the runner is not
    /// securing the lead.
    pub secure_lead: Option<SecureLeadState>,
    /// Time of the next secure lead check in seconds.
    pub next_secure_lead_check: f64,
    /// State related to compete before spurt mechanics. If `None`, the runner is
    /// not competing.
    pub compete_before_spurt: Option<CompeteState>,
    /// Time of the next compete before spurt (and Stamina Keep) check in seconds.
    pub next_compete_check: f64,
    /// Ratio of the HP required to finish the race that the runner tries to keep
    /// (Stamina Keep, 持久力温存). Rolled before the race begins; zero before 2.5 Anniv.
    pub stamina_keep_ratio: f64,
    /// Whether the runner is in Stamina Keep mode. While in it, she won't compete
    /// before spurt. Reset by recovery skills.
    pub is_stamina_keep: bool,
    /// Overtake targets being tracked. If `None`, no overtake mode active.
    pub overtake_state: Option<OvertakeState>,
    /// Time spent having at least one overtake target in seconds.
    pub overtake_target_time: f64,
    /// Time spent being an overtake target of someone else in seconds.
    pub being_overtake_target_time: f64,
    /// Extra move lane position in course widths.
    ///
    /// It's set during early/mid phase based on furthest visible runner. Used as
    /// fallback in overtake mode lane selection.
    pub extra_move_lane: Option<f64>,
    /// State related to order (rank) in the race.
    pub order: OrderState,
    /// Distance differences between this runner and others.
    pub position_diff: PositionDiff,
    /// Number of visible runners in this runner's field of view.
    pub visible_count: u8,
    /// Information about nearby runners.
    pub nearby: NearbyState,
    /// Blocking state information.
    pub blocked: BlockedState,
    /// Whether the target lane must be updated this frame even if side-blocked,
    /// e.g. after being bumped by an overlapping runner.
    pub force_lane_update: bool,
    /// Surrounding state information.
    pub surrounded: SurroundState,
    /// Number of runners ahead and behind currently rushing.
    pub rush_counter: RushCounter,
    /// Current positioning strategy during the first 10 sections of the race.
    pub positioning_strategy: PositioningStrategy,
    /// Next time to check positioning strategy conditions, in seconds.
    pub next_positioning_check: f64,
    /// Current slope acceleration in m/s², if any.
    pub slope_acceleration: Option<f64>,
    /// Time spent going downhill in seconds.
    ///
    /// It's used to determine chance of downhill acceleration each second.
    pub downhill_time: f64,
    /// Number of uphills climbed to the end.
    pub uphills_climbed: u8,
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
    pub stamina_limit_break_base: f64,
    /// Active Stamina Limit Break target speed buff in m/s.
    ///
    /// It's set once the runner reaches its last spurt speed and lasts until the
    /// end of the race.
    pub stamina_limit_break: f64,
    /// All-out spurt (全開スパート) acceleration in m/s² before skill modifiers.
    ///
    /// Calculated before race begins; zero if the runner is not eligible.
    pub all_out_spurt_base: f64,
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
    /// Reference to a `RaceSolver`'s RNG, shared with other runners.
    pub rng: Rc<RefCell<rand_pcg::Pcg64Mcg>>,
    /// Structure used to determine corner entries and exits, along with skills
    /// triggered by those events.
    pub corner_manager: SegmentManager,
    /// Structure used to determine slope entries and exits, along with their
    /// acceleration/deceleration effects as well as skills triggered by slopes.
    pub slope_manager: SegmentManager,
    /// Structure used to determine straight entries and exits, along with skills
    /// triggered by those events.
    pub straight_manager: SegmentManager,
    /// Straight entered since skills were last checked, if any.
    pub entered_straight: Option<usize>,
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

    /// Number of skills the runner has, including those that failed the wisdom
    /// check.
    pub fn skill_count(&self) -> usize {
        self.available_skills.len() + self.failed_skills.len()
    }

    /// Determine the lane type based on the current lane position.
    pub fn lane_type(&self) -> LaneType {
        let lane = self.lane.current;
        if lane <= 0.2 {
            LaneType::Inner
        } else if lane <= 0.4 {
            LaneType::Middle
        } else if lane <= 0.6 {
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
        matches!(self.positioning_strategy, PositioningStrategy::PaceDown(_))
    }

    /// Update acceleration based on current speed and target speed.
    pub fn update_acceleration(&mut self, elapsed_time: f64) {
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
        self.speed + self.skill_modifiers.current_speed
    }

    /// Update the runner's speed based on acceleration and constraints.
    pub fn update_speed(&mut self, delta_time: f64, course_base_speed: f64) {
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

        // The all-out spurt keeps increasing target speed.
        // It ignores the usual acceleration formula and acceleration skills.
        if let AllOutSpurt::Active { speed_bonus } = &mut self.all_out_spurt {
            let acceleration = self.all_out_spurt_base
                + all_out_spurt::SKILL_SCALE * self.skill_modifiers.all_out_spurt_acceleration;
            *speed_bonus += acceleration * delta_time;
        }
    }

    /// Check if the runner is in her last spurt and has reached its speed.
    fn has_reached_last_spurt_speed(&self) -> bool {
        self.last_spurt.as_ref().is_some_and(|last_spurt| {
            self.position >= last_spurt.target_distance && self.speed >= last_spurt.max_speed
        })
    }

    /// Update target speed based on various factors.
    pub fn update_target_speed(&mut self, state: &RaceState) {
        let game_version = state.config.game_version;
        let target_speed = if self.hp.is_empty() {
            self.min_speed
        } else if let Some(last_spurt) = &self.last_spurt
            && self.position >= last_spurt.target_distance
        {
            last_spurt.max_speed
        } else {
            let position_multiplier = match self.positioning_strategy {
                PositioningStrategy::Normal => 1.0,
                PositioningStrategy::FrontSpeedUp => 1.04,
                PositioningStrategy::FrontOvertake => 1.05,
                PositioningStrategy::PaceUp(_) => 1.04,
                PositioningStrategy::PaceDown(_) => {
                    if game_version >= GameVersion::OnePointFiveAnniversary
                        && self.phase >= Phase::Mid
                    {
                        0.945
                    } else {
                        0.915
                    }
                }
                PositioningStrategy::PaceUpEx => 2.0,
            };
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
                * stamina_limit_break_random_factor(&mut *self.rng.borrow_mut());
        }

        // The all-out spurt is activated upon reaching last spurt speed (from 5th Anniv.).
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

        let force_in_modifier = if self.phase == Phase::Early
            && self.lane.current > FORCE_IN_MIN_LANE
            && self.blocked.side_in
        {
            self.force_in_modifier
        } else {
            0.0
        };

        let dueling_modifier = self.duel.speed_bonus();

        let struggle_modifier = self.spot_struggle.speed_bonus(state.elapsed_time);

        let secure_lead_modifier = self.secure_lead.as_ref().map_or(0.0, |s| s.modifier);

        let compete_modifier = self
            .compete_before_spurt
            .as_ref()
            .map_or(0.0, |c| c.modifier);

        self.target_speed = (target_speed
            + self.slope_acceleration.unwrap_or(0.0)
            + self.skill_modifiers.target_speed
            + self.section_speed_modifier
            + dueling_modifier
            + move_lane_modifier
            + force_in_modifier
            + struggle_modifier
            + secure_lead_modifier
            + compete_modifier
            + self.stamina_limit_break
            + self.all_out_spurt.speed_bonus())
        .min(MAX_TARGET_SPEED)
    }

    /// Schedule skills triggered in a random segment after the all-out spurt
    /// starts (`run_at_full_speed_random`). The segment is picked between the
    /// current position and the goal, and checked like any other position window.
    ///
    /// N.B. Unlike other random variables (see
    /// [`roll_condition`](crate::trigger::random::roll_condition)), it's rolled
    /// once per branch, not once per condition. In practice, at least for this
    /// variable, only one branch is left for evaluation; other branches would have
    /// contained a non-overlapping static guard that would have been false and
    /// therefore were removed when the runner was created.
    fn schedule_all_out_spurt_triggers(&mut self, state: &RaceState) {
        let course_distance = state.course.distance as f64;
        let mut rng = self.rng.borrow_mut();
        for logic_idx in self.trigger_scheduler.all_out_spurt_triggers.drain(..) {
            let start = random_segment(self.position, course_distance, &mut *rng);
            self.trigger_scheduler.position_windows.push(WindowTrigger {
                logic_idx,
                min_threshold: start,
                max_threshold: start + SEGMENT_LENGTH,
            });
        }
    }

    /// Update lane position towards target lane.
    pub fn update_lane(&mut self, course: &Course, delta_time: f64) {
        const LANE_ACCELERATION: f64 = 0.02 * 1.5;
        const MAX_SPEED: f64 = 0.6;

        // Skill modifiers override target lane if present.
        let target_lane = self.skill_modifiers.target_lane.unwrap_or(self.lane.target);

        let lane_diff = target_lane - self.lane.current;
        if lane_diff.abs() < 0.0001 {
            self.lane.current = target_lane;
            self.lane.current_speed = 0.0;
            self.lane.speed = 0.0;
            return;
        }

        let first_move_modifier =
            if self.phase <= Phase::Mid && self.position < course.move_lane_point() {
                1.0 + self.lane.current / course.width * 0.05
            } else {
                1.0
            };
        let order_modifier = if self.phase >= Phase::Late {
            1.0 + self.order.order as f64 * 0.01
        } else {
            1.0
        };
        let target_speed = 0.02
            * (0.3 + 0.001 * self.adjusted.power as f64)
            * first_move_modifier
            * order_modifier;

        // Current speed keeps changing towards the target speed even when blocked.
        let speed_step = LANE_ACCELERATION * delta_time;
        self.lane.current_speed = if self.lane.current_speed < target_speed {
            (self.lane.current_speed + speed_step).min(target_speed)
        } else {
            (self.lane.current_speed - speed_step).max(target_speed)
        };

        let moving_out = lane_diff > 0.0;
        let blocked = if moving_out {
            self.blocked.side_out
        } else {
            self.blocked.side_in
        };
        if blocked {
            self.lane.speed = 0.0;
            return;
        }

        let direction_modifier = if moving_out {
            1.0
        } else {
            -(1.0 + self.lane.current)
        };
        self.lane.speed = (self.lane.current_speed + self.skill_modifiers.lane_speed)
            .clamp(0.0, MAX_SPEED)
            * direction_modifier;

        let lane_change = self.lane.speed * delta_time;
        if lane_change.abs() >= lane_diff.abs() {
            self.lane.current = target_lane;
            self.lane.current_speed = 0.0;
        } else {
            self.lane.current += lane_change;
        }
    }

    /// Criteria for front-runners to speed up to stay ahead or catch up.
    fn can_speed_up_to_front(
        &self,
        distance_ahead: f64,
        is_single_front_runner: bool,
        game_version: GameVersion,
    ) -> bool {
        let distance_threshold = if self.positioning_style() == Style::Runaway {
            17.5
        } else if game_version >= GameVersion::OnePointFiveAnniversary && is_single_front_runner {
            12.5
        } else {
            4.5
        };

        distance_ahead < distance_threshold
    }

    /// Update positioning strategy for front-runners during the first 10 sections
    /// (5/12 of the race). The strategy determines whether to pace up to stay ahead
    /// (Speed Up) or to catch up with the first runner of the same strategy
    /// (Overtake).
    ///
    /// `lead` is the distance to the runner behind if this runner is first
    /// overall, or `None` otherwise. `strategy_lead` is the distance to the next
    /// runner with the same strategy if this runner is first among them, or `None`
    /// otherwise. Both are infinite if there's no such runner behind.
    pub fn update_positioning_front(
        &mut self,
        lead: Option<f64>,
        strategy_lead: Option<f64>,
        is_single_front_runner: bool,
        state: &RaceState,
    ) {
        let elapsed_time = state.elapsed_time;
        let game_version = state.config.game_version;
        let can_speed_up = |runner: &Self, lead| {
            runner.can_speed_up_to_front(lead, is_single_front_runner, game_version)
        };
        match self.positioning_strategy {
            PositioningStrategy::Normal => {
                if elapsed_time < self.next_positioning_check {
                    return;
                }

                self.next_positioning_check += 2.0;

                // Use original style to determine if runner was originally a front-runner that
                // should speed up.
                if self.is_rushed() && self.adjusted.style.is_front_like() {
                    self.positioning_strategy = PositioningStrategy::FrontSpeedUp;
                    return;
                }

                let chance = 0.2 * (self.adjusted.wiz as f64 * 0.1).log10();
                let pass_roll = self.rng.borrow_mut().random_bool(chance);
                if !pass_roll {
                    return;
                }

                if lead.is_some_and(|lead| can_speed_up(self, lead)) {
                    // First and not far enough ahead: try to stay ahead.
                    self.positioning_strategy = PositioningStrategy::FrontSpeedUp;
                } else if strategy_lead.is_none() {
                    // Not first in the same strategy: try to catch up.
                    self.positioning_strategy = PositioningStrategy::FrontOvertake;
                }
            }
            PositioningStrategy::FrontSpeedUp => {
                // Exit if sufficiently ahead (staying if overtaken).
                if lead.is_some_and(|lead| !can_speed_up(self, lead)) {
                    self.positioning_strategy = PositioningStrategy::Normal;
                }
            }
            PositioningStrategy::FrontOvertake => {
                // Exit if well ahead of the rest of the same strategy.
                let distance_threshold = if matches!(self.positioning_style(), Style::Runaway) {
                    27.5
                } else {
                    10.0
                };
                if strategy_lead.is_some_and(|lead| lead > distance_threshold) {
                    self.positioning_strategy = PositioningStrategy::Normal;
                }
            }
            _ => {
                debug_assert!(false, "BUG: Invalid positioning strategy for front runner")
            }
        }
    }

    /// Update positioning strategy for non-front-runners during the first 10
    /// sections (5/12 of the race).
    ///
    /// The strategy determines whether to pace up or down to keep distance from the
    /// pacemaker.
    ///
    /// `distance_behind` is the distance from the pacemaker in meters (0 if we are
    /// the pacemaker).
    pub fn update_positioning_non_front(&mut self, distance_behind: f64, state: &RaceState) {
        let elapsed_time = state.elapsed_time;
        let course_length = state.course.distance as f64;
        match self.positioning_strategy {
            PositioningStrategy::Normal => {
                if elapsed_time < self.next_positioning_check {
                    return;
                }

                self.next_positioning_check += 2.0;

                // Determine target distance range based on style and course length.
                let course_multiplier = 0.0008 * (course_length - 1000.0) + 1.0;
                let range = match self.positioning_style() {
                    Style::Pace => 3.0..(5.0 * course_multiplier),
                    Style::Late => (6.5 * course_multiplier)..(7.0 * course_multiplier),
                    Style::End => (7.5 * course_multiplier)..(8.0 * course_multiplier),
                    _ => 0.0..0.0,
                };
                if range.contains(&distance_behind) {
                    return;
                }

                if distance_behind > range.end {
                    // Catch up to a certain distance (if wiz allows).
                    let chance = 0.15 * (self.adjusted.wiz as f64 * 0.1).log10();
                    let pass_roll = self.rng.borrow_mut().random_bool(chance);
                    if !pass_roll {
                        return;
                    }

                    let threshold = self.rng.borrow_mut().random_range(range);
                    self.positioning_strategy = PositioningStrategy::PaceUp(threshold);
                } else if distance_behind < range.start
                    && self.skill_modifiers.current_speed.abs() < 0.0001
                    && self.skill_modifiers.target_speed.abs() < 0.0001
                {
                    // Pace down to a certain distance (unless affected by speed-increasing skills).
                    // 1.5 anniv. patch: if in mid-race, the maximum distance is replaced with the
                    // midpoint between min and max before rolling.
                    let threshold = if state.config.game_version
                        >= GameVersion::OnePointFiveAnniversary
                        && self.phase >= Phase::Mid
                    {
                        let min = range.start;
                        let max = range.end;
                        let mid = (min + max) / 2.0;
                        let adjusted_range = min..mid;
                        self.rng.borrow_mut().random_range(adjusted_range)
                    } else {
                        self.rng.borrow_mut().random_range(range)
                    };
                    self.positioning_strategy = PositioningStrategy::PaceDown(threshold);
                }
            }
            PositioningStrategy::PaceUp(threshold) => {
                // Exit when caught up within threshold.
                if distance_behind < threshold {
                    self.positioning_strategy = PositioningStrategy::Normal;
                    self.next_positioning_check = elapsed_time + 3.0;
                }
            }
            PositioningStrategy::PaceDown(threshold) => {
                // Exit when fallen back beyond threshold or if speed is increased by skills.
                if distance_behind > threshold
                    || self.skill_modifiers.current_speed.abs() > 0.0001
                    || self.skill_modifiers.target_speed.abs() > 0.0001
                {
                    self.positioning_strategy = PositioningStrategy::Normal;
                    self.next_positioning_check = elapsed_time + 3.0;
                }
            }
            _ => {
                debug_assert!(
                    false,
                    "BUG: Invalid positioning strategy for non-front runner"
                )
            }
        }
    }

    /// Enter or exit [`PositioningStrategy::PaceUpEx`] (1.5 anniversary and later).
    ///
    /// Returns `true` if the runner is in Pace Up Ex, in which case the regular
    /// positioning update must be skipped.
    ///
    /// `passive_ahead` tells whether any runner ahead has a style that should
    /// position her behind this runner.
    pub fn update_pace_up_ex(
        &mut self,
        passive_ahead: bool,
        pacemaker_style: Style,
        state: &RaceState,
    ) -> bool {
        if matches!(self.positioning_strategy, PositioningStrategy::PaceUpEx) {
            if !passive_ahead {
                self.positioning_strategy = PositioningStrategy::Normal;
                self.next_positioning_check = state.elapsed_time;
                return false;
            }
            return true;
        }

        let style = self.positioning_style();
        let should_enter = if style.is_front_like() {
            passive_ahead
        } else {
            pacemaker_style > style
        };
        if should_enter {
            // Overrides any other strategy.
            self.positioning_strategy = PositioningStrategy::PaceUpEx;
        }
        should_enter
    }

    /// Update the runner's position based on effective current speed.
    pub fn update_position(&mut self, delta_time: f64, state: &RaceState) {
        // Runner only moves for the part of the frame after her start delay.
        // The start delay may span several frames, so it's measured from the race
        // start. Clamped to avoid negative values from float drift in elapsed time.
        let move_time = delta_time
            .min(state.elapsed_time - self.start_delay)
            .max(0.0);
        let prev_position = self.position;
        // Use effective_speed which includes current_speed modifiers.
        self.position += self.effective_speed() * move_time;
        if self.finish_time.is_none() && self.position >= state.course.distance as f64 {
            // Calculate exact finish time.
            let remaining_distance = state.course.distance as f64 - prev_position;
            let time_to_finish = remaining_distance / self.effective_speed();
            self.finish_time = Some(state.elapsed_time - delta_time + time_to_finish);
        }
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
    pub fn update_phase(&mut self, state: &mut RaceState) {
        let prev_section = self.section;
        self.section = state.course.section_from_position(self.position);
        if self.section != prev_section {
            // Update section speed modifier.
            self.section_speed_modifier = roll_section_speed_modifier(
                &mut self.rng.borrow_mut(),
                self.adjusted.wiz,
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

            // Phase can only advance at section boundaries.
            let prev_phase = self.phase;
            self.phase = Phase::from_position(self.position, &state.course);
            if prev_phase != self.phase && self.phase >= Phase::Late && self.last_spurt.is_none() {
                self.last_spurt = Some(calculate_last_spurt(self, state));
            }
        }

        self.try_release_charge_up(state);
    }

    /// Release conserved power (脚色十分) upon entering the last spurt.
    ///
    /// Called after the position update, so the acceleration bonus starts on the
    /// next frame.
    fn try_release_charge_up(&mut self, state: &RaceState) {
        let Some(charge_up) = &self.charge_up else {
            return;
        };
        if charge_up.outcome != ChargeUpOutcome::Pending
            || !self
                .last_spurt
                .as_ref()
                .is_some_and(|l| self.position >= l.target_distance)
        {
            return;
        }

        let course = &state.course;
        let duration =
            charge_up::release_duration(charge_up.conserved, course.distance, course.distance_type);
        let outcome = if duration < charge_up::MIN_DURATION {
            ChargeUpOutcome::Insufficient
        } else {
            let activity = charge_up::activity_coefficient(
                self.spot_struggle.has_struggled(),
                self.has_rushed(),
            );
            ChargeUpOutcome::Released(ChargeUpRelease {
                conserved: charge_up.conserved,
                acceleration: charge_up::release_acceleration(
                    charge_up.power,
                    self.adjusted.style,
                    course.distance_type,
                    activity,
                ),
                duration,
                start_time: state.elapsed_time,
                start_position: self.position,
                expires_at: state.elapsed_time + duration,
            })
        };
        self.charge_up.as_mut().unwrap().outcome = outcome;
    }

    /// Conserve power (足を貯める) during the first 10 sections.
    ///
    /// Power is conserved while keeping position calmly. Speeding up, rushing or
    /// struggling for the lead prevent it, and impose a cooldown upon exit.
    pub fn update_charge_up(&mut self, delta_time: f64, elapsed_time: f64) {
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
        let rate = charge_up::position_keep_coefficient(self.is_pace_down())
            * charge_up::style_coefficient(self.adjusted.style);
        let Some(charge_up) = &mut self.charge_up else {
            return;
        };
        if blocked {
            charge_up.cooldown_until = elapsed_time + charge_up::COOLDOWN;
        } else if elapsed_time >= charge_up.cooldown_until {
            charge_up.conserved += rate * delta_time;
        }
    }

    /// Multiply the conserved power (足を貯める), if any, by `penalty`.
    fn penalize_charge_up(&mut self, penalty: f64) {
        if let Some(charge_up) = &mut self.charge_up {
            charge_up.conserved *= penalty;
        }
    }

    /// Update HP based on current speed, phase, and modifiers.
    pub fn update_hp(&mut self, delta_time: f64, state: &RaceState) {
        let mut multiplier = if self.spot_struggle.is_active(state.elapsed_time) {
            spot_struggle::hp_multiplier(self.adjusted.style, self.is_rushed())
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
        self.hp.current -= calculate_hp_per_second(
            self.speed,
            &state.course,
            state.params.track_condition,
            self.phase,
            self.adjusted.guts,
            multiplier,
        ) * delta_time;
        if self.hp.is_empty() {
            self.hp.has_emptied = true;
            // Running out of HP during the all-out spurt drops speed to minimum
            // immediately instead of decelerating (confirmed game bug).
            if self.all_out_spurt.is_active() {
                self.speed = self.min_speed;
                self.all_out_spurt = AllOutSpurt::Ended;
            }
        }
    }

    /// Get the current style used for positioning strategy decisions.
    pub fn positioning_style(&self) -> Style {
        if self.is_rushed() {
            self.rush_state.as_ref().unwrap().style
        } else {
            self.adjusted.style
        }
    }

    /// Enter rush (temptation, 掛かり) state.
    ///
    /// The runner will move ahead of the position dictated by their style, losing
    /// HP at an increased rate.
    fn start_rush(&mut self, state: &mut RaceState) {
        if let Some(rush_state) = &mut self.rush_state
            && !rush_state.is_rushed
        {
            rush_state.is_rushed = true;
            rush_state.start_time = state.elapsed_time;
            // Rushed for at least 3 seconds.
            rush_state.end_time = state.elapsed_time + 3.0;
            state.rushed_by_team[self.initial_state.team as usize][self.base_style() as usize] += 1;
            // Force positioning strategy change immediately.
            self.positioning_strategy = PositioningStrategy::Normal;
            self.next_positioning_check = state.elapsed_time;
            self.penalize_charge_up(charge_up::RUSH_PENALTY);
        }
    }

    /// Exit rush (temptation, 掛かり) state.
    fn end_rush(&mut self, state: &mut RaceState) {
        if let Some(rush_state) = &mut self.rush_state
            && rush_state.is_rushed
        {
            rush_state.is_rushed = false;
            // Ensure end time is current.
            rush_state.end_time = state.elapsed_time;
            state.rushed_by_team[self.initial_state.team as usize][self.base_style() as usize] -= 1;
            // Force positioning strategy change immediately.
            self.positioning_strategy = PositioningStrategy::Normal;
            self.next_positioning_check = state.elapsed_time;
        }
    }

    /// Update rush state based on elapsed time and random chance.
    pub fn update_rush_state(&mut self, state: &mut RaceState) {
        if let Some(rush_state) = &mut self.rush_state
            && rush_state.is_rushed
        {
            if state.elapsed_time >= rush_state.start_time + rush_state.max_duration {
                // End rush after max duration.
                self.end_rush(state);
                return;
            }
            if state.elapsed_time >= rush_state.end_time {
                // 55% chance of snapping out.
                if self.rng.borrow_mut().random_bool(0.55) {
                    self.end_rush(state);
                } else {
                    // Extend rush by 3 seconds.
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

    /// Start a spot struggle (位置取り争い), unless the runner already struggled.
    pub fn start_spot_struggle(&mut self, elapsed_time: f64) {
        if self.spot_struggle.has_struggled() {
            return;
        }
        self.spot_struggle = SpotStruggle::start(
            self.adjusted.guts,
            self.adjusted.style_aptitude,
            elapsed_time,
        );
        self.penalize_charge_up(charge_up::STRUGGLE_PENALTY);
    }

    /// End the secure lead (リード確保) speed boost once it expires.
    pub fn expire_secure_lead(&mut self, elapsed_time: f64) {
        if self
            .secure_lead
            .as_ref()
            .is_some_and(|s| elapsed_time >= s.expires_at)
        {
            self.secure_lead = None;
        }
    }

    /// Check if the runner should check whether to secure the lead on this frame.
    pub fn is_secure_lead_check_due(&self, state: &RaceState) -> bool {
        state.config.game_version >= GameVersion::TwoPointFiveAnniversary
            && self.adjusted.style != Style::End
            && !self.has_finished()
            && !self.hp.is_empty()
            && secure_lead::SECTIONS.contains(&self.section)
            && self.secure_lead.is_none()
            && state.elapsed_time >= self.next_secure_lead_check
    }

    /// Attempt to secure the lead (リード確保).
    ///
    /// If the lead over another runner with a later strategy is less than
    /// desirable, there is a chance for the runner to speed up for a short time at
    /// the cost of HP, followed by a cooldown.
    ///
    /// `is_alone` is whether no other runner with the same style is within 10m.
    pub fn try_secure_lead(&mut self, lead_insufficient: bool, is_alone: bool, state: &RaceState) {
        let elapsed_time = state.elapsed_time;
        self.next_secure_lead_check = elapsed_time + secure_lead::CHECK_INTERVAL;
        if !lead_insufficient || !self.rng.borrow_mut().random_bool(secure_lead::CHANCE) {
            return;
        }

        let style = self.adjusted.style;
        self.secure_lead = Some(SecureLeadState {
            modifier: secure_lead::target_speed_bonus(
                style,
                self.adjusted.guts,
                is_alone,
                state.config.game_version,
            ),
            expires_at: elapsed_time + secure_lead::DURATION,
        });
        self.hp.current -= secure_lead::hp_cost(style, state.course.distance);
        if self.hp.is_empty() {
            self.hp.has_emptied = true;
        }
        self.next_secure_lead_check = elapsed_time + secure_lead::DURATION + secure_lead::COOLDOWN;
    }

    /// End the compete before spurt (位置取り調整) speed boost once it expires.
    pub fn expire_compete_before_spurt(&mut self, elapsed_time: f64) {
        if self
            .compete_before_spurt
            .as_ref()
            .is_some_and(|c| elapsed_time >= c.expires_at)
        {
            self.compete_before_spurt = None;
        }
    }

    /// Returns true if `self` should check whether to compete before spurt (位置取り調整)
    /// on this frame.
    pub fn is_compete_check_due(&self, state: &RaceState) -> bool {
        state.config.game_version >= GameVersion::TwoPointFiveAnniversary
            && !self.has_finished()
            && !self.hp.is_empty()
            && compete_before_spurt::SECTIONS.contains(&self.section)
            && self.compete_before_spurt.is_none()
            && state.elapsed_time >= self.next_compete_check
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
    pub fn try_compete_before_spurt(
        &mut self,
        far_from_lead: bool,
        is_alone: bool,
        state: &RaceState,
    ) {
        let elapsed_time = state.elapsed_time;
        self.next_compete_check = elapsed_time + compete_before_spurt::CHECK_INTERVAL;

        if !self.is_stamina_keep
            && self.hp.current < hp_required_to_finish(self, state) * self.stamina_keep_ratio
            && self
                .rng
                .borrow_mut()
                .random_bool(compete_before_spurt::stamina_keep_chance(self.adjusted.wiz))
        {
            self.is_stamina_keep = true;
        }
        // ? Chance formula is unknown; set to always trigger for now.
        if self.is_stamina_keep || !(far_from_lead || self.nearby.count > 0) {
            return;
        }

        let style = self.adjusted.style;
        self.compete_before_spurt = Some(CompeteState {
            modifier: compete_before_spurt::target_speed_bonus(
                style,
                self.adjusted.power,
                self.adjusted.guts,
                is_alone,
            ),
            expires_at: elapsed_time + compete_before_spurt::DURATION,
        });
        self.hp.current -=
            compete_before_spurt::hp_cost(style, state.course.distance, far_from_lead);
        if self.hp.is_empty() {
            self.hp.has_emptied = true;
        }
        self.next_compete_check =
            elapsed_time + compete_before_spurt::DURATION + compete_before_spurt::COOLDOWN;
    }

    /// Start dueling (追い比べ), unless the runner already is.
    pub fn start_duel(&mut self) {
        if self.duel.is_dueling() {
            return;
        }
        self.duel = Duel::start(self.adjusted.guts);
    }

    /// Uncheck all overtake targets at the start of an update cycle.
    pub fn reset_overtake_checks(&mut self) {
        if let Some(state) = &mut self.overtake_state {
            for target in state.targets.values_mut() {
                target.is_checked = false;
            }
        }
    }

    /// Check if another runner qualifies as an overtake target and update tracking.
    ///
    /// Returns true if the target was added or updated.
    pub fn check_overtake_target(&mut self, target: &Runner, delta_time: f64) -> bool {
        let distance = target.position - self.position;

        // Must be at least 1m ahead. The maximum distance is given by the field of
        // view, checked by the caller.
        if distance < 1.0 {
            return false;
        }

        // Check if this is the closest blocker.
        let is_closest_blocker = self.is_front_blocked()
            && self
                .blocked
                .speed_limit
                .is_some_and(|limit| (limit - target.speed).abs() < 0.01);

        // Check if qualifies as overtake target.
        let qualifies = is_closest_blocker || {
            let speed_diff = self.speed - target.speed;
            // Can catch up within 15 seconds.
            // If speed_diff == 0, a divide by zero would occur.
            // At 1m, any speed_diff < 1/15 m/s CANNOT catch up within 15s.
            // Bigger distances require an even larger speed_diff.
            // Thus, using 1/15 as epsilon to avoid divide by zero is safe.
            let can_catch_up = if speed_diff >= (1.0 / 15.0) {
                let time_to_catch = distance / speed_diff;
                time_to_catch < 15.0
            } else {
                false
            };

            can_catch_up
                && ((target.target_speed < self.target_speed)
                    || (target.is_front_blocked() && target.speed < self.target_speed))
        };

        if !qualifies {
            return false;
        }

        // Get or create overtake state.
        if self.overtake_state.is_none() {
            self.overtake_state = Some(OvertakeState {
                targets: BTreeMap::new(),
                expires_at: None,
            });
        }

        let state = self.overtake_state.as_mut().unwrap();
        let target_entry = state
            .targets
            .entry(target.gate_roll)
            .or_insert(OvertakeTarget {
                is_checked: true,
                engagement_time: 0.0,
                is_closest_blocker: false,
            });

        target_entry.is_checked = true;
        target_entry.is_closest_blocker = is_closest_blocker;
        target_entry.engagement_time += delta_time;

        true
    }

    /// Finalize overtake state after all targets have been checked.
    ///
    /// Handles cleanup of unchecked targets and persistence timer.
    pub fn finalize_overtake_state(&mut self, delta_time: f64, elapsed_time: f64) {
        if let Some(state) = &mut self.overtake_state {
            // Remove unchecked targets.
            state.targets.retain(|_, target| target.is_checked);

            if state.targets.is_empty() {
                if let Some(expires_at) = state.expires_at {
                    if elapsed_time >= expires_at {
                        // Persistence expired, clear state.
                        self.overtake_state = None;
                        self.overtake_target_time = 0.0;
                    }
                } else {
                    // Just lost all targets, start persistence timer.
                    state.expires_at = Some(elapsed_time + 1.5);
                }
            } else {
                // Have targets, clear expiration and increment time.
                state.expires_at = None;
                self.overtake_target_time += delta_time;
            }
        }
    }

    /// Check if the runner is in overtake mode (has targets or in persistence
    /// period).
    pub fn is_in_overtake_mode(&self) -> bool {
        self.overtake_state.is_some()
    }

    /// Calculate lane target when in normal mode (not overtaking).
    pub fn normal_mode_lane_decision(
        &self,
        course: &Course,
        all_runners: &[Runner],
    ) -> LaneDecision {
        LaneDecision {
            target: self.normal_mode_lane_target(course, all_runners),
            extra_move_lane: self.extra_move_lane,
        }
    }

    /// Target lane in normal mode.
    ///
    /// Rules are checked in priority order; first match wins.
    fn normal_mode_lane_target(&self, course: &Course, all_runners: &[Runner]) -> f64 {
        // Out of HP - stay in current lane.
        if self.hp.is_empty() {
            return self.lane.current;
        }

        // Position keeping mode is pace-down.
        // Set lane target to fixed point.
        if matches!(self.positioning_strategy, PositioningStrategy::PaceDown(_)) {
            return f64::min(0.18, course.width);
        }

        // On final straight AND extra move lane is on the outside.
        if self.is_on_final_straight(course)
            && let Some(extra) = self.extra_move_lane
            && extra > self.lane.current
            && self.has_lane_space_to(extra, all_runners)
        {
            return extra.clamp(0.0, course.width);
        }

        // In early-race or mid-race, move inward.
        if matches!(self.phase, Phase::Early | Phase::Mid) {
            let target = self.lane.current - 0.05;
            if target >= 0.0 && self.has_lane_space_to(target, all_runners) {
                return target;
            }
        }

        // In mid-race and close to inside runner.
        if self.phase == Phase::Mid
            && let Some((inside_lane, distance_in_lanes)) = self.find_inside_runner(all_runners)
            && distance_in_lanes < 1.75
        {
            // Move away from inside runner.
            let target = inside_lane + horse_lanes_to_course_widths(2.0);
            if self.has_lane_space_to(target, all_runners) {
                return target.clamp(0.0, course.width);
            }
        }

        // Default - keep straight; target lane remains as-is.
        self.lane.target
    }

    /// Calculate target lane when in overtake mode.
    ///
    /// This should be called every frame after overtake state is finalized.
    pub fn overtake_lane_decision(&self, course: &Course, all_runners: &[Runner]) -> LaneDecision {
        let mut decision = LaneDecision {
            target: self.lane.target,
            extra_move_lane: self.extra_move_lane,
        };

        // Early exit if not in overtake mode or side-blocked.
        if !self.is_in_overtake_mode() || self.blocked.side() {
            return decision;
        }

        decision.extra_move_lane = self.next_extra_move_lane(all_runners);

        // Build candidate list from overtake targets and extra candidate.
        let mut candidates = Vec::with_capacity(32);
        if let Some(state) = &self.overtake_state {
            for &gate_roll in state.targets.keys() {
                self.add_target_candidates(&mut candidates, gate_roll, all_runners);
            }
        }
        self.add_extra_candidate(&mut candidates, all_runners);

        // Filter by lane space.
        candidates.retain(|c| self.has_lane_space_to(c.lane, all_runners));

        // Score candidates and select best (lowest score).
        // Keep current lane if no candidates.
        self.score_candidates(&mut candidates);
        if let Some(best) = candidates
            .iter()
            .min_by(|a, b| a.score.partial_cmp(&b.score).unwrap())
        {
            let mut target_lane = best.lane;

            // Check extra_move_lane override.
            if let Some(extra) = decision.extra_move_lane
                && target_lane < extra
                && self.has_lane_space_to(extra, all_runners)
            {
                target_lane = extra;
            }

            decision.target = target_lane.clamp(0.0, course.width);
        }

        decision
    }

    /// Find the innermost and outermost uma in a target's crowd.
    /// Crowd members are within 3m distance and less than 2 horse lanes away.
    fn find_crowd_boundaries(&self, target: &Runner, all_runners: &[Runner]) -> (f64, f64) {
        let mut inner_lane = target.lane.current;
        let mut outer_lane = target.lane.current;

        for other in all_runners {
            if other.gate_roll == target.gate_roll || other.gate_roll == self.gate_roll {
                continue;
            }

            let distance_gap = (other.position - target.position).abs();
            let lane_gap = (other.lane.current - target.lane.current).abs();

            if proximity::is_near_by(distance_gap, lane_gap, 3.0, 2.0) {
                inner_lane = inner_lane.min(other.lane.current);
                outer_lane = outer_lane.max(other.lane.current);
            }
        }

        (inner_lane, outer_lane)
    }

    /// Add overtake candidates from one target's crowd boundaries.
    fn add_target_candidates(
        &self,
        candidates: &mut Vec<OvertakeLaneCandidate>,
        target_gate: u8,
        all_runners: &[Runner],
    ) {
        let Some(target) = all_runners.iter().find(|r| r.gate_roll == target_gate) else {
            return;
        };

        let (inner, outer) = self.find_crowd_boundaries(target, all_runners);
        let reference_distance = target.position - self.position;

        // Try lane positions at ±1 horse lane from inner and outer boundaries.
        for &offset in &[-1.0, 1.0] {
            let offset = horse_lanes_to_course_widths(offset);

            for &base_lane in &[inner, outer] {
                let candidate_lane = base_lane + offset;

                if self.is_lane_candidate_valid(candidate_lane, reference_distance, all_runners) {
                    candidates.push(OvertakeLaneCandidate {
                        lane: candidate_lane,
                        score: 0.0,
                    });
                }
            }
        }
    }

    /// Add the extra candidate (straight or inward) unrelated to overtake targets.
    fn add_extra_candidate(
        &self,
        candidates: &mut Vec<OvertakeLaneCandidate>,
        all_runners: &[Runner],
    ) {
        let furthest_target_distance = self
            .overtake_state
            .as_ref()
            .and_then(|state| {
                state
                    .targets
                    .keys()
                    .filter_map(|&gate| all_runners.iter().find(|r| r.gate_roll == gate))
                    .map(|r| r.position)
                    .max_by(|a, b| a.partial_cmp(b).unwrap())
            })
            .unwrap_or(self.position);

        let candidate_lane =
            if self.phase <= Phase::Mid && self.lane.current >= horse_lanes_to_course_widths(1.0) {
                // Check 1 horse lane inward.
                self.lane.current - horse_lanes_to_course_widths(1.0)
            } else {
                // Check going straight.
                self.lane.current
            };

        let check_distance = furthest_target_distance + 3.0;

        if self.is_lane_candidate_valid(candidate_lane, check_distance, all_runners) {
            candidates.push(OvertakeLaneCandidate {
                lane: candidate_lane,
                score: 0.0,
            });
        }
    }

    /// Check if a candidate lane is valid (no runner blocking the spot).
    fn is_lane_candidate_valid(
        &self,
        candidate_lane: f64,
        reference_distance: f64,
        all_runners: &[Runner],
    ) -> bool {
        for other in all_runners {
            if other.gate_roll == self.gate_roll {
                continue;
            }

            // Check distance window: OwnerDistance <= Distance <= ReferenceDistance + 0.5.
            // Check lane window: CandidateLane ± 0.8 HorseLane.
            if proximity::is_near_by(
                other.position - self.position,
                (other.lane.current - candidate_lane).abs(),
                reference_distance + 0.5,
                0.8,
            ) {
                // Someone is blocking this candidate.
                return false;
            }
        }

        true
    }

    /// Score candidates using phase-dependent coefficients.
    fn score_candidates(&self, candidates: &mut [OvertakeLaneCandidate]) {
        for candidate in candidates {
            let lane_diff = (candidate.lane - self.lane.current).abs();
            let is_inside = candidate.lane < self.lane.current;

            let phase_coef = match self.phase {
                Phase::Early => {
                    if is_inside {
                        1.0
                    } else {
                        100.0
                    }
                }
                Phase::Mid => 1.0,
                Phase::Late | Phase::Last => {
                    if is_inside {
                        1.0
                    } else {
                        1.15
                    }
                }
            };

            candidate.score = lane_diff * phase_coef;
        }
    }

    /// Check if runner has lane space to reach target lane.
    /// Simplified check: looks for runners blocking the direct path.
    fn has_lane_space_to(&self, target_lane: f64, all_runners: &[Runner]) -> bool {
        let min_lane = self.lane.current.min(target_lane);
        let max_lane = self.lane.current.max(target_lane);
        let lane_range = min_lane..=max_lane;

        for other in all_runners {
            if other.gate_roll == self.gate_roll {
                continue;
            }

            let distance = (other.position - self.position).abs();
            let lane = other.lane.current;

            // If runner is very close and in the lane change path, consider it blocking.
            if distance < 1.05 && lane_range.contains(&lane) {
                return false;
            }
        }

        true
    }

    /// Next extra_move_lane based on furthest visible runner ahead.
    ///
    /// Only active during Early and Mid phases. Keeps the current value if no
    /// runner ahead is visible.
    fn next_extra_move_lane(&self, all_runners: &[Runner]) -> Option<f64> {
        if self.phase > Phase::Mid {
            return None;
        }

        // Find furthest visible runner ahead.
        all_runners
            .iter()
            .filter(|r| r.position > self.position && self.is_visible(r))
            .max_by(|a, b| a.position.partial_cmp(&b.position).unwrap())
            .map_or(self.extra_move_lane, |furthest| Some(furthest.lane.current))
    }

    /// Check if runner is currently on the final straight.
    pub fn is_on_final_straight(&self, course: &Course) -> bool {
        if course.straights.is_empty() {
            return false;
        }
        self.current_straight()
            .is_some_and(|idx| idx == course.straights.len() - 1)
    }

    /// Find the closest uma on the inside (lower lane value).
    /// Returns (inside_uma_lane, distance_in_horse_lanes) if found.
    fn find_inside_runner(&self, all_runners: &[Runner]) -> Option<(f64, f64)> {
        let mut closest_inside: Option<(f64, f64)> = None;

        for other in all_runners {
            // Skip self.
            if other.gate_roll == self.gate_roll {
                continue;
            }

            // Only consider runners on the inside (lower lane value).
            if other.lane.current >= self.lane.current {
                continue;
            }

            // Check if within reasonable distance.
            let position_distance = (self.position - other.position).abs();
            if position_distance > 5.0 {
                continue;
            }

            // Calculate lane distance in horse lanes.
            let lane_diff_in_horse_lanes =
                course_widths_to_horse_lanes(self.lane.current - other.lane.current);

            // Only consider if within 3 horse lanes.
            if lane_diff_in_horse_lanes > 3.0 {
                continue;
            }

            // Track closest (smallest lane difference).
            match closest_inside {
                None => {
                    closest_inside = Some((other.lane.current, lane_diff_in_horse_lanes));
                }
                Some((_, current_diff)) => {
                    if lane_diff_in_horse_lanes < current_diff {
                        closest_inside = Some((other.lane.current, lane_diff_in_horse_lanes));
                    }
                }
            }
        }

        closest_inside
    }

    /// Get the current corner index if in a corner.
    pub fn current_corner(&self) -> Option<usize> {
        self.corner_manager.current()
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
        self.straight_manager.current()
    }

    /// Check for straight entry.
    ///
    /// Skills triggered by the entry are checked on the next skill check (see
    /// [`Self::check_state_triggers`]).
    pub fn check_straight_start(&mut self, straights: &[Straight]) {
        if let Some(straight_idx) = self.straight_manager.check_entry(self.position, straights) {
            self.entered_straight = Some(straight_idx);
        }
    }

    /// Check for straight exit and update state.
    pub fn check_straight_end(&mut self, straights: &[Straight]) {
        // Only check; no triggers are fired by this event.
        self.straight_manager.check_exit(self.position, straights);
    }

    /// Get the current slope index if on a slope.
    pub fn current_slope(&self) -> Option<usize> {
        self.slope_manager.current()
    }

    /// Update slope state and apply slope-based acceleration/deceleration.
    pub fn update_slope(&mut self, slopes: &[Slope], delta_time: f64, rng: &mut impl Rng) {
        // Exit first, so that a slope right after another one is entered in the same
        // frame.
        if let Some(slope_idx) = self.slope_manager.check_exit(self.position, slopes) {
            // Exiting slope - stop any slope-based acceleration/deceleration.
            self.stop_slope_acceleration();
            if slopes[slope_idx].is_uphill() {
                self.uphills_climbed += 1;
            }
        }

        if let Some(slope_idx) = self.slope_manager.check_entry(self.position, slopes) {
            // Entering slope - apply uphill deceleration if applicable.
            if slopes[slope_idx].is_uphill() {
                self.start_uphill_deceleration(slopes[slope_idx].slope);
            } else {
                // Downhill - reset downhill timer.
                self.downhill_time = 0.0;
            }
        } else if let Some(idx) = self.current_slope() {
            // On slope - check for downhill acceleration.
            if slopes[idx].is_downhill() {
                self.downhill_time += delta_time;
                let has_second_passed = if self.downhill_time > 0.0 {
                    {
                        self.downhill_time -= 1.0;
                        true
                    }
                } else {
                    false
                };
                if has_second_passed {
                    if self.is_accelerating_downhill() {
                        // 20% chance to stop each second.
                        if rng.random_bool(0.2) {
                            self.stop_slope_acceleration();
                        }
                    } else {
                        // Wisdom * 0.04% chance to start each second.
                        let chance = self.adjusted.wiz as f64 * 0.0004;
                        if rng.random_bool(chance) {
                            self.start_downhill_acceleration(slopes[idx].slope);
                        }
                    }
                }
            }
        }
    }

    /// Update order (rank) and overtakes based on current order.
    ///
    /// The order change is negative when overtaking, and positive when being
    /// overtaken.
    pub fn update_order(&mut self, current_order: u8, course: &Course) {
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
                if self.current_corner().is_some() {
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

    /// Extend the active effects that can still be extended by overtaking, once
    /// per overtake.
    fn extend_effects_on_overtake(&mut self, overtakes: u16) {
        for timed in &mut self.active_timed_effects {
            let extension = &mut timed.overtake_extension;
            let count = overtakes.min(extension.remaining as u16) as u8;
            timed.expires_at += extension.duration * count as f64;
            extension.remaining -= count;
        }
    }

    /// Trigger the active additional effects applied on any of `triggers`,
    /// `count` times, up to their maximum.
    fn trigger_additional_effects(&mut self, triggers: &[AdditionalActivation], count: u16) {
        let mut triggered = false;
        for timed in &mut self.active_timed_effects {
            let additional = timed.effect.additional;
            if triggers.contains(&additional) {
                let total = (timed.additional_triggers as u16 + count)
                    .min(additional.max_triggers() as u16);
                triggered |= total as u8 != timed.additional_triggers;
                timed.additional_triggers = total as u8;
            }
        }
        if triggered {
            self.recalculate_skill_modifiers();
        }
    }

    /// Update order rate (percentage rank) based on total runners.
    ///
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
    ) {
        self.position_diff.from_first = position_first - self.position;
        self.position_diff.from_next = position_next - self.position;
        self.position_diff.rate_from_first = self.position_diff.from_first / position_range * 100.0;
    }

    /// Update position difference from the runner behind, and the largest lead
    /// taken before the Late race.
    ///
    /// The order must be updated first.
    pub fn update_diff_from_behind(&mut self, position_prev: f64) {
        self.position_diff.from_prev = self.position - position_prev;
        if self.order.order == 1 && self.phase < Phase::Late {
            self.position_diff.max_lead = self
                .position_diff
                .max_lead
                .max(self.position_diff.from_prev);
        }
    }

    /// Length of this runner's field of view in meters.
    ///
    /// Starts at 20m, modified by skill bonuses. The width of the field of view
    /// doesn't change, so a longer one is also narrower at a given distance.
    pub fn visible_distance(&self) -> f64 {
        // ? Assume minimum (shortened by stacked debuffs) is 0m.
        (20.0 + self.skill_modifiers.visible_distance).max(0.0)
    }

    /// Check if another runner is visible to this runner.
    pub fn is_visible(&self, other: &Self) -> bool {
        let distance = (self.position - other.position).abs();
        let visible_distance = self.visible_distance();
        // A runner without field of view sees nothing.
        visible_distance > 0.0
            && distance <= visible_distance
            && (self.lane.current - other.lane.current).abs()
                <= horse_lanes_to_course_widths((distance / visible_distance) * 11.5 + 2.0) / 2.0
    }

    /// Update front-blocked state for this runner based on another runner.
    pub fn update_front_blocked_by(&mut self, other: &Self) {
        let distance = (other.position - self.position).abs();
        self.blocked.speed_limit = Some((0.988 + 0.012 * distance) * other.speed);
    }

    /// Update side-blocked state for this runner and another.
    ///
    /// `lane_diff` is the other runner's lane minus this runner's lane. Runners in
    /// the same lane block each other in both directions.
    pub fn update_side_blocked_by(&mut self, other: &mut Self, lane_diff: f64) {
        if lane_diff >= 0.0 {
            self.blocked.side_out = true;
            other.blocked.side_in = true;
        }
        if lane_diff <= 0.0 {
            self.blocked.side_in = true;
            other.blocked.side_out = true;
        }
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
        // Both runners are near each other.
        self.nearby.count += 1;
        other.nearby.count += 1;
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
        self.nearby.behind_in = false;
        self.nearby.ahead_same_lane_count = 0;
        self.nearby.behind_same_lane_count = 0;
        self.nearby.behind_set1_count = 0;
        self.blocked.speed_limit = None;
        self.blocked.side_in = false;
        self.blocked.side_out = false;
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
        if self.blocked.side() {
            self.blocked.side_time += delta_time;
            if self.phase == Phase::Mid {
                self.blocked.mid_side_time += delta_time;
                self.blocked.max_mid_side_time = self
                    .blocked
                    .max_mid_side_time
                    .max(self.blocked.mid_side_time);
            }
        } else {
            self.blocked.side_time = 0.0;
            self.blocked.mid_side_time = 0.0;
        }
    }

    /// Check the preconditions whose window contains the current position.
    ///
    /// This must be called at the start of each frame, before any other trigger, so
    /// that a skill can activate in the same frame its precondition is satisfied.
    pub fn check_precondition_windows(&mut self, state: &RaceState) {
        let inside = self
            .trigger_scheduler
            .precondition_windows
            .advance(self.position, EPSILON);

        let mut any_unlocked = false;
        for logic_idx in inside {
            any_unlocked |= self.try_activate_skill(logic_idx, state);
        }

        // Stop checking every branch whose preconditions were just satisfied.
        if any_unlocked {
            let TriggerScheduler {
                precondition_windows,
                skill_logics,
                precondition_satisfied,
                ..
            } = &mut self.trigger_scheduler;
            precondition_windows.remove_if(|window| {
                matches!(
                    skill_logics[window.logic_idx].action,
                    LogicAction::Unlock(id) if precondition_satisfied[id]
                )
            });
        }
    }

    /// Resolve the guards that can't change during the race (e.g.
    /// `distance_type==3`), so that they aren't checked every frame.
    ///
    /// Guards that are true are removed. Logics with a false guard can never
    /// succeed, so their triggers are removed.
    fn resolve_static_guards(&mut self, state: &RaceState) {
        let is_static = |comp: &Comparison| comp.var.is_static();

        let mut unsatisfiable = Vec::new();
        for logic_idx in 0..self.trigger_scheduler.skill_logics.len() {
            let logic = &self.trigger_scheduler.skill_logics[logic_idx];
            let guards: ComparisonList = logic
                .guards
                .iter()
                .filter(|comp| {
                    !is_static(comp) || !EvaluatorFull.evaluate(comp, Some(logic), state, self)
                })
                .cloned()
                .collect();
            if guards.iter().any(is_static) {
                unsatisfiable.push(logic_idx);
            }
            self.trigger_scheduler.skill_logics[logic_idx].guards = guards;
        }

        self.trigger_scheduler.remove_triggers(&unsatisfiable);
    }

    /// Check every skill trigger.
    ///
    /// All skills are checked at once, at the start of each frame, against runner's
    /// state at that point. Nothing is updated in between, so every check sees the
    /// same values.
    pub fn check_skills(&mut self, state: &mut RaceState) {
        self.reset_activation_flags();
        self.check_race_start_triggers(state);
        // Preconditions go first, so that a skill can activate in the same frame its
        // precondition is satisfied.
        self.check_precondition_windows(state);
        self.check_state_triggers(state);
        // Depend on the activations above.
        self.check_activation_dependent_triggers(state);
        self.check_heal_dependent_triggers(state);
    }

    /// Check if the runner can activate skills during a step ending at
    /// `step_end_time`.
    ///
    /// For that, she needs to move during the step (i.e. she has left the gate by
    /// then) and hasn't finished yet.
    pub fn can_activate_skills(&self, step_end_time: f64) -> bool {
        self.start_delay <= step_end_time && !self.has_finished()
    }

    /// Check race start triggers, on the first frame the runner moves.
    ///
    /// Out-of-gate skills activate here; their pre-race effects were already
    /// applied before the race.
    pub fn check_race_start_triggers(&mut self, state: &RaceState) {
        // Taken, so that they're only checked once.
        let race_start_indices = std::mem::take(&mut self.trigger_scheduler.race_start_triggers);
        for trigger_index in race_start_indices {
            self.try_activate_skill(trigger_index, state);
        }
    }

    /// Reset activation flags before checking skills.
    pub fn reset_activation_flags(&mut self) {
        self.activation_flags = SkillActivationFlags::default();
    }

    /// Check activation-dependent triggers.
    ///
    /// This should be called after all other triggers have been checked.
    pub fn check_activation_dependent_triggers(&mut self, state: &RaceState) {
        // Pass 1: Check is_activate_any_skill triggers.
        if self.activation_flags.any_activated {
            let any_activation_indices = self.trigger_scheduler.any_activation_triggers.to_vec();
            for trigger_index in any_activation_indices {
                self.try_activate_skill(trigger_index, state);
            }
        }
    }

    /// Check heal activation-dependent triggers.
    ///
    /// This should be called after all other triggers (and any-skill
    /// activation-dependent triggers) have been checked.
    pub fn check_heal_dependent_triggers(&mut self, state: &RaceState) {
        // Pass 2: Check is_activate_heal_skill triggers.
        if self.activation_flags.heal_activated {
            let heal_activation_indices = self.trigger_scheduler.heal_activation_triggers.to_vec();
            for trigger_index in heal_activation_indices {
                self.try_activate_skill(trigger_index, state);
            }
        }
    }

    /// Check `is_other_character_activate_advantage_skill` triggers.
    ///
    /// Other runners' activations are only known once every runner has checked
    /// her skills, so this is called by the solver afterwards.
    pub fn check_advantage_triggers(&mut self, state: &RaceState) {
        let buff_activation_indices = self.trigger_scheduler.buff_activation_triggers.to_vec();
        for trigger_index in buff_activation_indices {
            self.try_activate_skill(trigger_index, state);
        }
    }

    /// Check the triggers that don't depend on this frame's activations.
    ///
    /// Windows (position, time, HP) containing the current values are checked, as
    /// well as entering the last straight since the last check.
    ///
    /// These triggers are checked together in the order the logics were built (by
    /// skill, then by alternative), so that when several alternatives of a skill
    /// are satisfied on the same frame, the first one wins.
    pub fn check_state_triggers(&mut self, state: &RaceState) {
        let entered_last_straight = self
            .entered_straight
            .take()
            .is_some_and(|straight_idx| straight_idx + 1 == state.course.straights.len());

        let scheduler = &mut self.trigger_scheduler;
        let mut inside = scheduler.position_windows.advance(self.position, EPSILON);
        inside.extend(scheduler.time_windows.advance(state.elapsed_time, EPSILON));

        // HP can recover, so HP windows are never dropped.
        let hp = self.hp.percent();
        inside.extend(
            scheduler
                .hp_windows
                .iter()
                .filter(|window| {
                    hp >= window.min_threshold - EPSILON && hp <= window.max_threshold + EPSILON
                })
                .map(|window| window.logic_idx),
        );
        if entered_last_straight {
            inside.extend(&scheduler.last_straight_triggers);
        }

        // Try activations in the order the logics were built.
        inside.sort_unstable();
        for trigger_index in inside {
            self.try_activate_skill(trigger_index, state);
        }
    }

    /// Activate a skill by its index and alternative index.
    pub fn activate_skill(
        &mut self,
        skill_index: usize,
        alternative_index: usize,
        state: &RaceState,
    ) {
        // Activating a skill triggers the additional effects of those already active,
        // before this skill's own effects are applied.
        self.trigger_additional_effects(
            &[
                AdditionalActivation::ActivateAnySkill1,
                AdditionalActivation::ActivateAnySkill2,
            ],
            1,
        );

        // Clone the skill data we need to avoid borrow checker issues.
        let skill_id = self.available_skills[skill_index].id.clone();
        let alternative =
            self.available_skills[skill_index].alternatives[alternative_index].clone();

        // Scale with the runner's situation, distance and effects. Effects lasting
        // until the end of the race have an infinite base duration, which stays
        // infinite.
        let duration_factor =
            state.course.distance as f64 / 1000.0 * self.skill_modifiers.duration_scaling;
        let scaled_duration = self.situational_base_duration(&alternative) * duration_factor;
        // Extensions from overtaking scale like the base duration.
        let overtake_extension = if alternative.time_usage == TimeUsage::IncrementOrderUp {
            OvertakeExtension {
                duration: skill_scaling::OVERTAKE_EXTENSION * duration_factor,
                remaining: skill_scaling::MAX_OVERTAKE_EXTENSIONS,
            }
        } else {
            OvertakeExtension::default()
        };

        // Wiz Limit Break bonus, fixed at activation time (5th Anniv. onward).
        let wiz_bonus = if state.config.game_version >= GameVersion::FifthAnniversary
            && self.available_skills[skill_index].is_wiz_limit_break_eligible()
        {
            wiz_limit_break_phase_coefficient(self.adjusted.style, self.phase)
                * wiz_limit_break_multiplier(self.adjusted.wiz)
        } else {
            0.0
        };

        // Apply each effect in the alternative. Effects on other runners are applied
        // later by the solver, which knows about every runner.
        // N.B. Pre-race effects were applied before the race (see
        // `UnreadyRunner::apply_out_of_gate_skills`), but still count as activated.
        for effect in &alternative.effects {
            if !effect.skill_type.is_pre_race_effect() {
                // Scale the value with the runner's situation.
                let scaled = SkillEffect {
                    modifier: self.scaled_modifier(effect),
                    ..effect.clone()
                };
                if scaled.target == SkillTarget::Oneself {
                    self.apply_skill_effect(
                        &scaled,
                        scaled_duration,
                        overtake_extension,
                        wiz_bonus,
                        state,
                    );
                } else {
                    self.pending_effects.push(PendingEffect {
                        effect: scaled,
                        duration: scaled_duration,
                        wiz_bonus,
                    });
                }
            }
            // Buffs count as activated even if their value scaled down to zero.
            if effect.target == SkillTarget::Oneself && Skill::is_effect_positive(effect) {
                self.activation_flags
                    .buff_activated
                    .insert(effect.skill_type);
            }
        }

        // Recalculate skill modifiers from active effects.
        self.recalculate_skill_modifiers();

        self.start_cooldown(skill_index, alternative_index, state);
        self.disable_other_alternatives(skill_index, alternative_index);

        // Track skill activation.
        *self
            .activated_skill_ids
            .entry(skill_id.clone())
            .or_default() |= 1 << alternative_index;
        self.activated_alternatives
            .insert((skill_index, alternative_index));
        self.activated_skills.push(SkillActivationRecord {
            skill_id,
            distance: self.position,
        });

        // Increment activation counts for condition evaluation.
        let section_length = state.course.distance as f64 / 24.0;
        self.trigger_scheduler.activation_counts.increment(
            self.phase,
            self.position,
            section_length,
        );

        // Set activation flags for activation-dependent triggers.
        self.activation_flags.any_activated = true;
        if self.is_heal_skill_by_index(skill_index) {
            self.activation_flags.heal_activated = true;
        }
    }

    /// Given a trigger index, attempt to activate the associated skill.
    ///
    /// Returns whether the logic succeeded.
    fn try_activate_skill(&mut self, trigger_index: usize, state: &RaceState) -> bool {
        // Get skill logic and conditions.
        let logic = &self.trigger_scheduler.skill_logics[trigger_index];

        // Check if precondition is satisfied; skip if not.
        if let Some(id) = logic.precondition
            && !self.trigger_scheduler.precondition_satisfied[id]
        {
            return false;
        }

        if state.elapsed_time < logic.cooldown_until {
            return false;
        }
        let skill_index = logic.skill_index;
        let alternative_index = logic.alternative_index;
        let action = logic.action;
        let should_activate =
            evaluate_comparison_list(&logic.guards, &EvaluatorFull, Some(logic), state, self);

        if should_activate {
            match action {
                // Starts the cooldown.
                LogicAction::Activate => self.activate_skill(skill_index, alternative_index, state),
                LogicAction::Unlock(id) => {
                    self.trigger_scheduler.precondition_satisfied[id] = true;
                    // Never checked again.
                    self.trigger_scheduler.skill_logics[trigger_index].cooldown_until =
                        f64::INFINITY;
                }
            }
        }
        should_activate
    }

    /// Start the cooldown of a skill alternative, shared by all its OR branches.
    ///
    /// The base cooldown scales with the course distance by a factor of *course
    /// distance / 1000*.
    fn start_cooldown(&mut self, skill_index: usize, alternative_index: usize, state: &RaceState) {
        let base_cooldown =
            self.available_skills[skill_index].alternatives[alternative_index].base_cooldown;
        let cooldown = base_cooldown * state.course.distance as f64 / 1000.0;
        let cooldown_until = state.elapsed_time + cooldown;

        for logic in &mut self.trigger_scheduler.skill_logics {
            if logic.skill_index == skill_index
                && logic.alternative_index == alternative_index
                && logic.action == LogicAction::Activate
            {
                logic.cooldown_until = cooldown_until;
            }
        }
    }

    /// Disable the other alternatives of a skill once one of them activates, as a
    /// skill activates through a single alternative.
    ///
    /// Continuations (see [`SkillAlternative::is_continuation`]) are the exception:
    /// they can only activate after another alternative did.
    ///
    /// When several alternatives are satisfied on the same frame, the first one
    /// checked wins. Triggers that don't depend on this frame's activations are
    /// checked by alternative (see [`Self::check_state_triggers`]), but
    /// activation-dependent ones (any skill, heal, other runners' buffs) are
    /// checked after them, as they depend on their activations.
    ///
    /// N.B.: A later alternative could then win over an activation-dependent
    /// earlier one. It's assumed that no skill will have such alternatives.
    fn disable_other_alternatives(&mut self, skill_index: usize, alternative_index: usize) {
        let alternatives = &self.available_skills[skill_index].alternatives;
        for logic in &mut self.trigger_scheduler.skill_logics {
            if logic.skill_index == skill_index
                && logic.alternative_index != alternative_index
                && !alternatives[logic.alternative_index].is_continuation()
            {
                // Never checked again.
                logic.cooldown_until = f64::INFINITY;
            }
        }
    }

    /// Base duration of a skill alternative scaled with the runner's situation
    /// (see [`TimeUsage`]), before distance scaling.
    fn situational_base_duration(&self, alternative: &SkillAlternative) -> f64 {
        use TimeUsage::*;
        let base = alternative.base_duration;
        match alternative.time_usage {
            // Overtakes extend the effects later instead.
            Direct | IncrementOrderUp => base,
            MultiplyDistanceDiffTop => {
                let distance = self.position_diff.from_first;
                base * skill_scaling::distance_from_first_duration_multiplier(distance)
            }
            MultiplyRemainHp1 => {
                base * skill_scaling::remaining_hp_duration_multiplier_1(self.hp.current)
            }
            MultiplyRemainHp2 => {
                base * skill_scaling::remaining_hp_duration_multiplier_2(self.hp.current)
            }
            MultiplyBlockedSideMaxContinueTimePhaseMiddleRun => {
                let time = self.blocked.max_mid_side_time;
                base * skill_scaling::mid_side_blocked_time_multiplier(time)
            }
            IncrementUphillCount => {
                base + skill_scaling::uphill_duration_bonus(self.uphills_climbed)
            }
        }
    }

    /// Value of an effect's modifier scaled with the runner's situation (see
    /// [`ValueUsage`]).
    fn scaled_modifier(&self, effect: &SkillEffect) -> f64 {
        use ValueUsage::*;
        let modifier = effect.modifier;
        match effect.value_usage {
            Direct => modifier,
            MultiplySkillNum => {
                modifier * skill_scaling::skill_count_multiplier(self.skill_count())
            }
            Aoharu => modifier * skill_scaling::AOHARU_MULTIPLIER,
            // Both variants share the same multipliers for now.
            MultiplyRandom1 | MultiplyRandom2 => {
                let roll = self.rng.borrow_mut().random();
                modifier * skill_scaling::random_multiplier(roll)
            }
            Climax => modifier * skill_scaling::CLIMAX_MULTIPLIER,
            MultiplyMaximumRawStatus => {
                let raw = &self.raw;
                let max_stat = [raw.speed, raw.stamina, raw.power, raw.guts, raw.wiz]
                    .into_iter()
                    .max()
                    .unwrap();
                modifier * skill_scaling::max_raw_stat_multiplier(max_stat)
            }
            MultiplyActivateSpecificTagSkillCount => {
                let green_count = self
                    .available_skills
                    .iter()
                    .filter(|skill| {
                        skill.is_green() && self.activated_skill_ids.contains_key(&skill.id)
                    })
                    .count();
                modifier * skill_scaling::green_skill_count_multiplier(green_count)
            }
            AddDistanceDiffTop => {
                modifier + skill_scaling::distance_from_first_bonus(self.position_diff.from_first)
            }
            MultiplyBlockedSideMaxContinueTimePhaseMiddleRun => {
                let time = self.blocked.max_mid_side_time;
                modifier * skill_scaling::mid_side_blocked_time_multiplier(time)
            }
            MultiplySpeed1 => modifier * skill_scaling::speed_multiplier_1(self.adjusted.speed),
            MultiplySpeed2 => modifier * skill_scaling::speed_multiplier_2(self.adjusted.speed),
            MultiplyArcGlobalPotentialLevel => modifier * skill_scaling::L_ARC_MULTIPLIER,
            MultiplyTopLeadAmount => {
                modifier * skill_scaling::lead_multiplier(self.position_diff.max_lead)
            }
            CornerOvertakeCount => {
                let overtakes = self.order.overtakes_in_late_corners;
                modifier * skill_scaling::late_corner_overtake_multiplier(overtakes)
            }
            FanCount => {
                modifier * skill_scaling::fan_count_multiplier(self.initial_state.fan_count)
            }
            UafWinCount => modifier * skill_scaling::UAF_WIN_MULTIPLIER,
            CookingPoints => modifier * skill_scaling::COOKING_MULTIPLIER,
            ResearchLevel => modifier * skill_scaling::RESEARCH_MULTIPLIER,
            FanLove => modifier * skill_scaling::FAN_LOVE_MULTIPLIER,
            DevelopmentPoints => modifier * skill_scaling::DEVELOPMENT_MULTIPLIER,
        }
    }

    /// Apply a single skill effect to the runner.
    ///
    /// Effects can be categorized as follows:
    /// - Timed effects (speed, acceleration...): modify a stat for a certain
    ///   duration that is constantly tracked.
    /// - Recovery: instant effect that recovers HP.
    pub fn apply_skill_effect(
        &mut self,
        effect: &SkillEffect,
        scaled_duration: f64,
        overtake_extension: OvertakeExtension,
        wiz_bonus: f64,
        state: &RaceState,
    ) {
        let effect_type = &effect.skill_type;

        // Pre-race effects (stat boosts, start delay...) never reach this point (see
        // `activate_skill`).

        // Timed effects (tracked in active_timed_effects).
        if effect_type.is_timed_effect() {
            let expires_at = state.elapsed_time + scaled_duration;
            // Apply Wiz Limit Break bonus to target and current speed effects.
            let bonus = match effect_type {
                SkillType::TargetSpeed | SkillType::CurrentSpeedWithNaturalDeceleration => {
                    wiz_bonus
                }
                _ => 0.0,
            };
            self.active_timed_effects.push(ActiveTimedEffect {
                effect: effect.clone(),
                value: effect.modifier * (1.0 + bonus),
                expires_at,
                overtake_extension,
                additional_triggers: 0,
            });
        }
        // Recovery (HP restoration, or HP loss if negative), as a fraction of max HP.
        else if matches!(effect_type, SkillType::Recovery) {
            self.hp.recover(self.hp.maximum * effect.modifier);

            // HP loss (e.g., a skill's drawback) is not a heal.
            if effect.modifier > 0.0 {
                // Increment heal skill count.
                self.trigger_scheduler.activation_counts.increment_heal();

                // Recovering HP resets Stamina Keep (2.5 Anniv. onward).
                self.is_stamina_keep = false;

                // Recalculate last spurt if in Late phase or later (1st Anniv. onward).
                if state.config.game_version >= GameVersion::FirstAnniversary
                    && self.phase >= Phase::Late
                {
                    self.last_spurt = Some(calculate_last_spurt(self, state));
                }
            }
        }
        // Activate random gold skills (one per unit of modifier).
        else if matches!(effect_type, SkillType::ActivateRandomGold) {
            self.activate_random_gold_skills(effect.modifier as usize, state);
        }
        // Activate unique skills again (one per unit of modifier).
        else if matches!(effect_type, SkillType::ActivateUnique) {
            self.activate_unique_skills(effect.modifier as usize, state);
        }
        // Rush time extension.
        else if matches!(effect_type, SkillType::RushTimeUp) {
            // Extend only if the Uma is bound to rush at all.
            // ? Assume it also applies if already rushing for now.
            if let Some(rush_state) = &mut self.rush_state {
                rush_state.max_duration += effect.modifier;
            }
        }
    }

    /// Activate up to `count` random gold skills that haven't activated yet,
    /// bypassing their conditions.
    ///
    /// The first alternative of each skill is activated, so that later ones that
    /// continue it (see [`SkillAlternative::is_continuation`]) can still activate.
    fn activate_random_gold_skills(&mut self, count: usize, state: &RaceState) {
        let mut candidates: Vec<usize> = self
            .golden_skill_indices
            .iter()
            .copied()
            .filter(|&skill_index| {
                !self
                    .activated_alternatives
                    .iter()
                    .any(|&(activated, _)| activated == skill_index)
            })
            .collect();
        for _ in 0..count {
            if candidates.is_empty() {
                break;
            }
            let idx = self.rng.borrow_mut().random_range(0..candidates.len());
            let skill_index = candidates.swap_remove(idx);
            // ? Assume the first alternative is activated, even when the others are
            // variants for other courses or running styles (e.g. "Piercing Azure").
            self.activate_skill(skill_index, 0, state);
        }
    }

    /// Activate up to `count` unique skills, bypassing their conditions and
    /// cooldowns.
    ///
    /// Each activation is a new instance of the skill: its values (duration, Wiz
    /// Limit Break bonus...) are calculated anew, and its effects stack with any
    /// still active from a previous activation. The (first) alternative that
    /// activated before is used, or the first one if the skill hasn't activated
    /// yet.
    ///
    /// N.B. Out-of-gate skills are skipped, as their effects are only applied
    /// before the race.
    fn activate_unique_skills(&mut self, count: usize, state: &RaceState) {
        let unique_indices: Vec<usize> = self
            .available_skills
            .iter()
            .enumerate()
            .filter(|(_, skill)| skill.is_unique() && !skill.should_activate_out_of_gate())
            .map(|(i, _)| i)
            .take(count)
            .collect();
        for skill_index in unique_indices {
            let alternative_index = self
                .activated_alternatives
                .iter()
                .filter(|&&(activated, _)| activated == skill_index)
                .map(|&(_, alternative_index)| alternative_index)
                // Lowest, as the set's order isn't deterministic.
                .min()
                .unwrap_or(0);
            self.activate_skill(skill_index, alternative_index, state);
        }
    }

    /// Apply a skill effect casted by another runner.
    pub fn receive_skill_effect(&mut self, pending: &PendingEffect, state: &RaceState) {
        // Overtakes only extend the caster's own effects.
        self.apply_skill_effect(
            &pending.effect,
            pending.duration,
            OvertakeExtension::default(),
            pending.wiz_bonus,
            state,
        );
        self.recalculate_skill_modifiers();
    }

    /// Check if a skill at the given index is a heal skill.
    ///
    /// A heal skill has an alternative that contains a positive Recovery effect.
    fn is_heal_skill_by_index(&self, skill_index: usize) -> bool {
        let skill = &self.available_skills[skill_index];
        skill.alternatives.iter().any(|alt| {
            alt.effects
                .iter()
                .any(|eff| matches!(eff.skill_type, SkillType::Recovery) && eff.modifier > 0.0)
        })
    }

    /// Update skill durations and remove expired effects.
    pub fn update_skill_durations(&mut self, elapsed_time: f64) {
        let before_count = self.active_timed_effects.len();

        // Transfer expiring CurrentSpeedWithNaturalDeceleration modifiers into the
        // runner's speed, so that it decelerates naturally instead of dropping
        // instantly.
        self.active_timed_effects.iter().for_each(|effect| {
            if effect.expires_at <= elapsed_time
                && matches!(
                    effect.effect.skill_type,
                    SkillType::CurrentSpeedWithNaturalDeceleration
                )
            {
                self.speed += effect.current_value();
            }
        });

        // Remove expired effects.
        self.active_timed_effects
            .retain(|effect| effect.expires_at > elapsed_time);

        // Recalculate modifiers if any effects expired.
        if self.active_timed_effects.len() != before_count {
            self.recalculate_skill_modifiers();
        }
    }

    /// Recalculate skill modifiers from active timed effects.
    fn recalculate_skill_modifiers(&mut self) {
        use SkillType::*;
        let mut mods = SkillModifiers::default();

        for timed in &self.active_timed_effects {
            match timed.effect.skill_type {
                Acceleration => {
                    mods.acceleration += timed.current_value();
                }
                AccelerationAllOutSpurt => {
                    mods.all_out_spurt_acceleration += timed.current_value();
                }
                TargetSpeed => {
                    mods.target_speed += timed.current_value();
                }
                CurrentSpeed | CurrentSpeedWithNaturalDeceleration => {
                    mods.current_speed += timed.current_value();
                }
                LaneMovementSpeed => {
                    mods.lane_speed += timed.current_value();
                }
                Fov => {
                    mods.visible_distance += timed.current_value();
                }
                LaneChange => {
                    mods.target_lane = Some(timed.current_value());
                }
                ExtendEvolvedDuration => {
                    // Duration scaling is multiplicative.
                    mods.duration_scaling *= timed.current_value();
                }
                _ => {}
            }
        }

        self.skill_modifiers = mods;
    }
}
