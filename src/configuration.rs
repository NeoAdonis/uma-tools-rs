//! Settings for the simulation behavior, game mechanics and language.
//!
//! # Examples
//!
//! Simulate races as they ran before a game update, e.g. to measure how much
//! the 5th Anniversary changed a runner's times. Versions are ordered, so
//! later ones include the changes of earlier ones:
//!
//! ```
//! use uma_tools::configuration::{Configuration, GameVersion};
//!
//! let before = Configuration {
//!     game_version: GameVersion::MechaUmaUpdate,
//!     seed: Some(564),
//!     ..Configuration::default()
//! };
//! let after = Configuration {
//!     game_version: GameVersion::FifthAnniversary,
//!     ..before
//! };
//! assert!(before.game_version < after.game_version);
//! ```

use rand::{Rng, RngExt};
use serde::de::{Error, IntoDeserializer};
use serde::{Deserialize, Deserializer};
use std::path::Path;

use crate::uma::Mood;

/// A version of the game, which determines the behavior of some mechanics.
///
/// This list is sorted in chronological order, from the original release to the
/// latest update, allowing to compare if a certain version is before or after a
/// certain update. The list is not exhaustive, and only includes the versions
/// that have a significant impact on the game mechanics.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Deserialize)]
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
    /// After the 2nd anniversary update in Japan (1 year after release in global).
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

/// Language to be used for track names, skill names, etc.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize)]
pub enum Language {
    Japanese,
    English,
}

/// How a random check is resolved.
///
/// Used to force the outcome of random checks (e.g. wit checks or rushing)
/// for analysis purposes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Deserialize)]
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
    /// Resolves a check that succeeds with probability `p`.
    ///
    /// The RNG is consumed even if the outcome is forced, so that changing the
    /// mode doesn't shift later draws of the same stream (e.g. a skill's trigger
    /// rolls after its wit check) for the same seed.
    ///
    /// # Panics
    ///
    /// Panics if `p` is not in `[0, 1]`, whatever the mode.
    pub(crate) fn resolve<R: Rng + ?Sized>(self, rng: &mut R, p: f64) -> bool {
        let roll = rng.random_bool(p);
        match self {
            Roll::Random => roll,
            Roll::Always => true,
            Roll::Never => false,
        }
    }
}

/// Method used to assign default values to runners (e.g. [`DefaultGate`] or
/// [`DefaultMood`]) when they get ready to race.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize)]
pub enum Assignment {
    /// Runners keep the values set for them, and the rest get the default.
    Unset,
    /// Every runner gets the default, even if a value was set for her.
    All,
}

/// Gate of the runners that get a default one (see
/// [`Configuration::gate_assignment`]).
///
/// Gates can be set for some runners (see [`Entrant::gate`]).
///
/// [`Entrant::gate`]: crate::Entrant::gate
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize)]
pub enum DefaultGate {
    /// The lowest free gates, in the order runners were added to the race.
    InOrder,
    /// Random free gates.
    Random,
}

/// Mood of the runners that get a default one (see
/// [`Configuration::mood_assignment`]).
///
/// In JSON, either a [`Mood`] (e.g. `"Great"`) or `"Random"`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DefaultMood {
    /// The given mood.
    Fixed(Mood),
    /// A random mood, every mood being equally likely.
    Random,
}

impl<'de> Deserialize<'de> for DefaultMood {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = String::deserialize(deserializer)?;
        if value == "Random" {
            return Ok(DefaultMood::Random);
        }
        let mood = Mood::deserialize(value.as_str().into_deserializer());
        let mood = mood.map_err(|_: D::Error| {
            let expected = "expected a mood or \"Random\"";
            D::Error::custom(format!("invalid default mood {value:?}: {expected}"))
        })?;
        Ok(DefaultMood::Fixed(mood))
    }
}

/// Runtime configuration shared by everything in a race.
///
/// Every field is optional when deserialized; missing fields take their
/// default value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize)]
#[serde(default, rename_all = "camelCase", deny_unknown_fields)]
pub struct Configuration {
    /// Version of the game whose mechanics are simulated.
    pub game_version: GameVersion,
    /// Language to be used for track names, skill names, etc.
    pub language: Language,
    /// Seed for pseudo-random number generation. Ensures reproducibility.
    ///
    /// If `None`, seed 0 is used. Apps that want varied races must pick a seed
    /// themselves.
    pub seed: Option<u64>,
    /// How the wit checks of skills that have one are resolved before the race,
    /// except for skills whose wit check is pinned (see
    /// [`SkillPin::WitCheck`]). Skills without one always pass unless pinned.
    ///
    /// [`SkillPin::WitCheck`]: crate::race::pins::SkillPin::WitCheck
    pub wit_checks: Roll,
    /// How runners' rush checks are resolved before the race, except for
    /// runners whose rushing is pinned (see [`RushPins::rushes`]).
    ///
    /// [`RushPins::rushes`]: crate::race::pins::RushPins::rushes
    pub rushing: Roll,
    /// Fan count for runners that don't define their own.
    pub default_fan_count: u32,
    /// Which runners get [`Self::default_gate`] (unset gates by default).
    pub gate_assignment: Assignment,
    /// Gate of the runners that get a default one (random by default).
    pub default_gate: DefaultGate,
    /// Which runners get [`Self::default_mood`] (every runner by default).
    pub mood_assignment: Assignment,
    /// Mood of the runners that get a default one (Great by default).
    pub default_mood: DefaultMood,
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
            gate_assignment: Assignment::Unset,
            default_gate: DefaultGate::Random,
            mood_assignment: Assignment::All,
            default_mood: DefaultMood::Fixed(Mood::Great),
        }
    }
}

impl Configuration {
    /// Parses `Configuration` from a JSON string.
    ///
    /// # Errors
    ///
    /// Returns an error if the JSON is invalid.
    ///
    /// # Examples
    ///
    /// Make every wit check pass, e.g. to measure the best case of a skill,
    /// while keeping the defaults for everything else:
    ///
    /// ```
    /// use uma_tools::configuration::{Configuration, Roll};
    ///
    /// # fn main() -> Result<(), serde_json::Error> {
    /// let json = r#"{ "seed": 564, "witChecks": "Always" }"#;
    /// let config = Configuration::from_json(json)?;
    /// assert_eq!(config.wit_checks, Roll::Always);
    /// assert_eq!(config.rushing, Roll::Random);
    /// # Ok(())
    /// # }
    /// ```
    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json)
    }

    /// Parses `Configuration` from a JSON file.
    ///
    /// # Errors
    ///
    /// Returns an error if the file can't be read or the JSON is invalid.
    ///
    /// See [`Configuration::from_json`] for an example.
    pub fn from_json_file(
        path: impl AsRef<Path>,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
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
    fn test_default_moods() {
        let cases = [
            (r#""Random""#, Some(DefaultMood::Random)),
            (r#""Awful""#, Some(DefaultMood::Fixed(Mood::Awful))),
            (r#""Fine""#, None),
        ];
        for (mood, expected) in cases {
            let json = format!(r#"{{ "defaultMood": {mood} }}"#);
            let actual = Configuration::from_json(&json).ok();
            assert_eq!(actual.map(|config| config.default_mood), expected, "{mood}");
        }
    }

    #[test]
    fn test_unknown_fields_are_rejected() {
        let error = Configuration::from_json(r#"{ "witCheck": "Always" }"#).unwrap_err();
        assert!(
            error.to_string().contains("unknown field `witCheck`"),
            "{error}"
        );
    }

    #[test]
    fn test_repository_config_file_is_valid() {
        Configuration::from_json_file("config.json").unwrap();
    }
}
