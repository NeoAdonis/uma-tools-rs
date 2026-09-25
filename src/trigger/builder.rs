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
//! 2. **Parse precondition** - Parse precondition string (if present).
//! 3. **Pre-roll random triggers** - Determine random activation points.
//! 4. **Split condition** - Separate triggers from guards using [`split_condition()`].
//! 5. **Create [`SkillActivationLogic`]** - Store triggers + guards + precondition.
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
//!   guard_condition: Some(Condition), // hp_per>=20 & ground_type==2
//!   precondition: Some(Condition), // change_order_onetime<0
//!   /* ... */
//! }
//! ```
//!
//! **Runtime behavior**:
//!
//!   - **Every frame**: Check if overtaken anyone (`change_order_onetime<0`).
//!     Once true, skill unlocked for activation attempts.
//!   - **At 1850m** (if unlocked): Trigger fires, evaluate guards
//!     `hp_per>=20 & ground_type==2`.
//!   - **If guards pass**: Activate skill.

use crate::condition::parser::*;
use crate::course::{Course, Phase};
use crate::trigger::random::roll_condition;
use crate::trigger::skill_activation::*;
use crate::trigger::system::*;
use crate::uma::skill::*;
use crate::units::furlongs_to_meters;
use rand::Rng;

/// Build a complete TriggerScheduler from a list of skills.
///
/// This uses trigger-guard separation. For each skill:
///
/// 1. Pre-roll random triggers.
/// 2. Parse precondition (if present).
/// 3. Split condition into triggers + guards.
/// 4. Create [`SkillActivationLogic`] (triggers + guards + precondition).
/// 5. Index triggers into scheduler queues.
/// 6. Initialize precondition tracking (locked/unlocked state).
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

            // Parse precondition (if any)
            let precondition = if !alternative.precondition.is_empty() {
                parse_condition(&alternative.precondition).ok()
            } else {
                None
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
                    triggers.clone()
                };

                // Create SkillActivationLogic
                let logic = SkillActivationLogic::new(
                    skill_idx,
                    alt_idx,
                    final_triggers.clone(),
                    guards,
                    precondition.clone(),
                );

                // Store in scheduler
                let logic_idx = scheduler.skill_logics.len();
                scheduler.skill_logics.push(logic);

                // Index each trigger into appropriate scheduler structure
                for trigger in &final_triggers {
                    index_trigger(trigger, logic_idx, &mut scheduler, course);
                }
            }
        }
    }

    // Initialize array to track precondition satisfaction (parallel to skill_logics)
    // Preconditions are checked every frame until satisfied, unlocking the skill
    // Skills with preconditions start "locked" (false)
    // Skills without preconditions start "unlocked" (true)
    scheduler.precondition_satisfied = scheduler
        .skill_logics
        .iter()
        .map(|logic| logic.precondition.is_none())
        .collect();

    scheduler
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

            scheduler.window_triggers.push(WindowTrigger {
                logic_idx,
                variable: *variable,
                min_threshold: min_value,
                max_threshold: max_value,
            });
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
