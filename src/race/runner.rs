use crate::condition::evaluator::{evaluate_condition, evaluate_out_of_gate_condition};
use crate::condition::parser::{CompOp, parse_condition};
use crate::course::geometry::{Corner, Slope, Straight};
use crate::course::*;
use crate::race::hp::*;
use crate::race::last_spurt::*;
use crate::race::modifiers::*;
use crate::race::parameters::RaceParameters;
use crate::race::skill_activation::{ActiveTimedEffect, SkillActivationRecord};
use crate::race::solver::RaceState;
use crate::race::transitions::*;
use crate::trigger::builder::build_trigger_scheduler;
use crate::trigger::system::{ActivationCounts, TriggerScheduler};
use crate::uma::skill::*;
use crate::uma::*;
use rand::prelude::*;
use std::cell::RefCell;
use std::collections::HashSet;
use std::rc::Rc;

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

/// Adjust base stats according to course and race parameters.
fn adjust_stats(uma: &Uma, course: &Course, parameters: &RaceParameters) -> Uma {
    let course_modifier = course_speed_modifier(uma, &course.course_set_status);
    // n.b. In single mode, all uma gain 400 on stats after adjustment.
    // This is not implemented here.
    Uma {
        speed: calculate_adjusted_speed(uma.speed, parameters.condition, course_modifier),
        stamina: uma.stamina,
        power: calculate_adjusted_power(uma.power, course.surface, parameters.condition),
        guts: uma.guts,
        wiz: calculate_adjusted_wiz(uma.wiz, uma.track_aptitude),
        style: uma.style,
        distance_aptitude: uma.distance_aptitude,
        track_aptitude: uma.track_aptitude,
        style_aptitude: uma.style_aptitude,
    }
}

/// Modify a stat value, ensuring it doesn't go below 1
fn modify_stat(stat: &mut u16, modifier: i32) {
    *stat = if modifier >= 0 {
        stat.saturating_add(modifier as u16)
    } else {
        stat.saturating_sub((-modifier) as u16).max(1)
    };
}

/// Apply stat boost effects to Uma stats
fn apply_stat_boost(uma: &mut Uma, effect_type: &SkillType, modifier: i32) {
    match effect_type {
        SkillType::SpeedUp => modify_stat(&mut uma.speed, modifier),
        SkillType::StaminaUp => modify_stat(&mut uma.stamina, modifier),
        SkillType::PowerUp => modify_stat(&mut uma.power, modifier),
        SkillType::GutsUp => modify_stat(&mut uma.guts, modifier),
        SkillType::WizUp => modify_stat(&mut uma.wiz, modifier),
        SkillType::AllStatsUp => {
            modify_stat(&mut uma.speed, modifier);
            modify_stat(&mut uma.stamina, modifier);
            modify_stat(&mut uma.power, modifier);
            modify_stat(&mut uma.guts, modifier);
            modify_stat(&mut uma.wiz, modifier);
        }
        _ => {} // Not a stat boost effect
    }
}

/// Apply start delay modification effects
fn apply_start_delay_modification(start_delay: &mut f64, effect_type: &SkillType, modifier: i32) {
    match effect_type {
        SkillType::MultiplyStartDelay => {
            *start_delay *= modifier as f64 / 10000.0;
        }
        SkillType::SetStartDelay => {
            *start_delay = modifier as f64 / 10000.0;
        }
        _ => {} // Not a start delay effect
    }
}

/// Apply style change effects
fn apply_style_change(style: &mut Style, effect_type: &SkillType) {
    #[allow(clippy::single_match)] // Future-proofing for additional style changes
    match effect_type {
        SkillType::ChangeStyleOonige => {
            *style = Style::Oonige;
        }
        _ => {} // Not a style change effect
    }
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
    pub min_speed: f64,
    pub base_target_speed: f64,
    pub section_speed_modifier: f64,
    pub available_skills: Vec<Skill>,
    pub activated_skills: Vec<SkillActivationRecord>,
    pub activated_skill_ids: HashSet<String>,
    pub skill_modifiers: SkillModifiers,
    pub active_timed_effects: Vec<ActiveTimedEffect>,
    pub golden_skill_indices: Vec<usize>,
    pub is_rushed: bool, // TODO: implement
    pub rush_section: Option<u8>,
    pub is_start_dash: bool,
    pub slope_acceleration: Option<f64>,
    pub section: u8,
    pub phase: Phase,
    pub last_spurt: Option<LastSpurt>,
    pub rng: Rc<RefCell<rand_pcg::Pcg64Mcg>>,
    pub corner_manager: CornerManager,
    pub slope_manager: SlopeManager,
    pub straight_manager: StraightManager,
    pub trigger_scheduler: TriggerScheduler,
}

// n.b. While no trigger checks for a good start,
// it is used in Team Trials to award bonus points.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum StartType {
    Good,
    Normal,
    Late,
}

/// Builder for constructing a Runner with proper out-of-gate skill activation
pub struct RunnerBuilder {
    // Immutable context
    pub base: Uma,
    pub motivation: Motivation,
    pub gate_roll: u8,

    // Mutable state being constructed
    adjusted: Uma,
    start_delay: f64,

    // Activation tracking
    activation_counts: ActivationCounts,
    activated_skill_ids: HashSet<String>,
    activated_skills: Vec<SkillActivationRecord>,

    // RNG
    rng: Rc<RefCell<rand_pcg::Pcg64Mcg>>,

    // Skills
    trigger_scheduler: TriggerScheduler,
    golden_skill_indices: Vec<usize>,
    out_of_gate_skills: Vec<Skill>,
    runtime_skills: Vec<Skill>,
}

impl RunnerBuilder {
    /// Create a new RunnerBuilder with initial values
    pub fn new(
        uma: Uma,
        motivation: Motivation,
        course: &Course,
        params: &RaceParameters,
        skills: Vec<Skill>,
        rng: Rc<RefCell<rand_pcg::Pcg64Mcg>>,
    ) -> Self {
        // Calculate base stats
        // TODO: check for collisions; maybe use a hashset instead of a vector?
        let gate_roll: u8 = rng.borrow_mut().random_range(0..=params.gates - 1_u8);
        let base_uma = Uma::raw_to_base(uma, motivation);

        // Calculate skill activation chance and determine whether skills can activate
        // n.b. As observed, skill activation update happens before out-of-gate skills activate.
        // Thus, out-of-gate skills don't affect skill activation chance.
        let skill_chance = (1.0 - 90.0 / uma.wiz as f64).max(0.2);
        let available_skills = skills
            .into_iter()
            .filter(|skill| {
                // Unique and green skills always activate
                if skill.is_unique() || skill.is_green() {
                    return true;
                }
                // Other skills activate based on chance
                let roll: f64 = rng.borrow_mut().random();
                roll < skill_chance
            })
            .collect::<Vec<Skill>>();

        // Build trigger scheduler from the available skills (after filtering)
        let trigger_scheduler =
            build_trigger_scheduler(&available_skills, course, &mut *rng.borrow_mut());

        // Partition skills into out-of-gate and runtime skills
        let (out_of_gate_skills, runtime_skills): (Vec<_>, Vec<_>) = available_skills
            .into_iter()
            .partition(|skill| skill.should_activate_out_of_gate());

        // Calculate adjusted stats (before out-of-gate skills)
        let adjusted_uma = adjust_stats(&base_uma, course, params);

        // Determine start delay
        let start_delay = rng.borrow_mut().random_range(0.0..=0.1);

        // Build golden skill indices for random activation
        let golden_skill_indices = runtime_skills
            .iter()
            .enumerate()
            .filter(|(_, skill)| {
                skill.rarity == SkillRarity::Gold || skill.rarity == SkillRarity::Evolution
            })
            .map(|(i, _)| i)
            .collect::<Vec<usize>>();

        RunnerBuilder {
            base: base_uma,
            motivation,
            gate_roll,
            adjusted: adjusted_uma,
            start_delay,
            activation_counts: ActivationCounts::default(),
            activated_skill_ids: HashSet::new(),
            activated_skills: Vec::new(),
            rng,
            trigger_scheduler,
            golden_skill_indices,
            out_of_gate_skills,
            runtime_skills,
        }
    }

    /// Apply a single out-of-gate skill's effects
    fn apply_skill_effect(&mut self, skill: &Skill, alt_index: usize) {
        let skill_id = skill.id.clone();
        let alternative = &skill.alternatives[alt_index];

        // Apply each effect in the alternative
        for effect in &alternative.effects {
            let effect_type = &effect.skill_type;

            // Apply stat boosts
            if is_stat_boost(effect_type) {
                apply_stat_boost(&mut self.adjusted, effect_type, effect.modifier);
            }
            // Apply start delay modifications
            else if is_start_delay_modification(effect_type) {
                apply_start_delay_modification(&mut self.start_delay, effect_type, effect.modifier);
            }
            // Apply style changes
            else if is_style_change(effect_type) {
                apply_style_change(&mut self.adjusted.style, effect_type);
            }
            // Timed effects, recovery, and other runtime-only effects are not applicable out-of-gate.
            // These should not appear in out-of-gate skills, so they are gracefully ignored.
        }

        // Track skill activation
        self.activated_skill_ids.insert(skill_id.clone());
        self.activated_skills.push(SkillActivationRecord {
            skill_id,
            distance: 0.0, // Out-of-gate activations occur at position 0
        });

        // Increment activation counts (all out-of-gate skills count as "start" phase)
        self.activation_counts.increment(Phase::Early, 0.0, 0.0);
    }

    /// Apply all out-of-gate skills
    fn apply_out_of_gate_skills(&mut self, state: &RaceState) {
        let skills = self.out_of_gate_skills.clone();
        for (alt_index, skill) in skills.iter().enumerate() {
            let result = parse_condition(&skill.alternatives.first().unwrap().condition);
            evaluate_out_of_gate_condition(&result.unwrap(), state, self);
            self.apply_skill_effect(skill, alt_index);
        }
    }

    /// Build the final Runner with all modifications applied
    pub fn build(self, course: &Course) -> Runner {
        // Convert stamina to HP (using FINAL adjusted stats after out-of-gate boosts)
        let max_hp =
            0.8 * hp_style_coefficient(self.adjusted.style) * (self.adjusted.stamina as f64)
                + course.distance as f64;

        // Set starting speed (done in the constructor)
        // Determine rush chance
        let rush_chance = (0.65 / (0.1 * self.base.wiz as f64 + 1.0).log10()).powf(2.0);
        let roll: f64 = self.rng.borrow_mut().random();
        let rush_section = if roll < rush_chance {
            // Uma will rush in a random section between 2 and 9 (1-indexed)
            Some(self.rng.borrow_mut().random_range(1..=8))
        } else {
            None
        };

        // Calculate section coefficient for base speed adjustment
        let section_speed_coefficient =
            roll_section_speed_coefficient(&mut self.rng.borrow_mut(), self.adjusted.wiz);

        // Initialize race state
        Runner {
            base: self.base,
            adjusted: self.adjusted,
            motivation: self.motivation,
            gate_roll: self.gate_roll,
            speed: 3.0,
            acceleration: 0.0, // start dash acceleration is applied separately
            min_speed: 0.85 * course.base_speed()
                + (200.0 + self.adjusted.guts as f64).sqrt() * 0.001,
            base_target_speed: 0.85 * course.base_speed(),
            start_delay: self.start_delay,
            available_skills: self.runtime_skills,
            activated_skills: self.activated_skills,
            activated_skill_ids: self.activated_skill_ids,
            skill_modifiers: SkillModifiers::default(),
            active_timed_effects: Vec::new(),
            golden_skill_indices: self.golden_skill_indices,
            section: 0,
            phase: Phase::Early,
            section_speed_modifier: course.base_speed() * section_speed_coefficient,
            hp: max_hp,
            max_hp,
            position: 0.0,
            rush_section,
            is_rushed: false,
            is_start_dash: true,
            slope_acceleration: None,
            last_spurt: None,
            rng: self.rng,
            corner_manager: CornerManager::new(),
            slope_manager: SlopeManager::new(),
            straight_manager: StraightManager::new(),
            trigger_scheduler: self.trigger_scheduler,
        }
    }
}

impl Runner {
    pub fn new(mut builder: RunnerBuilder, state: &RaceState) -> Self {
        // Apply out-of-gate skills with proper activation tracking
        builder.apply_out_of_gate_skills(state); // TODO: check

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

        self.acceleration =
            base_acceleration + self.skill_modifiers.acceleration + start_dash_modifier;
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
            base_target_speed(
                course_base_speed,
                self.adjusted.style,
                self.phase,
                self.adjusted.speed,
                self.adjusted.distance_aptitude,
            )
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

    pub fn update_position(&mut self, delta_time: f64, state: &RaceState) {
        // Use effective_speed which includes current_speed modifiers
        self.position += self.effective_speed() * delta_time;
        self.check_distance_triggers(state);
    }

    pub fn update_phase(&mut self, state: &RaceState) {
        let prev_section = self.section;
        self.section = state.course.section_from_position(self.position);
        if self.section != prev_section {
            let section_speed_coefficient =
                roll_section_speed_coefficient(&mut self.rng.borrow_mut(), self.adjusted.wiz);
            self.section_speed_modifier = state.course.base_speed() * section_speed_coefficient;
            if self.rush_section.is_some_and(|s| s == self.section) {
                // TODO: implement rush logic
                //self.is_rushed = true;
            }
            // Phase can only advance at section boundaries
            let prev_phase = self.phase;
            self.phase = Phase::from_position(self.position, &state.course);
            if prev_phase != self.phase {
                self.check_phase_triggers(state);
            }
        }
    }

    pub fn update_hp(&mut self, delta_time: f64, state: &RaceState) {
        let was_empty = self.hp <= 0.0;
        self.hp -= calculate_hp_consumption_per_second(
            self.speed,
            state,
            self.phase,
            self.adjusted.guts,
            self.is_rushed,
            self.is_pace_down(),
            self.is_downhill_accel(),
        ) * delta_time;
        // Fire HpEmpty triggers if HP just became empty
        if !was_empty && self.hp <= 0.0 {
            self.check_hp_empty_triggers(state);
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
            self.pull_trigger_for_precondition(trigger_index, state);
        }
    }

    /// Check if precondition is satisfied and mark as unlocked if true
    fn pull_trigger_for_precondition(&mut self, trigger_index: usize, state: &RaceState) {
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
            && evaluate_condition(&precond, state, self)
        {
            self.trigger_scheduler.precondition_satisfied[trigger_index] = true;
        }
    }

    fn pull_trigger(&mut self, trigger_index: usize, state: &RaceState) {
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
            evaluate_condition(guard, state, self)
        } else {
            true
        };

        if should_activate {
            self.activate_skill(
                skill_index,
                alternative_index,
                state.elapsed_time,
                &state.course,
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
            self.pull_trigger(trigger_index, state);
        }
    }

    pub fn check_phase_triggers(&mut self, state: &RaceState) {
        while let Some(trigger) = self.trigger_scheduler.phase_triggers.peek() {
            if self.phase != trigger.phase {
                break; // Wrong phase
            }

            let trigger = self.trigger_scheduler.phase_triggers.pop().unwrap();
            for trigger_index in trigger.skill_indices {
                self.pull_trigger(trigger_index, state);
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
            self.pull_trigger(trigger_index, state);
        }
    }

    pub fn check_hp_empty_triggers(&mut self, _state: &RaceState) {
        // TODO: implement
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
            self.pull_trigger(trigger_index, state);
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
            self.pull_trigger(trigger_index, state);
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
            self.pull_trigger(trigger_index, state);
        }

        // Mark attempted
        for trigger_index in triggers_to_mark_attempted {
            self.trigger_scheduler.skill_logics[trigger_index].mark_attempted();
        }
    }

    pub fn update_last_spurt_state(&mut self, state: &RaceState) {
        if self.phase >= Phase::Mid && self.last_spurt.is_none() {
            calculate_last_spurt(self, state);
        }
    }

    pub fn current_corner(&self) -> Option<usize> {
        self.corner_manager.current_corner()
    }

    pub fn check_corner_start(&mut self, corners: &[Corner], state: &RaceState) {
        // Check for corner entry using the corner manager
        if let Some(corner_idx) = self.corner_manager.check_entry(self.position, corners) {
            // Find all corner entry triggers for this corner
            let skill_indices: Vec<usize> = self
                .trigger_scheduler
                .corner_entry_triggers
                .iter()
                .filter(|t| t.corner_index == corner_idx)
                .flat_map(|t| t.skill_indices.iter().copied())
                .collect();

            // Pull triggers for all skills registered for this corner entry
            for skill_index in skill_indices {
                self.pull_trigger(skill_index, state);
            }
        }
    }

    pub fn check_corner_end(&mut self, corners: &[Corner], state: &RaceState) {
        // Check for corner exit using the corner manager
        if let Some(corner_idx) = self.corner_manager.check_exit(self.position, corners) {
            // Find all corner exit triggers for this corner
            let skill_indices: Vec<usize> = self
                .trigger_scheduler
                .corner_exit_triggers
                .iter()
                .filter(|t| t.corner_index == corner_idx)
                .flat_map(|t| t.skill_indices.iter().copied())
                .collect();

            // Pull triggers for all skills registered for this corner exit
            for skill_index in skill_indices {
                self.pull_trigger(skill_index, state);
            }
        }
    }

    pub fn current_straight(&self) -> Option<usize> {
        self.straight_manager.current_straight()
    }

    pub fn check_straight_start(&mut self, straights: &[Straight], state: &RaceState) {
        // Check for straight entry using the straight manager
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
                self.pull_trigger(skill_index, state);
            }
        }
    }

    pub fn check_straight_end(&mut self, straights: &[Straight], _state: &RaceState) {
        // Check for straight exit using the straight manager
        // Note: No triggers fire on straight exit currently
        // This is here for completeness and potential future use
        if let Some(_straight_idx) = self.straight_manager.check_exit(self.position, straights) {
            // No triggers fire on exit for straights
        }
    }

    pub fn current_slope(&self) -> Option<usize> {
        self.slope_manager.current_slope()
    }

    pub fn update_slope(&mut self, slopes: &[Slope], rng: &mut impl Rng, state: &RaceState) {
        // Check for slope entry using the slope manager
        if let Some((slope_idx, _slope_type)) =
            self.slope_manager.check_entry(self.position, slopes)
        {
            // Trigger slope entry skills
            let skill_indices: Vec<usize> = self
                .trigger_scheduler
                .slope_entry_triggers
                .iter()
                .filter(|t| t.slope_index == slope_idx)
                .flat_map(|t| t.skill_indices.iter().copied())
                .collect();

            for skill_index in skill_indices {
                self.pull_trigger(skill_index, state);
            }

            if slopes[slope_idx].is_uphill() {
                self.start_uphill_acceleration(slopes[slope_idx].slope_percent());
            }
        } else if let Some(slope_idx) = self.slope_manager.check_exit(self.position, slopes) {
            // Trigger slope exit skills
            let skill_indices: Vec<usize> = self
                .trigger_scheduler
                .slope_exit_triggers
                .iter()
                .filter(|t| t.slope_index == slope_idx)
                .flat_map(|t| t.skill_indices.iter().copied())
                .collect();

            for skill_index in skill_indices {
                self.pull_trigger(skill_index, state);
            }

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
                // TODO: implement last_spurt recalculation
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
                self.activate_skill(golden_skill_idx, 0, elapsed_time, course);
            }
        }
        // Style change
        else if is_style_change(effect_type) {
            apply_style_change(&mut self.adjusted.style, effect_type);
        }
        // Unimplemented skill types
        else {
            match effect_type {
                SkillType::Fov => todo!(),
                SkillType::RushTime => todo!(),
                SkillType::LaneMovementSpeed => todo!(),
                SkillType::RushChance => todo!(),
                SkillType::LaneChange => todo!(),
                SkillType::ActivateSympathy => todo!(),
                _ => {} // Already handled above
            }
        }
    }

    /// Activate a skill by its index and alternative index
    pub fn activate_skill(
        &mut self,
        skill_index: usize,
        alternative_index: usize,
        elapsed_time: f64,
        course: &Course,
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
                SkillTarget::Oneself => {
                    self.apply_skill_effect(effect, elapsed_time, scaled_duration, course)
                }
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
    }
}
