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
fn name<K: Eq + Hash>(
    names: &'static HashMap<K, NamePair>,
    id: K,
    language: Language,
) -> Option<&'static str> {
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
        assert_eq!(track_name(10006, Language::Japanese), Some("東京"));
        assert_eq!(track_name(10006, Language::English), Some("Tokyo"));
        assert_eq!(
            skill_name(200331, Language::Japanese),
            Some("弧線のプロフェッサー")
        );
        assert_eq!(
            character_name(100101, Language::Japanese),
            Some("[スペシャルドリーマー]スペシャルウィーク")
        );
    }

    #[test]
    fn test_inherited_unique_names_have_a_suffix() {
        let unique = skill_name(100011, Language::English).unwrap();
        let inherited = skill_name(900011, Language::English).unwrap();
        assert_eq!(inherited, format!("{unique} (inherited)"));
    }

    #[test]
    fn test_unknown_names_are_none() {
        assert_eq!(track_name(0, Language::English), None);
        assert_eq!(skill_name(0, Language::English), None);
        // Known card, but no English name yet.
        assert_eq!(character_name(100303, Language::English), None);
        assert!(character_name(100303, Language::Japanese).is_some());
    }
}
