//! Shared helpers for unit tests.

use crate::race::parameters::*;

/// Standard race parameters (firm turf, sunny spring G1 with 9 gates) with the
/// given RNG seed.
pub fn race_params(rng_seed: u64) -> RaceParameters {
    RaceParameters {
        track_condition: TrackCondition::Firm,
        weather: Weather::Sunny,
        season: Season::Spring,
        time: Time::None,
        grade: Grade::G1,
        gates: 9,
        rng_seed,
    }
}
