//! # Trigger Builder
//!
//! This module builds the [`TriggerScheduler`] of a runner: it compiles skill
//! conditions into triggers and guards (see `skill_activation.rs`), and indexes
//! the triggers so they can be checked efficiently.
//!
//! ## Architecture
//!
//! For each skill alternative:
//!
//! 1. **Pre-roll random triggers** - Determine random activation segments,
//!    shared by every alternative of the skill.
//! 2. **Compile condition** - Turn each OR branch into a trigger + guards using
//!    [`compile_condition()`].
//! 3. **Compile precondition** - Turn the precondition (if present) into logics
//!    that unlock the skill, the same way.
//! 4. **Create [`SkillActivationLogic`]** - Store trigger + guards +
//!    precondition ID.
//! 5. **Index triggers** - Add to appropriate scheduler queues/lists.
//!
//! Alternatives whose condition or precondition can never be satisfied on the
//! course are skipped.
//!
//! The RNG is consumed in skill order while building, so changing what gets
//! rolled (or in which order) changes race results for a given seed.
//!
//! ## Example
//!
//! **Input skill**:
//!
//! - **Condition**: `hp_per>=20 & is_finalcorner_random==1 & ground_type==2`
//! - **Precondition**: `change_order_onetime<0`
//!
//! **After compiling**:
//!
//! ```rust,ignore
//! SkillActivationLogic {
//!   // change_order_onetime<0 can be true anywhere
//!   trigger: Window { Position, 0, inf },
//!   guards: [change_order_onetime<0],
//!   precondition: None,
//!   action: Unlock(0),
//!   /* ... */
//! }
//!
//! SkillActivationLogic {
//!   // random segment in final corner
//!   trigger: Segments { ranges: [(1850, 1860)] },
//!   guards: [hp_per>=20, ground_type==2],
//!   precondition: Some(0), // unlocked by the logic above
//!   action: Activate,
//!   /* ... */
//! }
//! ```
//!
//! **Runtime behavior**:
//!
//!   - **Every frame** (within the precondition window): Check if overtaken
//!     anyone (`change_order_onetime<0`). Once true, skill unlocked for
//!     activation attempts.
//!   - **At 1850m** (if unlocked): Trigger fires, evaluate guards
//!     `hp_per>=20 & ground_type==2`.
//!   - **If guards pass**: Activate skill.

use crate::course::Course;
use crate::skill::*;
use crate::trigger::random::{RandomRolls, roll_condition};
use crate::trigger::skill_activation::*;
use crate::trigger::system::*;
use rand::Rng;

/// Build a complete TriggerScheduler from a list of skills.
///
/// For each skill alternative:
///
/// 1. Pre-roll random triggers (once per skill, shared by its alternatives).
/// 2. Compile the condition into one logic per OR branch.
/// 3. Compile the precondition (if present) into logics that unlock the
///    alternative.
/// 4. Index triggers into scheduler queues.
///
/// Alternatives whose condition or precondition can never be satisfied on this
/// course are skipped. So are out-of-gate alternatives: their condition is
/// evaluated before the race, then they're activated by a race start logic
/// (see [`TriggerScheduler::add_race_start_logic`]). Other alternatives of
/// their skills (e.g. continuations) are compiled as usual.
pub fn build_trigger_scheduler(
    skills: &[Skill],
    course: &Course,
    rng: &mut impl Rng,
) -> TriggerScheduler {
    let mut scheduler = TriggerScheduler::default();

    for (skill_idx, skill) in skills.iter().enumerate() {
        // Random variables are rolled per skill, so that its alternatives share them.
        let mut rolls = RandomRolls::default();
        for (alt_idx, alternative) in skill.alternatives.iter().enumerate() {
            if alternative.is_out_of_gate() {
                continue;
            }

            // One logic per OR branch, except the ones whose random roll failed.
            let condition = &alternative.condition;
            let rolled = roll_condition(condition, course, &mut rolls, rng);
            let branches = compile_condition(condition, &rolled, course);
            if branches.is_empty() {
                // Condition can't be satisfied, skill never activates.
                continue;
            }

            // Compile precondition (if any).
            let is_continuation = alternative.is_continuation();
            let precondition = if let Some(parsed) = &alternative.precondition {
                let branches = compile_condition(parsed, &[], course);
                if branches.is_empty() {
                    // Precondition can't be satisfied, skill never activates.
                    continue;
                }
                Some(add_precondition(
                    branches,
                    skill_idx,
                    alt_idx,
                    is_continuation,
                    &mut scheduler,
                ))
            } else {
                None
            };

            for branch in branches {
                add_logic(
                    SkillActivationLogic::new(
                        skill_idx,
                        alt_idx,
                        branch.trigger,
                        branch.guards,
                        precondition,
                        LogicAction::Activate,
                        is_continuation,
                    ),
                    &mut scheduler,
                );
            }
        }
    }

    scheduler
}

impl TriggerScheduler {
    /// Add the logic activating an out-of-gate skill alternative on the first
    /// frame the runner moves.
    ///
    /// Its condition was already satisfied before the race, so it has no
    /// guards.
    pub fn add_race_start_logic(
        &mut self,
        skill_idx: usize,
        alt_idx: usize,
        is_continuation: bool,
    ) {
        add_logic(
            SkillActivationLogic::new(
                skill_idx,
                alt_idx,
                TriggerCondition::RaceStart,
                Vec::new(),
                None,
                LogicAction::Activate,
                is_continuation,
            ),
            self,
        );
    }
}

/// Add the logics unlocking a precondition, and return the precondition ID.
///
/// N.B. These logics are added before the logics they unlock, so that an event
/// firing both (e.g. entering the last straight) unlocks the skill first.
fn add_precondition(
    branches: Vec<CompiledBranch>,
    skill_idx: usize,
    alt_idx: usize,
    is_continuation: bool,
    scheduler: &mut TriggerScheduler,
) -> usize {
    let precondition_id = scheduler.precondition_satisfied.len();
    scheduler.precondition_satisfied.push(false);

    for branch in branches {
        // Windows go to their own queue, checked before the triggers they unlock (see
        // `Runner::check_skills`). That queue tracks positions only: time and HP
        // windows cover the whole race.
        let trigger = match branch.trigger {
            TriggerCondition::Window {
                variable: WindowVariable::Position,
                ..
            } => branch.trigger,
            TriggerCondition::Window { .. } => TriggerCondition::Window {
                variable: WindowVariable::Position,
                min_threshold: 0.0,
                max_threshold: f64::INFINITY,
            },
            event => event,
        };
        let logic = SkillActivationLogic::new(
            skill_idx,
            alt_idx,
            trigger,
            branch.guards,
            None,
            LogicAction::Unlock(precondition_id),
            is_continuation,
        );

        if let TriggerCondition::Window {
            min_threshold,
            max_threshold,
            ..
        } = logic.trigger
        {
            scheduler.precondition_windows.push(WindowTrigger {
                logic_idx: scheduler.skill_logics.len(),
                min_threshold,
                max_threshold,
            });
            scheduler.skill_logics.push(logic);
        } else {
            add_logic(logic, scheduler);
        }
    }

    precondition_id
}

/// Store a logic in the scheduler and index its trigger.
fn add_logic(logic: SkillActivationLogic, scheduler: &mut TriggerScheduler) {
    let logic_idx = scheduler.skill_logics.len();
    index_trigger(&logic.trigger, logic_idx, scheduler);
    scheduler.skill_logics.push(logic);
}

/// Index a trigger into the appropriate scheduler structure.
///
/// The `logic_idx` is the index into `scheduler.skill_logics`.
fn index_trigger(trigger: &TriggerCondition, logic_idx: usize, scheduler: &mut TriggerScheduler) {
    use TriggerCondition::*;

    match trigger {
        Segments { ranges } => {
            for &(min_threshold, max_threshold) in ranges {
                scheduler.position_windows.push(WindowTrigger {
                    logic_idx,
                    min_threshold,
                    max_threshold,
                });
            }
        }

        Window {
            variable,
            min_threshold,
            max_threshold,
        } => {
            let window = WindowTrigger {
                logic_idx,
                min_threshold: *min_threshold,
                max_threshold: *max_threshold,
            };
            match variable {
                WindowVariable::Position => scheduler.position_windows.push(window),
                WindowVariable::Time => scheduler.time_windows.push(window),
                WindowVariable::HpPercent => scheduler.hp_windows.push(window),
            }
        }

        LastStraightEntry => scheduler.last_straight_triggers.push(logic_idx),
        RaceStart => scheduler.race_start_triggers.push(logic_idx),
        AllOutSpurtStart => scheduler.all_out_spurt_triggers.push(logic_idx),
        AnySkillActivation => scheduler.any_activation_triggers.push(logic_idx),
        BuffSkillActivation => scheduler.buff_activation_triggers.push(logic_idx),
        HealSkillActivation => scheduler.heal_activation_triggers.push(logic_idx),
    }
}
