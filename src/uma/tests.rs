//! Tests for the uma module

use super::*;

#[test]
fn test_motivation_multiplier() {
    // Normal = 0; multiplier should be 1.0
    assert_eq!(motivation_multiplier(Motivation::Normal), 1.0);

    // Great = +2; multiplier should be 1.04
    assert!((motivation_multiplier(Motivation::Great) - 1.04).abs() < 0.001);

    // Awful = -2; multiplier should be 0.96
    assert!((motivation_multiplier(Motivation::Awful) - 0.96).abs() < 0.001);
}
