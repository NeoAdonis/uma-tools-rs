use crate::units::meters_to_bashin;

/// Finish margin (着差) between a runner and the one that placed immediately
/// ahead, as displayed after a race.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Margin {
    Nose,
    Head,
    Neck,
    /// Margin in bashin, between a neck and 10 bashin.
    Bashin(f64),
    /// More than 10 bashin.
    Distance,
}

impl Margin {
    /// Convert a margin in meters to its displayed category.
    pub fn from_meters(meters: f64) -> Self {
        let bashin = meters_to_bashin(meters);
        if meters < 0.2 {
            Margin::Nose
        } else if meters < 0.4 {
            Margin::Head
        } else if meters < 0.8 {
            Margin::Neck
        } else if bashin > 10.0 {
            Margin::Distance
        } else {
            Margin::Bashin(bashin)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_from_meters() {
        assert_eq!(Margin::from_meters(0.0), Margin::Nose);
        assert_eq!(Margin::from_meters(0.19), Margin::Nose);
        assert_eq!(Margin::from_meters(0.2), Margin::Head);
        assert_eq!(Margin::from_meters(0.39), Margin::Head);
        assert_eq!(Margin::from_meters(0.4), Margin::Neck);
        assert_eq!(Margin::from_meters(0.79), Margin::Neck);
        assert_eq!(Margin::from_meters(0.8), Margin::Bashin(0.32));
        assert_eq!(Margin::from_meters(25.0), Margin::Bashin(10.0));
        assert_eq!(Margin::from_meters(25.1), Margin::Distance);
    }
}
