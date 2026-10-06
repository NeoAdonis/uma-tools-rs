use super::*;
use crate::skill::{
    DEFAULT_COOLDOWN, PopularityStat as Stat, SkillAlternative, SkillData, SkillEffect,
    SkillRarity, SkillTarget, SkillType,
};

/// Create a skill with a single alternative with the given effects and no
/// popularity data.
fn skill(id: &str, rarity: SkillRarity, effects: &[(SkillType, SkillTarget, i32)]) -> Skill {
    Skill::new(
        id.to_string(),
        SkillData {
            rarity,
            alternatives: vec![SkillAlternative {
                base_duration: 30000,
                base_cooldown: DEFAULT_COOLDOWN,
                condition: "".to_string(),
                precondition: "".to_string(),
                effects: effects
                    .iter()
                    .map(|&(skill_type, target, modifier)| SkillEffect {
                        skill_type,
                        target,
                        modifier,
                    })
                    .collect(),
            }],
            popularity: None,
            wisdom_check: true,
        },
    )
}

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
        ("200961", points(&[(Stat::Speed, 30.0)])),
        ("200962", points(&[(Stat::Speed, 20.0)])),
        ("200011", points(&[(Stat::Speed, 45.0)])),
        ("200342", points(&[(Stat::Power, 20.0)])),
        ("200433", points(&[(Stat::Speed, -30.0)])),
        ("200851", points(&[(Stat::Speed, 10.0), (Stat::Wiz, 10.0)])),
        ("202521", points(&[(Stat::Power, 60.0)])),
        ("10071", points(&[(Stat::Speed, 30.0)])),
        ("900011", points(&[(Stat::Power, 15.0)])),
        ("900091", points(&[(Stat::Speed, 7.0), (Stat::Power, 7.0)])),
    ];
    for (id, expected) in cases {
        assert_eq!(skill_popularity(&skills[id]), expected, "skill {id}");
    }
}

#[test]
fn test_skill_popularity_reads_data() {
    use crate::skill::SkillPopularity as Entry;

    let base = skill(
        "1",
        SkillRarity::Gold,
        &[(SkillType::TargetSpeed, SkillTarget::Oneself, 3500)],
    );
    let with_popularity = |popularity| {
        Skill::new(
            base.id.clone(),
            SkillData {
                popularity,
                ..(*base).clone()
            },
        )
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
        let json = format!(r#"{{"rarity": 1, "alternatives": [], {popularity} "wisdomCheck": 1}}"#);
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
        Some(vec![(Stat::Speed, 10), (Stat::Wiz, 10)]),
    );
    assert_eq!(
        parse(r#""popularity": [{"stat": 2, "value": -30}],"#),
        Some(vec![(Stat::Stamina, -30)]),
    );
}

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
    // Judge scores: 1.5 + 0.8 - 0.3 = 2.0.
    // Mark points: 4 + 4 + 0 = 8.
    assert_eq!(scores.total_points(), 10.0);
}

#[test]
fn test_phi_calculation() {
    // Test normal case.
    let phi = calculate_phi(15.0, 10.0, 5.0);
    // 1 std dev above mean = 50 + 10 = 60.
    assert!((phi - 60.0).abs() < 0.001);

    // Test zero std dev (all values same).
    let phi = calculate_phi(10.0, 10.0, 0.0);
    // Mean value = 50.
    assert_eq!(phi, 50.0);
}

#[test]
fn test_calculate_stats() {
    let values = vec![10.0, 20.0, 30.0];
    let (mean, std_dev) = calculate_stats(&values);

    assert!((mean - 20.0).abs() < 0.001);
    assert!(std_dev > 0.0);
}
