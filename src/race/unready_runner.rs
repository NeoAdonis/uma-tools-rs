use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

use rand::Rng;

use crate::configuration::{GAME_BEHAVIOR, GameBehavior};
use crate::evaluator::{EvaluatorOutOfGate, evaluate_condition};
use crate::parser::parse_condition;
use crate::race::hp::{HpState, hp_style_multiplier};
use crate::race::modifiers::roll_section_speed_modifier;
use crate::race::popularity::JudgeScores;
use crate::race::runner::*;
use crate::race::transitions::*;
use crate::trigger::builder::*;
use crate::trigger::skill_activation::SkillActivationRecord;
use crate::trigger::system::*;
use crate::*;

/// Adjust base stats according to course and race parameters.
fn adjust_stats(uma: &Uma, course: &Course, parameters: &RaceParameters) -> Uma {
    let course_modifier = course_speed_modifier(uma, &course.course_set_status);
    // N.B. In single mode, all uma gain 400 on stats after adjustment.
    // This is not implemented here.
    Uma {
        speed: calculate_adjusted_speed(uma.speed, parameters.track_condition, course_modifier),
        stamina: uma.stamina,
        power: calculate_adjusted_power(uma.power, course.surface, parameters.track_condition),
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

/// A data structure that holds all information needed to initialize a Runner.
/// It allows for proper activation of out-of-gate skills that can take into
/// account the runner's base stats, race parameters, and most importantly,
/// data about other runners.
pub struct UnreadyRunner {
    // === Immutable context ===
    /// Base stats before adjustments. Also used for skill conditions in
    /// out-of-gate skills.
    pub base: Uma,
    /// Gate number (1-indexed). It determines starting lane position.
    /// It is also used as a unique identifier for the runner.
    pub gate_roll: u8,

    // === Mutable state being constructed ===
    /// Set of static values related to the runner at the start of the race.
    pub initial_state: InitialState,
    /// Adjusted stats after applying modifiers and out-of-gate skills.
    /// For the most part, these are the stats used for race calculations.
    adjusted: Uma,
    /// Start delay (i.e. time taken to leave the gate) in seconds.
    start_delay: f64,
    /// Rush chance modifier from out-of-gate skills.
    rush_chance_modifier: f64,

    // === Activation tracking ===
    /// Counts of skill activations by phase and type.
    activation_counts: ActivationCounts,
    /// Set of activated skill IDs.
    activated_skill_ids: HashSet<String>,
    /// List of activated skills with their activation distances.
    activated_skills: Vec<SkillActivationRecord>,

    // === Skills ===
    /// Scheduler for skill triggers.
    trigger_scheduler: TriggerScheduler,
    /// Skills that can be activated after the race starts.
    pub runtime_skills: Vec<Skill>,
    /// Skills that activate out of gate as soon as the race starts.
    pub out_of_gate_skills: Vec<Skill>,
    /// Indices of golden skills in `runtime_skills` for activation by skills that
    /// activate golden skills.
    golden_skill_indices: Vec<usize>,
    /// Temporary: Number of runners that have the same skill as the one currently being
    /// evaluated. Used for conditions like `same_skill_horse_count` that need to know
    /// which skill they're checking.
    pub same_skill_count: u8,

    // === Random number generator ===
    /// Shared RNG for this runner. It is cloned from the RaceSolver's RNG.
    rng: Rc<RefCell<rand_pcg::Pcg64Mcg>>,
}

impl UnreadyRunner {
    /// Create a new RunnerBuilder with initial values
    pub fn new(
        uma: Uma,
        gate_roll: u8,
        motivation: Motivation,
        course: &Course,
        params: &RaceParameters,
        skills: Vec<Skill>,
        rng: Rc<RefCell<rand_pcg::Pcg64Mcg>>,
    ) -> Self {
        // Calculate base stats
        let gate_block = Self::get_gate_bracket(gate_roll, params.gates);
        let base_uma = Uma::raw_to_base(uma, motivation);

        // Calculate skill activation chance and determine whether skills can activate
        // N.B. As observed, skill activation update happens before out-of-gate skills activate.
        // Thus, out-of-gate skills don't affect skill activation chance.
        let skill_chance = (1.0 - 90.0 / base_uma.wiz as f64).max(0.2);
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

        UnreadyRunner {
            base: base_uma,
            gate_roll,
            adjusted: adjusted_uma,
            initial_state: InitialState {
                motivation,
                gate_block,
                popularity_rank: 0,
                judge_scores: JudgeScores::default(),
            },
            start_delay,
            activation_counts: ActivationCounts::default(),
            activated_skill_ids: HashSet::new(),
            activated_skills: Vec::new(),
            rng,
            trigger_scheduler,
            golden_skill_indices,
            out_of_gate_skills,
            runtime_skills,
            same_skill_count: 0,
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

    /// Determine gate bracket from gate roll and total gates.
    ///
    /// There are 8 brackets. In races with eight (or fewer) participants, the gates
    /// will correspond one-to-one to a gate bracket (i.e., gate 1 to bracket 1,
    /// gate 2 to bracket 2, etc.).
    ///
    /// In races with more than 8 participants, gates will be added in reverse,
    /// starting from the last bracket. So, in a race with 9 participants, the extra
    /// gate will go to bracket 8 (paticipants in gate numbers 8 and 9 will share
    /// bracket 8); with 10 participants, to brackets 8 and 7 (participants in gates
    /// 7 and 8 will share bracket 7, gates 8 and 9 will share bracket 8), etc. If
    /// there are more than 16 participants, the process starts from bracket 8 again.
    ///
    /// N.B. All inputs and outputs are 1-indexed.
    fn get_gate_bracket(gate_roll: u8, max_gates: u8) -> u8 {
        if max_gates <= 8 {
            return gate_roll;
        }

        // Convert to 0-indexed for easier calculation
        let mut gate_roll = gate_roll - 1;

        let extra = max_gates - 8;
        let base_extra = extra / 8; // Extra gates all brackets get
        let remainder = extra % 8; // Additional gates for last brackets

        // Gates per bracket
        let gates_per_small = 1 + base_extra;
        let gates_per_large = gates_per_small + 1;

        // Number of brackets with fewer gates
        let num_small = 8 - remainder;
        let total_in_small = num_small * gates_per_small;

        gate_roll = if gate_roll < total_in_small {
            // In a "smaller" bracket
            gate_roll / gates_per_small
        } else {
            // In a "larger" bracket (with extra gate)
            num_small + (gate_roll - total_in_small) / gates_per_large
        };

        // Convert back to 1-indexed
        gate_roll + 1
    }

    /// Apply all out-of-gate skills
    pub fn apply_out_of_gate_skills(&mut self, state: &RaceState) {
        let skills = self.out_of_gate_skills.clone();
        for (alt_index, skill) in skills.iter().enumerate() {
            // Update count (same_skill_horse_count)
            self.same_skill_count = state.existing_skills.get(&skill.id).cloned().unwrap_or(0);

            let result = parse_condition(&skill.alternatives.first().unwrap().condition);
            evaluate_condition(&result.unwrap(), &EvaluatorOutOfGate, None, state, self);
            self.apply_out_of_gate_skill_effect(skill, alt_index);
        }
    }

    /// Build the final Runner with all modifications applied
    pub fn build(self, course: &Course) -> Runner {
        // Convert stamina to HP (using FINAL adjusted stats after out-of-gate boosts)
        let max_hp =
            0.8 * hp_style_multiplier(self.adjusted.style) * (self.adjusted.stamina as f64)
                + course.distance as f64;

        // Set starting speed (done in the constructor)
        // Determine rush chance
        let rush_chance = (0.065 / (0.1 * self.base.wiz as f64 + 1.0).log10()).powf(2.0)
            + self.rush_chance_modifier;
        let can_rush = self.rng.borrow_mut().random_bool(rush_chance);
        let rush_state = if can_rush {
            let style = match self.adjusted.style {
                Style::Front | Style::Runaway => self.adjusted.style,
                Style::Pace => Style::Front,
                Style::Late => {
                    // 75% chance to change strategy to Front, 25% to Pace
                    if self.rng.borrow_mut().random_bool(0.75) {
                        Style::Front
                    } else {
                        Style::Pace
                    }
                }
                Style::End => {
                    // 70% chance to change strategy to Front, 20% to Pace, 10% to Late
                    let roll: f64 = self.rng.borrow_mut().random();
                    if roll < 0.7 {
                        Style::Front
                    } else if roll < 0.9 {
                        Style::Pace
                    } else {
                        Style::Late
                    }
                }
            };
            // Uma will rush in a random section between 2 and 9 (1-indexed)
            let section = self.rng.borrow_mut().random_range(1..=8);
            Some(RushState {
                section,
                start_time: 0.0,
                end_time: f64::MAX,
                max_duration: 12.0,
                is_rushed: false,
                style,
            })
        } else {
            None
        };

        // Calculate section modifier for base speed adjustment
        let section_speed_modifier = roll_section_speed_modifier(
            &mut self.rng.borrow_mut(),
            self.adjusted.wiz,
            course.base_speed(),
        );

        // Calculate force-in modifier based on style and randomness
        let force_in_modifier = if matches!(GAME_BEHAVIOR, GameBehavior::Original) {
            0.0
        } else {
            self.rng.borrow_mut().random_range(0.0..0.1)
                + match self.adjusted.style {
                    Style::Runaway | Style::Front => 0.02,
                    Style::Pace | Style::Late => 0.01,
                    Style::End => 0.3,
                }
        };

        // Initialize race state
        Runner {
            gate_roll: self.gate_roll,
            base: self.base,
            adjusted: self.adjusted,
            initial_state: self.initial_state,
            speed: 3.0,
            acceleration: 0.0, // start dash acceleration is applied separately
            min_speed: 0.85 * course.base_speed()
                + (200.0 + self.adjusted.guts as f64).sqrt() * 0.001,
            base_target_speed: 0.85 * course.base_speed(),
            force_in_modifier,
            lane: LaneState::new(course.starting_lane(self.gate_roll)),
            visible_count: 0,
            nearby: NearbyState::default(),
            blocked: BlockedState::default(),
            surrounded: SurroundState::default(),
            start_delay: self.start_delay,
            available_skills: self.runtime_skills,
            activated_skills: self.activated_skills,
            activated_skill_ids: self.activated_skill_ids,
            activated_alternatives: HashSet::new(),
            skill_modifiers: SkillModifiers::default(),
            active_timed_effects: Vec::new(),
            golden_skill_indices: self.golden_skill_indices,
            section: 0,
            phase: Phase::Early,
            section_speed_modifier,
            hp: HpState::new(max_hp),
            position: 0.0,
            rush_chance,
            rush_state,
            is_start_dash: true,
            next_positioning_check: self.start_delay,
            positioning_strategy: PositioningStrategy::Normal,
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
            duel_targets: HashMap::new(),
            duel_count: 0,
            overtake_state: None,
            overtake_target_time: 0.0,
            being_overtake_target_time: 0.0,
            extra_move_lane: None,
            order: OrderState::default(),
            position_diff: PositionDiff::default(),
            rush_counter: RushCounter::default(),
            activation_flags: SkillActivationFlags::default(),
        }
    }
}
