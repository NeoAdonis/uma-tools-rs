/// Configuration settings for the game behavior.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum GameBehavior {
    /// When the game was first released
    Original,
    /// After the first anniversary update in Japan (Half-anniversary update in global)
    FirstAnniversary,
    /// After the 1.5 anniversary update in Japan
    OnePointFiveAnniversary,
}

pub static GAME_BEHAVIOR: GameBehavior = GameBehavior::FirstAnniversary;

/// Supported languages for track names, skill names, etc.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Language {
    Japanese,
    English,
}
