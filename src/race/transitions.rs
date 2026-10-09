use crate::course::geometry::*;

/// Course segment (corner, slope or straight) spanning from `start()`
/// (inclusive) to `end()` (exclusive), in meters.
pub trait Segment {
    fn start(&self) -> f64;
    fn end(&self) -> f64;
}

impl Segment for Corner {
    fn start(&self) -> f64 {
        self.start
    }

    fn end(&self) -> f64 {
        self.start + self.length
    }
}

impl Segment for Slope {
    fn start(&self) -> f64 {
        self.start
    }

    fn end(&self) -> f64 {
        self.start + self.length
    }
}

impl Segment for Straight {
    fn start(&self) -> f64 {
        self.start
    }

    fn end(&self) -> f64 {
        self.end
    }
}

/// Structure used for segment entry/exit operations, tracking which segment of
/// a kind (e.g. corners) a runner is in.
#[derive(Default)]
pub struct SegmentManager {
    current: Option<usize>,
}

impl SegmentManager {
    /// Check for segment entry, returns segment index if entered.
    pub fn check_entry<S: Segment>(&mut self, position: f64, segments: &[S]) -> Option<usize> {
        if self.current.is_some() {
            // Already in a segment.
            return None;
        }

        let idx = segments
            .iter()
            .position(|segment| position >= segment.start() && position < segment.end())?;
        self.current = Some(idx);
        Some(idx)
    }

    /// Check for segment exit, returns segment index if exited.
    pub fn check_exit<S: Segment>(&mut self, position: f64, segments: &[S]) -> Option<usize> {
        let idx = self.current?;
        if position >= segments[idx].end() {
            self.current = None;
            return Some(idx);
        }
        None
    }

    /// Get the current segment index if in a segment.
    pub fn current(&self) -> Option<usize> {
        self.current
    }
}

#[cfg(test)]
mod tests;
