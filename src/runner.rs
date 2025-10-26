use crate::course::*;
use crate::race_parameters::*;
use crate::skill::*;
use crate::uma::*;

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
    pub target_speed: f64,
    pub available_skills: Vec<Skill>,
    pub activated_skills: Vec<Skill>,
    pub is_rushed: bool, // TODO: implement
    pub rush_section: Option<u8>,
    pub phase: Phase,
    pub last_spurt: Option<LastSpurt>,
    pub current_corner_idx: Option<usize>,
    pub current_slope_idx: Option<usize>,
}

pub fn adjust_stats(uma: &Uma, course: &Course, parameters: &RaceParameters) -> Uma {
    let course_modifier = course_speed_modifier(&uma, &course.course_set_status);
    // n.b.: In single mode, all uma gain 400 on stats after adjustment.
    // This is not implemented here.
    Uma {
        speed: calculate_adjusted_speed(
            uma.speed,
            course.surface,
            parameters.condition,
            course_modifier,
        ),
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

enum StartType {
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
    ) -> Self {
        // 1. Calculate base stats
        let mut start_delay = 1.0;
        let gate_roll: i32 = 1; // TODO: random
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
                let roll: f64 = 0.0; // TODO: random
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
        start_delay *= 0.05; // TODO: random up to 0.1 seconds
        // 7. Calculate adjusted stats
        let adjusted_uma = adjust_stats(&base_uma, &course, &params);
        // 8. Convert stamina to HP
        let max_hp = 0.8 * hp_style_coefficient(adjusted_uma.style) * (adjusted_uma.stamina as f64)
            + course.distance as f64;
        // 9. Set starting speed (this is done in the constructor)
        // 10. Determine rush chance
        let rush_chance = (0.65 / (0.1 * uma.wiz as f64 + 1.0).log10()).powf(2.0);
        let roll: f64 = 1.0; // TODO: random
        let rush_section = if roll < rush_chance { Some(2) } else { None }; // TODO: random section from 2 to 9

        // Initialize race state
        Runner {
            base: base_uma,
            adjusted: adjusted_uma,
            motivation,
            gate_roll: gate_roll as u8,
            speed: 3.0,
            acceleration: 24.0, // start dash
            min_speed: 0.85 * course.base_speed()
                + (200.0 + adjusted_uma.guts as f64).sqrt() * 0.001,
            target_speed: 0.85 * course.base_speed(),
            start_delay,
            available_skills,
            activated_skills: Vec::new(),
            phase: Phase::Early,
            hp: max_hp,
            max_hp: max_hp,
            hp_guts_modifier: 1.0 + 200.0 / (600.0 * adjusted_uma.guts as f64).sqrt(),
            position: 0.0,
            rush_section,
            is_rushed: false,
            last_spurt: None,
            current_corner_idx: None,
            current_slope_idx: None,
        }
    }

    // TODO: report
    #[allow(dead_code)]
    fn start_type(&self) -> StartType {
        match self.start_delay {
            d if d < 0.02 => StartType::Good,
            d if d > 0.08 => StartType::Late,
            _ => StartType::Normal,
        }
    }
}
