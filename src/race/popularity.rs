//! # Popularity Calculation
//!
//! An runner's popularity (favorite rank) is obtained from three "judges". Each judge
//! gives a score based on various stats:
//!
//! - **Left Judge**: AdjustedSpeed * φ(BaseGuts)^0.5
//! - **Center Judge**: AdjustedStamina * φ(BaseGuts)^0.8 * φ(BaseWiz)^0.6
//! - **Right Judge**: AdjustedPower * φ(BaseWiz)^0.5
//!
//! Where φ(Stat) is the standard deviation score for the computed stat across all
//! runners (mean 50.0, std dev 10.0).
//!
//! Stats are adjusted by adding points from activable skills to the base stats;
//! skills related to speed, stamina, and power contribute points to those stats
//! respectively. The sum is then multiplied by distance/track aptitude and
//! motivation coefficients.
//!
//! For each judge, the runners receive marks based on their rank; first five places
//! receive, in order, ◎, ○, ▲, or △ (two runners get the last mark). The other runners
//! receive no mark. Having any of these marks adds ~4 points to the Uma's popularity
//! score corresponding to that judge.
//!
//! Finally, the runner's total popularity score is the sum of the points from each
//! judge. The runner are then ranked by their total popularity score to determine
//! their final favorite rank.

use std::collections::HashMap;

use crate::race::modifiers::{distance_proficiency_multiplier, ground_type_proficiency_multiplier};
use crate::race::solver::RaceState;
use crate::race::unready_runner::UnreadyRunner;
use crate::skill::{Skill, SkillRarity, SkillType};
use crate::uma::motivation_multiplier;

/// Popularity mark assigned by a judge
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PopularityMark {
    None,
    Fourth, // △
    Third,  // ▲
    Second, // ○
    First,  // ◎
}

impl PopularityMark {
    /// Get the points value for this mark (~4 points each)
    pub fn points(&self) -> f64 {
        match self {
            PopularityMark::None => 0.0,
            PopularityMark::Fourth => 4.0,
            PopularityMark::Third => 4.0,
            PopularityMark::Second => 4.0,
            PopularityMark::First => 4.0,
        }
    }

    /// Get the display character for this mark
    pub fn as_char(&self) -> char {
        match self {
            PopularityMark::None => '-',
            PopularityMark::Fourth => '△',
            PopularityMark::Third => '▲',
            PopularityMark::Second => '○',
            PopularityMark::First => '◎',
        }
    }
}

/// Scores and marks from all three judges
#[derive(Debug, Clone)]
pub struct JudgeScores {
    pub left_score: f64,
    pub left_mark: PopularityMark,
    pub center_score: f64,
    pub center_mark: PopularityMark,
    pub right_score: f64,
    pub right_mark: PopularityMark,
}

impl Default for JudgeScores {
    fn default() -> Self {
        JudgeScores {
            left_score: 0.0,
            left_mark: PopularityMark::None,
            center_score: 0.0,
            center_mark: PopularityMark::None,
            right_score: 0.0,
            right_mark: PopularityMark::None,
        }
    }
}

impl JudgeScores {
    /// Get total popularity points from all judges
    pub fn total_points(&self) -> f64 {
        self.left_mark.points() + self.center_mark.points() + self.right_mark.points()
    }
}

/// Stats that a skill can affect
#[derive(Debug, Clone, Copy)]
struct StatContributions {
    speed: bool,
    stamina: bool,
    power: bool,
}

/// Determine which stats a skill's effects contribute to for popularity calculation
fn categorize_skill_effects(skill: &Skill) -> StatContributions {
    let mut contributions = StatContributions {
        speed: false,
        stamina: false,
        power: false,
    };

    // Check all alternatives for effect types
    for alternative in &skill.alternatives {
        for effect in &alternative.effects {
            match effect.skill_type {
                // Speed-altering skills
                SkillType::SpeedUp
                | SkillType::CurrentSpeed
                | SkillType::CurrentSpeedWithNaturalDeceleration
                | SkillType::TargetSpeed => {
                    contributions.speed = true;
                }
                // Stamina-altering skills (including Recovery)
                SkillType::StaminaUp | SkillType::Recovery => {
                    contributions.stamina = true;
                }
                // Power-altering skills (including Acceleration and FoV)
                SkillType::PowerUp
                | SkillType::Acceleration
                | SkillType::AccelerationFullThrottle
                | SkillType::Fov => {
                    contributions.power = true;
                }
                // All stats up
                SkillType::AllStatsUp => {
                    contributions.speed = true;
                    contributions.stamina = true;
                    contributions.power = true;
                }
                _ => {}
            }
        }
    }

    contributions
}

/// Calculate added points to stats from activable skills
fn calculate_added_points(runner: &UnreadyRunner, state: &RaceState) -> (f64, f64, f64) {
    let mut added_speed = 0.0;
    let mut added_stamina = 0.0;
    let mut added_power = 0.0;

    // Get all skills (both out-of-gate and runtime)
    let all_skills: Vec<&Skill> = runner
        .out_of_gate_skills
        .iter()
        .chain(runner.runtime_skills.iter())
        .collect();

    for skill in all_skills {
        // Skip skills that can't activate on this course
        if !skill.is_activable_on_course(state, runner) {
            continue;
        }

        // Determine base points based on rarity
        let base_points = match skill.rarity {
            SkillRarity::Gold | SkillRarity::Evolution => 60.0,
            _ => 20.0,
        };

        // Determine which stats this skill contributes to
        let contributions = categorize_skill_effects(skill);

        // Add points to each applicable stat, weighted by activation chance
        let weighted_points = base_points * skill.activation_chance(runner.base.wiz);
        if contributions.speed {
            added_speed += weighted_points;
        }
        if contributions.stamina {
            added_stamina += weighted_points;
        }
        if contributions.power {
            added_power += weighted_points;
        }
    }

    (added_speed, added_stamina, added_power)
}

/// Calculate standard deviation score (φ) for a value given a mean and standard
/// deviation. Mean value scores 50; each standard deviation adds/subtracts 10 points.
///
/// Returns 50.0 if `std_dev` is zero (all values are the same).
fn calculate_phi(value: f64, mean: f64, std_dev: f64) -> f64 {
    if std_dev.abs() < 1e-9 {
        // If all values are the same, return 50.0 (no deviation)
        50.0
    } else {
        (value - mean) / std_dev * 10.0 + 50.0
    }
}

/// Calculate mean and standard deviation for a set of values
fn calculate_stats(values: &[f64]) -> (f64, f64) {
    if values.is_empty() {
        return (0.0, 0.0);
    }

    let mean = values.iter().sum::<f64>() / values.len() as f64;

    let variance = values
        .iter()
        .map(|v| {
            let diff = v - mean;
            diff * diff
        })
        .sum::<f64>()
        / values.len() as f64;

    let std_dev = variance.sqrt();

    (mean, std_dev)
}

/// Data for calculating popularity for a single runner
struct RunnerPopData {
    adjusted_speed: f64,
    adjusted_stamina: f64,
    adjusted_power: f64,
    base_guts: f64,
    base_wiz: f64,
}

/// Calculate the three judge scores for each runner
fn calculate_judge_scores(runners_data: &[RunnerPopData]) -> Vec<(f64, f64, f64)> {
    let n = runners_data.len();

    // First, calculate φ scores for base stats across all runners
    let guts_values: Vec<f64> = runners_data.iter().map(|r| r.base_guts).collect();
    let wiz_values: Vec<f64> = runners_data.iter().map(|r| r.base_wiz).collect();

    let (guts_mean, guts_std) = calculate_stats(&guts_values);
    let (wiz_mean, wiz_std) = calculate_stats(&wiz_values);

    let guts_phi: Vec<f64> = guts_values
        .iter()
        .map(|&v| calculate_phi(v, guts_mean, guts_std))
        .collect();

    let wiz_phi: Vec<f64> = wiz_values
        .iter()
        .map(|&v| calculate_phi(v, wiz_mean, wiz_std))
        .collect();

    // Calculate intermediate values for each judge
    let mut left_values = Vec::with_capacity(n);
    let mut center_values = Vec::with_capacity(n);
    let mut right_values = Vec::with_capacity(n);

    for (i, runner) in runners_data.iter().enumerate() {
        // Left Judge: AdjustedSpeed * φ(BaseGuts)^0.5
        left_values.push(runner.adjusted_speed * guts_phi[i].abs().powf(0.5));

        // Center Judge: AdjustedStamina * φ(BaseGuts)^0.8 * φ(BaseWiz)^0.6
        center_values.push(
            runner.adjusted_stamina * guts_phi[i].abs().powf(0.8) * wiz_phi[i].abs().powf(0.6),
        );

        // Right Judge: AdjustedPower * φ(BaseWiz)^0.5
        right_values.push(runner.adjusted_power * wiz_phi[i].abs().powf(0.5));
    }

    // Calculate φ scores for each judge's values
    let (left_mean, left_std) = calculate_stats(&left_values);
    let (center_mean, center_std) = calculate_stats(&center_values);
    let (right_mean, right_std) = calculate_stats(&right_values);

    let mut scores = Vec::with_capacity(n);
    for i in 0..n {
        scores.push((
            calculate_phi(left_values[i], left_mean, left_std),
            calculate_phi(center_values[i], center_mean, center_std),
            calculate_phi(right_values[i], right_mean, right_std),
        ));
    }

    scores
}

/// Assign marks to top 5 runners for a single judge's scores
fn assign_marks_for_judge(scores: &[(usize, f64)]) -> HashMap<usize, PopularityMark> {
    // Sort by score (descending)
    let mut sorted: Vec<(usize, f64)> = scores.to_vec();
    sorted.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

    let mut marks = HashMap::new();

    // Assign marks to top 5 (or fewer if there are fewer runners)
    for (rank, (runner_idx, _score)) in sorted.iter().enumerate() {
        let mark = match rank {
            0 => PopularityMark::First,      // ◎
            1 => PopularityMark::Second,     // ○
            2 => PopularityMark::Third,      // ▲
            3 | 4 => PopularityMark::Fourth, // △ (both 4th and 5th get this)
            _ => break,                      // No marks beyond top 5
        };
        marks.insert(*runner_idx, mark);
    }

    marks
}

/// Calculate popularity rankings for all runners
pub fn calculate_popularity(runners: &mut [UnreadyRunner], state: &RaceState) -> Vec<u8> {
    let n = runners.len();

    // Calculate added points and adjusted stats for each runner
    let mut runners_data = Vec::with_capacity(n);

    for runner in runners.iter() {
        let (added_speed, added_stamina, added_power) = calculate_added_points(runner, state);

        // Get coefficients
        let mood_coef = motivation_multiplier(runner.initial_state.motivation);
        let distance_coef = distance_proficiency_multiplier(runner.base.distance_aptitude);
        let track_coef = ground_type_proficiency_multiplier(runner.base.track_aptitude);

        // Calculate adjusted stats for popularity
        // Note: These are different from the adjusted stats used in the race simulation
        let adjusted_speed = (runner.base.speed as f64 + added_speed) * distance_coef * mood_coef;
        let adjusted_stamina = (runner.base.stamina as f64 + added_stamina) * mood_coef;
        let adjusted_power =
            (runner.base.power as f64 + added_power) * distance_coef * track_coef * mood_coef;

        runners_data.push(RunnerPopData {
            adjusted_speed,
            adjusted_stamina,
            adjusted_power,
            base_guts: runner.base.guts as f64,
            base_wiz: runner.base.wiz as f64,
        });
    }

    // Calculate judge scores
    let judge_scores = calculate_judge_scores(&runners_data);

    // Assign marks for each judge
    let left_scores: Vec<(usize, f64)> = judge_scores
        .iter()
        .enumerate()
        .map(|(i, &(l, _, _))| (i, l))
        .collect();
    let center_scores: Vec<(usize, f64)> = judge_scores
        .iter()
        .enumerate()
        .map(|(i, &(_, c, _))| (i, c))
        .collect();
    let right_scores: Vec<(usize, f64)> = judge_scores
        .iter()
        .enumerate()
        .map(|(i, &(_, _, r))| (i, r))
        .collect();

    let left_marks = assign_marks_for_judge(&left_scores);
    let center_marks = assign_marks_for_judge(&center_scores);
    let right_marks = assign_marks_for_judge(&right_scores);

    // Calculate total popularity points and update runners
    let mut popularity_data: Vec<(usize, f64, JudgeScores)> = Vec::with_capacity(n);

    for i in 0..n {
        let left_mark = left_marks.get(&i).copied().unwrap_or(PopularityMark::None);
        let center_mark = center_marks
            .get(&i)
            .copied()
            .unwrap_or(PopularityMark::None);
        let right_mark = right_marks.get(&i).copied().unwrap_or(PopularityMark::None);

        let (left_score, center_score, right_score) = judge_scores[i];

        let judge_scores = JudgeScores {
            left_score,
            left_mark,
            center_score,
            center_mark,
            right_score,
            right_mark,
        };

        let total_points = judge_scores.total_points();
        popularity_data.push((i, total_points, judge_scores));
    }

    // Sort by total points (descending) to determine ranks
    popularity_data.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

    // Assign ranks (1-indexed, ties get same rank)
    let mut ranks = vec![0u8; n];
    let mut current_rank = 1u8;
    let mut prev_points = f64::MAX;

    for (runner_idx, points, judge_scores) in popularity_data {
        // If points are different from previous, update rank
        if (points - prev_points).abs() > 1e-9 {
            current_rank = (ranks.iter().filter(|&&r| r > 0).count() + 1) as u8;
        }

        ranks[runner_idx] = current_rank;
        runners[runner_idx].initial_state.judge_scores = judge_scores;
        prev_points = points;
    }

    ranks
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_popularity_mark_points() {
        assert_eq!(PopularityMark::None.points(), 0.0);
        assert_eq!(PopularityMark::First.points(), 4.0);
        assert_eq!(PopularityMark::Second.points(), 4.0);
        assert_eq!(PopularityMark::Third.points(), 4.0);
        assert_eq!(PopularityMark::Fourth.points(), 4.0);
    }

    #[test]
    fn test_judge_scores_total() {
        let scores = JudgeScores {
            left_score: 1.5,
            left_mark: PopularityMark::First,
            center_score: 0.8,
            center_mark: PopularityMark::Second,
            right_score: -0.3,
            right_mark: PopularityMark::None,
        };
        assert_eq!(scores.total_points(), 8.0); // 4 + 4 + 0
    }

    #[test]
    fn test_stat_contributions() {
        use crate::skill::*;

        // Test speed skill
        let speed_skill = Skill::new(
            "test_speed".to_string(),
            SkillData {
                rarity: SkillRarity::White,
                alternatives: vec![SkillAlternative {
                    base_duration: 0,
                    condition: "".to_string(),
                    precondition: "".to_string(),
                    effects: vec![SkillEffect {
                        skill_type: SkillType::CurrentSpeed,
                        target: SkillTarget::Oneself,
                        modifier: 100,
                    }],
                }],
                wisdom_check: true,
            },
        );

        let contributions = categorize_skill_effects(&speed_skill);
        assert!(contributions.speed);
        assert!(!contributions.stamina);
        assert!(!contributions.power);

        // Test AllStatsUp
        let all_stats_skill = Skill::new(
            "test_all".to_string(),
            SkillData {
                rarity: SkillRarity::White,
                alternatives: vec![SkillAlternative {
                    base_duration: 0,
                    condition: "".to_string(),
                    precondition: "".to_string(),
                    effects: vec![SkillEffect {
                        skill_type: SkillType::AllStatsUp,
                        target: SkillTarget::Oneself,
                        modifier: 50,
                    }],
                }],
                wisdom_check: true,
            },
        );

        let contributions = categorize_skill_effects(&all_stats_skill);
        assert!(contributions.speed);
        assert!(contributions.stamina);
        assert!(contributions.power);
    }

    #[test]
    fn test_phi_calculation() {
        // Test normal case
        let phi = calculate_phi(15.0, 10.0, 5.0);
        // 1 std dev above mean = 50 + 10 = 60
        assert!((phi - 60.0).abs() < 0.001);

        // Test zero std dev (all values same)
        let phi = calculate_phi(10.0, 10.0, 0.0);
        // mean value = 50
        assert_eq!(phi, 50.0);
    }

    #[test]
    fn test_calculate_stats() {
        let values = vec![10.0, 20.0, 30.0];
        let (mean, std_dev) = calculate_stats(&values);

        assert!((mean - 20.0).abs() < 0.001);
        assert!(std_dev > 0.0);
    }
}
