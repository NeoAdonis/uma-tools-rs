//! Random number streams.
//!
//! Every source of randomness in a race draws from its own stream, derived
//! from the race seed by hashing a [`Stream`] key: one per runner mechanic
//! (rush, positioning, slopes...) and a few per skill (wit check, trigger
//! rolls...). This way, a runner's skills only consume their own streams, so
//! adding or removing a skill doesn't shift the draws of unrelated mechanics,
//! skills or runners. Two races with the same seed that only differ in one
//! skill stay paired: they're identical until the skill makes a difference
//! (e.g. by activating), which reduces the variance when comparing them.
//!
//! Draws tied to a recurring event with a natural index (e.g. the section speed
//! modifier, rolled once per section) use a stream keyed by that index, so they
//! stay aligned even if the event happens at a different time.
//!
//! Streams are only meant to be reproducible for a given build: a change in the
//! simulator (or in the standard library's hasher) may change every draw.

use std::hash::{DefaultHasher, Hash, Hasher};

use rand::SeedableRng;
use rand_pcg::Pcg64Mcg;

/// Random number generator used by every stream.
pub(crate) type RaceRng = Pcg64Mcg;

/// Key of a random number stream, derived from a parent [`RngSeed`].
#[derive(Debug, Clone, Copy, Hash)]
pub(crate) enum Stream {
    // === Race streams ===
    /// Runner by insertion index, whose own streams derive from it.
    Runner(u8),
    /// Random offset of the scoreboard time.
    Scoreboard,

    // === Runner streams ===
    /// Gate roll.
    Gate,
    /// Mood roll.
    Mood,
    /// Start delay.
    StartDelay,
    /// Rush chance check, rush style and section, and snap-out checks.
    Rush,
    /// Force-in modifier.
    ForceIn,
    /// Stamina Keep ratio and checks.
    StaminaKeep,
    /// Section speed modifier, by section.
    SectionSpeed(u8),
    /// Downhill mode checks, by slope index.
    Slope(usize),
    /// Positioning strategy checks and thresholds.
    Positioning,
    /// Last spurt candidate checks.
    LastSpurt,
    /// Secure lead checks.
    SecureLead,
    /// Lane offset upon entering the final corner or straight.
    FinalLane,

    // === Skill streams, by skill ID ===
    /// Wit check.
    WitCheck(i32),
    /// Random trigger rolls (see
    /// [`RandomRolls`](crate::trigger::random::RandomRolls)).
    TriggerRolls(i32),
    /// Lotteries of out-of-gate skills (`random_lot`, `random_lot_shared`).
    Lottery(i32),
    /// Draws during the race (value scaling, targets, gold skill picks...).
    Skill(i32),
}

/// Seed from which random number streams are derived.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct RngSeed(u64);

impl RngSeed {
    pub(crate) fn new(seed: u64) -> Self {
        Self(seed)
    }

    /// Seed of the sub-stream identified by `stream`.
    pub(crate) fn derive(self, stream: Stream) -> Self {
        let mut hasher = DefaultHasher::new();
        self.0.hash(&mut hasher);
        stream.hash(&mut hasher);
        Self(hasher.finish())
    }

    /// Random number generator of the sub-stream identified by `stream`.
    pub(crate) fn rng(self, stream: Stream) -> RaceRng {
        RaceRng::seed_from_u64(self.derive(stream).0)
    }
}

/// Random number streams of a runner during the race.
///
/// Streams for one-off draws, and those keyed by event, are derived when needed
/// (see [`Self::keyed`]).
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct RunnerRng {
    /// Seed of the runner, from which every stream derives.
    pub seed: RngSeed,
    pub rush: RaceRng,
    pub stamina_keep: RaceRng,
    /// Stream of the last slope the runner entered (see [`Stream::Slope`]),
    /// replaced upon entering each slope.
    pub slope: RaceRng,
    pub positioning: RaceRng,
    pub last_spurt: RaceRng,
    pub secure_lead: RaceRng,
    /// In-race stream of each skill, in the runner's skill order (see
    /// [`Stream::Skill`]).
    pub skills: Vec<RaceRng>,
}

impl RunnerRng {
    /// Create the streams of a runner with the given `skill_ids`, in order.
    pub(crate) fn new(seed: RngSeed, skill_ids: impl IntoIterator<Item = i32>) -> Self {
        Self {
            seed,
            rush: seed.rng(Stream::Rush),
            stamina_keep: seed.rng(Stream::StaminaKeep),
            slope: seed.rng(Stream::Slope(0)),
            positioning: seed.rng(Stream::Positioning),
            last_spurt: seed.rng(Stream::LastSpurt),
            secure_lead: seed.rng(Stream::SecureLead),
            skills: skill_ids
                .into_iter()
                .map(|id| seed.rng(Stream::Skill(id)))
                .collect(),
        }
    }

    /// Stream of the runner identified by `stream`, used for one-off draws or
    /// draws keyed by event.
    pub(crate) fn keyed(&self, stream: Stream) -> RaceRng {
        self.seed.rng(stream)
    }
}

#[cfg(test)]
mod tests;
