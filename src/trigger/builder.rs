//! # Trigger Builder
//!
//! This module builds the [`TriggerScheduler`] of a runner: it compiles skill
//! conditions into triggers and guards (see `skill_activation.rs`), and indexes the
//! triggers so they can be checked efficiently.
//!
//! ## Architecture
//!
//! For each skill alternative:
//!
//! 1. **Parse condition** - Parse condition string into AST.
//! 2. **Pre-roll random triggers** - Determine random activation points.
//! 3. **Compile condition** - Turn each OR branch into a trigger + guards using
//!    [`compile_condition()`].
//! 4. **Compile precondition** - Turn the precondition (if present) into logics
//!    that unlock the skill, the same way.
//! 5. **Create [`SkillActivationLogic`]** - Store trigger + guards + precondition ID.
//! 6. **Index triggers** - Add to appropriate scheduler queues/lists.
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
//!   trigger: Position { meters: [1850] }, // random point in final corner
//!   guards: [hp_per>=20, ground_type==2],
//!   precondition: Some(0), // unlocked by the logic above
//!   action: Activate,
//!   /* ... */
//! }
//! ```
//!
//! **Runtime behavior**:
//!
//!   - **Every frame** (within the precondition window): Check if overtaken anyone
//!     (`change_order_onetime<0`). Once true, skill unlocked for activation attempts.
//!   - **At 1850m** (if unlocked): Trigger fires, evaluate guards
//!     `hp_per>=20 & ground_type==2`.
//!   - **If guards pass**: Activate skill.

use crate::condition::parser::*;
use crate::course::Course;
use crate::skill::*;
use crate::trigger::random::roll_condition;
use crate::trigger::skill_activation::*;
use crate::trigger::system::*;
use rand::Rng;

/// Build a complete TriggerScheduler from a list of skills.
///
/// For each skill alternative:
///
/// 1. Pre-roll random triggers.
/// 2. Compile the condition into one logic per OR branch.
/// 3. Compile the precondition (if present) into logics that unlock the
///    alternative.
/// 4. Index triggers into scheduler queues.
///
/// Alternatives whose condition or precondition can never be satisfied on this
/// course are skipped.
pub fn build_trigger_scheduler(
    skills: &[Skill],
    course: &Course,
    rng: &mut impl Rng,
) -> TriggerScheduler {
    let mut scheduler = TriggerScheduler::new();

    for (skill_idx, skill) in skills.iter().enumerate() {
        for (alt_idx, alternative) in skill.alternatives.iter().enumerate() {
            // Parse condition
            let condition = match parse_condition(&alternative.condition) {
                Ok(cond) => cond,
                Err(_) => continue, // Skip unparseable conditions
            };

            // One logic per OR branch, except the ones whose random roll failed
            let rolls = roll_condition(&condition, course, rng);
            let branches = compile_condition(&condition, &rolls, course);
            if branches.is_empty() {
                continue; // Condition can't be satisfied, skill never activates
            }

            // Compile precondition (if any)
            let precondition = if alternative.precondition.is_empty() {
                None
            } else {
                let Ok(parsed) = parse_condition(&alternative.precondition) else {
                    continue; // Skip unparseable preconditions
                };
                let branches = compile_condition(&parsed, &[], course);
                if branches.is_empty() {
                    continue; // Precondition can't be satisfied, skill never activates
                }
                Some(add_precondition(
                    branches,
                    skill_idx,
                    alt_idx,
                    &mut scheduler,
                ))
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
                    ),
                    &mut scheduler,
                );
            }
        }
    }

    scheduler
}

/// Add the logics unlocking a precondition, and return the precondition ID.
///
/// N.B. These logics are added before the logics they unlock, so that an event
/// firing both (e.g. entering the last straight) unlocks the skill first.
fn add_precondition(
    branches: Vec<CompiledBranch>,
    skill_idx: usize,
    alt_idx: usize,
    scheduler: &mut TriggerScheduler,
) -> usize {
    let precondition_id = scheduler.precondition_satisfied.len();
    scheduler.precondition_satisfied.push(false);

    for branch in branches {
        // Windows go to their own queue, checked before any other trigger. That
        // queue tracks positions only: time and HP windows cover the whole race
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
        Position { meters } => {
            for &threshold_meters in meters {
                scheduler.distance_triggers.push(DistanceTrigger {
                    threshold_meters,
                    logic_idx,
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
        FullThrottleStart => scheduler.full_throttle_triggers.push(logic_idx),
        AnySkillActivation => scheduler.any_activation_triggers.push(logic_idx),
        BuffSkillActivation => scheduler.buff_activation_triggers.push(logic_idx),
        HealSkillActivation => scheduler.heal_activation_triggers.push(logic_idx),
    }
}
