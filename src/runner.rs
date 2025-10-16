use crate::course::*;
use crate::race_parameters::*;
use crate::skill::*;
use crate::uma::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum GameBehavior {
    Original,         // when the game was first released
    FirstAnniversary, // after the first anniversary update
}

pub fn hp_style_coefficient(style: Style) -> f64 {
    match style {
        Style::Oonige => 0.86,
        Style::Front => 0.95,
        Style::Pace => 0.89,
        Style::Late => 1.0,
        Style::End => 0.995,
    }
}

pub struct Runner {
    pub base: Uma,
    pub adjusted: Uma,
    pub mood: Mood,
    pub start_delay: u16,
    pub gate_roll: u8,
    pub speed: f64,        // in m/s
    pub acceleration: f64, // in m/s^2
    pub hp: f64,
    pub max_hp: f64,
    pub hp_guts_modifier: f64,
    pub hp_subpar_accept_chance: f64, //TODO: investigate
    pub min_speed: f64,
    pub target_speed: f64,
    pub activated_recoveries: u8,
    pub activated_skills: [u8; 4],
    pub phase: Phase,
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
        wit: calculate_adjusted_wit(uma.wit, uma.track_aptitude),
        style: uma.style,
        distance_aptitude: uma.distance_aptitude,
        track_aptitude: uma.track_aptitude,
        style_aptitude: uma.style_aptitude,
    }
}

impl Runner {
    pub fn new(
        uma: Uma,
        mood: Mood,
        course: Course,
        params: RaceParameters,
        skills: Vec<Skill>,
        behavior: GameBehavior,
    ) -> Self {
        // 1. Calculate base stats
        let mut start_delay = 1.0;
        let gate_roll: i32 = 1; // TODO: random
        let base_uma = Uma::raw_to_base(uma, mood);
        // 2. Calculate skill activation chance and determine whether skills can activate
        let skill_chance = (1.0 - 90.0 / uma.wit as f64).max(0.2);
        // 3. Activate out of gate skills
        // TODO: implement
        // let gold_indices: Vec<usize> = skills.iter().enumerate()
        //     .filter(|(_, skill)| {
        //         (skill.rarity == SkillRarity::Gold || skill.rarity == SkillRarity::Evolution)
        //             && skill.effects.iter().all(|ef| ef.skill_type > SkillType::WitUp)
        //     })
        //     .map(|(i, _)| i)
        //     .collect();
        // 4. Use item.
        // n.b.: This is the same as setting race parameters ahead of time so no action needed here.
        // 5. Set starting location based off gate number
        // TODO: implement starting location; assume no effect for now
        // 6. Calculate start delay
        start_delay *= 0.5; // TODO: random
        // 7. Calculate adjusted stats
        let adjusted_uma = adjust_stats(&base_uma, &course, &params);
        // 8. Convert stamina to HP
        let max_hp = 0.8 * hp_style_coefficient(adjusted_uma.style) * (adjusted_uma.stamina as f64)
            + course.distance as f64;
        // 9. Set starting speed (this is done in the constructor)
        // 10. Determine rush chance
        let rush_chance = (0.65 / (0.1 * uma.wit as f64 + 1.0).log10()).powf(2.0);

        // Initialize race state
        let min_speed =
            0.85 * course.base_speed() + (200.0 + adjusted_uma.guts as f64).sqrt() * 0.001;
        let hp_guts_modifier = 1.0 + 200.0 / (600.0 * adjusted_uma.guts as f64).sqrt();
        let hp_subpar_accept_chance = (15.0 + 0.05 * adjusted_uma.wit as f64).round();
        Runner {
            base: base_uma,
            adjusted: adjusted_uma,
            mood,
            gate_roll: gate_roll as u8,
            speed: 3.0,
            acceleration: 24.0, // start dash
            min_speed: min_speed,
            target_speed: 0.85 * course.base_speed(),
            start_delay: start_delay as u16,
            activated_recoveries: 0,
            activated_skills: [0; 4],
            phase: Phase::Early,
            hp: max_hp,
            max_hp: max_hp,
            hp_guts_modifier: hp_guts_modifier,
            hp_subpar_accept_chance: hp_subpar_accept_chance,
        }
    }

    pub fn step(&mut self, dt: f64) {
        // TODO: implement
        // Pre-Update
        //  Check end of corner
        // Update
        //  Activate skills
        //  Recover HP by skills
        //  Update last spurt state
        //  Update target speed
        //  Calculate acceleration
        //  Update phase
        //  Calculate distance and position
        //  Check for slope
        // Post-Update
        // Events
        //  Check start of corner
    }
}
