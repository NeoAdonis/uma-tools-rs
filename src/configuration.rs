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

pub static GAME_BEHAVIOR: GameBehavior = GameBehavior::OnePointFiveAnniversary;

/// Supported languages for track names, skill names, etc.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Language {
    Japanese,
    English,
}
