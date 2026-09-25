//! # Skill Activation Logic
//!
//! This module implements the refined skill activation system that separates
//! **triggers** (which determine WHEN to check a skill) from **guards**
//! (which must be true AT activation time).
//!
//! ## Overview
//!
//! The skill activation logic is divided into the following main components:
//!
//! **Triggers** determine WHEN to evaluate a skill.
//!
//!   - One-time triggers: Fire once at a specific point. Obtained mainly from random
//!     conditions; activation points are pre-rolled.
//!   - Continuous triggers: Checked every frame. Used for conditions that can change
//!     at any time.
//!   - Window triggers: Active within a range (e.g., distance or time windows).
//!   - Monotonic triggers: Obtained by promoting monotonic guard conditions (values
//!     that only increase or decrease, e.g., distance, time, HP) to triggers when no
//!     explicit triggers exist and no dynamic guards are present.
//!
//! **Guards** are evaluated ONLY when trigger fires.
//!
//!    - Represented as a flat list (`Vec<`[`Comparison``]`>`) - all of these must be true
//!      for a skill to activate.
//!    - OR operations are handled by splitting condition trees into separate
//!      lists (branches).
//!    - Can contain static guards - conditions that never change and can be
//!      pre-evaluated before the race starts (e.g, ground type, running style).
//!    - It can also contain dynamic guards - conditions to be evaluated at activation
//!      time (e.g., HP, rush status). Some of these can be promoted to triggers if
//!      they are monotonic.
//!
//! **Preconditions** unlock skills permanently (evaluated every frame).
//!
//!    - Allows event-based activation gates (e.g., "activate after overtaking
//!      someone").
//!    - Checked before triggers/guards can be evaluated.
//!    - Once satisfied, skill remains unlocked for the rest of the race for
//!      activation attempts.
//!
//! **Activation State** tracks whether skill has fired.
//!
//!    - One-time triggers mark skill as "attempted" after first check.
//!    - Continuous triggers can be checked multiple times.
//!
//! ## Examples
//!
//! Consider a skill with condition: `hp_per>=20 & is_finalcorner_random==1`
//!
//! - **Trigger**: `is_finalcorner_random` pre-rolls to distance 1850m
//!   (within final corner).
//! - **Guards**: `[hp_per>=20]` - must be true when we reach 1850m.
//!
//! Consider another skill with condition: `distance_rate>=80 & ground_type==2`
//!
//! - **Trigger**: `distance_rate>=80` (promoted because no explicit triggers exist).
//! - **Guards**: `[ground_type==2]` - checked before the race starts.
//!
//! Now consider a skill with **precondition**: `change_order_onetime<0`
//!
//! - **Every frame**: Check if overtaken anyone. Once true, skill unlocked.
//! - **At trigger time** (e.g., 1850m): If unlocked, evaluate guards → activate.
//! - **If never unlocked**: Trigger fires but skill never activates.

use crate::condition::parser::*;
use crate::condition::variables::*;
use crate::course::*;
use crate::uma::skill::SkillEffect;

/// Complete activation logic for a skill.
///
/// Combines triggers (WHEN to check), guards (IF to activate), and preconditions
/// (unlock gates) in a single structure.
#[derive(Clone, Debug)]
pub struct SkillActivationLogic {
    /// Index into runner.available_skills.
    pub skill_index: usize,

    /// Which alternative of this skill (skills can have multiple condition sets).
    pub alternative_index: usize,

    /// Triggers that determine WHEN to check this skill
    pub triggers: Vec<TriggerCondition>,

    /// Guards that must ALL be true AT activation time.
    /// Empty list means no guard checks needed (always activates when trigger fires).
    pub guards: ComparisonList,

    /// Precondition that unlocks the skill (permanent one-time gate).
    /// Checked every frame until satisfied; once true, skill remains unlocked for
    /// the race.
    /// Enables event-based activation (e.g., "activate after overtaking someone").
    pub precondition: Option<Condition>,

    /// Activation state tracking.
    pub state: ActivationState,
}

/// Tracks whether this skill has been checked/activated.
#[derive(Clone, Debug, Default)]
pub struct ActivationState {
    /// Has this skill successfully activated?
    pub has_activated: bool,

    /// For one-time triggers: has the trigger fired (even if guards failed)?
    /// Prevents re-checking skills that failed their guards.
    /// N.B. This only applies to one-time triggers like random gates.
    /// Monotonic triggers can be retried until activated.
    pub onetime_trigger_attempted: bool,
}

/// Monotonic variables that can have window conditions.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WindowVariable {
    /// Time elapsed since race start (seconds).
    AccumulateTime,
    /// Distance covered as percentage (0-100).
    DistanceRate,
    /// HP percentage (0-100).
    HpPercent,
    /// Spatial position (meters from start).
    /// Used for corners, slopes, straights.
    SpatialPosition,
}

/// A trigger condition that determines WHEN to check a skill.
#[derive(Clone, Debug)]
pub enum TriggerCondition {
    // ========== MONOTONIC TRIGGERS ==========
    /// Time elapsed since race start.
    Time { threshold: f64, operator: CompOp },

    /// Distance covered (as percentage 0-100).
    DistanceCovered { threshold: f64, operator: CompOp },

    /// Distance remaining (in meters).
    DistanceRemaining { threshold: f64, operator: CompOp },

    /// HP percentage (0-100).
    HpPercent { threshold: f64, operator: CompOp },

    /// Furlong crossed (distance in furlongs)
    FurlongCrossed { threshold: f64, operator: CompOp },

    // ========== WINDOW TRIGGERS ==========
    /// Window trigger: skill can activate within [min, max] range.
    /// Checked continuously while within window.
    /// Pattern: variable>=min & variable<=max.
    Window {
        variable: WindowVariable,
        min_threshold: f64,
        max_threshold: f64,
    },

    // ========== PHASE TRIGGERS ==========
    /// Phase transition - fires once when entering the specified phase.
    /// Note: phase>=2 and phase==2 both fire when Late phase begins.
    PhaseChange { phase: Phase },

    // ========== SPATIAL TRIGGERS ==========
    /// Entering or exiting a straight.
    StraightTransition {
        is_last: bool, // true = only last straight
        is_entry: bool,
        is_onetime: bool, // true = one-time event
    },

    // ========== DEFERRED RANDOM TRIGGERS ==========
    /// Random point after Full-Throttle Sprint starts. The point is rolled when
    /// the sprint starts and scheduled as a distance trigger.
    /// Used for `run_at_full_speed_random==1` conditions.
    FullThrottleStart,

    // ========== CONTINUOUS ==========
    /// Must be checked every frame.
    /// Used for conditions that can't be efficiently triggered.
    Continuous,

    // ========== ACTIVATION-DEPENDENT TRIGGERS ==========
    /// Fires when any skill activates.
    /// Used for is_activate_any_skill==1 conditions.
    AnySkillActivation,

    /// Fires when a buff skill activates.
    /// Used for is_other_character_activate_advantage_skill==1 conditions.
    BuffSkillActivation,

    /// Fires when a heal skill activates.
    /// Used for is_activate_heal_skill==1 conditions.
    HealSkillActivation,
}

/// A timed skill effect that is currently active.
#[derive(Debug, Clone)]
pub struct ActiveTimedEffect {
    /// Skill effect being applied.
    pub effect: SkillEffect,
    /// Final modifier value (already divided by 10000 and including any
    /// activation-time bonus such as the Wiz Limit Break buff).
    pub value: f64,
    /// Race time in seconds when effect expires.
    pub expires_at: f64,
}

/// Record of a skill activation during the race.
#[derive(Debug, Clone)]
pub struct SkillActivationRecord {
    /// Skill ID of the activated skill from the global skill list.
    pub skill_id: String,
    /// Distance when the skill was activated (in meters from start).
    pub distance: f64,
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
    /// Create a new skill activation logic.
    pub fn new(
        skill_index: usize,
        alternative_index: usize,
        triggers: Vec<TriggerCondition>,
        guards: ComparisonList,
        precondition: Option<Condition>,
    ) -> Self {
        SkillActivationLogic {
            skill_index,
            alternative_index,
            triggers,
            guards,
            precondition,
            state: ActivationState::default(),
        }
    }

    /// Should this skill be checked at all?
    /// Returns false if already activated or one-time trigger already attempted.
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

    /// Mark that we've attempted this trigger (for one-time triggers only).
    pub fn mark_attempted(&mut self) {
        self.state.onetime_trigger_attempted = true;
    }

    /// Mark that skill has successfully activated.
    pub fn mark_activated(&mut self) {
        self.state.has_activated = true;
        self.state.onetime_trigger_attempted = true;
    }
}

/// Split a condition tree into triggers and guards
///
/// This is the core algorithm that separates conditions based on their role:
/// - Triggers determine WHEN to check
/// - Guards must be all true AT check time
///
/// N.B. Preconditions are NOT handled here; they're separated earlier in the parsing
/// pipeline (see `builder.rs`). This function only splits the main `condition` field.
///
/// # OR Handling
///
/// For conditions with OR (`@`), this returns **multiple** trigger-guard pairs.
/// The caller should create separate SkillActivationLogic for each OR branch.
/// Each branch's guards are guaranteed to be a pure AND conjunction (flat list).
///
/// # Example
///
/// Input:
///
/// ```text
/// phase_random==1 & hp_per>=20
/// @
/// corner==2 & ground_type==1
/// ```
///
/// Output (2 branches):
///
/// - Branch 1: triggers=[DistanceCovered], guards=[hp_per>=20]
/// - Branch 2: triggers=[DistanceCovered], guards=[ground_type==1]
///
/// # Simple Example
///
/// Input: `hp_per>=20 & is_finalcorner_random==1 & ground_type==2`
///
/// Output (1 branch):
///
/// - Triggers: `[DistanceCovered { distance==1850m }]`
/// - Guards: `[hp_per>=20, ground_type==2]`
pub fn split_condition(
    condition: &Condition,
    random_activation: Vec<f64>,
    course: &Course,
) -> Vec<(Vec<TriggerCondition>, ComparisonList)> {
    let branches = split_comp_tree(&condition.comp_tree, &random_activation, course);

    // Apply promotion logic to each branch at the top level
    branches
        .into_iter()
        .map(|(mut triggers, guards)| {
            // Check if we already have explicit triggers (Window, Random, Spatial, Phase, etc.)
            let has_explicit_trigger = triggers
                .iter()
                .any(|t| !matches!(t, TriggerCondition::Continuous));

            // Try to promote a single monotonic guard to a window, but only if:
            // 1. We DON'T already have explicit triggers
            // 2. We DO have guards
            // 3. The guards contain at least one dynamic guard
            //
            // This optimization handles the specific case where we have BOTH
            // monotonic guards (that could become windows) AND dynamic guards
            // (that would normally force Continuous trigger in Rule 2).
            let has_dynamic_guard = contains_dynamic_guard_in_list(&guards);
            let remaining_guards =
                if !has_explicit_trigger && !guards.is_empty() && has_dynamic_guard {
                    let (window_trigger, new_guards) =
                        try_promote_single_monotonic_to_window(guards);
                    if let Some(window) = window_trigger {
                        triggers.push(window);
                    }
                    new_guards
                } else {
                    // Keep original guards - let apply_monotonic_promotion() handle it
                    guards
                };

            // Then apply standard monotonic promotion rules
            // If we promoted a window above, Rule 1 will keep it (explicit trigger exists)
            let (final_triggers, final_guards) =
                apply_monotonic_promotion(triggers, remaining_guards);

            // Finally, remove all Continuous triggers if we have any explicit triggers
            let cleaned_triggers = if final_triggers
                .iter()
                .any(|t| !matches!(t, TriggerCondition::Continuous))
            {
                final_triggers
                    .into_iter()
                    .filter(|t| !matches!(t, TriggerCondition::Continuous))
                    .collect()
            } else {
                final_triggers
            };
            (cleaned_triggers, final_guards)
        })
        .collect()
}

/// Try to detect a window pattern: `variable>=min & variable<=max`.
/// Returns `Some((window_trigger, remaining_guards))` if detected, `None` otherwise.
fn try_detect_window(
    left: &ComparisonTree,
    right: &ComparisonTree,
) -> Option<(TriggerCondition, ComparisonList)> {
    use VarType::*;

    // We're looking for single comparisons on both sides
    if let (ComparisonTree::Single(left_comp), ComparisonTree::Single(right_comp)) = (left, right) {
        // Check if both comparisons use the same variable
        let left_var = match &left_comp.var {
            AccumulateTime => Some(WindowVariable::AccumulateTime),
            Spatial(SpatialVar::DistanceRate) => Some(WindowVariable::DistanceRate),
            Status(StatusVar::HpPer) => Some(WindowVariable::HpPercent),
            _ => None,
        };

        let right_var = match &right_comp.var {
            AccumulateTime => Some(WindowVariable::AccumulateTime),
            Spatial(SpatialVar::DistanceRate) => Some(WindowVariable::DistanceRate),
            Status(StatusVar::HpPer) => Some(WindowVariable::HpPercent),
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
                    vec![], // No remaining guards
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
                    vec![], // No remaining guards
                ));
            }
        }
    }

    None
}

/// Recursively split a `ComparisonTree`, handling OR branches.
/// Returns a list of (triggers, guards) where guards is a flat list of comparisons.
fn split_comp_tree(
    comp_tree: &ComparisonTree,
    random_activation: &Vec<f64>,
    course: &Course,
) -> Vec<(Vec<TriggerCondition>, ComparisonList)> {
    match comp_tree {
        ComparisonTree::Single(comp) => {
            // Base case: single comparison
            // DON'T apply promotion here - it will be done after combining
            vec![split_single_comp(comp, random_activation, course)]
        }

        ComparisonTree::Binary { left, op, right } => match op {
            BinOp::And => {
                // Try to detect window pattern first (before splitting further)
                // Note: try_detect_window only matches direct AND of two comparisons
                // If there are additional conditions, they'll be handled below
                if let Some((window_trigger, guards)) = try_detect_window(left, right) {
                    // Window detected! Return it as a single trigger with no guards
                    // (Additional guards would require more complex pattern like: (A>=X & A<=Y) & B
                    // which isn't matched by our simple pattern detector)
                    return vec![(vec![window_trigger], guards)];
                }

                // Not a simple window, proceed with normal AND processing
                // AND: Combine triggers and guards from both sides
                let left_splits = split_comp_tree(left, random_activation, course);
                let right_splits = split_comp_tree(right, random_activation, course);

                // For AND, we should have exactly one split on each side
                // (unless there are nested ORs, which we'll handle recursively)
                let mut results = Vec::new();

                for (left_triggers, left_guards) in left_splits {
                    for (right_triggers, right_guards) in &right_splits {
                        // Combine triggers
                        let mut combined_triggers = left_triggers.clone();
                        combined_triggers.extend(right_triggers.clone());

                        // Combine guards by concatenating the lists (all must be true)
                        let mut combined_guards = left_guards.clone();
                        combined_guards.extend(right_guards.clone());

                        // Promotion will be applied at top level
                        results.push((combined_triggers, combined_guards));
                    }
                }

                results
            }

            BinOp::Or => {
                // OR: Create separate branches
                let mut results = Vec::new();

                // Add all branches from left side
                results.extend(split_comp_tree(left, random_activation, course));

                // Add all branches from right side
                results.extend(split_comp_tree(right, random_activation, course));

                results
            }
        },
    }
}

/// Apply monotonic guard promotion rules:
///
/// 1. If there are explicit triggers (Random/Spatial/Window), keep them regardless
///    of guards.
/// 2. If no explicit triggers AND there are dynamic guards, use Continuous trigger.
/// 3. If no explicit triggers AND no dynamic guards, try to promote monotonic guards
///    to triggers.
/// 4. If only guards remain after promotion, add Continuous trigger.
fn apply_monotonic_promotion(
    mut triggers: Vec<TriggerCondition>,
    guards: ComparisonList,
) -> (Vec<TriggerCondition>, ComparisonList) {
    // Check if we have explicit triggers (not Continuous)
    let has_explicit_trigger = triggers
        .iter()
        .any(|t| !matches!(t, TriggerCondition::Continuous));

    // Check if guards contain any DynamicGuard variables
    let has_dynamic_guard = contains_dynamic_guard_in_list(&guards);

    if has_explicit_trigger {
        // Rule 1: Explicit triggers exist → keep them, guards stay as guards
        // This includes Window, Random, Spatial, Phase triggers
        // DynamicGuards don't force Continuous when we already have explicit triggers
        (triggers, guards)
    } else if has_dynamic_guard {
        // Rule 2: No explicit triggers + DynamicGuards → use Continuous trigger
        (vec![TriggerCondition::Continuous], guards)
    } else {
        // Rule 3: No explicit triggers, no DynamicGuards → try to promote MonotonicGuards
        if !guards.is_empty() {
            let (promoted_triggers, remaining_guards) = promote_monotonic_guards(&guards);

            if !promoted_triggers.is_empty() {
                // Successfully promoted some MonotonicGuards to triggers
                triggers.extend(promoted_triggers);
                (triggers, remaining_guards)
            } else {
                // Rule 4: No MonotonicGuards to promote → Continuous trigger
                if triggers.is_empty() {
                    triggers.push(TriggerCondition::Continuous);
                }
                (triggers, guards)
            }
        } else {
            // No guards at all
            if triggers.is_empty() {
                // No triggers, no guards → Continuous (shouldn't happen often)
                triggers.push(TriggerCondition::Continuous);
            }
            (triggers, guards)
        }
    }
}

/// Extract `MonotonicGuard`s from guard list and promote them to triggers.
///
/// Returns: `(promoted triggers, remaining guards)`
fn promote_monotonic_guards(guards: &ComparisonList) -> (Vec<TriggerCondition>, ComparisonList) {
    let mut promoted_triggers = Vec::new();
    let mut remaining_guards = Vec::new();

    // Classify each comparison
    for comp in guards {
        let role = comp.var.variable_role();
        if matches!(role, VariableRole::MonotonicGuard) {
            // Promote to trigger
            let trigger = extract_monotonic_trigger(comp);
            promoted_triggers.push(trigger);

            // For activate_count_* variables (and other activation counters),
            // keep the comparison as a guard because they promote to Continuous
            // triggers that must check the guard condition every frame
            if matches!(comp.var, VarType::Activation(_)) {
                remaining_guards.push(comp.clone());
            }
        } else {
            // Keep as guard
            remaining_guards.push(comp.clone());
        }
    }

    (promoted_triggers, remaining_guards)
}

/// Get the promotion priority for a monotonic variable as follows:
///
/// - [`DistanceRate`](SpatialVar::DistanceRate): 3 (highest - most commonly used spatial trigger)
/// - [`AccumulateTime`](VarType::AccumulateTime): 2 (medium - time-based progression)
/// - [`HpPer`](StatusVar::HpPer): 1 (lowest - less predictable due to recovery)
/// - Others: 0 (not promotable)
///
/// Higher priority variables are preferred when promoting a single monotonic
/// guard to a window trigger in the presence of dynamic guards.
fn get_promotion_priority(var: &VarType) -> i32 {
    use VarType::*;
    match var {
        Spatial(SpatialVar::DistanceRate) => 3,
        AccumulateTime => 2,
        Status(StatusVar::HpPer) => 1,
        _ => 0,
    }
}

/// Try to promote a single monotonic guard to a window trigger.
///
/// This is an optimization for conditions with both monotonic and dynamic guards.
/// Instead of using a Continuous trigger that checks all conditions every frame,
/// we can promote the highest-priority monotonic guard to a Window trigger,
/// reducing per-frame evaluation overhead.
///
/// Returns: `(promoted window trigger, remaining guards)`
///
/// If no suitable monotonic guard is found, returns `(None, original guards)`.
fn try_promote_single_monotonic_to_window(
    guards: ComparisonList,
) -> (Option<TriggerCondition>, ComparisonList) {
    use VarType::*;

    if guards.is_empty() {
        return (None, vec![]);
    }

    // Collect all monotonic guards with their priorities
    let mut monotonic_candidates: Vec<(i32, Comparison)> = Vec::new();
    for comp in &guards {
        let role = comp.var.variable_role();
        if matches!(role, VariableRole::MonotonicGuard) {
            let priority = get_promotion_priority(&comp.var);
            if priority > 0 {
                monotonic_candidates.push((priority, comp.clone()));
            }
        }
    }

    if monotonic_candidates.is_empty() {
        return (None, guards);
    }

    // Sort by priority (highest first)
    monotonic_candidates.sort_by_key(|a| std::cmp::Reverse(a.0));

    // Take the highest priority candidate
    let (_priority, best_comp) = &monotonic_candidates[0];

    // Convert the comparison to a Window trigger
    // For single-sided comparisons, we need to infer the implicit bounds
    let window_trigger = match &best_comp.var {
        Spatial(SpatialVar::DistanceRate) => match best_comp.op {
            CompOp::GreaterEqual | CompOp::Greater => Some(TriggerCondition::Window {
                variable: WindowVariable::DistanceRate,
                min_threshold: best_comp.value as f64,
                max_threshold: 100.0,
            }),
            CompOp::LessEqual | CompOp::Less => Some(TriggerCondition::Window {
                variable: WindowVariable::DistanceRate,
                min_threshold: 0.0,
                max_threshold: best_comp.value as f64,
            }),
            _ => None,
        },
        AccumulateTime => {
            match best_comp.op {
                CompOp::GreaterEqual | CompOp::Greater => {
                    // accumulatetime>=X doesn't need a window, use Time trigger
                    None
                }
                CompOp::LessEqual | CompOp::Less => Some(TriggerCondition::Window {
                    variable: WindowVariable::AccumulateTime,
                    min_threshold: 0.0,
                    max_threshold: best_comp.value as f64,
                }),
                _ => None,
            }
        }
        Status(StatusVar::HpPer) => {
            match best_comp.op {
                CompOp::GreaterEqual | CompOp::Greater => Some(TriggerCondition::Window {
                    variable: WindowVariable::HpPercent,
                    min_threshold: best_comp.value as f64,
                    max_threshold: 100.0,
                }),
                CompOp::LessEqual | CompOp::Less => {
                    // hp_per<=X doesn't need a window, use HpPercent trigger
                    None
                }
                _ => None,
            }
        }
        _ => None,
    };

    let Some(window) = window_trigger else {
        return (None, guards);
    };

    // Remove the promoted comparison from the guards list
    let remaining_guards: ComparisonList = guards
        .into_iter()
        .filter(|comp| comp != best_comp)
        .collect();

    (Some(window), remaining_guards)
}

/// Check if a comparison list contains any DynamicGuard variables.
fn contains_dynamic_guard_in_list(guards: &ComparisonList) -> bool {
    guards
        .iter()
        .any(|comp| comp.var.variable_role() == VariableRole::DynamicGuard)
}

/// Split a single comparison into trigger and guard.
///
/// This function does NOT make promotion decisions for monotonic guards.
/// It treats them as guards by default. Promotion happens at a higher level
/// in [`split_comp_tree()`] after analyzing all conditions together.
fn split_single_comp(
    comp: &Comparison,
    random_activation: &[f64],
    course: &Course,
) -> (Vec<TriggerCondition>, ComparisonList) {
    use VariableRole::*;

    let role = comp.var.variable_role();

    match role {
        RandomTrigger => {
            // Extract random activation trigger
            if let VarType::Random(_) = &comp.var {
                // If we are here, it means that the activation roll succeeded
                // → return no guards; the skill always activates at the rolled point
                (vec![], vec![])
            } else {
                // Create trigger based on random activation distance
                let trigger = TriggerCondition::DistanceCovered {
                    threshold: random_activation[0],
                    operator: comp.op,
                };
                // TODO: remove the used activation distance
                (vec![trigger], vec![])
            }
        }

        DeferredRandomTrigger => (vec![TriggerCondition::FullThrottleStart], vec![]),

        MonotonicGuard => {
            // MonotonicGuard: can be trigger OR guard depending on context
            // Default to guard here; promotion happens at higher level
            (vec![], vec![comp.clone()])
        }

        PhaseTrigger => {
            // Phase with comparison operators (<=, >=, <, >) should be guards, not triggers
            // Only phase==N creates a trigger (fires at phase entry)
            // phase<=1 means "while in Early or Mid" (check at trigger time)
            if comp.op == CompOp::Equal {
                // phase==N → PhaseTrigger (fire at phase entry)
                if let Some(trigger) = extract_phase_trigger(comp) {
                    (vec![trigger], vec![])
                } else {
                    (vec![TriggerCondition::Continuous], vec![])
                }
            } else {
                // phase<=N, phase>=N, etc. → DynamicGuard (check at trigger time)
                (vec![], vec![comp.clone()])
            }
        }

        SpatialTrigger => {
            if let Some(trigger) = extract_spatial_trigger(comp, course) {
                (vec![trigger], vec![])
            } else {
                (vec![TriggerCondition::Continuous], vec![])
            }
        }

        ActivationTrigger => {
            if comp.op == CompOp::Equal
                && comp.value == 1
                && let VarType::Activation(activation_var) = &comp.var
            {
                match activation_var {
                    ActivationVar::IsActivateAnySkill => {
                        return (vec![TriggerCondition::AnySkillActivation], vec![]);
                    }
                    ActivationVar::IsActivateHealSkill => {
                        return (vec![TriggerCondition::HealSkillActivation], vec![]);
                    }
                    _ => {} // Other activation variables stay as guards
                }
            }
            (vec![], vec![comp.clone()])
        }

        StaticGuard | DynamicGuard => {
            // Static and Dynamic guards - keep as guard
            (vec![], vec![comp.clone()])
        }
    }
}

/// Extract monotonic trigger from comparison.
///
/// Detects operator direction mismatches and converts them to window triggers:
///
/// - Increasing monotonic variables (`distance_rate`, `accumulatetime`) with `<=`
///   → `Window [0, threshold]`
/// - Decreasing monotonic variables (`hp_per`, `remain_distance`) with `>=`
///   → `Window [threshold, MAX]`
fn extract_monotonic_trigger(comp: &Comparison) -> TriggerCondition {
    use VarType::*;

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
        AccumulateTime => {
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

        Spatial(SpatialVar::DistanceRate) => {
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

        Spatial(SpatialVar::RemainDistance) => {
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

        Status(StatusVar::HpPer) => {
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

        Status(StatusVar::IsHpEmptyOnetime) => TriggerCondition::HpPercent {
            threshold: 0.0,
            operator: CompOp::LessEqual,
        },

        Spatial(SpatialVar::Furlong) => {
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

        Activation(_) => {
            // Treat as continuous trigger
            TriggerCondition::Continuous {}
        }

        _ => panic!("Not a monotonic trigger: {:?}", comp.var),
    }
}

/// Extract phase trigger from comparison.
///
/// Phase triggers fire once when entering the specified phase.
/// The comparison operator is ignored - both `phase==2` and `phase>=2`
/// fire when Late phase begins (at the phase boundary).
///
/// This is semantically correct because:
///
/// - Phases are discrete states, not continuous values.
/// - Skills activate on state transitions (Early→Mid→Late→Last).
/// - `phase>=2` conceptually means "when in Late or Last phase".
/// - The trigger fires at Late entry; the skill remains active during Late+Last.
fn extract_phase_trigger(comp: &Comparison) -> Option<TriggerCondition> {
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

/// Convert spatial conditions to Window triggers based on course geometry.
fn extract_spatial_trigger(comp: &Comparison, course: &Course) -> Option<TriggerCondition> {
    if let VarType::Spatial(var_type) = &comp.var {
        match var_type {
            SpatialVar::Corner => {
                match (comp.op, comp.value) {
                    // corner==N [N>0] → Window[corner_N.start, corner_N.end]
                    (CompOp::Equal, n) if n > 0 => {
                        let corner_idx = (n - 1) as usize; // Convert from 1-based index
                        if corner_idx < course.corners.len() {
                            let corner = &course.corners[corner_idx];
                            Some(TriggerCondition::Window {
                                variable: WindowVariable::SpatialPosition,
                                min_threshold: corner.start,
                                max_threshold: corner.start + corner.length,
                            })
                        } else {
                            // Invalid corner index - fall back to continuous
                            Some(TriggerCondition::Continuous)
                        }
                    }

                    // corner!=0 → Continuous (multiple disjoint ranges - complex case)
                    (CompOp::NotEqual, 0) => Some(TriggerCondition::Continuous),

                    // corner==0 (not a corner) or corner!=N [N>0] → Not a trigger, stays as guard
                    _ => None,
                }
            }
            SpatialVar::IsFinalcorner => {
                // is_finalcorner==1 → Window[final_corner.start, final_corner.end]
                if comp.op == CompOp::Equal && comp.value == 1 {
                    course
                        .corners
                        .last()
                        .map(|corner| TriggerCondition::Window {
                            variable: WindowVariable::SpatialPosition,
                            min_threshold: corner.start,
                            max_threshold: corner.start + corner.length,
                        })
                } else {
                    // is_finalcorner==0 or other operators → Guard
                    None
                }
            }
            SpatialVar::IsFinalcornerLaterhalf => {
                // is_finalcorner_laterhalf==1 → Window[final_corner.midpoint, final_corner.end]
                if comp.op == CompOp::Equal && comp.value == 1 {
                    if let Some(corner) = course.corners.last() {
                        let midpoint = corner.start + corner.length / 2.0;
                        Some(TriggerCondition::Window {
                            variable: WindowVariable::SpatialPosition,
                            min_threshold: midpoint,
                            max_threshold: corner.start + corner.length,
                        })
                    } else {
                        // No corners in course
                        None
                    }
                } else {
                    // Other operators → Guard
                    None
                }
            }
            SpatialVar::Slope => {
                match (comp.op, comp.value) {
                    // slope==1 or slope==2 → Continuous (multiple disjoint ranges - complex case)
                    (CompOp::Equal, 1) | (CompOp::Equal, 2) => Some(TriggerCondition::Continuous),

                    // slope==0 (flat) or slope!=X → Not a trigger, stays as guard
                    _ => None,
                }
            }
            SpatialVar::IsLastStraight => {
                // is_last_straight==1 → Window[last_straight.start, last_straight.end]
                if comp.op == CompOp::Equal && comp.value == 1 {
                    course
                        .straights
                        .last()
                        .map(|straight| TriggerCondition::Window {
                            variable: WindowVariable::SpatialPosition,
                            min_threshold: straight.start,
                            max_threshold: straight.end,
                        })
                } else {
                    // Other operators → Guard
                    None
                }
            }
            SpatialVar::IsLastStraightOnetime => {
                // is_last_straight_onetime==1 → Keep as one-time event trigger
                // This is genuinely a one-time activation, not a continuous window
                if comp.op == CompOp::Equal && comp.value == 1 {
                    Some(TriggerCondition::StraightTransition {
                        is_last: true,
                        is_entry: true,
                        is_onetime: true,
                    })
                } else {
                    // Other operators → Guard
                    None
                }
            }
            SpatialVar::StraightFrontType => {
                // straight_front_type==1 or ==2 → Continuous (multiple straights)
                if comp.op == CompOp::Equal && (comp.value == 1 || comp.value == 2) {
                    // Complex case: there may be multiple straights of the same type
                    // Fall back to continuous checking
                    Some(TriggerCondition::Continuous)
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
