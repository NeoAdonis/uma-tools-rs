use crate::course::*;
use crate::race::hp::*;
use crate::race::parameters::{RaceParameters, TrackCondition};
use crate::uma::skill::*;
use crate::uma::*;
use rand::prelude::*;
use std::cell::RefCell;
use std::rc::Rc;

#[allow(dead_code)]
pub enum GameBehavior {
    Original,         // when the game was first released
    FirstAnniversary, // after the first anniversary update
}

pub static GAME_BEHAVIOR: GameBehavior = GameBehavior::Original;

pub fn hp_style_coefficient(style: Style) -> f64 {
    match style {
        Style::Oonige => 0.86,
        Style::Front => 0.95,
        Style::Pace => 0.89,
        Style::Late => 1.0,
        Style::End => 0.995,
    }
}

pub struct LastSpurt {
    pub max_speed: f64,
    pub target_distance: f64,
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
    pub hp_guts_modifier: f64,
    pub min_speed: f64,
    pub base_target_speed: f64,
    pub available_skills: Vec<Skill>,
    pub activated_skills: Vec<Skill>,
    pub is_rushed: bool, // TODO: implement
    pub rush_section: Option<u8>,
    pub is_start_dash: bool,
    pub slope_acceleration: Option<f64>,
    pub phase: Phase,
    pub last_spurt: Option<LastSpurt>,
    pub current_corner_idx: Option<usize>,
    pub current_slope_idx: Option<usize>,
    pub current_straight_idx: Option<usize>,
    pub rng: Rc<RefCell<rand_pcg::Pcg64Mcg>>,
}

/// Adjust base stats according to course and race parameters.
pub fn adjust_stats(uma: &Uma, course: &Course, parameters: &RaceParameters) -> Uma {
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

struct LastSpurtCandidate {
    speed: f64,
    distance: f64,
    time: f64,
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

// n.b. While no comparison checks for a good start,
// it is used in Team Trials to award bonus points.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum StartType {
    Good,
    Normal,
    Late,
}

impl Runner {
    pub fn new(
        uma: Uma,
        motivation: Motivation,
        course: &Course,
        params: &RaceParameters,
        skills: Vec<Skill>,
        rng: Rc<RefCell<rand_pcg::Pcg64Mcg>>,
    ) -> Self {
        // 1. Calculate base stats
        let mut start_delay = 1.0;
        let gate_roll: i32 = rng.borrow_mut().random_range(0..=100);
        let base_uma = Uma::raw_to_base(uma, motivation);
        // 2. Calculate skill activation chance and determine whether skills can activate
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
        // 3. Activate out of gate skills
        // TODO: implement
        // let gold_indices: Vec<usize> = skills.iter().enumerate()
        //     .filter(|(_, skill)| {
        //         (skill.rarity == SkillRarity::Gold || skill.rarity == SkillRarity::Evolution)
        //             && skill.effects.iter().all(|ef| ef.skill_type > SkillType::WizUp)
        //     })
        //     .map(|(i, _)| i)
        //     .collect();
        // 4. Use item.
        // n.b.: This is the same as setting race parameters ahead of time so no action needed here.
        // 5. Set starting location based off gate number
        // TODO: implement starting location; assume no effect for now
        // 6. Calculate start delay
        start_delay *= rng.borrow_mut().random_range(0.0..=0.1);
        // 7. Calculate adjusted stats
        let adjusted_uma = adjust_stats(&base_uma, course, params);
        // 8. Convert stamina to HP
        let max_hp = 0.8 * hp_style_coefficient(adjusted_uma.style) * (adjusted_uma.stamina as f64)
            + course.distance as f64;
        // 9. Set starting speed (this is done in the constructor)
        // 10. Determine rush chance
        let rush_chance = (0.65 / (0.1 * uma.wiz as f64 + 1.0).log10()).powf(2.0);
        let roll: f64 = rng.borrow_mut().random();
        let rush_section = if roll < rush_chance {
            Some(rng.borrow_mut().random_range(2..=9))
        } else {
            None
        };

        // Initialize race state
        Runner {
            base: base_uma,
            adjusted: adjusted_uma,
            motivation,
            gate_roll: gate_roll as u8,
            speed: 3.0,
            acceleration: 0.0, // start dash acceleration is applied separately
            min_speed: 0.85 * course.base_speed()
                + (200.0 + adjusted_uma.guts as f64).sqrt() * 0.001,
            base_target_speed: 0.85 * course.base_speed(),
            start_delay,
            available_skills,
            activated_skills: Vec::new(),
            phase: Phase::Early,
            hp: max_hp,
            max_hp,
            hp_guts_modifier: 1.0 + 200.0 / (600.0 * adjusted_uma.guts as f64).sqrt(),
            position: 0.0,
            rush_section,
            is_rushed: false,
            is_start_dash: true,
            slope_acceleration: None,
            last_spurt: None,
            current_corner_idx: None,
            current_slope_idx: None,
            current_straight_idx: None,
            rng,
        }
    }

    pub fn start_type(&self) -> StartType {
        match self.start_delay {
            d if d < 0.02 => StartType::Good,
            d if d > 0.08 => StartType::Late,
            _ => StartType::Normal,
        }
    }

    pub fn accelerate(&mut self, skill_modifier: f64) {
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

        self.acceleration = base_acceleration + skill_modifier + start_dash_modifier;
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
        self.base_target_speed + self.slope_acceleration.unwrap_or(0.0)
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
        self.base_target_speed = if let Some(last_spurt) = &self.last_spurt {
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
        // TODO: review
        self.speed += self.acceleration * delta_time;
        if self.speed > self.target_speed() {
            self.speed = self.target_speed();
        }
        if self.is_start_dash && self.speed >= 0.85 * course_base_speed {
            self.is_start_dash = false;
        } else if !self.is_start_dash && self.speed < self.min_speed {
            self.speed = self.min_speed;
        }
    }

    pub fn update_acceleration(&mut self) {
        if self.speed < self.target_speed() {
            // TODO: calculate skill modifier
            self.accelerate(0.0);
        } else {
            self.decelerate();
        }
    }

    pub fn update_position(&mut self, delta_time: f64) {
        self.position += self.speed * delta_time;
    }

    pub fn calculate_last_spurt(&mut self, course: &Course, condition: TrackCondition) {
        // TODO: review function

        let max_last_spurt_speed = last_spurt_speed_max(
            course.base_speed(),
            self.adjusted.style,
            self.adjusted.speed,
            self.adjusted.guts,
            self.adjusted.distance_aptitude,
        );
        let late_race_base_target_speed = base_target_speed(
            course.base_speed(),
            self.adjusted.style,
            Phase::Late,
            self.adjusted.speed,
            self.adjusted.distance_aptitude,
        );

        let remaining_distance = course.distance as f64 - self.position;

        // Estimate stamina usage up to 60m before goal line
        let calculation_distance = remaining_distance - 60.0;

        // If HP is sufficient for max speed, use it immediately
        let max_hp_required = estimate_hp_consumption(
            max_last_spurt_speed,
            calculation_distance,
            course.base_speed(),
            course.surface,
            condition,
            self.adjusted.guts,
        );

        if self.hp >= max_hp_required {
            self.last_spurt = Some(LastSpurt {
                max_speed: max_last_spurt_speed,
                target_distance: remaining_distance,
            });
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
                course.base_speed(),
                course.surface,
                condition,
                self.adjusted.guts,
            );

            // Find distance we can actually run at this speed with available HP
            let usable_distance = if hp_required <= self.hp {
                remaining_distance
            } else {
                // Scale down distance based on HP available
                let hp_ratio = self.hp / hp_required;
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

        let acceptance_chance = (15.0 + 0.05 * self.adjusted.wiz as f64) / 100.0;
        let rng_roll: f64 = self.rng.borrow_mut().random();

        // Try each candidate from best to worst
        candidates.sort_by(|a, b| a.time.partial_cmp(&b.time).unwrap());
        let mut cumulative_probability = 0.0;
        for candidate in &candidates {
            cumulative_probability += acceptance_chance;
            if rng_roll < cumulative_probability {
                self.last_spurt = Some(LastSpurt {
                    max_speed: candidate.speed,
                    target_distance: candidate.distance,
                });
            }
        }

        // If all checks fail, use the slowest (last) candidate
        self.last_spurt = Some(if let Some(last) = candidates.last() {
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
        });
    }
}
