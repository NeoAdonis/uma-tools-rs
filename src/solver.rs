use crate::course::*;
use crate::hp::*;
use crate::race_parameters::*;
use crate::race_transitions::*;
use crate::runner::*;
use crate::skill::*;
use crate::trigger_builder::*;
use crate::trigger_system::*;
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

// fn displayed_time(seconds: f64) -> f64 {
//     seconds * 1.18
// }

fn process_skills(skills: &[Skill], phase: Phase, perspective: Perspective) {
    // for skill in skills {
    // Check if skill can activate in this phase and perspective
    // if skill.perspective != perspective && skill.perspective != Perspective::All {
    //     continue;
    // }

    // // Check each effect of the skill
    // for effect in &skill.effects {
    //     // Check if effect can activate in this phase
    //     if effect.phase != phase && effect.phase != Phase::All {
    //         continue;
    //     }

    //     // Roll for activation chance
    //     let roll: f64 = 0;
    //     if roll < effect.activation_chance {
    //         // Activate the skill effect
    //         activate_skill(state, effect);
    //     }
    // }
    // }
}

pub struct RaceState {
    pub runner: Runner,
    pub course: Course,
    pub params: RaceParameters,
    pub rng: Rc<RefCell<rand_pcg::Pcg64Mcg>>,
    pub trigger_scheduler: TriggerScheduler,
    pub corner_manager: CornerManager,
    pub slope_manager: SlopeManager,
    pub straight_manager: StraightManager,
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

        // Build trigger scheduler from the available skills (after filtering)
        let trigger_scheduler =
            build_trigger_scheduler(&runner.available_skills, &course, &mut *rng.borrow_mut());

        RaceState {
            runner,
            course,
            params,
            rng,
            trigger_scheduler,
            corner_manager: CornerManager::new(),
            slope_manager: SlopeManager::new(),
            straight_manager: StraightManager::new(),
            elapsed_time: 0.0,
        }
    }

    pub fn step(&mut self, delta_time: f64) {
        // Pre-Update
        self.check_corner_end();
        self.check_straight_end();
        self.elapsed_time += delta_time; // Track race time
        // Update
        // TODO: implement "Activate skills"
        // TODO: implement "Recover HP by skills"
        self.update_last_spurt_state();
        self.runner.update_target_speed(self.course.base_speed()); // TODO: review
        self.runner.update_acceleration();
        self.update_phase();
        self.runner
            .update_speed(delta_time, self.course.base_speed());
        self.runner.update_position(delta_time);
        self.update_hp(delta_time);
        self.update_slope();
        // Post-Update
        self.check_corner_start();
        self.check_straight_start();
    }

    fn check_corner_start(&mut self) {
        // Check for corner entry using the corner manager
        if let Some(corner_idx) = self
            .corner_manager
            .check_entry(self.runner.position, &self.course.corners)
        {
            self.runner.current_corner_idx = Some(corner_idx);
            // TODO: Trigger corner entry skills
        }
    }

    fn check_corner_end(&mut self) {
        // Check for corner exit using the corner manager
        if let Some(corner_idx) = self
            .corner_manager
            .check_exit(self.runner.position, &self.course.corners)
        {
            self.runner.current_corner_idx = None;
            // TODO: Trigger corner exit skills
        }
    }

    fn check_straight_start(&mut self) {
        // Check for straight entry using the straight manager
        if let Some(straight_idx) = self
            .straight_manager
            .check_entry(self.runner.position, &self.course.straights)
        {
            self.runner.current_straight_idx = Some(straight_idx);
            // TODO: Trigger straight entry skills
        }
    }

    fn check_straight_end(&mut self) {
        // Check for straight exit using the straight manager
        if let Some(straight_idx) = self
            .straight_manager
            .check_exit(self.runner.position, &self.course.straights)
        {
            self.runner.current_straight_idx = None;
            // TODO: Trigger straight exit skills
        }
    }

    fn update_last_spurt_state(&mut self) {
        if self.runner.phase >= Phase::Mid && self.runner.last_spurt.is_none() {
            self.runner
                .calculate_last_spurt(&self.course, self.params.condition);
        }
    }

    fn update_hp(&mut self, delta_time: f64) {
        self.runner.hp -= calculate_hp_consumption_per_second(
            self.runner.speed,
            self.course.base_speed(),
            self.course.surface,
            self.params.condition,
            self.runner.phase,
            self.runner.adjusted.guts,
            self.runner.is_rushed,
            self.runner.is_pace_down(),
            self.runner.is_downhill_accel(),
        ) * delta_time;
    }

    fn update_slope(&mut self) {
        // Check for slope entry using the slope manager
        if let Some((slope_idx, _slope_type)) = self
            .slope_manager
            .check_entry(self.runner.position, &self.course.slopes)
        {
            // TODO: Trigger slope entry skills
            if self.course.slopes[slope_idx].is_uphill() {
                self.runner
                    .start_uphill_acceleration(self.course.slopes[slope_idx].slope_percent());
            }
            self.runner.current_slope_idx = Some(slope_idx);
        } else if let Some(slope_idx) = self
            .slope_manager
            .check_exit(self.runner.position, &self.course.slopes)
        {
            // TODO: Trigger slope exit skills
            self.runner.stop_slope_acceleration();
            self.runner.current_slope_idx = None;
        } else if let Some(idx) = self.runner.current_slope_idx {
            // On slope - check for downhill acceleration
            if self.course.slopes[idx].is_downhill() {
                let has_second_passed = false; // TODO: track time on slope
                if has_second_passed {
                    if self.runner.is_downhill_accel() {
                        // 20% chance to stop each second
                        let roll: f64 = self.rng.borrow_mut().random();
                        if roll < 0.2 {
                            self.runner.stop_slope_acceleration();
                        }
                    } else {
                        let roll: f64 = self.rng.borrow_mut().random();
                        let chance = self.runner.adjusted.wiz as f64 * 0.0004;
                        if roll < chance {
                            self.runner
                                .start_downhill_acceleration(self.course.slopes[idx].slope_percent());
                        }
                    }
                }
            }
        }
    }

    fn find_next_slope(&self) -> Option<usize> {
        self.course
            .slopes
            .iter()
            .position(|c| c.start > self.runner.position)
    }

    fn find_next_corner(&self) -> Option<usize> {
        self.course
            .corners
            .iter()
            .position(|c| c.start > self.runner.position)
    }

    pub fn run(&mut self) {
        let dt = 1.0 / 15.0; // 15 FPS
        // TODO: replace `for` with `while`
        //while self.runner.position < self.course.distance as f64 {
        for _ in 0..1 {
            self.step(dt);
        }
    }

    pub fn update_phase(&mut self) {
        self.runner.phase = Phase::from_position(self.runner.position, &self.course);
    }
}

pub fn activate_skill(state: &mut RaceState, skill: &SkillEffect) {
    match skill.skill_type {
        SkillType::SpeedUp => {
            modify_stat(&mut state.runner.adjusted.speed, skill.modifier);
        }
        SkillType::StaminaUp => {
            modify_stat(&mut state.runner.adjusted.stamina, skill.modifier);
        }
        SkillType::PowerUp => {
            modify_stat(&mut state.runner.adjusted.power, skill.modifier);
        }
        SkillType::GutsUp => {
            modify_stat(&mut state.runner.adjusted.guts, skill.modifier);
        }
        SkillType::WizUp => {
            modify_stat(&mut state.runner.adjusted.wiz, skill.modifier);
        }
        SkillType::MultiplyStartDelay => {
            //state.runner.start_delay *= skill.modifier;
        }
        SkillType::SetStartDelay => {
            //state.runner.start_delay = skill.modifier;
        }
        SkillType::TargetSpeed => {
            state.runner.base_target_speed += skill.modifier as f64;
            //TODO: this.activeTargetSpeedSkills.push({skillId: s.skillId, perspective: s.perspective, durationTimer: this.getNewTimer(-scaledDuration), modifier: ef.modifier});
        }
        SkillType::Acceleration => {
            state.runner.acceleration += skill.modifier as f64;
            //TODO: this.activeAccelSkills.push({skillId: s.skillId, perspective: s.perspective, durationTimer: this.getNewTimer(-scaledDuration), modifier: ef.modifier});
        }
        SkillType::CurrentSpeed => {
            state.runner.speed += skill.modifier as f64;
            //TODO: this.activeCurrentSpeedSkills.push({skillId: s.skillId, perspective: s.perspective, durationTimer: this.getNewTimer(-scaledDuration), modifier: ef.modifier,naturalDeceleration: ef.type == SkillType.CurrentSpeedWithNaturalDeceleration});
        }
        SkillType::CurrentSpeedWithNaturalDeceleration => {
            state.runner.speed += skill.modifier as f64;
            //TODO: this.activeCurrentSpeedSkills.push({skillId: s.skillId, perspective: s.perspective, durationTimer: this.getNewTimer(-scaledDuration), modifier: ef.modifier,naturalDeceleration: ef.type == SkillType.CurrentSpeedWithNaturalDeceleration});
        }
        SkillType::Recovery => {
            // TODO: implement recovery logic
            //state.recover(skill.modifier);
            if !matches!(GAME_BEHAVIOR, GameBehavior::Original) && state.runner.phase >= Phase::Mid
            {
                //state.runner.last_spurt = calculate_last_spurt(current_hp, remaining_distance, base_speed, max_last_spurt_speed, late_race_base_target_speed, wiz_stat, track, condition, guts_stat, rng_roll);
            }
        }
        SkillType::ActivateRandomGold => {
            // TODO: activate random gold skill
        }
        SkillType::ExtendEvolvedDuration => {
            // TODO: Extend duration of evolved skills
        }
        SkillType::ChangeStyleOonige => todo!(),
        SkillType::Fov => todo!(),
        SkillType::RushTime => todo!(),
        SkillType::LaneMovementSpeed => todo!(),
        SkillType::RushChance => todo!(),
        SkillType::AllStatsUp => todo!(),
        SkillType::LaneChange => todo!(),
        SkillType::Unknown41 => todo!(),
    }
    //state.runner.activated_skills[state.runner.phase as usize] += 1;
}
