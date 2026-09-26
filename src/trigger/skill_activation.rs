//! # Skill Activation Logic
//!
//! This module compiles skill conditions into **triggers** (which determine WHEN to
//! check a skill) and **guards** (which must be true AT activation time).
//!
//! ## Overview
//!
//! A condition is an OR of AND chains (branches). Each branch is compiled into a
//! single trigger and a list of guards:
//!
//! - **Guards**: every comparison of the branch, except the ones the evaluator can't
//!   compute (random variables and `is_last_straight_onetime`). The guards alone
//!   decide whether the skill activates.
//! - **Trigger**: decides when to evaluate the guards. It never replaces a guard, so
//!   it only needs to fire *at least* whenever all guards could be true:
//!   - Events: random points, entering the last straight, skill activations.
//!   - Windows: the range of positions (or time, or HP) where every comparison of the
//!     branch can be true, checked every frame while inside it.
//!
//! **Preconditions** are compiled the same way into logics whose action is to unlock
//! the skill instead of activating it (see [`LogicAction`]).
//!
//! ## Examples
//!
//! Condition `hp_per>=20 & is_finalcorner_random==1`:
//!
//! - **Trigger**: `Position { meters: [1850] }` (random point rolled on the final
//!   corner).
//! - **Guards**: `[hp_per>=20]`.
//!
//! Condition `phase==1 & corner!=0 & order<=3` (on a course with corners at
//! 300-500m and 900-1100m, and Mid phase at 400-1600m):
//!
//! - **Trigger**: `Window { Position, 400, 1100 }` (Mid phase ∩ corners).
//! - **Guards**: `[phase==1, corner!=0, order<=3]`.

use crate::condition::parser::*;
use crate::condition::variables::*;
use crate::course::*;
use crate::skill::SkillEffect;
use crate::units::furlongs_to_meters;

/// Complete activation logic for a skill.
///
/// Combines a trigger (WHEN to check), guards (IF to activate), and a precondition
/// (unlock gate) in a single structure.
#[derive(Clone, Debug)]
pub struct SkillActivationLogic {
    /// Index into runner.available_skills.
    pub skill_index: usize,

    /// Which alternative of this skill (skills can have multiple condition sets).
    pub alternative_index: usize,

    /// Trigger that determines WHEN to check this logic.
    pub trigger: TriggerCondition,

    /// Guards that must ALL be true AT activation time.
    /// Empty list means no guard checks needed (always activates when trigger fires).
    pub guards: ComparisonList,

    /// Precondition that must be satisfied before this logic can succeed, as an
    /// index into `scheduler.precondition_satisfied`.
    /// Enables event-based activation (e.g., "activate after overtaking someone").
    pub precondition: Option<usize>,

    /// What happens when the trigger fires and the guards pass.
    pub action: LogicAction,

    /// Whether the logic already succeeded. It's never checked again afterwards.
    pub has_activated: bool,
}

/// What a [`SkillActivationLogic`] does when it succeeds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LogicAction {
    /// Activate the skill alternative.
    Activate,
    /// Mark the given precondition as satisfied.
    Unlock(usize),
}

/// Variables that window triggers can be based on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WindowVariable {
    /// Position (meters from start).
    Position,
    /// Time elapsed since race start (seconds).
    Time,
    /// HP percentage (0-100).
    HpPercent,
}

/// A trigger condition that determines WHEN to check a skill.
#[derive(Clone, Debug)]
pub enum TriggerCondition {
    /// Checked once when reaching each position (in meters).
    /// Used for random points.
    Position { meters: Vec<f64> },

    /// Checked every frame while the variable is within `[min, max]`.
    Window {
        variable: WindowVariable,
        min_threshold: f64,
        max_threshold: f64,
    },

    /// Checked once, when entering the last straight
    /// (`is_last_straight_onetime`).
    LastStraightEntry,

    /// Random point after Full-Throttle Sprint starts. The point is rolled when
    /// the sprint starts and scheduled as a position trigger.
    /// Used for `run_at_full_speed_random==1` conditions.
    FullThrottleStart,

    /// Checked when any skill activates.
    /// Used for `is_activate_any_skill==1` conditions.
    AnySkillActivation,

    /// Checked when a buff skill activates.
    /// Used for `is_other_character_activate_advantage_skill` conditions.
    BuffSkillActivation,

    /// Checked when a heal skill activates.
    /// Used for `is_activate_heal_skill==1` conditions.
    HealSkillActivation,
}

/// A timed skill effect that is currently active.
#[derive(Debug, Clone)]
pub struct ActiveTimedEffect {
    /// Skill effect being applied.
    pub effect: SkillEffect,
    /// Final modifier value (already divided by 10000 and including any
    /// activation-time bonus such as the Wiz Limit Break buff).
    pub value: f64,
    /// Race time in seconds when effect expires.
    pub expires_at: f64,
}

/// Record of a skill activation during the race.
#[derive(Debug, Clone)]
pub struct SkillActivationRecord {
    /// Skill ID of the activated skill from the global skill list.
    pub skill_id: String,
    /// Distance when the skill was activated (in meters from start).
    pub distance: f64,
}

impl SkillActivationLogic {
    /// Create a new skill activation logic.
    pub fn new(
        skill_index: usize,
        alternative_index: usize,
        trigger: TriggerCondition,
        guards: ComparisonList,
        precondition: Option<usize>,
        action: LogicAction,
    ) -> Self {
        SkillActivationLogic {
            skill_index,
            alternative_index,
            trigger,
            guards,
            precondition,
            action,
            has_activated: false,
        }
    }
}

/// One OR branch of a compiled condition.
#[derive(Clone, Debug)]
pub struct CompiledBranch {
    /// When to check the branch.
    pub trigger: TriggerCondition,
    /// Comparisons that must all be true when checked.
    pub guards: ComparisonList,
}

/// Compile a condition into one [`CompiledBranch`] per OR branch.
///
/// `rolls` holds the points rolled for the random variables of each branch, or
/// `None` if its roll failed (see
/// [`roll_condition`](crate::trigger::random::roll_condition)). Branches without an
/// entry (e.g. preconditions, which aren't rolled) have no random points.
///
/// Branches that can never be true on this course (e.g. `slope==2` without
/// downhills, or a failed roll) are dropped, so the result is empty if the condition
/// can never be satisfied.
///
/// # Example
///
/// Input: `phase_random==1 & hp_per>=20 @ corner==2 & ground_type==1`
///
/// Output (2 branches):
///
/// - Branch 1: trigger `Position` (rolled point), guards `[hp_per>=20]`
/// - Branch 2: trigger `Window` (corner 2), guards `[corner==2, ground_type==1]`
pub fn compile_condition(
    condition: &Condition,
    rolls: &[Option<Vec<f64>>],
    course: &Course,
) -> Vec<CompiledBranch> {
    condition
        .comp_tree
        .or_branches()
        .into_iter()
        .enumerate()
        .filter_map(|(idx, comps)| {
            let points = match rolls.get(idx) {
                Some(roll) => roll.as_deref()?, // Failed roll: never satisfied
                None => &[],
            };
            compile_branch(&comps, points, course)
        })
        .collect()
}

/// Compile an AND chain, with the `points` rolled for its random variables.
/// Returns `None` if it can never be true on this course.
fn compile_branch(
    comps: &[&Comparison],
    points: &[f64],
    course: &Course,
) -> Option<CompiledBranch> {
    use VarType::*;

    let mut event = None;
    let mut guards = Vec::new();
    let mut position = FULL_RANGE;
    let mut time = FULL_RANGE;
    let mut hp = FULL_RANGE;

    for comp in comps {
        match &comp.var {
            // Rolled when building the scheduler: if we're here, the roll succeeded
            Random(_) => {}

            // Random points: checked once at the rolled point. A branch holds at
            // most one event (checked against the skill data)
            RandomSpatial(RandomSpatialVar::RunAtFullSpeedRandom) => {
                event.get_or_insert(TriggerCondition::FullThrottleStart);
            }
            RandomSpatial(_) | RandomPhase(_) => {
                // Not rolled (e.g. in a precondition): the branch can't be triggered
                if points.is_empty() {
                    return None;
                }
                event.get_or_insert(TriggerCondition::Position {
                    meters: points.to_vec(),
                });
            }

            // Only true on the frame the last straight is entered
            Spatial(SpatialVar::IsLastStraightOnetime) => {
                course.straights.last()?;
                event.get_or_insert(TriggerCondition::LastStraightEntry);
            }

            // Skill activations: only true on frames where a skill activated.
            // Unlike the events above, these can be evaluated, so they stay as guards
            Activation(activation) => {
                let activation_event = match (activation, comp.op, comp.value) {
                    (ActivationVar::IsActivateAnySkill, CompOp::Equal, 1) => {
                        Some(TriggerCondition::AnySkillActivation)
                    }
                    (ActivationVar::IsActivateHealSkill, CompOp::Equal, 1) => {
                        Some(TriggerCondition::HealSkillActivation)
                    }
                    _ => None,
                };
                if let Some(activation_event) = activation_event {
                    event.get_or_insert(activation_event);
                }
                guards.push((*comp).clone());
            }

            _ => {
                position = intersect(position, position_range(comp, course)?);
                time = intersect(time, time_range(comp));
                hp = intersect(hp, hp_range(comp));
                guards.push((*comp).clone());
            }
        }
    }

    if is_empty(position) || is_empty(time) || is_empty(hp) {
        return None; // Some comparisons can never be true at the same time
    }

    // Prefer position windows: they're the most common and cheapest to track
    let window = |variable, (min_threshold, max_threshold)| TriggerCondition::Window {
        variable,
        min_threshold,
        max_threshold,
    };
    let trigger = event.unwrap_or_else(|| {
        if position != FULL_RANGE || (time == FULL_RANGE && hp == FULL_RANGE) {
            window(WindowVariable::Position, position)
        } else if time != FULL_RANGE {
            window(WindowVariable::Time, time)
        } else {
            window(WindowVariable::HpPercent, hp)
        }
    });

    Some(CompiledBranch { trigger, guards })
}

/// Range of values `(min, max)` a variable can take (both inclusive).
type Range = (f64, f64);

/// Range containing every value.
const FULL_RANGE: Range = (0.0, f64::INFINITY);

/// Range containing no value.
const EMPTY_RANGE: Range = (f64::INFINITY, f64::NEG_INFINITY);

/// All race phases, in order.
const PHASES: [Phase; 4] = [Phase::Early, Phase::Mid, Phase::Late, Phase::Last];

fn intersect(a: Range, b: Range) -> Range {
    (a.0.max(b.0), a.1.min(b.1))
}

fn is_empty(range: Range) -> bool {
    range.0 > range.1
}

/// Smallest range containing all given ranges.
fn envelope(ranges: impl Iterator<Item = Range>) -> Range {
    ranges.fold(EMPTY_RANGE, |(min, max), (start, end)| {
        (min.min(start), max.max(end))
    })
}

/// Range of values of a variable, floored when evaluated, where a comparison is
/// true. As values are floored, `variable<=N` holds until `N + 1`.
fn floored_range(op: CompOp, value: i32) -> Range {
    let value = value as f64;
    match op {
        CompOp::Greater | CompOp::GreaterEqual => (value, f64::INFINITY),
        CompOp::Less | CompOp::LessEqual => (f64::NEG_INFINITY, value + 1.0),
        CompOp::Equal => (value, value + 1.0),
        CompOp::NotEqual => (f64::NEG_INFINITY, f64::INFINITY),
    }
}

/// Get the range of positions (in meters from the start) where a comparison can be
/// true. The range contains every position where it's true, but may contain some
/// where it isn't (e.g. `corner!=0` covers from the first corner to the last one).
///
/// Returns [`FULL_RANGE`] if the comparison doesn't depend on the position, and
/// `None` if it's never true on this course (e.g. `slope==2` without downhills).
fn position_range(comp: &Comparison, course: &Course) -> Option<Range> {
    use VarType::*;

    let distance = course.distance as f64;
    let last_corner = course.corners.last();
    let corner_range =
        |corner: &crate::course::geometry::Corner| (corner.start, corner.start + corner.length);
    let subdivision = |boundaries| subdivision_range(comp.value, course, boundaries);

    let range = match (&comp.var, comp.op, comp.value) {
        (Spatial(SpatialVar::DistanceRate), op, value) => {
            let (min, max) = floored_range(op, value);
            (min / 100.0 * distance, max / 100.0 * distance)
        }
        (Spatial(SpatialVar::RemainDistance), op, value) => {
            let (min, max) = floored_range(op, value);
            (distance - max, distance - min)
        }
        (Spatial(SpatialVar::Furlong), op, value) => {
            let (min, max) = floored_range(op, value);
            (furlongs_to_meters(min), furlongs_to_meters(max))
        }

        // Union of the phases that satisfy the comparison
        (Phase(PhaseVar::Phase), op, value) => envelope(
            PHASES
                .iter()
                .filter(|phase| op.apply(**phase as i32, value))
                .map(|phase| phase.boundaries(course)),
        ),
        (Phase(PhaseVar::PhaseFirstquarter), CompOp::Equal, _) => {
            subdivision(crate::course::Phase::boundaries_first_quarter)
        }
        (Phase(PhaseVar::PhaseFirsthalf), CompOp::Equal, _) => {
            subdivision(crate::course::Phase::boundaries_first_half)
        }
        (Phase(PhaseVar::PhaseLaterhalf), CompOp::Equal, _) => {
            subdivision(crate::course::Phase::boundaries_second_half)
        }

        // Final corner (or its second half) and beyond
        (Spatial(SpatialVar::IsFinalcorner), CompOp::Equal, 1) => {
            last_corner.map_or(EMPTY_RANGE, |corner| (corner.start, f64::INFINITY))
        }
        (Spatial(SpatialVar::IsFinalcornerLaterhalf), CompOp::Equal, 1) => last_corner
            .map_or(EMPTY_RANGE, |corner| {
                (corner.start + corner.length / 2.0, f64::INFINITY)
            }),
        // After the final corner (anywhere without corners)
        (Spatial(SpatialVar::IsLastStraight), CompOp::Equal, 1) => last_corner
            .map_or(FULL_RANGE, |corner| {
                (corner.start + corner.length, f64::INFINITY)
            }),

        (Spatial(SpatialVar::Corner), CompOp::NotEqual, 0) => {
            envelope(course.corners.iter().map(corner_range))
        }
        // Every occurrence of the corner (multiple if the race has more than one lap)
        (Spatial(SpatialVar::Corner), CompOp::Equal, n) if n > 0 => envelope(
            course
                .corners
                .iter()
                .enumerate()
                .filter(|(idx, _)| course.corner_number(*idx) as i32 == n)
                .map(|(_, corner)| corner_range(corner)),
        ),
        (Spatial(SpatialVar::Slope), CompOp::Equal, slope_type) if slope_type != 0 => envelope(
            course
                .slopes
                .iter()
                .filter(|slope| slope.slope_type() as i32 == slope_type)
                .map(|slope| (slope.start, slope.start + slope.length)),
        ),
        (Spatial(SpatialVar::StraightFrontType), CompOp::Equal, front_type) if front_type != 0 => {
            envelope(
                course
                    .straights
                    .iter()
                    .filter(|straight| straight.front_type as i32 == front_type)
                    .map(|straight| (straight.start, straight.end)),
            )
        }

        _ => FULL_RANGE,
    };

    let range = intersect(range, FULL_RANGE);
    (!is_empty(range)).then_some(range)
}

/// Boundaries of a phase subdivision (e.g. [`Phase::boundaries_first_half`]).
type PhaseBoundaries = fn(&Phase, &Course) -> Range;

/// Range of a phase subdivision (`phase_firsthalf==N`, ...) for phase `value`.
fn subdivision_range(value: i32, course: &Course, boundaries: PhaseBoundaries) -> Range {
    PHASES
        .iter()
        .find(|phase| **phase as i32 == value)
        .map_or(EMPTY_RANGE, |phase| boundaries(phase, course))
}

/// Get the range of elapsed times (in seconds) where a comparison can be true.
fn time_range(comp: &Comparison) -> Range {
    match comp.var {
        VarType::AccumulateTime => intersect(floored_range(comp.op, comp.value), FULL_RANGE),
        _ => FULL_RANGE,
    }
}

/// Get the range of HP percentages where a comparison can be true.
fn hp_range(comp: &Comparison) -> Range {
    match comp.var {
        VarType::Status(StatusVar::HpPer) => {
            intersect(floored_range(comp.op, comp.value), FULL_RANGE)
        }
        _ => FULL_RANGE,
    }
}

#[cfg(test)]
mod tests;
