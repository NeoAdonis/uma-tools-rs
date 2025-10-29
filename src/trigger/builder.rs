//! Trigger Builder - Converts skill conditions into trigger-guard separation
//! This module is part of the trigger-guard separation architecture.
//! See `skill_activation.rs` for detailed explanation.
//!
//! # Architecture
//!
//! For each skill alternative:
//! 1. **Parse condition** - Parse condition string into AST
//! 2. **Pre-roll random triggers** - Determine random activation points
//! 3. **Split condition** - Separate triggers from guards using `split_condition()`
//! 4. **Create SkillActivationLogic** - Store triggers + guards together
//! 5. **Index triggers** - Add to appropriate scheduler queues/lists
//!
//! # Example
//!
//! ```ignore
//! Input skill condition: "hp_per>=20 & is_finalcorner_random==1 & ground_type==2"
//!
//! After splitting:
//!   SkillActivationLogic {
//!     skill_index: 0,
//!     triggers: [RandomActivation { distance: 1850m }],
//!     guard_condition: "hp_per>=20 & ground_type==2",
//!   }
//!
//! Runtime behavior:
//!   At 1850m:
//!     - Random trigger fires
//!     - Evaluate ONLY guards: "hp_per>=20 & ground_type==2"
//!     - If true, activate skill
//! ```

use crate::condition::parser::*;
use crate::course::{Course, Phase};
use crate::race::skill_activation::*;
use crate::trigger::random::preroll_random_triggers;
use crate::trigger::system::*;
use crate::uma::skill::*;
use rand::Rng;

/// Build a complete TriggerScheduler from a list of skills
///
/// This uses trigger-guard separation. For each skill:
/// 1. Pre-roll random triggers
/// 2. Split condition into triggers + guards
/// 3. Create SkillActivationLogic
/// 4. Index triggers into scheduler queues
pub fn build_trigger_scheduler(
    skills: &[Skill],
    course: &Course,
    rng: &mut impl Rng,
) -> TriggerScheduler {
    let mut scheduler = TriggerScheduler::new();

    // Pre-roll all random triggers (still needed for random activation distances)
    scheduler.random_activations = preroll_random_triggers(skills, course, rng);

    // Process each skill and create SkillActivationLogic
    for (skill_idx, skill) in skills.iter().enumerate() {
        // Check if this skill has a random activation (clone to avoid borrow issues)
        let random_activation = scheduler.random_activations.get(&skill_idx).cloned();

        // Skip if random activation failed (skill never activates)
        if let Some(ref activation) = random_activation
            && !activation.activated
        {
            continue;
        }

        // Process each alternative condition
        for (alt_idx, alternative) in skill.alternatives.iter().enumerate() {
            // Parse condition
            let condition = match parse_condition(&alternative.condition) {
                Ok(cond) => cond,
                Err(_) => continue, // Skip unparseable conditions
            };

            // Parse precondition (if any)
            let precondition = if !alternative.precondition.is_empty() {
                parse_condition(&alternative.precondition).ok()
            } else {
                None
            };

            // Split condition into triggers and guards
            // This returns MULTIPLE branches if there are OR conditions
            let branches = split_condition(&condition, random_activation.as_ref());

            // Create SkillActivationLogic for each branch
            for (triggers, guard_condition) in branches {
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
                    guard_condition,
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

    scheduler
}

/// Index a trigger into the appropriate scheduler structure
///
/// Takes a trigger from SkillActivationLogic and adds it to the correct queue/list.
/// The `logic_idx` is the index into `scheduler.skill_logics`.
fn index_trigger(
    trigger: &TriggerCondition,
    logic_idx: usize,
    scheduler: &mut TriggerScheduler,
    course: &Course,
) {
    use crate::race::skill_activation::TriggerCondition;

    match trigger {
        TriggerCondition::Time {
            threshold,
            operator,
        } => {
            scheduler.time_triggers.push(TimeTrigger {
                threshold_seconds: *threshold,
                skill_index: logic_idx,
                operator: *operator,
            });
        }

        TriggerCondition::DistanceCovered {
            threshold,
            operator,
        } => {
            // Convert percentage to meters
            let threshold_meters = threshold / 100.0 * course.distance as f64;
            scheduler.distance_triggers.push(DistanceTrigger {
                threshold_meters,
                skill_index: logic_idx,
                operator: *operator,
                trigger_type: DistanceTriggerType::DistanceRate,
            });
        }

        TriggerCondition::DistanceRemaining {
            threshold,
            operator,
        } => {
            scheduler.distance_triggers.push(DistanceTrigger {
                threshold_meters: *threshold,
                skill_index: logic_idx,
                operator: *operator,
                trigger_type: DistanceTriggerType::RemainDistance,
            });
        }

        TriggerCondition::HpPercent {
            threshold,
            operator,
        } => {
            scheduler.hp_triggers.push(HpTrigger {
                threshold_percent: *threshold,
                skill_index: logic_idx,
                operator: *operator,
            });
        }

        TriggerCondition::PhaseChange { phase, operator: _ } => {
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
                existing.skill_indices.push(logic_idx);
            } else {
                triggers.push(PhaseTransitionTrigger {
                    phase: *phase,
                    boundary_distance,
                    skill_indices: vec![logic_idx],
                });
            }

            scheduler.phase_triggers.extend(triggers);
        }

        TriggerCondition::RandomActivation { .. } => {
            // Random triggers are handled specially - they're stored in random_activations
            // We'll check them when we reach the activation distance
            // For now, we don't need to index them separately
        }

        TriggerCondition::LastStraightEntry => {
            scheduler.onetime_triggers.push(OnetimeTrigger {
                skill_index: logic_idx,
                trigger_type: OnetimeType::LastStraightEntry,
                activated: false,
            });
        }

        TriggerCondition::PositionChange { delta } => {
            scheduler.onetime_triggers.push(OnetimeTrigger {
                skill_index: logic_idx,
                trigger_type: OnetimeType::PositionChange(*delta),
                activated: false,
            });
        }

        TriggerCondition::HpEmpty => {
            scheduler.onetime_triggers.push(OnetimeTrigger {
                skill_index: logic_idx,
                trigger_type: OnetimeType::HpEmpty,
                activated: false,
            });
        }

        TriggerCondition::Continuous => {
            if !scheduler.continuous_triggers.contains(&logic_idx) {
                scheduler.continuous_triggers.push(logic_idx);
            }
        }

        // Spatial triggers - TODO: Implement properly
        TriggerCondition::CornerTransition { .. }
        | TriggerCondition::SlopeTransition { .. }
        | TriggerCondition::StraightTransition { .. } => {
            // For now, treat as continuous
            if !scheduler.continuous_triggers.contains(&logic_idx) {
                scheduler.continuous_triggers.push(logic_idx);
            }
        }
    }
}
