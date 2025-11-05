use rand::Rng;

use crate::course::{Course, Phase};
use crate::race::hp::estimate_hp_consumption;
use crate::race::modifiers::*;
use crate::race::parameters::TrackCondition;
use crate::race::runner::{GAME_BEHAVIOR, GameBehavior, Runner};
use crate::uma::{Aptitude, Style};

pub struct LastSpurt {
    pub max_speed: f64,
    pub target_distance: f64,
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

pub fn calculate_last_spurt(runner: &mut Runner, course: &Course, condition: TrackCondition) {
    // TODO: review function

    let max_last_spurt_speed = last_spurt_speed_max(
        course.base_speed(),
        runner.adjusted.style,
        runner.adjusted.speed,
        runner.adjusted.guts,
        runner.adjusted.distance_aptitude,
    );
    let late_race_base_target_speed = base_target_speed(
        course.base_speed(),
        runner.adjusted.style,
        Phase::Late,
        runner.adjusted.speed,
        runner.adjusted.distance_aptitude,
    );

    let remaining_distance = course.distance as f64 - runner.position;

    // Estimate stamina usage up to 60m before goal line
    let calculation_distance = remaining_distance - 60.0;

    // If HP is sufficient for max speed, use it immediately
    let max_hp_required = estimate_hp_consumption(
        max_last_spurt_speed,
        course,
        condition,
        calculation_distance,
        runner.adjusted.guts,
    );

    if runner.hp >= max_hp_required {
        runner.last_spurt = Some(LastSpurt {
            max_speed: max_last_spurt_speed,
            target_distance: runner.position,
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
            course,
            condition,
            calculation_distance,
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
    let rng_roll: f64 = runner.rng.borrow_mut().random();

    // Try each candidate from best to worst
    candidates.sort_by(|a, b| a.time.partial_cmp(&b.time).unwrap());
    let mut cumulative_probability = 0.0;
    for candidate in &candidates {
        cumulative_probability += acceptance_chance;
        if rng_roll < cumulative_probability {
            runner.last_spurt = Some(LastSpurt {
                max_speed: candidate.speed,
                target_distance: course.distance as f64 - candidate.distance,
            });
        }
    }

    // If all checks fail, use the slowest (last) candidate
    runner.last_spurt = Some(if let Some(last) = candidates.last() {
        LastSpurt {
            max_speed: last.speed,
            target_distance: course.distance as f64 - last.distance,
        }
    } else {
        // Fallback: use late-race base speed for full distance
        // TODO: ensure this case is unreachable
        LastSpurt {
            max_speed: late_race_base_target_speed,
            target_distance: runner.position,
        }
    });
}
