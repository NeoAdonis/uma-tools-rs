/// Configuration settings for the game behavior.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum GameBehavior {
    /// When the game was first released
    Original,
    /// After the first anniversary update in Japan (Half-anniversary update in global)
    FirstAnniversary,
    /// After the 1.5 anniversary update in Japan
    OnePointFiveAnniversary,
    /// After the 2.5 anniversary update in Japan
    TwoPointFiveAnniversary,
    /// After the Mecha Umamusume scenario release in Japan (2024-10-29)
    MechaUmaUpdate,
    /// After the 5th anniversary update in Japan
    FifthAnniversary,
}

impl GameBehavior {
    /// The most recent game behavior.
    pub const LATEST: GameBehavior = GameBehavior::FifthAnniversary;
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
    pub game_behavior: GameBehavior,
    pub language: Language,
}

impl Default for Configuration {
    fn default() -> Self {
        Configuration {
            game_behavior: GameBehavior::LATEST,
            language: Language::English,
        }
    }
}
