use super::super::variables::*;

#[test]
fn test_known_variables_parse() {
    // Test a sample of known variables
    assert!(matches!(
        VarType::parse("base_speed"),
        VarType::BaseStat(StatKind::Speed)
    ));
    assert!(matches!(
        VarType::parse("accumulatetime"),
        VarType::AccumulateTime
    ));
    assert!(matches!(
        VarType::parse("order"),
        VarType::Order(OrderVar::Order)
    ));
    assert!(matches!(
        VarType::parse("bashin_diff_behind"),
        VarType::Distance(DistanceVar::BashinDiffBehind)
    ));
    assert!(matches!(
        VarType::parse("corner_count"),
        VarType::Course(CourseVar::CornerCount)
    ));
    assert!(matches!(
        VarType::parse("corner"),
        VarType::Spatial(SpatialVar::Corner)
    ));
    assert!(matches!(
        VarType::parse("random_lot"),
        VarType::Random(RandomVar::RandomLot)
    ));
    assert!(matches!(VarType::parse("always"), VarType::Always));
}

#[test]
fn test_unknown_variable_fallback() {
    let result = VarType::parse("this_var_does_not_exist");
    assert!(matches!(result, VarType::Unknown(_)));
    if let VarType::Unknown(s) = result {
        assert_eq!(s, "this_var_does_not_exist");
    }
}

#[test]
fn test_display_formatting() {
    assert_eq!(
        format!("{}", VarType::BaseStat(StatKind::Speed)),
        "base_speed"
    );
    assert_eq!(format!("{}", VarType::Always), "always");
}
