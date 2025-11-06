use std::cell::RefCell;
use std::collections::HashSet;
use std::rc::Rc;

use rand::prelude::*;

use crate::condition::evaluator::{EvaluatorFull, evaluate_condition};
use crate::condition::parser::CompOp;
use crate::course::geometry::{Corner, Slope, Straight};
use crate::course::*;
use crate::race::hp::*;
use crate::race::last_spurt::*;
use crate::race::modifiers::*;
use crate::race::parameters::TrackCondition;
use crate::race::runner_builder::RunnerBuilder;
use crate::race::skill_activation::{ActiveTimedEffect, SkillActivationRecord};
use crate::race::solver::RaceState;
use crate::race::transitions::*;
use crate::trigger::system::TriggerScheduler;
use crate::uma::skill::*;
use crate::uma::*;

#[allow(dead_code)]
pub enum GameBehavior {
    Original,         // when the game was first released
    FirstAnniversary, // after the first anniversary update
}

pub static GAME_BEHAVIOR: GameBehavior = GameBehavior::Original;

/// Skill modifiers that are applied during race calculations
#[derive(Debug, Clone)]
pub struct SkillModifiers {
    pub target_speed: f64,
    pub current_speed: f64,
    pub acceleration: f64,
    pub duration_scaling: f64, // Multiplicative, defaults to 1.0
}

impl Default for SkillModifiers {
    fn default() -> Self {
        Self {
            target_speed: 0.0,
            current_speed: 0.0,
            acceleration: 0.0,
            duration_scaling: 1.0, // Multiplicative default
        }
    }
}

/// Apply style change effects
#[inline]
pub fn apply_style_change(style: &mut Style, effect_type: &SkillType) {
    #[allow(clippy::single_match)] // Account for possible future additions
    match effect_type {
        SkillType::ChangeStyleRunaway => {
            *style = Style::Runaway;
        }
        _ => {} // Not a style change effect
    }
}

/// Check if an effect type is a stat boost (applied permanently)
#[inline]
pub fn is_stat_boost(effect_type: &SkillType) -> bool {
    matches!(
        effect_type,
        SkillType::SpeedUp
            | SkillType::StaminaUp
            | SkillType::PowerUp
            | SkillType::GutsUp
            | SkillType::WizUp
            | SkillType::AllStatsUp
    )
}

/// Check if an effect type is a start delay modification
#[inline]
pub fn is_start_delay_modification(effect_type: &SkillType) -> bool {
    matches!(
        effect_type,
        SkillType::MultiplyStartDelay | SkillType::SetStartDelay
    )
}

/// Check if an effect type is a style change
#[inline]
pub fn is_style_change(effect_type: &SkillType) -> bool {
    matches!(effect_type, SkillType::ChangeStyleRunaway)
}

/// Check if an effect type is a timed effect
#[inline]
pub fn is_timed_effect(effect_type: &SkillType) -> bool {
    matches!(
        effect_type,
        SkillType::Acceleration
            | SkillType::TargetSpeed
            | SkillType::CurrentSpeed
            | SkillType::CurrentSpeedWithNaturalDeceleration
            | SkillType::ExtendEvolvedDuration
    )
}

/// State related to rushing (temptation, 掛かり)
pub struct RushState {
    pub section: u8,
    pub start_time: f64,
    pub end_time: f64,
    pub max_duration: f64,
    pub is_rushed: bool,
}

/// Modifier applied during spot struggle and its expiration time
pub struct StruggleState {
    modifier: f64,
    expires_at: f64,
}

/// Modifiers applied during dueling
pub struct DuelingState {
    target_speed_modifier: f64,
    acceleration_modifier: f64,
}

/// An Uma with additional data related to racing,
/// that is, an Uma who is running.
pub struct Runner {
    pub base: Uma,
    pub adjusted: Uma,
    pub motivation: Motivation,
    pub start_delay: f64, // in seconds
    pub gate_roll: u8,
    pub speed: f64,        // in m/s
    pub acceleration: f64, // in m/s^2
    pub position: f64,     // in m
    pub hp: f64,
    pub max_hp: f64,
    pub has_emptied_hp: bool,
    pub min_speed: f64,
    pub base_target_speed: f64,
    pub target_lane: f64,
    pub section_speed_modifier: f64,
    pub available_skills: Vec<Skill>,
    pub activated_skills: Vec<SkillActivationRecord>,
    pub activated_skill_ids: HashSet<String>,
    pub skill_modifiers: SkillModifiers,
    pub active_timed_effects: Vec<ActiveTimedEffect>,
    pub golden_skill_indices: Vec<usize>,
    pub rush_chance: f64,
    pub rush_state: Option<RushState>,
    pub is_start_dash: bool,
    pub spot_struggle: Option<StruggleState>,
    pub dueling: Option<DuelingState>,
    pub duel_count: u8,
    pub slope_acceleration: Option<f64>,
    pub section: u8,
    pub phase: Phase,
    pub last_spurt: Option<LastSpurt>,
    pub rng: Rc<RefCell<rand_pcg::Pcg64Mcg>>,
    pub corner_manager: CornerManager,
    pub slope_manager: SlopeManager,
    pub straight_manager: StraightManager,
    pub trigger_scheduler: TriggerScheduler,
    pub finish_time: Option<f64>,
    pub skill_activated_flag: bool,
    pub heal_activated_flag: bool,
}

// n.b. While no trigger checks for a good start,
// it is used in Team Trials to award bonus points.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum StartType {
    Good,
    Normal,
    Late,
}

impl Runner {
    pub fn new(mut builder: RunnerBuilder, state: &RaceState) -> Self {
        // Apply out-of-gate skills with proper activation tracking
        builder.apply_out_of_gate_skills(state);

        // Build final Runner with all modifications applied
        builder.build(&state.course)
    }

    /// Recalculate skill modifiers from active timed effects
    pub fn recalculate_skill_modifiers(&mut self) {
        let mut mods = SkillModifiers::default();

        for effect in &self.active_timed_effects {
            match effect.effect.skill_type {
                SkillType::Acceleration => {
                    mods.acceleration += effect.effect.modifier as f64;
                }
                SkillType::TargetSpeed => {
                    mods.target_speed += effect.effect.modifier as f64;
                }
                SkillType::CurrentSpeed | SkillType::CurrentSpeedWithNaturalDeceleration => {
                    mods.current_speed += effect.effect.modifier as f64;
                }
                SkillType::ExtendEvolvedDuration => {
                    // Duration scaling is multiplicative
                    mods.duration_scaling *= effect.effect.modifier as f64 / 10000.0;
                }
                _ => {}
            }
        }

        self.skill_modifiers = mods;
    }

    pub fn start_type(&self) -> StartType {
        match self.start_delay {
            d if d < 0.02 => StartType::Good,
            d if d > 0.08 => StartType::Late,
            _ => StartType::Normal,
        }
    }

    /// Check if the runner is currently in a rushed state
    pub fn is_rushed(&self) -> bool {
        self.rush_state.as_ref().is_some_and(|s| s.is_rushed)
    }

    // Check if the runner has rushed at some point
    pub fn has_rushed(&self) -> bool {
        self.rush_state.as_ref().is_some_and(|s| s.start_time > 0.0)
    }

    pub fn has_finished(&self) -> bool {
        self.finish_time.is_some()
    }

    pub fn accelerate(&mut self) {
        let base_accel = if self.is_uphill() { 0.0004 } else { 0.0006 };

        let power_component = (500.0 * self.adjusted.power as f64).sqrt();
        let strategy_coef =
            acceleration_strategy_phase_coefficient(self.adjusted.style, self.phase);
        let ground_modifier = ground_type_proficiency_modifier(self.adjusted.track_aptitude);
        let distance_modifier =
            acceleration_distance_proficiency_modifier(self.adjusted.distance_aptitude);

        let base_acceleration =
            base_accel * power_component * strategy_coef * ground_modifier * distance_modifier;

        let start_dash_modifier = if self.is_start_dash { 24.0 } else { 0.0 };

        let dueling_modifier = self
            .dueling
            .as_ref()
            .map_or(0.0, |d| d.acceleration_modifier);

        self.acceleration = base_acceleration
            + self.skill_modifiers.acceleration
            + start_dash_modifier
            + dueling_modifier;
    }

    pub fn decelerate(&mut self) {
        self.acceleration = if self.hp <= 0.0 {
            -1.2
        // } else if pace_down_mode {
        //     -0.5
        } else {
            match self.phase {
                Phase::Early => -1.2,
                Phase::Mid => -0.8,
                Phase::Late | Phase::Last => -1.0,
            }
        };
    }

    pub fn target_speed(&self) -> f64 {
        self.base_target_speed
            + self.slope_acceleration.unwrap_or(0.0)
            + self.skill_modifiers.target_speed
            + self
                .dueling
                .as_ref()
                .map_or(0.0, |d| d.target_speed_modifier)
    }

    pub fn is_uphill(&self) -> bool {
        self.slope_acceleration.is_some_and(|s| s > 0.0)
    }

    pub fn is_downhill_accel(&self) -> bool {
        self.slope_acceleration.is_some_and(|s| s < 0.0)
    }

    pub fn start_uphill_acceleration(&mut self, slope: f64) {
        self.slope_acceleration = Some(-slope * 200.0 / self.adjusted.power as f64);
    }

    pub fn start_downhill_acceleration(&mut self, slope: f64) {
        if slope > 0.0 {
            panic!("Not a downhill slope");
        }
        self.slope_acceleration = Some(-slope / 10.0 + 0.3);
    }

    pub fn stop_slope_acceleration(&mut self) {
        self.slope_acceleration = None;
    }

    pub fn is_pace_down(&self) -> bool {
        // TODO: implement
        false
    }

    pub fn update_target_speed(&mut self, course_base_speed: f64) {
        self.base_target_speed = if let Some(last_spurt) = &self.last_spurt
            && self.position >= last_spurt.target_distance
        {
            last_spurt.max_speed
        } else {
            let struggle_mod = self.spot_struggle.as_ref().map_or(0.0, |s| s.modifier);
            base_target_speed(
                course_base_speed,
                self.adjusted.style,
                self.phase,
                self.adjusted.speed,
                self.adjusted.distance_aptitude,
            ) + struggle_mod
        };
    }

    pub fn update_speed(&mut self, delta_time: f64, course_base_speed: f64) {
        // Update speed based on acceleration
        self.speed += self.acceleration * delta_time;

        // Clamp to target speed
        if self.speed > self.target_speed() {
            self.speed = self.target_speed();
        }

        // Handle start dash end and min speed
        if self.is_start_dash && self.speed >= 0.85 * course_base_speed {
            self.is_start_dash = false;
        } else if !self.is_start_dash && self.speed < self.min_speed {
            self.speed = self.min_speed;
        }
    }

    pub fn update_acceleration(&mut self) {
        if self.speed < self.target_speed() {
            self.accelerate();
        } else {
            self.decelerate();
        }
    }

    /// Get the effective speed including skill modifiers
    /// This is the actual speed used for position calculations
    pub fn effective_speed(&self) -> f64 {
        self.speed + self.skill_modifiers.current_speed + self.section_speed_modifier
    }

    /// Update the runner's position based on effective current speed.
    pub fn update_position(&mut self, delta_time: f64, state: &RaceState) {
        // Adjust delta time if the start of the frame is right before start delay
        // (By calling this function, we are guaranteed that the runner should have started on this frame)
        let delta_time = if (state.elapsed_time - delta_time) < self.start_delay {
            delta_time - self.start_delay
        } else {
            delta_time
        };
        let prev_position = self.position;
        // Use effective_speed which includes current_speed modifiers
        self.position += self.effective_speed() * delta_time;
        if self.position >= state.course.distance as f64 {
            // Calculate exact finish time
            let remaining_distance = state.course.distance as f64 - prev_position;
            let time_to_finish = remaining_distance / self.effective_speed();
            self.finish_time = Some(state.elapsed_time - delta_time + time_to_finish);
        }
        self.check_distance_triggers(state);
    }

    fn start_rush(&mut self, state: &mut RaceState) {
        if let Some(rush_state) = &mut self.rush_state
            && !rush_state.is_rushed
        {
            rush_state.is_rushed = true;
            rush_state.start_time = state.elapsed_time;
            // Rushed for at least 3 seconds
            rush_state.end_time = state.elapsed_time + 3.0;
            state
                .rushed_by_style
                .entry(self.base.style)
                .and_modify(|e| *e += 1)
                .or_insert(1);
            // TODO: implement position strat change
        }
    }

    fn end_rush(&mut self, state: &mut RaceState) {
        if let Some(rush_state) = &mut self.rush_state
            && rush_state.is_rushed
        {
            rush_state.is_rushed = false;
            rush_state.end_time = state.elapsed_time; // Ensure end time is current
            state
                .rushed_by_style
                .entry(self.base.style)
                .and_modify(|e| *e -= 1);
        }
    }

    pub fn update_rush_state(&mut self, state: &mut RaceState) {
        if let Some(rush_state) = &mut self.rush_state
            && rush_state.is_rushed
        {
            if state.elapsed_time >= rush_state.start_time + rush_state.max_duration {
                // End rush after max duration
                self.end_rush(state);
                return;
            }
            if state.elapsed_time >= rush_state.end_time {
                // 55% chance of snapping out
                let roll: f64 = self.rng.borrow_mut().random_range(0.0..100.0);
                if roll < 55.0 {
                    self.end_rush(state);
                } else {
                    // Extend rush by 3 seconds
                    rush_state.end_time = state.elapsed_time + 3.0;
                }
            }
        }
    }

    pub fn update_phase(&mut self, state: &mut RaceState) {
        let prev_section = self.section;
        self.section = state.course.section_from_position(self.position);
        if self.section != prev_section {
            let section_speed_coefficient =
                roll_section_speed_coefficient(&mut self.rng.borrow_mut(), self.adjusted.wiz);
            self.section_speed_modifier = state.course.base_speed() * section_speed_coefficient;
            if let Some(rush_state) = &mut self.rush_state
                && rush_state.section == self.section
                && !rush_state.is_rushed
                && rush_state.end_time < state.elapsed_time
            {
                self.start_rush(state);
            }
            // Phase can only advance at section boundaries
            let prev_phase = self.phase;
            self.phase = Phase::from_position(self.position, &state.course);
            if prev_phase != self.phase {
                self.check_phase_triggers(state);
            }
        }
    }

    /// Update HP based on current speed, phase, and modifiers
    pub fn update_hp(&mut self, delta_time: f64, state: &RaceState) {
        let mut coefficient = if let Some(struggle) = &self.spot_struggle
            && state.elapsed_time < struggle.expires_at
        {
            if self.adjusted.style == Style::Runaway {
                if self.is_rushed() { 7.7 } else { 3.5 }
            } else if self.adjusted.style == Style::Front {
                if self.is_rushed() { 3.6 } else { 1.4 }
            } else {
                panic!("BUG: spot struggle should only apply to Runaway and Front styles");
            }
        } else if self.is_rushed() {
            1.5
        } else {
            1.0
        };
        if self.is_pace_down() {
            coefficient *= 0.6;
        }
        if self.is_downhill_accel() {
            coefficient *= 0.4;
        }
        self.hp -= calculate_hp_consumption_per_second(
            self.speed,
            &state.course,
            state.params.condition,
            self.phase,
            self.adjusted.guts,
            coefficient,
        ) * delta_time;
        if self.hp <= 0.0 {
            self.has_emptied_hp = true;
        }
        // Exit dueling state if HP is below 5%
        if self.hp_percent() < 5.0 {
            self.dueling = None;
        }
        self.check_hp_triggers(state);
    }

    pub fn hp_percent(&self) -> f64 {
        self.hp / self.max_hp * 100.0
    }

    /// Check if a monotonic trigger should fire based on operator
    fn should_fire_trigger(current_value: f64, threshold: f64, operator: CompOp) -> bool {
        match operator {
            CompOp::GreaterEqual => current_value >= threshold,
            CompOp::Greater => current_value > threshold,
            CompOp::LessEqual => current_value <= threshold,
            CompOp::Less => current_value < threshold,
            CompOp::Equal => (current_value - threshold).abs() < 0.01,
            CompOp::NotEqual => (current_value - threshold).abs() >= 0.01,
        }
    }

    /// Check all preconditions to detect satisfaction at any point during the race
    /// This must be called every frame to implement proper "one-time unlock" semantics
    /// where preconditions can be satisfied at any time, not just when triggers fire
    pub fn check_all_preconditions(&mut self, state: &RaceState) {
        // Iterate through all skills and check their preconditions
        let skill_count = self.trigger_scheduler.skill_logics.len();
        for trigger_index in 0..skill_count {
            self.check_precondition(trigger_index, state);
        }
    }

    /// Check if precondition is satisfied and mark as unlocked if true
    fn check_precondition(&mut self, trigger_index: usize, state: &RaceState) {
        // Skip if precondition already satisfied
        if self.trigger_scheduler.precondition_satisfied[trigger_index] {
            return;
        }

        // Evaluate precondition
        let precondition = {
            let logic = &self.trigger_scheduler.skill_logics[trigger_index];
            logic.precondition.clone()
        };
        if let Some(precond) = precondition
            && evaluate_condition(&precond, &EvaluatorFull, state, self)
        {
            self.trigger_scheduler.precondition_satisfied[trigger_index] = true;
        }
    }

    /// Given a trigger index, attempt to activate the associated skill
    fn try_activate_skill(&mut self, trigger_index: usize, state: &RaceState) {
        // Check if precondition is satisfied; skip if not
        if !self.trigger_scheduler.precondition_satisfied[trigger_index] {
            return;
        }

        // Get skill logic and conditions
        let (skill_index, alternative_index, guard_condition, is_onetime) = {
            let logic = &mut self.trigger_scheduler.skill_logics[trigger_index];
            if !logic.should_check() {
                return;
            }
            (
                logic.skill_index,
                logic.alternative_index,
                logic.guard_condition.clone(),
                logic.triggers.iter().any(|t| t.is_onetime()),
            )
        };
        let should_activate = if let Some(guard) = &guard_condition {
            evaluate_condition(guard, &EvaluatorFull, state, self)
        } else {
            true
        };

        if should_activate {
            self.activate_skill(
                skill_index,
                alternative_index,
                state.elapsed_time,
                &state.course,
                state.params.condition,
            );
            self.trigger_scheduler.skill_logics[trigger_index].mark_activated();
        } else if is_onetime {
            // Onetime trigger that failed guards - mark attempted so we don't retry
            self.trigger_scheduler.skill_logics[trigger_index].mark_attempted();
        }
        // Note: Monotonic triggers that fail guards are NOT marked as attempted
        // This allows window triggers to retry them continuously while in window
    }

    pub fn check_continuous_triggers(&mut self, state: &RaceState) {
        // Clone indices to avoid borrow conflicts
        let continuous_indices: Vec<usize> = self.trigger_scheduler.continuous_triggers.to_vec();

        for trigger_index in continuous_indices {
            self.try_activate_skill(trigger_index, state);
        }
    }

    /// Reset activation flags at the start of each frame
    pub fn reset_activation_flags(&mut self) {
        self.skill_activated_flag = false;
        self.heal_activated_flag = false;
    }

    /// Check activation-dependent triggers
    /// This should be called at the end of each frame after all other triggers have been checked
    pub fn check_activation_dependent_triggers(&mut self, state: &RaceState) {
        // Pass 1: Check is_activate_any_skill triggers
        if self.skill_activated_flag {
            let any_activation_indices: Vec<usize> =
                self.trigger_scheduler.any_activation_triggers.to_vec();

            for trigger_index in any_activation_indices {
                self.try_activate_skill(trigger_index, state);
            }
        }

        // Pass 2: Check is_activate_heal_skill triggers
        if self.heal_activated_flag {
            let heal_activation_indices: Vec<usize> =
                self.trigger_scheduler.heal_activation_triggers.to_vec();

            for trigger_index in heal_activation_indices {
                self.try_activate_skill(trigger_index, state);
            }
        }
    }

    pub fn check_phase_triggers(&mut self, state: &RaceState) {
        while let Some(trigger) = self.trigger_scheduler.phase_triggers.peek() {
            if self.phase != trigger.phase {
                break; // Wrong phase
            }

            let trigger = self.trigger_scheduler.phase_triggers.pop().unwrap();
            for trigger_index in trigger.skill_indices {
                self.try_activate_skill(trigger_index, state);
            }
        }
    }

    pub fn check_distance_triggers(&mut self, state: &RaceState) {
        while let Some(trigger) = self.trigger_scheduler.distance_triggers.peek() {
            if !Self::should_fire_trigger(self.position, trigger.threshold_meters, trigger.operator)
            {
                break; // Threshold not reached yet, this and all subsequent triggers require further distance
            }

            let trigger = self.trigger_scheduler.distance_triggers.pop().unwrap();
            let trigger_index = trigger.skill_index;
            self.try_activate_skill(trigger_index, state);
        }
    }

    pub fn check_hp_triggers(&mut self, state: &RaceState) {
        while let Some(trigger) = self.trigger_scheduler.hp_triggers.peek() {
            if !Self::should_fire_trigger(
                self.hp_percent(),
                trigger.threshold_percent,
                trigger.operator,
            ) {
                break; // Threshold not reached yet, this and all subsequent triggers require lower HP
            }

            let trigger = self.trigger_scheduler.hp_triggers.pop().unwrap();
            let trigger_index = trigger.skill_index;
            self.try_activate_skill(trigger_index, state);
        }
    }

    pub fn check_time_triggers(&mut self, state: &RaceState) {
        while let Some(trigger) = self.trigger_scheduler.time_triggers.peek() {
            if !Self::should_fire_trigger(
                state.elapsed_time,
                trigger.threshold_seconds,
                trigger.operator,
            ) {
                break; // Threshold not reached yet, this and all subsequent triggers require more time
            }

            let trigger = self.trigger_scheduler.time_triggers.pop().unwrap();
            let trigger_index = trigger.skill_index;
            self.try_activate_skill(trigger_index, state);
        }
    }

    pub fn check_window_triggers(&mut self, state: &RaceState) {
        use crate::race::skill_activation::WindowVariable;

        // Collect trigger checks to avoid borrowing conflicts
        let mut triggers_to_pull = Vec::new();
        let mut triggers_to_mark_attempted = Vec::new();

        for trigger in &self.trigger_scheduler.window_triggers {
            // Get current value for the window variable
            let current_value = match trigger.variable {
                WindowVariable::AccumulateTime => state.elapsed_time,
                WindowVariable::DistanceRate => self.position, // Already in meters (converted at build time)
                WindowVariable::HpPercent => self.hp_percent(),
                WindowVariable::SpatialPosition => self.position, // Spatial features use position directly
            };

            // Check if we're within the window
            if current_value >= trigger.min_threshold && current_value <= trigger.max_threshold {
                // Within window - try activation
                triggers_to_pull.push(trigger.skill_index);
            } else if current_value > trigger.max_threshold {
                // Exited window without activating - check if we should mark as attempted
                let logic = &self.trigger_scheduler.skill_logics[trigger.skill_index];
                if !logic.state.has_activated && !logic.state.onetime_trigger_attempted {
                    triggers_to_mark_attempted.push(trigger.skill_index);
                }
            }
            // If current_value < min_threshold, we haven't entered the window yet (do nothing)
        }

        // Pull triggers
        for trigger_index in triggers_to_pull {
            self.try_activate_skill(trigger_index, state);
        }

        // Mark attempted
        for trigger_index in triggers_to_mark_attempted {
            self.trigger_scheduler.skill_logics[trigger_index].mark_attempted();
        }
    }

    /// a.k.a. Lead Competition, 位置取り争い
    pub fn start_spot_struggle(&mut self, elapsed_time: f64) {
        let modifier = (500.0 * self.adjusted.guts as f64).powf(0.6) * 0.0001;
        let duration = (700.0 * self.adjusted.guts as f64).sqrt() * 0.012;
        self.spot_struggle = Some(StruggleState {
            modifier,
            expires_at: elapsed_time + duration,
        });
    }

    /// a.k.a. Compete Fight, 追い比べ
    pub fn start_dueling(&mut self) {
        let target_speed_modifier = (200.0 * self.adjusted.guts as f64).powf(0.708) * 0.0001;
        let acceleration_modifier = (160.0 * self.adjusted.guts as f64).powf(0.59) * 0.0001;
        self.dueling = Some(DuelingState {
            target_speed_modifier,
            acceleration_modifier,
        });
        self.duel_count += 1;
    }

    /// Check if the runner is currently in a spot struggle (位置取り争い)
    pub fn is_spot_struggling(&self) -> bool {
        self.spot_struggle.is_some()
    }

    /// Check if the runner is currently dueling (追い比べ)
    pub fn is_dueling(&self) -> bool {
        self.dueling.is_some()
    }

    pub fn update_last_spurt_state(&mut self, course: &Course, condition: TrackCondition) {
        if self.phase >= Phase::Mid && self.last_spurt.is_none() {
            calculate_last_spurt(self, course, condition);
        }
    }

    pub fn current_corner(&self) -> Option<usize> {
        self.corner_manager.current_corner()
    }

    pub fn check_corner_start(&mut self, corners: &[Corner]) {
        self.corner_manager.check_entry(self.position, corners);
    }

    pub fn check_corner_end(&mut self, corners: &[Corner]) {
        self.corner_manager.check_exit(self.position, corners);
    }

    pub fn current_straight(&self) -> Option<usize> {
        self.straight_manager.current_straight()
    }

    pub fn check_straight_start(&mut self, straights: &[Straight], state: &RaceState) {
        if let Some(straight_idx) = self.straight_manager.check_entry(self.position, straights) {
            // Trigger straight entry skills
            // Note: straight triggers are only for entry, not exit
            let skill_indices: Vec<usize> = self
                .trigger_scheduler
                .straight_triggers
                .iter()
                .filter(|t| t.straight_index == straight_idx)
                .flat_map(|t| t.skill_indices.iter().copied())
                .collect();

            for skill_index in skill_indices {
                self.try_activate_skill(skill_index, state);
            }
        }
    }

    pub fn check_straight_end(&mut self, straights: &[Straight]) {
        // Check for straight exit only; no triggers are fired by this event
        self.straight_manager.check_exit(self.position, straights);
    }

    pub fn current_slope(&self) -> Option<usize> {
        self.slope_manager.current_slope()
    }

    pub fn update_slope(&mut self, slopes: &[Slope], rng: &mut impl Rng) {
        if let Some((slope_idx, _slope_type)) =
            self.slope_manager.check_entry(self.position, slopes)
        {
            if slopes[slope_idx].is_uphill() {
                self.start_uphill_acceleration(slopes[slope_idx].slope_percent());
            }
        } else if let Some(_slope_idx) = self.slope_manager.check_exit(self.position, slopes) {
            self.stop_slope_acceleration();
        } else if let Some(idx) = self.current_slope() {
            // On slope - check for downhill acceleration
            if slopes[idx].is_downhill() {
                let has_second_passed = false; // TODO: track time on slope
                if has_second_passed {
                    if self.is_downhill_accel() {
                        // 20% chance to stop each second
                        let roll: f64 = rng.random();
                        if roll < 0.2 {
                            self.stop_slope_acceleration();
                        }
                    } else {
                        let roll: f64 = rng.random();
                        let chance = self.adjusted.wiz as f64 * 0.0004;
                        if roll < chance {
                            self.start_downhill_acceleration(slopes[idx].slope_percent());
                        }
                    }
                }
            }
        }
    }

    pub fn lane_type(&self, course: &Course) -> LaneType {
        // TODO: track current lane then use that instead of target lane
        let percent = self.target_lane / course.width();
        if percent <= 0.2 {
            LaneType::Inner
        } else if percent <= 0.4 {
            LaneType::Middle
        } else if percent <= 0.6 {
            LaneType::Outer
        } else {
            LaneType::Outside
        }
    }

    /// Update skill durations and remove expired effects
    pub fn update_skill_durations(&mut self, elapsed_time: f64) {
        let before_count = self.active_timed_effects.len();

        // Remove expired effects
        self.active_timed_effects
            .retain(|effect| effect.expires_at > elapsed_time);

        // Recalculate modifiers if any effects expired
        if self.active_timed_effects.len() != before_count {
            self.recalculate_skill_modifiers();
        }
    }

    pub fn apply_skill_effect(
        &mut self,
        effect: &SkillEffect,
        elapsed_time: f64,
        scaled_duration: f64,
        course: &Course,
        condition: TrackCondition,
    ) {
        let effect_type = &effect.skill_type;

        // Stat boosts and start delay modifications should have been applied out of gate.
        // Therefore, these are gracefully ignored here.

        // Timed effects (tracked in active_timed_effects)
        if is_timed_effect(effect_type) {
            let expires_at = elapsed_time + scaled_duration;
            self.active_timed_effects.push(ActiveTimedEffect {
                effect: effect.clone(),
                expires_at,
            });
        }
        // Recovery (HP restoration)
        else if matches!(effect_type, SkillType::Recovery) {
            self.hp = (self.hp + effect.modifier as f64).min(self.max_hp);

            // Increment heal skill count
            self.trigger_scheduler.activation_counts.increment_heal();

            // Recalculate last spurt if in Mid phase or later (FirstAnniversary behavior)
            if !matches!(GAME_BEHAVIOR, GameBehavior::Original) && self.phase >= Phase::Mid {
                calculate_last_spurt(self, course, condition);
            }
        }
        // Activate random gold skill
        else if matches!(effect_type, SkillType::ActivateRandomGold) {
            if !self.golden_skill_indices.is_empty() {
                let idx = self
                    .rng
                    .borrow_mut()
                    .random_range(0..self.golden_skill_indices.len());
                let golden_skill_idx = self.golden_skill_indices[idx];
                // Recursively activate the golden skill (use first alternative)
                self.activate_skill(golden_skill_idx, 0, elapsed_time, course, condition);
            }
        }
        // Style change
        else if is_style_change(effect_type) {
            apply_style_change(&mut self.adjusted.style, effect_type);
        }
        // Rush time extension
        else if matches!(effect_type, SkillType::RushTimeUp) {
            // Extend only if the Uma is bound to rush at all
            // ? Does this apply as well if already rushing? Assume it does for now
            if let Some(rush_state) = &mut self.rush_state {
                rush_state.max_duration += effect.modifier as f64 / 10000.0;
            }
        }
        // FOV change
        else if matches!(effect_type, SkillType::Fov) {
            // TODO: implement FOV change
        }
        // Lane movement speed change
        else if matches!(effect_type, SkillType::LaneMovementSpeed) {
            // TODO: implement lane movement speed change
        }
        // Lane change
        else if matches!(effect_type, SkillType::LaneChange) {
            // Change target lane to (modifier/100)% of course width
            // TODO: review
            let lane_change_percent = effect.modifier as f64 / 10000.0;
            self.target_lane = course.width() * lane_change_percent;
        }
        // "Sympathy" skill activation
        else if matches!(effect_type, SkillType::ActivateSympathy) {
            // Force activation of skill with ID "201631" if available
            if let Some((skill_index, _)) = self
                .available_skills
                .iter()
                .enumerate()
                .find(|(_, skill)| skill.id == "201631")
            {
                self.activate_skill(skill_index, 0, elapsed_time, course, condition);
            }
        }
    }

    /// Check if a skill at the given index is a heal skill
    /// A skill is considered a heal skill if any of its alternatives contain a Recovery effect
    fn is_heal_skill_by_index(&self, skill_index: usize) -> bool {
        use crate::uma::skill::SkillType;

        let skill = &self.available_skills[skill_index];
        skill.alternatives.iter().any(|alt| {
            alt.effects
                .iter()
                .any(|eff| matches!(eff.skill_type, SkillType::Recovery))
        })
    }

    /// Activate a skill by its index and alternative index
    pub fn activate_skill(
        &mut self,
        skill_index: usize,
        alternative_index: usize,
        elapsed_time: f64,
        course: &Course,
        condition: TrackCondition,
    ) {
        // Clone the skill data we need to avoid borrow checker issues
        let skill_id = self.available_skills[skill_index].id.clone();
        let alternative =
            self.available_skills[skill_index].alternatives[alternative_index].clone();

        // Calculate scaled duration
        let base_duration = alternative.base_duration as f64 / 10000.0; // Convert from 10000ths of a second
        let scaled_duration = base_duration * self.skill_modifiers.duration_scaling;

        // Apply each effect in the alternative
        for effect in &alternative.effects {
            match effect.target {
                SkillTarget::Oneself => self.apply_skill_effect(
                    effect,
                    elapsed_time,
                    scaled_duration,
                    course,
                    condition,
                ),
                _ => continue, // TODO: Handle other targets
            }
        }

        // Recalculate skill modifiers from active effects
        self.recalculate_skill_modifiers();

        // Track skill activation
        self.activated_skill_ids.insert(skill_id.clone());
        self.activated_skills.push(SkillActivationRecord {
            skill_id,
            distance: self.position,
        });

        // Increment activation counts for condition evaluation
        let section_length = course.distance as f64 / 24.0;
        self.trigger_scheduler.activation_counts.increment(
            self.phase,
            self.position,
            section_length,
        );

        // Set activation flags for activation-dependent triggers
        self.skill_activated_flag = true;
        if self.is_heal_skill_by_index(skill_index) {
            self.heal_activated_flag = true;
        }
    }
}
