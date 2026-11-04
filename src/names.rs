//! Names of characters, skills and tracks in every supported language.
//!
//! Names come from `data/names.json` (see `scripts/make_names.pl`), embedded
//! at compile time and parsed on first use.
//!
//! # Examples
//!
//! ```
//! use uma_tools::configuration::Language;
//! use uma_tools::names;
//!
//! assert_eq!(names::track_name(10006, Language::English), Some("Tokyo"));
//! assert_eq!(
//!     names::skill_name(200331, Language::English),
//!     Some("Professor of Curvature")
//! );
//! assert_eq!(
//!     names::character_name(100101, Language::English),
//!     Some("[Special Dreamer] Special Week")
//! );
//! ```

use serde::Deserialize;
use std::collections::HashMap;
use std::hash::Hash;
use std::sync::LazyLock;

use crate::configuration::Language;

/// Names in Japanese and English, in that order. Empty if unknown.
type NamePair = [String; 2];

/// Every name in `data/names.json`, by kind, then by ID.
#[derive(Deserialize)]
struct Names {
    characters: HashMap<u32, NamePair>,
    skills: HashMap<i32, NamePair>,
    tracks: HashMap<u16, NamePair>,
}

static NAMES: LazyLock<Names> = LazyLock::new(|| {
    serde_json::from_str(include_str!("../data/names.json")).expect("names.json must be valid")
});

/// Name with the given ID in `names`, or `None` if the ID or its name in that
/// language is unknown.
fn name<K: Eq + Hash>(names: &HashMap<K, NamePair>, id: K, language: Language) -> Option<&str> {
    let [japanese, english] = names.get(&id)?;
    let name = match language {
        Language::Japanese => japanese,
        Language::English => english,
    };
    (!name.is_empty()).then_some(name.as_str())
}

/// Get the name of a character card (outfit and Uma) by card ID.
pub fn character_name(card_id: u32, language: Language) -> Option<&'static str> {
    name(&NAMES.characters, card_id, language)
}

/// Get the name of a skill by skill ID.
pub fn skill_name(skill_id: i32, language: Language) -> Option<&'static str> {
    name(&NAMES.skills, skill_id, language)
}

/// Get the name of a racetrack by track ID.
pub fn track_name(track_id: u16, language: Language) -> Option<&'static str> {
    name(&NAMES.tracks, track_id, language)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_names_by_language() {
        assert_eq!(track_name(10001, Language::Japanese), Some("札幌"));
        assert_eq!(track_name(10001, Language::English), Some("Sapporo"));
        assert_eq!(
            skill_name(200332, Language::Japanese),
            Some("コーナー巧者○")
        );
        assert_eq!(
            skill_name(200332, Language::English),
            Some("Corner Adept ○")
        );
        assert_eq!(
            character_name(100201, Language::Japanese),
            Some("[サイレントイノセンス]サイレンススズカ")
        );
        assert_eq!(
            character_name(100201, Language::English),
            Some("[Innocent Silence] Silence Suzuka")
        );
    }

    #[test]
    fn test_unknown_names_are_none() {
        // A known ID, with no English name yet.
        let names = HashMap::from([(1, ["名前".to_string(), String::new()])]);
        assert_eq!(name(&names, 1, Language::Japanese), Some("名前"));
        assert_eq!(name(&names, 1, Language::English), None);
        assert_eq!(name(&names, 2, Language::Japanese), None);
    }
}
