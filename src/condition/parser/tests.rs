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
    assert!(parse_condition("x == 5").is_ok());
    assert!(parse_condition("x != 5").is_ok());
    assert!(parse_condition("x > 5").is_ok());
    assert!(parse_condition("x >= 5").is_ok());
    assert!(parse_condition("x < 5").is_ok());
    assert!(parse_condition("x <= 5").is_ok());
}

#[test]
fn test_negative_numbers() {
    let result = parse_condition("x > -10").unwrap();
    match result.comp_tree {
        ComparisonTree::Single(comp) => {
            assert_eq!(comp.value, -10);
        }
        _ => panic!("Expected Single"),
    }
}

#[test]
fn test_whitespace_handling() {
    let result1 = parse_condition("hp==100").unwrap();
    let result2 = parse_condition("  hp  ==  100  ").unwrap();
    let result3 = parse_condition("hp == 100").unwrap();

    assert_eq!(result1.comp_tree, result2.comp_tree);
    assert_eq!(result2.comp_tree, result3.comp_tree);
}

#[test]
fn test_variable_names() {
    assert!(parse_condition("simple == 1").is_ok());
    assert!(parse_condition("with_underscore == 1").is_ok());
    assert!(parse_condition("camelCase == 1").is_ok());
    assert!(parse_condition("PascalCase == 1").is_ok());
    assert!(parse_condition("var123 == 1").is_ok());
}

#[test]
fn test_display_trait() {
    let condition = parse_condition("hp_per > 50 & base_stamina >= 30").unwrap();
    let display = format!("{}", condition);
    assert!(display.contains("hp_per") || display.contains("hp"));
    assert!(display.contains("50"));
    assert!(display.contains("stamina"));
    assert!(display.contains("30"));
}

#[test]
fn test_error_cases() {
    assert!(parse_condition("").is_err());
    assert!(parse_condition("hp").is_err());
    assert!(parse_condition("hp ==").is_err());
    assert!(parse_condition("== 100").is_err());
    // Single '=' not allowed.
    assert!(parse_condition("hp = 100").is_err());
    // Incomplete.
    assert!(parse_condition("hp > 50 &").is_err());
}

#[test]
fn test_and_precedence_over_or() {
    // "a & b @ c & d" should parse as "(a & b) @ (c & d)".
    let result = parse_condition("a == 1 & b == 2 @ c == 3 & d == 4").unwrap();
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
    let result = parse_condition("a == 1 & b == 2 & c == 3").unwrap();
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
    let result = parse_condition("a == 1 @ b == 2 @ c == 3").unwrap();
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
    // Unknown variables should be parsed as VarType::Unknown.
    let result = parse_condition("this_var_does_not_exist == 42").unwrap();
    match result.comp_tree {
        ComparisonTree::Single(comp) => {
            assert!(matches!(comp.var, VarType::Unknown(_)));
            if let VarType::Unknown(s) = comp.var {
                assert_eq!(s, "this_var_does_not_exist");
            }
            assert_eq!(comp.value, 42);
        }
        _ => panic!("Expected Single"),
    }
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
