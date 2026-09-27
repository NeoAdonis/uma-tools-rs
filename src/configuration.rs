//! Configuration for the game mechanics and language settings.

/// Represents different versions of the game.
///
/// Used to determine the behavior for some of the mechanics. This list is
/// sorted in chronological order, from the original release to the latest
/// update, allowing to compare if a certain version is before or after a
/// certain update. The list is not exhaustive, and only includes the versions
/// that have a significant impact on the game mechanics.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum GameVersion {
    /// When the game was first released.
    Original,
    /// After the first anniversary update in Japan (Half-anniversary update in
    /// global).
    ///
    /// This update introduced spot struggles and stamina dueling mechanics,
    /// a buff to make the guts stat more useful, last spurt recalculation with
    /// HP changes, and a force-in modifier for lane changes.
    FirstAnniversary,
    /// After the 1.5 anniversary update in Japan.
    ///
    /// This update makes adjustments to positioning. It softened pace-down
    /// positioning in the mid-race, making the slowdown milder and the target
    /// gap to the pacemaker smaller, and let a lone front runner keep speeding
    /// up to stay ahead with a larger lead.
    OnePointFiveAnniversary,
    /// After the 2nd anniversary update in Japan.
    ///
    /// This update introduces charge up mechanics.
    SecondAnniversary,
    /// After the 2.5 anniversary update in Japan.
    ///
    /// This update introduced stamina limit breaks, compete before spurt,
    /// stamina keep and lead securing mechanics.
    TwoPointFiveAnniversary,
    /// After the Mecha Umamusume scenario release in Japan.
    ///
    /// This update introduced a buff to stamina limit breaks for long-distance
    /// courses.
    MechaUmaUpdate,
    /// After the 5th anniversary update in Japan.
    ///
    /// This update introduce full-throttle sprint and wiz limit break
    /// mechanics, increased stat caps and added a buff for front-runners when
    /// securing the lead.
    FifthAnniversary,
}

impl GameVersion {
    /// The most recent game version.
    pub const LATEST: GameVersion = GameVersion::FifthAnniversary;
}

/// Supported languages for track names, skill names, etc.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Language {
    Japanese,
    English,
}

/// Runtime configuration shared by everything in a race.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Configuration {
    pub game_version: GameVersion,
    pub language: Language,
}

impl Default for Configuration {
    fn default() -> Self {
        Configuration {
            game_version: GameVersion::LATEST,
            language: Language::English,
        }
    }
}
