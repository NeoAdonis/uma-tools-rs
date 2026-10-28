//! Lane movement, overtake target tracking and target lane decisions.

use std::collections::BTreeMap;

use rand::prelude::*;

use crate::configuration::GameVersion;
use crate::course::*;
use crate::race::proximity;
use crate::race::rng::Stream;
use crate::race::transitions::*;
use crate::units::*;

use super::Runner;

/// Lane state including current and target lane positions.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LaneState {
    /// Current lane position.
    pub current: CourseWidths,
    /// Target lane position.
    pub target: CourseWidths,
    /// Lateral speed applied on the last lane update in course widths per second.
    ///
    /// Positive values move outward, negative values move inward and zero means the
    /// runner didn't change lanes on the last frame.
    pub speed: f64,
    /// Non-directional lane change speed in course widths per second.
    ///
    /// It changes towards the lane change target speed, before skills and direction
    /// are accounted for, and resets when the target lane is reached.
    pub current_speed: f64,
}

impl LaneState {
    pub(crate) fn new(lane: CourseWidths) -> Self {
        Self {
            current: lane,
            target: lane,
            speed: 0.0,
            current_speed: 0.0,
        }
    }
}

/// Result of a lane target calculation to be written back to the runner.
///
/// Both fields hold the runner's current values when unchanged.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct LaneDecision {
    pub(crate) target: CourseWidths,
    pub(crate) extra_move_lane: Option<CourseWidths>,
}

/// Overtake mode state including persistence timer.
#[derive(Debug, Default, Clone, PartialEq)]
pub(crate) struct OvertakeState {
    /// Ordered map of overtake targets by gate_roll.
    ///
    /// Lane candidates are always considered in the same order.
    targets: BTreeMap<u8, OvertakeTarget>,
    /// Time when overtake mode should expire (None if active with targets).
    expires_at: Option<f64>,
}

/// Information about an overtake target ahead of this runner.
#[derive(Debug, Default, Clone, PartialEq)]
pub(crate) struct OvertakeTarget {
    /// Whether this target has been checked in the current update cycle.
    is_checked: bool,
    /// Time spent being an overtake target in seconds.
    engagement_time: f64,
    /// Whether this target is the closest blocker.
    is_closest_blocker: bool,
}

impl Runner {
    /// Update lane position towards target lane.
    pub(crate) fn update_lane(&mut self, course: &Course, delta_time: f64) {
        const LANE_ACCELERATION: f64 = 0.02 * 1.5;
        const MAX_SPEED: f64 = 0.6;

        let target_lane = self.lane.target;

        let lane_diff = target_lane - self.lane.current;
        if lane_diff.abs() < CourseWidths(0.0001) {
            self.lane.current = target_lane;
            self.lane.current_speed = 0.0;
            self.lane.speed = 0.0;
            return;
        }

        let before_move_lane = course
            .move_lane_point()
            .is_some_and(|point| self.position < point);
        let first_move_modifier = if self.phase <= Phase::Mid && before_move_lane {
            1.0 + self.lane.current / course.width * 0.05
        } else {
            1.0
        };
        let order_modifier = if self.phase >= Phase::Late {
            1.0 + self.order.order as f64 * 0.01
        } else {
            1.0
        };
        let target_speed = 0.02
            * (0.3 + 0.001 * self.adjusted.power as f64)
            * first_move_modifier
            * order_modifier;

        // Current speed keeps changing towards the target speed even when blocked.
        let speed_step = LANE_ACCELERATION * delta_time;
        self.lane.current_speed = if self.lane.current_speed < target_speed {
            (self.lane.current_speed + speed_step).min(target_speed)
        } else {
            (self.lane.current_speed - speed_step).max(target_speed)
        };

        let moving_out = lane_diff > CourseWidths(0.0);
        let blocked = if moving_out {
            self.blocked.side_out
        } else {
            self.blocked.side_in
        };
        if blocked {
            self.lane.speed = 0.0;
            return;
        }

        let direction_modifier = if moving_out {
            1.0
        } else {
            -(1.0 + self.lane.current.0)
        };
        self.lane.speed = (self.lane.current_speed + self.skill_modifiers.lane_speed)
            .clamp(0.0, MAX_SPEED)
            * direction_modifier;

        let lane_change = CourseWidths(self.lane.speed * delta_time);
        if lane_change.abs() >= lane_diff.abs() {
            self.lane.current = target_lane;
            self.lane.current_speed = 0.0;
        } else {
            self.lane.current += lane_change;
        }
    }

    /// Uncheck all overtake targets at the start of an update cycle.
    pub(crate) fn reset_overtake_checks(&mut self) {
        if let Some(state) = &mut self.overtake_state {
            for target in state.targets.values_mut() {
                target.is_checked = false;
            }
        }
    }

    /// Check if another runner qualifies as an overtake target and update tracking.
    ///
    /// Returns true if the target was added or updated.
    pub(crate) fn check_overtake_target(&mut self, target: &Runner, delta_time: f64) -> bool {
        let distance = target.position - self.position;

        // Must be at least 1m ahead. The maximum distance is given by the field of
        // view, checked by the caller.
        if distance < 1.0 {
            return false;
        }

        // Can catch up within 15 seconds. At 1m or more, a speed difference below
        // 1/15 m/s can't, so it also avoids dividing by zero.
        let speed_diff = self.speed - target.speed;
        let can_catch_up = speed_diff >= 1.0 / 15.0 && distance / speed_diff < 15.0;
        let is_slower = target.target_speed < self.target_speed
            || (target.is_front_blocked() && target.speed < self.target_speed);
        if !(can_catch_up && is_slower) {
            return false;
        }

        let state = self.overtake_state.get_or_insert_default();
        let entry = state.targets.entry(target.gate_roll).or_default();
        entry.is_checked = true;
        entry.is_closest_blocker = false;
        entry.engagement_time += delta_time;
        true
    }

    /// Track the closest runner blocking from the front, if any, as an overtake
    /// target, regardless of the usual criteria.
    ///
    /// Must be called once every runner ahead has been checked. Returns her gate
    /// roll if she wasn't already a target in this update cycle.
    pub(crate) fn track_front_blocker_target(&mut self, delta_time: f64) -> Option<u8> {
        let gate_roll = self.blocked.front_blocker?;
        let state = self.overtake_state.get_or_insert_default();
        let entry = state.targets.entry(gate_roll).or_default();
        entry.is_closest_blocker = true;
        if entry.is_checked {
            return None;
        }
        entry.is_checked = true;
        entry.engagement_time += delta_time;
        Some(gate_roll)
    }

    /// Finalize overtake state after all targets have been checked.
    ///
    /// Handles cleanup of unchecked targets and persistence timer.
    pub(crate) fn finalize_overtake_state(&mut self, delta_time: f64, elapsed_time: f64) {
        if let Some(state) = &mut self.overtake_state {
            // Remove unchecked targets.
            state.targets.retain(|_, target| target.is_checked);

            if state.targets.is_empty() {
                if let Some(expires_at) = state.expires_at {
                    if elapsed_time >= expires_at {
                        // Persistence expired, clear state.
                        self.overtake_state = None;
                        self.overtake_target_time = 0.0;
                    }
                } else {
                    // Just lost all targets, start persistence timer.
                    state.expires_at = Some(elapsed_time + 1.5);
                }
            } else {
                // Have targets, clear expiration and increment time.
                state.expires_at = None;
                self.overtake_target_time += delta_time;
            }
        }
    }

    /// Check if the runner is in overtake mode (has targets or in persistence
    /// period).
    pub(crate) fn is_in_overtake_mode(&self) -> bool {
        self.overtake_state.is_some()
    }

    /// Calculate the target lane for this frame, or `None` if it shouldn't be
    /// updated (see [`Self::should_update_target_lane`]).
    ///
    /// The first mode that applies is used:
    /// 1. Fixed mode, while a skill sets the target lane (see
    ///    [`Self::fixed_mode_lane_target`]).
    /// 2. Normal mode, if not in overtake mode, or during the Early and Mid race
    ///    up to 200m before the move lane point.
    /// 3. Overtake mode.
    pub(crate) fn target_lane_decision(
        &self,
        course: &Course,
        all_runners: &[Runner],
    ) -> Option<LaneDecision> {
        if !self.should_update_target_lane() {
            return None;
        }

        if let Some(target) = self.fixed_mode_lane_target() {
            return Some(LaneDecision {
                target,
                extra_move_lane: self.extra_move_lane,
            });
        }

        let near_move_lane = course
            .move_lane_point()
            .is_some_and(|point| (0.0..=200.0).contains(&(point - self.position)));
        let use_normal_mode =
            !self.is_in_overtake_mode() || (self.phase <= Phase::Mid && near_move_lane);
        let decision = if use_normal_mode {
            self.normal_mode_lane_decision(course, all_runners)
        } else {
            self.overtake_lane_decision(course, all_runners)
        };
        Some(decision)
    }

    /// Check if the target lane should be updated: when the runner is less than
    /// 0.5 horse lanes from it, or blocked on the side she's moving towards.
    pub(crate) fn should_update_target_lane(&self) -> bool {
        (self.lane.target - self.lane.current).abs() < CourseWidths::from_horse_lanes(0.5)
            || !self.has_lane_space_to(self.lane.target)
    }

    /// Target lane in fixed mode, used while a skill effect sets the target
    /// lane, unless the runner is blocked from moving towards it.
    pub(crate) fn fixed_mode_lane_target(&self) -> Option<CourseWidths> {
        self.skill_modifiers
            .target_lane
            .filter(|&lane| self.has_lane_space_to(lane))
    }

    /// Set the extra move lane upon entering the final corner, based on the
    /// current lane. Before the 1st Anniv., it was set upon entering the final
    /// straight instead.
    pub(crate) fn update_final_extra_move_lane(
        &mut self,
        transitions: &Transitions,
        course: &Course,
        game_version: GameVersion,
    ) {
        let entered_final = if game_version >= GameVersion::FirstAnniversary {
            let last = course.corners.len().checked_sub(1);
            transitions
                .corner
                .entered
                .is_some_and(|idx| Some(idx) == last)
        } else {
            let last = course.straights.len().checked_sub(1);
            transitions
                .straight
                .entered
                .is_some_and(|idx| Some(idx) == last)
        };
        if entered_final {
            let offset = self.rng.keyed(Stream::FinalLane).random_range(0.0..0.1);
            let ratio = (self.lane.current / CourseWidths(0.1)).clamp(0.0, 1.0);
            self.extra_move_lane = Some(CourseWidths(ratio * 0.5 + offset));
        }
    }

    /// Calculate lane target when in normal mode (not overtaking).
    pub(crate) fn normal_mode_lane_decision(
        &self,
        course: &Course,
        all_runners: &[Runner],
    ) -> LaneDecision {
        LaneDecision {
            target: self.normal_mode_lane_target(course, all_runners),
            extra_move_lane: self.extra_move_lane,
        }
    }

    /// Target lane in normal mode.
    ///
    /// Rules are checked in priority order; first match wins.
    fn normal_mode_lane_target(&self, course: &Course, all_runners: &[Runner]) -> CourseWidths {
        // Out of HP - stay in current lane.
        if self.hp.is_empty() {
            return self.lane.current;
        }

        // Position keeping mode is pace-down.
        // Set lane target to fixed point.
        if self.is_pace_down() {
            return CourseWidths(0.18).min(course.width);
        }

        // During and after the final corner (anywhere on a straight course), if
        // the extra move lane is on the outside.
        if course
            .corners
            .last()
            .is_none_or(|corner| self.position >= corner.start)
            && let Some(extra) = self.extra_move_lane
            && extra > self.lane.current
            && self.has_lane_space_to(extra)
        {
            return extra.clamp(CourseWidths(0.0), course.width);
        }

        // In early-race or mid-race, move inward.
        if self.phase <= Phase::Mid {
            let target = self.lane.current - CourseWidths(0.05);
            if target >= CourseWidths(0.0) && self.has_lane_space_to(target) {
                return target;
            }
        }

        // In mid-race and close to inside runner.
        if self.phase == Phase::Mid
            && let Some((inside_lane, distance_in_lanes)) = self.find_inside_runner(all_runners)
            && distance_in_lanes < 1.75
        {
            // Move away from inside runner, even if blocked.
            let target = inside_lane + CourseWidths::from_horse_lanes(2.0);
            return target.clamp(CourseWidths(0.0), course.width);
        }

        // Otherwise, keep straight.
        self.lane.current
    }

    /// Calculate target lane when in overtake mode.
    ///
    /// This should be called every frame after overtake state is finalized.
    pub(crate) fn overtake_lane_decision(
        &self,
        course: &Course,
        all_runners: &[Runner],
    ) -> LaneDecision {
        let mut decision = LaneDecision {
            target: self.lane.target,
            extra_move_lane: self.extra_move_lane,
        };

        // Early exit if not in overtake mode.
        let Some(state) = &self.overtake_state else {
            return decision;
        };

        decision.extra_move_lane = self.next_extra_move_lane(all_runners);

        // Candidates from each target (in gate order), then the extra one.
        // The best (lowest score, first on ties) with lane space is accepted,
        // or the current lane if there's none.
        let targets: Vec<&Runner> = state
            .targets
            .keys()
            .filter_map(|&gate| all_runners.iter().find(|r| r.gate_roll == gate))
            .collect();
        let best_lane = targets
            .iter()
            .flat_map(|target| self.target_lane_candidates(target, all_runners))
            .chain(self.extra_lane_candidate(&targets, all_runners))
            .filter(|&lane| self.has_lane_space_to(lane))
            .map(|lane| (lane, self.overtake_lane_score(lane)))
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map_or(self.lane.current, |(lane, _)| lane);

        // Move to the extra move lane instead if it's outside and reachable.
        let target_lane = match decision.extra_move_lane {
            Some(extra) if best_lane < extra && self.has_lane_space_to(extra) => extra,
            _ => best_lane,
        };
        decision.target = target_lane.clamp(CourseWidths(0.0), course.width);
        decision
    }

    /// Runners in `all_runners` other than this one.
    fn others<'a>(&self, all_runners: &'a [Runner]) -> impl Iterator<Item = &'a Runner> {
        let gate_roll = self.gate_roll;
        all_runners.iter().filter(move |r| r.gate_roll != gate_roll)
    }

    /// Find the innermost and outermost umas in a target's crowd (the target
    /// herself if alone). Crowd members are up to 3m ahead of the target and
    /// less than 2 horse lanes away.
    fn find_crowd_boundaries<'a>(
        &self,
        target: &'a Runner,
        all_runners: &'a [Runner],
    ) -> (&'a Runner, &'a Runner) {
        self.others(all_runners)
            .filter(|other| {
                other.gate_roll != target.gate_roll
                    && proximity::is_near_by(
                        other.position - target.position,
                        (other.lane.current - target.lane.current).abs(),
                        3.0,
                        2.0,
                    )
            })
            .fold((target, target), |(inner, outer), other| {
                let inner = if other.lane.current < inner.lane.current {
                    other
                } else {
                    inner
                };
                let outer = if other.lane.current > outer.lane.current {
                    other
                } else {
                    outer
                };
                (inner, outer)
            })
    }

    /// Candidate lanes to overtake `target`: 1 horse lane inside and outside
    /// each boundary uma of her crowd, unless a visible uma is in the spot up to
    /// 0.5m past that boundary uma.
    fn target_lane_candidates<'a>(
        &'a self,
        target: &'a Runner,
        all_runners: &'a [Runner],
    ) -> impl Iterator<Item = CourseWidths> + 'a {
        let (inner, outer) = self.find_crowd_boundaries(target, all_runners);
        let offset = CourseWidths::from_horse_lanes(1.0);
        [
            (inner, -offset),
            (outer, -offset),
            (inner, offset),
            (outer, offset),
        ]
        .into_iter()
        .filter_map(move |(boundary, offset)| {
            let lane = boundary.lane.current + offset;
            let max_distance = boundary.position - self.position + 0.5;
            self.is_lane_candidate_valid(lane, max_distance, all_runners)
                .then_some(lane)
        })
    }

    /// Candidate lane unrelated to the overtake `targets`: 1 horse lane inward
    /// during the Early and Mid race (if possible), or straight otherwise,
    /// unless a visible uma is in the spot up to 3m past the furthest target.
    fn extra_lane_candidate(
        &self,
        targets: &[&Runner],
        all_runners: &[Runner],
    ) -> Option<CourseWidths> {
        // ? Without targets (while overtake mode persists), measured from the
        // runner herself.
        let furthest_target_position = targets
            .iter()
            .map(|r| r.position)
            .max_by(f64::total_cmp)
            .unwrap_or(self.position);

        let one_lane = CourseWidths::from_horse_lanes(1.0);
        let lane = if self.phase <= Phase::Mid && self.lane.current >= one_lane {
            self.lane.current - one_lane
        } else {
            self.lane.current
        };

        let max_distance = furthest_target_position - self.position + 3.0;
        self.is_lane_candidate_valid(lane, max_distance, all_runners)
            .then_some(lane)
    }

    /// Check if a candidate lane is valid: no visible runner ahead within
    /// `max_distance` meters and 0.8 horse lanes of the candidate.
    fn is_lane_candidate_valid(
        &self,
        candidate_lane: CourseWidths,
        max_distance: f64,
        all_runners: &[Runner],
    ) -> bool {
        !self.others(all_runners).any(|other| {
            self.is_visible(other)
                && proximity::is_near_by(
                    other.position - self.position,
                    (other.lane.current - candidate_lane).abs(),
                    max_distance,
                    0.8,
                )
        })
    }

    /// Score of an overtake candidate `lane` (lower is better): the lane change,
    /// with moving outward penalized, heavily in the Early race.
    fn overtake_lane_score(&self, lane: CourseWidths) -> f64 {
        let outward_coef = match self.phase {
            Phase::RunUp | Phase::Early => 100.0,
            Phase::Mid => 1.0,
            Phase::Late | Phase::Last => 1.15,
        };
        let coef = if lane < self.lane.current {
            1.0
        } else {
            outward_coef
        };
        (lane - self.lane.current).abs().0 * coef
    }

    /// Check if the runner has lane space to reach `target_lane`, i.e. she
    /// isn't side-blocked towards it (see [`proximity::check_side_blocked`]).
    fn has_lane_space_to(&self, target_lane: CourseWidths) -> bool {
        if target_lane > self.lane.current {
            !self.blocked.side_out
        } else if target_lane < self.lane.current {
            !self.blocked.side_in
        } else {
            true
        }
    }

    /// Next extra_move_lane based on furthest visible runner ahead.
    ///
    /// Only updated during Early and Mid phases, so that the one set upon
    /// entering the final corner is kept. Keeps the current value if no runner
    /// ahead is visible.
    fn next_extra_move_lane(&self, all_runners: &[Runner]) -> Option<CourseWidths> {
        if self.phase > Phase::Mid {
            return self.extra_move_lane;
        }

        // Find furthest visible runner ahead.
        all_runners
            .iter()
            .filter(|r| r.position > self.position && self.is_visible(r))
            .max_by(|a, b| a.position.total_cmp(&b.position))
            .map_or(self.extra_move_lane, |furthest| Some(furthest.lane.current))
    }

    /// Find the closest uma on the inside (lower lane value), within 5m and 3
    /// horse lanes. Returns (inside_uma_lane, distance_in_horse_lanes) if found.
    fn find_inside_runner(&self, all_runners: &[Runner]) -> Option<(CourseWidths, f64)> {
        self.others(all_runners)
            .filter(|other| {
                other.lane.current < self.lane.current
                    && (self.position - other.position).abs() <= 5.0
            })
            .map(|other| {
                let lane_diff = (self.lane.current - other.lane.current).horse_lanes();
                (other.lane.current, lane_diff)
            })
            .filter(|&(_, lane_diff)| lane_diff <= 3.0)
            // First on ties.
            .min_by(|a, b| a.1.total_cmp(&b.1))
    }
}

#[cfg(test)]
mod tests;
