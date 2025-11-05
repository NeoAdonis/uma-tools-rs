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
use crate::uma::skill::SkillEffect;

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
    /// NOTE: This only applies to onetime triggers (random gates, spatial events)
    /// Monotonic triggers can be retried until activated
    pub onetime_trigger_attempted: bool,
}

/// Monotonic variables that can have window conditions
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WindowVariable {
    /// Time elapsed since race start (seconds)
    AccumulateTime,
    /// Distance covered as percentage (0-100)
    DistanceRate,
    /// HP percentage (0-100)
    HpPercent,
}

/// A trigger condition that determines WHEN to check a skill
#[derive(Clone, Debug)]
pub enum TriggerCondition {
    // ========== MONOTONIC TRIGGERS ==========
    /// Time elapsed since race start
    Time { threshold: f64, operator: CompOp },

    /// Distance covered (as percentage 0-100)
    DistanceCovered { threshold: f64, operator: CompOp },

    /// Distance remaining (in meters)
    DistanceRemaining { threshold: f64, operator: CompOp },

    /// HP percentage (0-100)
    HpPercent { threshold: f64, operator: CompOp },

    /// Furlong crossed (distance in furlongs)
    FurlongCrossed { threshold: f64, operator: CompOp },

    // ========== WINDOW TRIGGERS ==========
    /// Window trigger: skill can activate within [min, max] range
    /// Checked continuously while within window
    /// Pattern: variable>=min & variable<=max
    Window {
        variable: WindowVariable,
        min_threshold: f64,
        max_threshold: f64,
    },

    // ========== PHASE TRIGGERS ==========
    /// Phase transition - fires once when entering the specified phase
    /// Note: phase>=2 and phase==2 both fire when Late phase begins
    PhaseChange { phase: Phase },

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
        is_last: bool, // true = only last straight
        is_entry: bool,
        is_onetime: bool, // true = one-time event
    },

    /// Position change in race
    PositionChange { delta: i32 }, // Negative = overtake, positive = overtaken

    // ========== CONTINUOUS ==========
    /// Must be checked every frame
    /// Used for conditions that can't be efficiently triggered
    Continuous,
}

/// A timed skill effect that is currently active
#[derive(Debug, Clone)]
pub struct ActiveTimedEffect {
    pub effect: SkillEffect,
    pub expires_at: f64, // Race time (seconds) when effect expires
}

/// Record of a skill activation during the race
#[derive(Debug, Clone)]
pub struct SkillActivationRecord {
    pub skill_id: String,
    pub distance: f64, // Distance when activated
}

impl TriggerCondition {
    /// Is this a one-time trigger that can only fire once?
    pub fn is_onetime(&self) -> bool {
        if let TriggerCondition::StraightTransition { is_onetime, .. } = self {
            *is_onetime
        } else {
            false
        }
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
    /// Returns false if already activated or one-time trigger already attempted
    pub fn should_check(&self) -> bool {
        if self.state.has_activated {
            return false; // Already activated
        }

        // For one-time triggers, check if we've already attempted
        // Monotonic triggers can be retried until activation
        let has_onetime = self.triggers.iter().any(|t| t.is_onetime());
        if has_onetime && self.state.onetime_trigger_attempted {
            return false; // One-time trigger already fired
        }

        true
    }

    /// Mark that we've attempted this trigger (for one-time triggers only)
    pub fn mark_attempted(&mut self) {
        self.state.onetime_trigger_attempted = true;
    }

    /// Mark that skill has successfully activated
    pub fn mark_activated(&mut self) {
        self.state.has_activated = true;
        self.state.onetime_trigger_attempted = true;
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
    random_activation: Vec<f64>,
) -> Vec<(Vec<TriggerCondition>, Option<Condition>)> {
    let branches = split_comp_list(&condition.comp_list, &random_activation);

    // Apply promotion logic to each branch at the top level
    branches
        .into_iter()
        .map(|(triggers, guards)| apply_monotonic_promotion(triggers, guards))
        .collect()
}

/// Try to detect a window pattern: variable>=min & variable<=max
/// Returns Some((window_trigger, remaining_guard)) if detected, None otherwise
fn try_detect_window(
    left: &CompList,
    right: &CompList,
) -> Option<(TriggerCondition, Option<Condition>)> {
    // We're looking for single comparisons on both sides
    if let (CompList::Single(left_comp), CompList::Single(right_comp)) = (left, right) {
        // Check if both comparisons use the same variable
        let left_var = match &left_comp.var {
            VarType::AccumulateTime => Some(WindowVariable::AccumulateTime),
            VarType::Spatial(SpatialVar::DistanceRate) => Some(WindowVariable::DistanceRate),
            VarType::Status(StatusVar::HpPer) => Some(WindowVariable::HpPercent),
            _ => None,
        };

        let right_var = match &right_comp.var {
            VarType::AccumulateTime => Some(WindowVariable::AccumulateTime),
            VarType::Spatial(SpatialVar::DistanceRate) => Some(WindowVariable::DistanceRate),
            VarType::Status(StatusVar::HpPer) => Some(WindowVariable::HpPercent),
            _ => None,
        };

        // Check if same variable with >= and <= operators
        if let (Some(var), Some(right_var_check)) = (left_var, right_var)
            && var == right_var_check
        {
            // Check for pattern: left>=X & right<=Y
            if left_comp.op == CompOp::GreaterEqual && right_comp.op == CompOp::LessEqual {
                return Some((
                    TriggerCondition::Window {
                        variable: var,
                        min_threshold: left_comp.value as f64,
                        max_threshold: right_comp.value as f64,
                    },
                    None, // No remaining guards
                ));
            }
            // Check for pattern: left<=Y & right>=X (reversed order)
            if left_comp.op == CompOp::LessEqual && right_comp.op == CompOp::GreaterEqual {
                return Some((
                    TriggerCondition::Window {
                        variable: var,
                        min_threshold: right_comp.value as f64,
                        max_threshold: left_comp.value as f64,
                    },
                    None, // No remaining guards
                ));
            }
        }
    }

    None
}

/// Recursively split a CompList, handling OR branches
fn split_comp_list(
    comp_list: &CompList,
    random_activation: &Vec<f64>,
) -> Vec<(Vec<TriggerCondition>, Option<Condition>)> {
    match comp_list {
        CompList::Single(comp) => {
            // Base case: single comparison
            // DON'T apply promotion here - it will be done after combining
            vec![split_single_comp(comp, random_activation)]
        }

        CompList::Binary { left, op, right } => match op {
            BinOp::And => {
                // Try to detect window pattern first (before splitting further)
                // Note: try_detect_window only matches direct AND of two comparisons
                // If there are additional conditions, they'll be handled below
                if let Some((window_trigger, _)) = try_detect_window(left, right) {
                    // Window detected! Return it as a single trigger with no guards
                    // (Additional guards would require more complex pattern like: (A>=X & A<=Y) & B
                    // which isn't matched by our simple pattern detector)
                    return vec![(vec![window_trigger], None)];
                }

                // Not a simple window, proceed with normal AND processing
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
/// 1. If there are DynamicGuards, add Continuous trigger to ensure constant checking
/// 2. If there are explicit triggers (RandomGate/Spatial/Phase), MonotonicGuards stay as guards
/// 3. If no explicit triggers exist, promote MonotonicGuards to triggers
/// 4. If only guards remain after promotion, add Continuous trigger
fn apply_monotonic_promotion(
    mut triggers: Vec<TriggerCondition>,
    guard_condition: Option<Condition>,
) -> (Vec<TriggerCondition>, Option<Condition>) {
    // Check if we have explicit triggers (not Continuous)
    let has_explicit_trigger = triggers
        .iter()
        .any(|t| !matches!(t, TriggerCondition::Continuous));

    // Check if guards contain any DynamicGuard variables
    let has_dynamic_guard = contains_dynamic_guard(&guard_condition);

    if has_dynamic_guard {
        // Rule 1. Dynamic guards exist → must be checked continuously; add Continuous trigger
        (vec![TriggerCondition::Continuous], guard_condition)
    } else if has_explicit_trigger {
        // Rule 2: Explicit triggers exist → MonotonicGuards stay as guards
        (triggers, guard_condition)
    } else {
        // Rule 3: No explicit triggers → try to promote MonotonicGuards
        if let Some(guard) = guard_condition {
            let (promoted_triggers, remaining_guards) = promote_monotonic_guards(&guard);

            if !promoted_triggers.is_empty() {
                // Successfully promoted some MonotonicGuards to triggers
                triggers.extend(promoted_triggers);
                (triggers, remaining_guards)
            } else {
                // Rule 4: No MonotonicGuards to promote → Continuous trigger
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

/// Check if a condition contains any DynamicGuard variables
fn contains_dynamic_guard(condition: &Option<Condition>) -> bool {
    if let Some(cond) = condition {
        check_comp_list_for_dynamic_guard(&cond.comp_list)
    } else {
        false
    }
}

/// Recursively check if a CompList contains DynamicGuard variables
fn check_comp_list_for_dynamic_guard(comp_list: &CompList) -> bool {
    match comp_list {
        CompList::Single(comp) => comp.var.variable_role() == VariableRole::DynamicGuard,
        CompList::Binary { left, right, .. } => {
            check_comp_list_for_dynamic_guard(left) || check_comp_list_for_dynamic_guard(right)
        }
    }
}

/// Split a single comparison into trigger and guard
///
/// Note: This function does NOT make promotion decisions for MonotonicGuard.
/// It treats them as guards by default. Promotion happens at a higher level
/// in split_comp_list after analyzing all conditions together.
fn split_single_comp(
    comp: &Comp,
    random_activation: &[f64],
) -> (Vec<TriggerCondition>, Option<Condition>) {
    let role = comp.var.variable_role();

    match role {
        VariableRole::RandomGate => {
            // Extract random activation trigger
            if let VarType::Random(_) = &comp.var {
                // If we are here, it means that the activation roll succeeded
                // → create a trigger that always fires
                let guard = Condition {
                    comp_list: CompList::Single(Comp {
                        var: VarType::Always,
                        op: CompOp::Equal,
                        value: 1,
                    }),
                };
                (vec![], Some(guard))
            } else {
                // Create trigger based on random activation distance
                let trigger = TriggerCondition::DistanceCovered {
                    threshold: random_activation[0],
                    operator: comp.op,
                };
                // TODO: remove the used activation distance
                (vec![trigger], None)
            }
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
            // Phase with comparison operators (<=, >=, <, >) should be guards, not triggers
            // Only phase==N creates a trigger (fires at phase entry)
            // phase<=1 means "while in Early or Mid" (check at trigger time)
            if comp.op == CompOp::Equal {
                // phase==N → PhaseTrigger (fire at phase entry)
                if let Some(trigger) = extract_phase_trigger(comp) {
                    (vec![trigger], None)
                } else {
                    (vec![TriggerCondition::Continuous], None)
                }
            } else {
                // phase<=N, phase>=N, etc. → DynamicGuard (check at trigger time)
                let guard = Condition {
                    comp_list: CompList::Single(comp.clone()),
                };
                (vec![], Some(guard))
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
    }
}

/// Extract monotonic trigger from comparison
///
/// Detects operator direction mismatches and converts them to window triggers:
/// - Increasing monotonic variables (distance_rate, accumulatetime) with <= → Window [0, threshold]
/// - Decreasing monotonic variables (hp_per, remain_distance) with >= → Window [threshold, MAX]
fn extract_monotonic_trigger(comp: &Comp) -> TriggerCondition {
    // Helper to determine if this is a reversed operator that needs window conversion
    let needs_window_conversion = |is_increasing: bool, op: CompOp| -> bool {
        match (is_increasing, op) {
            // Increasing variable with <= or < → needs window [0, threshold]
            (true, CompOp::LessEqual | CompOp::Less) => true,
            // Decreasing variable with >= or > → needs window [threshold, MAX]
            (false, CompOp::GreaterEqual | CompOp::Greater) => true,
            _ => false,
        }
    };

    match &comp.var {
        VarType::AccumulateTime => {
            // AccumulateTime is monotonically increasing
            if needs_window_conversion(true, comp.op) {
                TriggerCondition::Window {
                    variable: WindowVariable::AccumulateTime,
                    min_threshold: 0.0,
                    max_threshold: comp.value as f64,
                }
            } else {
                TriggerCondition::Time {
                    threshold: comp.value as f64,
                    operator: comp.op,
                }
            }
        }

        VarType::Spatial(SpatialVar::DistanceRate) => {
            // DistanceRate is monotonically increasing
            if needs_window_conversion(true, comp.op) {
                TriggerCondition::Window {
                    variable: WindowVariable::DistanceRate,
                    min_threshold: 0.0,
                    max_threshold: comp.value as f64,
                }
            } else {
                TriggerCondition::DistanceCovered {
                    threshold: comp.value as f64,
                    operator: comp.op,
                }
            }
        }

        VarType::Spatial(SpatialVar::RemainDistance) => {
            // RemainDistance is monotonically decreasing
            // Note: <= is normal direction for decreasing, >= is reversed
            if needs_window_conversion(false, comp.op) {
                // This case is rare: remain_distance>=X means "while still far from finish"
                // Window from threshold to infinity (represented as course length)
                // Convert to distance rate for window
                TriggerCondition::Window {
                    variable: WindowVariable::DistanceRate,
                    min_threshold: 0.0,
                    max_threshold: 100.0 - (comp.value as f64),
                }
            } else {
                TriggerCondition::DistanceRemaining {
                    threshold: comp.value as f64,
                    operator: comp.op,
                }
            }
        }

        VarType::Status(StatusVar::HpPer) => {
            // Even though HpPer can recover, for the most part it constantly decreases
            // Treat as decreasing for window conversion purposes
            if needs_window_conversion(false, comp.op) {
                TriggerCondition::Window {
                    variable: WindowVariable::HpPercent,
                    min_threshold: comp.value as f64,
                    max_threshold: 100.0,
                }
            } else {
                TriggerCondition::HpPercent {
                    threshold: comp.value as f64,
                    operator: comp.op,
                }
            }
        }

        VarType::Status(StatusVar::IsHpEmptyOnetime) => TriggerCondition::HpPercent {
            threshold: 0.0,
            operator: CompOp::LessEqual,
        },

        VarType::Spatial(SpatialVar::Furlong) => {
            // Furlong is monotonically increasing
            if needs_window_conversion(true, comp.op) {
                // furlong<=N means "activate while in first N furlongs"
                // We don't have a furlong window type, so fall back to continuous
                // (This is rare enough that it doesn't warrant a new window type)
                TriggerCondition::FurlongCrossed {
                    threshold: comp.value as f64,
                    operator: comp.op,
                }
            } else {
                TriggerCondition::FurlongCrossed {
                    threshold: comp.value as f64,
                    operator: comp.op,
                }
            }
        }

        _ => panic!("Not a monotonic trigger: {:?}", comp.var),
    }
}

/// Extract phase trigger from comparison
///
/// Phase triggers fire once when entering the specified phase.
/// The comparison operator is ignored - both `phase==2` and `phase>=2`
/// fire when Late phase begins (at the phase boundary).
///
/// This is semantically correct because:
/// - Phases are discrete states, not continuous values
/// - Skills activate on state transitions (Early→Mid→Late→Last)
/// - `phase>=2` conceptually means "when in Late or Last phase"
/// - The trigger fires at Late entry; the skill remains active during Late+Last
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
            Some(TriggerCondition::PhaseChange { phase })
        }
        _ => None,
    }
}

/// Extract spatial trigger from comparison
fn extract_spatial_trigger(comp: &Comp) -> Option<TriggerCondition> {
    if let VarType::Spatial(var_type) = &comp.var {
        match var_type {
            SpatialVar::Corner => {
                match (comp.op, comp.value) {
                    // corner==N [N>0] → Entry to specific corner N
                    (CompOp::Equal, n) if n > 0 => {
                        Some(TriggerCondition::CornerTransition {
                            corner_index: Some((n - 1) as usize), // Convert 1-indexed to 0-indexed
                            is_entry: true,
                        })
                    }

                    // corner!=0 → Entry to any corner
                    (CompOp::NotEqual, 0) => {
                        Some(TriggerCondition::CornerTransition {
                            corner_index: None, // Any corner
                            is_entry: true,
                        })
                    }

                    // corner==0 (not a corner) or corner!=N [N>0] (anywhere except corner N) → Not a trigger
                    _ => None,
                }
            }
            SpatialVar::Slope => {
                match (comp.op, comp.value) {
                    // slope==1 → Entry to uphill
                    (CompOp::Equal, 1) => Some(TriggerCondition::SlopeTransition {
                        slope_type: SlopeType::Uphill,
                        is_entry: true,
                    }),

                    // slope==2 → Entry to downhill
                    (CompOp::Equal, 2) => Some(TriggerCondition::SlopeTransition {
                        slope_type: SlopeType::Downhill,
                        is_entry: true,
                    }),

                    // slope==0 (flat) or slope!=X (anywhere but uphill/downhill) → Not a trigger
                    _ => None,
                }
            }
            SpatialVar::IsLastStraight => {
                // is_last_straight==1 → Entry to final straight
                if comp.op == CompOp::Equal && comp.value == 1 {
                    Some(TriggerCondition::StraightTransition {
                        is_last: true,
                        is_entry: true,
                        is_onetime: false,
                    })
                } else {
                    // Other operators treated as continuous guards
                    None
                }
            }
            SpatialVar::IsLastStraightOnetime => {
                // is_last_straight_onetime==1 → Entry to final straight
                if comp.op == CompOp::Equal && comp.value == 1 {
                    Some(TriggerCondition::StraightTransition {
                        is_last: true,
                        is_entry: true,
                        is_onetime: true,
                    })
                } else {
                    // Other operators treated as continuous guards
                    None
                }
            }
            SpatialVar::StraightFrontType => {
                // straight_front_type==1 → Entry to audience-side straight
                // straight_front_type==2 → Entry to opposite-side straight
                if comp.op == CompOp::Equal && (comp.value == 1 || comp.value == 2) {
                    Some(TriggerCondition::StraightTransition {
                        is_last: false, // Not specifically the final straight
                        is_entry: true,
                        is_onetime: false,
                    })
                } else {
                    // Not a trigger
                    None
                }
            }
            _ => None,
        }
    } else {
        None
    }
}

#[cfg(test)]
mod tests; // Tests are in skill_activation/tests.rs
