//! Condition Evaluator - Evaluates guard conditions against race state
//!
//! This module evaluates guard conditions, i.e., conditions that determine whether
//! a triggered skill can activate or not.
//!
//! Trigger conditions (random gates, monotonic thresholds, onetime events) are handled
//! by the trigger system and should NEVER reach this evaluator.
//!
//! # Architecture
//!
//! Conditions are parsed into tree structures (`Condition` -> `CompList` -> `Comp`)
//! and evaluated recursively:
//! - **Binary operators**: AND (&) and OR (@) with short-circuit evaluation
//! - **Leaf comparisons**: Variable operator value (e.g., `hp_per <= 50`)
//!
//! # Variable Access
//!
//! The `get_variable_value()` function maps ~150+ condition variables to their
//! current runtime values from `RaceState`. Variables are grouped by category:
//! - **Race-level**: elapsed_time, course properties, race parameters
//! - **Runner-level**: position, hp, phase, speed, skill activation state
//! - **Multi-uma**: Order, near counts, blocking (STUBBED for single-uma mode)
//!
//! # Float Handling
//!
//! All comparisons use `i32` values. Float variables (hp, position, percentages)
//! are converted using `.floor()` for consistent rounding behavior.
//!
//! # Multi-Uma Limitations
//!
//! For single-uma simulation, multi-uma variables return sensible stub values:
//! - `order` = 1 (always first place)
//! - `order_rate` = 0 (0% = top position)
//! - `near_count` = 0 (no other umas)
//! - `blocked_front` = 0 (not blocked)
//!
//! These are clearly marked with `// STUB:` comments.

use crate::condition::parser::*;
use crate::condition::variables::*;
use crate::course::Course;
use crate::race::parameters::RaceParameters;
use crate::race::runner::RunnerBuilder;
use crate::race::runner::{Runner, StartType};
use crate::race::solver::RaceState;
use crate::uma::*;
use crate::units::*;

/// Evaluate a guard condition against current race state using the provided evaluator.
///
/// **IMPORTANT**: This should ONLY be called with guard conditions after trigger-guard
/// separation. Trigger variables (random gates, monotonic triggers, onetime events) will
/// cause a panic if they reach this evaluator.
///
/// Returns `true` if the guard condition is satisfied, `false` otherwise.
///
/// # Usage
///
/// This function is called by the trigger system when a trigger fires:
///
/// ```ignore
/// // When HP trigger fires at 50%:
/// let skill_logic = &skill_activation_logics[trigger.skill_logic_idx];
///
/// if let Some(guard) = &skill_logic.guard_condition {
///     // Evaluate ONLY the guard (no trigger variables)
///     if evaluate_condition(guard, &race_state) {
///         activate_skill(&race_state, &skill);
///     }
/// }
/// ```
///
/// # Example Guard Conditions
///
/// Valid (guard variables only):
/// - `"ground_type==2 & motivation>=4"` ✓
/// - `"running_style==1 & season==1"` ✓
/// - `"hp_per>=20"` ✓ (Note: hp_per is a guard when checked AT trigger time)
///
/// Invalid (contains trigger variables):
/// - `"phase_random==1 & hp_per>=20"` ✗ (will panic)
/// - `"is_last_straight_onetime==1"` ✗ (will panic)
///
/// # Panics
///
/// Panics if the condition contains trigger variables that should have been
/// separated during the splitting phase.
pub fn evaluate_condition<E>(
    condition: &Condition,
    evaluator: &E,
    state: &E::State,
    context: &E::Context,
) -> bool
where
    E: CompEvaluator,
{
    evaluate_comp_list(&condition.comp_list, |comp| {
        evaluator.evaluate(comp, state, context)
    })
}

/// Recursively evaluate condition tree (handles AND/OR logic)
fn evaluate_comp_list<F>(comp_list: &CompList, eval_fn: F) -> bool
where
    F: Fn(&Comp) -> bool + Copy,
{
    match comp_list {
        CompList::Single(comp) => eval_fn(comp),
        CompList::Binary { left, op, right } => match op {
            BinOp::And => evaluate_comp_list(left, eval_fn) && evaluate_comp_list(right, eval_fn),
            BinOp::Or => evaluate_comp_list(left, eval_fn) || evaluate_comp_list(right, eval_fn),
        },
    }
}

/// Trait for evaluating a single comparison.
///
/// # Usage
///
/// Implement this trait for different evaluation contexts.
/// `State` and `Context` associated types define the data sources used.
/// `State` should represent the overall race state, while `Context` represents the
/// runner data to use.
///
/// The main use case is to get values from a provided race state and runner, then
/// perform the comparison directly.
/// You could also create your own race state with some of the values replaced by
/// estimations or other expectations as needed. For instance, you could simulate
/// best-case or worst-case scenarios by overriding certain variable values rather
/// than having to craft a specific set of runner data to match those scenarios.
///
/// # Examples
///
/// - `EvaluatorFull`: Uses actual race state and runner data as defined by `RaceState`
///   and `Runner`.
/// - `EvaluatorOutOfGate`: Uses only data available before a race starts as defined by
///   `RaceState` and `RunnerBuilder`.
pub trait CompEvaluator {
    type State;
    type Context;

    /// Evaluate a single comparison.
    fn evaluate(&self, comp: &Comp, state: &Self::State, context: &Self::Context) -> bool;
}

/// Reference evaluator using actual race state and runner data.
pub struct EvaluatorFull;
impl CompEvaluator for EvaluatorFull {
    type State = RaceState;
    type Context = Runner;

    /// Evaluate a single comparison using actual race state and runner data.
    fn evaluate(&self, comp: &Comp, state: &Self::State, context: &Self::Context) -> bool {
        let runner = context;
        // Special cases: skill ID checks require different logic
        match &comp.var {
            VarType::Activation(ActivationVar::IsUsedSkillId) => {
                // Check if skill with ID == comp.value has been activated
                let skill_id_str = comp.value.to_string();
                let skill_exists = runner.activated_skill_ids.contains(&skill_id_str);

                match comp.op {
                    CompOp::Equal => skill_exists,
                    CompOp::NotEqual => !skill_exists,
                    _ => false, // Other operators don't make sense for skill ID checks
                }
            }
            VarType::Activation(ActivationVar::IsExistSkillId) => {
                // Check if at least one runner has skill with ID == comp.value
                state.existing_skills.contains(&comp.value.to_string())
            }
            _ => {
                // Normal case: get variable value and compare
                let current_value = get_variable_value(&comp.var, state, runner);
                let threshold = comp.value;

                match comp.op {
                    CompOp::Equal => current_value == threshold,
                    CompOp::NotEqual => current_value != threshold,
                    CompOp::Greater => current_value > threshold,
                    CompOp::GreaterEqual => current_value >= threshold,
                    CompOp::Less => current_value < threshold,
                    CompOp::LessEqual => current_value <= threshold,
                }
            }
        }
    }
}

/// Evaluator for out-of-gate context (before race starts).
pub struct EvaluatorOutOfGate;
impl CompEvaluator for EvaluatorOutOfGate {
    type State = RaceState;
    type Context = RunnerBuilder;

    /// Evaluate a single comparison in out-of-gate context, using only data available before a race starts.
    fn evaluate(&self, comp: &Comp, state: &Self::State, context: &Self::Context) -> bool {
        let builder = context;
        match &comp.var {
            VarType::Activation(ActivationVar::IsExistSkillId) => {
                // Special case: skill ID checks require different logic
                // Check if at least one runner has skill with ID == comp.value
                state.existing_skills.contains(&comp.value.to_string())
            }
            _ => {
                // Normal case: get variable value and compare
                let current_value = get_variable_value_out_of_gate(&comp.var, state, builder);
                let threshold = comp.value;

                match comp.op {
                    CompOp::Equal => current_value == threshold,
                    CompOp::NotEqual => current_value != threshold,
                    CompOp::Greater => current_value > threshold,
                    CompOp::GreaterEqual => current_value >= threshold,
                    CompOp::Less => current_value < threshold,
                    CompOp::LessEqual => current_value <= threshold,
                }
            }
        }
    }
}

/// Get current integer value for any variable type
fn get_variable_value(var: &VarType, state: &RaceState, runner: &Runner) -> i32 {
    match var {
        VarType::BaseStat(stat_var) => get_base_stat(stat_var, &runner.base),
        VarType::Order(pos_var) => get_rank_value(pos_var, state),
        VarType::Distance(dist_var) => get_distance_value(dist_var, state, runner),
        VarType::Phase(phase_var) => get_phase_value(phase_var, &state.course, runner),
        VarType::Course(course_var) => get_course_value(course_var, &state.course),
        VarType::Spatial(location_var) => get_location_value(location_var, &state.course, runner),
        VarType::Activation(act_var) => get_activation_value(act_var, runner),
        VarType::RaceParam(param_var) => get_race_param_value(param_var, &state.params),
        VarType::RunningStyle(style_var) => {
            get_running_style_value(style_var, state, runner.adjusted.style)
        }
        VarType::Status(status_var) => get_status_value(status_var, runner),
        VarType::Random(trigger_var) => get_random_value(trigger_var, state),
        VarType::Blocking(blocking_var) => get_blocking_value(blocking_var, state),
        VarType::Temptation(tempt_var) => get_temptation_value(tempt_var, state, runner),
        VarType::AccumulateTime => state.elapsed_time.floor() as i32,
        VarType::Always => 1,     // Always returns 1 (true)
        VarType::Unknown(_) => 0, // Default to 0
    }
}

/// Get current integer value for any variable type in out-of-gate context
fn get_variable_value_out_of_gate(
    var: &VarType,
    state: &RaceState,
    builder: &RunnerBuilder,
) -> i32 {
    match var {
        VarType::BaseStat(stat_var) => get_base_stat(stat_var, &builder.base),
        VarType::Course(course_var) => get_course_value(course_var, &state.course),
        VarType::RaceParam(param_var) => get_race_param_value(param_var, &state.params),
        VarType::RunningStyle(style_var) => {
            get_running_style_value(style_var, state, builder.base.style)
        }
        VarType::Status(status_var) => get_basic_status_value(status_var, builder),
        VarType::Always => 1, // Always returns 1 (true)
        _ => 0,               // Not applicable in out-of-gate context
    }
}

/// Get base stat value
fn get_base_stat(stat: &StatKind, uma: &Uma) -> i32 {
    match stat {
        StatKind::Speed => uma.speed as i32,
        StatKind::Stamina => uma.stamina as i32,
        StatKind::Power => uma.power as i32,
        StatKind::Guts => uma.guts as i32,
        StatKind::Wiz => uma.wiz as i32,
    }
}

/// Get distance-related values
fn get_distance_value(var: &DistanceVar, state: &RaceState, runner: &Runner) -> i32 {
    match var {
        // Multi-uma stubs - distance differences
        DistanceVar::BashinDiffBehind => 999, // STUB: No uma behind (large value)
        DistanceVar::BashinDiffInfront => 999, // STUB: No uma ahead (large value)
        DistanceVar::DistanceDiffRate => 0,   // STUB: 0% = first place
        DistanceVar::DistanceDiffTop => 0,    // STUB: 0m behind first (we are first)
        DistanceVar::DistanceDiffTopFloat => 0, // STUB: 0 decimeters behind first

        // Remaining distance for any player character
        DistanceVar::RemainDistanceViewerId => {
            // STUB: Single-uma mode, same as self
            remain_distance(&runner.position, state.course.distance)
        }
    }
}

/// Get phase-related values
fn get_phase_value(var: &PhaseVar, course: &Course, runner: &Runner) -> i32 {
    match var {
        // Current phase: 0=Early, 1=Mid, 2=Late, 3=Last
        PhaseVar::Phase => runner.phase as i32,

        // Phase subdivisions
        PhaseVar::PhaseFirstquarter => (runner.position < course.quarterway()) as i32,
        PhaseVar::PhaseFirsthalf => (runner.position < course.halfway()) as i32,
        PhaseVar::PhaseLaterhalf => (runner.position >= course.halfway()) as i32,

        // Random triggers - handled by trigger system, not evaluated here
        PhaseVar::PhaseCornerRandom
        | PhaseVar::PhaseFirsthalfRandom
        | PhaseVar::PhaseFirstHalfStraightRandom
        | PhaseVar::PhaseFirstquarterRandom
        | PhaseVar::PhaseLaterhalfRandom
        | PhaseVar::PhaseLatterHalfStraightRandom
        | PhaseVar::PhaseRandom
        | PhaseVar::PhaseStraightRandom => {
            panic!(
                "BUG: Random phase trigger {:?} should not be evaluated - it's a trigger variable!",
                var
            )
        }
    }
}

/// Get course-related values
fn get_course_value(var: &CourseVar, course: &Course) -> i32 {
    match var {
        // Total corners on course
        CourseVar::CornerCount => course.corners.len() as i32,

        // Direct course distance
        CourseVar::CourseDistance => course.distance,

        // Distance type: 1=short, 2=mile, 3=medium, 4=long
        CourseVar::DistanceType => course.distance_type as i32,

        // Ground type: 1=turf, 2=dirt
        CourseVar::GroundType => course.surface as i32,

        // Is basis distance (multiple of 400m)
        CourseVar::IsBasisDistance => course.is_basis_distance() as i32,

        // Is exchange race (dirt grade): races at Ooi, Kawasaki, Funabashi, or Morioka
        CourseVar::IsDirtgrade => course.is_exchange_race() as i32,

        // Rotation: 1=clockwise, 2=counterclockwise, 4=straight
        CourseVar::Rotation => course.turn as i32,

        // Track ID
        CourseVar::TrackId => course.race_track_id as i32,
    }
}

/// Get location-related values
fn get_location_value(var: &SpatialVar, course: &Course, runner: &Runner) -> i32 {
    match var {
        // Which corner: 0=none, 1-4=corner number
        SpatialVar::Corner => {
            if let Some(idx) = runner.current_corner() {
                (idx + 1) as i32 // 1-based index
            } else {
                0
            }
        }

        // Is on final corner or beyond
        SpatialVar::IsFinalcorner => {
            if course.corners.is_empty() {
                0 // False if no corners
            } else {
                // "Beyond" also applies for a straight after final corner
                let last_corner_idx = course.corners.len() - 1;
                let last_corner = &course.corners[last_corner_idx];
                if runner.position >= last_corner.start {
                    1
                } else {
                    0
                }
            }
        }

        // Is on second half of final corner
        SpatialVar::IsFinalcornerLaterhalf => {
            if course.corners.is_empty() {
                0
            } else {
                let last_corner_idx = course.corners.len() - 1;
                let last_corner = &course.corners[last_corner_idx];
                let corner_midpoint = last_corner.start + last_corner.length / 2.0;
                if runner.position >= corner_midpoint {
                    1
                } else {
                    0
                }
            }
        }

        // Is on last straight
        SpatialVar::IsLastStraight => {
            if course.corners.is_empty() {
                1 // First (and only) straight is last straight
            } else {
                // Last straight starts after final corner ends
                let last_corner = course.corners.last().unwrap();
                let last_straight_start = last_corner.start + last_corner.length;
                if runner.position >= last_straight_start {
                    1
                } else {
                    0
                }
            }
        }

        // One-time trigger - handled by trigger system
        SpatialVar::IsLastStraightOnetime => {
            panic!(
                "BUG: is_last_straight_onetime should not be evaluated - it's a trigger variable!"
            )
        }

        // Current lane: 0=inner, 1=middle, 2=outer, 3=outside
        SpatialVar::LaneType => runner.lane_type(course) as i32,

        // Slope: 0=flat, 1=uphill, 2=downhill
        SpatialVar::Slope => {
            if let Some(idx) = runner.current_slope() {
                course.slopes[idx].slope_type() as i32
            } else {
                0
            }
        }

        // Straight front type: 1=facing audience, 2=opposite from audience
        SpatialVar::StraightFrontType => {
            if let Some(idx) = runner.current_straight() {
                course.straights[idx].front_type as i32
            } else {
                0 // Not in a straight
            }
        }

        // Distance covered as percentage (0-100)
        SpatialVar::DistanceRate => distance_rate(&runner.position, course.distance),

        // Distance in furlongs
        SpatialVar::Furlong => distance_to_furlongs(runner.position),

        // Remaining distance in meters
        SpatialVar::RemainDistance => remain_distance(&runner.position, course.distance),

        // Random triggers - handled by trigger system
        SpatialVar::AllCornerRandom
        | SpatialVar::CornerRandom
        | SpatialVar::DistanceRateAfterRandom
        | SpatialVar::DownSlopeRandom
        | SpatialVar::IsFinalcornerRandom
        | SpatialVar::LastStraightRandom
        | SpatialVar::StraightRandom
        | SpatialVar::UpSlopeRandom
        | SpatialVar::UpSlopeRandomLaterHalf => {
            panic!(
                "BUG: {:?} should not be evaluated - it's a trigger variable!",
                var
            )
        }
    }
}

/// Get race parameter values
fn get_race_param_value(var: &RaceParamVar, params: &RaceParameters) -> i32 {
    match var {
        // Track condition: 1=firm, 2=good, 3=soft, 4=heavy
        RaceParamVar::GroundCondition => params.condition as i32,

        // Race grade: refer to Grade enum for values
        RaceParamVar::Grade => params.grade as i32,

        // Popularity rank (multi-uma)
        RaceParamVar::Popularity => 1, // STUB: Always most popular in single-uma mode

        // Season: 1=spring, 2=summer, 3=autumn, 4=winter, 5=sakura
        //
        // n.b. Cherry blossom season is semantically part of spring;
        // conditions that check for spring should also match sakura.
        // When applicable, this is handled directly in the condition
        // definitions (i.e., "season==1@season==5").
        RaceParamVar::Season => params.season as i32,

        // Time of day: 0=none/any, 1=morning, 2=noon, 3=evening, 4=night
        RaceParamVar::Time => params.time as i32,

        // Number of visible horses (multi-uma)
        RaceParamVar::Visiblehorse => 0, // STUB: No other horses in single-uma mode

        // Weather: 1=sunny, 2=cloudy, 3=rainy, 4=snowy
        RaceParamVar::Weather => params.weather as i32,

        // Multi-uma stubs
        RaceParamVar::IsExistCharaId => 0, // STUB: No other characters
        RaceParamVar::SameSkillHorseCount => 1, // STUB: Only self has this skill
    }
}

/// Get activation counter values
fn get_activation_value(var: &ActivationVar, runner: &Runner) -> i32 {
    match var {
        ActivationVar::ActivateCountAll => runner.trigger_scheduler.activation_counts.all as i32,
        ActivationVar::ActivateCountEndAfter => {
            runner.trigger_scheduler.activation_counts.end_after as i32
        }
        ActivationVar::ActivateCountHeal => runner.trigger_scheduler.activation_counts.heal as i32,
        ActivationVar::ActivateCountLaterHalf => {
            runner.trigger_scheduler.activation_counts.later_half as i32
        }
        ActivationVar::ActivateCountMiddle => {
            runner.trigger_scheduler.activation_counts.middle as i32
        }
        ActivationVar::ActivateCountStart => {
            runner.trigger_scheduler.activation_counts.start as i32
        }

        // Multi-uma stub
        ActivationVar::ActivateCountAllTeam => 0, // STUB: No team in single-uma mode

        // Continuous checks
        ActivationVar::IsActivateAnySkill => 0, // STUB: Needs per-frame tracking
        ActivationVar::IsActivateHealSkill => 0, // STUB: Needs per-frame tracking
        ActivationVar::IsActivateOtherSkillDetail => 0, // STUB: Needs tracking

        ActivationVar::IsOtherCharacterActivateAdvantageSkill => 0, // STUB

        // These cases are handled specially because they use the comparison
        // value as parameter instead of comparing against it.
        ActivationVar::IsExistSkillId => {
            panic!("is_exist_skill_id: should be handled in evaluate_comparison()")
        }
        ActivationVar::IsUsedSkillId => {
            panic!("is_used_skill_id: should be handled in evaluate_comparison()")
        }

        // Multi-uma showdown stubs
        ActivationVar::CompeteFightCount => {
            unimplemented!("compete_fight_count: showdown behavior not yet implemented")
        }
    }
}

/// Get running style values
fn get_running_style_value(var: &RunningStyleVar, state: &RaceState, my_style: Style) -> i32 {
    match var {
        // TODO: generate simulation values for one-runner simulations
        RunningStyleVar::RunningStyleCountNigeOtherself => {
            get_style_count_front_otherself(state, my_style)
        }
        RunningStyleVar::RunningStyleCountOikomiOtherself => {
            get_style_count_otherself(state, Style::End, my_style)
        }
        RunningStyleVar::RunningStyleCountSame => get_style_count(state, my_style),
        RunningStyleVar::RunningStyleCountSameRate => {
            (get_style_count(state, my_style) as f64 / state.params.gates as f64 * 100.0).floor()
                as i32
        }
        RunningStyleVar::RunningStyleCountSashiOtherself => {
            get_style_count_otherself(state, Style::Late, my_style)
        }
        RunningStyleVar::RunningStyleCountSenkoOtherself => {
            get_style_count_otherself(state, Style::Pace, my_style)
        }
        RunningStyleVar::RunningStyleEqualPopularityOne => 1, // STUB: We are most popular
    }
}

fn get_style_count(state: &RaceState, my_style: Style) -> i32 {
    state.runners_by_style.get(&my_style).cloned().unwrap_or(0) as i32
}

fn get_style_count_otherself(state: &RaceState, style: Style, my_style: Style) -> i32 {
    state.runners_by_style.get(&style).cloned().unwrap_or(0) as i32 - (my_style == style) as i32
}

fn get_style_count_front_otherself(state: &RaceState, my_style: Style) -> i32 {
    // Oonige is considered a Front variant
    get_style_count_otherself(state, Style::Front, my_style)
        + get_style_count_otherself(state, Style::Oonige, my_style)
}

fn get_style_temptation_count(state: &RaceState, style: Style) -> i32 {
    state.rushed_by_style.get(&style).cloned().unwrap_or(0) as i32
}

fn get_style_temptation_count_front(state: &RaceState) -> i32 {
    // Oonige is considered a Front variant
    get_style_temptation_count(state, Style::Front)
        + get_style_temptation_count(state, Style::Oonige)
}

fn get_style_temptation_count_opponent(state: &RaceState, style: Style, runner: &Runner) -> i32 {
    // TODO: Opponents != Otherself; find actual opponent count
    get_style_temptation_count(state, style) - (runner.is_rushed() as i32)
}

fn get_style_temptation_opponent_count_front(state: &RaceState, runner: &Runner) -> i32 {
    get_style_temptation_count(state, Style::Front)
        + get_style_temptation_count(state, Style::Oonige)
        - (runner.is_rushed() as i32)
}

fn get_basic_status_value(var: &StatusVar, builder: &RunnerBuilder) -> i32 {
    match var {
        // Motivation: 1=awful, 2=bad, 3=normal, 4=good, 5=great
        StatusVar::Motivation => builder.motivation as i32,

        // Running style: 1=Front, 2=Pace, 3=Late, 4=End
        StatusVar::RunningStyle => match builder.base.style {
            Style::Oonige => 1, // Oonige is a Front variant
            _ => builder.base.style as i32,
        },

        // Post number (starting gate)
        StatusVar::PostNumber => builder.gate_roll as i32,

        _ => panic!("Unsupported status variable in out-of-gate context"),
    }
}

/// Get status values (HP, last spurt, continuous checks)
fn get_status_value(var: &StatusVar, runner: &Runner) -> i32 {
    match var {
        // Motivation: 1=awful, 2=bad, 3=normal, 4=good, 5=great
        StatusVar::Motivation => runner.motivation as i32,

        // Running style: 1=Front, 2=Pace, 3=Late, 4=End
        StatusVar::RunningStyle => match runner.adjusted.style {
            Style::Oonige => 1, // Oonige is a Front variant
            _ => runner.adjusted.style as i32,
        },

        // Post number (starting gate)
        StatusVar::PostNumber => runner.gate_roll as i32,

        // HP as percentage (0-100)
        StatusVar::HpPer => hp_per(runner),

        // HP is (or has been at some point) empty
        StatusVar::IsHpEmptyOnetime => runner.has_emptied_hp as i32,

        // Is in last spurt mode (not the phase)
        StatusVar::IsLastspurt => {
            if runner.last_spurt.is_some() {
                1
            } else {
                0
            }
        }

        // Last spurt state: 0=can't finish, 1=above base, 2=can finish at max
        StatusVar::Lastspurt => {
            // TODO: change logic to determine outcome of last spurt
            if runner.last_spurt.is_some() {
                2 // Can finish at max
            } else {
                0 // Not in last spurt
            }
        }

        // Start is too late
        StatusVar::IsBadstart => {
            if runner.start_type() == StartType::Late {
                1
            } else {
                0
            }
        }

        // Multi-uma stubs
        StatusVar::IsBehindIn => 0,             // STUB: No uma behind
        StatusVar::BehindNearLaneTime => 0,     // STUB
        StatusVar::BehindNearLaneTimeSet1 => 0, // STUB
        StatusVar::InfrontNearLaneTime => 0,    // STUB

        // Continuous checks
        StatusVar::IsMoveLane => 0, // STUB: Lane change tracking not implemented
        StatusVar::IsOvertake => 0, // STUB: Multi-uma
        StatusVar::IsSurrounded => 0, // STUB: Multi-uma
        StatusVar::IsTemptation => runner.is_rushed() as i32,
        StatusVar::TemptationCount => runner.has_rushed() as i32, // You can only rush once
    }
}

/// Get rank-related values (mostly multi-uma stubs)
fn get_rank_value(var: &OrderVar, _state: &RaceState) -> i32 {
    match var {
        // All multi-uma stubs - return sensible single-uma values
        OrderVar::Order => 1,            // STUB: Always first place
        OrderVar::OrderRate => 0,        // STUB: 0% = top position
        OrderVar::NearCount => 0,        // STUB: No other umas nearby
        OrderVar::NearInfrontCount => 0, // STUB: No umas ahead

        // Change in order - handled by trigger system
        OrderVar::ChangeOrderOnetime => {
            panic!("BUG: change_order_onetime should not be evaluated - it's a trigger variable!")
        }

        // Multi-uma stubs
        OrderVar::ChangeOrderUpEndAfter => 0, // STUB: Never overtaken
        OrderVar::ChangeOrderUpFinalcornerAfter => 0, // STUB
        OrderVar::ChangeOrderUpMiddle => 0,   // STUB

        // Order rate continue checks
        OrderVar::OrderRateIn20Continue => 1, // STUB: Always in top 20% (first place)
        OrderVar::OrderRateIn40Continue => 1, // STUB
        OrderVar::OrderRateIn50Continue => 1, // STUB
        OrderVar::OrderRateIn80Continue => 1, // STUB
        OrderVar::OrderRateOut20Continue => 0, // STUB: Never outside top 20%
        OrderVar::OrderRateOut40Continue => 0, // STUB
        OrderVar::OrderRateOut50Continue => 0, // STUB
        OrderVar::OrderRateOut70Continue => 0, // STUB

        // Overtake timing
        OrderVar::OvertakeTargetNoOrderUpTime => 0, // STUB
        OrderVar::OvertakeTargetTime => 0,          // STUB
    }
}

/// Get blocking values (all multi-uma stubs)
fn get_blocking_value(var: &BlockingVar, _state: &RaceState) -> i32 {
    match var {
        // All blocking variables return 0 (not blocked) in single-uma mode
        BlockingVar::BlockedAllContinuetime => 0, // STUB: Not blocked
        BlockingVar::BlockedFront => 0,           // STUB: Not blocked
        BlockingVar::BlockedFrontContinuetime => 0, // STUB: Not blocked
        BlockingVar::BlockedSideContinuetime => 0, // STUB: Not blocked
    }
}

/// Get temptation/rush values of other umas (all multi-uma stubs)
fn get_temptation_value(var: &TemptationVar, state: &RaceState, runner: &Runner) -> i32 {
    match var {
        // All other umas temptation - multi-uma stubs
        TemptationVar::TemptationCountBehind => 0, // STUB: No umas behind
        TemptationVar::TemptationCountInfront => 0, // STUB: No umas ahead
        TemptationVar::TemptationOpponentCountBehind => 0, // STUB
        TemptationVar::TemptationOpponentCountInfront => 0, // STUB
        TemptationVar::RunningStyleTemptationCountNige => get_style_temptation_count_front(state),
        TemptationVar::RunningStyleTemptationCountOikomi => {
            get_style_temptation_count(state, Style::End)
        }
        TemptationVar::RunningStyleTemptationCountSashi => {
            get_style_temptation_count(state, Style::Late)
        }
        TemptationVar::RunningStyleTemptationCountSenko => {
            get_style_temptation_count(state, Style::Pace)
        }
        TemptationVar::RunningStyleTemptationOpponentCountNige => {
            get_style_temptation_opponent_count_front(state, runner)
        }
        TemptationVar::RunningStyleTemptationOpponentCountOikomi => {
            get_style_temptation_count_opponent(state, Style::End, runner)
        }
        TemptationVar::RunningStyleTemptationOpponentCountSashi => {
            get_style_temptation_count_opponent(state, Style::Late, runner)
        }
        TemptationVar::RunningStyleTemptationOpponentCountSenko => {
            get_style_temptation_count_opponent(state, Style::Pace, runner)
        }
    }
}

/// Get random values
fn get_random_value(var: &RandomVar, _state: &RaceState) -> i32 {
    match var {
        // Pre-rolled at race start - handled by trigger system
        RandomVar::RandomLot => {
            panic!("BUG: random_lot should not be evaluated - it's a trigger variable!")
        }
    }
}

/// Convert HP to percentage (0-100) using floor for consistent rounding
fn hp_per(runner: &Runner) -> i32 {
    (runner.hp / runner.max_hp * 100.0).floor() as i32
}

/// Convert position to distance rate percentage (0-100)
fn distance_rate(position: &f64, course_distance: i32) -> i32 {
    (position / course_distance as f64 * 100.0).floor() as i32
}

/// Calculate remaining distance in meters
fn remain_distance(position: &f64, course_distance: i32) -> i32 {
    (course_distance as f64 - position).floor() as i32
}

/// Convert distance in meters to furlongs
fn distance_to_furlongs(distance_meters: f64) -> i32 {
    meters_to_furlongs(distance_meters).floor() as i32
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::course::*;
    use crate::race::parameters::*;
    use crate::race::solver::RaceSolver;
    use crate::skill::*;
    use crate::uma::Aptitude;

    /// Helper to create a minimal test race state
    fn create_test_race_solver() -> RaceSolver<Runner> {
        let uma = Uma {
            speed: 1200,
            stamina: 1200,
            power: 1200,
            guts: 1200,
            wiz: 1200,
            style: Style::Late,
            distance_aptitude: Aptitude::S,
            track_aptitude: Aptitude::S,
            style_aptitude: Aptitude::S,
        };

        let course_data = CourseData {
            distance: 2000,
            distance_type: DistanceType::Mile,
            surface: Track::Turf,
            turn: Direction::Left,
            course_set_status: vec![],
            finish_time_min: 120000,
            finish_time_max: 150000,
            lane_max: 10000,
            corners: vec![],
            slopes: vec![],
            straights: vec![],
            race_track_id: 10006,
            _course: (),
        };

        let course = Course::new_test(10006, course_data);

        let params = RaceParameters {
            condition: TrackCondition::Good,
            weather: Weather::Sunny,
            season: Season::Spring,
            grade: Grade::G1,
            time: Time::Noon,
            gates: 18,
            rng_seed: 12345,
        };

        let skills = vec![];

        let mut solver = RaceSolver::new(course, params);
        solver.add_runner(uma, Motivation::Good, skills);
        solver.build_runners()
    }

    #[test]
    fn test_is_used_skill_id_with_activated_skill() {
        let mut solver = create_test_race_solver();

        // Add skill ID "202051" to activated_skill_ids
        solver.runners[0]
            .activated_skill_ids
            .insert("202051".to_string());

        // Create comparison: is_used_skill_id == 202051
        let comp = Comp {
            var: VarType::Activation(ActivationVar::IsUsedSkillId),
            op: CompOp::Equal,
            value: 202051,
        };

        // Should return true because skill 202051 is activated
        assert!(EvaluatorFull.evaluate(&comp, &solver.state, &solver.runners[0]));
    }

    #[test]
    fn test_is_used_skill_id_without_activated_skill() {
        let state = create_test_race_solver();

        // No skills in activated_skills

        // Create comparison: is_used_skill_id == 202051
        let comp = Comp {
            var: VarType::Activation(ActivationVar::IsUsedSkillId),
            op: CompOp::Equal,
            value: 202051,
        };

        // Should return false because skill 202051 is not activated
        assert!(!EvaluatorFull.evaluate(&comp, &state.state, &state.runners[0]));
    }

    #[test]
    fn test_is_used_skill_id_with_different_skill() {
        let mut solver = create_test_race_solver();

        // Add skill ID "203781" to activated_skill_ids (different from what we're checking)
        solver.runners[0]
            .activated_skill_ids
            .insert("203781".to_string());

        // Create comparison: is_used_skill_id == 202051
        let comp = Comp {
            var: VarType::Activation(ActivationVar::IsUsedSkillId),
            op: CompOp::Equal,
            value: 202051,
        };

        // Should return false because skill 202051 is not activated (only 203781 is)
        assert!(!EvaluatorFull.evaluate(&comp, &solver.state, &solver.runners[0]));
    }

    #[test]
    fn test_is_used_skill_id_not_equal() {
        let mut state = create_test_race_solver();

        // Add skill ID "202051" to activated_skill_ids
        state.runners[0]
            .activated_skill_ids
            .insert("202051".to_string());

        // Create comparison: is_used_skill_id != 202051
        let comp = Comp {
            var: VarType::Activation(ActivationVar::IsUsedSkillId),
            op: CompOp::NotEqual,
            value: 202051,
        };

        // Should return false because skill 202051 IS activated
        assert!(!EvaluatorFull.evaluate(&comp, &state.state, &state.runners[0]));
    }

    #[test]
    fn test_is_exist_skill_id_in_available_skills() {
        let uma = Uma {
            speed: 1200,
            stamina: 1200,
            power: 1200,
            guts: 1200,
            wiz: 1200,
            style: Style::Late,
            distance_aptitude: Aptitude::S,
            track_aptitude: Aptitude::S,
            style_aptitude: Aptitude::S,
        };

        let course_data = CourseData {
            distance: 2000,
            distance_type: DistanceType::Mile,
            surface: Track::Turf,
            turn: Direction::Left,
            course_set_status: vec![],
            finish_time_min: 120000,
            finish_time_max: 150000,
            lane_max: 10000,
            corners: vec![],
            slopes: vec![],
            straights: vec![],
            race_track_id: 10006,
            _course: (),
        };

        let course = Course::new_test(10006, course_data);

        let params = RaceParameters {
            condition: TrackCondition::Good,
            weather: Weather::Sunny,
            season: Season::Spring,
            grade: Grade::G1,
            time: Time::Noon,
            gates: 18,
            rng_seed: 12345,
        };

        // Create a skill with ID "203061"
        let skill_data = SkillData {
            rarity: SkillRarity::White,
            alternatives: vec![],
        };
        let skill = Skill::new("203061".to_string(), skill_data);

        let skills = vec![skill];

        let mut solver = RaceSolver::new(course, params);
        solver.add_runner(uma, Motivation::Good, skills);
        let solver = solver.build_runners();

        // Create comparison: is_exist_skill_id == 203061
        let comp = Comp {
            var: VarType::Activation(ActivationVar::IsExistSkillId),
            op: CompOp::Equal,
            value: 203061,
        };

        // Should return true because skill 203061 is in available_skills
        assert!(EvaluatorFull.evaluate(&comp, &solver.state, &solver.runners[0]));
    }
}
