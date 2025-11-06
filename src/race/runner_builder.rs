use std::cell::RefCell;
use std::collections::HashSet;
use std::rc::Rc;

use rand::Rng;

use crate::evaluator::{EvaluatorOutOfGate, evaluate_condition};
use crate::parser::parse_condition;
use crate::race::hp::hp_style_coefficient;
use crate::race::modifiers::roll_section_speed_coefficient;
use crate::race::runner::*;
use crate::race::skill_activation::SkillActivationRecord;
use crate::race::transitions::*;
use crate::trigger::builder::*;
use crate::trigger::system::*;
use crate::*;

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

/// Modify a stat value, ensuring it doesn't go below 1
fn modify_stat(stat: &mut u16, modifier: i32) {
    *stat = if modifier >= 0 {
        stat.saturating_add(modifier as u16)
    } else {
        stat.saturating_sub((-modifier) as u16).max(1)
    };
}

/// Check if an effect type is a rush chance effect
#[inline]
pub fn is_rush_chance_effect(effect_type: &SkillType) -> bool {
    matches!(effect_type, SkillType::RushChanceUp)
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
    rush_chance_modifier: f64,

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
            rush_chance_modifier: 0.0,
        }
    }

    /// Apply a single out-of-gate skill's effects
    fn apply_out_of_gate_skill_effect(&mut self, skill: &Skill, alt_index: usize) {
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
            // Apply rush chance modifications
            else if is_rush_chance_effect(effect_type) {
                self.rush_chance_modifier += effect.modifier as f64 / 10000.0;
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
    pub fn apply_out_of_gate_skills(&mut self, state: &RaceState) {
        let skills = self.out_of_gate_skills.clone();
        for (alt_index, skill) in skills.iter().enumerate() {
            let result = parse_condition(&skill.alternatives.first().unwrap().condition);
            evaluate_condition(&result.unwrap(), &EvaluatorOutOfGate, state, self);
            self.apply_out_of_gate_skill_effect(skill, alt_index);
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
        let rush_chance = (6.5 / (0.1 * self.base.wiz as f64 + 1.0).log10()).powf(2.0)
            + self.rush_chance_modifier;
        let roll: f64 = self.rng.borrow_mut().random_range(0.0..100.0);
        let rush_state = if roll < rush_chance {
            // Uma will rush in a random section between 2 and 9 (1-indexed)
            let section = self.rng.borrow_mut().random_range(1..=8);
            Some(RushState {
                section,
                start_time: 0.0,
                end_time: f64::MAX,
                max_duration: 12.0,
                is_rushed: false,
            })
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
            target_lane: course.starting_lane(self.gate_roll),
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
            has_emptied_hp: false,
            position: 0.0,
            rush_chance,
            rush_state,
            is_start_dash: true,
            slope_acceleration: None,
            last_spurt: None,
            rng: self.rng,
            corner_manager: CornerManager::new(),
            slope_manager: SlopeManager::new(),
            straight_manager: StraightManager::new(),
            trigger_scheduler: self.trigger_scheduler,
            finish_time: None,
            spot_struggle: None,
            dueling: None,
            duel_count: 0,
            skill_activated_flag: false,
            heal_activated_flag: false,
        }
    }
}
