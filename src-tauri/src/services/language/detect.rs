//! Language detection per paragraph (section 14, "Sprachverwaltung"), local
//! with `whatlang`.
//!
//! Which language a part of a text is checked in:
//!
//! 1. A part marked with a language (`lang`) keeps it.
//! 2. The detection of the paragraph, when it is switched on and sure
//!    enough: at least [`MIN_LETTERS`] letters and a confidence of at
//!    least [`MIN_CONFIDENCE`]. A
//!    detected language becomes the matching language of the document's
//!    profile (`de` → `de-CH` in a Swiss document), then the default
//!    language, then any dictionary of it, else the bare language (no
//!    dictionary: only the rules apply). Very short paragraphs inherit the
//!    language of the paragraph before them.
//! 3. The profile of the document.
//! 4. The default language of the app.

use super::dictionaries::{DictionaryStore, base_language};
use super::tokens::MASK;

/// Paragraphs with fewer letters are not detected; they inherit.
pub const MIN_LETTERS: usize = 40;
/// The confidence a detection needs. `whatlang`'s own "reliable" is
/// stricter and refuses plain English sentences (0.72); a wrong detection
/// costs little, because a word correct in a language of the document's
/// profile is never flagged.
pub const MIN_CONFIDENCE: f64 = 0.6;

/// ISO 639-3 codes of `whatlang` and their ISO 639-1 forms, for the
/// languages likely to have Hunspell dictionaries.
const CODES: &[(&str, &str)] = &[
    ("deu", "de"),
    ("eng", "en"),
    ("fra", "fr"),
    ("spa", "es"),
    ("ita", "it"),
    ("nld", "nl"),
    ("por", "pt"),
    ("dan", "da"),
    ("swe", "sv"),
    ("nob", "nb"),
    ("fin", "fi"),
    ("pol", "pl"),
    ("ces", "cs"),
    ("slk", "sk"),
    ("slv", "sl"),
    ("hrv", "hr"),
    ("hun", "hu"),
    ("ron", "ro"),
    ("bul", "bg"),
    ("rus", "ru"),
    ("ukr", "uk"),
    ("ell", "el"),
    ("tur", "tr"),
    ("lit", "lt"),
    ("lav", "lv"),
    ("est", "et"),
    ("cat", "ca"),
    ("epo", "eo"),
    ("lat", "la"),
];

/// The language of a paragraph (ISO 639-1), when the detection is sure.
pub fn detect(text: &str) -> Option<&'static str> {
    let clean: String = text
        .chars()
        .map(|c| if c == MASK { ' ' } else { c })
        .collect();
    if clean.chars().filter(|c| c.is_alphabetic()).count() < MIN_LETTERS {
        return None;
    }
    let info = whatlang::detect(&clean)?;
    if info.confidence() < MIN_CONFIDENCE {
        return None;
    }
    let code = info.lang().code();
    CODES
        .iter()
        .find(|(three, _)| *three == code)
        .map(|(_, two)| *two)
}

/// The tag a detected language is checked in (rule 2 above).
pub fn choose(
    detected: &str,
    profile: &[String],
    default: &str,
    store: &DictionaryStore,
) -> String {
    if let Some(tag) = profile.iter().find(|tag| base_language(tag) == detected) {
        return tag.clone();
    }
    if base_language(default) == detected {
        return default.to_owned();
    }
    store
        .resolve(detected)
        .unwrap_or_else(|| detected.to_owned())
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    fn store() -> DictionaryStore {
        DictionaryStore::new(
            Some(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("dictionaries")),
            PathBuf::from("/nonexistent"),
        )
    }

    #[test]
    fn paragraphs_of_some_length_are_detected() {
        assert_eq!(
            detect("Die Sprachprüfung erkennt die Sprache eines Absatzes, wenn er lang genug ist."),
            Some("de")
        );
        assert_eq!(
            detect(
                "The language check detects the language of a paragraph when it is long enough."
            ),
            Some("en")
        );
        assert_eq!(
            detect("La vérification reconnaît la langue d'un paragraphe quand il est assez long."),
            Some("fr")
        );
        assert_eq!(detect("Kurz und knapp."), None);
        assert_eq!(detect(&"\u{FFFC}".repeat(200)), None);
    }

    #[test]
    fn a_detected_language_takes_the_variant_of_the_profile() {
        let store = store();
        let swiss = vec!["de-CH".to_owned(), "en-GB".to_owned()];
        assert_eq!(choose("de", &swiss, "de-DE", &store), "de-CH");
        assert_eq!(choose("en", &swiss, "de-DE", &store), "en-GB");
        assert_eq!(choose("en", &[], "de-DE", &store), "en-US");
        assert_eq!(choose("de", &[], "de-AT", &store), "de-AT");
        assert_eq!(choose("fr", &[], "de-DE", &store), "fr");
    }
}
