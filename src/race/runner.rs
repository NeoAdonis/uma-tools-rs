use std::cell::RefCell;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::rc::Rc;

use rand::prelude::*;

use crate::condition::evaluator::{CompEvaluator, EvaluatorFull, evaluate_comparison_list};
use crate::condition::parser::{Comparison, ComparisonList};
use crate::configuration::GameVersion;
use crate::course::geometry::{Corner, Slope, Straight};
use crate::course::*;
use crate::race::charge_up::{self, ChargeUp, ChargeUpOutcome, ChargeUpRelease};
use crate::race::full_throttle::{self, FullThrottle};
use crate::race::hp::*;
use crate::race::last_spurt::*;
use crate::race::modifiers::*;
use crate::race::popularity::JudgeScores;
use crate::race::secure_lead;
use crate::race::solver::RaceState;
use crate::race::transitions::*;
use crate::race::unready_runner::UnreadyRunner;
use crate::skill::*;
use crate::trigger::skill_activation::{ActiveTimedEffect, LogicAction, SkillActivationRecord};
use crate::trigger::system::{DistanceTrigger, TriggerScheduler};
use crate::uma::*;

/// Skill modifiers that are applied during race calculations.
#[derive(Debug, Clone)]
pub struct SkillModifiers {
    /// Modifies target speed. Positive values increase it.
    pub target_speed: f64,
    /// Modifies current speed directly. Positive values increase it.
    pub current_speed: f64,
    /// Modifies acceleration. Positive values increase it.
    pub acceleration: f64,
    /// Modifies Full-Throttle Sprint acceleration (listed value, before scaling).
    pub full_throttle_acceleration: f64,
    /// Modifies lane movement speed. Positive values increase lane change speed.
    pub lane_speed: f64,
    /// Modifies visible distance. Positive values increase it.
    pub visible_distance: f64,
    /// Sets the target lane directly (0.0 to 1.0, relative to course width).
    /// `None` means no change.
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
            full_throttle_acceleration: 0.0,
            lane_speed: 0.0,
            visible_distance: 0.0,
            target_lane: None,
            duration_scaling: 1.0, // Multiplicative default
        }
    }
}

/// Maximum target speed a runner can reach in m/s.
pub const MAX_TARGET_SPEED: f64 = 30.0;

/// Tolerance for floating-point comparisons in skill triggers.
const EPSILON: f64 = 0.0001;

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
    use SkillType::*;
    matches!(
        effect_type,
        Acceleration
            | AccelerationFullThrottle
            | TargetSpeed
            | CurrentSpeed
            | CurrentSpeedWithNaturalDeceleration
            | Fov
            | ExtendEvolvedDuration
            | LaneMovementSpeed
            | LaneChange
    )
}

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

/// Modifier applied during spot struggle and its expiration time.
pub struct StruggleState {
    modifier: f64,
    expires_at: f64,
}

/// Speed boost applied while securing the lead (リード確保) and its expiration time.
pub struct SecureLeadState {
    pub modifier: f64,
    pub expires_at: f64,
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

/// First section where positioning strategies no longer apply.
pub const POSITIONING_END_SECTION: u8 = 10;

/// Positioning strategy during the first 10 sections (5/12) of the race.
/// For front-runners, it determines the pacing behavior to stay ahead or catch up.
/// For non-front-runners, it determines the pacing behavior to keep their
/// distance from the pacemaker.
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
    /// Front-runners enter it when any such runner is ahead; other runners
    /// enter it when the pacemaker is such a runner. Exits once no such runner
    /// is ahead. Takes priority over all other strategies.
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
    /// Current lateral speed in m/s.
    pub speed: f64,
}

impl LaneState {
    pub fn new(lane: f64) -> Self {
        Self {
            current: lane,
            target: lane,
            speed: 0.0,
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
    /// Gate number (1-indexed). It determines starting lane position.
    /// It is also used as a unique identifier for the runner.
    pub gate_roll: u8,
    /// Base stats before adjustments. After calculating the adjusted stats,
    /// the base stats are only used to evaluate some skill conditions.
    pub base: Uma,
    /// Adjusted stats after applying modifiers and out-of-gate skills.
    /// For the most part, these are the stats used for race calculations.
    pub adjusted: Uma,
    /// Set of static values related to the runner at the start of the race.
    pub initial_state: InitialState,
    /// Start delay (i.e. time taken to leave the gate) in seconds.
    pub start_delay: f64,
    /// Current speed in m/s. Besides position updates, it also affects HP consumption.
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
    /// Spped modifier applied early in the race to encourage running towards the
    /// inner fence. Calculated before race begins based on running style.
    pub force_in_modifier: f64,
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
    /// Chance of entering rush (temptation, 掛かり) state as a percentage.
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
    /// State related to secure lead. If `None`, the runner is not securing the lead.
    pub secure_lead: Option<SecureLeadState>,
    /// Time of the next secure lead check in seconds.
    pub next_secure_lead_check: f64,
    /// Duel targets being tracked for potential dueling. Gate number is used as key.
    pub duel_targets: HashMap<u8, DuelTarget>,
    /// Count of duels the runner has engaged in.
    pub duel_count: u8,
    /// Overtake targets being tracked. If `None`, no overtake mode active.
    pub overtake_state: Option<OvertakeState>,
    /// Time spent having at least one overtake target in seconds.
    pub overtake_target_time: f64,
    /// Time spent being an overtake target of someone else in seconds.
    pub being_overtake_target_time: f64,
    /// Extra move lane position in meters. Set during early/mid phase based on
    /// furthest visible runner. Used as fallback in overtake mode lane selection.
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
    /// Surrounding state information.
    pub surrounded: SurroundState,
    /// Number of runners currently in rush (temptation, 掛かり) state ahead and behind.
    pub rush_counter: RushCounter,
    /// Current positioning strategy during the first 10 sections (5/12) of the race.
    pub positioning_strategy: PositioningStrategy,
    /// Next time to check positioning strategy conditions, in seconds.
    pub next_positioning_check: f64,
    /// Current slope acceleration in m/s², if any.
    pub slope_acceleration: Option<f64>,
    /// Time spent going downhill in seconds. Used to determine chance of downhill
    /// acceleration each second.
    pub downhill_time: f64,
    /// Current section index (0-23).
    pub section: u8,
    /// Current race phase.
    pub phase: Phase,
    /// Information about the last spurt phase. It is calculated when entering the
    /// Mid phase. It indicates when the last spurt can start and its max speed.
    pub last_spurt: Option<LastSpurt>,
    /// Stamina Limit Break (スタミナ勝負) buff before its random factor, in m/s.
    /// Calculated before race begins; zero if the runner is not eligible.
    pub stamina_limit_break_base: f64,
    /// Active Stamina Limit Break target speed buff in m/s. It is set once the
    /// runner reaches its last spurt speed and lasts until the end of the race.
    pub stamina_limit_break: f64,
    /// Full-Throttle Sprint (全開スパート) acceleration in m/s² before skill modifiers.
    /// Calculated before race begins; zero if the runner is not eligible.
    pub full_throttle_base: f64,
    /// Full-Throttle Sprint state. It is activated once the runner reaches its
    /// last spurt speed, and keeps increasing target speed until the end of the race.
    pub full_throttle: FullThrottle,
    /// Charge Up (足を貯める) state. It records the power conserved during the
    /// first 10 sections and its release upon entering the last spurt.
    /// `None` if the runner is not eligible.
    pub charge_up: Option<ChargeUp>,
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
    /// Create a new Runner from an UnreadyRunner and RaceState.
    pub(crate) fn new(mut builder: UnreadyRunner, state: &RaceState) -> Self {
        // Apply out-of-gate skills with proper activation tracking
        builder.apply_out_of_gate_skills(state);

        // Build final Runner with all modifications applied
        let mut runner = builder.build(state);
        runner.resolve_static_guards(state);
        runner
    }

    /// Get the base style, treating Runaway as a variant of Front.
    pub fn base_style(&self) -> Style {
        self.adjusted.style.base_style()
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

        let dueling_modifier = self
            .dueling
            .as_ref()
            .map_or(0.0, |d| d.acceleration_modifier);

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
        // Update speed based on acceleration
        self.speed += self.acceleration * delta_time;

        // Handle start dash end and min speed
        if self.is_start_dash && self.speed >= 0.85 * course_base_speed {
            self.is_start_dash = false;
        } else if !self.is_start_dash && self.speed < self.min_speed {
            self.speed = self.min_speed;
        }
        // Cap speed along with target speed to avoid overshooting due to acceleration.
        // ? Is overshooting allowed? Cap it for now to avoid spikes.
        self.speed = self.speed.min(MAX_TARGET_SPEED);

        // Full-Throttle Sprint keeps increasing target speed.
        // It ignores the usual acceleration formula and acceleration skills.
        if let FullThrottle::Active { speed_bonus } = &mut self.full_throttle {
            let acceleration = self.full_throttle_base
                + full_throttle::SKILL_SCALE * self.skill_modifiers.full_throttle_acceleration;
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

        // Stamina Limit Break is activated upon reaching last spurt speed (2.5 Anniv. onward)
        if self.stamina_limit_break == 0.0
            && self.stamina_limit_break_base > 0.0
            && self.has_reached_last_spurt_speed()
        {
            self.stamina_limit_break = self.stamina_limit_break_base
                * stamina_limit_break_random_factor(&mut *self.rng.borrow_mut());
        }

        // Full-Throttle Sprint is activated upon reaching last spurt speed (5th Anniv. onward)
        if self.full_throttle == FullThrottle::Pending
            && self.full_throttle_base > 0.0
            && !self.hp.is_empty()
            && self.has_reached_last_spurt_speed()
        {
            self.full_throttle = FullThrottle::Active { speed_bonus: 0.0 };
            self.schedule_full_throttle_triggers(state);
        }

        // TODO: Also check if runner changed lanes on last frame
        let move_lane_modifier = if game_version >= GameVersion::FirstAnniversary
            && self.skill_modifiers.lane_speed > 0.0
        {
            (0.0002 * self.adjusted.power as f64).sqrt()
        } else {
            0.0
        };

        // TODO: Apply only when an uma is more than 0.12 course width away from the inner fence and inside is open.
        let force_in_modifier = if matches!(self.phase, Phase::Early) {
            self.force_in_modifier
        } else {
            0.0
        };

        let dueling_modifier = self
            .dueling
            .as_ref()
            .map_or(0.0, |d| d.target_speed_modifier);

        let struggle_modifier = self.spot_struggle.as_ref().map_or(0.0, |s| s.modifier);

        let secure_lead_modifier = self.secure_lead.as_ref().map_or(0.0, |s| s.modifier);

        self.target_speed = (target_speed
            + self.slope_acceleration.unwrap_or(0.0)
            + self.skill_modifiers.target_speed
            + self.section_speed_modifier
            + dueling_modifier
            + move_lane_modifier
            + force_in_modifier
            + struggle_modifier
            + secure_lead_modifier
            + self.stamina_limit_break
            + self.full_throttle.speed_bonus())
        .min(MAX_TARGET_SPEED)
    }

    /// Schedule skills triggered at a random point after the Full-Throttle Sprint
    /// starts (`run_at_full_speed_random`). The point is picked between the
    /// current position and the goal.
    fn schedule_full_throttle_triggers(&mut self, state: &RaceState) {
        let course_distance = state.course.distance as f64;
        let mut rng = self.rng.borrow_mut();
        for logic_idx in self.trigger_scheduler.full_throttle_triggers.drain(..) {
            let threshold_meters = if self.position < course_distance {
                rng.random_range(self.position..course_distance)
            } else {
                self.position
            };
            self.trigger_scheduler
                .distance_triggers
                .push(DistanceTrigger {
                    threshold_meters,
                    logic_idx,
                });
        }
    }

    /// Update lane position towards target lane.
    pub fn update_lane(&mut self, course: &Course, delta_time: f64) {
        const LANE_ACCELERATION: f64 = 0.03; // m/s²

        // Skill modifiers override target lane if present
        let target_lane = if let Some(skill_target) = self.skill_modifiers.target_lane {
            skill_target * course.width()
        } else {
            self.lane.target
        };

        let lane_diff = target_lane - self.lane.current;
        if lane_diff.abs() < 0.0001 {
            self.lane.current = target_lane;
            self.lane.speed = 0.0;
            return;
        }

        let order_modifier = if self.phase >= Phase::Late {
            1.0 + self.order.order as f64 * 0.01
        } else {
            1.0
        };
        // TODO: obtain proper first move lane point
        let first_move_modifier = if self.phase == Phase::Early {
            1.0 + self.lane.current / course.width() * 0.05
        } else {
            1.0
        };

        let target_speed = 0.02
            * (0.3 + 0.001 * self.adjusted.power as f64)
            * first_move_modifier
            * order_modifier;
        self.lane.speed = (self.lane.speed + LANE_ACCELERATION * delta_time).min(target_speed);

        let skill_modifier = self.skill_modifiers.lane_speed;
        let direction_modifier = if lane_diff > 0.0 {
            1.0
        } else {
            -(1.0 + self.lane.current)
        };

        let actual_speed = (self.lane.speed + skill_modifier).clamp(0.0, 0.6) * direction_modifier;
        let actual_change = actual_speed * delta_time;

        // TODO: check if blocked from lane change
        if actual_change.abs() >= lane_diff.abs() {
            self.lane.current = target_lane;
            self.lane.speed = 0.0;
        } else {
            self.lane.current += actual_change;
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
    /// (5/12 of the race).
    /// The strategy determines whether to pace up to stay ahead or catch up with
    /// first place.
    ///
    /// `distance_ahead` is the distance to the runner behind IF this runner is in first place,
    /// or 0 otherwise.
    pub fn update_positioning_front(
        &mut self,
        distance_ahead: f64,
        is_single_front_runner: bool,
        state: &RaceState,
    ) {
        let elapsed_time = state.elapsed_time;
        let game_version = state.config.game_version;
        match self.positioning_strategy {
            PositioningStrategy::Normal => {
                if elapsed_time < self.next_positioning_check {
                    return;
                }

                self.next_positioning_check += 2.0;

                // Use original style to determine if runner was originally a
                // front-runner that should speed up.
                if self.is_rushed() && self.adjusted.style.is_front_like() {
                    self.positioning_strategy = PositioningStrategy::FrontSpeedUp;
                    return;
                }

                let chance = 0.2 * (self.adjusted.wiz as f64 * 0.1).log10();
                let pass_roll = self.rng.borrow_mut().random_bool(chance);
                if !pass_roll {
                    return;
                }

                if distance_ahead >= 0.0 {
                    if !self.can_speed_up_to_front(
                        distance_ahead,
                        is_single_front_runner,
                        game_version,
                    ) {
                        return;
                    }
                    // Try to stay ahead
                    self.positioning_strategy = PositioningStrategy::FrontSpeedUp;
                } else {
                    // Try to catch up with pacemaker
                    self.positioning_strategy = PositioningStrategy::FrontOvertake;
                }
            }
            PositioningStrategy::FrontSpeedUp => {
                // Exit if sufficiently ahead
                if !self.can_speed_up_to_front(distance_ahead, is_single_front_runner, game_version)
                {
                    self.positioning_strategy = PositioningStrategy::Normal;
                }
            }
            PositioningStrategy::FrontOvertake => {
                // Exit if already in first place and well ahead
                let distance_threshold = if matches!(self.positioning_style(), Style::Runaway) {
                    27.5
                } else {
                    10.0
                };
                if distance_ahead > distance_threshold {
                    self.positioning_strategy = PositioningStrategy::Normal;
                }
            }
            _ => {
                debug_assert!(false, "BUG: Invalid positioning strategy for front runner")
            }
        }
    }

    /// Update positioning strategy for non-front-runners during the first 10 sections
    /// (5/12 of the race).
    /// The strategy determines whether to pace up or down to keep distance from
    /// the pacemaker.
    ///
    /// `distance_behind` is the distance from the pacemaker in meters
    /// (0 if we are the pacemaker).
    pub fn update_positioning_non_front(&mut self, distance_behind: f64, state: &RaceState) {
        let elapsed_time = state.elapsed_time;
        let course_length = state.course.distance as f64;
        match self.positioning_strategy {
            PositioningStrategy::Normal => {
                if elapsed_time < self.next_positioning_check {
                    return;
                }

                self.next_positioning_check += 2.0;

                // Determine target distance range based on style and course length
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
                    // Catch up to a certain distance (if wiz allows)
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
                    // Pace down to a certain distance
                    // (unless affected by speed-increasing skills)
                    // 1.5 anniv. patch: if in mid-race, the maximum distance is
                    // replaced with the midpoint between min and max before rolling
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
                // Exit when caught up within threshold
                if distance_behind < threshold {
                    self.positioning_strategy = PositioningStrategy::Normal;
                    self.next_positioning_check = elapsed_time + 3.0;
                }
            }
            PositioningStrategy::PaceDown(threshold) => {
                // Exit when fallen back beyond threshold
                // or when affected by speed-increasing skills
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

    /// Enter or exit [`PositioningStrategy::PaceUpEx`] (1.5 anniv. and later).
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
            // Overrides any other strategy
            self.positioning_strategy = PositioningStrategy::PaceUpEx;
        }
        should_enter
    }

    /// Update the runner's position based on effective current speed.
    pub fn update_position(&mut self, delta_time: f64, state: &RaceState) {
        // Runner only moves for the part of the frame after her start delay.
        // The start delay may span several frames, so it's measured from the
        // race start. Clamped to avoid negative values from float drift in
        // elapsed time.
        let move_time = delta_time
            .min(state.elapsed_time - self.start_delay)
            .max(0.0);
        let prev_position = self.position;
        // Use effective_speed which includes current_speed modifiers
        self.position += self.effective_speed() * move_time;
        if self.position >= state.course.distance as f64 {
            // Calculate exact finish time
            let remaining_distance = state.course.distance as f64 - prev_position;
            let time_to_finish = remaining_distance / self.effective_speed();
            self.finish_time = Some(state.elapsed_time - delta_time + time_to_finish);
        }
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
            if prev_phase != self.phase && self.phase >= Phase::Mid && self.last_spurt.is_none() {
                self.last_spurt = Some(calculate_last_spurt(self, state));
            }
        }

        self.try_release_charge_up(state);
    }

    /// Release conserved power (脚色十分) upon entering the last spurt.
    ///
    /// Called after the position update, so the acceleration bonus starts on
    /// the next frame.
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
            let activity =
                charge_up::activity_coefficient(self.spot_struggle.is_some(), self.has_rushed());
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
    /// Power is conserved while keeping position calmly. Speeding up, rushing
    /// or struggling for the lead prevent it, and impose a cooldown upon exit.
    pub fn update_charge_up(&mut self, delta_time: f64, elapsed_time: f64) {
        if self.section >= POSITIONING_END_SECTION {
            return;
        }
        let blocked = self.is_rushed()
            || self
                .spot_struggle
                .as_ref()
                .is_some_and(|s| elapsed_time < s.expires_at)
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
            // Running out of HP during Full-Throttle Sprint drops speed to minimum
            // immediately instead of decelerating (confirmed game bug)
            if self.full_throttle.is_active() {
                self.speed = self.min_speed;
                self.full_throttle = FullThrottle::Ended;
            }
        }
        // Exit dueling state if HP is below 5%
        if self.hp.percent() < 5.0 {
            self.dueling = None;
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
            // Force positioning strategy change immediately
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
            rush_state.end_time = state.elapsed_time; // Ensure end time is current
            state
                .rushed_by_style
                .entry(self.base.style)
                .and_modify(|e| *e -= 1);
            // Force positioning strategy change immediately
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
                // End rush after max duration
                self.end_rush(state);
                return;
            }
            if state.elapsed_time >= rush_state.end_time {
                // 55% chance of snapping out
                if self.rng.borrow_mut().random_bool(0.55) {
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
        // ? Can rushed runners behaving like front-runners enter spot struggle?
        // ? Assume no for now; use adjusted style only.
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
        self.penalize_charge_up(charge_up::STRUGGLE_PENALTY);
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

    /// Check if the runner should check whether to secure the lead (リード確保)
    /// on this frame.
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
    /// If the lead over another runner with a later strategy is less than desirable,
    /// there is a chance for the runner to speed up for a short time at the cost
    /// of HP, followed by a cooldown.
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

    /// Uncheck all overtake targets at the start of an update cycle.
    pub fn reset_overtake_checks(&mut self) {
        if let Some(state) = &mut self.overtake_state {
            for target in state.targets.values_mut() {
                target.is_checked = false;
            }
        }
    }

    /// Check if another runner qualifies as an overtake target and update tracking.
    /// Returns true if the target was added or updated.
    pub fn check_overtake_target(&mut self, target: &Runner, delta_time: f64) -> bool {
        let distance = target.position - self.position;

        // Must be 1-20m ahead
        if !(1.0..=20.0).contains(&distance) {
            return false;
        }

        // Check if this is the closest blocker
        let is_closest_blocker = self.is_front_blocked()
            && self
                .blocked
                .speed_limit
                .is_some_and(|limit| (limit - target.speed).abs() < 0.01);

        // Check if qualifies as overtake target
        let qualifies = is_closest_blocker || {
            let speed_diff = self.speed - target.speed;
            // Can catch up within 15 seconds
            // If speed_diff == 0, a divide by zero would occur
            // At distance 1m, any speed_diff < 1/15 m/s CANNOT catch up within 15s
            // Bigger distances require an even larger speed_diff
            // Thus, using 1/15 as epsilon to avoid divide by zero is safe
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

        // Get or create overtake state
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
    /// Handles cleanup of unchecked targets and persistence timer.
    pub fn finalize_overtake_state(&mut self, delta_time: f64, elapsed_time: f64) {
        if let Some(state) = &mut self.overtake_state {
            // Remove unchecked targets
            state.targets.retain(|_, target| target.is_checked);

            if state.targets.is_empty() {
                if let Some(expires_at) = state.expires_at {
                    if elapsed_time >= expires_at {
                        // Persistence expired, clear state
                        self.overtake_state = None;
                        self.overtake_target_time = 0.0;
                    }
                } else {
                    // Just lost all targets, start persistence timer
                    state.expires_at = Some(elapsed_time + 1.5);
                }
            } else {
                // Have targets, clear expiration and increment time
                state.expires_at = None;
                self.overtake_target_time += delta_time;
            }
        }
    }

    /// Check if the runner is in overtake mode (has targets or in persistence period).
    pub fn is_in_overtake_mode(&self) -> bool {
        self.overtake_state.is_some()
    }

    /// Calculate lane target when in normal mode (not overtaking).
    /// Rules are checked in priority order; first match wins.
    pub fn calculate_normal_mode_lane_target(&mut self, course: &Course, all_runners: &[Runner]) {
        // Out of HP - stay in current lane
        if self.hp.is_empty() {
            self.lane.target = self.lane.current;
            return;
        }

        // Position keeping mode is pace-down
        // Set target to 0.18 of course width (18% from inside)
        if matches!(self.positioning_strategy, PositioningStrategy::PaceDown(_)) {
            self.lane.target = (0.18 * course.width()).clamp(0.0, course.width());
            return;
        }

        // On final straight AND extra move lane is on the outside
        if self.is_on_final_straight(course)
            && let Some(extra) = self.extra_move_lane
            && extra > self.lane.current
            && self.has_lane_space_to(extra, all_runners)
        {
            self.lane.target = extra.clamp(0.0, course.width());
            return;
        }

        // In early-race or mid-race - move inward by 0.05 course width
        if matches!(self.phase, Phase::Early | Phase::Mid) {
            let target = self.lane.current - 0.05 * course.width();
            if target >= 0.0 && self.has_lane_space_to(target, all_runners) {
                self.lane.target = target;
                return;
            }
        }

        // In mid-race AND less than 1.75 horse lane from inside runner
        if self.phase == Phase::Mid
            && let Some((inside_lane, distance_in_lanes)) =
                self.find_inside_runner(all_runners, course)
            && distance_in_lanes < 1.75
        {
            // Move to 2.0 horse lanes away from inside runner
            let target = inside_lane + 2.0 * course.lane_width();
            if self.has_lane_space_to(target, all_runners) {
                self.lane.target = target.clamp(0.0, course.width());
            }
        }

        // Default - keep straight; target lane remains as-is
    }

    /// Calculate target lane when in overtake mode.
    /// Should be called every frame after overtake state is finalized.
    pub fn calculate_overtake_lane_target(&mut self, course: &Course, all_runners: &[Runner]) {
        // Early exit if not in overtake mode or side-blocked
        if !self.is_in_overtake_mode() || self.blocked.side {
            return;
        }

        // Update extra move lane
        self.update_extra_move_lane(course, all_runners);

        // Build candidate list from overtake targets and extra candidate
        let mut candidates = Vec::with_capacity(32);
        if let Some(state) = &self.overtake_state {
            for &gate_roll in state.targets.keys() {
                self.add_target_candidates(&mut candidates, gate_roll, course, all_runners);
            }
        }
        self.add_extra_candidate(&mut candidates, course, all_runners);

        // Filter by lane space
        candidates.retain(|c| self.has_lane_space_to(c.lane, all_runners));

        // Score candidates and select best (lowest score)
        // Keep current lane if no candidates
        self.score_candidates(&mut candidates);
        if let Some(best) = candidates
            .iter()
            .min_by(|a, b| a.score.partial_cmp(&b.score).unwrap())
        {
            let mut target_lane = best.lane;

            // Check extra_move_lane override
            if let Some(extra) = self.extra_move_lane
                && target_lane < extra
                && self.has_lane_space_to(extra, all_runners)
            {
                target_lane = extra;
            }

            self.lane.target = target_lane.clamp(0.0, course.width());
        }
    }

    /// Find the innermost and outermost uma in a target's crowd.
    /// Crowd members are within 3m distance and less than 2 horse lanes away.
    fn find_crowd_boundaries(
        &self,
        target: &Runner,
        course: &Course,
        all_runners: &[Runner],
    ) -> (f64, f64) {
        let mut inner_lane = target.lane.current;
        let mut outer_lane = target.lane.current;

        for other in all_runners {
            if other.gate_roll == target.gate_roll || other.gate_roll == self.gate_roll {
                continue;
            }

            let distance_gap = (other.position - target.position).abs();
            let lane_gap = (other.lane.current - target.lane.current).abs();

            if course.is_near_by(distance_gap, lane_gap, 3.0, 2.0) {
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
        course: &Course,
        all_runners: &[Runner],
    ) {
        let Some(target) = all_runners.iter().find(|r| r.gate_roll == target_gate) else {
            return;
        };

        let (inner, outer) = self.find_crowd_boundaries(target, course, all_runners);
        let reference_distance = target.position - self.position;

        // Try lane positions at ±1 horse lane from inner and outer boundaries
        for &offset in &[-1.0, 1.0] {
            let offset_meters = offset * course.lane_width();

            for &base_lane in &[inner, outer] {
                let candidate_lane = base_lane + offset_meters;

                if self.is_lane_candidate_valid(
                    candidate_lane,
                    reference_distance,
                    course,
                    all_runners,
                ) {
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
        course: &Course,
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

        let candidate_lane = if self.phase <= Phase::Mid && self.lane.current >= course.lane_width()
        {
            // Check 1 horse lane inward
            self.lane.current - course.lane_width()
        } else {
            // Check going straight
            self.lane.current
        };

        let check_distance = furthest_target_distance + 3.0;

        if self.is_lane_candidate_valid(candidate_lane, check_distance, course, all_runners) {
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
        course: &Course,
        all_runners: &[Runner],
    ) -> bool {
        for other in all_runners {
            if other.gate_roll == self.gate_roll {
                continue;
            }

            // Check distance window: OwnerDistance <= Distance <= ReferenceDistance + 0.5
            // Check lane window: CandidateLane ± 0.8 HorseLane
            if course.is_near_by(
                other.position - self.position,
                (other.lane.current - candidate_lane).abs(),
                reference_distance + 0.5,
                0.8,
            ) {
                return false; // Someone is blocking this candidate
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

            // If runner is very close and in the lane change path, consider it blocking
            if distance < 1.05 && lane_range.contains(&lane) {
                return false;
            }
        }

        true
    }

    /// Update extra_move_lane based on furthest visible runner ahead.
    /// Only active during Early and Mid phases.
    fn update_extra_move_lane(&mut self, course: &Course, all_runners: &[Runner]) {
        if self.phase > Phase::Mid {
            self.extra_move_lane = None;
            return;
        }

        // Find furthest visible runner ahead
        let furthest_visible = all_runners
            .iter()
            .filter(|r| r.position > self.position && self.is_visible(r, course))
            .max_by(|a, b| a.position.partial_cmp(&b.position).unwrap());

        if let Some(furthest) = furthest_visible {
            self.extra_move_lane = Some(furthest.lane.current);
        }
    }

    /// Check if runner is currently on the final straight.
    fn is_on_final_straight(&self, course: &Course) -> bool {
        if course.straights.is_empty() {
            return false;
        }
        self.current_straight()
            .is_some_and(|idx| idx == course.straights.len() - 1)
    }

    /// Find the closest uma on the inside (lower lane value).
    /// Returns (inside_uma_lane, distance_in_horse_lanes) if found.
    fn find_inside_runner(&self, all_runners: &[Runner], course: &Course) -> Option<(f64, f64)> {
        let mut closest_inside: Option<(f64, f64)> = None;

        for other in all_runners {
            // Skip self
            if other.gate_roll == self.gate_roll {
                continue;
            }

            // Only consider runners on the inside (lower lane value)
            if other.lane.current >= self.lane.current {
                continue;
            }

            // Check if within reasonable distance
            let position_distance = (self.position - other.position).abs();
            if position_distance > 5.0 {
                continue;
            }

            // Calculate lane distance in horse lanes
            let lane_diff_in_horse_lanes =
                (self.lane.current - other.lane.current) / course.lane_width();

            // Only consider if within 3 horse lanes
            if lane_diff_in_horse_lanes > 3.0 {
                continue;
            }

            // Track closest (smallest lane difference)
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

    /// Check for straight entry. Skills triggered by the entry are checked on the
    /// next skill check (see [`Self::check_last_straight_triggers`]).
    pub fn check_straight_start(&mut self, straights: &[Straight]) {
        if let Some(straight_idx) = self.straight_manager.check_entry(self.position, straights) {
            self.entered_straight = Some(straight_idx);
        }
    }

    /// Check the skills triggered by entering the last straight since the last check.
    pub fn check_last_straight_triggers(&mut self, state: &RaceState) {
        let Some(straight_idx) = self.entered_straight.take() else {
            return;
        };
        if straight_idx + 1 != state.course.straights.len() {
            return; // Not the last straight
        }
        for logic_idx in self.trigger_scheduler.last_straight_triggers.clone() {
            self.try_activate_skill(logic_idx, state);
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
    pub fn update_slope(&mut self, slopes: &[Slope], delta_time: f64, rng: &mut impl Rng) {
        // Exit first, so that a slope right after another one is entered in the
        // same frame
        if self
            .slope_manager
            .check_exit(self.position, slopes)
            .is_some()
        {
            // Exiting slope - stop any slope-based acceleration/deceleration
            self.stop_slope_acceleration();
        }

        if let Some((slope_idx, _slope_type)) =
            self.slope_manager.check_entry(self.position, slopes)
        {
            // Entering slope - apply uphill deceleration if applicable
            if slopes[slope_idx].is_uphill() {
                self.start_uphill_deceleration(slopes[slope_idx].slope_percent());
            } else {
                // Downhill - reset downhill timer
                self.downhill_time = 0.0;
            }
        } else if let Some(idx) = self.current_slope() {
            // On slope - check for downhill acceleration
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
                        // 20% chance to stop each second
                        if rng.random_bool(0.2) {
                            self.stop_slope_acceleration();
                        }
                    } else {
                        // Wisdom * 0.04% chance to start each second
                        let chance = self.adjusted.wiz as f64 * 0.0004;
                        if rng.random_bool(chance) {
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
        // Visibility starts at 20m, modified by skill bonuses
        let visible_distance = 20.0 + self.skill_modifiers.visible_distance;
        distance <= visible_distance
            && (self.lane.current - other.lane.current).abs()
                <= ((distance / visible_distance) * 11.5 + 2.0) * course.lane_width() / 2.0
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
        self.nearby.ahead_same_lane_count = 0;
        self.nearby.behind_same_lane_count = 0;
        self.nearby.behind_set1_count = 0;
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

    /// Check the preconditions whose window contains the current position.
    ///
    /// This must be called at the start of each frame, before any other trigger,
    /// so that a skill can activate in the same frame its precondition is satisfied.
    pub fn check_precondition_windows(&mut self, state: &RaceState) {
        let inside = self
            .trigger_scheduler
            .precondition_windows
            .advance(self.position, EPSILON);

        let mut any_unlocked = false;
        for logic_idx in inside {
            self.try_activate_skill(logic_idx, state);
            any_unlocked |= self.trigger_scheduler.skill_logics[logic_idx].has_activated;
        }

        // Stop checking every branch of the preconditions that were just satisfied
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
    /// All skills are checked at once, at the start of each frame, against the
    /// runner's state at that point. Nothing is updated in between, so every
    /// check sees the same values.
    pub fn check_skills(&mut self, state: &mut RaceState) {
        self.reset_activation_flags();
        // Preconditions go first, so that a skill can activate in the same frame
        // its precondition is satisfied
        self.check_precondition_windows(state);
        self.check_distance_triggers(state);
        self.check_window_triggers(state);
        self.check_last_straight_triggers(state);
        // Depend on the activations above
        self.check_activation_dependent_triggers(state);
        self.check_buff_dependent_triggers(state);
        self.extract_buff_activations(state);
    }

    /// Reset activation flags before checking skills.
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
    /// This should be called after all other triggers have been checked.
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
    /// This should be called after all other triggers (and any-skill
    /// activation-dependent triggers) have been checked.
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

    /// Check position triggers (random points) that have been reached.
    pub fn check_distance_triggers(&mut self, state: &RaceState) {
        while let Some(trigger) = self.trigger_scheduler.distance_triggers.peek() {
            if self.position < trigger.threshold_meters {
                break; // Threshold not reached yet, this and all subsequent triggers require further distance
            }

            let trigger = self.trigger_scheduler.distance_triggers.pop().unwrap();
            let trigger_index = trigger.logic_idx;
            self.try_activate_skill(trigger_index, state);
        }
    }

    /// Check window triggers (position, time, HP) containing the current values.
    pub fn check_window_triggers(&mut self, state: &RaceState) {
        let scheduler = &mut self.trigger_scheduler;
        let mut inside = scheduler.position_windows.advance(self.position, EPSILON);
        inside.extend(scheduler.time_windows.advance(state.elapsed_time, EPSILON));

        // HP can recover, so HP windows are never dropped
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

        // Try activations in the order the logics were built
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
        // Clone the skill data we need to avoid borrow checker issues
        let skill_id = self.available_skills[skill_index].id.clone();
        let alternative =
            self.available_skills[skill_index].alternatives[alternative_index].clone();

        // Calculate scaled duration
        let base_duration = alternative.base_duration as f64 / 10000.0; // Convert from 10000ths of a second
        let scaled_duration = base_duration * self.skill_modifiers.duration_scaling;

        // Wiz Limit Break bonus, fixed at activation time (5th Anniv. onward)
        let wiz_bonus = if state.config.game_version >= GameVersion::FifthAnniversary
            && self.available_skills[skill_index].is_wiz_limit_break_eligible()
        {
            wiz_limit_break_phase_coefficient(self.adjusted.style, self.phase)
                * wiz_limit_break_multiplier(self.adjusted.wiz)
        } else {
            0.0
        };

        // Apply each effect in the alternative
        for effect in &alternative.effects {
            match effect.target {
                SkillTarget::Oneself => {
                    self.apply_skill_effect(effect, scaled_duration, wiz_bonus, state)
                }
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
        let section_length = state.course.distance as f64 / 24.0;
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
        // Get skill logic and conditions
        let logic = &self.trigger_scheduler.skill_logics[trigger_index];

        // Check if precondition is satisfied; skip if not
        if let Some(id) = logic.precondition
            && !self.trigger_scheduler.precondition_satisfied[id]
        {
            return;
        }

        if logic.has_activated {
            return;
        }
        let skill_index = logic.skill_index;
        let alternative_index = logic.alternative_index;
        let action = logic.action;
        let should_activate =
            evaluate_comparison_list(&logic.guards, &EvaluatorFull, Some(logic), state, self);

        if should_activate {
            match action {
                LogicAction::Activate => self.activate_skill(skill_index, alternative_index, state),
                LogicAction::Unlock(id) => self.trigger_scheduler.precondition_satisfied[id] = true,
            }
            // Never checked again
            self.trigger_scheduler.skill_logics[trigger_index].has_activated = true;
        }
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
        scaled_duration: f64,
        wiz_bonus: f64,
        state: &RaceState,
    ) {
        let effect_type = &effect.skill_type;

        // Stat boosts and start delay modifications should have been applied out of gate.
        // Therefore, these are gracefully ignored here.

        // Timed effects (tracked in active_timed_effects)
        if is_timed_effect(effect_type) {
            let expires_at = state.elapsed_time + scaled_duration;
            // Apply Wiz Limit Break bonus to target speed and current speed effects
            let bonus = match effect_type {
                SkillType::TargetSpeed | SkillType::CurrentSpeedWithNaturalDeceleration => {
                    wiz_bonus
                }
                _ => 0.0,
            };
            self.active_timed_effects.push(ActiveTimedEffect {
                effect: effect.clone(),
                value: effect.modifier as f64 / 10000.0 * (1.0 + bonus),
                expires_at,
            });
        }
        // Recovery (HP restoration)
        else if matches!(effect_type, SkillType::Recovery) {
            self.hp.recover(effect.modifier as f64);

            // Increment heal skill count
            self.trigger_scheduler.activation_counts.increment_heal();

            // Recalculate last spurt if in Mid phase or later (1st Anniv. onward)
            if state.config.game_version >= GameVersion::FirstAnniversary
                && self.phase >= Phase::Mid
            {
                self.last_spurt = Some(calculate_last_spurt(self, state));
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
                self.activate_skill(golden_skill_idx, 0, state);
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
        // "Sympathy" skill activation
        else if matches!(effect_type, SkillType::ActivateSympathy) {
            // Force activation of skill with ID "201631" if available
            if let Some((skill_index, _)) = self
                .available_skills
                .iter()
                .enumerate()
                .find(|(_, skill)| skill.id == "201631")
            {
                self.activate_skill(skill_index, 0, state);
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

    /// Update skill durations and remove expired effects.
    pub fn update_skill_durations(&mut self, elapsed_time: f64) {
        let before_count = self.active_timed_effects.len();

        // For expiring CurrentSpeedWithNaturalDeceleration effects,
        // compensate speed then allow natural deceleration to take effect.
        self.active_timed_effects.iter().for_each(|effect| {
            if effect.expires_at > elapsed_time
                && matches!(
                    effect.effect.skill_type,
                    SkillType::CurrentSpeedWithNaturalDeceleration
                )
            {
                self.speed += effect.value;
            }
        });

        // Remove expired effects
        self.active_timed_effects
            .retain(|effect| effect.expires_at > elapsed_time);

        // Recalculate modifiers if any effects expired
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
                    mods.acceleration += timed.value;
                }
                AccelerationFullThrottle => {
                    mods.full_throttle_acceleration += timed.value;
                }
                TargetSpeed => {
                    mods.target_speed += timed.value;
                }
                CurrentSpeed | CurrentSpeedWithNaturalDeceleration => {
                    mods.current_speed += timed.value;
                }
                LaneMovementSpeed => {
                    mods.lane_speed += timed.value;
                }
                Fov => {
                    mods.visible_distance += timed.value;
                }
                LaneChange => {
                    mods.target_lane = Some(timed.value);
                }
                ExtendEvolvedDuration => {
                    // Duration scaling is multiplicative
                    mods.duration_scaling *= timed.value;
                }
                _ => {}
            }
        }

        self.skill_modifiers = mods;
    }
}
