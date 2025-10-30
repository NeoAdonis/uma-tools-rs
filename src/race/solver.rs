use crate::course::{Course, Phase};
use crate::race::parameters::RaceParameters;
use crate::race::runner::{
    ActiveTimedEffect, GAME_BEHAVIOR, GameBehavior, Runner, SkillActivationRecord,
};
use crate::trigger::builder::*;
use crate::trigger::system::*;
use crate::uma::skill::*;
use crate::uma::*;
use rand::prelude::*;
use std::cell::RefCell;
use std::rc::Rc;

pub fn modify_stat(stat: &mut u16, modifier: i32) {
    // Make sure stat doesn't go below 1
    *stat = if modifier >= 0 {
        stat.saturating_add(modifier as u16)
    } else {
        stat.saturating_sub((-modifier) as u16).max(1)
    };
}

fn section_coefficient_range(wiz_stat: u16) -> (f64, f64) {
    // Max = WizStat / 5500 * log10(WizStat * 0.1) [%]
    // Min = Max - 0.65 [%]
    let max = (wiz_stat as f64 / 5500.0) * (wiz_stat as f64 * 0.1).log10();
    let min = max - 0.65;
    (min / 100.0, max / 100.0)
}

// TODO: implement blocking logic
// When blocked, a uma’s speed is capped between 0.988x(at 0m) to 1.0x(at 2m) of the speed of the uma in front, scaled linearly to the distance in between.
// ref. https://docs.google.com/document/d/15VzW9W2tXBBTibBRbZ8IVpW6HaMX8H0RP03kq6Az7Xg/edit?tab=t.0#heading=h.eyiunid0e18k

// TODO: implement lane change logic
// ref. https://docs.google.com/document/d/15VzW9W2tXBBTibBRbZ8IVpW6HaMX8H0RP03kq6Az7Xg/edit?tab=t.0#heading=h.ohdxzwro17n5

// TODO: implement start delay logic
// ref. https://docs.google.com/document/d/15VzW9W2tXBBTibBRbZ8IVpW6HaMX8H0RP03kq6Az7Xg/edit?tab=t.0#heading=h.iq42ksqbg2p

// TODO: implement position keep logic
// ref. https://docs.google.com/document/d/15VzW9W2tXBBTibBRbZ8IVpW6HaMX8H0RP03kq6Az7Xg/edit?tab=t.0#heading=h.tulele26d1pm

// TODO: implement rush logic
// ref. https://docs.google.com/document/d/15VzW9W2tXBBTibBRbZ8IVpW6HaMX8H0RP03kq6Az7Xg/edit?tab=t.0#heading=h.awbjufvej6ud

pub struct RaceState {
    pub runners: Vec<Runner>,
    pub course: Course,
    pub params: RaceParameters,
    pub rng: Rc<RefCell<rand_pcg::Pcg64Mcg>>,
    pub trigger_scheduler: TriggerScheduler,
    pub elapsed_time: f64, // Race time in seconds (shared by all umas)
}

impl RaceState {
    pub fn new(
        uma: Uma,
        motivation: Motivation,
        course: Course,
        params: RaceParameters,
        skills: Vec<Skill>,
    ) -> Self {
        let seed = params.rng_seed;
        let rng = Rc::new(RefCell::new(rand_pcg::Pcg64Mcg::seed_from_u64(seed)));

        // Create runner first (it filters available skills based on wiz)
        let runner = Runner::new(uma, motivation, &course, &params, skills, Rc::clone(&rng));

        let runners = vec![runner]; // Single runner for now

        // Build trigger scheduler from the available skills (after filtering)
        // ! TODO: support multiple runners
        let trigger_scheduler = build_trigger_scheduler(
            &runners[0].available_skills,
            &course,
            &mut *rng.borrow_mut(),
        );

        RaceState {
            runners,
            course,
            params,
            rng,
            trigger_scheduler,
            elapsed_time: 0.0,
        }
    }

    // TODO: complete implementation and report
    #[allow(dead_code)]
    fn displayed_time(&self) -> f64 {
        self.elapsed_time * 1.18
    }

    pub fn step(&mut self, delta_time: f64) {
        // Pre-Update
        for runner in &mut self.runners {
            runner.check_corner_end(&self.course.corners);
            runner.check_straight_end(&self.course.straights);
        }

        self.elapsed_time += delta_time; // Track race time

        // Update
        self.check_skill_activations();
        // TODO: implement "Recover HP by skills"

        for runner in &mut self.runners {
            runner.update_last_spurt_state(&self.course, self.params.condition);
            runner.update_target_speed(self.course.base_speed()); // TODO: review
            runner.update_acceleration();
            runner.update_phase(&self.course);
            runner.update_speed(delta_time, self.course.base_speed());
            runner.update_position(delta_time);
            runner.update_hp(delta_time, &self.course, self.params.condition);
            runner.update_slope(&self.course.slopes, &mut *self.rng.borrow_mut());
            // Update skill durations and remove expired effects
            runner.update_skill_durations(self.elapsed_time);
            // Post-Update
            runner.check_corner_start(&self.course.corners);
            runner.check_straight_start(&self.course.straights);
        }
    }

    /// Check and activate skills based on triggers
    fn check_skill_activations(&mut self) {
        // TODO: Check monotonic triggers (time, distance, HP)
        // TODO: Check random activation triggers
        // TODO: Check continuous triggers
    }

    pub fn run(&mut self) {
        let dt = 1.0 / 15.0; // 15 FPS
        // TODO: replace `for` with `while`
        //while self.runner.position < self.course.distance as f64 {
        for _ in 0..1 {
            self.step(dt);
        }
    }
}

/// Activate a skill by its index and alternative index
pub fn activate_skill(
    runner: &mut Runner,
    skill_index: usize,
    alternative_index: usize,
    elapsed_time: f64,
    course: &Course,
    trigger_scheduler: &mut TriggerScheduler,
) {
    // Clone the skill data we need to avoid borrow checker issues
    let skill_id = runner.available_skills[skill_index].id.clone();
    let alternative = runner.available_skills[skill_index].alternatives[alternative_index].clone();

    // Calculate scaled duration
    let base_duration = alternative.base_duration as f64 / 10000.0; // Convert from 10000ths of a second
    let scaled_duration = base_duration * runner.skill_modifiers.duration_scaling;

    // Apply each effect in the alternative
    for effect in &alternative.effects {
        match effect.skill_type {
            // Instant stat modifications (permanent)
            SkillType::SpeedUp => {
                modify_stat(&mut runner.adjusted.speed, effect.modifier);
            }
            SkillType::StaminaUp => {
                modify_stat(&mut runner.adjusted.stamina, effect.modifier);
            }
            SkillType::PowerUp => {
                modify_stat(&mut runner.adjusted.power, effect.modifier);
            }
            SkillType::GutsUp => {
                modify_stat(&mut runner.adjusted.guts, effect.modifier);
            }
            SkillType::WizUp => {
                modify_stat(&mut runner.adjusted.wiz, effect.modifier);
            }
            SkillType::AllStatsUp => {
                modify_stat(&mut runner.adjusted.speed, effect.modifier);
                modify_stat(&mut runner.adjusted.stamina, effect.modifier);
                modify_stat(&mut runner.adjusted.power, effect.modifier);
                modify_stat(&mut runner.adjusted.guts, effect.modifier);
                modify_stat(&mut runner.adjusted.wiz, effect.modifier);
            }

            // Timed effects (tracked in active_timed_effects)
            SkillType::Acceleration
            | SkillType::TargetSpeed
            | SkillType::CurrentSpeed
            | SkillType::CurrentSpeedWithNaturalDeceleration
            | SkillType::ExtendEvolvedDuration => {
                let expires_at = elapsed_time + scaled_duration;
                runner.active_timed_effects.push(ActiveTimedEffect {
                    skill_id: skill_id.clone(),
                    skill_index,
                    alternative_index,
                    effect: effect.clone(),
                    expires_at,
                });
            }

            // Recovery (HP restoration)
            SkillType::Recovery => {
                runner.hp = (runner.hp + effect.modifier as f64).min(runner.max_hp);

                // Increment heal skill count
                trigger_scheduler.activation_counts.increment_heal();

                // Recalculate last spurt if in Mid phase or later (FirstAnniversary behavior)
                if !matches!(GAME_BEHAVIOR, GameBehavior::Original) && runner.phase >= Phase::Mid {
                    // TODO: implement last_spurt recalculation
                }
            }

            // Activate random gold skill
            SkillType::ActivateRandomGold => {
                if !runner.golden_skill_indices.is_empty() {
                    let idx = runner
                        .rng
                        .borrow_mut()
                        .random_range(0..runner.golden_skill_indices.len());
                    let golden_skill_idx = runner.golden_skill_indices[idx];
                    // Recursively activate the golden skill (use first alternative)
                    activate_skill(
                        runner,
                        golden_skill_idx,
                        0,
                        elapsed_time,
                        course,
                        trigger_scheduler,
                    );
                }
            }

            // Start delay modifications
            SkillType::MultiplyStartDelay => {
                runner.start_delay *= effect.modifier as f64 / 10000.0;
            }
            SkillType::SetStartDelay => {
                runner.start_delay = effect.modifier as f64 / 10000.0;
            }

            // Style change
            SkillType::ChangeStyleOonige => {
                runner.adjusted.style = Style::Oonige;
            }

            // Unimplemented skill types
            SkillType::Fov => todo!(),
            SkillType::RushTime => todo!(),
            SkillType::LaneMovementSpeed => todo!(),
            SkillType::RushChance => todo!(),
            SkillType::LaneChange => todo!(),
            SkillType::Unknown41 => todo!(),
        }
    }

    // Recalculate skill modifiers from active effects
    runner.recalculate_skill_modifiers();

    // Track skill activation
    runner.activated_skill_ids.insert(skill_id.clone());
    runner.activated_skills.push(SkillActivationRecord {
        skill_id,
        skill_index,
        alternative_index,
        distance: runner.position,
    });

    // Increment activation counts for condition evaluation
    let section_length = course.distance as f64 / 24.0;
    trigger_scheduler
        .activation_counts
        .increment(runner.phase, runner.position, section_length);
}
