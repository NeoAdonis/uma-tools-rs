//! # Popularity Calculation
//!
//! A runner's popularity (favorite rank) is obtained from three "judges". Each
//! judge gives a score based on various stats:
//!
//! - **Left Judge**: φ(AdjustedSpeed * φ(RawGuts)^0.5)
//! - **Center Judge**: φ(AdjustedStamina * φ(RawGuts)^0.8 * φ(RawWit)^0.6)
//! - **Right Judge**: φ(AdjustedPower * φ(RawWit)^0.5)
//!
//! Where φ(Stat) is the standard deviation score for the computed stat across
//! all runners (mean 50, std dev 10).
//!
//! Stats are adjusted by adding points from activable skills to the raw stats.
//! Each skill gives a number of points to one or two stats, read from the skill
//! data (e.g. 20 for white skills, 60 for gold skills; negative skills take
//! points away), scaled by its activation chance with raw wit. The sum is
//! then multiplied by mood and aptitude coefficients:
//!
//! - **Speed**: mood and distance aptitude.
//! - **Stamina**: mood.
//! - **Power**: mood, distance aptitude (acceleration) and ground type
//!   aptitude.
//!
//! For each judge, the runners receive marks based on their rank; first five
//! places receive, in order, ◎, ○, ▲, or △ (two runners get the last mark). The
//! other runners receive no mark. Each mark adds a bonus to the impact of that
//! judge.
//!
//! The impact of a judge is `Score^e + MarkBonus`, where `e` is 1 if the score
//! is above 55, 0.8 if any of the runner's judge scores is above 70, and
//! `Score - 54` otherwise. Finally, the runner's total popularity score is the
//! product of the impacts of all judges. The runners are then ranked by their
//! total popularity score to determine their final favorite rank.

use std::collections::HashMap;

use crate::race::modifiers::{
    acceleration_distance_proficiency_multiplier, distance_proficiency_multiplier,
    ground_type_proficiency_multiplier,
};
use crate::race::solver::RaceState;
use crate::race::unready_runner::UnreadyRunner;
use crate::skill::{PopularityStat, Skill};
use crate::uma::motivation_multiplier;

/// Popularity points a skill adds to each stat.
///
/// Guts and wit points don't affect the judges (they use base guts and wit),
/// but they are tracked because they take part of a skill's points.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub(crate) struct SkillPopularity {
    speed: f64,
    stamina: f64,
    power: f64,
    guts: f64,
    wiz: f64,
}

impl SkillPopularity {
    /// Create popularity points with `points` added to `stat`.
    fn with(mut self, stat: PopularityStat, points: f64) -> Self {
        match stat {
            PopularityStat::Speed => self.speed += points,
            PopularityStat::Stamina => self.stamina += points,
            PopularityStat::Power => self.power += points,
            PopularityStat::Guts => self.guts += points,
            PopularityStat::Wiz => self.wiz += points,
            PopularityStat::None => {}
        }
        self
    }
}

/// Get the popularity points a skill adds to each stat.
///
/// If there's no popularity data, it returns zero points for all stats.
fn skill_popularity(skill: &Skill) -> SkillPopularity {
    match &skill.popularity {
        Some(popularity) => popularity
            .iter()
            .fold(SkillPopularity::default(), |p, entry| {
                p.with(entry.stat, entry.value as f64)
            }),
        None => SkillPopularity::default(),
    }
}

/// Popularity mark assigned by a judge.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PopularityMark {
    None,
    Fourth, // △
    Third,  // ▲
    Second, // ○
    First,  // ◎
}

impl PopularityMark {
    /// Get the bonus this mark adds to the impact of a judge.
    fn points(&self) -> f64 {
        match self {
            PopularityMark::None => 0.0,
            PopularityMark::Fourth => 0.2,
            PopularityMark::Third => 0.5,
            PopularityMark::Second => 1.0,
            PopularityMark::First => 2.0,
        }
    }

    /// Get the display character for this mark.
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

/// Scores and marks from all three judges.
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
    /// Get total popularity points: the product of the impacts of all judges.
    pub fn total_points(&self) -> f64 {
        let high = [self.left_score, self.center_score, self.right_score]
            .iter()
            .any(|&score| score > 70.0);
        judge_impact(self.left_score, self.left_mark, high)
            * judge_impact(self.center_score, self.center_mark, high)
            * judge_impact(self.right_score, self.right_mark, high)
    }
}

/// Calculate the impact of a judge on popularity: `score^e + mark bonus`.
///
/// `high` is whether any of the runner's judge scores is above 70.
fn judge_impact(score: f64, mark: PopularityMark, high: bool) -> f64 {
    let exponent = if score > 55.0 {
        1.0
    } else if high {
        0.8
    } else {
        score - 54.0
    };
    score.powf(exponent) + mark.points()
}

/// Calculate the popularity points added to a runner's stats by its skills.
///
/// Skills that don't pass the popularity guards for this race add no points.
/// The points of each skill are weighted by its activation chance with raw wit,
/// regardless of whether the skill passed its wit check.
pub(crate) fn skill_bonus<'a>(
    skills: impl IntoIterator<Item = &'a Skill>,
    runner: &UnreadyRunner,
    state: &RaceState,
) -> SkillPopularity {
    skills
        .into_iter()
        .filter(|skill| skill.is_popularity_eligible(state, runner))
        .fold(SkillPopularity::default(), |total, skill| {
            let points = skill_popularity(skill);
            let chance = skill.activation_chance(runner.raw.wiz);
            SkillPopularity {
                speed: total.speed + points.speed * chance,
                stamina: total.stamina + points.stamina * chance,
                power: total.power + points.power * chance,
                guts: total.guts + points.guts * chance,
                wiz: total.wiz + points.wiz * chance,
            }
        })
}

/// Calculate standard deviation score (φ) for a value given a mean and standard
/// deviation.
///
/// Mean value scores 50; each standard deviation adds/subtracts 10
/// points.
///
/// With a population standard deviation, a value can't be further than
/// √(n − 1) standard deviations from the mean of n values. Thus, with up to 18
/// runners, φ is always within ~8.8-91.2 and never reaches zero or below.
///
/// Returns 50.0 if `std_dev` is zero (all values are the same).
fn calculate_phi(value: f64, mean: f64, std_dev: f64) -> f64 {
    if std_dev.abs() < 1e-9 {
        // If all values are the same, return 50.0 (no deviation).
        50.0
    } else {
        (value - mean) / std_dev * 10.0 + 50.0
    }
}

/// Calculate mean and standard deviation for a set of values.
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

/// Data for calculating popularity for a single runner.
struct RunnerPopData {
    adjusted_speed: f64,
    adjusted_stamina: f64,
    adjusted_power: f64,
    raw_guts: f64,
    raw_wiz: f64,
}

/// Calculate the three judge scores for each runner.
fn calculate_judge_scores(runners_data: &[RunnerPopData]) -> Vec<(f64, f64, f64)> {
    let n = runners_data.len();

    // Calculate φ scores for raw stats across all runners.
    let guts_values: Vec<f64> = runners_data.iter().map(|r| r.raw_guts).collect();
    let wiz_values: Vec<f64> = runners_data.iter().map(|r| r.raw_wiz).collect();

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

    // Calculate intermediate values for each judge.
    let mut left_values = Vec::with_capacity(n);
    let mut center_values = Vec::with_capacity(n);
    let mut right_values = Vec::with_capacity(n);

    for (i, runner) in runners_data.iter().enumerate() {
        // Left Judge: AdjustedSpeed * φ(RawGuts)^0.5.
        left_values.push(runner.adjusted_speed * guts_phi[i].powf(0.5));

        // Center Judge: AdjustedStamina * φ(RawGuts)^0.8 * φ(RawWit)^0.6.
        center_values.push(runner.adjusted_stamina * guts_phi[i].powf(0.8) * wiz_phi[i].powf(0.6));

        // Right Judge: AdjustedPower * φ(RawWit)^0.5.
        right_values.push(runner.adjusted_power * wiz_phi[i].powf(0.5));
    }

    // Calculate φ scores for each judge's values.
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

/// Assign marks to top 5 runners for a single judge's scores.
fn assign_marks_for_judge(scores: &[(usize, f64)]) -> HashMap<usize, PopularityMark> {
    // Sort by score (descending).
    let mut sorted: Vec<(usize, f64)> = scores.to_vec();
    sorted.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

    let mut marks = HashMap::new();

    // Assign marks to top 5 (or fewer if there are fewer runners).
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

/// Calculate popularity rankings for all runners.
pub fn calculate_popularity(runners: &mut [UnreadyRunner]) -> Vec<u8> {
    let n = runners.len();

    // Calculate adjusted stats for each runner.
    let mut runners_data = Vec::with_capacity(n);

    for runner in runners.iter() {
        let bonus = &runner.popularity_bonus;

        // Get coefficients.
        let mood_coef = motivation_multiplier(runner.initial_state.motivation);
        let raw = &runner.raw;
        let speed_distance_coef = distance_proficiency_multiplier(raw.distance_aptitude);
        let power_distance_coef =
            acceleration_distance_proficiency_multiplier(raw.distance_aptitude);
        let track_coef = ground_type_proficiency_multiplier(raw.track_aptitude);

        // Calculate adjusted stats for popularity from raw stats.
        // These are different from the adjusted stats used in a race simulation.
        let adjusted_speed = (raw.speed as f64 + bonus.speed) * speed_distance_coef * mood_coef;
        let adjusted_stamina = (raw.stamina as f64 + bonus.stamina) * mood_coef;
        let adjusted_power =
            (raw.power as f64 + bonus.power) * power_distance_coef * track_coef * mood_coef;

        runners_data.push(RunnerPopData {
            adjusted_speed,
            adjusted_stamina,
            adjusted_power,
            raw_guts: raw.guts as f64,
            raw_wiz: raw.wiz as f64,
        });
    }

    // Calculate judge scores.
    let judge_scores = calculate_judge_scores(&runners_data);

    // Assign marks for each judge.
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

    // Calculate total popularity points and update runners.
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

    // Sort by total points (descending) to determine ranks.
    popularity_data.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

    // Assign ranks (1-indexed, ties get same rank).
    let mut ranks = vec![0u8; n];
    let mut current_rank = 1u8;
    let mut prev_points = f64::MAX;

    for (runner_idx, points, judge_scores) in popularity_data {
        // If points are different from previous, update rank.
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
mod tests;
