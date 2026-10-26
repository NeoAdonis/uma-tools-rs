//! # Condition Evaluator
//!
//! This module evaluates guard conditions, i.e., conditions that determine
//! whether a triggered skill can activate or not.
//!
//! Trigger conditions (random gates, monotonic thresholds, windows) are handled
//! by the trigger system and should NEVER reach this evaluator.
//!
//! ## Architecture
//!
//! Conditions are parsed into OR branches ([`Condition::branches`]), each one
//! being a flat list of comparisons that must all be true. This module
//! evaluates those lists with [`evaluate_comparison_list`], one comparison
//! (e.g., `hp_per <= 50`) at a time.
//!
//! The [`CompEvaluator`] trait abstracts the evaluation context, allowing
//! different implementations for various scenarios. For instance,
//! [`EvaluatorFull`] uses actual race state and runner data, while
//! [`EvaluatorOutOfGate`] uses only data available before the race starts to
//! evaluate out-of-gate conditions.
//!
//! Comparisons must have been rewritten for the runner first (see
//! [Rewriting](#rewriting)).
//!
//! Each [`Comparison`] is evaluated separately, allowing an evaluator to
//! either fetch real-time values, simulate expected values, or even override
//! certain comparison results for hypothetical scenarios. The latter two might
//! be required if, for some reason, all variables needed for a condition are
//! not known at evaluation time; case in point: single runner simulations where
//! opponent data is not available.
//!
//! ## Rewriting
//!
//! Some comparisons can't be evaluated by reading a variable and comparing it
//! against a value, but they can be turned into comparisons that can, once
//! facts fixed before the race are known: how many gates the race has, or
//! whether a runner ignores popularity requirements. [`rewrite_comparison`]
//! rewrites them for a given runner, which keeps these special cases out of
//! the evaluators:
//!
//! - `order_rate OP X` becomes `order OP round(gates * X%)` (see
//!   [`order_at_rate`]).
//! - Comparisons on `popularity` are always true, so dropped, if the runner
//!   ignores popularity requirements (see
//!   [`InitialState::ignores_popularity`]).
//!
//! In-race guards are rewritten once, when the runner is created. Out-of-gate
//! conditions are rewritten right before they're evaluated instead, since a
//! runner may start ignoring popularity requirements in the middle of its
//! out-of-gate skills.
//!
//! ## Variable Access
//!
//! It's up to the evaluator implementation to fetch/provide variable values.
//! However, common variable retrieval logic is provided via the
//! [`get_variable_value()`] function.
//!
//! [`get_variable_value()`] maps ~150+ condition variables (see the
//! [variable reference](crate::condition::variable_reference)) to their current
//! runtime values from [`RaceState`] or [`Runner`]. Variables are grouped by
//! categories, each with their own set of parameters required for evaluation.
//! To mention a few:
//!
//! - **Race-level**: elapsed time, course properties, race parameters.
//! - **Runner-level**: position, HP, phase, speed, skill activation state.
//! - **Multi-runner**: order, near counts, blocking; these values are
//!   precomputed during race updates, then stored either as race-level or
//!   runner-level values depending on their nature.
//!
//! A similar function, [`get_variable_value_out_of_gate()`], is provided for
//! out-of-gate evaluations, which only uses data available before a race
//! starts.
//!
//! ## Float Handling
//!
//! All comparisons use integer values. Float variables (HP, position,
//! percentages) are converted using [`f64::floor()`] for consistent rounding
//! behavior.
//!
//! ## Unimplemented Variables
//!
//! There are variables that are not implemented at all as they are only used in
//! non-PvP contexts, which is out of scope for this simulator. The evaluator
//! will panic if these are encountered:
//!
//! - `is_exist_chara_id`
//! - `remain_distance_viewer_id`

use crate::condition::parser::*;
use crate::condition::variables::*;
use crate::course::Course;
use crate::course::geometry::Segment;
use crate::race::parameters::RaceParameters;
use crate::race::runner::{InitialState, order_at_rate};
use crate::race::runner::{Runner, SkillActivationFlags, StartType};
use crate::race::solver::RaceState;
use crate::race::unready_runner::UnreadyRunner;
use crate::trigger::skill_activation::SkillActivationLogic;
use crate::uma::*;
use crate::units::*;

/// Returns `true` if ALL comparisons from a list evaluate to `true`, or list
/// is empty.
///
/// This is the primary function for evaluating guards after trigger-guard
/// separation. Guards are guaranteed to be a flat list of comparisons that need
/// to be satisfied for a skill to activate.
///
/// Short-circuit evaluation is used: it stops at the first false comparison.
///
/// For example, it's used inside [`Runner`] to evaluate guards as follows:
///
/// ```rust,ignore
/// let logic = &self.trigger_scheduler.skill_logics[trigger_index];
/// let should_activate = evaluate_comparison_list(
///     &logic.guards,
///     &EvaluatorFull,
///     Some(logic),
///     state,
///     self,
/// );
/// ```
///
/// Examples of guard evaluation:
///
/// - `[]` → true (no guards, always activates).
/// - `[ground_type==2]` → true if ground is dirt.
/// - `[ground_type==2, motivation>=4]` → true if both conditions are met.
/// - `[hp_per>=20, running_style==1]` → true if HP >= 20% AND style is Front
///   Runner.
///
/// This function panics if any comparison contains trigger variables that
/// should have been separated during the splitting phase.
pub fn evaluate_comparison_list<E>(
    comparisons: &ComparisonList,
    evaluator: &E,
    logic_ctx: Option<&SkillActivationLogic>,
    state: &E::State,
    context: &E::Context,
) -> bool
where
    E: CompEvaluator,
{
    // Short-circuit AND: return false on first failure.
    comparisons
        .iter()
        .all(|comp| evaluator.evaluate(comp, logic_ctx, state, context))
}

/// Rewrite a comparison for a runner (see [Rewriting](self#rewriting)).
///
/// Returns `None` if the comparison is always true for the runner.
pub fn rewrite_comparison(
    comp: &Comparison,
    state: &RaceState,
    entry: &InitialState,
) -> Option<Comparison> {
    use VarType::*;

    match &comp.var {
        Entry(EntryVar::Popularity) if entry.ignores_popularity => None,
        Order(OrderVar::OrderRate) => Some(Comparison {
            var: Order(OrderVar::Order),
            op: comp.op,
            value: order_at_rate(state.params.gates, comp.value),
        }),
        _ => Some(comp.clone()),
    }
}

/// Trait for evaluating a single comparison.
///
/// Implement this trait for different evaluation contexts.
/// `State` and `Context` associated types define the data sources used.
/// `State` should represent the overall race state, while `Context` represents
/// the runner data to use.
///
/// The main use case is to get values from a provided race state and runner,
/// then perform the comparison directly.
/// You could also create your own race state with some of the values replaced
/// by estimations or other expectations as needed. For instance, you could
/// simulate best-case or worst-case scenarios by overriding certain variable
/// values rather than having to craft a specific set of runner data to match
/// those scenarios.
///
/// The current implementation provides the following evaluators for different
/// contexts:
///
/// - [`EvaluatorFull`]: uses actual race state ([`RaceState`]) and runner data
///   ([`Runner`]) to evaluate conditions during a race.
/// - [`EvaluatorOutOfGate`]: uses only data available before a race starts as
///   defined by [`RaceState`] and [`UnreadyRunner`].
pub trait CompEvaluator {
    type State;
    type Context;

    /// Evaluate a single comparison.
    fn evaluate(
        &self,
        comp: &Comparison,
        logic_ctx: Option<&SkillActivationLogic>,
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
        comp: &Comparison,
        logic_ctx: Option<&SkillActivationLogic>,
        state: &Self::State,
        context: &Self::Context,
    ) -> bool {
        use VarType::*;
        let runner = context;
        // Special cases: skill queries require different logic.
        match &comp.var {
            ActivationQuery(var) => evaluate_activation_query(var, comp, state, runner),
            Lineup(LineupVar::IsExistSkillId) => evaluate_is_exist_skill_id(comp, state),
            _ => {
                // Normal case: get variable value and compare.
                let current_value = get_variable_value(&comp.var, state, runner, logic_ctx);
                let threshold = comp.value;

                comp.op.apply(current_value, threshold)
            }
        }
    }
}

/// Evaluator for out-of-gate context (before a race starts).
pub struct EvaluatorOutOfGate;
impl CompEvaluator for EvaluatorOutOfGate {
    type State = RaceState;
    type Context = UnreadyRunner;

    /// Evaluate a single comparison in out-of-gate context, using only data
    /// available before a race starts.
    fn evaluate(
        &self,
        comp: &Comparison,
        _logic_ctx: Option<&SkillActivationLogic>,
        state: &Self::State,
        context: &Self::Context,
    ) -> bool {
        use VarType::*;
        match &comp.var {
            // Special case: skill queries require different logic.
            Lineup(LineupVar::IsExistSkillId) => evaluate_is_exist_skill_id(comp, state),
            // "random_lot==X" means X% chance, rolled every time it's evaluated.
            // N.B. It currently behaves like `random_lot_shared`: skills using it have
            // a single alternative, or alternatives that exclude each other (e.g.
            // `popularity<=3` and `popularity>=4`), so it's never rolled twice.
            Random(RandomVar::RandomLot) => context.roll_lot() < comp.value as f64,
            // Like `random_lot`, but rolled once per skill.
            Random(RandomVar::RandomLotShared) => context.evaluated_skill_lot() < comp.value as f64,
            _ => {
                // Normal case: get variable value and compare.
                let current_value = get_variable_value_out_of_gate(&comp.var, state, context);
                let threshold = comp.value;

                comp.op.apply(current_value, threshold)
            }
        }
    }
}

/// Get current integer value for any variable type.
pub fn get_variable_value(
    var: &VarType,
    state: &RaceState,
    runner: &Runner,
    logic_ctx: Option<&SkillActivationLogic>,
) -> i32 {
    use VarType::*;
    match var {
        BaseStat(stat_var) => get_base_stat(stat_var, &runner.base),
        Entry(entry_var) => get_entry_value(entry_var, &runner.initial_state),
        Order(pos_var) => get_rank_value(pos_var, runner),
        Distance(dist_var) => get_distance_value(dist_var, runner),
        Phase(phase_var) => get_phase_value(phase_var, &state.course, runner),
        Course(course_var) => get_course_value(course_var, &state.course),
        Spatial(location_var) => get_location_value(location_var, &state.course, runner),
        Activation(act_var) => get_activation_value(act_var, state, runner, logic_ctx),
        RaceParam(param_var) => get_race_param_value(param_var, &state.params),
        RunningStyle(style_var) => get_running_style_value(style_var, state, runner.adjusted.style),
        Status(status_var) => get_status_value(status_var, runner),
        Blocking(blocking_var) => get_blocking_value(blocking_var, runner),
        Temptation(tempt_var) => get_temptation_value(tempt_var, state, runner),
        Lineup(lineup_var) => {
            // Extract the skill being evaluated from logic context.
            let skill_id = logic_ctx
                .and_then(|logic| runner.available_skills.get(logic.skill_index))
                .map_or(0, |skill| skill.id);
            get_lineup_value(lineup_var, state, skill_id)
        }
        AccumulateTime => state.elapsed_time.floor() as i32,
        // Always returns 1 (true).
        Always => 1,
        ActivationQuery(_) => panic!("BUG: {var} should be handled in evaluate()"),
        // Normally a trigger; evaluates to whether the all-out spurt is active.
        RandomSpatial(RandomSpatialVar::RunAtFullSpeedRandom) => {
            runner.all_out_spurt.is_active() as i32
        }
        Random(_) | RandomSpatial(_) | RandomPhase(_) => {
            panic!(
                "BUG: Random variable {:?} should not be evaluated - it's a trigger variable!",
                var
            )
        }
    }
}

/// Get current integer value for any variable type in out-of-gate context.
pub fn get_variable_value_out_of_gate(
    var: &VarType,
    state: &RaceState,
    builder: &UnreadyRunner,
) -> i32 {
    use VarType::*;
    match var {
        BaseStat(stat_var) => get_base_stat(stat_var, &builder.base),
        Entry(entry_var) => get_entry_value(entry_var, &builder.initial_state),
        Course(course_var) => get_course_value(course_var, &state.course),
        RaceParam(param_var) => get_race_param_value(param_var, &state.params),
        RunningStyle(style_var) => get_running_style_value(style_var, state, builder.base.style),
        Lineup(lineup_var) => get_lineup_value(lineup_var, state, builder.evaluated_skill_id),
        // Always returns 1 (true).
        Always => 1,
        // Not applicable in out-of-gate contexts.
        _ => 0,
    }
}

/// Get base stat value.
fn get_base_stat(stat: &BaseStatVar, uma: &Uma) -> i32 {
    use BaseStatVar::*;
    match stat {
        BaseSpeed => uma.speed as i32,
        BaseStamina => uma.stamina as i32,
        BasePower => uma.power as i32,
        BaseGuts => uma.guts as i32,
        BaseWiz => uma.wit as i32,
    }
}

/// Get values related to distance between runners.
fn get_distance_value(var: &DistanceVar, runner: &Runner) -> i32 {
    use DistanceVar::*;
    match var {
        BashinDiffBehind => meters_to_bashin(runner.position_diff.from_prev).floor() as i32,
        BashinDiffInfront => meters_to_bashin(runner.position_diff.from_next).floor() as i32,
        DistanceDiffRate => runner.position_diff.rate_from_first.floor() as i32,
        DistanceDiffTop => runner.position_diff.from_first.floor() as i32,
        DistanceDiffTopFloat => (runner.position_diff.from_first * 10.0).floor() as i32,
    }
}

/// Get phase-related values.
fn get_phase_value(var: &PhaseVar, course: &Course, runner: &Runner) -> i32 {
    use PhaseVar::*;

    match var {
        // Current phase: 0=Early, 1=Mid, 2=Late, 3=Last.
        Phase => runner.phase as i32,

        // Phase subdivisions: phase number if inside subdivision, -1 otherwise.
        PhaseFirstquarter => {
            let (_, end) = runner.phase.boundaries_first_quarter(course);
            if runner.position < end {
                runner.phase as i32
            } else {
                -1
            }
        }
        PhaseFirsthalf => {
            let (_, end) = runner.phase.boundaries_first_half(course);
            if runner.position < end {
                runner.phase as i32
            } else {
                -1
            }
        }
        PhaseLaterhalf => {
            let (start, _) = runner.phase.boundaries_second_half(course);
            if runner.position >= start {
                runner.phase as i32
            } else {
                -1
            }
        }
    }
}

/// Get course-related values.
fn get_course_value(var: &CourseVar, course: &Course) -> i32 {
    use CourseVar::*;

    match var {
        // Total corners on course.
        CornerCount => course.corners.len() as i32,

        // Direct course distance.
        CourseDistance => course.distance,

        // Distance type: 1=short, 2=mile, 3=medium, 4=long.
        DistanceType => course.distance_type as i32,

        // Ground type: 1=turf, 2=dirt.
        GroundType => course.surface as i32,

        // Is held at a racetrack outside Japan: Longchamp, Santa Anita Park or Del Mar.
        IsAbroad => course.is_abroad() as i32,

        // Is basis distance (multiple of 400m).
        IsBasisDistance => course.is_basis_distance() as i32,

        // Is exchange race (dirt grade): races at Ooi, Kawasaki, Funabashi or Morioka.
        IsDirtgrade => course.is_exchange_race() as i32,

        // Is held at a racetrack with tight turns.
        IsTightTrack => course.is_tight_track() as i32,

        // Rotation: 1=clockwise, 2=counterclockwise, 4=straight.
        Rotation => course.turn as i32,

        // Track ID.
        TrackId => course.race_track_id as i32,
    }
}

/// Get location-related values.
fn get_location_value(var: &SpatialVar, course: &Course, runner: &Runner) -> i32 {
    use SpatialVar::*;

    match var {
        // Which corner: 0=none, 1-4=corner number.
        Corner => runner
            .segments
            .corner()
            .map_or(0, |idx| course.corner_number(idx) as i32),

        // Is on final corner or beyond.
        IsFinalcorner => {
            if course.corners.is_empty() {
                // False if no corners.
                0
            } else {
                // "Beyond" also applies for a straight after final corner.
                let last_corner_idx = course.corners.len() - 1;
                let last_corner = &course.corners[last_corner_idx];
                if runner.position >= last_corner.start {
                    1
                } else {
                    0
                }
            }
        }

        // Is on second half of final corner.
        IsFinalcornerLaterhalf => {
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

        // Is on last straight.
        IsLastStraight => {
            if course.corners.is_empty() {
                // First (and only) straight is last straight.
                1
            } else {
                // Last straight starts after final corner ends.
                let last_corner = course.corners.last().unwrap();
                if runner.position >= last_corner.end() {
                    1
                } else {
                    0
                }
            }
        }

        // One-time trigger - handled by trigger system.
        IsLastStraightOnetime => {
            panic!(
                "BUG: is_last_straight_onetime should not be evaluated - it's a trigger variable!"
            )
        }

        // Current lane: 0=inner, 1=middle, 2=outer, 3=outside.
        LaneType => runner.lane_type() as i32,

        // Slope: 0=flat, 1=uphill, 2=downhill.
        Slope => {
            if let Some(idx) = runner.segments.slope() {
                course.slopes[idx].slope_type() as i32
            } else {
                0
            }
        }

        // Straight front type: 1=facing audience, 2=opposite from audience.
        StraightFrontType => {
            if let Some(idx) = runner.segments.straight() {
                course.straights[idx].front_type as i32
            } else {
                // Not in a straight.
                0
            }
        }

        // Distance covered as percentage (0-100).
        DistanceRate => course.distance_rate(runner.position).floor() as i32,

        // Distance in furlongs.
        Furlong => meters_to_furlongs(runner.position).floor() as i32,

        // Remaining distance in meters.
        RemainDistance => course.remain_distance(runner.position).floor() as i32,

        // Remaining distance for any player (non-NPC) runner.
        RemainDistanceViewerId => {
            unimplemented!("remain_distance_viewer_id not implemented")
        }
    }
}

/// Get race parameter values.
fn get_race_param_value(var: &RaceParamVar, params: &RaceParameters) -> i32 {
    use RaceParamVar::*;

    match var {
        // Track condition: 1=firm, 2=good, 3=soft, 4=heavy.
        GroundCondition => params.track_condition as i32,

        // Race grade: refer to Grade enum for values.
        Grade => params.grade as i32,

        // Season: 1=spring, 2=summer, 3=autumn, 4=winter, 5=sakura.
        //
        // N.B. Cherry blossom season is semantically part of spring, so conditions that
        // check for spring should also match sakura. When applicable, this is handled
        // directly in the condition definitions (i.e., "season==1@season==5").
        Season => params.season as i32,

        // Time of day: 0=none/any, 1=morning, 2=noon, 3=evening, 4=night.
        Time => params.time as i32,

        // Weather: 1=sunny, 2=cloudy, 3=rainy, 4=snowy.
        Weather => params.weather as i32,
    }
}

/// Get lineup values (from every runner entered in the race).
///
/// `skill_id` is the ID of the skill whose condition is being evaluated.
fn get_lineup_value(var: &LineupVar, state: &RaceState, skill_id: i32) -> i32 {
    use LineupVar::*;

    match var {
        // Number of runners with the skill being evaluated.
        SameSkillHorseCount => state.existing_skills.get(&skill_id).copied().unwrap_or(0) as i32,

        IsExistSkillId => panic!("BUG: {var} should be handled in evaluate()"),

        // Existence checks - out of scope.
        IsExistCharaId => unimplemented!("is_exist_chara_id not implemented"),
    }
}

/// Evaluate `is_exist_skill_id`: whether any runner has the skill with ID ==
/// `comp.value`.
///
/// Unlike most variables, it uses the comparison value as parameter instead of
/// comparing against it.
fn evaluate_is_exist_skill_id(comp: &Comparison, state: &RaceState) -> bool {
    let exists = state.existing_skills.contains_key(&comp.value);
    match comp.op {
        CompOp::Equal => exists,
        CompOp::NotEqual => !exists,
        // Rejected by the parser.
        op => unreachable!("is_exist_skill_id doesn't support operator {op:?}"),
    }
}

/// Evaluate a comparison on skill activations that uses the comparison value
/// as parameter (a skill ID or type) instead of comparing against it.
fn evaluate_activation_query(
    var: &ActivationQueryVar,
    comp: &Comparison,
    state: &RaceState,
    runner: &Runner,
) -> bool {
    use ActivationQueryVar::*;

    match var {
        IsUsedSkillId | IsUsedSkillIdWithDetailOne => {
            // Check if skill with ID == `comp.value` has been activated (through
            // its first alternative for `is_used_skill_id_with_detail_one`).
            let mask = runner.activated_mask(comp.value);
            let is_used = match var {
                IsUsedSkillId => mask != 0,
                _ => mask & 1 != 0,
            };

            match comp.op {
                CompOp::Equal => is_used,
                CompOp::NotEqual => !is_used,
                // Rejected by the parser.
                op => unreachable!("{var} doesn't support operator {op:?}"),
            }
        }
        IsOtherCharacterActivateAdvantageSkill | IsPopularityTopCharacterActivateAdvantageSkill => {
            // Check if a buff whose type is `comp.value` (or any skill for -1) has
            // been activated this frame by any other runner, or by the most popular
            // one (the runner herself included).
            // Unimplemented skill types are rejected by the parser.
            let value = (comp.value != -1).then(|| {
                skill_type(comp.value).unwrap_or_else(|| {
                    unreachable!("{var}: unimplemented skill type {}", comp.value)
                })
            });
            let activated = |flags: &SkillActivationFlags| match value {
                Some(value) => flags.buff_activated.contains(&value),
                None => flags.any_activated,
            };
            match var {
                IsOtherCharacterActivateAdvantageSkill => {
                    // -1 is rejected by the parser.
                    let value = value.unwrap_or_else(|| unreachable!("{var}: any skill"));
                    // Discount the runner's own activation.
                    let own = activated(&runner.activation_flags) as u8;
                    state
                        .buffs_activated
                        .get(&value)
                        .is_some_and(|&count| count > own)
                }
                _ => activated(&state.popularity_top_activations),
            }
        }
    }
}

/// Get activation counter values.
fn get_activation_value(
    var: &ActivationVar,
    state: &RaceState,
    runner: &Runner,
    logic_ctx: Option<&SkillActivationLogic>,
) -> i32 {
    use ActivationVar::*;

    match var {
        ActivateCountAll => runner.trigger_scheduler.activation_counts.all as i32,
        ActivateCountEndAfter => runner.trigger_scheduler.activation_counts.end_after as i32,
        ActivateCountHeal => runner.trigger_scheduler.activation_counts.heal as i32,
        ActivateCountLaterHalf => runner.trigger_scheduler.activation_counts.later_half as i32,
        ActivateCountMiddle => runner.trigger_scheduler.activation_counts.middle as i32,
        ActivateCountParamSkill => runner.trigger_scheduler.activation_counts.green as i32,
        ActivateCountStart => runner.trigger_scheduler.activation_counts.start as i32,

        // Skill activations in the current frame.
        IsActivateAnySkill => runner.activation_flags.any_activated as i32,
        IsActivateHealSkill => runner.activation_flags.heal_activated as i32,
        IsActivateOtherSkillDetail => {
            // Check if another alternative of the same skill has activated.
            logic_ctx.is_some_and(|logic| {
                runner.activated_alternatives[logic.skill_index] & !(1 << logic.alternative_index)
                    != 0
            }) as i32
        }

        // Times involved in a duel: 0 or 1; you can duel once at most.
        CompeteFightCount => runner.duel.has_dueled() as i32,

        // Skill activations by the runner's team (including the runner).
        ActivateCountAllTeam => state.team_activations[runner.initial_state.team as usize] as i32,
    }
}

/// Get running style values.
fn get_running_style_value(var: &RunningStyleVar, state: &RaceState, my_style: Style) -> i32 {
    use RunningStyleVar::*;
    let my_style = my_style.base_style();

    match var {
        // TODO: generate simulation values for one-runner simulations.
        // Running style: 1=Front, 2=Pace, 3=Late, 4=End.
        RunningStyle => my_style as i32,
        RunningStyleCountNigeOtherself => get_style_count_otherself(state, Style::Front, my_style),
        RunningStyleCountOikomiOtherself => get_style_count_otherself(state, Style::End, my_style),
        RunningStyleCountSame => get_style_count(state, my_style),
        RunningStyleCountSameRate => {
            (get_style_count(state, my_style) as f64 / state.params.gates as f64 * 100.0).floor()
                as i32
        }
        RunningStyleCountSashiOtherself => get_style_count_otherself(state, Style::Late, my_style),
        RunningStyleCountSenkoOtherself => get_style_count_otherself(state, Style::Pace, my_style),
        RunningStyleEqualPopularityOne => (my_style == state.style_most_popular) as i32,
    }
}

fn get_style_count(state: &RaceState, my_style: Style) -> i32 {
    state.runners_by_style.get(&my_style).cloned().unwrap_or(0) as i32
}

fn get_style_count_otherself(state: &RaceState, style: Style, my_style: Style) -> i32 {
    state.runners_by_style.get(&style).cloned().unwrap_or(0) as i32 - (my_style == style) as i32
}

fn get_style_temptation_count(state: &RaceState, style: Style) -> i32 {
    let rushed = state.rushed_by_team.iter().map(|team| team[style as usize]);
    rushed.map(i32::from).sum()
}

fn get_style_temptation_opponent_count(state: &RaceState, style: Style, runner: &Runner) -> i32 {
    let team = &state.rushed_by_team[runner.initial_state.team as usize];
    get_style_temptation_count(state, style) - team[style as usize] as i32
}

/// Get runner entry values.
fn get_entry_value(var: &EntryVar, state: &InitialState) -> i32 {
    use EntryVar::*;

    match var {
        // Number of fans.
        FanCount => state.fan_count as i32,

        // Mood: 1=awful, 2=bad, 3=normal, 4=good, 5=great.
        // N.B. `Mood` ranges from -2 (awful) to 2 (great).
        Motivation => state.mood as i32 + 3,

        // Popularity rank.
        Popularity => state.popularity_rank as i32,

        // Gate bracket: 1=inner-most, 8=outer-most.
        PostNumber => state.gate_block as i32,
    }
}

/// Get status values (HP, last spurt, continuous checks).
fn get_status_value(var: &StatusVar, runner: &Runner) -> i32 {
    use StatusVar::*;

    match var {
        // HP as percentage (0-100).
        HpPer => runner.hp.percent().floor() as i32,

        // HP is (or has been at some point) empty.
        IsHpEmptyOnetime => runner.hp.has_emptied as i32,

        // Is in last spurt mode (not the phase).
        IsLastspurt => runner.active_last_spurt().is_some() as i32,

        // Last spurt state:
        // 0=can't finish at base speed, 1=above base speed, 2=max speed.
        // -1 if not calculated yet, so that it doesn't match any state.
        Lastspurt => runner
            .last_spurt
            .as_ref()
            .map_or(-1, |spurt| spurt.category as i32),

        // Start is too late.
        IsBadstart => matches!(runner.start_type(), StartType::Late) as i32,
        // Start is good.
        // ? Assumed to be a Good Start; under investigation.
        IsGoodstart => matches!(runner.start_type(), StartType::Good) as i32,

        // Lane movement.
        IsMoveLane => {
            if runner.lane.speed < 0.0 {
                1 // Moving inward.
            } else if runner.lane.speed > 0.0 {
                2 // Moving outward.
            } else {
                0 // Not moving.
            }
        }

        // Overtake mode.
        IsOvertake => runner.is_in_overtake_mode() as i32,

        // Inherited skills, including those that failed the wit check.
        SuccessionSkillCount => runner
            .available_skills
            .iter()
            .chain(&runner.failed_skills)
            .filter(|skill| skill.is_inherited_unique())
            .count() as i32,

        // Temptation/rush check.
        IsTemptation => runner.is_rushed() as i32,
        // Times rushed: 0 or 1; you can rush once at most.
        TemptationCount => runner.has_rushed() as i32,
    }
}

/// Get rank-related values.
fn get_rank_value(var: &OrderVar, runner: &Runner) -> i32 {
    use OrderVar::*;
    let order = &runner.order;
    let at_rate = |rate: i32| order.order_at_rate(rate);

    match var {
        // Order.
        Order => order.order as i32,
        OrderRate => panic!("BUG: {var} should have been rewritten"),

        // Order change checks.
        ChangeOrderOnetime => order.delta as i32,
        ChangeOrderUpEndAfter => order.overtakes_since_late as i32,
        ChangeOrderUpFinalcornerAfter => order.overtakes_since_last_corner as i32,
        ChangeOrderUpMiddle => order.overtakes_since_mid as i32,

        // Order rate continue checks: whether `order_rate<=X` (in) or
        // `order_rate>=X` (out) held for the entire race, i.e., whether the worst
        // order was always within the top X%, or the best order always at the
        // rounded order or worse. The rounded order counts as both, e.g., always
        // being 2nd in a 9-runner race (9 * 20% = 1.8 => 2) satisfies `in20` and
        // `out20`.
        OrderRateIn20Continue => (order.worst as i32 <= at_rate(20)) as i32,
        OrderRateIn40Continue => (order.worst as i32 <= at_rate(40)) as i32,
        OrderRateIn50Continue => (order.worst as i32 <= at_rate(50)) as i32,
        OrderRateIn80Continue => (order.worst as i32 <= at_rate(80)) as i32,
        OrderRateOut20Continue => (order.best as i32 >= at_rate(20)) as i32,
        OrderRateOut40Continue => (order.best as i32 >= at_rate(40)) as i32,
        OrderRateOut50Continue => (order.best as i32 >= at_rate(50)) as i32,
        OrderRateOut70Continue => (order.best as i32 >= at_rate(70)) as i32,
    }
}

/// Get blocking values.
fn get_blocking_value(var: &BlockingVar, runner: &Runner) -> i32 {
    use BlockingVar::*;

    match var {
        // Blocking checks.
        BlockedFront => runner.is_front_blocked() as i32,
        BlockedFrontContinuetime => runner.blocked.front_time as i32,
        BlockedSideContinuetime => runner.blocked.side_time as i32,
        BlockedAllContinuetime => runner.blocked.front_time.min(runner.blocked.side_time) as i32,
        IsSurrounded => runner.is_surrounded() as i32,

        // Near counts.
        NearCount => runner.nearby.count as i32,
        NearInfrontCount => runner.nearby.ahead_same_lane_count as i32,
        InfrontNearLaneTime => runner.nearby.ahead_same_lane_time.floor() as i32,
        BehindNearLaneTime => runner.nearby.behind_same_lane_time.floor() as i32,
        BehindNearLaneTimeSet1 => runner.nearby.behind_set1_time.floor() as i32,
        IsBehindIn => runner.nearby.behind_in as i32,

        // Target overtake times.
        OvertakeTargetNoOrderUpTime => runner.overtake_target_time as i32,
        OvertakeTargetTime => runner.being_overtake_target_time as i32,

        // Visible runner count.
        Visiblehorse => runner.visible_count as i32,
    }
}

/// Get temptation/rush values of other umas.
fn get_temptation_value(var: &TemptationVar, state: &RaceState, runner: &Runner) -> i32 {
    use TemptationVar::*;

    match var {
        // Rush counts by position.
        TemptationCountBehind => runner.rush_counter.behind as i32,
        TemptationCountInfront => runner.rush_counter.ahead as i32,
        TemptationOpponentCountBehind => runner.rush_counter.opponents_behind as i32,
        TemptationOpponentCountInfront => runner.rush_counter.opponents_ahead as i32,

        // Rush counts by running style.
        RunningStyleTemptationCountNige => get_style_temptation_count(state, Style::Front),
        RunningStyleTemptationCountOikomi => get_style_temptation_count(state, Style::End),
        RunningStyleTemptationCountSashi => get_style_temptation_count(state, Style::Late),
        RunningStyleTemptationCountSenko => get_style_temptation_count(state, Style::Pace),
        RunningStyleTemptationOpponentCountNige => {
            get_style_temptation_opponent_count(state, Style::Front, runner)
        }
        RunningStyleTemptationOpponentCountOikomi => {
            get_style_temptation_opponent_count(state, Style::End, runner)
        }
        RunningStyleTemptationOpponentCountSashi => {
            get_style_temptation_opponent_count(state, Style::Late, runner)
        }
        RunningStyleTemptationOpponentCountSenko => {
            get_style_temptation_opponent_count(state, Style::Pace, runner)
        }
    }
}

/// Check popularity guards in a condition.
///
/// Returns true if all popularity guards pass in at least one OR branch (see
/// [`VarType::is_popularity_guard`]). Comparisons on any other variable are
/// assumed to be satisfiable.
pub fn check_popularity_guards(
    condition: &Condition,
    state: &RaceState,
    runner: &UnreadyRunner,
) -> bool {
    condition.branches.iter().any(|branch| {
        branch.iter().all(|comp| {
            !comp.var.is_popularity_guard()
                || EvaluatorOutOfGate::evaluate(&EvaluatorOutOfGate, comp, None, state, runner)
        })
    })
}

#[cfg(test)]
mod tests;
