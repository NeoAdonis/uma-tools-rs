use super::*;
use crate::condition::variables::*;

#[test]
fn test_simple_condition() {
    let result = parse_condition("hp_per == 100").unwrap();
    match result.comp_tree {
        ComparisonTree::Single(comp) => {
            assert!(matches!(comp.var, VarType::Status(_)));
            assert_eq!(comp.op, CompOp::Equal);
            assert_eq!(comp.value, 100);
        }
        _ => panic!("Expected Single"),
    }
}

#[test]
fn test_binary_and() {
    let result = parse_condition("hp_per > 50 & base_stamina >= 30").unwrap();
    match result.comp_tree {
        ComparisonTree::Binary { left, op, right } => {
            // Left should be a Single comparison.
            match *left {
                ComparisonTree::Single(ref comp) => {
                    assert!(matches!(comp.var, VarType::Status(_)));
                    assert_eq!(comp.op, CompOp::Greater);
                    assert_eq!(comp.value, 50);
                }
                _ => panic!("Expected Single on left"),
            }
            assert_eq!(op, BinOp::And);
            // Right should be a Single comparison.
            match *right {
                ComparisonTree::Single(ref comp) => {
                    assert!(matches!(comp.var, VarType::BaseStat(_)));
                    assert_eq!(comp.op, CompOp::GreaterEqual);
                    assert_eq!(comp.value, 30);
                }
                _ => panic!("Expected Single on right"),
            }
        }
        _ => panic!("Expected Binary"),
    }
}

#[test]
fn test_multiple_conditions() {
    // With new precedence: "order == 1 & phase != 2 @ corner < 3" parses as
    // two OR branches: "order == 1 & phase != 2" and "corner < 3".
    let result = parse_condition("order == 1 & phase != 2 @ corner < 3").unwrap();
    match result.comp_tree {
        ComparisonTree::Binary { left, op, right } => {
            // Top level is OR.
            assert_eq!(op, BinOp::Or);
            // Left side should be "order == 1 & phase != 2".
            match *left {
                ComparisonTree::Binary {
                    left: ref inner_left,
                    op: inner_op,
                    right: ref inner_right,
                } => {
                    assert_eq!(inner_op, BinOp::And);
                    match **inner_left {
                        ComparisonTree::Single(ref comp) => {
                            assert!(matches!(comp.var, VarType::Order(_)));
                            assert_eq!(comp.value, 1);
                        }
                        _ => panic!("Expected Single"),
                    }
                    match **inner_right {
                        ComparisonTree::Single(ref comp) => {
                            assert!(matches!(comp.var, VarType::Phase(_)));
                            assert_eq!(comp.value, 2);
                        }
                        _ => panic!("Expected Single"),
                    }
                }
                _ => panic!("Expected Binary on left"),
            }
            // Right side should be "corner < 3".
            match *right {
                ComparisonTree::Single(ref comp) => {
                    assert!(matches!(comp.var, VarType::Spatial(_)));
                    assert_eq!(comp.value, 3);
                }
                _ => panic!("Expected Single on right"),
            }
        }
        _ => panic!("Expected Binary"),
    }
}

#[test]
fn test_all_comparison_operators() {
    assert!(parse_condition("hp_per == 5").is_ok());
    assert!(parse_condition("hp_per != 5").is_ok());
    assert!(parse_condition("hp_per > 5").is_ok());
    assert!(parse_condition("hp_per >= 5").is_ok());
    assert!(parse_condition("hp_per < 5").is_ok());
    assert!(parse_condition("hp_per <= 5").is_ok());
}

#[test]
fn test_negative_numbers() {
    let result = parse_condition("hp_per > -10").unwrap();
    match result.comp_tree {
        ComparisonTree::Single(comp) => {
            assert_eq!(comp.value, -10);
        }
        _ => panic!("Expected Single"),
    }
}

#[test]
fn test_whitespace_handling() {
    let result1 = parse_condition("hp_per==100").unwrap();
    let result2 = parse_condition("  hp_per  ==  100  ").unwrap();
    let result3 = parse_condition("hp_per == 100").unwrap();

    assert_eq!(result1.comp_tree, result2.comp_tree);
    assert_eq!(result2.comp_tree, result3.comp_tree);
}

#[test]
fn test_variable_names() {
    // Any identifier is lexed as a variable name, then rejected if unknown.
    for name in [
        "simple",
        "with_underscore",
        "_leading_underscore",
        "camelCase",
        "PascalCase",
        "var123",
    ] {
        let err = parse_condition(&format!("{name} == 1")).unwrap_err();
        assert_eq!(err.to_string(), format!("Unknown variable '{name}'"));
    }
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
        // Nested expressions are parenthesized.
        (
            "hp_per==1&phase==2@corner<3",
            "(hp_per == 1 & phase == 2) @ (corner < 3)",
        ),
    ];
    for (input, expected) in cases {
        let condition = parse_condition(input).unwrap();
        assert_eq!(condition.to_string(), expected, "{input}");
    }
}

#[test]
fn test_error_cases() {
    let cases = [
        ("", "Expected variable"),
        ("== 100", "Expected variable"),
        ("hp_per", "Expected comparison operator"),
        ("hp_per ==", "Expected value"),
        // Incomplete.
        ("hp_per > 50 &", "Expected variable"),
        ("hp_per > 50 @", "Expected variable"),
        ("hp_per==1 2", "Unexpected token 'Value(2)' at position 3"),
        // Single '=' or '!' not allowed.
        ("hp_per = 100", "Unexpected character '=' at position 7"),
        ("hp_per !100", "Unexpected character '!' at position 7"),
        ("hp_per =", "Unexpected character '=' at position 7"),
        ("hp_per==1 # 2", "Unexpected character '#' at position 10"),
        // Out of `i32` range.
        (
            "hp_per==99999999999",
            "Invalid number '99999999999' at position 8",
        ),
        (
            "hp_per==-99999999999",
            "Invalid number '-99999999999' at position 8",
        ),
    ];
    for (input, expected) in cases {
        let error = parse_condition(input).unwrap_err();
        assert_eq!(error.to_string(), expected, "{input:?}");
    }
}

#[test]
fn test_comp_op_apply() {
    // Results for values below, equal to and above 5.
    let cases = [
        (CompOp::Equal, [false, true, false]),
        (CompOp::NotEqual, [true, false, true]),
        (CompOp::Greater, [false, false, true]),
        (CompOp::GreaterEqual, [false, true, true]),
        (CompOp::Less, [true, false, false]),
        (CompOp::LessEqual, [true, true, false]),
    ];
    for (op, expected) in cases {
        let actual = [4, 5, 6].map(|lhs| op.apply(lhs, 5));
        assert_eq!(actual, expected, "{op}");
    }
}

#[test]
fn test_and_precedence_over_or() {
    // "a & b @ c & d" should parse as "(a & b) @ (c & d)".
    let result = parse_condition("hp_per == 1 & phase == 2 @ corner == 3 & order == 4").unwrap();
    match result.comp_tree {
        ComparisonTree::Binary { left, op, right } => {
            // Top level is OR.
            assert_eq!(op, BinOp::Or);
            // Both sides should be AND expressions.
            assert!(matches!(
                *left,
                ComparisonTree::Binary { op: BinOp::And, .. }
            ));
            assert!(matches!(
                *right,
                ComparisonTree::Binary { op: BinOp::And, .. }
            ));
        }
        _ => panic!("Expected Binary with OR at top level"),
    }
}

#[test]
fn test_left_associativity_and() {
    // "a & b & c" should parse as "(a & b) & c".
    let result = parse_condition("hp_per == 1 & phase == 2 & corner == 3").unwrap();
    match result.comp_tree {
        ComparisonTree::Binary { left, op, right } => {
            assert_eq!(op, BinOp::And);
            // Left should be another AND expression.
            assert!(matches!(
                *left,
                ComparisonTree::Binary { op: BinOp::And, .. }
            ));
            // Right should be a single comparison.
            assert!(matches!(*right, ComparisonTree::Single(_)));
        }
        _ => panic!("Expected Binary"),
    }
}

#[test]
fn test_left_associativity_or() {
    // "a @ b @ c" should parse as "(a @ b) @ c".
    let result = parse_condition("hp_per == 1 @ phase == 2 @ corner == 3").unwrap();
    match result.comp_tree {
        ComparisonTree::Binary { left, op, right } => {
            assert_eq!(op, BinOp::Or);
            // Left should be another OR expression.
            assert!(matches!(
                *left,
                ComparisonTree::Binary { op: BinOp::Or, .. }
            ));
            // Right should be a single comparison.
            assert!(matches!(*right, ComparisonTree::Single(_)));
        }
        _ => panic!("Expected Binary"),
    }
}

#[test]
fn test_complex_precedence() {
    // "a @ b & c @ d" should parse as "a @ (b & c) @ d".
    let result = parse_condition("order == 1 @ phase == 2 & corner == 3 @ grade == 4").unwrap();
    match result.comp_tree {
        ComparisonTree::Binary {
            left: left1,
            op: op1,
            right: right1,
        } => {
            assert_eq!(op1, BinOp::Or);
            // Left should be another OR expression.
            match *left1 {
                ComparisonTree::Binary {
                    left: ref left2,
                    op: op2,
                    right: ref right2,
                } => {
                    assert_eq!(op2, BinOp::Or);
                    // `left2` should be 'order == 1'.
                    assert!(matches!(**left2, ComparisonTree::Single(_)));
                    // `right2` should be 'phase == 2 & corner == 3'.
                    assert!(matches!(
                        **right2,
                        ComparisonTree::Binary { op: BinOp::And, .. }
                    ));
                }
                _ => panic!("Expected nested OR on left"),
            }
            // Right should be 'grade == 4'.
            assert!(matches!(*right1, ComparisonTree::Single(_)));
        }
        _ => panic!("Expected Binary"),
    }
}

#[test]
fn test_unknown_variable() {
    let err = parse_condition("this_var_does_not_exist == 42").unwrap_err();
    assert_eq!(
        err.to_string(),
        "Unknown variable 'this_var_does_not_exist'"
    );
}

#[test]
fn test_known_variables() {
    // Test a few known variables to ensure they parse correctly.
    let result = parse_condition("always == 1").unwrap();
    match result.comp_tree {
        ComparisonTree::Single(comp) => {
            assert!(matches!(comp.var, VarType::Always));
        }
        _ => panic!("Expected Single"),
    }

    let result = parse_condition("random_lot == 30").unwrap();
    match result.comp_tree {
        ComparisonTree::Single(comp) => {
            assert!(matches!(comp.var, VarType::Random(_)));
        }
        _ => panic!("Expected Single"),
    }
}
