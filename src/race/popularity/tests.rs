use super::*;
use crate::race::pins::Pins;
use crate::race::rng::RngSeed;
use crate::skill::{PopularityStat as Stat, Skill, SkillType};
use crate::test_utils::{alternative, assert_close, synthetic_skill};
use std::sync::Arc;

/// Create popularity points with the given points added to each stat.
fn points(stats: &[(Stat, f64)]) -> SkillPopularity {
    stats
        .iter()
        .fold(SkillPopularity::default(), |p, &(stat, points)| {
            p.with(stat, points)
        })
}

#[test]
fn test_skill_popularity_reads_data() {
    use crate::skill::SkillPopularity as Entry;

    let with_popularity = |popularity| {
        Arc::new(Skill {
            id: 1,
            popularity,
            ..synthetic_skill(vec![])
        })
    };
    let with_data = |entries: Vec<Entry>| with_popularity(Some(entries));
    let entry = |stat, value| Entry { stat, value };

    assert_eq!(
        skill_popularity(&with_data(vec![entry(Stat::Power, 40)])),
        points(&[(Stat::Power, 40.0)]),
    );
    assert_eq!(
        skill_popularity(&with_data(vec![
            entry(Stat::Stamina, 7),
            entry(Stat::Guts, 7),
            entry(Stat::Wit, 7),
        ])),
        points(&[(Stat::Stamina, 7.0), (Stat::Guts, 7.0), (Stat::Wit, 7.0)]),
    );

    // No points for no stat, nor without popularity data.
    assert_eq!(
        skill_popularity(&with_data(vec![entry(Stat::None, 30)])),
        SkillPopularity::default()
    );
    assert_eq!(
        skill_popularity(&with_popularity(None)),
        SkillPopularity::default()
    );
}

/// Build a skill with one alternative per condition, giving 100 speed points.
fn popularity_skill(conditions: &[&str], wit_check: bool) -> Arc<Skill> {
    use crate::skill::SkillPopularity as Entry;

    let alternatives = conditions
        .iter()
        .map(|condition| alternative(condition, SkillType::TargetSpeed, 0.35))
        .collect();
    Arc::new(Skill {
        id: 1,
        popularity: Some(vec![Entry {
            stat: Stat::Speed,
            value: 100,
        }]),
        wit_check,
        ..synthetic_skill(alternatives)
    })
}

/// Get the speed points a skill adds to a Pace runner with 1000 wit on a 2000m
/// (medium distance) course.
fn speed_bonus(skill: &Skill) -> f64 {
    use crate::race::solver::RaceSolver;
    use crate::test_utils::{load_course, race_params, seeded_config, uma};
    use crate::uma::{BaseStyle, Mood, Uma};

    let state = RaceSolver::new(load_course(10104), race_params(), seeded_config(0)).state;
    let uma = Uma {
        speed: 1000,
        guts: 1000,
        ..uma(BaseStyle::Pace)
    };
    let mut runner = UnreadyRunner::new(
        uma,
        Mood::Normal,
        &state,
        vec![],
        Pins::default(),
        RngSeed::new(0),
    );
    runner.assign_gate(1, &state);
    skill_bonus([skill], &runner, &state).speed
}

#[test]
fn test_skill_bonus_popularity_guards() {
    let bonus = |conditions: &[&str]| speed_bonus(&popularity_skill(conditions, false));

    // Guards are evaluated.
    assert_eq!(bonus(&["distance_type==3"]), 100.0);
    assert_eq!(bonus(&["distance_type==1"]), 0.0);
    // Other variables are assumed to be satisfiable, even static ones.
    assert_eq!(bonus(&["course_distance==1200"]), 100.0);
    // Mixed with other variables in AND/OR.
    assert_eq!(bonus(&["distance_type==1@hp_per<=30"]), 100.0);
    assert_eq!(bonus(&["distance_type==1&hp_per<=30"]), 0.0);
    // Eligible if any alternative is eligible.
    assert_eq!(bonus(&["distance_type==1", "distance_type==3"]), 100.0);
    assert_eq!(bonus(&["distance_type==1", "distance_type==2"]), 0.0);
}

#[test]
fn test_skill_bonus_activation_chance() {
    // With 1000 raw wit, the activation chance is 91%.
    let bonus = speed_bonus(&popularity_skill(&["always==1"], true));
    assert_close!(bonus, 91.0);
}

#[test]
fn test_judge_impact() {
    use PopularityMark::*;

    // Above 55, the score is used as-is.
    assert_eq!(judge_impact(60.0, First, false), 62.0);
    assert_eq!(judge_impact(60.0, None, true), 60.0);
    // At or below 55 with the high flag, the exponent is 0.8.
    assert_eq!(judge_impact(55.0, None, true), 55f64.powf(0.8));
    // Otherwise, the exponent is the score minus 54.
    assert_eq!(judge_impact(55.0, Third, false), 55.5);
    assert_eq!(judge_impact(54.5, None, false), 54.5f64.sqrt());
    assert_eq!(judge_impact(54.0, Fourth, false), 1.2);
}

#[test]
fn test_judge_scores_total() {
    let scores = |left_score, center_score, right_score| JudgeScores {
        left_score,
        left_mark: PopularityMark::First,
        center_score,
        center_mark: PopularityMark::Second,
        right_score,
        right_mark: PopularityMark::None,
    };

    // No score above 70: no high flag.
    let total = scores(60.0, 54.5, 56.0).total_points();
    assert_eq!(total, 62.0 * (54.5f64.sqrt() + 1.0) * 56.0);

    // A score above 70 sets the high flag for all judges.
    let total = scores(60.0, 54.5, 75.0).total_points();
    assert_eq!(total, 62.0 * (54.5f64.powf(0.8) + 1.0) * 75.0);
}

/// Get the popularity ranks of runners with the given umas, in order.
fn popularity_ranks(umas: &[crate::uma::Uma]) -> Vec<u8> {
    use crate::race::solver::RaceSolver;
    use crate::test_utils::{load_course, race_params, seeded_config};
    use crate::uma::Mood;

    let state = RaceSolver::new(load_course(10104), race_params(), seeded_config(0)).state;
    let runners: Vec<_> = umas
        .iter()
        .enumerate()
        .map(|(i, &uma)| {
            let mut runner = UnreadyRunner::new(
                uma,
                Mood::Normal,
                &state,
                vec![],
                Pins::default(),
                RngSeed::new(0),
            );
            runner.assign_gate(i as u8 + 1, &state);
            runner
        })
        .collect();
    calculate_popularity(&runners)
        .into_iter()
        .map(|(rank, _)| rank)
        .collect()
}

#[test]
fn test_popularity_ranks_weak_runners() {
    use crate::test_utils::uma;
    use crate::uma::{BaseStyle, Uma};

    // Stats decrease with each runner, so the weakest ones get judge scores
    // well below 54 and tiny (but distinct) points.
    let umas: Vec<Uma> = (0..8)
        .map(|i| Uma {
            speed: 1200 - 100 * i,
            stamina: 1000 - 100 * i,
            power: 1000 - 100 * i,
            ..uma(BaseStyle::Pace)
        })
        .collect();
    assert_eq!(popularity_ranks(&umas), [1, 2, 3, 4, 5, 6, 7, 8]);
}

#[test]
fn test_popularity_ranks_ties() {
    use crate::test_utils::uma;
    use crate::uma::{BaseStyle, Uma};

    // Identical runners with the same marks share a rank, also when their
    // points are tiny, and the next rank skips the tied places. Marks follow
    // gate order between identical runners, so the strong ones don't tie
    // except for the two △, and the weak ones get no marks.
    let strong = uma(BaseStyle::Pace);
    let weak = Uma {
        speed: 300,
        stamina: 300,
        power: 300,
        ..strong
    };
    let ranks = popularity_ranks(&[strong, strong, strong, strong, strong, weak, weak, weak]);
    assert_eq!(ranks, [1, 2, 3, 4, 4, 6, 6, 6]);
}

#[test]
fn test_phi_calculation() {
    // Test normal case.
    let phi = calculate_phi(15.0, 10.0, 5.0);
    // 1 std dev above mean = 50 + 10 = 60.
    assert_close!(phi, 60.0, tolerance = 0.001);

    // Test zero std dev (all values same).
    let phi = calculate_phi(10.0, 10.0, 0.0);
    // Mean value = 50.
    assert_eq!(phi, 50.0);
}

#[test]
fn test_calculate_stats() {
    let values = vec![10.0, 20.0, 30.0];
    let (mean, std_dev) = calculate_stats(&values);

    assert_close!(mean, 20.0);
    // Population standard deviation: sqrt(200 / 3).
    assert_close!(std_dev, (200.0_f64 / 3.0).sqrt());

    // No values (e.g. no runners).
    assert_eq!(calculate_stats(&[]), (0.0, 0.0));
}
