//! Tracking of the course segments (corners, straights and slopes) a runner is
//! in.
//!
//! Each frame, the runner's new position is checked against the course
//! segments of each kind, reporting the ones exited and entered. These
//! transitions drive slope effects (see [`Runner::update_slope`]) and skills
//! triggered by entering the last straight, while the current segments are
//! read by condition variables such as `corner`, `slope` and
//! `straight_front_type`.
//!
//! [`Runner::update_slope`]: crate::race::runner::Runner::update_slope

use crate::course::Course;
use crate::course::geometry::Segment;

/// Segment indices exited and entered during a [`SegmentManager::update`].
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash, Copy)]
pub(crate) struct Transition {
    pub(crate) exited: Option<usize>,
    pub(crate) entered: Option<usize>,
}

/// Structure used for segment entry/exit operations, tracking which segment of
/// a kind (e.g. corners) a runner is in.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct SegmentManager {
    current: Option<usize>,
}

impl SegmentManager {
    /// Update the current segment for `position`, returning any transition.
    ///
    /// The current segment is exited before looking for a new one, so that a
    /// segment right after another one is entered in the same update.
    pub(crate) fn update<S: Segment>(&mut self, position: f64, segments: &[S]) -> Transition {
        let exited = self.current.filter(|&idx| position >= segments[idx].end());
        if exited.is_some() {
            self.current = None;
        }

        let entered = match self.current {
            Some(_) => None,
            None => segments
                .iter()
                .position(|segment| segment.contains(position)),
        };
        if entered.is_some() {
            self.current = entered;
        }

        Transition { exited, entered }
    }

    /// Get the current segment index if in a segment.
    pub(crate) fn current(&self) -> Option<usize> {
        self.current
    }
}

/// Transitions of each kind of segment during a [`SegmentState::update`].
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash, Copy)]
pub(crate) struct Transitions {
    pub(crate) corner: Transition,
    pub(crate) straight: Transition,
    pub(crate) slope: Transition,
}

/// Course segments a runner is currently in.
#[derive(Debug, Default, Clone, PartialEq)]
pub(crate) struct SegmentState {
    corner: SegmentManager,
    straight: SegmentManager,
    slope: SegmentManager,
    /// Straight entered since it was last taken, if any.
    ///
    /// Skills triggered by entering a straight are checked on the next skill
    /// check, which may not happen on the same frame.
    entered_straight: Option<usize>,
}

impl SegmentState {
    /// Update the current segments for `position`, returning the transitions.
    pub(crate) fn update(&mut self, position: f64, course: &Course) -> Transitions {
        let transitions = Transitions {
            corner: self.corner.update(position, &course.corners),
            straight: self.straight.update(position, &course.straights),
            slope: self.slope.update(position, &course.slopes),
        };
        if transitions.straight.entered.is_some() {
            self.entered_straight = transitions.straight.entered;
        }
        transitions
    }

    /// Get the current corner index if in a corner.
    pub(crate) fn corner(&self) -> Option<usize> {
        self.corner.current()
    }

    /// Get the current straight index if on a straight.
    pub(crate) fn straight(&self) -> Option<usize> {
        self.straight.current()
    }

    /// Get the current slope index if on a slope.
    pub(crate) fn slope(&self) -> Option<usize> {
        self.slope.current()
    }

    /// Check if currently on the final straight.
    pub(crate) fn is_on_final_straight(&self, course: &Course) -> bool {
        self.straight()
            .is_some_and(|idx| idx + 1 == course.straights.len())
    }

    /// Take the straight entered since the last call, if any.
    pub(crate) fn take_entered_straight(&mut self) -> Option<usize> {
        self.entered_straight.take()
    }
}

#[cfg(test)]
mod tests;
