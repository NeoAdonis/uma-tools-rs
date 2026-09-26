//! # Trigger Builder
//!
//! This module contains functions for separating skill conditions into triggers and
//! guards to facilitate efficient skill activation checking. See
//! `skill_activation.rs` for a detailed explanation.
//!
//! ## Architecture
//!
//! For each skill alternative:
//!
//! 1. **Parse condition** - Parse condition string into AST.
//! 2. **Pre-roll random triggers** - Determine random activation points.
//! 3. **Compile precondition** - Turn the precondition (if present) into logics
//!    that unlock the skill using [`compile_precondition()`].
//! 4. **Split condition** - Separate triggers from guards using [`split_condition()`].
//! 5. **Create [`SkillActivationLogic`]** - Store triggers + guards + precondition ID.
//! 6. **Index triggers** - Add to appropriate scheduler queues/lists.
//!
//! ## Example
//!
//! **Input skill**:
//!
//! - **Condition**: `hp_per>=20 & is_finalcorner_random==1 & ground_type==2`
//! - **Precondition**: `change_order_onetime<0`
//!
//! **After splitting**:
//!
//! ```rust,ignore
//! SkillActivationLogic {
//!   triggers: [Position {
//!     meters: 1850, // random point in final corner
//!   }],
//!   guards: [hp_per>=20, ground_type==2],
//!   precondition: Some(0), // unlocked by the logic below
//!   action: Activate,
//!   /* ... */
//! }
//!
//! SkillActivationLogic {
//!   // change_order_onetime<0 can be true anywhere
//!   triggers: [Window { SpatialPosition, 0, inf }],
//!   guards: [change_order_onetime<0],
//!   precondition: None,
//!   action: Unlock(0),
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
use crate::course::{Course, Phase};
use crate::skill::*;
use crate::trigger::random::roll_condition;
use crate::trigger::skill_activation::*;
use crate::trigger::system::*;
use crate::units::furlongs_to_meters;
use rand::Rng;

/// Build a complete TriggerScheduler from a list of skills.
///
/// This uses trigger-guard separation. For each skill alternative:
///
/// 1. Pre-roll random triggers.
/// 2. Compile the precondition (if present) into logics that unlock the
///    alternative (see [`compile_precondition()`]). If it can never be satisfied,
///    skip the alternative.
/// 3. Split condition into triggers + guards.
/// 4. Create [`SkillActivationLogic`]s (triggers + guards + precondition ID).
/// 5. Index triggers into scheduler queues.
pub fn build_trigger_scheduler(
    skills: &[Skill],
    course: &Course,
    rng: &mut impl Rng,
) -> TriggerScheduler {
    let mut scheduler = TriggerScheduler::new();

    // Process each skill and create SkillActivationLogic
    for (skill_idx, skill) in skills.iter().enumerate() {
        // Process each alternative condition
        for (alt_idx, alternative) in skill.alternatives.iter().enumerate() {
            // Parse condition
            let condition = match parse_condition(&alternative.condition) {
                Ok(cond) => cond,
                Err(_) => continue, // Skip unparseable conditions
            };

            let random_activations = match roll_condition(&condition, course, rng) {
                Some(activations) => activations,
                None => continue, // Random roll failed, skill never activates
            };

            // Compile precondition (if any)
            let precondition = if alternative.precondition.is_empty() {
                None
            } else {
                let Ok(parsed) = parse_condition(&alternative.precondition) else {
                    continue; // Skip unparseable preconditions
                };
                let branches = compile_precondition(&parsed, course);
                if branches.is_empty() {
                    continue; // Precondition can't be satisfied, skill never activates
                }
                Some(add_precondition(
                    branches,
                    skill_idx,
                    alt_idx,
                    &mut scheduler,
                    course,
                ))
            };

            // Split condition into triggers and guards
            // This returns MULTIPLE branches if there are OR conditions
            let branches = split_condition(&condition, random_activations, course);

            // Create SkillActivationLogic for each branch
            for (triggers, guards) in branches {
                // Add continuous trigger if no triggers found
                let final_triggers = if triggers.is_empty() {
                    vec![TriggerCondition::Continuous]
                } else {
                    triggers
                };

                add_logic(
                    SkillActivationLogic::new(
                        skill_idx,
                        alt_idx,
                        final_triggers,
                        guards,
                        precondition,
                        LogicAction::Activate,
                    ),
                    &mut scheduler,
                    course,
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
    branches: Vec<PreconditionBranch>,
    skill_idx: usize,
    alt_idx: usize,
    scheduler: &mut TriggerScheduler,
    course: &Course,
) -> usize {
    let precondition_id = scheduler.precondition_satisfied.len();
    scheduler.precondition_satisfied.push(false);

    for branch in branches {
        let logic = SkillActivationLogic::new(
            skill_idx,
            alt_idx,
            vec![branch.trigger.clone()],
            branch.guards,
            None,
            LogicAction::Unlock(precondition_id),
        );

        // Position windows go to their own queue, checked at the start of each frame
        if let TriggerCondition::Window {
            min_threshold,
            max_threshold,
            ..
        } = branch.trigger
        {
            let logic_idx = scheduler.skill_logics.len();
            scheduler.skill_logics.push(logic);
            scheduler.precondition_windows.push(WindowTrigger {
                logic_idx,
                min_threshold,
                max_threshold,
            });
        } else {
            add_logic(logic, scheduler, course);
        }
    }

    precondition_id
}

/// Store a logic in the scheduler and index its triggers.
fn add_logic(logic: SkillActivationLogic, scheduler: &mut TriggerScheduler, course: &Course) {
    let logic_idx = scheduler.skill_logics.len();
    for trigger in &logic.triggers {
        index_trigger(trigger, logic_idx, scheduler, course);
    }
    scheduler.skill_logics.push(logic);
}

/// Index a trigger into the appropriate scheduler structure.
///
/// Takes a trigger from [`SkillActivationLogic`] and adds it to the correct queue/list.
/// The `logic_idx` is the index into `scheduler.skill_logics`.
fn index_trigger(
    trigger: &TriggerCondition,
    logic_idx: usize,
    scheduler: &mut TriggerScheduler,
    course: &Course,
) {
    use TriggerCondition::*;

    match trigger {
        Time {
            threshold,
            operator,
        } => {
            scheduler.time_triggers.push(TimeTrigger {
                threshold_seconds: *threshold,
                logic_idx,
                operator: *operator,
            });
        }

        // Distance triggers are converted to a position from the start (in meters)
        DistanceCovered { threshold } => {
            scheduler.distance_triggers.push(DistanceTrigger {
                threshold_meters: threshold / 100.0 * course.distance as f64,
                logic_idx,
            });
        }

        DistanceRemaining { threshold } => {
            scheduler.distance_triggers.push(DistanceTrigger {
                threshold_meters: course.distance as f64 - threshold,
                logic_idx,
            });
        }

        FurlongCrossed { threshold } => {
            scheduler.distance_triggers.push(DistanceTrigger {
                threshold_meters: furlongs_to_meters(*threshold),
                logic_idx,
            });
        }

        Position { meters } => {
            scheduler.distance_triggers.push(DistanceTrigger {
                threshold_meters: *meters,
                logic_idx,
            });
        }

        HpPercent {
            threshold,
            operator,
            ..
        } => {
            scheduler.hp_triggers.push(HpTrigger {
                threshold_percent: *threshold,
                logic_idx,
                operator: *operator,
            });
        }

        PhaseChange { phase } => {
            // Calculate boundary distance for this phase
            let boundary_distance = match phase {
                Phase::Early => 0.0,
                Phase::Mid => course.section_length() * 4.0,
                Phase::Late => course.section_length() * 16.0,
                Phase::Last => course.section_length() * 20.0,
            };

            // Find or create phase trigger for this boundary
            let mut triggers: Vec<PhaseTransitionTrigger> =
                scheduler.phase_triggers.drain().collect();

            if let Some(existing) = triggers
                .iter_mut()
                .find(|t| t.boundary_distance == boundary_distance)
            {
                existing.logic_indices.push(logic_idx);
            } else {
                triggers.push(PhaseTransitionTrigger {
                    phase: *phase,
                    boundary_distance,
                    logic_indices: vec![logic_idx],
                });
            }

            scheduler.phase_triggers.extend(triggers);
        }

        Window {
            variable,
            min_threshold,
            max_threshold,
        } => {
            // Convert distance-based windows to positions from the start (in meters)
            let course_distance = course.distance as f64;
            let (min_value, max_value) = match variable {
                WindowVariable::DistanceRate => (
                    min_threshold / 100.0 * course_distance,
                    max_threshold / 100.0 * course_distance,
                ),
                // Remaining distance decreases, so the bounds are swapped
                WindowVariable::RemainDistance => (
                    (course_distance - max_threshold).max(0.0),
                    course_distance - min_threshold,
                ),
                _ => (*min_threshold, *max_threshold),
            };

            let window = WindowTrigger {
                logic_idx,
                min_threshold: min_value,
                max_threshold: max_value,
            };
            match variable {
                WindowVariable::AccumulateTime => scheduler.time_windows.push(window),
                WindowVariable::HpPercent => scheduler.hp_windows.push(window),
                WindowVariable::DistanceRate
                | WindowVariable::RemainDistance
                | WindowVariable::SpatialPosition => scheduler.position_windows.push(window),
            }
        }

        Continuous => {
            if !scheduler.continuous_triggers.contains(&logic_idx) {
                scheduler.continuous_triggers.push(logic_idx);
            }
        }

        FullThrottleStart => {
            scheduler.full_throttle_triggers.push(logic_idx);
        }

        // Activation-dependent triggers
        AnySkillActivation => {
            if !scheduler.any_activation_triggers.contains(&logic_idx) {
                scheduler.any_activation_triggers.push(logic_idx);
            }
        }

        BuffSkillActivation => {
            if !scheduler.buff_activation_triggers.contains(&logic_idx) {
                scheduler.buff_activation_triggers.push(logic_idx);
            }
        }

        HealSkillActivation => {
            if !scheduler.heal_activation_triggers.contains(&logic_idx) {
                scheduler.heal_activation_triggers.push(logic_idx);
            }
        }

        // Straight triggers
        StraightTransition {
            is_last,
            is_entry: _,   // implicit - we'll check on entry at runtime
            is_onetime: _, // handled at runtime too
        } => {
            // For straights, we need to create triggers for each straight
            // For is_final==true, register for the last straight only
            // For is_final==false, register for all straights

            let straight_indices: Vec<usize> = if *is_last {
                // Find the last straight (highest index)
                if course.straights.is_empty() {
                    vec![]
                } else {
                    vec![course.straights.len() - 1]
                }
            } else {
                (0..course.straights.len()).collect()
            };

            for straight_idx in straight_indices {
                if let Some(existing) = scheduler
                    .straight_triggers
                    .iter_mut()
                    .find(|t| t.straight_index == straight_idx && t.is_last == *is_last)
                {
                    existing.logic_indices.push(logic_idx);
                } else {
                    scheduler.straight_triggers.push(StraightTrigger {
                        straight_index: straight_idx,
                        is_last: *is_last,
                        logic_indices: vec![logic_idx],
                    });
                }
            }
        }
    }
}
