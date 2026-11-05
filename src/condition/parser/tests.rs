use super::*;

/// Branches of a parsed condition, with each comparison as a string.
fn branches(input: &str) -> Vec<Vec<String>> {
    let condition = Condition::from_str(input).unwrap();
    condition
        .branches
        .iter()
        .map(|branch| branch.iter().map(Comparison::to_string).collect())
        .collect()
}

#[test]
fn test_and_binds_tighter_than_or() {
    assert_eq!(
        branches("order==1@phase==2&corner==3@grade==4"),
        [
            vec!["order == 1"],
            vec!["phase == 2", "corner == 3"],
            vec!["grade == 4"],
        ]
    );
}

#[test]
fn test_whitespace_around_names_operators_and_values_is_ignored() {
    assert_eq!(
        branches("  hp_per  ==  100  &order<= 3@  phase!=2  "),
        [vec!["hp_per == 100", "order <= 3"], vec!["phase != 2"]]
    );
}

#[test]
fn test_display() {
    // The skill data round-trip ignores spaces, so check them here.
    let condition = Condition::from_str("hp_per>50&order<3@corner!=0").unwrap();
    assert_eq!(
        condition.to_string(),
        "hp_per > 50 & order < 3 @ corner != 0"
    );
}

#[test]
fn test_error_cases() {
    use ParseError::*;

    let cases = [
        // Empty conditions, branches or comparisons.
        ("", ExpectedOperator("".into())),
        ("hp_per>50&", ExpectedOperator("".into())),
        ("hp_per>50@", ExpectedOperator("".into())),
        // Single '=' or '!' not allowed.
        ("hp_per=100", ExpectedOperator("hp_per=100".into())),
        ("hp_per!100", ExpectedOperator("hp_per!100".into())),
        ("hp per==1", UnknownVariable("hp per".into())),
        ("hp_per==", InvalidValue("hp_per==".into())),
        ("hp_per==1 2", InvalidValue("hp_per==1 2".into())),
        // Leftovers of the longest operator end up in the value.
        ("hp_per!==1", InvalidValue("hp_per!==1".into())),
        ("hp_per<=>1", InvalidValue("hp_per<=>1".into())),
    ];
    for (input, expected) in cases {
        assert_eq!(
            Condition::from_str(input).unwrap_err(),
            expected,
            "{input:?}"
        );
    }

    // Comparisons the evaluator can't evaluate; the error holds the input.
    let unsupported = [
        // Skill ID checks only support `==` and `!=`.
        "is_used_skill_id>=202051",
        "is_used_skill_id_with_detail_one<204452",
        "is_exist_skill_id>203061",
        // Buff type checks only support `==` and implemented skill types: 7
        // isn't one, and 283 and -1 don't even fit in a skill type ID.
        "is_other_character_activate_advantage_skill!=27",
        "is_other_character_activate_advantage_skill==7",
        "is_other_character_activate_advantage_skill==283",
        "is_other_character_activate_advantage_skill==-1",
        // Same, but -1 means any skill.
        "is_popularity_top_character_activate_advantage_skill!=-1",
        "is_popularity_top_character_activate_advantage_skill==-2",
        // Unimplemented variables are rejected with any comparison.
        "is_exist_chara_id==1001",
        "remain_distance_viewer_id<=850",
        // Random points only support `==`.
        "phase_random!=1",
        "corner_random>=2",
        // Lotteries only support `==` and percentages.
        "random_lot>=30",
        "random_lot==101",
        "random_lot_shared==-1",
        // Flags only support `==1`.
        "always==0",
        "always!=1",
        "all_corner_random==0",
    ];
    for input in unsupported {
        let expected = UnsupportedComparison(input.into());
        assert_eq!(
            Condition::from_str(input).unwrap_err(),
            expected,
            "{input:?}"
        );
    }
}

#[test]
fn test_error_display() {
    let cases = [
        ("hp_per=1", "expected comparison operator in 'hp_per=1'"),
        ("unknown==1", "unknown variable 'unknown'"),
        ("hp_per==x", "invalid value in 'hp_per==x'"),
        (
            "is_exist_skill_id<1",
            "unsupported comparison 'is_exist_skill_id<1'",
        ),
    ];
    for (input, expected) in cases {
        assert_eq!(
            Condition::from_str(input).unwrap_err().to_string(),
            expected,
            "{input:?}"
        );
    }
}

#[test]
fn test_all_skill_conditions() {
    // Every valid condition in the skill data displays back to itself (without
    // spaces), covering the forms of valid conditions. Unsupported ones are
    // reported by `test_skill_data_is_supported`.
    let json = std::fs::read_to_string("data/skill_data.json").unwrap();
    let skills: serde_json::Value = serde_json::from_str(&json).unwrap();
    let alternatives = skills
        .as_object()
        .unwrap()
        .values()
        .flat_map(|skill| skill["alternatives"].as_array().unwrap());
    for alternative in alternatives {
        for key in ["condition", "precondition"] {
            let input = alternative[key].as_str().unwrap();
            if let Ok(condition) = Condition::from_str(input) {
                assert_eq!(condition.to_string().replace(' ', ""), input);
            }
        }
    }
}
