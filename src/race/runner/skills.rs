//! Skill trigger checks, activation and active effects.
//!
//! ## Checking Skills
//!
//! When a trigger fires, [`Runner::try_activate_skill`] checks its logic:
//!
//! 1. Stop if the logic waits for a precondition that isn't satisfied yet.
//! 2. Stop if the logic unlocks a precondition that is already satisfied, or
//!    activates an alternative that is in cooldown. Cooldowns are kept by the
//!    runner ([`Runner::cooldown_until`]), so that skills activated without
//!    logics (see below) have one too.
//! 3. Stop if another alternative of the skill already activated, unless the
//!    logic belongs to a continuation (see below).
//! 4. Evaluate the guards (all must be true).
//! 5. On success, run the action:
//!    - [`Activate`]: activate the skill (see below), which starts the cooldown
//!      of the alternative. Once it ends, the logic can succeed again.
//!    - [`Unlock`]: mark the precondition as satisfied. The logic is never
//!      checked again.
//!
//! ## Activating a Skill
//!
//! Activating a skill ([`Runner::activate_skill`]):
//!
//! - Triggers the additional effects of the skills already active that apply
//!   on any skill activation (see [`AdditionalActivation`]).
//! - Records the activation for reporting ([`Runner::activated_skills`]),
//!   before the effects, so that skills they activate are recorded after it.
//! - Applies each effect targeting the runner herself. **Timed effects**
//!   (speed, acceleration...) are stored with an expiration time (base duration
//!   scaled by the runner's situation, the course distance and active
//!   duration-modifying effects; or the end of the race for effects lasting
//!   until then) and folded into her skill modifiers; **instant effects**
//!   (recovery...) apply immediately.
//!   **Pre-race effects** (stats, start delay...) are skipped, as they were
//!   applied before the race.
//! - Queues each effect targeting other runners in `pending_effects`, with the
//!   caster's scaled duration and wit limit break bonus. Once every runner has
//!   checked her skills, the solver selects the targets ([`SkillTarget`]) and
//!   applies the effects to them.
//! - Starts the cooldown of the activated alternative (all its OR branches).
//!   Timed effects record the skill that applied them, so that its effects
//!   being active can be checked as well.
//! - Records the alternative in the skill's mask of activated alternatives
//!   ([`Runner::activated_alternatives`]), used for `is_used_skill_id`,
//!   `is_activate_other_skill_detail`, forced activations and value scaling.
//!   Once any other alternative is in the mask, an alternative can't activate
//!   for the rest of the race: a skill activates through a single alternative.
//!   The exception are continuations, alternatives using
//!   `is_activate_other_skill_detail`, which can only activate after another
//!   alternative did.
//! - Updates the activation counts, after the effects, so that effects
//!   scaled by them don't count their own skill.
//! - Sets this frame's activation flags, which fire any-skill and heal
//!   activation triggers.
//!
//! When several alternatives are satisfied on the same frame, the first one
//! checked wins, which is the first one in order as long as they don't depend
//! on this frame's activations (see [`Runner::check_skills`]).
//!
//! **Known gap:** activation-dependent triggers are checked after the others,
//! so a later alternative could win over an activation-dependent earlier one.
//! No current skill has such alternatives.
//!
//! ## Forced Activations
//!
//! Some effects activate other skills of the runner, bypassing their
//! conditions: [`SkillType::ActivateRandomGold`] (e.g. "564 Escapades") and
//! [`SkillType::ActivateUnique`]. See [`Runner::activate_random_gold_skills`]
//! and [`Runner::activate_unique_skills`].
//!
//! Random gold skills can be picked among those that failed the wit check,
//! which have no logics and only activate this way, and among those that
//! already activated, once their cooldown and effects are over.
//!
//! [`Activate`]: crate::trigger::skill_activation::LogicAction::Activate
//! [`Unlock`]: crate::trigger::skill_activation::LogicAction::Unlock

use rand::prelude::*;

use crate::condition::evaluator::{CompEvaluator, EvaluatorFull, evaluate_comparison_list};
use crate::condition::parser::{Comparison, ComparisonList};
use crate::configuration::GameVersion;
use crate::course::*;
use crate::race::modifiers::*;
use crate::race::skill_scaling;
use crate::race::solver::RaceState;
use crate::skill::*;
use crate::trigger::random::{SEGMENT_LENGTH, random_segment};
use crate::trigger::skill_activation::{
    ActiveTimedEffect, LogicAction, OvertakeExtension, PendingEffect, SkillActivationRecord,
};
use crate::trigger::system::{TriggerScheduler, WindowTrigger};
use crate::units::{CourseWidths, meters_to_bashin};

use super::{EPSILON, Runner};

/// Skill modifiers that are applied during race calculations.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct SkillModifiers {
    /// Modifies target speed. Positive values increase it.
    pub(crate) target_speed: f64,
    /// Modifies current speed directly. Positive values increase it.
    pub(crate) current_speed: f64,
    /// Modifies acceleration. Positive values increase it.
    pub(crate) acceleration: f64,
    /// Modifies all-out spurt acceleration (listed value, before scaling).
    pub(crate) all_out_spurt_acceleration: f64,
    /// Modifies lane movement speed. Positive values increase lane change speed.
    pub(crate) lane_speed: f64,
    /// Modifies visible distance. Positive values increase it.
    pub(crate) visible_distance: f64,
    /// Sets the target lane directly, or `None` if no change.
    pub(crate) target_lane: Option<CourseWidths>,
    /// Affects duration of timed effects. Multiplicative - 1.0 means no change.
    pub(crate) duration_scaling: f64,
}

impl Default for SkillModifiers {
    fn default() -> Self {
        Self {
            target_speed: 0.0,
            current_speed: 0.0,
            acceleration: 0.0,
            all_out_spurt_acceleration: 0.0,
            lane_speed: 0.0,
            visible_distance: 0.0,
            target_lane: None,
            duration_scaling: 1.0, // Multiplicative default.
        }
    }
}

impl SkillModifiers {
    /// Check if skills are modifying target or current speed, in either
    /// direction.
    pub(crate) fn has_speed_effects(&self) -> bool {
        self.current_speed.abs() >= 0.0001 || self.target_speed.abs() >= 0.0001
    }

    /// Sum up the modifiers of the active timed `effects`.
    pub(crate) fn from_effects(effects: &[ActiveTimedEffect]) -> Self {
        use SkillType::*;
        let mut mods = SkillModifiers::default();
        for timed in effects {
            let value = timed.current_value();
            match timed.effect.skill_type {
                Acceleration => mods.acceleration += value,
                AccelerationAllOutSpurt => mods.all_out_spurt_acceleration += value,
                TargetSpeed => mods.target_speed += value,
                CurrentSpeed | CurrentSpeedWithNaturalDeceleration => mods.current_speed += value,
                LaneMovementSpeed => mods.lane_speed += value,
                Fov => mods.visible_distance += value,
                LaneChange => mods.target_lane = Some(CourseWidths(value)),
                // Duration scaling is multiplicative.
                ExtendEvolvedDuration => mods.duration_scaling *= value,
                _ => {}
            }
        }
        mods
    }
}

/// Skill activation flags for the current update cycle.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SkillActivationFlags {
    /// Indicates if a skill was activated in the current update cycle.
    pub any_activated: bool,
    /// Indicates if a healing skill was activated in the current update cycle.
    pub heal_activated: bool,
    /// Effect types of the buff (advantage) skills activated in the current update
    /// cycle. Only effects on oneself count.
    pub buff_activated: SkillTypeSet,
}

impl Runner {
    /// Resolve the guards that can't change during the race (e.g.
    /// `distance_type==3`), so that they aren't checked every frame.
    ///
    /// Guards that are true are removed. Logics with a false guard can never
    /// succeed, so their triggers are removed.
    ///
    /// This can't be done when the scheduler is built, since some of these values
    /// (popularity, running style changed by out-of-gate skills...) are only final
    /// once the runner is created.
    pub(super) fn resolve_static_guards(&mut self, state: &RaceState) {
        let is_static = |comp: &Comparison| comp.var.is_static();

        let mut unsatisfiable = Vec::new();
        for logic_idx in 0..self.trigger_scheduler.skill_logics.len() {
            let logic = &self.trigger_scheduler.skill_logics[logic_idx];
            let guards: ComparisonList = logic
                .guards
                .iter()
                .filter(|comp| {
                    !is_static(comp) || !EvaluatorFull.evaluate(comp, Some(logic), state, self)
                })
                .cloned()
                .collect();
            if guards.iter().any(is_static) {
                unsatisfiable.push(logic_idx);
            }
            self.trigger_scheduler.skill_logics[logic_idx].guards = guards;
        }

        self.trigger_scheduler.remove_triggers(&unsatisfiable);
    }

    /// Check every skill trigger.
    ///
    /// All skills are checked at once, at the start of each frame, against runner's
    /// state at that point. Nothing is updated in between, so every check sees the
    /// same values.
    ///
    /// The following order is used to check triggers:
    ///
    /// 1. Race start triggers, only on the first frame the runner moves. Their
    ///    logics never wait for a precondition: they belong to out-of-gate skills,
    ///    whose preconditions were evaluated before the race, along with their
    ///    conditions.
    /// 2. Precondition windows, so that a skill can activate in the same frame its
    ///    precondition is satisfied.
    /// 3. Triggers that don't depend on this frame's activations: windows and
    ///    last straight entry, in the order the logics were built.
    /// 4. Any-skill and heal activation triggers, which depend on the activations
    ///    above.
    ///
    /// `is_other_character_activate_advantage_skill` (and
    /// `is_popularity_top_character_activate_advantage_skill`) triggers depend on
    /// the activations of every runner, so they're checked afterwards (see
    /// [`Self::check_advantage_triggers`]).
    pub(crate) fn check_skills(&mut self, state: &mut RaceState) {
        self.activation_flags = SkillActivationFlags::default();
        // Out-of-gate skills activate on the first frame the runner moves; their
        // pre-race effects were already applied before the race. Taken, so that
        // they're only checked once.
        self.try_activate_all(state, |scheduler, indices| {
            indices.append(&mut scheduler.race_start_triggers);
        });
        // Preconditions go first, so that a skill can activate in the same frame its
        // precondition is satisfied.
        self.check_precondition_windows(state);
        self.check_state_triggers(state);
        // Depend on the activations above. Heal-dependent triggers go last, as
        // any-skill-dependent ones may activate heals.
        if self.activation_flags.any_activated {
            self.try_activate_all(state, |scheduler, indices| {
                indices.extend_from_slice(&scheduler.any_activation_triggers);
            });
        }
        if self.activation_flags.heal_activated {
            self.try_activate_all(state, |scheduler, indices| {
                indices.extend_from_slice(&scheduler.heal_activation_triggers);
            });
        }
    }

    /// Check if the runner can activate skills during a step ending at
    /// `step_end_time`.
    ///
    /// For that, she needs to move during the step (i.e. she has left the gate by
    /// then) and hasn't finished yet.
    pub(crate) fn can_activate_skills(&self, step_end_time: f64) -> bool {
        self.start_delay <= step_end_time && !self.has_finished()
    }

    /// Check the preconditions whose window contains the current position.
    ///
    /// This must be called before any trigger whose logic can wait for a
    /// precondition, so that a skill can activate in the same frame its
    /// precondition is satisfied.
    fn check_precondition_windows(&mut self, state: &RaceState) {
        let position = self.position;
        let any_unlocked = self.try_activate_all(state, |scheduler, indices| {
            indices.extend(scheduler.precondition_windows.advance(position, EPSILON));
        });

        // Stop checking every branch whose preconditions were just satisfied.
        if any_unlocked {
            let TriggerScheduler {
                precondition_windows,
                skill_logics,
                precondition_satisfied,
                ..
            } = &mut self.trigger_scheduler;
            precondition_windows.remove_if(|window| {
                matches!(
                    skill_logics[window.logic_idx].action,
                    LogicAction::Unlock(id) if precondition_satisfied[id]
                )
            });
        }
    }

    /// Check the triggers that don't depend on this frame's activations.
    ///
    /// Windows (position, time, HP) containing the current values are checked, as
    /// well as entering the last straight since the last check.
    ///
    /// These triggers are checked together in the order the logics were built (by
    /// skill, then by alternative), so that when several alternatives of a skill
    /// are satisfied on the same frame, the first one wins.
    fn check_state_triggers(&mut self, state: &RaceState) {
        let entered_last_straight = self
            .segments
            .take_entered_straight()
            .is_some_and(|straight_idx| straight_idx + 1 == state.course.straights.len());

        let position = self.position;
        let hp = self.hp.percent();
        self.try_activate_all(state, |scheduler, inside| {
            inside.extend(scheduler.position_windows.advance(position, EPSILON));
            inside.extend(scheduler.time_windows.advance(state.elapsed_time, EPSILON));

            // HP can recover, so HP windows are never dropped.
            inside.extend(
                scheduler
                    .hp_windows
                    .iter()
                    .filter(|window| {
                        hp >= window.min_threshold - EPSILON && hp <= window.max_threshold + EPSILON
                    })
                    .map(|window| window.logic_idx),
            );
            if entered_last_straight {
                inside.extend(&scheduler.last_straight_triggers);
            }

            // Try activations in the order the logics were built.
            inside.sort_unstable();
        });
    }

    /// Check `is_other_character_activate_advantage_skill` and
    /// `is_popularity_top_character_activate_advantage_skill` triggers.
    ///
    /// Other runners' activations are only known once every runner has checked
    /// her skills, so this is called by the solver afterwards.
    pub(crate) fn check_advantage_triggers(&mut self, state: &RaceState) {
        self.try_activate_all(state, |scheduler, indices| {
            indices.extend_from_slice(&scheduler.buff_activation_triggers);
        });
    }

    /// Attempt to activate the skills associated with each trigger index, in
    /// order.
    ///
    /// The indices are added by `collect` to the scheduler's check buffer, which
    /// is reused so that checks don't allocate.
    ///
    /// Returns whether any logic succeeded.
    fn try_activate_all(
        &mut self,
        state: &RaceState,
        collect: impl FnOnce(&mut TriggerScheduler, &mut Vec<usize>),
    ) -> bool {
        // Taken, since activating skills needs the whole runner.
        let mut trigger_indices = std::mem::take(&mut self.trigger_scheduler.check_buffer);
        collect(&mut self.trigger_scheduler, &mut trigger_indices);
        let mut any_succeeded = false;
        for &trigger_index in &trigger_indices {
            any_succeeded |= self.try_activate_skill(trigger_index, state);
        }
        trigger_indices.clear();
        self.trigger_scheduler.check_buffer = trigger_indices;
        any_succeeded
    }

    /// Given a trigger index, attempt to activate the associated skill.
    ///
    /// Returns whether the logic succeeded.
    fn try_activate_skill(&mut self, trigger_index: usize, state: &RaceState) -> bool {
        // Get skill logic and conditions.
        let logic = &self.trigger_scheduler.skill_logics[trigger_index];

        // Check if precondition is satisfied; skip if not.
        if let Some(id) = logic.precondition
            && !self.trigger_scheduler.precondition_satisfied[id]
        {
            return false;
        }

        // Satisfied preconditions are never checked again, and activations wait for
        // the cooldown of their alternative.
        match logic.action {
            LogicAction::Unlock(id) if self.trigger_scheduler.precondition_satisfied[id] => {
                return false;
            }
            LogicAction::Activate
                if self.is_in_cooldown(logic.skill_index, logic.alternative_index, state) =>
            {
                return false;
            }
            _ => {}
        }

        // A skill activates through a single alternative. Continuations are the
        // exception: they can only activate after another alternative did.
        //
        // When several alternatives are satisfied on the same frame, the first one
        // checked wins. Triggers that don't depend on this frame's activations are
        // checked by alternative (see [`Self::check_state_triggers`]), but
        // activation-dependent ones (any skill, heal, other runners' buffs) are
        // checked after them, as they depend on their activations.
        //
        // N.B.: A later alternative could then win over an activation-dependent
        // earlier one. It's assumed that no skill will have such alternatives.
        let others_activated =
            self.activated_alternatives[logic.skill_index] & !(1 << logic.alternative_index);
        if others_activated != 0 && !logic.is_continuation {
            return false;
        }
        let skill_index = logic.skill_index;
        let alternative_index = logic.alternative_index;
        let action = logic.action;
        let should_activate =
            evaluate_comparison_list(&logic.guards, &EvaluatorFull, Some(logic), state, self);

        if should_activate {
            match action {
                // Starts the cooldown.
                LogicAction::Activate => self.activate_skill(skill_index, alternative_index, state),
                LogicAction::Unlock(id) => self.trigger_scheduler.precondition_satisfied[id] = true,
            }
        }
        should_activate
    }

    /// Activate a skill by its index and alternative index.
    pub(crate) fn activate_skill(
        &mut self,
        skill_index: usize,
        alternative_index: usize,
        state: &RaceState,
    ) {
        // Activating a skill triggers the additional effects of those already active,
        // before this skill's own effects are applied.
        self.trigger_additional_effects(
            &[
                AdditionalActivation::ActivateAnySkill1,
                AdditionalActivation::ActivateAnySkill2,
            ],
            1,
        );

        // Clone the skill data we need to avoid borrow checker issues.
        let skill_id = self.skills[skill_index].id;
        let alternative = self.skills[skill_index].alternatives[alternative_index].clone();

        // Record the activation before applying the effects, so that skills
        // activated by them are recorded after this one.
        self.activated_skills.push(SkillActivationRecord {
            skill_id,
            distance: self.position,
        });

        // Scale with the runner's situation, distance and effects. Effects lasting
        // until the end of the race have an infinite base duration, which stays
        // infinite.
        let duration_factor =
            state.course.distance as f64 / 1000.0 * self.skill_modifiers.duration_scaling;
        let scaled_duration = self.situational_base_duration(&alternative) * duration_factor;
        // Extensions from overtaking scale like the base duration.
        let overtake_extension =
            if alternative.duration_scaling == DurationScaling::IncrementOrderUp {
                OvertakeExtension {
                    duration: skill_scaling::OVERTAKE_EXTENSION * duration_factor,
                    remaining: skill_scaling::MAX_OVERTAKE_EXTENSIONS,
                }
            } else {
                OvertakeExtension::default()
            };

        // Wit limit break bonus, fixed at activation time (5th Anniv. onward).
        let wit_bonus = if state.config.game_version >= GameVersion::FifthAnniversary
            && self.skills[skill_index].is_wit_limit_break_eligible()
        {
            wit_limit_break_phase_coefficient(self.adjusted.style, self.phase)
                * wit_limit_break_multiplier(self.adjusted.wit)
        } else {
            0.0
        };

        // Apply each effect in the alternative. Effects on other runners are applied
        // later by the solver, which knows about every runner.
        // N.B. Pre-race effects were applied before the race (see
        // `UnreadyRunner::apply_out_of_gate_skills`), but still count as activated.
        for effect in &alternative.effects {
            if !effect.skill_type.is_pre_race_effect() {
                // Scale the value with the runner's situation.
                let scaled = SkillEffect {
                    modifier: self.scaled_modifier(effect, skill_index, state),
                    ..effect.clone()
                };
                if scaled.target == SkillTarget::Oneself {
                    self.apply_skill_effect(
                        &scaled,
                        scaled_duration,
                        overtake_extension,
                        wit_bonus,
                        Some(skill_index),
                        state,
                    );
                } else {
                    self.pending_effects.push(PendingEffect {
                        effect: scaled,
                        duration: scaled_duration,
                        wit_bonus,
                        target_roll: self.rng.skills[skill_index].random(),
                    });
                }
            }
            // Buffs count as activated even if their value scaled down to zero.
            if effect.target == SkillTarget::Oneself && effect.is_positive() {
                self.activation_flags
                    .buff_activated
                    .insert(effect.skill_type);
            }
        }

        // Recalculate skill modifiers from active effects.
        self.recalculate_skill_modifiers();

        self.start_cooldown(skill_index, alternative_index, state);

        // Track skill activation.
        self.activated_alternatives[skill_index] |= 1 << alternative_index;

        // Increment activation counts for condition evaluation.
        self.trigger_scheduler.activation_counts.increment(
            self.phase,
            self.position,
            state.course.halfway(),
        );
        if self.skills[skill_index].is_green() {
            self.trigger_scheduler.activation_counts.green += 1;
        }

        // Set activation flags for activation-dependent triggers.
        self.activation_flags.any_activated = true;
        if self.is_heal_skill_by_index(skill_index) {
            self.activation_flags.heal_activated = true;
        }
    }

    /// Start the cooldown of a skill alternative, shared by all its OR branches.
    ///
    /// The base cooldown scales with the course distance by a factor of *course
    /// distance / 1000*.
    fn start_cooldown(&mut self, skill_index: usize, alternative_index: usize, state: &RaceState) {
        let base_cooldown = self.skills[skill_index].alternatives[alternative_index].base_cooldown;
        let cooldown = base_cooldown * state.course.distance as f64 / 1000.0;
        self.cooldown_until[skill_index][alternative_index] = state.elapsed_time + cooldown;
    }

    /// Check if a skill alternative is waiting for its cooldown to end.
    fn is_in_cooldown(
        &self,
        skill_index: usize,
        alternative_index: usize,
        state: &RaceState,
    ) -> bool {
        state.elapsed_time < self.cooldown_until[skill_index][alternative_index]
    }

    /// Check if any timed effect applied by a skill is still active.
    fn is_skill_active(&self, skill_index: usize) -> bool {
        self.active_timed_effects
            .iter()
            .any(|timed| timed.source == Some(skill_index))
    }

    /// Alternative used to activate a skill bypassing its conditions: the (first)
    /// one that activated before, or the first one if the skill hasn't activated
    /// yet.
    fn forced_alternative(&self, skill_index: usize) -> usize {
        match self.activated_alternatives[skill_index] {
            0 => 0,
            mask => mask.trailing_zeros() as usize,
        }
    }

    /// Base duration of a skill alternative scaled with the runner's situation
    /// (see [`DurationScaling`]), before distance scaling.
    fn situational_base_duration(&self, alternative: &SkillAlternative) -> f64 {
        use DurationScaling::*;
        let base = alternative.base_duration;
        match alternative.duration_scaling {
            // Overtakes extend the effects later instead.
            Direct | IncrementOrderUp => base,
            MultiplyDistanceDiffTop => {
                let distance = self.position_diff.from_first;
                base * skill_scaling::distance_from_first_duration_multiplier(distance)
            }
            MultiplyRemainHp1 => {
                base * skill_scaling::remaining_hp_duration_multiplier_1(self.hp.current())
            }
            MultiplyRemainHp2 => {
                base * skill_scaling::remaining_hp_duration_multiplier_2(self.hp.current())
            }
            MultiplyBlockedSideMaxContinueTimePhaseMiddleRun => {
                let time = self.blocked.max_mid_side_time;
                base * skill_scaling::mid_side_blocked_time_multiplier(time)
            }
            IncrementUphillCount => {
                base + skill_scaling::uphill_duration_bonus(self.uphills_climbed)
            }
        }
    }

    /// Value of an effect's modifier scaled with its skill's level, then with
    /// the runner's situation (see [`ValueScaling`]).
    fn scaled_modifier(
        &mut self,
        effect: &SkillEffect,
        skill_index: usize,
        state: &RaceState,
    ) -> f64 {
        use ValueScaling::*;
        let level = self.skills[skill_index].level;
        let modifier = skill_scaling::level_scaled_modifier(effect, level);
        match effect.value_scaling {
            Direct => modifier,
            MultiplySkillNum => {
                modifier * skill_scaling::skill_count_multiplier(self.skill_count())
            }
            UnityCupSpeed | UnityCupStamina | UnityCupPower | UnityCupGuts | UnityCupWit => {
                modifier * skill_scaling::UNITY_CUP_MULTIPLIER
            }
            // Both variants share the same multipliers for now.
            MultiplyRandom1 | MultiplyRandom2 => {
                let roll = self.rng.skills[skill_index].random();
                modifier * skill_scaling::random_multiplier(roll)
            }
            Climax => modifier * skill_scaling::CLIMAX_MULTIPLIER,
            MultiplyMaximumRawStatus => {
                let raw = &self.raw;
                let max_stat = [raw.speed, raw.stamina, raw.power, raw.guts, raw.wit]
                    .into_iter()
                    .max()
                    .unwrap();
                modifier * skill_scaling::max_raw_stat_multiplier(max_stat)
            }
            MultiplyActivateSpecificTagSkillCount => {
                let green_count = self.trigger_scheduler.activation_counts.green;
                modifier * skill_scaling::green_skill_count_multiplier(green_count)
            }
            AddDistanceDiffTop => {
                modifier + skill_scaling::distance_from_first_bonus(self.position_diff.from_first)
            }
            AddDistanceTypeMedium => {
                modifier + skill_scaling::medium_distance_bonus(state.course.distance_type())
            }
            CinderellaGray => {
                let course = &state.course;
                let fans = self.initial_state.fan_count;
                modifier
                    + skill_scaling::cinderella_gray_bonus(course.id, course.distance_type(), fans)
            }
            AddBashinDiffTop => {
                let bashin = meters_to_bashin(self.position_diff.from_first);
                modifier + skill_scaling::bashin_from_first_bonus(bashin)
            }
            MultiplyBlockedSideMaxTimeMidRace => {
                let time = self.blocked.max_mid_side_time;
                modifier * skill_scaling::mid_side_blocked_time_multiplier(time)
            }
            MultiplySpeed1 => modifier * skill_scaling::speed_multiplier_1(self.adjusted.speed),
            MultiplySpeed2 => modifier * skill_scaling::speed_multiplier_2(self.adjusted.speed),
            MultiplyArcGlobalPotentialLevel => {
                modifier * skill_scaling::ARC_GLOBAL_POTENTIAL_MULTIPLIER
            }
            MultiplyTopLeadAmount => {
                modifier * skill_scaling::lead_multiplier(self.position_diff.max_lead)
            }
            CornerOvertakeCount => {
                let overtakes = self.order.overtakes_in_late_corners;
                modifier * skill_scaling::late_corner_overtake_multiplier(overtakes)
            }
            FanCount => {
                modifier * skill_scaling::fan_count_multiplier(self.initial_state.fan_count)
            }
            UafWinCount => modifier * skill_scaling::UAF_WIN_MULTIPLIER,
            CookingPoints => modifier * skill_scaling::COOKING_MULTIPLIER,
            ResearchLevel => modifier * skill_scaling::RESEARCH_MULTIPLIER,
            FanLove => modifier * skill_scaling::FAN_LOVE_MULTIPLIER,
            DevelopmentPoints => modifier * skill_scaling::DEVELOPMENT_MULTIPLIER,
            OnsenLegendaryInn => modifier * skill_scaling::ONSEN_LEGENDARY_INN_MULTIPLIER,
            TeamRank => modifier * skill_scaling::TEAM_RANK_MULTIPLIER,
            ExcitementPoints => modifier * skill_scaling::EXCITEMENT_MULTIPLIER,
        }
    }

    /// Apply a single skill effect to the runner.
    ///
    /// Effects can be categorized as follows:
    /// - Timed effects (speed, acceleration...): modify a stat for a certain
    ///   duration that is constantly tracked.
    /// - Recovery: instant effect that recovers HP.
    ///
    /// `source` is the index of the runner's own skill that applies the effect, or
    /// `None` if it was cast by another runner.
    pub(crate) fn apply_skill_effect(
        &mut self,
        effect: &SkillEffect,
        scaled_duration: f64,
        overtake_extension: OvertakeExtension,
        wit_bonus: f64,
        source: Option<usize>,
        state: &RaceState,
    ) {
        // Pre-race effects (stat boosts, start delay...) never reach this point (see
        // `activate_skill`).
        match effect.skill_type {
            // Timed effects (tracked in active_timed_effects).
            effect_type if effect_type.is_timed_effect() => {
                // Apply wit limit break bonus to target and current speed effects.
                let bonus = match effect_type {
                    SkillType::TargetSpeed | SkillType::CurrentSpeedWithNaturalDeceleration => {
                        wit_bonus
                    }
                    _ => 0.0,
                };
                self.active_timed_effects.push(ActiveTimedEffect {
                    effect: effect.clone(),
                    value: effect.modifier * (1.0 + bonus),
                    expires_at: state.elapsed_time + scaled_duration,
                    overtake_extension,
                    additional_triggers: 0,
                    source,
                });
            }
            // Recovery (HP restoration, or HP loss if negative), as a fraction of max
            // HP.
            SkillType::Recovery => {
                self.hp.recover(self.hp.maximum() * effect.modifier);

                // HP loss (e.g., a skill's drawback) is not a heal.
                if effect.modifier > 0.0 {
                    // Increment heal skill count.
                    self.trigger_scheduler.activation_counts.increment_heal();

                    // Recovering HP resets Stamina Keep (2.5 Anniv. onward).
                    self.stamina_keep.is_active = false;

                    // Recalculate last spurt if in Late phase or later (1st Anniv.
                    // onward).
                    if state.config.game_version >= GameVersion::FirstAnniversary
                        && self.phase >= Phase::Late
                    {
                        self.last_spurt = Some(self.calculate_last_spurt(state));
                    }
                }
            }
            // Activate random gold skills (one per unit of modifier).
            // ? Assume only the runner's own skills do it, as in the data.
            SkillType::ActivateRandomGold => {
                if let Some(source) = source {
                    self.activate_random_gold_skills(source, effect.modifier as usize, state);
                }
            }
            // Activate unique skills again (one per unit of modifier).
            SkillType::ActivateUnique => {
                self.activate_unique_skills(effect.modifier as usize, state);
            }
            // Rush time extension, only if the runner is bound to rush at all.
            // ? Assume it also applies if already rushing for now.
            SkillType::RushTimeUp => {
                if let Some(rush_state) = &mut self.rush_state {
                    rush_state.max_duration += effect.modifier;
                }
            }
            _ => {}
        }
    }

    /// Activate up to `count` distinct random gold skills (see [`Skill::is_gold`]),
    /// bypassing their conditions.
    ///
    /// Skills that failed the wit check can be picked too, but out-of-gate skills
    /// can't, as their effects are only applied before the race. Skills still in
    /// cooldown or with active effects are skipped, so a skill that already
    /// activated can be picked again once both are over.
    ///
    /// The alternative that activated before is used, or the first one if the
    /// skill hasn't activated yet, so that later ones that continue it (see
    /// [`SkillAlternative::is_continuation`]) can still activate.
    ///
    /// Skills are picked with the stream of `source`, the index of the skill that
    /// activates them.
    fn activate_random_gold_skills(&mut self, source: usize, count: usize, state: &RaceState) {
        let mut candidates: Vec<usize> = (0..self.skills.len())
            .filter(|&skill_index| {
                let skill = &self.skills[skill_index];
                skill.is_gold()
                    && !skill.should_activate_out_of_gate()
                    && self.can_force_gold_skill(skill_index, state)
            })
            .collect();
        for _ in 0..count {
            if candidates.is_empty() {
                break;
            }
            let idx = self.rng.skills[source].random_range(0..candidates.len());
            let skill_index = candidates.swap_remove(idx);
            // ? Assume the first alternative is activated if none did, even when the
            // others are variants for other courses or running styles (e.g. "Piercing
            // Azure").
            let alternative_index = self.forced_alternative(skill_index);
            self.activate_skill(skill_index, alternative_index, state);
            // Activations may affect other candidates (e.g. through effects that
            // activate skills), so they're checked again after each pick.
            candidates.retain(|&skill_index| self.can_force_gold_skill(skill_index, state));
        }
    }

    /// Check if a gold skill can be activated by
    /// [`Self::activate_random_gold_skills`]: it isn't in cooldown, and none of its
    /// effects are active.
    fn can_force_gold_skill(&self, skill_index: usize, state: &RaceState) -> bool {
        let alternative_index = self.forced_alternative(skill_index);
        !self.is_in_cooldown(skill_index, alternative_index, state)
            && !self.is_skill_active(skill_index)
    }

    /// Activate up to `count` unique skills, bypassing their conditions and
    /// cooldowns.
    ///
    /// Each activation is a new instance of the skill: its values (duration, wit
    /// limit break bonus...) are calculated anew, and its effects stack with any
    /// still active from a previous activation. The alternative is chosen like for
    /// [`Self::activate_random_gold_skills`].
    ///
    /// N.B. Out-of-gate skills are skipped, as their effects are only applied
    /// before the race.
    fn activate_unique_skills(&mut self, count: usize, state: &RaceState) {
        let unique_indices: Vec<usize> = self
            .available_skills()
            .iter()
            .enumerate()
            .filter(|(_, skill)| skill.is_unique() && !skill.should_activate_out_of_gate())
            .map(|(i, _)| i)
            .take(count)
            .collect();
        for skill_index in unique_indices {
            let alternative_index = self.forced_alternative(skill_index);
            self.activate_skill(skill_index, alternative_index, state);
        }
    }

    /// Apply a skill effect casted by another runner.
    pub(crate) fn receive_skill_effect(&mut self, pending: &PendingEffect, state: &RaceState) {
        // Overtakes only extend the caster's own effects.
        self.apply_skill_effect(
            &pending.effect,
            pending.duration,
            OvertakeExtension::default(),
            pending.wit_bonus,
            None,
            state,
        );
        self.recalculate_skill_modifiers();
    }

    /// Mask of the activated alternatives of the skill with the given ID (see
    /// [`Self::activated_alternatives`]), or 0 if the runner doesn't have it.
    pub fn activated_mask(&self, skill_id: i32) -> u8 {
        self.skills
            .iter()
            .position(|skill| skill.id == skill_id)
            .map_or(0, |skill_index| self.activated_alternatives[skill_index])
    }

    /// Check if a skill at the given index is a heal skill.
    ///
    /// A heal skill has an alternative that contains a positive Recovery effect.
    fn is_heal_skill_by_index(&self, skill_index: usize) -> bool {
        let skill = &self.skills[skill_index];
        skill.alternatives.iter().any(|alt| {
            alt.effects
                .iter()
                .any(|eff| matches!(eff.skill_type, SkillType::Recovery) && eff.modifier > 0.0)
        })
    }

    /// Update skill durations and remove expired effects.
    pub(crate) fn update_skill_durations(&mut self, elapsed_time: f64) {
        let before_count = self.active_timed_effects.len();

        // Remove expired effects. Expiring CurrentSpeedWithNaturalDeceleration
        // modifiers are transferred into the runner's speed, so that it decelerates
        // naturally instead of dropping instantly.
        self.active_timed_effects.retain(|effect| {
            let is_active = effect.expires_at > elapsed_time;
            if !is_active
                && effect.effect.skill_type == SkillType::CurrentSpeedWithNaturalDeceleration
            {
                self.speed += effect.current_value();
            }
            is_active
        });

        // Recalculate modifiers if any effects expired.
        if self.active_timed_effects.len() != before_count {
            self.recalculate_skill_modifiers();
        }
    }

    /// Recalculate skill modifiers from active timed effects.
    fn recalculate_skill_modifiers(&mut self) {
        self.skill_modifiers = SkillModifiers::from_effects(&self.active_timed_effects);
    }

    /// Extend the active effects that can still be extended by overtaking, once
    /// per overtake.
    pub(super) fn extend_effects_on_overtake(&mut self, overtakes: u16) {
        for timed in &mut self.active_timed_effects {
            let extension = &mut timed.overtake_extension;
            let count = overtakes.min(extension.remaining as u16) as u8;
            timed.expires_at += extension.duration * count as f64;
            extension.remaining -= count;
        }
    }

    /// Trigger the active additional effects applied on any of `triggers`,
    /// `count` times, up to their maximum.
    pub(super) fn trigger_additional_effects(
        &mut self,
        triggers: &[AdditionalActivation],
        count: u16,
    ) {
        let mut triggered = false;
        for timed in &mut self.active_timed_effects {
            let additional = timed.effect.additional;
            if triggers.contains(&additional) {
                let total = (timed.additional_triggers as u16 + count)
                    .min(additional.max_triggers() as u16);
                triggered |= total as u8 != timed.additional_triggers;
                timed.additional_triggers = total as u8;
            }
        }
        if triggered {
            self.recalculate_skill_modifiers();
        }
    }

    /// Schedule skills triggered in a random segment after the all-out spurt
    /// starts (`run_at_full_speed_random`). The segment is picked between the
    /// current position and the goal, and checked like any other position window.
    ///
    /// N.B. Unlike other random variables (see
    /// [`RandomRolls::roll`](crate::trigger::random::RandomRolls::roll)),
    /// it's rolled once per branch, not once per condition. In practice, at
    /// least for this variable, only one branch is left for evaluation; other
    /// branches would have contained a non-overlapping static guard that would
    /// have been false and therefore were removed when the runner was created.
    pub(super) fn schedule_all_out_spurt_triggers(&mut self, state: &RaceState) {
        let goal = state.course.length();
        let rng = &mut self.rng;
        let scheduler = &mut self.trigger_scheduler;
        for logic_idx in scheduler.all_out_spurt_triggers.drain(..) {
            let skill_index = scheduler.skill_logics[logic_idx].skill_index;
            let start = random_segment(self.position, goal, &mut rng.skills[skill_index]);
            scheduler.position_windows.push(WindowTrigger {
                logic_idx,
                min_threshold: start,
                max_threshold: start + SEGMENT_LENGTH,
            });
        }
    }
}

#[cfg(test)]
mod tests;
