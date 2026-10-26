//! Runners before the race: their stats, skills and out-of-gate skills.
//!
//! An [`UnreadyRunner`] holds everything known about a runner before the race
//! starts. Once every runner of the race is known, out-of-gate skills are
//! applied, and the [`Runner`] is built.
//!
//! ## Skill Setup
//!
//! When the runner is created:
//!
//! 1. **Wit check**: each skill is kept with a probability based on the
//!    runner's wit (skills without a wit check are always kept). Skills that
//!    fail it can't activate, but still count towards the runner's skills.
//! 2. **Scheduler**: the [`TriggerScheduler`] is built for the kept skills (see
//!    [`build_trigger_scheduler`]). Logics refer to skills by their index in
//!    this list. Out-of-gate skills are skipped.
//!
//! ## Out-of-Gate Skills
//!
//! Out-of-gate skills apply before the race starts: they're the skills with any
//! effect that modifies a parameter needed before the race
//! ([`SkillType::is_pre_race_effect`]), i.e. stats, start delay, running style,
//! rush chance, popularity requirements, or "Sympathy" activation.
//!
//! Their conditions only use values known before the race, so they aren't
//! compiled into triggers. Instead, [`UnreadyRunner::apply_out_of_gate_skills`]
//! chooses the first alternative whose precondition (if any) and condition both
//! hold, evaluated with [`EvaluatorOutOfGate`]; if none holds, the skill
//! doesn't activate. Before the race, a precondition is just another
//! requirement (e.g. `distance_type==3 @ distance_type==4` for a skill whose
//! condition is `base_stamina>=1200`). Skills that ignore popularity
//! requirements ([`Skill::ignores_popularity`]) are applied first, so that
//! every other skill sees it. Then:
//!
//! 1. Its pre-race effects are applied before the [`Runner`] is built.
//! 2. A logic with a `RaceStart` trigger is added for it, so the skill
//!    activates on the first frame the runner moves. The logic has no guards
//!    nor precondition, since both were already satisfied. From there, it's
//!    handled like any other skill: its other effects (e.g. acceleration) apply
//!    with their usual duration and cooldown, and the activation is recorded
//!    and counted.
//!
//! Lotteries (only used by out-of-gate skills) are evaluated here too:
//! `random_lot==X` holds if a new draw (from 0 to 100) is below X, every time
//! it's evaluated. `random_lot_shared==X` is the same, but with a single draw
//! per skill, kept for every alternative, so that several alternatives don't
//! improve its chances. In practice, both currently behave the same: skills
//! using `random_lot` have a single alternative, or alternatives that exclude
//! each other.
//!
//! "Sympathy" activated by another runner bypasses its condition: a race start
//! logic is added for it, unless it already activates (see
//! [`UnreadyRunner::activate_sympathy`]).

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use rand::RngExt;

use crate::condition::parser::Condition;
use crate::configuration::GameVersion;
use crate::evaluator::{CompEvaluator, EvaluatorOutOfGate};
use crate::race::all_out_spurt::{self, AllOutSpurt};
use crate::race::charge_up::{self, ChargeUp};
use crate::race::compete_before_spurt::StaminaKeep;
use crate::race::duel::Duel;
use crate::race::hp::{HpState, calculate_max_hp};
use crate::race::modifiers::{
    calculate_adjusted_power, calculate_adjusted_speed, calculate_adjusted_wit,
    course_speed_modifier, roll_section_speed_modifier, stamina_limit_break_base_buff,
};
use crate::race::popularity::{self, JudgeScores, SkillPopularity};
use crate::race::runner::*;
use crate::race::rush::{self, RushState};
use crate::race::spot_struggle::SpotStruggle;
use crate::race::timed_boost::TimedBoost;
use crate::race::transitions::*;
use crate::trigger::builder::*;
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
        wit: calculate_adjusted_wit(uma.wit, uma.style_aptitude),
        style: uma.style,
        distance_aptitude: uma.distance_aptitude,
        track_aptitude: uma.track_aptitude,
        style_aptitude: uma.style_aptitude,
        fan_count: uma.fan_count,
    }
}

/// Apply a stat boost effect to an [`Uma`]'s stats.
///
/// `modifier` is in stat points.
fn apply_stat_boost(uma: &mut Uma, effect_type: &SkillType, modifier: f64) {
    let modifier = modifier as i32;
    match effect_type {
        SkillType::SpeedUp => modify_stat(&mut uma.speed, modifier),
        SkillType::StaminaUp => modify_stat(&mut uma.stamina, modifier),
        SkillType::PowerUp => modify_stat(&mut uma.power, modifier),
        SkillType::GutsUp => modify_stat(&mut uma.guts, modifier),
        SkillType::WitUp => modify_stat(&mut uma.wit, modifier),
        SkillType::AllStatsUp => {
            modify_stat(&mut uma.speed, modifier);
            modify_stat(&mut uma.stamina, modifier);
            modify_stat(&mut uma.power, modifier);
            modify_stat(&mut uma.guts, modifier);
            modify_stat(&mut uma.wit, modifier);
        }
        // Not a stat boost effect.
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
    /// Raw stats before halving and adjustments. Used for popularity and Charge Up.
    pub raw: Uma,
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

    // === Skills ===
    /// Scheduler for skill triggers.
    trigger_scheduler: TriggerScheduler,
    /// Skills that passed the wit check. Out-of-gate skills among them are
    /// evaluated before the race (see [`Self::apply_out_of_gate_skills`]).
    pub skills: Vec<Skill>,
    /// Skills that failed the wit check. They can't activate, but they still
    /// count towards the runner's skills (e.g. for popularity or skill count
    /// scaling).
    pub failed_skills: Vec<Skill>,
    /// Popularity points added to stats by all skills, including those that
    /// failed the wit check.
    pub(crate) popularity_bonus: SkillPopularity,
    /// Indices of golden skills in `skills` for activation by skills that activate
    /// golden skills.
    golden_skill_indices: Vec<usize>,
    /// Temporary: ID of the out-of-gate skill currently being evaluated. Used for
    /// conditions like `same_skill_horse_count` that need to know which skill
    /// they're checking.
    pub evaluated_skill_id: i32,
    /// Temporary: lottery draw of the out-of-gate skill currently being evaluated,
    /// if drawn yet (see [`Self::evaluated_skill_lot`]).
    evaluated_skill_lot_draw: Cell<Option<f64>>,
    /// Whether an out-of-gate skill activated "Sympathy" for every runner (see
    /// [`Self::activate_sympathy`]).
    pub(crate) casts_sympathy: bool,

    // === Random number generator ===
    /// Shared RNG for this runner. It is cloned from the RaceSolver's RNG.
    rng: Rc<RefCell<rand_pcg::Pcg64Mcg>>,
}

impl UnreadyRunner {
    /// Create a new runner with initial values not yet ready for simulation.
    pub(crate) fn new(
        uma: Uma,
        gate_roll: u8,
        mood: Mood,
        state: &RaceState,
        skills: Vec<Skill>,
    ) -> Self {
        let course = &state.course;
        let params = &state.params;
        let rng = Rc::clone(&state.rng);

        // Calculate base stats.
        let gate_block = Self::get_gate_bracket(gate_roll, params.gates);
        let base_uma = Uma::raw_to_base(uma, mood);

        // Calculate skill activation chance and determine whether skills can activate.
        // N.B. As observed, skill activation update happens before out-of-gate skills
        // activate. Thus, out-of-gate skills don't affect skill activation chance.
        let (skills, failed_skills): (Vec<_>, Vec<_>) = skills.into_iter().partition(|skill| {
            state.config.wit_checks.resolve(
                &mut *rng.borrow_mut(),
                skill.activation_chance(base_uma.wit),
            )
        });

        let trigger_scheduler = build_trigger_scheduler(&skills, course, &mut *rng.borrow_mut());

        // Calculate adjusted stats (before out-of-gate skills).
        let adjusted_uma = adjust_stats(&base_uma, course, params);

        // Determine start delay (0.0 to 0.1 seconds).
        let start_delay = rng.borrow_mut().random_range(0.0..=0.1);

        // Build golden skill indices for random activation.
        // N.B. Out-of-gate skills can't be activated this way.
        let golden_skill_indices = skills
            .iter()
            .enumerate()
            .filter(|(_, skill)| {
                (skill.rarity == SkillRarity::Gold || skill.rarity == SkillRarity::Evolution)
                    && !skill.should_activate_out_of_gate()
            })
            .map(|(i, _)| i)
            .collect::<Vec<usize>>();

        let mut runner = UnreadyRunner {
            base: base_uma,
            raw: uma,
            gate_roll,
            adjusted: adjusted_uma,
            initial_state: InitialState {
                mood,
                gate_block,
                popularity_rank: 0,
                judge_scores: JudgeScores::default(),
                fan_count: uma.fan_count.unwrap_or(state.config.default_fan_count),
                ignores_popularity: false,
                // Assigned by the solver when the runner joins the race.
                team: 0,
            },
            start_delay,
            rng,
            trigger_scheduler,
            golden_skill_indices,
            skills,
            failed_skills,
            popularity_bonus: SkillPopularity::default(),
            evaluated_skill_id: 0,
            evaluated_skill_lot_draw: Cell::new(None),
            casts_sympathy: false,
            rush_chance_modifier: 0.0,
            power_skill_bonus: 0,
        };

        // Popularity guards are known before the race, so the skill bonus can be
        // calculated now. The runner is needed to evaluate the guards.
        let all_skills = runner.skills.iter().chain(&runner.failed_skills);
        runner.popularity_bonus = popularity::skill_bonus(all_skills, &runner, state);

        runner
    }

    /// Activate an out-of-gate skill alternative: apply its pre-race effects now,
    /// and activate it on the first frame the runner moves.
    ///
    /// The activation itself (other effects, cooldown, tracking...) is handled like
    /// any other skill's (see [`add_race_start_logic`]).
    fn activate_out_of_gate_skill(&mut self, skill_index: usize, alt_index: usize) {
        let skill = self.skills[skill_index].clone();
        for effect in &skill.alternatives[alt_index].effects {
            let effect_type = &effect.skill_type;

            // Apply stat boosts.
            if effect_type.is_stat_boost() {
                apply_stat_boost(&mut self.adjusted, effect_type, effect.modifier);
                if matches!(effect_type, SkillType::PowerUp | SkillType::AllStatsUp) {
                    self.power_skill_bonus += effect.modifier as i32;
                }
            }
            // Apply start delay modifications.
            else if matches!(effect_type, SkillType::MultiplyStartDelay) {
                self.start_delay *= effect.modifier;
            } else if matches!(effect_type, SkillType::AddStartDelay) {
                self.start_delay += effect.modifier;
            }
            // Apply style changes.
            else if let Some(style) = effect_type.style_change() {
                self.adjusted.style = style;
            }
            // Apply rush chance modifications.
            else if effect_type.is_rush_chance_effect() {
                self.rush_chance_modifier += effect.modifier;
            }
            // "Sympathy" activation affects every runner, so it's left to the solver.
            else if matches!(effect_type, SkillType::ActivateSympathy) {
                self.casts_sympathy = true;
            }
            // Popularity requirements are satisfied for the out-of-gate skills
            // evaluated afterwards and every skill during the race.
            else if matches!(effect_type, SkillType::IgnorePopularity) {
                self.initial_state.ignores_popularity = true;
            }
            // Other effects are applied when the skill activates.
        }

        let is_continuation = skill.alternatives[alt_index].is_continuation();
        add_race_start_logic(
            skill_index,
            alt_index,
            is_continuation,
            &mut self.trigger_scheduler,
        );
    }

    /// Whether the skill at `skill_index` activates on the first frame the runner
    /// moves.
    fn activates_on_race_start(&self, skill_index: usize) -> bool {
        let scheduler = &self.trigger_scheduler;
        scheduler
            .race_start_triggers
            .iter()
            .any(|&logic_idx| scheduler.skill_logics[logic_idx].skill_index == skill_index)
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

    /// Activate every out-of-gate skill whose condition is satisfied (see
    /// `activate_out_of_gate_skill`).
    pub fn apply_out_of_gate_skills(&mut self, state: &RaceState) {
        // Skills that ignore popularity requirements go first, so that every other
        // skill sees it, whatever order the skills were given in.
        let mut order: Vec<usize> = (0..self.skills.len()).collect();
        order.sort_by_key(|&skill_index| !self.skills[skill_index].ignores_popularity());

        for skill_index in order {
            let skill = &self.skills[skill_index];
            if !skill.should_activate_out_of_gate() {
                continue;
            }
            // Set the skill being evaluated (same_skill_horse_count, random_lot).
            self.evaluated_skill_id = skill.id;
            self.evaluated_skill_lot_draw.set(None);

            // Satisfied if all comparisons of any OR branch are true.
            let holds = |condition: &Condition| {
                condition.branches.iter().any(|branch| {
                    branch
                        .iter()
                        .all(|comp| EvaluatorOutOfGate.evaluate(comp, None, state, self))
                })
            };
            // Activate the first alternative whose precondition (if any) and condition
            // are satisfied, if any. Before the race, a precondition is just another
            // requirement.
            let alt_index = skill.alternatives.iter().position(|alternative| {
                alternative.precondition.as_ref().is_none_or(holds) && holds(&alternative.condition)
            });
            if let Some(alt_index) = alt_index {
                self.activate_out_of_gate_skill(skill_index, alt_index);
            }
        }
    }

    /// New lottery draw (from 0 to 100), used for `random_lot` conditions.
    pub(crate) fn roll_lot(&self) -> f64 {
        self.rng.borrow_mut().random_range(0.0..100.0)
    }

    /// Lottery draw (from 0 to 100) of the out-of-gate skill being evaluated, used
    /// for `random_lot_shared` conditions.
    ///
    /// Drawn on first use, then kept for every alternative of the skill, so that
    /// several alternatives don't improve its chances.
    pub(crate) fn evaluated_skill_lot(&self) -> f64 {
        if let Some(draw) = self.evaluated_skill_lot_draw.get() {
            return draw;
        }
        let draw = self.roll_lot();
        self.evaluated_skill_lot_draw.set(Some(draw));
        draw
    }

    /// Activate "Sympathy" if the runner has it, bypassing its condition
    /// (`same_skill_horse_count>=5`).
    ///
    /// This must be called after out-of-gate skills have been applied, so that it
    /// doesn't activate twice when its condition is also satisfied.
    pub fn activate_sympathy(&mut self) {
        let Some(skill_index) = self.skills.iter().position(|skill| skill.id == SYMPATHY_ID) else {
            return;
        };
        if !self.activates_on_race_start(skill_index) {
            self.activate_out_of_gate_skill(skill_index, 0);
        }
    }

    /// Build the final [`Runner`] with all modifications applied.
    pub(crate) fn build(self, state: &RaceState) -> Runner {
        let course = &state.course;
        let game_version = state.config.game_version;

        // Convert stamina to HP using adjusted stats after out-of-gate boosts.
        let max_hp = calculate_max_hp(self.adjusted.style, self.adjusted.stamina, course);

        // Set starting speed (done in the constructor).
        // Determine whether and how the runner will rush.
        let rush_chance = rush::chance(self.base.wit, self.rush_chance_modifier);
        let can_rush = state
            .config
            .rushing
            .resolve(&mut *self.rng.borrow_mut(), rush_chance / 100.0);
        let rush_state =
            can_rush.then(|| RushState::roll(self.adjusted.style, &mut *self.rng.borrow_mut()));

        // Calculate section modifier for base speed adjustment.
        let section_speed_modifier = roll_section_speed_modifier(
            &mut self.rng.borrow_mut(),
            self.adjusted.wit,
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
                    Style::End => 0.03,
                }
        };

        let stamina_keep = StaminaKeep::roll(game_version, &mut *self.rng.borrow_mut());

        // Initialize race state.
        Runner {
            gate_roll: self.gate_roll,
            base: self.base,
            raw: self.raw,
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
            activated_alternatives: vec![0; self.skills.len()],
            available_skills: self.skills,
            failed_skills: self.failed_skills,
            activated_skills: Vec::new(),
            skill_modifiers: SkillModifiers::default(),
            active_timed_effects: Vec::new(),
            pending_effects: Vec::new(),
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
            slope_speed_modifier: None,
            downhill_check_timer: 0.0,
            uphills_climbed: 0,
            last_spurt: None,
            stamina_limit_break_base: stamina_limit_break_base_buff(
                self.adjusted.stamina,
                course.distance,
                game_version,
            ),
            stamina_limit_break: 0.0,
            all_out_spurt_base: all_out_spurt::base_acceleration(
                self.adjusted.speed,
                self.adjusted.power,
                course.distance,
                game_version,
            ),
            all_out_spurt: AllOutSpurt::Pending,
            charge_up: ChargeUp::new(
                charge_up::charge_power(self.base.power, self.raw.power, self.power_skill_bonus),
                game_version,
            ),
            rng: self.rng,
            segments: SegmentState::default(),
            trigger_scheduler: self.trigger_scheduler,
            finish_time: None,
            finish_margin: None,
            spot_struggle: SpotStruggle::Pending,
            duel: Duel::Pending,
            secure_lead: TimedBoost::Idle { next_check: 0.0 },
            compete_before_spurt: TimedBoost::Idle { next_check: 0.0 },
            stamina_keep,
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
