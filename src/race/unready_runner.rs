use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

use rand::RngExt;

use crate::configuration::GameVersion;
use crate::evaluator::{CompEvaluator, EvaluatorOutOfGate};
use crate::parser::parse_condition;
use crate::race::charge_up::{self, ChargeUp};
use crate::race::compete_before_spurt;
use crate::race::full_throttle::{self, FullThrottle};
use crate::race::hp::{HpState, calculate_max_hp};
use crate::race::modifiers::{
    calculate_adjusted_power, calculate_adjusted_speed, calculate_adjusted_wiz,
    course_speed_modifier, roll_section_speed_modifier, stamina_limit_break_base_buff,
};
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
        wiz: calculate_adjusted_wiz(uma.wiz, uma.style_aptitude),
        style: uma.style,
        distance_aptitude: uma.distance_aptitude,
        track_aptitude: uma.track_aptitude,
        style_aptitude: uma.style_aptitude,
    }
}

/// Apply a stat boost effect to an [`Uma`]'s stats.
///
/// `modifier` is in 10000ths of a stat point.
fn apply_stat_boost(uma: &mut Uma, effect_type: &SkillType, modifier: i32) {
    let modifier = modifier / 10000;
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
        // Not a stat boost effect.
        _ => {}
    }
}

/// Apply start delay modification effects.
fn apply_start_delay_modification(start_delay: &mut f64, effect_type: &SkillType, modifier: i32) {
    match effect_type {
        SkillType::MultiplyStartDelay => {
            *start_delay *= modifier as f64 / 10000.0;
        }
        SkillType::SetStartDelay => {
            *start_delay = modifier as f64 / 10000.0;
        }
        // Not a start delay effect.
        _ => {}
    }
}

/// Modify a stat value, ensuring it doesn't go below 1.
fn modify_stat(stat: &mut u16, modifier: i32) {
    *stat = if modifier >= 0 {
        stat.saturating_add(modifier as u16)
    } else {
        stat.saturating_sub((-modifier) as u16).max(1)
    };
}

/// A data structure that holds all information needed to initialize a
/// [`Runner`]. It allows for proper activation of out-of-gate skills that can
/// take into account the runner's base stats, race parameters, and most
/// importantly, data about other runners.
pub struct UnreadyRunner {
    // === Immutable context ===
    /// Base stats before adjustments. Also used for skill conditions in out-of-gate
    /// skills.
    pub base: Uma,
    /// Raw power stat, before halving and motivation. Used for Charge Up.
    raw_power: u16,
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
    /// Power gained from out-of-gate skills. Used for Charge Up.
    power_skill_bonus: i32,

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
    /// Temporary: Number of runners that have the same skill as the one currently
    /// being evaluated. Used for conditions like `same_skill_horse_count` that need
    /// to know which skill they're checking.
    pub same_skill_count: u8,

    // === Random number generator ===
    /// Shared RNG for this runner. It is cloned from the RaceSolver's RNG.
    rng: Rc<RefCell<rand_pcg::Pcg64Mcg>>,
}

impl UnreadyRunner {
    /// Create a new runner with initial values not yet ready for simulation.
    pub(crate) fn new(
        uma: Uma,
        gate_roll: u8,
        motivation: Motivation,
        state: &RaceState,
        skills: Vec<Skill>,
    ) -> Self {
        let course = &state.course;
        let params = &state.params;
        let rng = Rc::clone(&state.rng);

        // Calculate base stats.
        let gate_block = Self::get_gate_bracket(gate_roll, params.gates);
        let raw_power = uma.power;
        let base_uma = Uma::raw_to_base(uma, motivation);

        // Calculate skill activation chance and determine whether skills can activate.
        // N.B. As observed, skill activation update happens before out-of-gate skills
        // activate. Thus, out-of-gate skills don't affect skill activation chance.
        let available_skills = skills
            .into_iter()
            .filter(|skill| {
                rng.borrow_mut()
                    .random_bool(skill.activation_chance(base_uma.wiz))
            })
            .collect::<Vec<Skill>>();

        // Partition skills into out-of-gate and runtime skills.
        let (out_of_gate_skills, runtime_skills): (Vec<_>, Vec<_>) = available_skills
            .into_iter()
            .partition(|skill| skill.should_activate_out_of_gate());

        // Build trigger scheduler from the runtime skills, as its skill indices
        // refer to them.
        let trigger_scheduler =
            build_trigger_scheduler(&runtime_skills, course, &mut *rng.borrow_mut());

        // Calculate adjusted stats (before out-of-gate skills).
        let adjusted_uma = adjust_stats(&base_uma, course, params);

        // Determine start delay (0.0 to 0.1 seconds).
        let start_delay = rng.borrow_mut().random_range(0.0..=0.1);

        // Build golden skill indices for random activation.
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
            raw_power,
            gate_roll,
            adjusted: adjusted_uma,
            initial_state: InitialState {
                motivation,
                gate_block,
                popularity_rank: 0,
                judge_scores: JudgeScores::default(),
                // TODO: Determine fan count from race state or parameters.
                fan_count: 0,
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
            power_skill_bonus: 0,
        }
    }

    /// Apply a single out-of-gate skill's effects.
    fn apply_out_of_gate_skill_effect(&mut self, skill: &Skill, alt_index: usize) {
        let skill_id = skill.id.clone();
        let alternative = &skill.alternatives[alt_index];

        // Apply each effect in the alternative.
        for effect in &alternative.effects {
            let effect_type = &effect.skill_type;

            // Apply stat boosts.
            if effect_type.is_stat_boost() {
                apply_stat_boost(&mut self.adjusted, effect_type, effect.modifier);
                if matches!(effect_type, SkillType::PowerUp | SkillType::AllStatsUp) {
                    self.power_skill_bonus += effect.modifier / 10000;
                }
            }
            // Apply start delay modifications.
            else if effect_type.is_start_delay_modification() {
                apply_start_delay_modification(&mut self.start_delay, effect_type, effect.modifier);
            }
            // Apply style changes.
            else if let Some(style) = effect_type.style_change() {
                self.adjusted.style = style;
            }
            // Apply rush chance modifications.
            else if effect_type.is_rush_chance_effect() {
                self.rush_chance_modifier += effect.modifier as f64 / 10000.0;
            }
            // Timed effects, recovery, and other runtime-only effects are not applicable
            // out-of-gate.
            // These should not appear in out-of-gate skills, so they're gracefully ignored.
        }

        // Track skill activation.
        self.activated_skill_ids.insert(skill_id.clone());
        self.activated_skills.push(SkillActivationRecord {
            skill_id,
            // Out-of-gate activations occur at position 0.
            distance: 0.0,
        });

        // Increment activation counts (all out-of-gate skills count as "start" phase).
        self.activation_counts.increment(Phase::Early, 0.0, 0.0);
    }

    /// Determine gate bracket from gate roll and total gates.
    ///
    /// There are 8 brackets. In races with eight (or fewer) participants, the gates
    /// will correspond one-to-one to a gate bracket (i.e., gate 1 to bracket 1,
    /// gate 2 to bracket 2, etc.).
    ///
    /// In races with more than 8 participants, gates will be added in reverse,
    /// starting from the last bracket. So in a race with 9 participants, the extra
    /// gate goes to bracket 8 (paticipants in gate numbers 8 and 9 share bracket
    /// 8); with 10 participants, to brackets 8 and 7 (participants in gates 7 and 8
    /// will share bracket 7, gates 8 and 9 will share bracket 8), etc. If there are
    /// more than 16 participants, the process starts from bracket 8 again.
    ///
    /// N.B. All inputs and outputs are 1-indexed.
    fn get_gate_bracket(gate_roll: u8, max_gates: u8) -> u8 {
        if max_gates <= 8 {
            return gate_roll;
        }

        // Convert to 0-indexed for easier calculation.
        let mut gate_roll = gate_roll - 1;

        let extra = max_gates - 8;
        // Extra gates all brackets get.
        let base_extra = extra / 8;
        // Additional gates for last brackets.
        let remainder = extra % 8;

        // Gates per bracket.
        let gates_per_small = 1 + base_extra;
        let gates_per_large = gates_per_small + 1;

        // Number of brackets with fewer gates.
        let num_small = 8 - remainder;
        let total_in_small = num_small * gates_per_small;

        gate_roll = if gate_roll < total_in_small {
            // In a "smaller" bracket.
            gate_roll / gates_per_small
        } else {
            // In a "larger" bracket (with extra gate).
            num_small + (gate_roll - total_in_small) / gates_per_large
        };

        // Convert back to 1-indexed.
        gate_roll + 1
    }

    /// Apply all out-of-gate skills.
    pub fn apply_out_of_gate_skills(&mut self, state: &RaceState) {
        let skills = self.out_of_gate_skills.clone();
        for skill in &skills {
            // Update count (same_skill_horse_count).
            self.same_skill_count = state.existing_skills.get(&skill.id).cloned().unwrap_or(0);

            // Apply the first alternative whose condition is satisfied, if any.
            let alt_index = skill.alternatives.iter().position(|alternative| {
                let Ok(condition) = parse_condition(&alternative.condition) else {
                    // Skip unparseable conditions.
                    return false;
                };
                // Satisfied if all comparisons of any OR branch are true.
                condition.comp_tree.or_branches().iter().any(|branch| {
                    branch
                        .iter()
                        .all(|comp| EvaluatorOutOfGate.evaluate(comp, None, state, self))
                })
            });
            if let Some(alt_index) = alt_index {
                self.apply_out_of_gate_skill_effect(skill, alt_index);
            }
        }
    }

    /// Build the final [`Runner`] with all modifications applied.
    pub(crate) fn build(self, state: &RaceState) -> Runner {
        let course = &state.course;
        let game_version = state.config.game_version;

        // Convert stamina to HP using adjusted stats after out-of-gate boosts.
        let max_hp = calculate_max_hp(self.adjusted.style, self.adjusted.stamina, course);

        // Set starting speed (done in the constructor).
        // Determine rush chance.
        let rush_chance = (6.5 / (0.1 * self.base.wiz as f64 + 1.0).log10()).powf(2.0)
            + self.rush_chance_modifier;
        let can_rush = self.rng.borrow_mut().random_bool(rush_chance / 100.0);
        let rush_state = if can_rush {
            let style = match self.adjusted.style {
                // Front-like styles (Front and Runaway) keep their style when rushing.
                Style::Front | Style::Runaway => self.adjusted.style,
                // Pace runner always changes her strategy to Front.
                Style::Pace => Style::Front,
                // 75% chance of changing strategy to Front, 25% to Pace.
                Style::Late => {
                    if self.rng.borrow_mut().random_bool(0.75) {
                        Style::Front
                    } else {
                        Style::Pace
                    }
                }
                // 70% chance of changing strategy to Front, 20% to Pace, 10% to Late.
                Style::End => {
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
            // Uma will rush in a random section between 2 and 9 (1-indexed).
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

        // Calculate section modifier for base speed adjustment.
        let section_speed_modifier = roll_section_speed_modifier(
            &mut self.rng.borrow_mut(),
            self.adjusted.wiz,
            course.base_speed(),
        );

        // Calculate force-in modifier based on style and randomness.
        let force_in_modifier = if game_version < GameVersion::FirstAnniversary {
            0.0
        } else {
            self.rng.borrow_mut().random_range(0.0..0.1)
                + match self.adjusted.style {
                    Style::Runaway | Style::Front => 0.02,
                    Style::Pace | Style::Late => 0.01,
                    Style::End => 0.3,
                }
        };

        // Roll the HP ratio kept by Stamina Keep (2.5 Anniv. onward).
        let stamina_keep_ratio = if game_version < GameVersion::TwoPointFiveAnniversary {
            0.0
        } else {
            self.rng
                .borrow_mut()
                .random_range(compete_before_spurt::STAMINA_KEEP_RATIO)
        };

        // Initialize race state.
        Runner {
            gate_roll: self.gate_roll,
            base: self.base,
            adjusted: self.adjusted,
            initial_state: self.initial_state,
            speed: 3.0,
            // Start dash acceleration is applied separately.
            acceleration: 0.0,
            min_speed: 0.85 * course.base_speed()
                + (200.0 + self.adjusted.guts as f64).sqrt() * 0.001,
            target_speed: 0.85 * course.base_speed(),
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
            downhill_time: 0.0,
            last_spurt: None,
            stamina_limit_break_base: stamina_limit_break_base_buff(
                self.adjusted.stamina,
                course.distance,
                game_version,
            ),
            stamina_limit_break: 0.0,
            full_throttle_base: full_throttle::base_acceleration(
                self.adjusted.speed,
                self.adjusted.power,
                course.distance,
                game_version,
            ),
            full_throttle: FullThrottle::Pending,
            charge_up: ChargeUp::new(
                charge_up::charge_power(self.base.power, self.raw_power, self.power_skill_bonus),
                game_version,
            ),
            rng: self.rng,
            corner_manager: CornerManager::new(),
            slope_manager: SlopeManager::new(),
            straight_manager: StraightManager::new(),
            entered_straight: None,
            trigger_scheduler: self.trigger_scheduler,
            finish_time: None,
            finish_margin: None,
            spot_struggle: None,
            dueling: None,
            secure_lead: None,
            next_secure_lead_check: 0.0,
            compete_before_spurt: None,
            next_compete_check: 0.0,
            stamina_keep_ratio,
            is_stamina_keep: false,
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

#[cfg(test)]
mod tests;
