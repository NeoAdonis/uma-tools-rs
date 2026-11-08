//! # Skill Activation Logic
//!
//! This module compiles skill conditions into **triggers** (which determine
//! WHEN to check a skill) and **guards** (which must be true AT activation
//! time).
//!
//! ## Overview
//!
//! A condition is an OR of AND chains (branches). Each branch is compiled into
//! a single trigger and a list of guards:
//!
//! - **Guards**: every comparison of the branch, except the ones the evaluator
//!   can't compute (random variables and `is_last_straight_onetime`). The
//!   guards alone decide whether the skill activates.
//! - **Trigger**: decides when to evaluate the guards. It never replaces a
//!   guard, so it only needs to fire *at least* whenever all guards could be
//!   true:
//!   - Events: random segments, entering the last straight, skill activations.
//!   - Windows: the range of positions (or time, or HP) where every comparison
//!     of the branch can be true, checked every frame while inside it.
//!
//! ## Compiling a Branch
//!
//! The trigger of a branch is chosen as follows:
//!
//! 1. **Event trigger**, if the branch contains an event (at most one per
//!    branch: skills with more are rejected when loaded):
//!    - A random variable: its rolled segments, or its whole area for skills
//!      whose activations are pinned ([`TriggerCondition::Segments`]), trimmed
//!      to the position range of the rest of the branch (see
//!      [`random`](crate::trigger::random)), or the all-out spurt start
//!      ([`TriggerCondition::AllOutSpurtStart`]) for
//!      `run_at_full_speed_random`. `random_lot` is only a roll: it adds no
//!      trigger.
//!    - `is_last_straight_onetime==1`: entering the last straight.
//!    - `is_activate_any_skill==1` / `is_activate_heal_skill==1`: a (heal)
//!      skill activating. These stay as guards too, since they can be
//!      evaluated.
//!    - `is_other_character_activate_advantage_skill==N`: another runner
//!      activating a buff with effect type N in the current frame. It stays as
//!      a guard too. Likewise for
//!      `is_popularity_top_character_activate_advantage_skill==N`, but only for
//!      the most popular runner, and N can be -1 for any skill.
//! 2. **Window trigger** otherwise. The ranges of the branch's comparisons
//!    are intersected per variable, and the window uses the first kind of
//!    range that the branch has, in this order:
//!    1. **Position**: distance rate, remaining distance, furlongs, corners,
//!       slopes, straights, final corner, last straight.
//!    2. **Time**: `accumulatetime`.
//!    3. **HP**: `hp_per`.
//!
//!    The phase is a variable of its own kind, not a position: it's updated
//!    from the position before the last move, a frame behind the one position
//!    windows are checked with. No window is based on it, so its comparisons
//!    (subdivisions included) are only checked as guards.
//!
//!    A branch with no range at all gets a position window over the whole
//!    race.
//!
//! Ranges must contain every value where the comparison can be true, so:
//!
//! - Values the evaluator floors hold up to the next integer
//!   (`distance_rate<=50` holds until 51%).
//! - Features with gaps use the smallest range containing all of them
//!   (`corner!=0` spans from the first corner's start to the last corner's
//!   end); the guard handles the gaps.
//! - A variable whose range isn't known simply doesn't narrow the window.
//!
//! If any range is empty (e.g. `slope==2` on a course without downhills, or
//! `distance_rate<=20&is_finalcorner==1` when the final corner starts later),
//! the branch can never be true and is dropped.
//!
//! ## Preconditions
//!
//! A precondition must have been true **at some point** before the skill can
//! activate; once satisfied, it stays satisfied for the rest of the race.
//! Example: "after overtaking someone (`change_order_onetime<0`), activate on
//! the last straight".
//!
//! A precondition is compiled into its own logics (one per OR branch) whose
//! action is [`LogicAction::Unlock`] instead of [`LogicAction::Activate`].
//! Their guards are the branch's comparisons; their trigger decides when those
//! are checked. All condition branches of the alternative wait on the same
//! precondition, and can succeed once any of its logics does. This reuses the
//! trigger-guard machinery: evaluating a precondition is the same as evaluating
//! guards.
//!
//! Preconditions are compiled like conditions, except that:
//!
//! - Their position windows go to a separate queue, checked before the
//!   triggers they unlock (see [`Runner::check_skills`]). That queue only
//!   tracks positions, so a precondition window based on time or HP covers the
//!   whole race instead.
//! - Unlock logics are added before the logics they unlock, so an event firing
//!   both unlocks the skill first.
//! - They aren't rolled: skills with random variables in their preconditions
//!   are rejected when loaded.
//!
//! ## Examples
//!
//! Condition `hp_per>=20 & is_finalcorner_random==1`:
//!
//! - **Trigger**: `Segments { ranges: [(1850, 1860)] }` (random 10m segment
//!   rolled on the final corner).
//! - **Guards**: `[hp_per>=20]`.
//!
//! Condition `phase==1 & corner!=0 & order<=3` (on a course with corners at
//! 300-500m and 900-1100m):
//!
//! - **Trigger**: `Window { Position, 300, 1100 }` (corners only).
//! - **Guards**: `[phase==1, corner!=0, order<=3]`.
//!
//! [`Runner::check_skills`]: crate::race::runner::Runner::check_skills

use crate::condition::parser::*;
use crate::condition::variables::*;
use crate::course::geometry::Segment;
use crate::course::*;
use crate::race::solver::RunnerId;
use crate::skill::{AdditionalActivation, SkillEffect};
use crate::units::furlongs_to_meters;

/// Complete activation logic for a skill.
///
/// Combines a trigger (WHEN to check), guards (IF to activate), and a
/// precondition (unlock gate) in a single structure.
#[derive(Clone, Debug, PartialEq)]
pub struct SkillActivationLogic {
    /// Index into runner.skills.
    pub skill_index: usize,

    /// Which alternative of this skill (skills can have multiple condition sets).
    pub alternative_index: usize,

    /// Trigger that determines WHEN to check this logic.
    pub(crate) trigger: TriggerCondition,

    /// Guards that must ALL be true AT activation time.
    ///
    /// An empty list means no guard checks needed (always activates when trigger
    /// fires).
    pub(crate) guards: ComparisonList,

    /// Precondition to satisfy before this logic can succeed, as an index into
    /// `scheduler.precondition_satisfied`.
    ///
    /// Enables event-based activation (e.g., "activate after overtaking someone").
    pub(crate) precondition: Option<usize>,

    /// What happens when the trigger fires and the guards pass.
    pub(crate) action: LogicAction,

    /// Whether the alternative is a continuation of another one (see
    /// [`crate::skill::SkillAlternative::is_continuation`]).
    pub(crate) is_continuation: bool,
}

/// What a [`SkillActivationLogic`] does when it succeeds.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum LogicAction {
    /// Activate the skill alternative.
    Activate,
    /// Mark the given precondition as satisfied.
    Unlock(usize),
    /// Check the conditions of a skill whose activations are pinned, without
    /// activating it (see [`SkillActivationRecord::conditions_met`]).
    ///
    /// `reports_missing` tells whether a missing activation (the skill's
    /// conditions holding with no pinned activation) is recorded (see
    /// [`Runner::missing_activations`]): only if a roll couldn't have
    /// prevented it, i.e. a failed wit check is ruled out (the skill has a
    /// pinned activation, or no wit check) and the branch has no random
    /// variable.
    ///
    /// [`Runner::missing_activations`]:
    ///     crate::race::runner::Runner::missing_activations
    Check { reports_missing: bool },
}

/// Variables that window triggers can be based on.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum WindowVariable {
    /// Position (meters from start).
    Position,
    /// Time elapsed since race start (seconds).
    Time,
    /// HP percentage (0-100).
    HpPercent,
}

/// A trigger condition that determines WHEN to check a skill.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum TriggerCondition {
    /// Checked every frame while the position is within any of the `(min, max)`
    /// ranges (in meters). Used for random segments, trimmed to the positions where
    /// the rest of the branch can be true.
    Segments { ranges: Vec<(f64, f64)> },

    /// Checked every frame while the variable is within `[min, max]`.
    Window {
        variable: WindowVariable,
        min_threshold: f64,
        max_threshold: f64,
    },

    /// Checked once, when entering the last straight (`is_last_straight_onetime`).
    LastStraightEntry,

    /// Checked once, on the first frame the runner moves (i.e. leaves the gate).
    /// Used for out-of-gate skills, whose condition is evaluated before the race.
    RaceStart,

    /// Random point after the all-out spurt starts. The point is rolled when the
    /// sprint starts and scheduled as a position trigger.
    /// Used for `run_at_full_speed_random==1` conditions.
    AllOutSpurtStart,

    /// Checked when any skill activates.
    /// Used for `is_activate_any_skill==1` conditions.
    AnySkillActivation,

    /// Checked once every runner has checked her skills, when any runner activated
    /// an advantage (buff) skill, or the most popular runner activated any skill.
    /// Used for `is_other_character_activate_advantage_skill` and
    /// `is_popularity_top_character_activate_advantage_skill` conditions.
    BuffSkillActivation,

    /// Checked when a heal skill activates.
    /// Used for `is_activate_heal_skill==1` conditions.
    HealSkillActivation,
}

/// A timed skill effect that is currently active.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ActiveTimedEffect {
    /// Skill effect being applied.
    pub(crate) effect: SkillEffect,
    /// Final modifier value, including any activation-time bonus such as the wit
    /// limit break buff.
    pub(crate) value: f64,
    /// Race time in seconds when effect expires.
    pub(crate) expires_at: f64,
    /// Extension of the effect when overtaking while it's active.
    pub(crate) overtake_extension: OvertakeExtension,
    /// Number of times an additional effect has been triggered (see
    /// [`AdditionalActivation`]). Unused for regular effects.
    pub(crate) additional_triggers: u8,
    /// Index of the runner's own skill that applied the effect, or `None` if it
    /// was cast by another runner.
    pub(crate) source: Option<usize>,
}

impl ActiveTimedEffect {
    /// Value currently applied by the effect. An additional effect applies its
    /// value once per trigger, so nothing until triggered.
    pub(crate) fn current_value(&self) -> f64 {
        if self.effect.additional == AdditionalActivation::Regular {
            self.value
        } else {
            self.value * self.additional_triggers as f64
        }
    }
}

/// Extension of a timed effect each time the runner overtakes while it's active
/// (see [`DurationScaling::IncrementOrderUp`][increment]).
///
/// The default doesn't extend the effect.
///
/// [increment]: crate::skill::DurationScaling::IncrementOrderUp
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub(crate) struct OvertakeExtension {
    /// Duration in seconds added for each overtake, already scaled.
    pub(crate) duration: f64,
    /// Number of extensions left.
    pub(crate) remaining: u8,
}

/// A skill effect that targets other runners, waiting to be applied to them.
///
/// Values that depend on the caster (duration, wit limit break bonus) are fixed
/// at activation time.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct PendingEffect {
    /// Skill effect to apply to each target.
    pub(crate) effect: SkillEffect,
    /// Duration in seconds, already scaled.
    pub(crate) duration: f64,
    /// Wit limit break bonus of the caster.
    pub(crate) wit_bonus: f64,
    /// Draw (from 0 to 1) to pick random targets, from the stream of the
    /// caster's skill.
    pub(crate) target_roll: f64,
    /// Pinned targets, instead of the usual ones (see
    /// [`PinnedActivation::targets`]).
    ///
    /// [`PinnedActivation::targets`]: crate::race::pins::PinnedActivation::targets
    pub(crate) targets: Option<Vec<RunnerId>>,
    /// Index in the caster's `activated_skills` of the activation's record,
    /// if the runners reached are recorded there (see
    /// [`SkillActivationRecord::affected`]).
    pub(crate) record: Option<usize>,
}

/// Pinned activation of a skill alternative during the race, waiting for its
/// time (see [`SkillPin::Activations`]).
///
/// [`SkillPin::Activations`]: crate::race::pins::SkillPin::Activations
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct QueuedActivation {
    /// Time since the race started, in seconds.
    pub(crate) time: f64,
    /// Index of the skill in the runner's skills.
    pub(crate) skill_index: usize,
    /// Index of the alternative that activates.
    pub(crate) alternative_index: usize,
    /// Pinned targets of her effects on others.
    pub(crate) targets: Option<Vec<RunnerId>>,
}

/// Record of a skill activation during the race.
#[derive(Debug, Clone, PartialEq)]
pub struct SkillActivationRecord {
    /// Skill ID of the activated skill from the global skill list.
    pub skill_id: i32,
    /// Index of the alternative that activated.
    pub alternative: usize,
    /// Time since the race started when the skill was activated, at the end of
    /// the frame whose move its effects first apply to (in seconds).
    /// Out-of-gate alternatives are recorded at 0, though they activate on her
    /// first frame of movement, the first one their effects during the race
    /// apply to.
    pub time: f64,
    /// Distance when the skill was activated (in meters from start).
    pub distance: f64,
    /// Duration of its effects in seconds: the base duration scaled by the
    /// course distance (× distance / 1000), her situation (see
    /// [`DurationScaling`]) and her duration effects. It's infinite for effects
    /// lasting until the end of the race, and 0 for instant skills.
    ///
    /// [`DurationScaling`]: crate::skill::DurationScaling
    pub duration: f64,
    /// Runners its effects reached: herself if any of them targets her,
    /// otherwise the runners her effects on others reached, once the solver
    /// picks them after every runner has checked her skills. Missing
    /// activations (see [`Runner::missing_activations`]) whose effects only
    /// reach others have none, as nobody is picked for them.
    ///
    /// [`Runner::missing_activations`]:
    ///     crate::race::runner::Runner::missing_activations
    pub affected: Vec<RunnerId>,
    /// Whether the skill's conditions allowed the activation, always true
    /// unless its activations are pinned (see [`SkillPin::Activations`]).
    /// Skills activated by another one (e.g.
    /// [`SkillType::ActivateRandomGold`]) skip their conditions, which still
    /// counts as allowed.
    ///
    /// A pinned activation is allowed if the skill's conditions held for its
    /// alternative on its frame, ignoring the rolls: the wit check, random
    /// lots, and the segment of random variables, which can be anywhere in
    /// their area (e.g. the whole phase for `phase_random`). Cooldowns still
    /// apply, started by the activations the conditions allowed or would have
    /// had.
    ///
    /// [`SkillPin::Activations`]: crate::race::pins::SkillPin::Activations
    /// [`SkillType::ActivateRandomGold`]:
    ///     crate::skill::SkillType::ActivateRandomGold
    pub conditions_met: bool,
}

/// One OR branch of a compiled condition.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct CompiledBranch {
    /// When to check the branch.
    pub(crate) trigger: TriggerCondition,
    /// Comparisons that must all be true when checked.
    pub(crate) guards: ComparisonList,
    /// Whether the branch has a random variable (random lots included).
    pub(crate) random: bool,
}

/// Compile a condition into one [`CompiledBranch`] per OR branch.
///
/// `segments` holds the intervals `(start, end)` the random variables of each
/// branch can be checked in: the segments they rolled (see
/// [`RandomRolls::roll`](crate::trigger::random::RandomRolls::roll)), or
/// their whole areas (see [`random_areas`]). `None` means a failed roll or an
/// empty area. Branches without an entry (e.g. preconditions, which aren't
/// rolled) have no random segments.
///
/// Branches that can never be true on this course (e.g. `slope==2` without
/// downhills, or a failed roll) are dropped, so the result is empty if the
/// condition can never be satisfied.
///
/// # Example
///
/// Input: `phase_random==1 & hp_per>=20 @ corner==2 & ground_type==1`
///
/// Output (2 branches):
///
/// - Branch 1: trigger `Segments` (rolled segment), guards `[hp_per>=20]`.
/// - Branch 2: trigger `Window` (corner 2), guards
///   `[corner==2, ground_type==1]`.
///
/// [`random_areas`]: crate::trigger::random::random_areas
pub(crate) fn compile_condition(
    condition: &Condition,
    segments: &[Option<Vec<Range>>],
    course: &Course,
) -> Vec<CompiledBranch> {
    condition
        .branches
        .iter()
        .enumerate()
        .filter_map(|(idx, comps)| {
            let segments = match segments.get(idx) {
                // Failed roll or empty area: never satisfied.
                Some(segments) => segments.as_deref()?,
                // Not rolled: random variables would be ignored. Skills with
                // them in their preconditions are rejected when loaded.
                None => {
                    debug_assert!(
                        !comps.iter().any(Comparison::is_random),
                        "random variables in a condition without rolls: {comps:?}"
                    );
                    &[]
                }
            };
            compile_branch(comps, segments, course)
        })
        .collect()
}

/// Compile an AND chain, with the `segments` its random variables can be
/// checked in. Returns `None` if it can never be true on this course.
fn compile_branch(
    comps: &[Comparison],
    segments: &[Range],
    course: &Course,
) -> Option<CompiledBranch> {
    use VarType::*;

    // The segments of a branch don't tell which comparison they come from,
    // so they can only belong to its single event. Skills with more events in a
    // branch are rejected when loaded.
    debug_assert!(
        comps.iter().filter(|comp| comp.is_event()).count() <= 1,
        "more than one event in a branch: {comps:?}"
    );

    let mut event = None;
    let mut guards = Vec::new();
    let mut position = FULL_RANGE;
    let mut time = FULL_RANGE;
    let mut hp = FULL_RANGE;

    for comp in comps {
        match &comp.var {
            // Rolled when building the scheduler.
            // If we're here, the roll succeeded.
            Random(_) => {}

            // Random segments: checked while inside the given segments.
            RandomSpatial(RandomSpatialVar::RunAtFullSpeedRandom) => {
                event = Some(TriggerCondition::AllOutSpurtStart);
            }
            RandomSpatial(_) | RandomPhase(_) => {
                // Filled once the position range of the branch is known. If there are no
                // segments, the branch is dropped below.
                event = Some(TriggerCondition::Segments { ranges: Vec::new() });
            }

            // Only true on the frame the last straight is entered.
            Spatial(SpatialVar::IsLastStraightOnetime) => {
                course.straights.last()?;
                event = Some(TriggerCondition::LastStraightEntry);
            }

            // Skill activations: only true on frames where a skill activated.
            // Unlike the events above, these can be evaluated, so they stay as guards.
            Activation(_) | ActivationQuery(_) => {
                if comp.is_event() {
                    event = Some(activation_event(comp));
                }
                guards.push(comp.clone());
            }

            _ => {
                position = intersect(position, position_range(comp, course)?);
                time = intersect(time, time_range(comp));
                hp = intersect(hp, hp_range(comp));
                guards.push(comp.clone());
            }
        }
    }

    if is_empty(position) || is_empty(time) || is_empty(hp) {
        // Some comparisons can never be true at the same time.
        return None;
    }

    // Segments trimmed to exclude always-false intervals.
    if let Some(TriggerCondition::Segments { ranges }) = &mut event {
        ranges.extend(
            segments
                .iter()
                .map(|&segment| intersect(segment, position))
                .filter(|&range| !is_empty(range)),
        );
        if ranges.is_empty() {
            // No segment where the rest of the branch can be true.
            return None;
        }
    }

    // Prefer position windows: they're the most common and cheapest to track.
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

    let random = comps.iter().any(Comparison::is_random);
    Some(CompiledBranch {
        trigger,
        guards,
        random,
    })
}

/// Trigger of an event on skill activations (see [`Comparison::is_event`]).
fn activation_event(comp: &Comparison) -> TriggerCondition {
    match comp.var {
        VarType::Activation(ActivationVar::IsActivateAnySkill) => {
            TriggerCondition::AnySkillActivation
        }
        VarType::Activation(ActivationVar::IsActivateHealSkill) => {
            TriggerCondition::HealSkillActivation
        }
        _ => TriggerCondition::BuffSkillActivation,
    }
}

/// Range of values `(min, max)` a variable can take (both inclusive).
///
/// It may contain values where the comparisons are false, but never misses one
/// where they're true, so a single point (`min == max`) isn't empty (e.g.
/// `remain_distance>=800&is_finalcorner==1` when the final corner starts 800m
/// before the goal).
type Range = (f64, f64);

/// Range containing every value.
const FULL_RANGE: Range = (0.0, f64::INFINITY);

/// Range containing no value.
const EMPTY_RANGE: Range = (f64::INFINITY, f64::NEG_INFINITY);

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
/// true. As values are floored, `variable<=N` holds until `N + 1`, and
/// `variable>N` from `N + 1`.
fn floored_range(op: CompOp, value: i32) -> Range {
    let value = value as f64;
    match op {
        CompOp::Greater => (value + 1.0, f64::INFINITY),
        CompOp::GreaterEqual => (value, f64::INFINITY),
        CompOp::Less => (f64::NEG_INFINITY, value),
        CompOp::LessEqual => (f64::NEG_INFINITY, value + 1.0),
        CompOp::Equal => (value, value + 1.0),
        CompOp::NotEqual => (f64::NEG_INFINITY, f64::INFINITY),
    }
}

/// Get the range of positions (in meters from the start) where a comparison can
/// be true.
///
/// The range contains every position where it's true, but may contain some
/// where it isn't (e.g. `corner!=0` covers from the first corner to the last
/// one).
///
/// Returns [`FULL_RANGE`] if the comparison doesn't depend on the position, and
/// `None` if it's never true on this course (e.g. `slope==2` without
/// downhills).
fn position_range(comp: &Comparison, course: &Course) -> Option<Range> {
    use VarType::*;

    let last_corner = course.corners.last();

    let range = match (&comp.var, comp.op, comp.value) {
        (Spatial(SpatialVar::DistanceRate), op, value) => {
            let (min, max) = floored_range(op, value);
            (course.position_at_rate(min), course.position_at_rate(max))
        }
        (Spatial(SpatialVar::RemainDistance), op, value) => {
            let (min, max) = floored_range(op, value);
            (course.length() - max, course.length() - min)
        }
        (Spatial(SpatialVar::Furlong), op, value) => {
            let (min, max) = floored_range(op, value);
            let position = |furlongs| course.run_up + furlongs_to_meters(furlongs);
            (position(min), position(max))
        }

        // Final corner (or its second half) and beyond.
        (Spatial(SpatialVar::IsFinalcorner), CompOp::Equal, 1) => {
            last_corner.map_or(EMPTY_RANGE, |corner| (corner.start, f64::INFINITY))
        }
        (Spatial(SpatialVar::IsFinalcornerLaterhalf), CompOp::Equal, 1) => last_corner
            .map_or(EMPTY_RANGE, |corner| {
                (corner.start + corner.length / 2.0, f64::INFINITY)
            }),
        // After the final corner (anywhere without corners).
        (Spatial(SpatialVar::IsLastStraight), CompOp::Equal, 1) => {
            last_corner.map_or(FULL_RANGE, |corner| (corner.end(), f64::INFINITY))
        }

        (Spatial(SpatialVar::Corner), CompOp::NotEqual, 0) => {
            envelope(course.corners.iter().map(Segment::range))
        }
        // Every occurrence of the corner (multiple if more than one lap).
        (Spatial(SpatialVar::Corner), CompOp::Equal, n) if n > 0 => envelope(
            course
                .corners
                .iter()
                .enumerate()
                .filter(|(idx, _)| course.corner_number(*idx) as i32 == n)
                .map(|(_, corner)| corner.range()),
        ),
        (Spatial(SpatialVar::Slope), CompOp::Equal, slope_type) if slope_type != 0 => envelope(
            course
                .slopes
                .iter()
                .filter(|slope| slope.slope_type() as i32 == slope_type)
                .map(Segment::range),
        ),
        (Spatial(SpatialVar::StraightFrontType), CompOp::Equal, front_type) if front_type != 0 => {
            envelope(
                course
                    .straights
                    .iter()
                    .filter(|straight| straight.front_type as i32 == front_type)
                    .map(Segment::range),
            )
        }

        _ => FULL_RANGE,
    };

    let range = intersect(range, FULL_RANGE);
    (!is_empty(range)).then_some(range)
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
