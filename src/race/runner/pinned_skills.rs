//! State of the skills whose activations are pinned (see
//! [`SkillPin::Activations`]).
//!
//! Pinned activations replace the skills' conditions, which are still checked
//! to report where they disagree with the pins. The skills' regular logic only
//! asks this module which skills are pinned; everything else about pins flows
//! through [`PinnedSkills`] at a few fixed points of the skill checks.
//!
//! [`SkillPin::Activations`]: crate::race::pins::SkillPin::Activations

use std::sync::Arc;

use crate::race::pins::{PinQueue, Pins};
use crate::skill::Skill;
use crate::trigger::skill_activation::{QueuedActivation, SkillActivationRecord};

/// State of the pinned activations of a runner's skills.
///
/// Skill indices are those of the runner's available skills, which go first
/// (see `Runner::skills`).
#[derive(Debug, Default, PartialEq)]
pub(crate) struct PinnedSkills {
    /// Whether the activations of each available skill are pinned, by skill
    /// index. Empty if none is, and answering `false` past its end.
    pinned: Vec<bool>,
    /// Pinned activations still to happen during the race.
    queue: PinQueue<QueuedActivation>,
    /// Pinned activations due in the current frame that haven't activated yet,
    /// by descending skill (the next one last), so that they activate in skill ID
    /// order along with the checks.
    due: Vec<QueuedActivation>,
    /// Skill index and index in `activated_skills` of this frame's pinned
    /// activations, until the next frame's checks start their cooldowns.
    this_frame: Vec<(usize, usize)>,
    /// Whether each skill has had a missing activation reported while a pinned
    /// activation of it was still to come, until one is due. Empty if no skill
    /// is pinned.
    missing_reported: Vec<bool>,
    /// IDs of the skills whose conditions didn't allow their pinned out-of-gate
    /// alternative to activate.
    ///
    /// Emptied on the first frame she moves, when those activations are
    /// recorded as unmet (see [`SkillActivationRecord::conditions_met`]).
    unmet_out_of_gate: Vec<i32>,
    /// Activations missing from the pins (see
    /// [`Runner::missing_activations`]).
    ///
    /// [`Runner::missing_activations`]:
    ///     crate::race::runner::Runner::missing_activations
    missing: Vec<SkillActivationRecord>,
}

impl PinnedSkills {
    /// Creates the state of the skills in `skills`, according to `pins`.
    ///
    /// Skills that failed the wit check must be left out: they're never pinned,
    /// since a pinned activation passes it whatever the roll.
    pub(crate) fn new(pins: &Pins, skills: &[Arc<Skill>]) -> Self {
        let is_pinned = |skill: &Skill| pins.activations(skill.id).is_some();
        if !skills.iter().any(|skill| is_pinned(skill)) {
            return Self::default();
        }
        // Out-of-gate ones already happened. Activations at the same time are
        // taken in order (by skill, then as pinned).
        let mut queue = Vec::new();
        for (skill_index, skill) in skills.iter().enumerate() {
            for activation in pins.activations(skill.id).unwrap_or_default() {
                if skill.alternatives[activation.alternative].is_out_of_gate() {
                    continue;
                }
                let queued = QueuedActivation {
                    skill_index,
                    alternative_index: activation.alternative,
                    targets: activation.targets.clone(),
                };
                queue.push((activation.time, queued));
            }
        }
        Self {
            pinned: skills.iter().map(|skill| is_pinned(skill)).collect(),
            queue: PinQueue::new(queue),
            missing_reported: vec![false; skills.len()],
            ..Self::default()
        }
    }

    /// Returns whether the activations of the skill with index `skill_index` are
    /// pinned.
    ///
    /// Skills that failed the wit check aren't.
    pub(crate) fn is_pinned(&self, skill_index: usize) -> bool {
        self.pinned.get(skill_index).copied().unwrap_or(false)
    }

    /// Returns the activations missing from the pins.
    pub(crate) fn missing(&self) -> &[SkillActivationRecord] {
        &self.missing
    }

    /// Records an activation missing from the pins.
    pub(crate) fn push_missing(&mut self, record: SkillActivationRecord) {
        self.missing.push(record);
    }

    /// Records that the conditions of the skill with ID `skill_id` didn't allow
    /// its pinned out-of-gate alternative.
    pub(crate) fn push_unmet_out_of_gate(&mut self, skill_id: i32) {
        self.unmet_out_of_gate.push(skill_id);
    }

    /// Takes the IDs of the skills whose pinned out-of-gate alternative wasn't
    /// allowed by its conditions.
    pub(crate) fn take_unmet_out_of_gate(&mut self) -> Vec<i32> {
        std::mem::take(&mut self.unmet_out_of_gate)
    }

    /// Takes the skill index and record index of this frame's pinned
    /// activations.
    pub(crate) fn take_this_frame(&mut self) -> Vec<(usize, usize)> {
        std::mem::take(&mut self.this_frame)
    }

    /// Moves the pinned activations due by `time` to the frame's queue, ordered
    /// by skill (then time, as queued), the next one last.
    pub(crate) fn collect_due(&mut self, time: f64) {
        while let Some(next) = self.queue.take_due(time) {
            self.due.push(next);
        }
        // Stable, so that the order of the same skill is kept.
        self.due.sort_by_key(|pin| pin.skill_index);
        self.due.reverse();
    }

    /// Returns whether pinned activations are due and haven't activated yet.
    pub(crate) fn has_due(&self) -> bool {
        !self.due.is_empty()
    }

    /// Takes the next due pinned activation of a skill up to `skill_index`, if
    /// any.
    ///
    /// Its skill may report a missing activation again.
    pub(crate) fn pop_due(&mut self, skill_index: usize) -> Option<QueuedActivation> {
        let next = self.due.pop_if(|next| next.skill_index <= skill_index)?;
        self.missing_reported[next.skill_index] = false;
        Some(next)
    }

    /// Registers that the pinned activation of the skill with index
    /// `skill_index` is about to be recorded at `record_index` in
    /// `activated_skills`.
    ///
    /// It waits for the conditions to confirm it on this frame (see
    /// [`Self::confirmable`]).
    pub(crate) fn activating(&mut self, skill_index: usize, record_index: usize) {
        self.this_frame.push((skill_index, record_index));
    }

    /// Returns the index in `activated` of this frame's pinned activation of the
    /// skill with ID `skill_id` through `alternative_index`, if the conditions
    /// haven't confirmed it yet.
    pub(crate) fn confirmable(
        &self,
        activated: &[SkillActivationRecord],
        skill_id: i32,
        alternative_index: usize,
    ) -> Option<usize> {
        self.this_frame
            .iter()
            .map(|&(_, record_index)| record_index)
            .find(|&record_index| {
                let record = &activated[record_index];
                record.skill_id == skill_id
                    && record.alternative == alternative_index
                    && !record.conditions_met
            })
    }

    /// Returns whether the skill with index `skill_index` can report a missing
    /// activation now.
    pub(crate) fn can_report_missing(&self, skill_index: usize) -> bool {
        !self.missing_reported[skill_index]
    }

    /// Records a missing activation of the skill with index `skill_index`,
    /// returning whether its cooldown should start, as if it activated.
    ///
    /// It shouldn't while the skill has a pinned activation still to come, so
    /// that the conditions can still confirm that one. No more are then reported
    /// (for any alternative) until one is due.
    pub(crate) fn report_missing(
        &mut self,
        skill_index: usize,
        record: SkillActivationRecord,
    ) -> bool {
        self.missing.push(record);
        let upcoming = self.queue.iter().any(|pin| pin.skill_index == skill_index);
        if upcoming {
            self.missing_reported[skill_index] = true;
        }
        !upcoming
    }
}
