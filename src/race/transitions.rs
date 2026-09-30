use crate::course::geometry::*;

/// Structure used for corner entry/exit operations.
pub struct CornerManager {
    current_corner: Option<usize>,
}

impl CornerManager {
    pub fn new() -> Self {
        CornerManager {
            current_corner: None,
        }
    }

    /// Check for corner entry, returns corner index if entered.
    pub fn check_entry(&mut self, position: f64, corners: &[Corner]) -> Option<usize> {
        if self.current_corner.is_some() {
            // Already in a corner.
            return None;
        }

        for (idx, corner) in corners.iter().enumerate() {
            if position >= corner.start && position < corner.start + corner.length {
                self.current_corner = Some(idx);
                return Some(idx);
            }
        }
        None
    }

    /// Check for corner exit, returns corner index if exited.
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

    /// Get the current corner index if in a corner.
    pub fn current_corner(&self) -> Option<usize> {
        self.current_corner
    }
}

impl Default for CornerManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Structure used for slope entry/exit operations.
pub struct SlopeManager {
    current_slope: Option<usize>,
}

impl SlopeManager {
    pub fn new() -> Self {
        SlopeManager {
            current_slope: None,
        }
    }

    /// Check for slope entry, returns (slope index, slope type) if entered.
    pub fn check_entry(&mut self, position: f64, slopes: &[Slope]) -> Option<(usize, SlopeType)> {
        if self.current_slope.is_some() {
            // Already on a slope.
            return None;
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

    /// Check for slope exit, returns slope index if exited.
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

    /// Get the current slope index if on a slope.
    pub fn current_slope(&self) -> Option<usize> {
        self.current_slope
    }
}

impl Default for SlopeManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Structure used for straight entry/exit operations.
pub struct StraightManager {
    current_straight: Option<usize>,
}

impl StraightManager {
    pub fn new() -> Self {
        StraightManager {
            current_straight: None,
        }
    }

    /// Check for straight entry, returns straight index if entered.
    pub fn check_entry(&mut self, position: f64, straights: &[Straight]) -> Option<usize> {
        if self.current_straight.is_some() {
            // Already in a straight.
            return None;
        }

        for (idx, straight) in straights.iter().enumerate() {
            if position >= straight.start && position < straight.end {
                self.current_straight = Some(idx);
                return Some(idx);
            }
        }
        None
    }

    /// Check for straight exit, returns straight index if exited.
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

    /// Get the current straight index if in a straight.
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
mod tests;
