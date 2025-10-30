//! Skill Activation Logic - Trigger-Guard Separation System
//!
//! This module implements the refined skill activation system that separates
//! **triggers** (which determine WHEN to check a skill) from **guards**
//! (which must be true AT activation time).
//!
//! # Overview
//!
//! The skill activation process is divided into three main components:
//!
//! 1. **Triggers** determine WHEN to evaluate a skill
//!    - Random gates: Fire once at pre-rolled location
//!    - Monotonic: Fire when threshold is crossed
//!    - One-time events: Fire when event occurs (can check permanently after)
//!
//! 2. **Guards** are evaluated ONLY when trigger fires
//!    - Static conditions: ground_type==2, motivation>=4
//!    - Runtime checks: hp_per>=20, running_style==1
//!
//! 3. **Activation State** tracks whether skill has fired
//!    - One-time triggers mark skill as "attempted" after first check
//!    - Continuous triggers can be checked multiple times
//!
//! # Example
//!
//! Consider a skill with condition: `hp_per>=20 & is_finalcorner_random==1`
//!
//! **Random Gate**: `is_finalcorner_random` pre-rolls to distance 1850m
//! **Guard**: `hp_per>=20` must be true when we reach 1850m

use crate::condition::parser::*;
use crate::condition::variables::*;
use crate::course::Phase;
use crate::course::geometry::SlopeType;
use crate::trigger::system::RandomActivation;

/// Complete activation logic for a skill
/// Replaces the old approach of scattering skill indices across multiple heaps
#[derive(Clone, Debug)]
pub struct SkillActivationLogic {
    /// Index into runner.available_skills
    pub skill_index: usize,

    /// Which alternative of this skill (skills can have multiple condition sets)
    pub alternative_index: usize,

    /// Triggers that determine WHEN to check this skill
    pub triggers: Vec<TriggerCondition>,

    /// Guards that must be true AT activation time
    /// If None, no guard checks needed (always activates when trigger fires)
    pub guard_condition: Option<Condition>,

    /// Precondition that must be evaluated before checking condition
    /// Evaluated at trigger time, before guard_condition
    pub precondition: Option<Condition>,

    /// Activation state tracking
    pub state: ActivationState,
}

/// Tracks whether this skill has been checked/activated
#[derive(Clone, Debug, Default)]
pub struct ActivationState {
    /// Has this skill successfully activated?
    pub has_activated: bool,

    /// For one-time triggers: has the trigger fired (even if guards failed)?
    /// Prevents re-checking skills that failed their guards
    pub trigger_attempted: bool,

    /// For continuous triggers: last frame this was checked
    /// Prevents duplicate checks in same frame
    pub last_checked_frame: Option<usize>,
}

/// A trigger condition that determines WHEN to check a skill
#[derive(Clone, Debug)]
pub enum TriggerCondition {
    // ========== RANDOM GATES ==========
    /// Random activation at specific distance
    /// Fires ONCE at this exact point, checks guards there
    RandomActivation {
        activation_distance: f64,
        /// If Some, only activate if still in this phase
        phase_restriction: Option<Phase>,
        /// If Some, only activate if still on this corner
        corner_restriction: Option<usize>,
        /// If Some, only activate if still on this slope type
        slope_restriction: Option<SlopeType>,
    },

    // ========== MONOTONIC TRIGGERS ==========
    /// Time elapsed since race start
    Time { threshold: f64, operator: CompOp },

    /// Distance covered (as percentage 0-100)
    DistanceCovered { threshold: f64, operator: CompOp },

    /// Distance remaining (in meters)
    DistanceRemaining { threshold: f64, operator: CompOp },

    /// HP percentage (0-100)
    HpPercent { threshold: f64, operator: CompOp },

    // ========== PHASE TRIGGERS ==========
    /// Phase transition
    PhaseChange { phase: Phase, operator: CompOp },

    // ========== SPATIAL TRIGGERS ==========
    /// Entering or exiting a corner
    CornerTransition {
        corner_index: Option<usize>, // None = any corner
        is_entry: bool,              // true = enter, false = exit
    },

    /// Entering or exiting a slope
    SlopeTransition {
        slope_type: SlopeType, // Uphill, Downhill, or Any
        is_entry: bool,
    },

    /// Entering or exiting a straight
    StraightTransition {
        is_final: bool, // true = only final straight
        is_entry: bool,
    },

    // ========== ONETIME EVENTS ==========
    /// Entering last straight (fires once)
    LastStraightEntry,

    /// Position change in race
    PositionChange { delta: i32 }, // Negative = overtake, positive = overtaken

    /// HP depleted
    HpEmpty,

    // ========== CONTINUOUS ==========
    /// Must be checked every frame
    /// Used for conditions that can't be efficiently triggered
    Continuous,
}

impl TriggerCondition {
    /// Is this a one-time trigger that can only fire once?
    pub fn is_onetime(&self) -> bool {
        matches!(
            self,
            TriggerCondition::RandomActivation { .. }
                | TriggerCondition::LastStraightEntry
                | TriggerCondition::PositionChange { .. }
                | TriggerCondition::HpEmpty
        )
    }

    /// Can this trigger fire multiple times?
    pub fn is_repeatable(&self) -> bool {
        !self.is_onetime()
    }
}

impl SkillActivationLogic {
    /// Create a new skill activation logic
    pub fn new(
        skill_index: usize,
        alternative_index: usize,
        triggers: Vec<TriggerCondition>,
        guard_condition: Option<Condition>,
        precondition: Option<Condition>,
    ) -> Self {
        SkillActivationLogic {
            skill_index,
            alternative_index,
            triggers,
            guard_condition,
            precondition,
            state: ActivationState::default(),
        }
    }

    /// Should this skill be checked at all?
    /// Returns false if already activated or trigger already attempted
    pub fn should_check(&self) -> bool {
        if self.state.has_activated {
            return false; // Already activated
        }

        // For one-time triggers, check if we've already attempted
        let has_onetime = self.triggers.iter().any(|t| t.is_onetime());
        if has_onetime && self.state.trigger_attempted {
            return false; // One-time trigger already fired
        }

        true
    }

    /// Mark that we've attempted this trigger (for onetime triggers)
    pub fn mark_attempted(&mut self) {
        self.state.trigger_attempted = true;
    }

    /// Mark that skill has successfully activated
    pub fn mark_activated(&mut self) {
        self.state.has_activated = true;
        self.state.trigger_attempted = true;
    }
}

/// Split a condition tree into triggers and guards
///
/// This is the core algorithm that separates conditions based on their role:
/// - Triggers determine WHEN to check
/// - Guards must be true AT check time
///
/// # OR Handling
///
/// For conditions with OR (`@`), this returns **multiple** trigger-guard pairs.
/// The caller should create separate SkillActivationLogic for each OR branch.
///
/// # Example
///
/// Input: `(phase_random==1 & hp_per>=20) @ (corner==2 & ground_type==1)`
///
/// Output (2 branches):
/// - Branch 1: triggers=[RandomActivation], guards="hp_per>=20"
/// - Branch 2: triggers=[CornerTransition], guards="ground_type==1"
///
/// # Simple Example
///
/// Input: `hp_per>=20 & is_finalcorner_random==1 & ground_type==2`
///
/// Output (1 branch):
/// - Triggers: [RandomActivation { distance: 1850m }]
/// - Guards: `hp_per>=20 & ground_type==2`
pub fn split_condition(
    condition: &Condition,
    random_activation: Option<&RandomActivation>,
) -> Vec<(Vec<TriggerCondition>, Option<Condition>)> {
    let branches = split_comp_list(&condition.comp_list, random_activation);

    // Apply promotion logic to each branch at the top level
    branches
        .into_iter()
        .map(|(triggers, guards)| apply_monotonic_promotion(triggers, guards))
        .collect()
}

/// Recursively split a CompList, handling OR branches
fn split_comp_list(
    comp_list: &CompList,
    random_activation: Option<&RandomActivation>,
) -> Vec<(Vec<TriggerCondition>, Option<Condition>)> {
    match comp_list {
        CompList::Single(comp) => {
            // Base case: single comparison
            // DON'T apply promotion here - it will be done after combining
            vec![split_single_comp(comp, random_activation)]
        }

        CompList::Binary { left, op, right } => match op {
            BinOp::And => {
                // AND: Combine triggers and guards from both sides
                let left_splits = split_comp_list(left, random_activation);
                let right_splits = split_comp_list(right, random_activation);

                // For AND, we should have exactly one split on each side
                // (unless there are nested ORs, which we'll handle recursively)
                let mut results = Vec::new();

                for (left_triggers, left_guard) in left_splits {
                    for (right_triggers, right_guard) in &right_splits {
                        // Combine triggers
                        let mut combined_triggers = left_triggers.clone();
                        combined_triggers.extend(right_triggers.clone());

                        // Combine guards with AND
                        let combined_guard = match (&left_guard, right_guard) {
                            (None, None) => None,
                            (Some(g), None) | (None, Some(g)) => Some(g.clone()),
                            (Some(g1), Some(g2)) => {
                                // Create AND of both guards
                                Some(Condition {
                                    comp_list: CompList::Binary {
                                        left: Box::new(g1.comp_list.clone()),
                                        op: BinOp::And,
                                        right: Box::new(g2.comp_list.clone()),
                                    },
                                })
                            }
                        };

                        // Promotion will be applied at top level
                        results.push((combined_triggers, combined_guard));
                    }
                }

                results
            }

            BinOp::Or => {
                // OR: Create separate branches
                let mut results = Vec::new();

                // Add all branches from left side
                results.extend(split_comp_list(left, random_activation));

                // Add all branches from right side
                results.extend(split_comp_list(right, random_activation));

                results
            }
        },
    }
}

/// Apply MonotonicGuard promotion rules
///
/// Rules:
/// 1. If there are explicit triggers (RandomGate/Spatial/Phase), MonotonicGuards stay as guards
/// 2. If no explicit triggers exist, promote MonotonicGuards to triggers
/// 3. If only guards remain after promotion, add Continuous trigger
fn apply_monotonic_promotion(
    mut triggers: Vec<TriggerCondition>,
    guard_condition: Option<Condition>,
) -> (Vec<TriggerCondition>, Option<Condition>) {
    // Check if we have explicit triggers (not Continuous)
    let has_explicit_trigger = triggers
        .iter()
        .any(|t| !matches!(t, TriggerCondition::Continuous));

    if has_explicit_trigger {
        // Rule 1: Explicit triggers exist → MonotonicGuards stay as guards
        (triggers, guard_condition)
    } else {
        // Rule 2: No explicit triggers → try to promote MonotonicGuards
        if let Some(guard) = guard_condition {
            let (promoted_triggers, remaining_guards) = promote_monotonic_guards(&guard);

            if !promoted_triggers.is_empty() {
                // Successfully promoted some MonotonicGuards to triggers
                triggers.extend(promoted_triggers);
                (triggers, remaining_guards)
            } else {
                // Rule 3: No MonotonicGuards to promote → Continuous trigger
                if triggers.is_empty() {
                    triggers.push(TriggerCondition::Continuous);
                }
                (triggers, Some(guard))
            }
        } else {
            // No guards at all
            if triggers.is_empty() {
                // No triggers, no guards → Continuous (shouldn't happen often)
                triggers.push(TriggerCondition::Continuous);
            }
            (triggers, None)
        }
    }
}

/// Extract MonotonicGuards from guard condition and promote them to triggers
///
/// Returns: (promoted triggers, remaining guard condition)
fn promote_monotonic_guards(guard: &Condition) -> (Vec<TriggerCondition>, Option<Condition>) {
    let mut promoted_triggers = Vec::new();
    let mut remaining_guards = Vec::new();

    // Collect all comparisons from guard
    collect_and_classify_comps(
        &guard.comp_list,
        &mut promoted_triggers,
        &mut remaining_guards,
    );

    // Rebuild remaining guard condition
    let final_guard = if remaining_guards.is_empty() {
        None
    } else if remaining_guards.len() == 1 {
        Some(Condition {
            comp_list: CompList::Single(remaining_guards[0].clone()),
        })
    } else {
        // Multiple guards - combine with AND
        Some(rebuild_guard_condition(remaining_guards))
    };

    (promoted_triggers, final_guard)
}

/// Recursively collect comparisons and classify them as triggers or guards
fn collect_and_classify_comps(
    comp_list: &CompList,
    promoted_triggers: &mut Vec<TriggerCondition>,
    remaining_guards: &mut Vec<Comp>,
) {
    match comp_list {
        CompList::Single(comp) => {
            let role = comp.var.variable_role();
            if matches!(role, VariableRole::MonotonicGuard) {
                // Promote to trigger
                promoted_triggers.push(extract_monotonic_trigger(comp));
            } else {
                // Keep as guard
                remaining_guards.push(comp.clone());
            }
        }
        CompList::Binary { left, op, right } => {
            if matches!(op, BinOp::And) {
                // For AND, recurse into both sides
                collect_and_classify_comps(left, promoted_triggers, remaining_guards);
                collect_and_classify_comps(right, promoted_triggers, remaining_guards);
            } else {
                // For OR, we can't split - keep as guard
                // This is a complex case that we'll treat conservatively
                remaining_guards.push(Comp {
                    var: VarType::Unknown("complex_or".to_string()),
                    op: CompOp::Equal,
                    value: 0,
                });
            }
        }
    }
}

/// Rebuild guard condition from list of comparisons
fn rebuild_guard_condition(guards: Vec<Comp>) -> Condition {
    if guards.is_empty() {
        panic!("Cannot rebuild guard condition from empty list");
    }

    let mut comp_list = CompList::Single(guards[0].clone());

    for guard in guards.iter().skip(1) {
        comp_list = CompList::Binary {
            left: Box::new(comp_list),
            op: BinOp::And,
            right: Box::new(CompList::Single(guard.clone())),
        };
    }

    Condition { comp_list }
}

/// Split a single comparison into trigger and guard
///
/// Note: This function does NOT make promotion decisions for MonotonicGuard.
/// It treats them as guards by default. Promotion happens at a higher level
/// in split_comp_list after analyzing all conditions together.
fn split_single_comp(
    comp: &Comp,
    random_activation: Option<&RandomActivation>,
) -> (Vec<TriggerCondition>, Option<Condition>) {
    let role = comp.var.variable_role();

    match role {
        VariableRole::RandomGate => {
            // Extract random activation trigger
            if let Some(activation) = random_activation {
                let trigger = TriggerCondition::RandomActivation {
                    activation_distance: activation.activation_distance,
                    phase_restriction: activation.phase_restriction,
                    corner_restriction: activation.corner_restriction,
                    slope_restriction: activation.slope_restriction,
                };
                (vec![trigger], None)
            } else {
                // No random activation available - treat as continuous
                (vec![TriggerCondition::Continuous], None)
            }
        }

        VariableRole::OnetimeEvent => {
            let trigger = extract_onetime_trigger(comp);
            (vec![trigger], None)
        }

        VariableRole::MonotonicGuard => {
            // MonotonicGuard: can be trigger OR guard depending on context
            // Default to guard here; promotion happens at higher level
            let guard = Condition {
                comp_list: CompList::Single(comp.clone()),
            };
            (vec![], Some(guard))
        }

        VariableRole::PhaseTrigger => {
            if let Some(trigger) = extract_phase_trigger(comp) {
                (vec![trigger], None)
            } else {
                (vec![TriggerCondition::Continuous], None)
            }
        }

        VariableRole::SpatialTrigger => {
            if let Some(trigger) = extract_spatial_trigger(comp) {
                (vec![trigger], None)
            } else {
                (vec![TriggerCondition::Continuous], None)
            }
        }

        VariableRole::StaticGuard | VariableRole::DynamicGuard => {
            // Static and Dynamic guards - keep as guard condition
            let guard = Condition {
                comp_list: CompList::Single(comp.clone()),
            };
            (vec![], Some(guard))
        }

        VariableRole::AlwaysTrue => {
            // Always true - no trigger or guard needed
            (vec![], None)
        }
    }
}

/// Extract onetime event trigger from comparison
fn extract_onetime_trigger(comp: &Comp) -> TriggerCondition {
    match &comp.var {
        VarType::Spatial(SpatialVar::IsLastStraightOnetime) => TriggerCondition::LastStraightEntry,
        VarType::Position(PositionVar::ChangeOrderOnetime) => {
            TriggerCondition::PositionChange { delta: comp.value }
        }
        VarType::Status(StatusVar::IsHpEmptyOnetime) => TriggerCondition::HpEmpty,
        _ => panic!("Not an onetime event: {:?}", comp.var),
    }
}

/// Extract monotonic trigger from comparison
fn extract_monotonic_trigger(comp: &Comp) -> TriggerCondition {
    match &comp.var {
        VarType::Phase(PhaseVar::AccumulateTime) => TriggerCondition::Time {
            threshold: comp.value as f64,
            operator: comp.op,
        },
        VarType::Distance(DistanceVar::DistanceRate) => TriggerCondition::DistanceCovered {
            threshold: comp.value as f64,
            operator: comp.op,
        },
        VarType::Distance(DistanceVar::RemainDistance) => TriggerCondition::DistanceRemaining {
            threshold: comp.value as f64,
            operator: comp.op,
        },
        VarType::Status(StatusVar::HpPer) => TriggerCondition::HpPercent {
            threshold: comp.value as f64,
            operator: comp.op,
        },
        _ => panic!("Not a monotonic trigger: {:?}", comp.var),
    }
}

/// Extract phase trigger from comparison
fn extract_phase_trigger(comp: &Comp) -> Option<TriggerCondition> {
    match &comp.var {
        VarType::Phase(PhaseVar::Phase) => {
            let phase = match comp.value {
                0 => Phase::Early,
                1 => Phase::Mid,
                2 => Phase::Late,
                3 => Phase::Last,
                _ => return None,
            };
            Some(TriggerCondition::PhaseChange {
                phase,
                operator: comp.op,
            })
        }
        _ => None,
    }
}

/// Extract spatial trigger from comparison
fn extract_spatial_trigger(comp: &Comp) -> Option<TriggerCondition> {
    match &comp.var {
        VarType::Spatial(SpatialVar::Corner) => {
            // corner!=0 means "any corner"
            // corner==X means "specific corner X"
            let corner_index = if comp.value == 0 {
                None // Any corner
            } else {
                Some((comp.value - 1) as usize) // Convert 1-indexed to 0-indexed
            };

            // For now, treat as entry trigger
            // TODO: Handle corner exit based on operator
            Some(TriggerCondition::CornerTransition {
                corner_index,
                is_entry: true,
            })
        }

        VarType::Spatial(SpatialVar::Slope) => {
            let slope_type = match comp.value {
                1 => SlopeType::Uphill,
                2 => SlopeType::Downhill,
                _ => SlopeType::Flat,
            };
            Some(TriggerCondition::SlopeTransition {
                slope_type,
                is_entry: true,
            })
        }

        VarType::Spatial(SpatialVar::IsLastStraight) => {
            Some(TriggerCondition::StraightTransition {
                is_final: true,
                is_entry: true,
            })
        }

        _ => {
            // Other spatial triggers - for now, mark as continuous
            Some(TriggerCondition::Continuous)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::condition::parser::parse_condition;

    #[test]
    fn test_split_simple_guard_only() {
        // Condition with only static guards (no triggers, no monotonic guards)
        let condition = parse_condition("ground_type==2&motivation>=4").unwrap();
        let branches = split_condition(&condition, None);

        // Should have 1 branch
        assert_eq!(branches.len(), 1);

        let (triggers, guards) = &branches[0];

        // Should have continuous trigger (fallback when no explicit triggers or monotonic guards)
        assert_eq!(triggers.len(), 1);
        assert!(matches!(triggers[0], TriggerCondition::Continuous));

        // Should preserve both guards
        assert!(guards.is_some());
    }

    #[test]
    fn test_split_hp_trigger_with_guards() {
        // Condition: hp_per<=50 & ground_type==2
        let condition = parse_condition("hp_per<=50&ground_type==2").unwrap();
        let branches = split_condition(&condition, None);

        // Should have 1 branch
        assert_eq!(branches.len(), 1);

        let (triggers, guards) = &branches[0];

        // Should extract HP trigger
        assert_eq!(triggers.len(), 1);
        assert!(matches!(
            triggers[0],
            TriggerCondition::HpPercent {
                threshold: 50.0,
                operator: CompOp::LessEqual
            }
        ));

        // Should have ground_type guard
        assert!(guards.is_some());
    }

    #[test]
    fn test_split_random_gate_with_guards() {
        // Condition: phase_random==1 & hp_per>=20
        let condition = parse_condition("phase_random==1&hp_per>=20").unwrap();

        // Simulate random activation
        let activation = RandomActivation {
            activation_distance: 800.0,
            phase_restriction: Some(Phase::Mid),
            corner_restriction: None,
            slope_restriction: None,
            activated: false,
        };

        let branches = split_condition(&condition, Some(&activation));

        // Should have 1 branch
        assert_eq!(branches.len(), 1);

        let (triggers, guards) = &branches[0];

        // Should have random gate trigger
        assert_eq!(triggers.len(), 1);
        assert!(matches!(
            triggers[0],
            TriggerCondition::RandomActivation { .. }
        ));

        // Should have hp_per guard (NOT phase_random)
        assert!(guards.is_some());
    }

    #[test]
    fn test_split_onetime_event() {
        // Condition: is_last_straight_onetime==1
        let condition = parse_condition("is_last_straight_onetime==1").unwrap();
        let branches = split_condition(&condition, None);

        // Should have 1 branch
        assert_eq!(branches.len(), 1);

        let (triggers, guards) = &branches[0];

        // Should extract onetime trigger
        assert_eq!(triggers.len(), 1);
        assert!(matches!(triggers[0], TriggerCondition::LastStraightEntry));

        // No guards
        assert!(guards.is_none());
    }

    #[test]
    fn test_split_or_condition() {
        // Condition: (phase_random==1 & hp_per>=20) @ (corner==2 & ground_type==1)
        // This should create 2 separate branches
        let condition =
            parse_condition("phase_random==1&hp_per>=20@corner!=0&ground_type==1").unwrap();

        let activation = RandomActivation {
            activation_distance: 800.0,
            phase_restriction: Some(Phase::Mid),
            corner_restriction: None,
            slope_restriction: None,
            activated: false,
        };

        let branches = split_condition(&condition, Some(&activation));

        // Should have 2 branches (one for each OR alternative)
        assert_eq!(branches.len(), 2);

        // Branch 1: random trigger + hp guard
        let (triggers1, guards1) = &branches[0];
        assert!(
            triggers1
                .iter()
                .any(|t| matches!(t, TriggerCondition::RandomActivation { .. }))
        );
        assert!(guards1.is_some());

        // Branch 2: corner trigger + ground_type guard
        let (triggers2, guards2) = &branches[1];
        assert!(
            triggers2
                .iter()
                .any(|t| matches!(t, TriggerCondition::CornerTransition { .. }))
        );
        assert!(guards2.is_some());
    }

    // Real skill tests based on actual skill_data.json entries

    #[test]
    fn test_real_skill_210101_phase_random_with_hp_guard() {
        // Based on skill ID 210101 (レースの真髄・体): "phase_random==3&hp_per>=2&is_lastspurt==1"
        // Tests Rule 1: MonotonicGuard (hp_per) stays as guard when paired with explicit trigger (phase_random)
        let condition = parse_condition("phase_random==3&hp_per>=2&is_lastspurt==1").unwrap();

        let activation = RandomActivation {
            activation_distance: 1500.0,
            phase_restriction: Some(Phase::Late),
            corner_restriction: None,
            slope_restriction: None,
            activated: false,
        };

        let branches = split_condition(&condition, Some(&activation));

        assert_eq!(branches.len(), 1);
        let (triggers, guards) = &branches[0];

        // Should have RandomActivation trigger
        assert_eq!(triggers.len(), 1);
        assert!(matches!(
            triggers[0],
            TriggerCondition::RandomActivation { .. }
        ));

        // hp_per and is_lastspurt should remain as guards
        assert!(guards.is_some());
    }

    #[test]
    fn test_real_skill_100141_hp_with_spatial_trigger() {
        // Based on skill ID 100141 (Victoria por plancha ☆): "is_last_straight==1&hp_per>=30&order<=2"
        // Tests Rule 1: MonotonicGuard (hp_per) stays as guard with spatial trigger (is_last_straight)
        let condition = parse_condition("is_last_straight==1&hp_per>=30&order<=2").unwrap();
        let branches = split_condition(&condition, None);

        assert_eq!(branches.len(), 1);
        let (triggers, guards) = &branches[0];

        // Should have StraightTransition trigger
        assert_eq!(triggers.len(), 1);
        assert!(matches!(
            triggers[0],
            TriggerCondition::StraightTransition { .. }
        ));

        // hp_per and order should remain as guards
        assert!(guards.is_some());
    }

    #[test]
    fn test_real_skill_100402111_accumulatetime_promoted() {
        // Based skill ID 100402111 (ドキッ☆じゃじゃウマ娘！): "running_style==1&slope==1&accumulatetime>=10"
        // Tests Rule 2: MonotonicGuard (accumulatetime) promoted to trigger when no explicit triggers
        let condition = parse_condition("running_style==1&slope==1&accumulatetime>=10").unwrap();
        let branches = split_condition(&condition, None);

        assert_eq!(branches.len(), 1);
        let (triggers, guards) = &branches[0];

        // Note: slope==1 creates a SlopeTransition trigger (spatial trigger),
        // so accumulatetime stays as guard per Rule 1
        // Updated test to match actual behavior
        assert_eq!(triggers.len(), 1);
        assert!(matches!(
            triggers[0],
            TriggerCondition::SlopeTransition { .. }
        ));

        // accumulatetime and running_style should remain as guards
        assert!(guards.is_some());
    }

    #[test]
    fn test_real_skill_210141_static_guards_only() {
        // Based on skill ID 210141 (レースの真髄・心): "motivation>=4"
        // Tests Rule 3: Only static guards -> Continuous trigger fallback
        let condition = parse_condition("motivation>=4").unwrap();
        let branches = split_condition(&condition, None);

        assert_eq!(branches.len(), 1);
        let (triggers, guards) = &branches[0];

        // Should have Continuous trigger (fallback)
        assert_eq!(triggers.len(), 1);
        assert!(matches!(triggers[0], TriggerCondition::Continuous));

        // motivation should remain as guard
        assert!(guards.is_some());
    }

    #[test]
    fn test_real_skill_101021_or_with_accumulatetime() {
        // Based on skill ID 101021 (ヴィヴァーチェ・ヴォラーレ); first alternative):
        // "distance_rate<=50&order_rate>=40&order_rate<=80&infront_near_lane_time>=3&accumulatetime>=10@
        //  distance_rate<=50&order_rate>=40&order_rate<=80&behind_near_lane_time>=3&accumulatetime>=10"
        // Tests Rule 2 with OR: Both branches should promote MonotonicGuards to triggers
        let condition = parse_condition(
            "distance_rate<=50&order_rate>=40&order_rate<=80&infront_near_lane_time>=3&accumulatetime>=10@\
             distance_rate<=50&order_rate>=40&order_rate<=80&behind_near_lane_time>=3&accumulatetime>=10"
        ).unwrap();

        let branches = split_condition(&condition, None);

        // Should have 2 branches (OR condition)
        assert_eq!(branches.len(), 2);

        // Both branches should have both distance_rate and accumulatetime promoted to triggers
        for (triggers, guards) in &branches {
            assert_eq!(triggers.len(), 2);
            // Should have both DistanceCovered and Time triggers
            assert!(
                triggers
                    .iter()
                    .any(|t| matches!(t, TriggerCondition::DistanceCovered { .. }))
            );
            assert!(
                triggers
                    .iter()
                    .any(|t| matches!(t, TriggerCondition::Time { .. }))
            );
            // Other conditions should remain as guards
            assert!(guards.is_some());
        }
    }
}
