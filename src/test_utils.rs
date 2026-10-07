//! Shared helpers for unit tests.

use crate::configuration::Configuration;
use crate::race::parameters::*;

/// Standard race parameters (firm turf, sunny spring G1 with 9 gates).
pub(crate) fn race_params() -> RaceParameters {
    RaceParameters {
        track_condition: TrackCondition::Firm,
        weather: Weather::Sunny,
        season: Season::Spring,
        time: Time::None,
        grade: Grade::G1,
        gates: 9,
    }
}

/// Default configuration with the given RNG seed.
pub(crate) fn seeded_config(seed: u64) -> Configuration {
    Configuration {
        seed: Some(seed),
        ..Default::default()
    }
}
