use crate::course::geometry::*;

/// Manages corner entry/exit detection
pub struct CornerManager {
    current_corner: Option<usize>,
}

impl CornerManager {
    pub fn new() -> Self {
        CornerManager {
            current_corner: None,
        }
    }

    /// Check for corner entry, returns corner index if entered
    pub fn check_entry(&mut self, position: f64, corners: &[Corner]) -> Option<usize> {
        if self.current_corner.is_some() {
            return None; // Already in a corner
        }

        for (idx, corner) in corners.iter().enumerate() {
            if position >= corner.start && position < corner.start + corner.length {
                self.current_corner = Some(idx);
                return Some(idx);
            }
        }
        None
    }

    /// Check for corner exit, returns corner index if exited
    pub fn check_exit(&mut self, position: f64, corners: &[Corner]) -> Option<usize> {
        if let Some(idx) = self.current_corner {
            let corner = &corners[idx];
            if position >= corner.start + corner.length {
                self.current_corner = None;
                return Some(idx);
            }
        }
        None
    }

    /// Get the current corner index if in a corner
    pub fn current_corner(&self) -> Option<usize> {
        self.current_corner
    }
}

impl Default for CornerManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Manages slope entry/exit detection
pub struct SlopeManager {
    current_slope: Option<usize>,
}

impl SlopeManager {
    pub fn new() -> Self {
        SlopeManager {
            current_slope: None,
        }
    }

    /// Check for slope entry, returns (slope index, slope type) if entered
    pub fn check_entry(&mut self, position: f64, slopes: &[Slope]) -> Option<(usize, SlopeType)> {
        if self.current_slope.is_some() {
            return None; // Already on a slope
        }

        for (idx, slope) in slopes.iter().enumerate() {
            if position >= slope.start && position < slope.start + slope.length {
                self.current_slope = Some(idx);
                let slope_type = slope.slope_type();
                return Some((idx, slope_type));
            }
        }
        None
    }

    /// Check for slope exit, returns slope index if exited
    pub fn check_exit(&mut self, position: f64, slopes: &[Slope]) -> Option<usize> {
        if let Some(idx) = self.current_slope {
            let slope = &slopes[idx];
            if position >= slope.start + slope.length {
                self.current_slope = None;
                return Some(idx);
            }
        }
        None
    }

    /// Get the current slope index if on a slope
    pub fn current_slope(&self) -> Option<usize> {
        self.current_slope
    }
}

impl Default for SlopeManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Manages straight entry/exit detection
pub struct StraightManager {
    current_straight: Option<usize>,
}

impl StraightManager {
    pub fn new() -> Self {
        StraightManager {
            current_straight: None,
        }
    }

    /// Check for straight entry, returns straight index if entered
    pub fn check_entry(&mut self, position: f64, straights: &[Straight]) -> Option<usize> {
        if self.current_straight.is_some() {
            return None; // Already in a straight
        }

        for (idx, straight) in straights.iter().enumerate() {
            if position >= straight.start && position < straight.end {
                self.current_straight = Some(idx);
                return Some(idx);
            }
        }
        None
    }

    /// Check for straight exit, returns straight index if exited
    pub fn check_exit(&mut self, position: f64, straights: &[Straight]) -> Option<usize> {
        if let Some(idx) = self.current_straight {
            let straight = &straights[idx];
            if position >= straight.end {
                self.current_straight = None;
                return Some(idx);
            }
        }
        None
    }

    /// Get the current straight index if in a straight
    pub fn current_straight(&self) -> Option<usize> {
        self.current_straight
    }
}

impl Default for StraightManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_corner(start: f64, length: f64) -> Corner {
        Corner { start, length }
    }

    fn create_test_slope(start: f64, length: f64, slope: i32) -> Slope {
        Slope::new(start, length, slope)
    }

    fn create_test_straight(start: f64, end: f64) -> Straight {
        Straight {
            start,
            end,
            front_type: FrontType::Audience,
        }
    }

    #[test]
    fn test_corner_entry() {
        let mut manager = CornerManager::new();
        let corners = vec![create_test_corner(100.0, 50.0)];

        // Before corner
        assert_eq!(manager.check_entry(50.0, &corners), None);

        // At corner start
        assert_eq!(manager.check_entry(100.0, &corners), Some(0));

        // Already in corner
        assert_eq!(manager.check_entry(125.0, &corners), None);
    }

    #[test]
    fn test_corner_exit() {
        let mut manager = CornerManager::new();
        let corners = vec![create_test_corner(100.0, 50.0)];

        // Enter corner
        manager.check_entry(100.0, &corners);

        // Still in corner
        assert_eq!(manager.check_exit(125.0, &corners), None);

        // At corner end
        assert_eq!(manager.check_exit(150.0, &corners), Some(0));

        // Already exited
        assert_eq!(manager.check_exit(175.0, &corners), None);
    }

    #[test]
    fn test_multiple_corners() {
        let mut manager = CornerManager::new();
        let corners = vec![
            create_test_corner(100.0, 50.0),
            create_test_corner(200.0, 30.0),
        ];

        // Enter first corner
        assert_eq!(manager.check_entry(100.0, &corners), Some(0));

        // Exit first corner
        assert_eq!(manager.check_exit(150.0, &corners), Some(0));

        // Enter second corner
        assert_eq!(manager.check_entry(200.0, &corners), Some(1));

        // Exit second corner
        assert_eq!(manager.check_exit(230.0, &corners), Some(1));
    }

    #[test]
    fn test_slope_entry_uphill() {
        let mut manager = SlopeManager::new();
        let slopes = vec![create_test_slope(100.0, 50.0, 500)]; // Positive = uphill

        // Before slope
        assert_eq!(manager.check_entry(50.0, &slopes), None);

        // At slope start
        let entry = manager.check_entry(100.0, &slopes);
        assert!(entry.is_some());
        let (idx, slope_type) = entry.unwrap();
        assert_eq!(idx, 0);
        assert_eq!(slope_type, SlopeType::Uphill);
    }

    #[test]
    fn test_slope_entry_downhill() {
        let mut manager = SlopeManager::new();
        let slopes = vec![create_test_slope(100.0, 50.0, -500)]; // Negative = downhill

        let entry = manager.check_entry(100.0, &slopes);
        assert!(entry.is_some());
        let (idx, slope_type) = entry.unwrap();
        assert_eq!(idx, 0);
        assert_eq!(slope_type, SlopeType::Downhill);
    }

    #[test]
    fn test_slope_exit() {
        let mut manager = SlopeManager::new();
        let slopes = vec![create_test_slope(100.0, 50.0, 500)];

        // Enter slope
        manager.check_entry(100.0, &slopes);

        // Still on slope
        assert_eq!(manager.check_exit(125.0, &slopes), None);

        // At slope end
        assert_eq!(manager.check_exit(150.0, &slopes), Some(0));

        // Already exited
        assert_eq!(manager.check_exit(175.0, &slopes), None);
    }

    #[test]
    fn test_straight_entry() {
        let mut manager = StraightManager::new();
        let straights = vec![create_test_straight(100.0, 150.0)];

        // Before straight
        assert_eq!(manager.check_entry(50.0, &straights), None);

        // At straight start
        assert_eq!(manager.check_entry(100.0, &straights), Some(0));

        // Already in straight
        assert_eq!(manager.check_entry(125.0, &straights), None);
    }

    #[test]
    fn test_straight_exit() {
        let mut manager = StraightManager::new();
        let straights = vec![create_test_straight(100.0, 150.0)];

        // Enter straight
        manager.check_entry(100.0, &straights);

        // Still in straight
        assert_eq!(manager.check_exit(125.0, &straights), None);

        // At straight end
        assert_eq!(manager.check_exit(150.0, &straights), Some(0));

        // Already exited
        assert_eq!(manager.check_exit(175.0, &straights), None);
    }

    #[test]
    fn test_multiple_straights() {
        let mut manager = StraightManager::new();
        let straights = vec![
            create_test_straight(100.0, 150.0),
            create_test_straight(200.0, 230.0),
        ];

        // Enter first straight
        assert_eq!(manager.check_entry(100.0, &straights), Some(0));

        // Exit first straight
        assert_eq!(manager.check_exit(150.0, &straights), Some(0));

        // Enter second straight
        assert_eq!(manager.check_entry(200.0, &straights), Some(1));

        // Exit second straight
        assert_eq!(manager.check_exit(230.0, &straights), Some(1));
    }
}
