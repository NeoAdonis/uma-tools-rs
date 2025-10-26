use crate::course::*;
use crate::race_parameters::*;
use crate::runner::*;
use crate::skill::*;
use crate::uma::*;

pub fn modify_stat(stat: &mut u16, modifier: i32) {
    // Make sure stat doesn't go below 1
    *stat = if modifier >= 0 {
        stat.saturating_add(modifier as u16)
    } else {
        stat.saturating_sub((-modifier) as u16).max(1)
    };
}

fn strategy_phase_coefficient(style: Style, phase: Phase) -> f64 {
    match (style, phase) {
        // Early-race
        (Style::Front, Phase::Early) => 1.0,
        (Style::Pace, Phase::Early) => 0.978,
        (Style::Late, Phase::Early) => 0.938,
        (Style::End, Phase::Early) => 0.931,
        (Style::Oonige, Phase::Early) => 1.063,
        // Mid-race
        (Style::Front, Phase::Mid) => 0.98,
        (Style::Pace, Phase::Mid) => 0.991,
        (Style::Late, Phase::Mid) => 0.998,
        (Style::End, Phase::Mid) => 1.0,
        (Style::Oonige, Phase::Mid) => 0.962,
        // Late-race and Last spurt
        (Style::Front, Phase::Late | Phase::Last) => 0.962,
        (Style::Pace, Phase::Late | Phase::Last) => 0.975,
        (Style::Late, Phase::Late | Phase::Last) => 0.994,
        (Style::End, Phase::Late | Phase::Last) => 1.0,
        (Style::Oonige, Phase::Late | Phase::Last) => 0.95,
    }
}

fn distance_proficiency_modifier(aptitude: Aptitude) -> f64 {
    match aptitude {
        Aptitude::S => 1.05,
        Aptitude::A => 1.0,
        Aptitude::B => 0.9,
        Aptitude::C => 0.8,
        Aptitude::D => 0.6,
        Aptitude::E => 0.4,
        Aptitude::F => 0.2,
        Aptitude::G => 0.1,
    }
}

fn base_target_speed(
    base_speed: f64,
    style: Style,
    phase: Phase,
    speed_stat: u16,
    distance_aptitude: Aptitude,
) -> f64 {
    let base = base_speed * strategy_phase_coefficient(style, phase);
    // Speed stat only affects target speed during late-race and last spurt
    match phase {
        Phase::Early | Phase::Mid => base,
        Phase::Late | Phase::Last => {
            let speed_modifier = (500.0 * speed_stat as f64).sqrt()
                * distance_proficiency_modifier(distance_aptitude)
                * 0.002;
            base + speed_modifier
        }
    }
}

fn section_coefficient_range(wiz_stat: u16) -> (f64, f64) {
    // Max = WizStat / 5500 * log10(WizStat * 0.1) [%]
    // Min = Max - 0.65 [%]
    let max = (wiz_stat as f64 / 5500.0) * (wiz_stat as f64 * 0.1).log10();
    let min = max - 0.65;
    (min / 100.0, max / 100.0)
}

fn last_spurt_speed_max(
    base_speed: f64,
    style: Style,
    speed_stat: u16,
    guts_stat: u16,
    distance_aptitude: Aptitude,
) -> f64 {
    let base_target_late = base_speed * strategy_phase_coefficient(style, Phase::Late);
    let base_component = (base_target_late + 0.01 * base_speed) * 1.05;
    let speed_component = (500.0 * speed_stat as f64).sqrt()
        * distance_proficiency_modifier(distance_aptitude)
        * 0.002;
    let guts_component = match GAME_BEHAVIOR {
        GameBehavior::Original => 0.0,
        GameBehavior::FirstAnniversary => (450.0 * guts_stat as f64).powf(0.597) * 0.0001,
    };
    base_component + speed_component + guts_component
}

fn hp_ground_modifier(track: Track, condition: TrackCondition) -> f64 {
    match (track, condition) {
        (Track::Turf, TrackCondition::Firm) => 1.0,
        (Track::Turf, TrackCondition::Good) => 1.0,
        (Track::Turf, TrackCondition::Soft) => 1.02,
        (Track::Turf, TrackCondition::Heavy) => 1.02,
        (Track::Dirt, TrackCondition::Firm) => 1.0,
        (Track::Dirt, TrackCondition::Good) => 1.0,
        (Track::Dirt, TrackCondition::Soft) => 1.01,
        (Track::Dirt, TrackCondition::Heavy) => 1.02,
    }
}

fn hp_guts_modifier(guts_stat: u16) -> f64 {
    1.0 + (200.0 / (600.0 * guts_stat as f64).sqrt())
}

fn calculate_hp_consumption_per_second(
    current_speed: f64,
    base_speed: f64,
    track: Track,
    condition: TrackCondition,
    phase: Phase,
    guts_stat: u16,
    is_rushed: bool,
    is_pace_down: bool,
    is_downhill_accel: bool,
) -> f64 {
    let speed_diff = current_speed - base_speed + 12.0;
    let base_consumption = 20.0 * speed_diff.powi(2) / 144.0;

    let mut status_modifier = 1.0;
    if is_rushed {
        status_modifier *= 1.6;
    }
    if is_pace_down {
        status_modifier *= 0.6;
    }
    if is_downhill_accel {
        status_modifier *= 0.4;
    }

    let ground_modifier = hp_ground_modifier(track, condition);

    let consumption = base_consumption * status_modifier * ground_modifier;

    // In late-race and last spurt, HP consumption is multiplied by guts modifier
    match phase {
        Phase::Late | Phase::Last => consumption * hp_guts_modifier(guts_stat),
        _ => consumption,
    }
}

struct LastSpurtCandidate {
    speed: f64,
    distance: f64,
    time: f64,
}

fn calculate_last_spurt(
    runner: &Runner,
    remaining_distance: f64,
    base_speed: f64,
    track: Track,
    condition: TrackCondition,
    rng_roll: f64, // Random value between 0.0 and 1.0 for wisdom check
) -> LastSpurt {
    let max_last_spurt_speed = last_spurt_speed_max(
        base_speed,
        runner.adjusted.style,
        runner.adjusted.speed,
        runner.adjusted.guts,
        runner.adjusted.distance_aptitude,
    );
    let late_race_base_target_speed = base_target_speed(
        base_speed,
        runner.adjusted.style,
        Phase::Late,
        runner.adjusted.speed,
        runner.adjusted.distance_aptitude,
    );

    // Estimate stamina usage up to 60m before goal line
    let calculation_distance = remaining_distance - 60.0;

    // If HP is sufficient for max speed, use it immediately
    let max_hp_required = estimate_hp_consumption(
        max_last_spurt_speed,
        calculation_distance,
        base_speed,
        track,
        condition,
        runner.adjusted.guts,
    );

    if runner.hp >= max_hp_required {
        return LastSpurt {
            max_speed: max_last_spurt_speed,
            target_distance: remaining_distance,
        };
    }

    // Build list of candidates by lowering speed by 0.1 m/s each step
    let mut candidates = Vec::new();
    let mut test_speed = max_last_spurt_speed;

    while test_speed >= late_race_base_target_speed {
        // Calculate distance where this speed would be used
        // This is where HP runs out or reaches the end
        let hp_required = estimate_hp_consumption(
            test_speed,
            calculation_distance,
            base_speed,
            track,
            condition,
            runner.adjusted.guts,
        );

        // Find distance we can actually run at this speed with available HP
        let usable_distance = if hp_required <= runner.hp {
            remaining_distance
        } else {
            // Scale down distance based on HP available
            let hp_ratio = runner.hp / hp_required;
            (calculation_distance * hp_ratio) + 60.0
        };

        // Calculate time to finish (ignoring acceleration)
        // Time for last spurt portion + time for remaining at late-race base speed
        let last_spurt_distance = usable_distance;
        let remaining_at_base = remaining_distance - last_spurt_distance;

        let time = if remaining_at_base > 0.0 {
            last_spurt_distance / test_speed + remaining_at_base / late_race_base_target_speed
        } else {
            remaining_distance / test_speed
        };

        candidates.push(LastSpurtCandidate {
            speed: test_speed,
            distance: usable_distance,
            time,
        });

        test_speed -= 0.1;
    }

    let acceptance_chance = (15.0 + 0.05 * runner.adjusted.wiz as f64) / 100.0;

    // Try each candidate from best to worst
    candidates.sort_by(|a, b| a.time.partial_cmp(&b.time).unwrap());
    let mut cumulative_probability = 0.0;
    for candidate in &candidates {
        cumulative_probability += acceptance_chance;
        if rng_roll < cumulative_probability {
            return LastSpurt {
                max_speed: candidate.speed,
                target_distance: candidate.distance,
            };
        }
    }

    // If all checks fail, use the slowest (last) candidate
    if let Some(last) = candidates.last() {
        LastSpurt {
            max_speed: last.speed,
            target_distance: last.distance,
        }
    } else {
        // Fallback: use late-race base speed for full distance
        // TODO: ensure this case is unreachable
        LastSpurt {
            max_speed: late_race_base_target_speed,
            target_distance: remaining_distance,
        }
    }
}

fn estimate_hp_consumption(
    speed: f64,
    distance: f64,
    base_speed: f64,
    track: Track,
    condition: TrackCondition,
    guts_stat: u16,
) -> f64 {
    // Estimate HP consumption for running at constant speed over distance
    // Using the HP consumption formula with Phase::Last

    // Time = distance / speed
    let time = distance / speed;

    // Calculate HP consumption per second at this speed
    // Assuming no special status modifiers (not rushed, not pace-down, not downhill)
    let hp_per_second = calculate_hp_consumption_per_second(
        speed,
        base_speed,
        track,
        condition,
        Phase::Last, // Last spurt phase for guts modifier
        guts_stat,
        false, // is_rushed
        false, // is_pace_down
        false, // is_downhill_accel
    );

    hp_per_second * time
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
}

impl RaceState {
    pub fn new(
        uma: Uma,
        motivation: Motivation,
        course: Course,
        params: RaceParameters,
        skills: Vec<Skill>,
    ) -> Self {
        let runner = Runner::new(uma, motivation, &course, &params, skills);
        RaceState {
            runner,
            course,
            params,
        }
    }
    pub fn step(&mut self, delta_time: f64) {
        // Pre-Update
        //  Check end of corner
        // TODO: implement corner effects
        if let Some(idx) = self.runner.current_corner_idx {
            if self.runner.position
                >= self.course.corners[idx].start + self.course.corners[idx].length
            {
                //self.on_corner_exit(idx); // and straight entry
                self.runner.current_corner_idx = None;
            }
        }
        // Update
        //  Activate skills
        // TODO: implement
        //  Recover HP by skills
        // TODO: implement
        //  Update last spurt state
        // TODO: review
        if (self.runner.position >= self.course.distance as f64 / 2.0)
            && (self.runner.last_spurt.is_none())
        {
            self.runner.last_spurt = Some(calculate_last_spurt(
                &self.runner,
                self.course.distance as f64 - self.runner.position,
                self.course.base_speed(),
                self.course.surface,
                self.params.condition,
                0.5, // TODO: random roll
            ));
        }
        //  Update target speed
        // TODO: review
        self.runner.target_speed = if let Some(last_spurt) = &self.runner.last_spurt {
            last_spurt.max_speed
        } else {
            base_target_speed(
                self.course.base_speed(),
                self.runner.adjusted.style,
                self.runner.phase,
                self.runner.adjusted.speed,
                self.runner.adjusted.distance_aptitude,
            )
        };
        //  Calculate acceleration/deceleration
        if self.runner.speed < self.runner.target_speed {
            // TODO: calculate parameters
            self.runner.accelerate(false, false, 0.0);
        } else {
            self.runner.decelerate();
        }
        //  Update phase:
        //  - Early: section < 4
        //  - Mid: section >= 4 && section < 16
        //  - Late: section >= 16 && section < 20
        //  - Last spurt: section >= 20
        self.runner.phase = if self.runner.position < self.course.section_length() * 4.0 {
            Phase::Early
        } else if self.runner.position < self.course.section_length() * 16.0 {
            Phase::Mid
        } else if self.runner.position < self.course.section_length() * 20.0 {
            Phase::Late
        } else {
            Phase::Last
        };
        //  Calculate distance and position
        // TODO: review
        self.runner.speed += self.runner.acceleration * delta_time;
        if self.runner.speed < self.runner.min_speed {
            self.runner.speed = self.runner.min_speed;
        }
        if self.runner.speed > self.runner.target_speed {
            self.runner.speed = self.runner.target_speed;
        }
        self.runner.position += self.runner.speed * delta_time;
        self.runner.hp -= calculate_hp_consumption_per_second(
            self.runner.speed,
            self.course.base_speed(),
            self.course.surface,
            self.params.condition,
            self.runner.phase,
            self.runner.adjusted.guts,
            self.runner.is_rushed,
            false, // TODO: is_pace_down
            false, // TODO: is_downhill_accel
        ) * delta_time;
        //  Check for slope
        // TODO: implement slope effects
        if self.runner.current_slope_idx.is_none() {
            if let Some(next_idx) = self.find_next_slope() {
                if self.runner.position >= self.course.slopes[next_idx].start {
                    //self.on_slope_enter(next_idx);
                    self.runner.current_slope_idx = Some(next_idx);
                }
            }
        } else {
            let idx = self.runner.current_slope_idx.unwrap();
            if self.runner.position
                >= self.course.slopes[idx].start + self.course.slopes[idx].length
            {
                //self.on_slope_exit(idx);
                self.runner.current_slope_idx = None;
            }
        }
        // Post-Update
        // Events
        //  Check start of corner
        // TODO: implement corner effects
        if self.runner.current_corner_idx.is_none() {
            if let Some(next_idx) = self.find_next_corner() {
                if self.runner.position >= self.course.corners[next_idx].start {
                    //self.on_corner_enter(next_idx); // and straight exit
                    self.runner.current_corner_idx = Some(next_idx);
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
            state.runner.target_speed += skill.modifier as f64;
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
