//! Configuration for the simulation behavior, game mechanics and language
//! settings.

use rand::{Rng, RngExt};
use serde::Deserialize;

/// Represents different versions of the game.
///
/// Used to determine the behavior for some of the mechanics. This list is
/// sorted in chronological order, from the original release to the latest
/// update, allowing to compare if a certain version is before or after a
/// certain update. The list is not exhaustive, and only includes the versions
/// that have a significant impact on the game mechanics.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Deserialize)]
pub enum GameVersion {
    /// When the game was first released.
    Original,
    /// After the first anniversary update in Japan (Half-anniversary update in
    /// global).
    ///
    /// This update introduced spot struggles and stamina dueling mechanics, a buff
    /// to make the guts stat more useful, last spurt recalculation with HP changes,
    /// and a force-in modifier for lane changes.
    FirstAnniversary,
    /// After the 1.5 anniversary update in Japan.
    ///
    /// This update makes adjustments to positioning. It softened pace-down
    /// positioning in the mid-race, making the slowdown milder and the target gap
    /// to the pacemaker smaller, and let a lone front runner keep speeding up to
    /// stay ahead with a larger lead.
    OnePointFiveAnniversary,
    /// After the 2nd anniversary update in Japan.
    ///
    /// This update introduces charge up mechanics.
    SecondAnniversary,
    /// After the 2.5 anniversary update in Japan.
    ///
    /// This update introduced stamina limit breaks, compete before spurt, stamina
    /// keep and lead securing mechanics.
    TwoPointFiveAnniversary,
    /// After the Mecha Umamusume scenario release in Japan.
    ///
    /// This update introduced a buff to stamina limit breaks for long-distance
    /// courses.
    MechaUmaUpdate,
    /// After the 5th anniversary update in Japan.
    ///
    /// This update introduced the all-out spurt and wit limit break mechanics,
    /// increased stat caps and added a buff for front-runners when securing the
    /// lead.
    FifthAnniversary,
}

impl GameVersion {
    /// The most recent game version.
    pub const LATEST: GameVersion = GameVersion::FifthAnniversary;
}

/// Supported languages for track names, skill names, etc.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
pub enum Language {
    Japanese,
    English,
}

/// How a random check is resolved.
///
/// Used to force the outcome of random checks (e.g. wit checks or rushing)
/// for analysis purposes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
pub enum Roll {
    /// The check succeeds randomly with the given probability.
    #[default]
    Random,
    /// The check always succeeds.
    Always,
    /// The check always fails.
    Never,
}

impl Roll {
    /// Resolve a check that succeeds with probability `p`.
    ///
    /// The RNG is consumed even if the outcome is forced, so that changing the
    /// mode doesn't shift unrelated random outcomes for the same seed.
    pub fn resolve<R: Rng + ?Sized>(self, rng: &mut R, p: f64) -> bool {
        let roll = rng.random_bool(p);
        match self {
            Roll::Random => roll,
            Roll::Always => true,
            Roll::Never => false,
        }
    }
}

/// Runtime configuration shared by everything in a race.
///
/// Every field is optional when deserialized; missing fields take their
/// default value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(default, rename_all = "camelCase", deny_unknown_fields)]
pub struct Configuration {
    pub game_version: GameVersion,
    pub language: Language,
    /// Seed for pseudo-random number generation. Ensures reproducibility.
    ///
    /// If `None`, a random seed is used.
    pub seed: Option<u64>,
    /// How skill wit checks are resolved before the race.
    pub wit_checks: Roll,
    /// How runners' rush checks are resolved before the race.
    pub rushing: Roll,
    /// Fan count for runners that don't define their own.
    pub default_fan_count: u32,
}

impl Default for Configuration {
    fn default() -> Self {
        Configuration {
            game_version: GameVersion::LATEST,
            language: Language::English,
            seed: None,
            wit_checks: Roll::Random,
            rushing: Roll::Random,
            default_fan_count: 0,
        }
    }
}

impl Configuration {
    /// Parse `Configuration` from a JSON string.
    ///
    /// Returns an error if the JSON is invalid.
    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json)
    }

    /// Parse `Configuration` from a JSON file.
    ///
    /// Returns an error if the file can't be read or JSON is invalid.
    pub fn from_json_file(path: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let content = std::fs::read_to_string(path)?;
        Ok(Self::from_json(&content)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_missing_fields_take_default_values() {
        let config = Configuration::from_json(r#"{ "seed": 7, "rushing": "Never" }"#).unwrap();
        assert_eq!(
            config,
            Configuration {
                seed: Some(7),
                rushing: Roll::Never,
                ..Default::default()
            }
        );
    }

    #[test]
    fn test_unknown_fields_are_rejected() {
        assert!(Configuration::from_json(r#"{ "witCheck": "Always" }"#).is_err());
    }

    #[test]
    fn test_repository_config_file_is_valid() {
        Configuration::from_json_file("config.json").unwrap();
    }
}
