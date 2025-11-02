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
use crate::trigger::random::roll_condition;
use crate::trigger::system::*;
use crate::uma::skill::*;
use crate::units::furlongs_to_meters;
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
                Ok(activations) => activations,
                Err(_) => continue, // Random roll failed, skill never activates
            };

            // Parse precondition (if any)
            let precondition = if !alternative.precondition.is_empty() {
                parse_condition(&alternative.precondition).ok()
            } else {
                None
            };

            // Split condition into triggers and guards
            // This returns MULTIPLE branches if there are OR conditions
            let branches = split_condition(&condition, random_activations);

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

        TriggerCondition::FurlongCrossed {
            threshold,
            operator,
        } => {
            let threshold_meters = furlongs_to_meters(*threshold);
            scheduler.distance_triggers.push(DistanceTrigger {
                threshold_meters,
                skill_index: logic_idx,
                operator: *operator,
                trigger_type: DistanceTriggerType::Furlong,
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

        TriggerCondition::Window {
            variable,
            min_threshold,
            max_threshold,
        } => {
            // Convert distance_rate percentage to meters if needed
            let (min_value, max_value) = match variable {
                crate::race::skill_activation::WindowVariable::DistanceRate => {
                    // Convert percentage to meters
                    let min_meters = min_threshold / 100.0 * course.distance as f64;
                    let max_meters = max_threshold / 100.0 * course.distance as f64;
                    (min_meters, max_meters)
                }
                _ => (*min_threshold, *max_threshold),
            };

            scheduler.window_triggers.push(WindowTrigger {
                skill_index: logic_idx,
                variable: *variable,
                min_threshold: min_value,
                max_threshold: max_value,
            });
        }

        TriggerCondition::Continuous => {
            if !scheduler.continuous_triggers.contains(&logic_idx) {
                scheduler.continuous_triggers.push(logic_idx);
            }
        }

        // Corner triggers
        TriggerCondition::CornerTransition {
            corner_index,
            is_entry,
        } => {
            // Determine which corners to register for
            let corner_indices: Vec<usize> = match corner_index {
                Some(idx) => vec![*idx],                     // Specific corner
                None => (0..course.corners.len()).collect(), // Any corner (all of them)
            };

            for corner_idx in corner_indices {
                let trigger_vec = if *is_entry {
                    &mut scheduler.corner_entry_triggers
                } else {
                    &mut scheduler.corner_exit_triggers
                };

                // Find or create trigger for this corner
                if let Some(existing) = trigger_vec
                    .iter_mut()
                    .find(|t| t.corner_index == corner_idx)
                {
                    // Add skill to existing trigger
                    existing.skill_indices.push(logic_idx);
                } else {
                    // Create new trigger for this corner
                    trigger_vec.push(CornerTransitionTrigger {
                        corner_index: corner_idx,
                        skill_indices: vec![logic_idx],
                    });
                }
            }
        }

        // Slope triggers
        TriggerCondition::SlopeTransition {
            slope_type,
            is_entry,
        } => {
            // Determine which slopes to register for
            let slope_indices: Vec<usize> = match slope_type {
                crate::course::geometry::SlopeType::Uphill => course
                    .slopes
                    .iter()
                    .enumerate()
                    .filter(|(_, s)| s.slope_type() == crate::course::geometry::SlopeType::Uphill)
                    .map(|(i, _)| i)
                    .collect(),
                crate::course::geometry::SlopeType::Downhill => course
                    .slopes
                    .iter()
                    .enumerate()
                    .filter(|(_, s)| s.slope_type() == crate::course::geometry::SlopeType::Downhill)
                    .map(|(i, _)| i)
                    .collect(),
                crate::course::geometry::SlopeType::Flat => {
                    vec![] // Flat is not a trigger
                }
            };

            for slope_idx in slope_indices {
                let trigger_vec = if *is_entry {
                    &mut scheduler.slope_entry_triggers
                } else {
                    &mut scheduler.slope_exit_triggers
                };

                // Find or create trigger for this slope
                if let Some(existing) = trigger_vec.iter_mut().find(|t| t.slope_index == slope_idx)
                {
                    existing.skill_indices.push(logic_idx);
                } else {
                    trigger_vec.push(SlopeTransitionTrigger {
                        slope_index: slope_idx,
                        skill_indices: vec![logic_idx],
                    });
                }
            }
        }

        // Straight triggers
        TriggerCondition::StraightTransition {
            is_final,
            is_entry: _, // is_entry is implicit - we'll check on entry in runtime
        } => {
            // For straights, we need to create triggers for each straight
            // For is_final==true, register for the last straight only
            // For is_final==false, register for all straights

            let straight_indices: Vec<usize> = if *is_final {
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
                    .find(|t| t.straight_index == straight_idx && t.is_final == *is_final)
                {
                    existing.skill_indices.push(logic_idx);
                } else {
                    scheduler.straight_triggers.push(StraightTrigger {
                        straight_index: straight_idx,
                        is_final: *is_final,
                        skill_indices: vec![logic_idx],
                    });
                }
            }
        }
    }
}
