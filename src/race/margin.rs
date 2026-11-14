//! Finish margins between runners, as displayed after a race.
//!
//! The solver records each runner's margin in meters (see
//! [`Runner::finish_margin`](crate::race::runner::Runner::finish_margin));
//! [`Margin`] converts it to the category shown in the race results.

use crate::units::meters_to_bashin;

/// Finish margin between a runner and the one that placed immediately
/// ahead, as displayed after a race.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Margin {
    /// Less than 0.2 meters.
    Nose,
    /// Between 0.2 and 0.4 meters.
    Head,
    /// Between 0.4 and 0.8 meters.
    Neck,
    /// Margin in bashin, between a neck and 10 bashin.
    Bashin(f64),
    /// More than 10 bashin.
    Distance,
}

impl Margin {
    /// Converts a margin in meters to its displayed category.
    ///
    /// # Examples
    ///
    /// Show the margins recorded by the solver as they appear in the race
    /// results:
    ///
    /// ```
    /// use uma_tools::race::margin::Margin;
    ///
    /// assert_eq!(Margin::from_meters(0.3), Margin::Head);
    /// assert_eq!(Margin::from_meters(3.75), Margin::Bashin(1.5));
    /// assert_eq!(Margin::from_meters(30.0), Margin::Distance);
    /// ```
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
        let cases = [
            (0.19, Margin::Nose),
            (0.2, Margin::Head),
            (0.39, Margin::Head),
            (0.4, Margin::Neck),
            (0.79, Margin::Neck),
            (0.8, Margin::Bashin(0.32)),
            (25.0, Margin::Bashin(10.0)),
            (25.1, Margin::Distance),
        ];
        for (meters, expected) in cases {
            assert_eq!(Margin::from_meters(meters), expected, "{meters}m");
        }
    }
}
