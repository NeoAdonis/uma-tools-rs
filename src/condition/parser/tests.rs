use super::*;
use crate::condition::variables::*;

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
fn test_simple_condition() {
    let condition = Condition::from_str("hp_per==100").unwrap();
    let [branch] = condition.branches.as_slice() else {
        panic!("expected a single branch: {condition:?}");
    };
    let [comp] = branch.as_slice() else {
        panic!("expected a single comparison: {condition:?}");
    };
    assert!(matches!(comp.var, VarType::Status(_)));
    assert_eq!(comp.op, CompOp::Equal);
    assert_eq!(comp.value, 100);
}

#[test]
fn test_branches() {
    let cases: [(&str, &[&[&str]]); 5] = [
        ("hp_per>50", &[&["hp_per > 50"]]),
        (
            "hp_per>50&base_stamina>=30",
            &[&["hp_per > 50", "base_stamina >= 30"]],
        ),
        ("order<3@order<=5", &[&["order < 3"], &["order <= 5"]]),
        // AND binds tighter than OR.
        (
            "order==1@phase==2&corner==3@grade==4",
            &[
                &["order == 1"],
                &["phase == 2", "corner == 3"],
                &["grade == 4"],
            ],
        ),
        (
            "hp_per==1&phase==2@corner==3&order==4",
            &[
                &["hp_per == 1", "phase == 2"],
                &["corner == 3", "order == 4"],
            ],
        ),
    ];
    for (input, expected) in cases {
        assert_eq!(branches(input), expected, "{input}");
    }
}

#[test]
fn test_all_comparison_operators() {
    for op in CompOp::ALL {
        let condition = Condition::from_str(&format!("hp_per{op}5")).unwrap();
        assert_eq!(condition.branches[0][0].op, op);
    }
}

#[test]
fn test_numbers() {
    for (input, expected) in [("hp_per>-10", -10), ("hp_per>0", 0), ("hp_per>+7", 7)] {
        assert_eq!(
            Condition::from_str(input).unwrap().branches[0][0].value,
            expected
        );
    }
}

#[test]
fn test_whitespace_handling() {
    let expected = Condition::from_str("hp_per==100&order<=3@phase!=2").unwrap();
    for input in [
        "hp_per == 100 & order <= 3 @ phase != 2",
        "  hp_per  ==  100  &order<= 3@  phase!=2  ",
    ] {
        assert_eq!(Condition::from_str(input).unwrap(), expected, "{input:?}");
    }
}

#[test]
fn test_variables_with_digits() {
    assert_eq!(
        branches("order_rate_in20_continue==1"),
        [["order_rate_in20_continue == 1"]]
    );
}

#[test]
fn test_display() {
    let cases = [
        (
            "hp_per>50&base_stamina>=30",
            "hp_per > 50 & base_stamina >= 30",
        ),
        ("order<3@order<=5", "order < 3 @ order <= 5"),
        ("corner!=0", "corner != 0"),
        (
            "hp_per==1&phase==2@corner<3",
            "hp_per == 1 & phase == 2 @ corner < 3",
        ),
    ];
    for (input, expected) in cases {
        let condition = Condition::from_str(input).unwrap();
        assert_eq!(condition.to_string(), expected, "{input}");
    }
}

#[test]
fn test_error_cases() {
    use ParseError::*;

    let cases = [
        ("", ExpectedOperator("".into())),
        ("hp_per", ExpectedOperator("hp_per".into())),
        // Empty branches or comparisons.
        ("hp_per>50&", ExpectedOperator("".into())),
        ("hp_per>50@", ExpectedOperator("".into())),
        ("@hp_per>50", ExpectedOperator("".into())),
        ("hp_per>50&&order==1", ExpectedOperator("".into())),
        // Single '=' or '!' not allowed.
        ("hp_per=100", ExpectedOperator("hp_per=100".into())),
        ("hp_per!100", ExpectedOperator("hp_per!100".into())),
        ("==100", UnknownVariable("".into())),
        (
            "this_var_does_not_exist==42",
            UnknownVariable("this_var_does_not_exist".into()),
        ),
        ("hp per==1", UnknownVariable("hp per".into())),
        ("hp_per==", InvalidValue("hp_per==".into())),
        ("hp_per==1 2", InvalidValue("hp_per==1 2".into())),
        ("hp_per==1#2", InvalidValue("hp_per==1#2".into())),
        ("hp_per!==1", InvalidValue("hp_per!==1".into())),
        ("hp_per<=>1", InvalidValue("hp_per<=>1".into())),
        ("hp_per==1.5", InvalidValue("hp_per==1.5".into())),
        ("hp_per==order", InvalidValue("hp_per==order".into())),
        // Out of `i32` range.
        (
            "hp_per==99999999999",
            InvalidValue("hp_per==99999999999".into()),
        ),
        // Skill ID checks only support `==` and `!=`.
        (
            "is_used_skill_id>=202051",
            UnsupportedComparison("is_used_skill_id>=202051".into()),
        ),
        (
            "is_used_skill_id_with_detail_one<204452",
            UnsupportedComparison("is_used_skill_id_with_detail_one<204452".into()),
        ),
        (
            "is_exist_skill_id>203061",
            UnsupportedComparison("is_exist_skill_id>203061".into()),
        ),
        // Buff type checks only support `==` and implemented skill types.
        (
            "is_other_character_activate_advantage_skill!=27",
            UnsupportedComparison("is_other_character_activate_advantage_skill!=27".into()),
        ),
        (
            "is_other_character_activate_advantage_skill==7",
            UnsupportedComparison("is_other_character_activate_advantage_skill==7".into()),
        ),
        (
            "is_other_character_activate_advantage_skill==-1",
            UnsupportedComparison("is_other_character_activate_advantage_skill==-1".into()),
        ),
        (
            "is_other_character_activate_advantage_skill==283",
            UnsupportedComparison("is_other_character_activate_advantage_skill==283".into()),
        ),
        (
            "is_popularity_top_character_activate_advantage_skill!=-1",
            UnsupportedComparison(
                "is_popularity_top_character_activate_advantage_skill!=-1".into(),
            ),
        ),
        (
            "is_popularity_top_character_activate_advantage_skill==-2",
            UnsupportedComparison(
                "is_popularity_top_character_activate_advantage_skill==-2".into(),
            ),
        ),
        // Unimplemented variables are rejected with any comparison.
        (
            "is_exist_chara_id==1001",
            UnsupportedComparison("is_exist_chara_id==1001".into()),
        ),
        (
            "remain_distance_viewer_id<=850",
            UnsupportedComparison("remain_distance_viewer_id<=850".into()),
        ),
        // Random points only support `==`.
        (
            "phase_random!=1",
            UnsupportedComparison("phase_random!=1".into()),
        ),
        (
            "corner_random>=2",
            UnsupportedComparison("corner_random>=2".into()),
        ),
        (
            "all_corner_random==0",
            UnsupportedComparison("all_corner_random==0".into()),
        ),
        // Lotteries only support `==` and percentages.
        (
            "random_lot>=30",
            UnsupportedComparison("random_lot>=30".into()),
        ),
        (
            "random_lot==101",
            UnsupportedComparison("random_lot==101".into()),
        ),
        (
            "random_lot_shared!=30",
            UnsupportedComparison("random_lot_shared!=30".into()),
        ),
        (
            "random_lot_shared==-1",
            UnsupportedComparison("random_lot_shared==-1".into()),
        ),
        // `always` only supports `==1`.
        ("always==0", UnsupportedComparison("always==0".into())),
        ("always!=1", UnsupportedComparison("always!=1".into())),
        ("always>=1", UnsupportedComparison("always>=1".into())),
    ];
    for (input, expected) in cases {
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
            expected
        );
    }
}

#[test]
fn test_known_variables() {
    let var = |input: &str| {
        Condition::from_str(input).unwrap().branches[0][0]
            .var
            .clone()
    };
    assert!(matches!(var("always==1"), VarType::Always));
    assert!(matches!(var("random_lot==30"), VarType::Random(_)));
}

#[test]
fn test_all_skill_conditions() {
    // Every condition in the skill data displays back to itself (without spaces).
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
            if input.is_empty() {
                continue;
            }
            let condition = Condition::from_str(input).unwrap_or_else(|e| panic!("{input}: {e}"));
            assert_eq!(condition.to_string().replace(' ', ""), input);
        }
    }
}
