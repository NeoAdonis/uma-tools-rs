use std::clone;

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
    behavior: GameBehavior,
) -> f64 {
    let base_target_late = base_speed * strategy_phase_coefficient(style, Phase::Late);
    let base_component = (base_target_late + 0.01 * base_speed) * 1.05;
    let speed_component = (500.0 * speed_stat as f64).sqrt()
        * distance_proficiency_modifier(distance_aptitude)
        * 0.002;
    let guts_component = match behavior {
        GameBehavior::Original => 0.0,
        GameBehavior::FirstAnniversary => (450.0 * guts_stat as f64).powf(0.597) * 0.0001,
    };
    base_component + speed_component + guts_component
}

fn acceleration_strategy_phase_coefficient(style: Style, phase: Phase) -> f64 {
    match (style, phase) {
        // Early-race
        (Style::Front, Phase::Early) => 1.0,
        (Style::Pace, Phase::Early) => 0.985,
        (Style::Late, Phase::Early) => 0.975,
        (Style::End, Phase::Early) => 0.945,
        (Style::Oonige, Phase::Early) => 1.17,
        // Mid-race
        (Style::Front | Style::Pace | Style::Late | Style::End, Phase::Mid) => 1.0,
        (Style::Oonige, Phase::Mid) => 0.94,
        // Late-race and Last spurt
        (Style::Front, Phase::Late | Phase::Last) => 0.996,
        (Style::Pace, Phase::Late | Phase::Last) => 0.996,
        (Style::Late, Phase::Late | Phase::Last) => 1.0,
        (Style::End, Phase::Late | Phase::Last) => 0.997,
        (Style::Oonige, Phase::Late | Phase::Last) => 0.956,
    }
}

fn ground_type_proficiency_modifier(aptitude: Aptitude) -> f64 {
    match aptitude {
        Aptitude::S => 1.05,
        Aptitude::A => 1.0,
        Aptitude::B => 0.9,
        Aptitude::C => 0.8,
        Aptitude::D => 0.7,
        Aptitude::E => 0.5,
        Aptitude::F => 0.3,
        Aptitude::G => 0.1,
    }
}

fn acceleration_distance_proficiency_modifier(aptitude: Aptitude) -> f64 {
    match aptitude {
        Aptitude::S | Aptitude::A | Aptitude::B | Aptitude::C | Aptitude::D => 1.0,
        Aptitude::E => 0.6,
        Aptitude::F => 0.5,
        Aptitude::G => 0.4,
    }
}

fn calculate_acceleration(
    power_stat: u16,
    style: Style,
    phase: Phase,
    track_aptitude: Aptitude,
    distance_aptitude: Aptitude,
    is_uphill: bool,
    is_start_dash: bool,
    skill_modifier: f64,
) -> f64 {
    // Accel = BaseAccel * sqrt(500.0 * PowerStat) * StrategyPhaseCoefficient
    //         * GroundTypeProficiencyModifier * DistanceProficiencyModifier
    //         + SkillModifier + StartDashModifier

    // Base acceleration: 0.0006 normally, 0.0004 when uphill
    let base_accel = if is_uphill { 0.0004 } else { 0.0006 };

    let power_component = (500.0 * power_stat as f64).sqrt();
    let strategy_coef = acceleration_strategy_phase_coefficient(style, phase);
    let ground_modifier = ground_type_proficiency_modifier(track_aptitude);
    let distance_modifier = acceleration_distance_proficiency_modifier(distance_aptitude);

    let base_acceleration =
        base_accel * power_component * strategy_coef * ground_modifier * distance_modifier;

    // Start dash adds 24.0 m/s^2 additional acceleration
    let start_dash_modifier = if is_start_dash { 24.0 } else { 0.0 };

    base_acceleration + skill_modifier + start_dash_modifier
}

fn calculate_deceleration(phase: Phase, out_of_hp: bool, pace_down_mode: bool) -> f64 {
    if out_of_hp {
        -1.2
    } else if pace_down_mode {
        // Pace Down mode added after 1.5 anniversary (assumed on 1st anniv. update)
        -0.5
    } else {
        match phase {
            Phase::Early => -1.2,
            Phase::Mid => -0.8,
            Phase::Late | Phase::Last => -1.0,
        }
    }
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
    // HPConsumptionPerSecond = 20.0 * (CurrentSpeed - BaseSpeed + 12.0)^2 / 144.0
    //                          * StatusModifier * GroundModifier

    let speed_diff = current_speed - base_speed + 12.0;
    let base_consumption = 20.0 * speed_diff.powi(2) / 144.0;

    // Status modifier (multiplicative)
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
    hp_required: f64,
}

fn calculate_last_spurt(
    current_hp: f64,
    remaining_distance: f64,
    base_speed: f64,
    max_last_spurt_speed: f64,
    late_race_base_target_speed: f64,
    wiz_stat: u16,
    track: Track,
    condition: TrackCondition,
    guts_stat: u16,
    rng_roll: f64, // Random value between 0.0 and 1.0 for wisdom check
) -> (f64, f64) {
    // Last spurt calculation estimates stamina usage up to 60m before goal line
    let calculation_distance = remaining_distance - 60.0;

    // Quick check: if HP is sufficient for max speed, use it immediately
    let max_hp_required = estimate_hp_consumption(
        max_last_spurt_speed,
        calculation_distance,
        base_speed,
        track,
        condition,
        guts_stat,
    );

    if current_hp >= max_hp_required {
        return (max_last_spurt_speed, remaining_distance);
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
            guts_stat,
        );

        // Find distance we can actually run at this speed with available HP
        let usable_distance = if hp_required <= current_hp {
            remaining_distance
        } else {
            // Scale down distance based on HP available
            let hp_ratio = current_hp / hp_required;
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
            hp_required,
        });

        test_speed -= 0.1;
    }

    // Sort candidates by time (best to worst)
    candidates.sort_by(|a, b| a.time.partial_cmp(&b.time).unwrap());

    // Wisdom check: 15 + 0.05 * WizStat [%]
    let acceptance_chance = (15.0 + 0.05 * wiz_stat as f64) / 100.0;

    // Try each candidate from best to worst
    let mut cumulative_probability = 0.0;
    for candidate in &candidates {
        cumulative_probability += acceptance_chance;
        if rng_roll < cumulative_probability {
            return (candidate.speed, candidate.distance);
        }
    }

    // If all checks fail, use the slowest (last) candidate
    if let Some(last) = candidates.last() {
        (last.speed, last.distance)
    } else {
        // Fallback: use late-race base speed for full distance
        (late_race_base_target_speed, remaining_distance)
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
    for skill in skills {
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
    }
}

pub struct RaceState {
    pub runner: Runner,
    pub behavior: GameBehavior,
}

impl RaceState {
    pub fn new(
        uma: Uma,
        motivation: Motivation,
        course: Course,
        params: RaceParameters,
        skills: Vec<Skill>,
    ) -> Self {
        let behavior = GameBehavior::Original;
        let runner = Runner::new(uma, motivation, course, params, skills, behavior);
        RaceState { runner, behavior }
    }
    pub fn step(&mut self, delta_time: f64) {
        self.runner.step(delta_time);
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
            state.runner.activated_recoveries += 1;
            //state.recover(skill.modifier);
            if state.behavior != GameBehavior::Original && state.runner.phase >= Phase::Mid {
                //TODO: this.updateLastSpurtState();
            }
            //TODO: if (!CC_GLOBAL && this.phase >= 2 && !this.isLastSpurt) this.updateLastSpurtState();
        }
        SkillType::ActivateRandomGold => {
            // TODO: activate random gold skill
        }
        SkillType::ExtendEvolvedDuration => {
            // TODO: Extend duration of evolved skills
        }
        // Unknown skill types - placeholder for future implementation
        SkillType::Unknown41 => {
            // TODO: implement these skill types when their effects are known
        }
        _ => {
            // TODO: implement other known skill types
        }
    }
    state.runner.activated_skills[state.runner.phase as usize] += 1;
}
