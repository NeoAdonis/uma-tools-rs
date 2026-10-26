use super::*;
use crate::skill::{PopularityStat as Stat, SkillData, SkillType};
use crate::test_utils::{alternative, assert_close, skill_data};

/// Create popularity points with the given points added to each stat.
fn points(stats: &[(Stat, f64)]) -> SkillPopularity {
    stats
        .iter()
        .fold(SkillPopularity::default(), |p, &(stat, points)| {
            p.with(stat, points)
        })
}

#[test]
fn test_skill_popularity_from_data() {
    let skills =
        Skill::load_skills_from_file("data/skill_data.json").expect("Failed to load skill data");

    // Expected values are the ones given in-game.
    let cases = [
        (200961, points(&[(Stat::Speed, 30.0)])),
        (200962, points(&[(Stat::Speed, 20.0)])),
        (200011, points(&[(Stat::Speed, 45.0)])),
        (200342, points(&[(Stat::Power, 20.0)])),
        (200433, points(&[(Stat::Speed, -30.0)])),
        (200851, points(&[(Stat::Speed, 10.0), (Stat::Wit, 10.0)])),
        (202521, points(&[(Stat::Power, 60.0)])),
        (10071, points(&[(Stat::Speed, 30.0)])),
        (900011, points(&[(Stat::Power, 15.0)])),
        (900091, points(&[(Stat::Speed, 7.0), (Stat::Power, 7.0)])),
    ];
    for (id, expected) in cases {
        assert_eq!(skill_popularity(&skills[&id]), expected, "skill {id}");
    }
}

#[test]
fn test_skill_popularity_reads_data() {
    use crate::skill::SkillPopularity as Entry;

    let with_popularity = |popularity| {
        let data = SkillData {
            popularity,
            ..skill_data(vec![])
        };
        Skill::new(1, data)
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
        ])),
        points(&[(Stat::Stamina, 7.0), (Stat::Guts, 7.0)]),
    );
    assert_eq!(
        skill_popularity(&with_data(vec![entry(Stat::Speed, -30)])),
        points(&[(Stat::Speed, -30.0)]),
    );

    // No entries means no points; same for a missing field.
    assert_eq!(
        skill_popularity(&with_data(vec![])),
        SkillPopularity::default()
    );
    assert_eq!(
        skill_popularity(&with_data(vec![entry(Stat::None, 30)])),
        SkillPopularity::default()
    );
    assert_eq!(
        skill_popularity(&with_popularity(None)),
        SkillPopularity::default()
    );
}

#[test]
fn test_skill_popularity_deserialization() {
    let parse = |popularity: &str| {
        let json = format!(r#"{{"rarity": 1, "alternatives": [], {popularity} "witCheck": 1}}"#);
        serde_json::from_str::<SkillData>(&json)
            .expect("Failed to parse skill data")
            .popularity
            .map(|entries| {
                entries
                    .iter()
                    .map(|e| (e.stat, e.value))
                    .collect::<Vec<_>>()
            })
    };

    assert_eq!(parse(""), None);
    assert_eq!(parse(r#""popularity": [],"#), Some(vec![]));
    assert_eq!(
        parse(r#""popularity": [{"stat": 3, "value": 60}],"#),
        Some(vec![(Stat::Power, 60)]),
    );
    assert_eq!(
        parse(r#""popularity": [{"stat": 1, "value": 10}, {"stat": 5, "value": 10}],"#),
        Some(vec![(Stat::Speed, 10), (Stat::Wit, 10)]),
    );
    assert_eq!(
        parse(r#""popularity": [{"stat": 2, "value": -30}],"#),
        Some(vec![(Stat::Stamina, -30)]),
    );
}

/// Build a skill with one alternative per condition, giving 100 speed points.
fn popularity_skill(conditions: &[&str], wit_check: bool) -> Skill {
    use crate::skill::SkillPopularity as Entry;

    let alternatives = conditions
        .iter()
        .map(|condition| alternative(condition, SkillType::TargetSpeed, 0.35))
        .collect();
    let data = SkillData {
        popularity: Some(vec![Entry {
            stat: Stat::Speed,
            value: 100,
        }]),
        wit_check,
        ..skill_data(alternatives)
    };
    Skill::new(1, data)
}

/// Get the speed points a skill adds to a Pace runner with 1000 wit on a 2000m
/// (medium distance) course.
fn speed_bonus(skill: &Skill) -> f64 {
    use crate::race::solver::RaceSolver;
    use crate::test_utils::{load_course, race_params, seeded_config, uma};
    use crate::uma::{Mood, Style, Uma};

    let state = RaceSolver::new(load_course(10104), race_params(), &seeded_config(0)).state;
    let uma = Uma {
        speed: 1000,
        guts: 1000,
        ..uma(Style::Pace)
    };
    let runner = UnreadyRunner::new(uma, 1, Mood::Normal, &state, vec![]);
    skill_bonus([skill], &runner, &state).speed
}

#[test]
fn test_skill_bonus_popularity_guards() {
    let bonus = |conditions: &[&str]| speed_bonus(&popularity_skill(conditions, false));

    // No guards: always eligible.
    assert_eq!(bonus(&["always==1"]), 100.0);
    // Listed guards are evaluated.
    assert_eq!(bonus(&["distance_type==3"]), 100.0);
    assert_eq!(bonus(&["distance_type==1"]), 0.0);
    assert_eq!(bonus(&["running_style==2"]), 100.0);
    assert_eq!(bonus(&["running_style==4"]), 0.0);
    // Other variables are assumed to be satisfiable, even static ones.
    assert_eq!(bonus(&["course_distance==1200"]), 100.0);
    assert_eq!(bonus(&["motivation==5&base_speed>=1200"]), 100.0);
    assert_eq!(bonus(&["phase==3&hp_per<=30"]), 100.0);
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
    assert_eq!(judge_impact(50.0, Second, true), 50f64.powf(0.8) + 1.0);
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
