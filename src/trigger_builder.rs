//! Trigger Builder - Converts skill conditions into an efficient trigger scheduler
//!
//! This module processes skill conditions and builds an optimized TriggerScheduler that
//! enables event-driven skill activation instead of checking all skills every simulation step.
//!
//! # Architecture
//!
//! Skills have conditions like "activate_time>=5.0" or "distance_rate>=50". Instead of
//! evaluating these conditions every frame (O(m × n) where m = steps, n = skills), we:
//!
//! 1. **Parse conditions** - Extract all comparisons from condition trees
//! 2. **Classify triggers** - Categorize based on variable behavior:
//!    - MonotonicIncreasing: time, distance_rate, furlong (use priority queue)
//!    - MonotonicDecreasing: remain_distance, hp_per (use priority queue)
//!    - PhaseTransition: phase changes (use pre-computed boundaries)
//!    - Spatial: corner/slope events (handled by transition managers)
//!    - Onetime: last_straight_onetime, change_order_onetime, hp_empty_onetime
//!    - Random: Evaluated once at race start and cached
//!    - Continuous: Variables that need checking every step (minimized)
//!
//! 3. **Pre-roll random conditions** - All random triggers evaluated before race starts
//! 4. **Build priority queues** - Heap-based scheduling for monotonic variables
//!
//! # Performance
//!
//! This reduces complexity from O(m × n) to O(m × c + k log n) where:
//! - m = simulation steps
//! - n = number of skills
//! - c = continuous-check skills (very small subset)
//! - k = number of trigger activations
//!
//! For a typical race: m ≈ 1500 (100s @ 15fps), n ≈ 50 skills, c ≈ 5
//! - Naive: 75,000 condition checks
//! - Event-driven: ~7,500 checks + minimal heap operations

use crate::condition_parser::*;
use crate::condition_variables::*;
use crate::course::*;
use crate::random_triggers::preroll_random_triggers;
use crate::skill::*;
use crate::trigger_system::*;
use crate::units::*;
use rand::Rng;

/// Build a complete TriggerScheduler from a list of skills
///
/// This function processes each skill's condition to extract triggers and populate
/// the appropriate data structures in the scheduler for efficient runtime checking.
pub fn build_trigger_scheduler(
    skills: &[Skill],
    course: &Course,
    rng: &mut impl Rng,
) -> TriggerScheduler {
    let mut scheduler = TriggerScheduler::new();

    // Pre-roll all random triggers
    scheduler.random_activations = preroll_random_triggers(skills, course, rng);

    // Process each skill to extract triggers
    for (skill_idx, skill) in skills.iter().enumerate() {
        // Skip if this skill was randomly disabled
        if let Some(activation) = scheduler.random_activations.get(&skill_idx) {
            if !activation.activated {
                continue;
            }
        }

        // Process each alternative condition
        for alternative in &skill.alternatives {
            if let Ok(condition) = parse_condition(&alternative.condition) {
                extract_triggers_from_condition(
                    &condition.comp_list,
                    skill_idx,
                    &mut scheduler,
                    course,
                );
            }
        }
    }

    scheduler
}

/// Extract triggers from a parsed condition tree
fn extract_triggers_from_condition(
    comp_list: &CompList,
    skill_idx: usize,
    scheduler: &mut TriggerScheduler,
    course: &Course,
) {
    // Visit all comparisons in the condition tree
    comp_list.visit_all_comps(&mut |comp| {
        match comp.var.trigger_class() {
            TriggerClass::MonotonicIncreasing => {
                add_monotonic_increasing_trigger(comp, skill_idx, scheduler, course);
            }
            TriggerClass::MonotonicDecreasing => {
                add_monotonic_decreasing_trigger(comp, skill_idx, scheduler);
            }
            TriggerClass::PhaseTransition => {
                add_phase_trigger(comp, skill_idx, scheduler, course);
            }
            TriggerClass::SpatialCorner => {
                add_corner_trigger(comp, skill_idx, scheduler, course);
            }
            TriggerClass::SpatialSlope => {
                add_slope_trigger(comp, skill_idx, scheduler, course);
            }
            TriggerClass::SpatialStraight => {
                add_straight_trigger(comp, skill_idx, scheduler);
            }
            TriggerClass::Onetime => {
                add_onetime_trigger(comp, skill_idx, scheduler);
            }
            TriggerClass::Continuous => {
                if !scheduler.continuous_triggers.contains(&skill_idx) {
                    scheduler.continuous_triggers.push(skill_idx);
                }
            }
            _ => {
                // Static, Random, MultiUmaStub, Unknown - handle separately or ignore
            }
        }
    });
}

fn add_monotonic_increasing_trigger(
    comp: &Comp,
    skill_idx: usize,
    scheduler: &mut TriggerScheduler,
    course: &Course,
) {
    match &comp.var {
        VarType::Phase(PhaseVar::AccumulateTime) => {
            let threshold = comp.value as f64;
            scheduler.time_triggers.push(TimeTrigger {
                threshold_seconds: threshold,
                skill_index: skill_idx,
                operator: comp.op,
            });
        }
        VarType::Distance(DistanceVar::DistanceRate) => {
            let threshold = comp.value as f64;
            // DistanceRate is a percentage (0-100), convert to meters
            let threshold_meters = threshold / 100.0 * course.distance as f64;
            scheduler.distance_triggers.push(DistanceTrigger {
                threshold_meters,
                skill_index: skill_idx,
                operator: comp.op,
                trigger_type: DistanceTriggerType::DistanceRate,
            });
        }
        VarType::Distance(DistanceVar::Furlong) => {
            let threshold = comp.value as f64;
            let threshold_meters = furlongs_to_meters(threshold);
            scheduler.distance_triggers.push(DistanceTrigger {
                threshold_meters,
                skill_index: skill_idx,
                operator: comp.op,
                trigger_type: DistanceTriggerType::Furlong,
            });
        }
        VarType::Activation(ActivationVar::ActivateCountAll)
        | VarType::Activation(ActivationVar::ActivateCountStart)
        | VarType::Activation(ActivationVar::ActivateCountMiddle)
        | VarType::Activation(ActivationVar::ActivateCountLaterHalf)
        | VarType::Activation(ActivationVar::ActivateCountEndAfter)
        | VarType::Activation(ActivationVar::ActivateCountHeal) => {
            // These need continuous checking as they depend on other skills activating
            if !scheduler.continuous_triggers.contains(&skill_idx) {
                scheduler.continuous_triggers.push(skill_idx);
            }
        }
        _ => {}
    }
}

fn add_monotonic_decreasing_trigger(
    comp: &Comp,
    skill_idx: usize,
    scheduler: &mut TriggerScheduler,
) {
    match &comp.var {
        VarType::Distance(DistanceVar::RemainDistance) => {
            let threshold = comp.value as f64;
            scheduler.distance_triggers.push(DistanceTrigger {
                threshold_meters: threshold,
                skill_index: skill_idx,
                operator: comp.op,
                trigger_type: DistanceTriggerType::RemainDistance,
            });
        }
        VarType::Status(StatusVar::HpPer) => {
            let threshold = comp.value as f64;
            scheduler.hp_triggers.push(HpTrigger {
                threshold_percent: threshold,
                skill_index: skill_idx,
                operator: comp.op,
            });
        }
        _ => {}
    }
}

fn add_phase_trigger(
    comp: &Comp,
    skill_idx: usize,
    scheduler: &mut TriggerScheduler,
    course: &Course,
) {
    if let VarType::Phase(PhaseVar::Phase) = &comp.var {
        let phase_value = comp.value;
        let phase = match phase_value {
            0 => Phase::Early,
            1 => Phase::Mid,
            2 => Phase::Late,
            _ => return, // Unknown phase
        };

        // Calculate boundary distance for this phase
        let boundary_distance = match phase {
            Phase::Early => 0.0,
            Phase::Mid => course.section_length() * 4.0,
            Phase::Late => course.section_length() * 16.0,
            Phase::Last => course.section_length() * 20.0,
        };

        // Find or create phase trigger for this boundary
        // We can't use iter_mut on BinaryHeap, so we'll convert to Vec, modify, and rebuild
        let mut triggers: Vec<PhaseTransitionTrigger> = scheduler.phase_triggers.drain().collect();

        if let Some(existing) = triggers
            .iter_mut()
            .find(|t| t.boundary_distance == boundary_distance)
        {
            existing.skill_indices.push(skill_idx);
        } else {
            triggers.push(PhaseTransitionTrigger {
                phase,
                boundary_distance,
                skill_indices: vec![skill_idx],
            });
        }

        // Rebuild the heap
        scheduler.phase_triggers.extend(triggers);
    }
}

fn add_corner_trigger(
    comp: &Comp,
    skill_idx: usize,
    scheduler: &mut TriggerScheduler,
    _course: &Course,
) {
    if let VarType::Course(CourseVar::Corner) = &comp.var {
        // This is a general "is in a corner" check
        // We'll add to both entry and exit lists to check on both events
        // The actual condition will be evaluated at runtime

        // For now, treat as continuous since we don't know which specific corner
        if !scheduler.continuous_triggers.contains(&skill_idx) {
            scheduler.continuous_triggers.push(skill_idx);
        }
    }
}

fn add_slope_trigger(
    comp: &Comp,
    skill_idx: usize,
    scheduler: &mut TriggerScheduler,
    _course: &Course,
) {
    if let VarType::Course(CourseVar::Slope) = &comp.var {
        // Similar to corners, general slope check - continuous
        if !scheduler.continuous_triggers.contains(&skill_idx) {
            scheduler.continuous_triggers.push(skill_idx);
        }
    }
}

fn add_straight_trigger(comp: &Comp, skill_idx: usize, scheduler: &mut TriggerScheduler) {
    // Straight triggers need continuous checking
    if !scheduler.continuous_triggers.contains(&skill_idx) {
        scheduler.continuous_triggers.push(skill_idx);
    }
}

fn add_onetime_trigger(comp: &Comp, skill_idx: usize, scheduler: &mut TriggerScheduler) {
    match &comp.var {
        VarType::Course(CourseVar::IsLastStraightOnetime) => {
            scheduler.onetime_triggers.push(OnetimeTrigger {
                skill_index: skill_idx,
                trigger_type: OnetimeType::LastStraightEntry,
                activated: false,
            });
        }
        VarType::Position(PositionVar::ChangeOrderOnetime) => {
            let delta = comp.value;
            scheduler.onetime_triggers.push(OnetimeTrigger {
                skill_index: skill_idx,
                trigger_type: OnetimeType::PositionChange(delta),
                activated: false,
            });
        }
        VarType::Status(StatusVar::IsHpEmptyOnetime) => {
            scheduler.onetime_triggers.push(OnetimeTrigger {
                skill_index: skill_idx,
                trigger_type: OnetimeType::HpEmpty,
                activated: false,
            });
        }
        _ => {}
    }
}
