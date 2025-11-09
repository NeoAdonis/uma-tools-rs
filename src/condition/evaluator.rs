//! Condition Evaluator - Evaluates guard conditions against race state
//!
//! This module evaluates guard conditions, i.e., conditions that determine whether
//! a triggered skill can activate or not.
//!
//! Trigger conditions (random gates, monotonic thresholds, windows) are handled
//! by the trigger system and should NEVER reach this evaluator.
//!
//! # Architecture
//!
//! Conditions are parsed into tree structures (`Condition` -> `CompList` -> `Comp`)
//! and evaluated recursively:
//!
//! - **Binary operators**: AND (&) and OR (@) with short-circuit evaluation.
//! - **Leaf comparisons**: Variable operator value (e.g., `hp_per <= 50`).
//!
//! The `CompEvaluator` trait abstracts the evaluation context, allowing different
//! implementations for various scenarios. For instance, `EvaluatorFull` uses
//! actual race state and runner data, while `EvaluatorOutOfGate` uses only
//! data available before the race starts to evaluate out-of-gate conditions.
//!
//! Each comparision (`Comp`) is evaluated separately, allowing an evaluator to
//! either fetch real-time values, simulate expected values, or even override
//! certain comparison results for hypothetical scenarios. The latter two might be
//! required if, for some reason, all variables needed for a condition are not
//! known at evaluation time; case in point: single runner simulations where opponent
//! data is not available.
//!
//! # Variable Access
//!
//! It's up to the evaluator implementation to fetch/provide variable values.
//! However, common variable retrieval logic is provided via the `get_variable_value()`
//! function.
//!
//! `get_variable_value()` maps ~150+ condition variables to their current runtime
//! values from `RaceState`. Variables are grouped by categories, each with their
//! own set of parameters required for evaluation. To mention a few:
//!
//! - **Race-level**: elapsed time, course properties, race parameters.
//! - **Runner-level**: position, HP, phase, speed, skill activation state.
//! - **Multi-runner**: Order, near counts, blocking (currently STUBBED, as most
//!   simulations would only care about single runner scenarios).
//!
//! A similar function, `get_variable_value_out_of_gate()`, is provided for
//! out-of-gate evaluations, which only uses data available before the race starts.
//!
//! # Float Handling
//!
//! All comparisons use integer values. Float variables (hp, position, percentages)
//! are converted using `.floor()` for consistent rounding behavior.
//!
//! # Multi-runner Limitations
//!
//! In the current state, variables depending on multiple runners) return sensible
//! stub values. For example:
//!
//! - `order` = 1 (always first place)
//! - `order_rate` = 0 (0% = top position)
//! - `near_count` = 0 (no other runners nearby)
//! - `blocked_front` = 0 (not blocked)
//!
//! These are clearly marked with `// STUB:` comments.

use crate::condition::parser::*;
use crate::condition::variables::*;
use crate::course::Course;
use crate::race::parameters::RaceParameters;
use crate::race::runner::{Runner, StartType};
use crate::race::solver::RaceState;
use crate::race::unready_runner::UnreadyRunner;
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
/// ```rust,ignore
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
    skill_id: String,
    state: &E::State,
    context: &E::Context,
) -> bool
where
    E: CompEvaluator,
{
    evaluate_comp_list(&condition.comp_list, |comp| {
        evaluator.evaluate(comp, skill_id.as_str(), state, context)
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
    fn evaluate(
        &self,
        comp: &Comp,
        skill_id: &str,
        state: &Self::State,
        context: &Self::Context,
    ) -> bool;
}

/// Reference evaluator using actual race state and runner data.
pub struct EvaluatorFull;
impl CompEvaluator for EvaluatorFull {
    type State = RaceState;
    type Context = Runner;

    /// Evaluate a single comparison using actual race state and runner data.
    fn evaluate(
        &self,
        comp: &Comp,
        skill_id: &str,
        state: &Self::State,
        context: &Self::Context,
    ) -> bool {
        use VarType::*;
        let runner = context;
        // Special cases: skill ID checks require different logic
        match &comp.var {
            Activation(ActivationVar::IsUsedSkillId) => {
                // Check if skill with ID == comp.value has been activated
                let skill_id_str = comp.value.to_string();
                let skill_exists = runner.activated_skill_ids.contains(&skill_id_str);

                match comp.op {
                    CompOp::Equal => skill_exists,
                    CompOp::NotEqual => !skill_exists,
                    _ => false, // Other operators don't make sense for skill ID checks
                }
            }
            Activation(ActivationVar::IsExistSkillId) => {
                // Check if at least one runner has skill with ID == comp.value
                state.existing_skills.contains_key(&comp.value.to_string())
            }
            RaceParam(RaceParamVar::SameSkillHorseCount) => {
                // Count of runners with same skill
                let count = state.existing_skills.get(skill_id).cloned().unwrap_or(0) as i32;
                match comp.op {
                    CompOp::Equal => count == comp.value,
                    CompOp::NotEqual => count != comp.value,
                    CompOp::Greater => count > comp.value,
                    CompOp::GreaterEqual => count >= comp.value,
                    CompOp::Less => count < comp.value,
                    CompOp::LessEqual => count <= comp.value,
                }
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
    type Context = UnreadyRunner;

    /// Evaluate a single comparison in out-of-gate context, using only data available before a race starts.
    fn evaluate(
        &self,
        comp: &Comp,
        skill_id: &str,
        state: &Self::State,
        context: &Self::Context,
    ) -> bool {
        use VarType::*;
        match &comp.var {
            // Special case: skill ID checks require different logic
            Activation(ActivationVar::IsExistSkillId) => {
                // Check if at least one runner has skill with ID == comp.value
                state.existing_skills.contains_key(&comp.value.to_string())
            }
            RaceParam(RaceParamVar::SameSkillHorseCount) => {
                // Count of runners with same skill
                let count = state.existing_skills.get(skill_id).cloned().unwrap_or(0) as i32;
                match comp.op {
                    CompOp::Equal => count == comp.value,
                    CompOp::NotEqual => count != comp.value,
                    CompOp::Greater => count > comp.value,
                    CompOp::GreaterEqual => count >= comp.value,
                    CompOp::Less => count < comp.value,
                    CompOp::LessEqual => count <= comp.value,
                }
            }
            _ => {
                // Normal case: get variable value and compare
                let current_value = get_variable_value_out_of_gate(&comp.var, state, context);
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
    use VarType::*;
    match var {
        BaseStat(stat_var) => get_base_stat(stat_var, &runner.base),
        Order(pos_var) => get_rank_value(pos_var, runner),
        Distance(dist_var) => get_distance_value(dist_var, runner),
        Phase(phase_var) => get_phase_value(phase_var, &state.course, runner),
        Course(course_var) => get_course_value(course_var, &state.course),
        Spatial(location_var) => get_location_value(location_var, &state.course, runner),
        Activation(act_var) => get_activation_value(act_var, runner),
        RaceParam(param_var) => get_race_param_value(param_var, &state.params),
        RunningStyle(style_var) => get_running_style_value(style_var, state, runner.adjusted.style),
        Status(status_var) => get_status_value(status_var, runner),
        Random(trigger_var) => get_random_value(trigger_var, state),
        Blocking(blocking_var) => get_blocking_value(blocking_var, runner),
        Temptation(tempt_var) => get_temptation_value(tempt_var, state, runner),
        AccumulateTime => state.elapsed_time.floor() as i32,
        Always => 1,     // Always returns 1 (true)
        Unknown(_) => 0, // Default to 0
    }
}

/// Get current integer value for any variable type in out-of-gate context
fn get_variable_value_out_of_gate(
    var: &VarType,
    state: &RaceState,
    builder: &UnreadyRunner,
) -> i32 {
    use VarType::*;
    match var {
        BaseStat(stat_var) => get_base_stat(stat_var, &builder.base),
        Course(course_var) => get_course_value(course_var, &state.course),
        RaceParam(param_var) => get_race_param_value(param_var, &state.params),
        RunningStyle(style_var) => get_running_style_value(style_var, state, builder.base.style),
        Status(status_var) => get_basic_status_value(status_var, builder),
        Always => 1, // Always returns 1 (true)
        _ => 0,      // Not applicable in out-of-gate context
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

/// Get values related to distance between runners
fn get_distance_value(var: &DistanceVar, runner: &Runner) -> i32 {
    match var {
        DistanceVar::BashinDiffBehind => {
            meters_to_bashin(runner.position_diff.from_prev).floor() as i32
        }
        DistanceVar::BashinDiffInfront => {
            meters_to_bashin(runner.position_diff.from_next).floor() as i32
        }
        DistanceVar::DistanceDiffRate => runner.position_diff.rate_from_first.floor() as i32,
        DistanceVar::DistanceDiffTop => runner.position_diff.from_first.floor() as i32,
        DistanceVar::DistanceDiffTopFloat => {
            (runner.position_diff.from_first * 10.0).floor() as i32
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

        // Remaining distance for any player (non-NPC) runner
        SpatialVar::RemainDistanceViewerId => {
            unimplemented!("remain_distance_viewer_id not implemented")
        }

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

        // Popularity rank
        RaceParamVar::Popularity => 1, // STUB: Always most popular in single-uma mode

        // Season: 1=spring, 2=summer, 3=autumn, 4=winter, 5=sakura
        //
        // N.B. Cherry blossom season is semantically part of spring;
        // conditions that check for spring should also match sakura.
        // When applicable, this is handled directly in the condition
        // definitions (i.e., "season==1@season==5").
        RaceParamVar::Season => params.season as i32,

        // Time of day: 0=none/any, 1=morning, 2=noon, 3=evening, 4=night
        RaceParamVar::Time => params.time as i32,

        // Weather: 1=sunny, 2=cloudy, 3=rainy, 4=snowy
        RaceParamVar::Weather => params.weather as i32,

        // Count of runners with same skill
        RaceParamVar::SameSkillHorseCount => {
            panic!("same_skill_horse_count: should be handled in evaluate_comparison()")
        }

        // Existence checks - out of scope
        RaceParamVar::IsExistCharaId => unimplemented!("is_exist_chara_id not implemented"),
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

        // Continuous checks
        ActivationVar::IsActivateAnySkill => runner.skill_activated_flag as i32,
        ActivationVar::IsActivateHealSkill => runner.heal_activated_flag as i32,
        ActivationVar::IsActivateOtherSkillDetail => 0, // STUB: Needs tracking

        ActivationVar::IsOtherCharacterActivateAdvantageSkill => 0, // STUB

        // Duel-related counts
        ActivationVar::CompeteFightCount => runner.duel_count as i32,

        // These cases are handled specially because they use the comparison
        // value as parameter instead of comparing against it.
        ActivationVar::IsExistSkillId => {
            panic!("is_exist_skill_id: should be handled in evaluate_comparison()")
        }
        ActivationVar::IsUsedSkillId => {
            panic!("is_used_skill_id: should be handled in evaluate_comparison()")
        }

        // Team-based checks - out of scope
        ActivationVar::ActivateCountAllTeam => {
            unimplemented!("activate_count_all_team not implemented")
        }
    }
}

/// Get running style values
fn get_running_style_value(var: &RunningStyleVar, state: &RaceState, my_style: Style) -> i32 {
    match var {
        // TODO: generate simulation values for one-runner simulations
        // Running style: 1=Front, 2=Pace, 3=Late, 4=End
        RunningStyleVar::RunningStyle => match my_style {
            Style::Runaway => 1, // Runaway is a Front variant
            _ => my_style as i32,
        },
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
    // Runaway is considered a Front variant
    get_style_count_otherself(state, Style::Front, my_style)
        + get_style_count_otherself(state, Style::Runaway, my_style)
}

fn get_style_temptation_count(state: &RaceState, style: Style) -> i32 {
    state.rushed_by_style.get(&style).cloned().unwrap_or(0) as i32
}

fn get_style_temptation_count_front(state: &RaceState) -> i32 {
    // Runaway is considered a Front variant
    get_style_temptation_count(state, Style::Front)
        + get_style_temptation_count(state, Style::Runaway)
}

fn get_style_temptation_count_opponent(state: &RaceState, style: Style, runner: &Runner) -> i32 {
    // TODO: Opponents != Otherself; find actual opponent count
    get_style_temptation_count(state, style) - (runner.is_rushed() as i32)
}

fn get_style_temptation_opponent_count_front(state: &RaceState, runner: &Runner) -> i32 {
    get_style_temptation_count(state, Style::Front)
        + get_style_temptation_count(state, Style::Runaway)
        - (runner.is_rushed() as i32)
}

fn get_basic_status_value(var: &StatusVar, builder: &UnreadyRunner) -> i32 {
    match var {
        // Motivation: 1=awful, 2=bad, 3=normal, 4=good, 5=great
        StatusVar::Motivation => builder.motivation as i32,

        // Gate bracket: 1=inner-most, 8=outer-most
        StatusVar::PostNumber => (builder.gate_block + 1) as i32,

        _ => panic!("Unsupported status variable in out-of-gate context"),
    }
}

/// Get status values (HP, last spurt, continuous checks)
fn get_status_value(var: &StatusVar, runner: &Runner) -> i32 {
    match var {
        // Motivation: 1=awful, 2=bad, 3=normal, 4=good, 5=great
        StatusVar::Motivation => runner.motivation as i32,

        // Post number (starting gate)
        StatusVar::PostNumber => runner.gate_roll as i32,

        // HP as percentage (0-100)
        StatusVar::HpPer => runner.hp_percent().floor() as i32,

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

        // Multi-runner checks
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

/// Get rank-related values
fn get_rank_value(var: &OrderVar, runner: &Runner) -> i32 {
    match var {
        // Order
        OrderVar::Order => runner.order.order as i32,
        OrderVar::OrderRate => runner.order.rate as i32,

        // Near counts
        OrderVar::NearCount => runner.nearby_count as i32,
        OrderVar::NearInfrontCount => runner.nearby_front_count as i32,

        // Order change checks
        OrderVar::ChangeOrderOnetime => runner.order.delta as i32,
        OrderVar::ChangeOrderUpEndAfter => runner.order.overtakes_since_late as i32,
        OrderVar::ChangeOrderUpFinalcornerAfter => runner.order.overtakes_since_last_corner as i32,
        OrderVar::ChangeOrderUpMiddle => runner.order.overtakes_since_mid as i32,

        // Order rate continue checks
        OrderVar::OrderRateIn20Continue => (runner.order.rate_min <= 20) as i32,
        OrderVar::OrderRateIn40Continue => (runner.order.rate_min <= 40) as i32,
        OrderVar::OrderRateIn50Continue => (runner.order.rate_min <= 50) as i32,
        OrderVar::OrderRateIn80Continue => (runner.order.rate_min <= 80) as i32,
        OrderVar::OrderRateOut20Continue => (runner.order.rate_max > 20) as i32,
        OrderVar::OrderRateOut40Continue => (runner.order.rate_max > 40) as i32,
        OrderVar::OrderRateOut50Continue => (runner.order.rate_max > 50) as i32,
        OrderVar::OrderRateOut70Continue => (runner.order.rate_max > 70) as i32,

        // Target overtake times
        OrderVar::OvertakeTargetNoOrderUpTime => 0, // STUB
        OrderVar::OvertakeTargetTime => 0,          // STUB
    }
}

/// Get blocking values
fn get_blocking_value(var: &BlockingVar, runner: &Runner) -> i32 {
    match var {
        BlockingVar::BlockedAllContinuetime => {
            runner.blocked.front_time.min(runner.blocked.side_time) as i32
        }
        BlockingVar::BlockedFront => runner.is_front_blocked() as i32,
        BlockingVar::BlockedFrontContinuetime => runner.blocked.front_time as i32,
        BlockingVar::BlockedSideContinuetime => runner.blocked.side_time as i32,
        BlockingVar::Visiblehorse => runner.visible_count as i32,
    }
}

/// Get temptation/rush values of other umas
fn get_temptation_value(var: &TemptationVar, state: &RaceState, runner: &Runner) -> i32 {
    match var {
        // Rush counts by position
        TemptationVar::TemptationCountBehind => runner.rush_counter.behind as i32,
        TemptationVar::TemptationCountInfront => runner.rush_counter.ahead as i32,
        TemptationVar::TemptationOpponentCountBehind => runner.rush_counter.behind as i32, // TODO: find actual opponent count
        TemptationVar::TemptationOpponentCountInfront => runner.rush_counter.ahead as i32, // TODO: find actual opponent count

        // Rush counts by running style
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
