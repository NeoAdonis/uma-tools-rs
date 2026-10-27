//! Field of view and blocking, surrounding and nearby state between runners.

use crate::course::*;
use crate::units::*;

use super::Runner;

/// Surrounding state information.
#[derive(Debug, Default, Clone)]
pub struct SurroundState {
    pub front: bool,
    pub back: bool,
    pub side: bool,
}

/// Blocking state information.
#[derive(Debug, Default, Clone)]
pub struct BlockedState {
    /// Speed limit imposed by front blocking, if any.
    /// This also determines whether the runner is front-blocked.
    pub speed_limit: Option<f64>,
    /// Gate roll of the closest runner blocking from the front, if any.
    pub front_blocker: Option<u8>,
    /// Whether the runner is currently side-blocked towards the inside.
    pub side_in: bool,
    /// Whether the runner is currently side-blocked towards the outside.
    pub side_out: bool,
    /// Time spent being front-blocked in seconds.
    pub front_time: f64,
    /// Time spent being side-blocked in seconds.
    pub side_time: f64,
    /// Time spent being side-blocked during the Mid race in seconds. Unlike
    /// [`Self::side_time`], blocking before the Mid race doesn't count.
    pub mid_side_time: f64,
    /// Longest time spent being side-blocked during the Mid race in seconds.
    pub max_mid_side_time: f64,
}

impl BlockedState {
    /// Whether the runner is currently side-blocked in either direction.
    pub fn side(&self) -> bool {
        self.side_in || self.side_out
    }
}

/// Information about nearby runners.
#[derive(Debug, Default, Clone)]
pub struct NearbyState {
    /// Total number of nearby runners.
    pub count: u8,
    /// Whether the runner behind is in a lane that's closer to the center.
    pub behind_in: bool,
    /// Number of nearby runners in the same lane ahead.
    /// Maps to variable `near_infront_count`.
    pub ahead_same_lane_count: u8,
    /// Time spent near runners in the same lane ahead in seconds.
    pub ahead_same_lane_time: f64,
    /// Number of nearby runners in the same lane behind.
    pub behind_same_lane_count: u8,
    /// Time spent near runners in the same lane behind in seconds.
    pub behind_same_lane_time: f64,
    /// Number of runners behind within a reasonable distance.
    /// Used to calculate variable `behind_near_lane_time_set1`.
    pub behind_set1_count: u8,
    /// Time spent near runners behind within a reasonable distance in seconds.
    /// Maps to variable `behind_near_lane_time_set1`.
    pub behind_set1_time: f64,
}

impl Runner {
    /// Length of this runner's field of view in meters.
    ///
    /// Starts at 20m, modified by skill bonuses. The width of the field of view
    /// doesn't change, so a longer one is also narrower at a given distance.
    pub fn visible_distance(&self) -> f64 {
        // ? Assume minimum (shortened by stacked debuffs) is 0m.
        (20.0 + self.skill_modifiers.visible_distance).max(0.0)
    }

    /// Check if another runner is visible to this runner.
    pub fn is_visible(&self, other: &Self) -> bool {
        let distance = (self.position - other.position).abs();
        let visible_distance = self.visible_distance();
        // A runner without field of view sees nothing.
        visible_distance > 0.0
            && distance <= visible_distance
            && (self.lane.current - other.lane.current).abs()
                <= horse_lanes_to_course_widths((distance / visible_distance) * 11.5 + 2.0) / 2.0
    }

    /// Mark this runner as blocked from the front by `ahead`, limiting her speed
    /// based on the distance to `ahead` and her speed.
    ///
    /// Runners ahead must be checked from the furthest to the closest, so that
    /// the closest one prevails.
    pub fn update_front_blocked_by(&mut self, ahead: &Self) {
        let distance = (ahead.position - self.position).abs();
        self.blocked.speed_limit = Some((0.988 + 0.012 * distance) * ahead.speed);
        self.blocked.front_blocker = Some(ahead.gate_roll);
    }

    /// Update side-blocked state for this runner and another.
    ///
    /// `lane_diff` is the other runner's lane minus this runner's lane. Runners in
    /// the same lane block each other in both directions.
    pub fn update_side_blocked_by(&mut self, other: &mut Self, lane_diff: f64) {
        if lane_diff >= 0.0 {
            self.blocked.side_out = true;
            other.blocked.side_in = true;
        }
        if lane_diff <= 0.0 {
            self.blocked.side_in = true;
            other.blocked.side_out = true;
        }
    }

    /// Mark this runner as surrounded from the front by `ahead`, and `ahead` as
    /// surrounded from the back by this runner.
    pub fn update_front_surrounded_by(&mut self, ahead: &mut Self) {
        self.surrounded.front = true;
        ahead.surrounded.back = true;
    }

    /// Mark this runner and `other` as surrounding each other from the side.
    pub fn update_side_surrounded_by(&mut self, other: &mut Self) {
        self.surrounded.side = true;
        other.surrounded.side = true;
    }

    /// Count `ahead` as a nearby runner in the same lane ahead of this runner,
    /// and this runner as one behind her.
    pub fn update_near_same_lane_by(&mut self, ahead: &mut Self) {
        self.nearby.ahead_same_lane_count += 1;
        ahead.nearby.behind_same_lane_count += 1;
    }

    /// Count this runner and `other` as nearby runners of each other.
    pub fn update_near_by(&mut self, other: &mut Self) {
        self.nearby.count += 1;
        other.nearby.count += 1;
    }

    /// Check if this runner is front-blocked.
    pub fn is_front_blocked(&self) -> bool {
        self.blocked.speed_limit.is_some()
    }

    /// Check if this runner is surrounded on all sides.
    pub fn is_surrounded(&self) -> bool {
        self.surrounded.front && self.surrounded.back && self.surrounded.side
    }

    /// Reset blocking status for the next update cycle.
    pub fn reset_blocking_status(&mut self) {
        self.visible_count = 0;
        self.nearby.count = 0;
        self.nearby.behind_in = false;
        self.nearby.ahead_same_lane_count = 0;
        self.nearby.behind_same_lane_count = 0;
        self.nearby.behind_set1_count = 0;
        self.blocked.speed_limit = None;
        self.blocked.front_blocker = None;
        self.blocked.side_in = false;
        self.blocked.side_out = false;
        self.surrounded = SurroundState::default();
    }

    /// Update nearby time counters based on current nearby counts.
    pub fn update_near_time(&mut self, delta_time: f64) {
        let nearby = &mut self.nearby;
        tick(
            &mut nearby.ahead_same_lane_time,
            nearby.ahead_same_lane_count > 0,
            delta_time,
        );
        tick(
            &mut nearby.behind_same_lane_time,
            nearby.behind_same_lane_count > 0,
            delta_time,
        );
        tick(
            &mut nearby.behind_set1_time,
            nearby.behind_set1_count > 0,
            delta_time,
        );
    }

    /// Update blocked time counters based on current blocking status.
    pub fn update_blocked_time(&mut self, delta_time: f64) {
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
