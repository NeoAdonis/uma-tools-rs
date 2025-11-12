/// Configuration settings for the game behavior.
pub enum GameBehavior {
    /// When the game was first released
    Original,
    /// After the first anniversary update in Japan (Half-anniversary update in global)
    FirstAnniversary,
}

pub static GAME_BEHAVIOR: GameBehavior = GameBehavior::FirstAnniversary;

/// Supported languages for the track names, skill names, etc.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Language {
    Japanese,
    English,
}
