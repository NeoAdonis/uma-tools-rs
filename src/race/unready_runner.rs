//! Runners before the race: their stats, skills and out-of-gate skills.
//!
//! When runners get ready, each [`Entrant`] gets her gate and mood, and becomes
//! an [`UnreadyRunner`], which holds everything known about her before the race
//! starts. Then out-of-gate skills are applied, and the [`Runner`] is built.
//!
//! [`Entrant`]: crate::Entrant
//!
//! # Skill Setup
//!
//! When the runner is created:
//!
//! 1. **Wit check**: each skill is kept with a probability based on the
//!    runner's wit (skills without a wit check are always kept). Skills that
//!    fail it can't activate, but still count towards the runner's skills.
//! 2. **Scheduler**: the `TriggerScheduler` is built for the kept skills (see
//!    `TriggerScheduler::from_skills`). Logics refer to skills by their
//!    index in this list. Out-of-gate alternatives are skipped.
//!
//! A pinned wit check (see [`SkillPin::WitCheck`]) replaces the rolled one,
//! even for skills without a wit check.
//!
//! # Out-of-Gate Skills
//!
//! Out-of-gate skills apply before the race starts: they're the skills with any
//! alternative that has an effect modifying a parameter needed before the race
//! (`SkillType::is_pre_race_effect`), i.e. stats, start delay, running style,
//! rush chance, popularity requirements, or "Sympathy" activation.
//!
//! The conditions of those alternatives only use values known before the race,
//! so they aren't compiled into triggers. Instead,
//! `UnreadyRunner::apply_out_of_gate_skills` chooses the first of them whose
//! precondition (if any) and condition both hold, evaluated with
//! `EvaluatorOutOfGate`; if none holds, the skill doesn't activate before the
//! race. Before the race, a precondition is just another requirement (e.g.
//! `distance_type==3 @ distance_type==4` for a skill whose condition is
//! `base_stamina>=1200`). Skills that ignore popularity requirements
//! (`Skill::ignores_popularity`) are applied first, so that every other skill
//! sees it. Then:
//!
//! 1. Its pre-race effects are applied before the [`Runner`] is built.
//! 2. A logic with a `RaceStart` trigger is added for it, so the skill
//!    activates on the first frame the runner moves. The logic has no guards
//!    nor precondition, since both were already satisfied. From there, it's
//!    handled like any other skill: its other effects (e.g. acceleration) apply
//!    with their usual duration and cooldown, and the activation is recorded
//!    and counted.
//!
//! The skill's other alternatives, if any, are compiled into triggers like any
//! other skill's. They're usually continuations (e.g. a speed boost late in the
//! race after a stat boost), which can only activate once the out-of-gate
//! alternative did.
//!
//! Lotteries (only used by out-of-gate skills) are evaluated here too:
//! `random_lot_shared==X` holds if the skill's draw (from 0 to 100) is below X.
//! There's a single draw per skill, kept for every alternative, so that several
//! alternatives don't improve its chances. `random_lot` should be drawn anew
//! every time it's evaluated, but it uses the skill's draw too: skills using it
//! have a single alternative, or alternatives that exclude each other, so it's
//! never evaluated twice in practice.
//!
//! "Sympathy" activated by another runner bypasses its condition: a race start
//! logic is added for it, unless it already activates (see
//! `UnreadyRunner::activate_sympathy`).

use std::sync::Arc;

use rand::RngExt;

use crate::condition::evaluator::{CompEvaluator, EvaluatorOutOfGate};
use crate::condition::parser::{Comparison, Condition};
use crate::configuration::{GameVersion, Roll};
use crate::course::{Course, Phase};
use crate::race::all_out_spurt::{self, AllOutSpurt};
use crate::race::charge_up::{self, ChargeUp};
use crate::race::compete_before_spurt::StaminaKeep;
use crate::race::duel::Duel;
use crate::race::hp::{HpState, calculate_max_hp};
use crate::race::modifiers::{
    calculate_adjusted_power, calculate_adjusted_speed, calculate_adjusted_wit,
    roll_section_speed_modifier, stamina_limit_break_base_buff,
};
use crate::race::parameters::{CAREER_STAT_BONUS, RaceParameters};
use crate::race::pins::{Pins, SkillPin};
use crate::race::popularity::{self, JudgeScores, SkillPopularity};
use crate::race::rng::{RngSeed, RunnerRng, Stream};
use crate::race::runner::*;
use crate::race::rush::{self, RushState};
use crate::race::skill_scaling;
use crate::race::solver::{RaceState, RunnerId};
use crate::race::spot_struggle::SpotStruggle;
use crate::race::timed_boost::TimedBoost;
use crate::race::transitions::*;
use crate::skill::{SYMPATHY_ID, Skill, SkillAlternative, SkillType};
use crate::trigger::skill_activation::{QueuedActivation, SkillActivationRecord};
use crate::trigger::system::*;
use crate::uma::{Aptitude, EffectiveStyle, Mood, Stats, Uma};

/// Adjusts base stats according to course, race parameters and style aptitude.
fn adjust_stats(
    base: &Stats,
    style_aptitude: Aptitude,
    course: &Course,
    parameters: &RaceParameters,
) -> Stats {
    let bonus = if parameters.career {
        CAREER_STAT_BONUS
    } else {
        0.0
    };
    // Added before the clamp, so a stat below 1 keeps its modifier deficit.
    let finish = |stat: f64| (stat + bonus).max(1.0);
    Stats {
        speed: finish(calculate_adjusted_speed(
            base,
            parameters.track_condition,
            &course.course_set_status,
        )),
        stamina: finish(base.stamina),
        power: finish(calculate_adjusted_power(
            base.power,
            course.surface,
            parameters.track_condition,
        )),
        guts: finish(base.guts),
        wit: finish(calculate_adjusted_wit(base.wit, style_aptitude)),
    }
}

/// Applies a stat boost effect to `uma`.
///
/// `modifier` is in stat points.
fn apply_stat_boost(uma: &mut Stats, effect_type: &SkillType, modifier: f64) {
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

/// Modifies a stat value, ensuring it doesn't go below 1.
fn modify_stat(stat: &mut f64, modifier: f64) {
    *stat = (*stat + modifier).max(1.0);
}

/// Returns whether `skill` passes its wit check for a runner with `base_wit`.
///
/// [`Configuration::wit_checks`] only forces the checks of skills that have
/// one, while pins force any. Every check is rolled, so that pinning or
/// forcing it doesn't shift the skill's other draws.
///
/// [`Configuration::wit_checks`]: crate::Configuration::wit_checks
fn passes_wit_check(
    skill: &Skill,
    base_wit: f64,
    pins: &Pins,
    state: &RaceState,
    rng_seed: RngSeed,
) -> bool {
    let roll = if skill.wit_check {
        state.config.wit_checks
    } else {
        Roll::Random
    };
    let mut rng = rng_seed.rng(Stream::WitCheck(skill.id));
    let passes = roll.resolve(&mut rng, skill.activation_chance(base_wit));
    match pins.skills.get(&skill.id) {
        Some(&SkillPin::WitCheck(pinned)) => pinned,
        // Pinned activations happen whatever the check says.
        Some(SkillPin::Activations(_)) => true,
        None => passes,
    }
}

/// Returns the activations of `skills` pinned in `pins` during the race, by
/// descending time.
///
/// Out-of-gate ones already happened. Activations at the same time keep their
/// order (by skill, then as pinned), so they're popped in that order.
fn queue_pinned_activations(pins: &Pins, skills: &[Arc<Skill>]) -> Vec<QueuedActivation> {
    let mut queue = Vec::new();
    for (skill_index, skill) in skills.iter().enumerate() {
        let Some(activations) = pins.activations(skill.id) else {
            continue;
        };
        for activation in activations {
            if skill.alternatives[activation.alternative].is_out_of_gate() {
                continue;
            }
            queue.push(QueuedActivation {
                time: activation.time,
                skill_index,
                alternative_index: activation.alternative,
                targets: activation.targets.clone(),
            });
        }
    }
    // Reversed first, so that the stable sort pops ties in order.
    queue.reverse();
    queue.sort_by(|a, b| b.time.total_cmp(&a.time));
    queue
}

/// Everything known about a runner before the race, needed to initialize a
/// [`Runner`].
///
/// It allows for proper activation of out-of-gate skills, which can take into
/// account the runner's base stats, race parameters and, most importantly, data
/// about other runners.
#[derive(Debug)]
pub struct UnreadyRunner {
    // === Immutable context ===
    /// ID of the runner in the race, assigned by the solver when she gets
    /// ready.
    pub(crate) id: RunnerId,
    /// Base stats before adjustments. Also used for skill conditions in out-of-gate
    /// skills.
    pub base: Stats,
    /// Raw stats before halving and adjustments, with the style and aptitudes.
    /// Used for popularity and charge up.
    pub raw: Uma,
    /// Gate number (1-indexed), which determines her starting lane position.
    /// `None` until assigned (see `Self::assign_gate`).
    pub gate: Option<u8>,

    // === Mutable state being constructed ===
    /// Set of static values related to the runner at the start of the race.
    pub(crate) initial_state: InitialState,
    /// Adjusted stats after applying modifiers and out-of-gate skills.
    /// For the most part, these are the stats used for race calculations.
    adjusted: Stats,
    /// Running style, which out-of-gate skills may change from the raw one.
    style: EffectiveStyle,
    /// Start delay (i.e. time taken to leave the gate) in seconds.
    start_delay: f64,
    /// Rush chance modifier from out-of-gate skills.
    rush_chance_modifier: f64,
    /// Power gained from out-of-gate skills. Used for charge up.
    power_skill_bonus: i32,

    // === Skills ===
    /// Scheduler for skill triggers.
    trigger_scheduler: TriggerScheduler,
    /// Skills that passed the wit check (skills without one always do).
    /// Out-of-gate skills among them are evaluated before the race (see
    /// `Self::apply_out_of_gate_skills`).
    ///
    /// A pinned wit check (see [`SkillPin::WitCheck`]) replaces the rolled one,
    /// even for skills without a wit check.
    pub skills: Vec<Arc<Skill>>,
    /// Skills that failed the wit check. They can't activate on their own, but
    /// they still count towards the runner's skills (e.g. for popularity or skill
    /// count scaling), and gold skills among them can be activated by other
    /// skills (see [`SkillType::ActivateRandomGold`]).
    pub failed_skills: Vec<Arc<Skill>>,
    /// Popularity points added to stats by all skills, including those that
    /// failed the wit check.
    pub(crate) popularity_bonus: SkillPopularity,
    /// Temporary: ID of the out-of-gate skill currently being evaluated. Used for
    /// conditions like `same_skill_horse_count` that need to know which skill
    /// they're checking.
    pub evaluated_skill_id: i32,
    /// Temporary: lottery draw (from 0 to 100) of the out-of-gate skill currently
    /// being evaluated, used for `random_lot` and `random_lot_shared` conditions.
    pub(crate) evaluated_skill_lot: f64,
    /// Whether an out-of-gate skill activated "Sympathy" for every runner (see
    /// [`Self::activate_sympathy`]).
    pub(crate) casts_sympathy: bool,
    /// Out-of-gate activations missing from the pins (see
    /// [`Runner::missing_activations`]).
    pub(crate) missing_activations: Vec<SkillActivationRecord>,
    /// IDs of the skills whose conditions didn't allow their pinned
    /// out-of-gate alternative to activate. Passed on to the [`Runner`] (see
    /// [`Runner::unmet_out_of_gate`]).
    pub(crate) unmet_out_of_gate: Vec<i32>,

    // === Random number generation ===
    /// Random outcomes fixed instead of rolled.
    pub(crate) pins: Pins,
    /// Seed from which every random stream of the runner derives (see
    /// [`crate::race::rng`]).
    pub(crate) rng_seed: RngSeed,
}

impl UnreadyRunner {
    /// Creates a runner with initial values not yet ready for simulation.
    ///
    /// Its random streams derive from `rng_seed`. Its gate is assigned later
    /// (see [`Self::assign_gate`]).
    pub(crate) fn new(
        uma: Uma,
        mood: Mood,
        state: &RaceState,
        skills: Vec<Arc<Skill>>,
        pins: Pins,
        rng_seed: RngSeed,
    ) -> Self {
        let course = &state.course;
        let params = &state.params;

        // Calculate base stats.
        let base = uma.to_base(mood);

        // Calculate skill activation chance and determine whether skills can activate.
        // N.B. As observed, skill activation update happens before out-of-gate skills
        // activate. Thus, out-of-gate skills don't affect skill activation chance.
        let (skills, failed_skills): (Vec<_>, Vec<_>) = skills
            .into_iter()
            .partition(|skill| passes_wit_check(skill, base.wit, &pins, state, rng_seed));

        let trigger_scheduler = TriggerScheduler::from_skills(&skills, course, &pins, rng_seed);

        // Calculate adjusted stats (before out-of-gate skills).
        let adjusted = adjust_stats(&base, uma.style_aptitude, course, params);

        // Determine start delay (0.0 to 0.1 seconds).
        let start_delay = rng_seed.rng(Stream::StartDelay).random_range(0.0..=0.1);

        UnreadyRunner {
            // Assigned by the solver.
            id: RunnerId::from_index(0),
            base,
            raw: uma,
            gate: None,
            adjusted,
            style: uma.style.into(),
            initial_state: InitialState {
                mood,
                // Assigned with the gate.
                gate_block: 0,
                popularity_rank: 0,
                judge_scores: JudgeScores::default(),
                fan_count: uma.fan_count.unwrap_or(state.config.default_fan_count),
                ignores_popularity: false,
                // Assigned by the solver when the runner joins the race.
                team: 0,
            },
            start_delay,
            rng_seed,
            pins,
            trigger_scheduler,
            skills,
            failed_skills,
            popularity_bonus: SkillPopularity::default(),
            evaluated_skill_id: 0,
            evaluated_skill_lot: 0.0,
            casts_sympathy: false,
            missing_activations: Vec::new(),
            unmet_out_of_gate: Vec::new(),
            rush_chance_modifier: 0.0,
            power_skill_bonus: 0,
        }
    }

    /// Assigns `gate` (1-indexed) to the runner, along with her gate bracket (post
    /// number).
    ///
    /// The popularity bonus of the runner's skills is also calculated, since it
    /// depends on the runner's gate.
    pub(crate) fn assign_gate(&mut self, gate: u8, state: &RaceState) {
        self.gate = Some(gate);
        self.initial_state.gate_block = Self::gate_bracket(gate, state.params.gates);

        // Popularity guards are known before the race, so the skill bonus can be
        // calculated now. The runner is needed to evaluate the guards.
        let all_skills = self.skills.iter().chain(&self.failed_skills);
        let all_skills = all_skills.map(Arc::as_ref);
        self.popularity_bonus = popularity::skill_bonus(all_skills, self, state);
    }

    /// Activates an out-of-gate skill alternative: applies its pre-race effects
    /// now, and activates it on the first frame the runner moves.
    ///
    /// The activation itself (other effects, cooldown, tracking...) is handled
    /// like any other skill's (see
    /// [`TriggerScheduler::add_race_start_logic`]).
    fn activate_out_of_gate_skill(&mut self, skill_index: usize, alt_index: usize) {
        let skill = self.skills[skill_index].clone();
        for effect in &skill.alternatives[alt_index].effects {
            let effect_type = &effect.skill_type;
            let modifier = skill_scaling::level_scaled_modifier(effect, skill.level);

            // Apply stat boosts.
            if effect_type.is_stat_boost() {
                apply_stat_boost(&mut self.adjusted, effect_type, modifier);
                if matches!(effect_type, SkillType::PowerUp | SkillType::AllStatsUp) {
                    self.power_skill_bonus += modifier as i32;
                }
            }
            // Apply start delay modifications.
            else if matches!(effect_type, SkillType::MultiplyStartDelay) {
                self.start_delay *= modifier;
            } else if matches!(effect_type, SkillType::AddStartDelay) {
                self.start_delay += modifier;
            }
            // Apply style changes.
            else if let Some(style) = effect_type.style_change() {
                self.style = style;
            }
            // Apply rush chance modifications.
            else if effect_type.is_rush_chance_effect() {
                self.rush_chance_modifier += modifier;
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
        self.trigger_scheduler
            .add_race_start_logic(skill_index, alt_index, is_continuation);
    }

    /// Returns whether the skill at `skill_index` activates on the first frame the
    /// runner moves.
    fn activates_on_race_start(&self, skill_index: usize) -> bool {
        let scheduler = &self.trigger_scheduler;
        scheduler
            .race_start_triggers
            .iter()
            .any(|&logic_idx| scheduler.skill_logics[logic_idx].skill_index == skill_index)
    }

    /// Determines the gate bracket from the gate roll and the total gates.
    ///
    /// There are 8 brackets. In races with eight (or fewer) participants, the gates
    /// will correspond one-to-one to a gate bracket (i.e., gate 1 to bracket 1,
    /// gate 2 to bracket 2, etc.).
    ///
    /// In races with more than 8 participants, gates will be added in reverse,
    /// starting from the last bracket. So in a race with 9 participants, the extra
    /// gate goes to bracket 8 (participants in gate numbers 8 and 9 share bracket
    /// 8); with 10 participants, to brackets 8 and 7 (participants in gates 7 and 8
    /// will share bracket 7, gates 8 and 9 will share bracket 8), etc. If there are
    /// more than 16 participants, the process starts from bracket 8 again.
    ///
    /// N.B. All inputs and outputs are 1-indexed.
    fn gate_bracket(gate_roll: u8, max_gates: u8) -> u8 {
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

    /// Activates every out-of-gate skill whose condition is satisfied (see
    /// `activate_out_of_gate_skill`).
    pub(crate) fn apply_out_of_gate_skills(&mut self, state: &RaceState) {
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
            self.evaluated_skill_lot = self
                .rng_seed
                .rng(Stream::Lottery(skill.id))
                .random_range(0.0..100.0);
            // Pinned activations replace the conditions, which are still checked,
            // ignoring the rolls: the lot always hits.
            let pinned = self.pins.activations(skill.id).map(|activations| {
                // Validated to be at most one.
                let alt_index = activations
                    .iter()
                    .map(|activation| activation.alternative)
                    .find(|&alt_index| skill.alternatives[alt_index].is_out_of_gate());
                (alt_index, self.pins.rules_out_failed_wit_check(skill))
            });
            if pinned.is_some() {
                self.evaluated_skill_lot = 0.0;
            }

            // Satisfied if all comparisons of any OR branch are true.
            let holds = |condition: &Condition| {
                condition.branches.iter().any(|branch| {
                    branch
                        .iter()
                        .all(|comp| EvaluatorOutOfGate.evaluate(comp, None, state, self))
                })
            };
            // Activate the first out-of-gate alternative whose precondition (if any)
            // and condition are satisfied, if any. Before the race, a precondition is
            // just another requirement.
            let allows = |alternative: &SkillAlternative| {
                alternative.precondition.as_ref().is_none_or(holds) && holds(&alternative.condition)
            };
            let alt_index = skill
                .alternatives
                .iter()
                .position(|alternative| alternative.is_out_of_gate() && allows(alternative));
            match pinned {
                None => {
                    if let Some(alt_index) = alt_index {
                        self.activate_out_of_gate_skill(skill_index, alt_index);
                    }
                }
                Some((Some(pinned_index), _)) => {
                    if !allows(&skill.alternatives[pinned_index]) {
                        self.unmet_out_of_gate.push(skill.id);
                    }
                    self.activate_out_of_gate_skill(skill_index, pinned_index);
                }
                Some((None, failed_wit_check_ruled_out)) => {
                    // Recorded unless a roll could explain it.
                    let is_random = |alt_index: usize| {
                        let condition = &skill.alternatives[alt_index].condition;
                        condition
                            .branches
                            .iter()
                            .flatten()
                            .any(Comparison::is_random)
                    };
                    if let Some(alt_index) = alt_index
                        && failed_wit_check_ruled_out
                        && !is_random(alt_index)
                    {
                        let alternative = &skill.alternatives[alt_index];
                        self.missing_activations.push(SkillActivationRecord {
                            skill_id: skill.id,
                            alternative: alt_index,
                            time: 0.0,
                            distance: 0.0,
                            // Before the race, only the distance scales it.
                            duration: alternative.base_duration * state.course.distance as f64
                                / 1000.0,
                            affected: Vec::from_iter(
                                alternative.affects_oneself().then_some(self.id),
                            ),
                            conditions_met: true,
                        });
                    }
                }
            }
        }
    }

    /// Activates "Sympathy" if the runner has it, bypassing its condition.
    ///
    /// This must be called after out-of-gate skills have been applied, so that it
    /// doesn't activate twice when its condition is also satisfied.
    ///
    /// If the runner has a pinned activation for it, the pin decides instead.
    pub(crate) fn activate_sympathy(&mut self) {
        let Some(skill_index) = self.skills.iter().position(|skill| skill.id == SYMPATHY_ID) else {
            return;
        };
        if self.pins.activations(SYMPATHY_ID).is_some() {
            return;
        }
        if !self.activates_on_race_start(skill_index) {
            self.activate_out_of_gate_skill(skill_index, 0);
        }
    }

    /// Builds the final [`Runner`] with all modifications applied.
    pub(crate) fn build(self, state: &RaceState) -> Runner {
        let gate_roll = self.gate.expect("gates are assigned before building");
        let course = &state.course;
        let game_version = state.config.game_version;

        // Convert stamina to HP using adjusted stats after out-of-gate boosts.
        let max_hp = calculate_max_hp(self.style, self.adjusted.stamina, course);

        // Skills that failed the wit check go last, so that the indices of those
        // that passed (used by the trigger logics) stay the same.
        let available_skill_count = self.skills.len();
        let mut skills = self.skills;
        skills.extend(self.failed_skills);
        let cooldown_until = skills
            .iter()
            .map(|skill| vec![0.0; skill.alternatives.len()])
            .collect();
        let mut rng = RunnerRng::new(self.rng_seed, skills.iter().map(|skill| skill.id));
        let pinned_activations = queue_pinned_activations(&self.pins, &skills);

        // Set starting speed (done in the constructor).
        // Determine whether and how the runner will rush.
        let rush_chance = rush::chance(self.base.wit, self.rush_chance_modifier);
        let rush_pins = self.pins.rush;
        let can_rush = state
            .config
            .rushing
            .resolve(&mut rng.rush, rush_chance / 100.0);
        // Rolled even if she doesn't rush, so that pinning whether she does
        // doesn't change the stream.
        let mut rush_state = RushState::roll(self.style, &mut rng.rush);
        rush_state.style = rush_pins.style.unwrap_or(rush_state.style);
        rush_state.section = rush_pins.section.unwrap_or(rush_state.section);
        let rush_state = rush_pins.rushes.unwrap_or(can_rush).then_some(rush_state);

        // Calculate section modifier for base speed adjustment.
        let section_speed_modifier = self.pins.section_speed(
            0,
            roll_section_speed_modifier(
                &mut rng.keyed(Stream::SectionSpeed(0)),
                self.adjusted.wit,
                course.base_speed(),
            ),
        );

        // Calculate move lane modifier.
        let move_lane_modifier = if game_version < GameVersion::FirstAnniversary {
            0.0
        } else {
            (0.0002 * self.adjusted.power).sqrt()
        };

        // Calculate force-in modifier based on style and randomness.
        let rolled_force_in = if game_version < GameVersion::FirstAnniversary {
            0.0
        } else {
            rng.keyed(Stream::ForceIn).random_range(0.0..0.1)
                + match self.style {
                    EffectiveStyle::Runaway | EffectiveStyle::Front => 0.02,
                    EffectiveStyle::Pace | EffectiveStyle::Late => 0.01,
                    EffectiveStyle::End => 0.03,
                }
        };
        let force_in_modifier = self.pins.force_in.unwrap_or(rolled_force_in);

        let stamina_keep = StaminaKeep::roll(game_version, &mut rng.stamina_keep);

        // Initialize race state.
        Runner {
            id: self.id,
            gate_roll,
            base: self.base,
            raw: self.raw,
            adjusted: self.adjusted,
            style: self.style,
            initial_state: self.initial_state,
            speed: 3.0,
            // Start dash acceleration is applied separately.
            acceleration: 0.0,
            min_speed: 0.85 * course.base_speed() + (200.0 * self.adjusted.guts).sqrt() * 0.001,
            target_speed: 0.85 * course.base_speed(),
            move_lane_modifier,
            force_in_modifier,
            lane: LaneState::new(course.starting_lane(gate_roll)),
            visible_count: 0,
            nearby: NearbyState::default(),
            blocked: BlockedState::default(),
            surrounded: SurroundState::default(),
            // Pinned after every out-of-gate skill changed it.
            start_delay: self.pins.start_delay.unwrap_or(self.start_delay),
            activated_alternatives: vec![0; skills.len()],
            cooldown_until,
            skills,
            available_skill_count,
            activated_skills: Vec::new(),
            missing_activations: self.missing_activations,
            pinned_this_frame: Vec::new(),
            unmet_out_of_gate: self.unmet_out_of_gate,
            skill_modifiers: SkillModifiers::default(),
            active_timed_effects: Vec::new(),
            pending_effects: Vec::new(),
            pinned_activations,
            section: 0,
            phase: Phase::from_position(0.0, course),
            section_speed_modifier,
            hp: HpState::new(max_hp),
            position: 0.0,
            rush_chance,
            rush_state,
            is_start_dash: true,
            next_positioning_check: 0.0,
            positioning_strategy: PositioningStrategy::Normal,
            positioning_end: f64::INFINITY,
            slope_speed_modifier: None,
            downhill_check_timer: 0.0,
            downhill_checks: 0,
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
            rng,
            pins: self.pins,
            segments: SegmentState::default(),
            last_move_start: None,
            trigger_scheduler: self.trigger_scheduler,
            finish_time: None,
            finish_margin: None,
            events: Vec::new(),
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
