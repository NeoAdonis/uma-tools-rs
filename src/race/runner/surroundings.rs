//! Field of view and blocking, surrounding and nearby state between runners.

use crate::course::*;
use crate::units::*;

use super::Runner;

/// Surrounding state information.
#[derive(Debug, Default, Clone, PartialEq)]
pub(crate) struct SurroundState {
    pub(crate) front: bool,
    pub(crate) back: bool,
    pub(crate) side: bool,
}

/// Blocking state information.
#[derive(Debug, Default, Clone, PartialEq)]
pub(crate) struct BlockedState {
    /// Share of the closest front blocker's speed the runner is limited to, if
    /// she's front-blocked.
    pub(crate) speed_ratio: Option<f64>,
    /// Gate roll of the closest runner blocking from the front, if any.
    pub(crate) front_blocker: Option<u8>,
    /// Whether the runner is currently side-blocked towards the inside.
    pub(crate) side_in: bool,
    /// Whether the runner is currently side-blocked towards the outside.
    pub(crate) side_out: bool,
    /// Time spent being front-blocked in seconds.
    pub(crate) front_time: f64,
    /// Time spent being side-blocked in seconds.
    pub(crate) side_time: f64,
    /// Time spent being side-blocked during the Mid race in seconds. Unlike
    /// [`Self::side_time`], blocking before the Mid race doesn't count.
    pub(crate) mid_side_time: f64,
    /// Longest time spent being side-blocked during the Mid race in seconds.
    pub(crate) max_mid_side_time: f64,
}

impl BlockedState {
    /// Whether the runner is currently side-blocked in either direction.
    pub(crate) fn side(&self) -> bool {
        self.side_in || self.side_out
    }
}

/// Information about nearby runners.
#[derive(Debug, Default, Clone, PartialEq)]
pub(crate) struct NearbyState {
    /// Total number of nearby runners.
    pub(crate) count: u8,
    /// Whether the runner behind is in a lane that's closer to the center.
    pub(crate) behind_in: bool,
    /// Number of nearby runners in the same lane ahead.
    /// Maps to variable `near_infront_count`.
    pub(crate) ahead_same_lane_count: u8,
    /// Time spent near runners in the same lane ahead in seconds.
    pub(crate) ahead_same_lane_time: f64,
    /// Number of nearby runners in the same lane behind.
    pub(crate) behind_same_lane_count: u8,
    /// Time spent near runners in the same lane behind in seconds.
    pub(crate) behind_same_lane_time: f64,
    /// Number of runners behind within a reasonable distance.
    /// Used to calculate variable `behind_near_lane_time_set1`.
    pub(crate) behind_set1_count: u8,
    /// Time spent near runners behind within a reasonable distance in seconds.
    /// Maps to variable `behind_near_lane_time_set1`.
    pub(crate) behind_set1_time: f64,
}

impl Runner {
    /// Length of this runner's field of view in meters.
    ///
    /// Starts at 20m, modified by skill bonuses. The width of the field of view
    /// doesn't change, so a longer one is also narrower at a given distance.
    pub(crate) fn visible_distance(&self) -> f64 {
        // ? Assume minimum (shortened by stacked debuffs) is 0m.
        (20.0 + self.skill_modifiers.visible_distance).max(0.0)
    }

    /// Check if another runner is visible to this runner.
    pub(crate) fn is_visible(&self, other: &Self) -> bool {
        let distance = (self.position - other.position).abs();
        let visible_distance = self.visible_distance();
        // A runner without field of view sees nothing.
        visible_distance > 0.0
            && distance <= visible_distance
            && (self.lane.current - other.lane.current).horse_lanes().abs() * 2.0
                <= (distance / visible_distance) * 11.5 + 2.0
    }

    /// Mark this runner as blocked from the front by `ahead`, limiting her speed
    /// to 98.8% of `ahead`'s speed when touching, up to 100% at 2m. The ratio
    /// is set now, but applies to `ahead`'s speed when this runner moves.
    ///
    /// Runners ahead must be checked from the furthest to the closest, so that
    /// the closest one prevails.
    pub(crate) fn update_front_blocked_by(&mut self, ahead: &Self) {
        let distance = (ahead.position - self.position).abs();
        self.blocked.speed_ratio = Some(0.988 + 0.012 * distance / 2.0);
        self.blocked.front_blocker = Some(ahead.gate_roll);
    }

    /// Update side-blocked state for this runner and another.
    ///
    /// `lane_diff` is the other runner's lane minus this runner's lane. Runners in
    /// the same lane block each other in both directions.
    pub(crate) fn update_side_blocked_by(&mut self, other: &mut Self, lane_diff: CourseWidths) {
        if lane_diff >= CourseWidths(0.0) {
            self.blocked.side_out = true;
            other.blocked.side_in = true;
        }
        if lane_diff <= CourseWidths(0.0) {
            self.blocked.side_in = true;
            other.blocked.side_out = true;
        }
    }

    /// Mark this runner as surrounded from the front by `ahead`, and `ahead` as
    /// surrounded from the back by this runner.
    pub(crate) fn update_front_surrounded_by(&mut self, ahead: &mut Self) {
        self.surrounded.front = true;
        ahead.surrounded.back = true;
    }

    /// Mark this runner and `other` as surrounding each other from the side.
    pub(crate) fn update_side_surrounded_by(&mut self, other: &mut Self) {
        self.surrounded.side = true;
        other.surrounded.side = true;
    }

    /// Count `ahead` as a nearby runner in the same lane ahead of this runner,
    /// and this runner as one behind her.
    pub(crate) fn update_near_same_lane_by(&mut self, ahead: &mut Self) {
        self.nearby.ahead_same_lane_count += 1;
        ahead.nearby.behind_same_lane_count += 1;
    }

    /// Count this runner and `other` as nearby runners of each other.
    pub(crate) fn update_near_by(&mut self, other: &mut Self) {
        self.nearby.count += 1;
        other.nearby.count += 1;
    }

    /// Check if this runner is front-blocked.
    pub fn is_front_blocked(&self) -> bool {
        self.blocked.speed_ratio.is_some()
    }

    /// Check if this runner is surrounded on all sides.
    pub(crate) fn is_surrounded(&self) -> bool {
        self.surrounded.front && self.surrounded.back && self.surrounded.side
    }

    /// Reset blocking status for the next update cycle.
    pub(crate) fn reset_blocking_status(&mut self) {
        self.visible_count = 0;
        self.nearby.count = 0;
        self.nearby.behind_in = false;
        self.nearby.ahead_same_lane_count = 0;
        self.nearby.behind_same_lane_count = 0;
        self.nearby.behind_set1_count = 0;
        self.blocked.speed_ratio = None;
        self.blocked.front_blocker = None;
        self.blocked.side_in = false;
        self.blocked.side_out = false;
        self.surrounded = SurroundState::default();
    }

    /// Update nearby time counters based on current nearby counts.
    ///
    /// The counters also reset when this runner's order changes, so the order
    /// must be updated first.
    pub(crate) fn update_near_time(&mut self, delta_time: f64) {
        let same_order = self.order.delta == 0;
        let nearby = &mut self.nearby;
        tick(
            &mut nearby.ahead_same_lane_time,
            same_order && nearby.ahead_same_lane_count > 0,
            delta_time,
        );
        tick(
            &mut nearby.behind_same_lane_time,
            same_order && nearby.behind_same_lane_count > 0,
            delta_time,
        );
        tick(
            &mut nearby.behind_set1_time,
            same_order && nearby.behind_set1_count > 0,
            delta_time,
        );
    }

    /// Update blocked time counters based on current blocking status.
    pub(crate) fn update_blocked_time(&mut self, delta_time: f64) {
        let is_front_blocked = self.is_front_blocked();
        tick(&mut self.blocked.front_time, is_front_blocked, delta_time);
        if self.blocked.side() {
            self.blocked.side_time += delta_time;
            if self.phase == Phase::Mid {
                self.blocked.mid_side_time += delta_time;
                self.blocked.max_mid_side_time = self
                    .blocked
                    .max_mid_side_time
                    .max(self.blocked.mid_side_time);
            }
        } else {
            self.blocked.side_time = 0.0;
            self.blocked.mid_side_time = 0.0;
        }
    }
}

/// Add `delta_time` to `timer` while `active`, or reset it otherwise.
fn tick(timer: &mut f64, active: bool, delta_time: f64) {
    *timer = if active { *timer + delta_time } else { 0.0 };
}

#[cfg(test)]
mod tests;
