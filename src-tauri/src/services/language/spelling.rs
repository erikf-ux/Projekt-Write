//! The spelling check with Hunspell dictionaries through `spellbook`
//! (ADR 0023) and the order of suggestions of section 14.

use std::sync::Arc;

use spellbook::Dictionary;

use super::autocorrect::edit_distance;
use super::dictionaries::{DictionaryStore, base_language};
use super::engine::{EngineError, FindingKind, LanguageEngine, OwnWords, RawFinding, normalize};
use super::tokens;

/// Suggestions shown at most (section 14).
pub const MAX_SUGGESTIONS: usize = 5;
/// Own words further away than this are no suggestion.
const OWN_DISTANCE: usize = 2;

pub struct SpellingEngine {
    store: Arc<DictionaryStore>,
}

/// Whether a word is correct: in the own words or the dictionary; with a
/// following full stop also as an abbreviation (`usw.`); with hyphens if
/// every part of two letters or more is correct (`Max-Planck-Institut`);
/// with an apostrophe if the part before it is (`Peter's`).
pub fn is_correct(dictionary: &Dictionary, own: &OwnWords, word: &str, dot: bool) -> bool {
    let word = normalize(word);
    let known = |w: &str| own.contains(w) || dictionary.check(w);
    if known(&word) || (dot && dictionary.check(&format!("{word}."))) {
        return true;
    }
    if word.contains(['-', '\u{2011}']) {
        let parts: Vec<&str> = word
            .split(['-', '\u{2011}'])
            .filter(|p| p.chars().count() > 1)
            .collect();
        if !parts.is_empty() && parts.iter().all(|part| known(part)) {
            return true;
        }
    }
    if let Some((head, _)) = word.split_once('\'') {
        return head.chars().count() > 1 && known(head);
    }
    false
}

/// Spellings with umlauts and ß, up to two changes (`Fruehstueck` →
/// `Frühstück`, `Grosse` → `Größe`).
fn umlaut_variants(word: &str) -> Vec<String> {
    const PAIRS: [(&str, &str); 10] = [
        ("ae", "ä"),
        ("oe", "ö"),
        ("ue", "ü"),
        ("Ae", "Ä"),
        ("Oe", "Ö"),
        ("Ue", "Ü"),
        ("ss", "ß"),
        ("a", "ä"),
        ("o", "ö"),
        ("u", "ü"),
    ];
    let mut level = vec![word.to_owned()];
    let mut all: Vec<String> = Vec::new();
    for _ in 0..2 {
        let mut next = Vec::new();
        for current in &level {
            for (from, to) in PAIRS {
                let mut start = 0;
                while let Some(found) = current[start..].find(from) {
                    let at = start + found;
                    let changed =
                        format!("{}{}{}", &current[..at], to, &current[at + from.len()..]);
                    if changed != word && !all.contains(&changed) {
                        all.push(changed.clone());
                        next.push(changed);
                    }
                    start = at + from.len();
                }
            }
        }
        level = next;
    }
    all
}

/// The words one step away that the dictionary or the own words accept:
/// a letter deleted, two neighbours swapped, a letter replaced or inserted
/// (letters of the language), and in German one umlaut or ß written out
/// (`ue` for `ü`). Deterministic, unlike the suggestion lists of the
/// engine, whose order and cut-off can differ between runs: the autocorrect
/// decides on this (condition 3).
pub fn close_words(
    dictionary: &Dictionary,
    own: &OwnWords,
    word: &str,
    alphabet: &[char],
    german: bool,
) -> Vec<String> {
    let chars: Vec<char> = word.chars().collect();
    let mut candidates: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    let collect = |chars: Vec<char>| chars.into_iter().collect::<String>();
    for i in 0..chars.len() {
        let mut deleted = chars.clone();
        deleted.remove(i);
        candidates.insert(collect(deleted));
        if i + 1 < chars.len() {
            let mut swapped = chars.clone();
            swapped.swap(i, i + 1);
            candidates.insert(collect(swapped));
        }
        for &letter in alphabet {
            if letter != chars[i] {
                let mut replaced = chars.clone();
                replaced[i] = letter;
                candidates.insert(collect(replaced));
            }
        }
    }
    for i in 0..=chars.len() {
        for &letter in alphabet {
            let mut inserted = chars.clone();
            inserted.insert(i, letter);
            candidates.insert(collect(inserted));
        }
    }
    if german {
        for (from, to) in [
            ("ae", "ä"),
            ("oe", "ö"),
            ("ue", "ü"),
            ("Ae", "Ä"),
            ("Oe", "Ö"),
            ("Ue", "Ü"),
            ("ss", "ß"),
        ] {
            for (at, _) in word.match_indices(from) {
                candidates.insert(format!("{}{}{}", &word[..at], to, &word[at + from.len()..]));
            }
        }
    }
    candidates.remove(word);
    candidates
        .into_iter()
        .filter(|candidate| {
            !candidate.is_empty() && (own.contains(candidate) || dictionary.check(candidate))
        })
        .collect()
}

fn capitalised_like(word: &str, suggestion: &str) -> String {
    let starts_upper = word.chars().next().is_some_and(char::is_uppercase);
    let mut chars = suggestion.chars();
    match chars.next() {
        Some(first) if starts_upper && first.is_lowercase() => {
            first.to_uppercase().chain(chars).collect()
        }
        _ => suggestion.to_owned(),
    }
}

/// Section 14: umlaut and ß variants first, then own words, then words
/// without a space or hyphen, then the rest. The list shown is cut to
/// [`MAX_SUGGESTIONS`]; the autocorrect looks at all of them.
pub fn order_suggestions(
    word: &str,
    language: &str,
    dictionary: &Dictionary,
    own: &OwnWords,
    found: Vec<String>,
) -> Vec<String> {
    let mut ordered: Vec<String> = Vec::new();
    let push = |candidate: String, ordered: &mut Vec<String>| {
        if candidate != word && !ordered.contains(&candidate) {
            ordered.push(candidate);
        }
    };
    if base_language(language) == "de" {
        for variant in umlaut_variants(word) {
            if dictionary.check(&variant) {
                push(variant, &mut ordered);
            }
        }
    }
    let lower = word.to_lowercase();
    let mut close: Vec<(usize, &String)> = own
        .iter()
        .map(|entry| (edit_distance(&lower, &entry.to_lowercase()), entry))
        .filter(|&(distance, _)| distance > 0 && distance <= OWN_DISTANCE)
        .collect();
    close.sort();
    for (_, entry) in close {
        push(capitalised_like(word, entry), &mut ordered);
    }
    let (single, joined): (Vec<String>, Vec<String>) = found
        .into_iter()
        .partition(|s| !s.contains(char::is_whitespace) && !s.contains('-'));
    for candidate in single.into_iter().chain(joined) {
        push(candidate, &mut ordered);
    }
    ordered
}

impl SpellingEngine {
    pub fn new(store: Arc<DictionaryStore>) -> Self {
        Self { store }
    }

    pub fn store(&self) -> &Arc<DictionaryStore> {
        &self.store
    }

    fn dictionary(&self, language: &str) -> Result<(String, Arc<Dictionary>), EngineError> {
        let tag = self
            .store
            .resolve(language)
            .ok_or_else(|| EngineError::NoDictionary(language.to_owned()))?;
        let dictionary = self.store.get(&tag)?;
        Ok((tag, dictionary))
    }

    /// Whether a single word is correct in `language`.
    pub fn check_word(
        &self,
        word: &str,
        language: &str,
        own: &OwnWords,
    ) -> Result<bool, EngineError> {
        let (_, dictionary) = self.dictionary(language)?;
        Ok(is_correct(&dictionary, own, word, false))
    }

    /// Suggestions for a word in the order of section 14. Takes up to
    /// about 350 ms: always off the input path.
    pub fn suggest(
        &self,
        word: &str,
        language: &str,
        own: &OwnWords,
    ) -> Result<Vec<String>, EngineError> {
        let (tag, dictionary) = self.dictionary(language)?;
        let word = normalize(word);
        let mut found = Vec::new();
        dictionary.suggest(&word, &mut found);
        let mut ordered = order_suggestions(&word, &tag, &dictionary, own, found);
        ordered.truncate(MAX_SUGGESTIONS);
        Ok(ordered)
    }
}

impl LanguageEngine for SpellingEngine {
    fn name(&self) -> &'static str {
        "spellbook"
    }

    fn check(
        &self,
        text: &str,
        language: &str,
        own: &OwnWords,
    ) -> Result<Vec<RawFinding>, EngineError> {
        let (_, dictionary) = self.dictionary(language)?;
        Ok(tokens::words(text)
            .into_iter()
            .filter(|word| !is_correct(&dictionary, own, word.text, word.dot))
            .map(|word| RawFinding {
                start: word.start,
                end: word.end,
                kind: FindingKind::Spelling,
                rule: "spelling".to_owned(),
                message: None,
                replacements: Vec::new(),
            })
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    fn engine() -> SpellingEngine {
        let bundled = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("dictionaries");
        SpellingEngine::new(Arc::new(DictionaryStore::new(
            Some(bundled),
            PathBuf::from("/nonexistent"),
        )))
    }

    fn flagged(engine: &SpellingEngine, text: &str, language: &str, own: &OwnWords) -> Vec<String> {
        engine
            .check(text, language, own)
            .unwrap()
            .into_iter()
            .map(|f| text[f.start..f.end].to_owned())
            .collect()
    }

    #[test]
    fn german_text_with_compounds_abbreviations_and_hyphens() {
        let engine = engine();
        let none = OwnWords::default();
        let text = "Die E-Mail-Adresse des Max-Planck-Instituts steht usw. im Wörterbuch, geht's?";
        assert_eq!(flagged(&engine, text, "de-DE", &none), Vec::<String>::new());
        assert_eq!(
            flagged(&engine, "Das Fesnter steht im Worterbuch.", "de-DE", &none),
            ["Fesnter", "Worterbuch"]
        );
    }

    #[test]
    fn swiss_german_writes_ss_and_english_has_its_own_dictionaries() {
        let engine = engine();
        let none = OwnWords::default();
        assert_eq!(
            flagged(&engine, "Die Strasse ist gross.", "de-CH", &none),
            Vec::<String>::new()
        );
        assert_eq!(
            flagged(&engine, "Die Strasse ist gross.", "de-DE", &none),
            ["Strasse", "gross"]
        );
        assert_eq!(
            flagged(&engine, "The colour of the centre.", "en-GB", &none),
            Vec::<String>::new()
        );
        assert_eq!(
            flagged(&engine, "The colour of the centre.", "en-US", &none),
            ["colour", "centre"]
        );
    }

    #[test]
    fn own_words_count_as_correct_and_come_first_after_umlauts() {
        let engine = engine();
        let own = OwnWords::new(["Flohmarktstand".to_owned(), "Tauri".to_owned()]);
        assert_eq!(
            flagged(&engine, "Am Flohmarktstand mit Tauri.", "de-DE", &own),
            Vec::<String>::new()
        );
        assert_eq!(
            flagged(&engine, "Am Flohmarktstand.", "de-DE", &OwnWords::default()),
            ["Flohmarktstand"]
        );
        let suggestions = engine.suggest("Flohmarkstand", "de-DE", &own).unwrap();
        assert_eq!(
            suggestions.first().map(String::as_str),
            Some("Flohmarktstand")
        );
    }

    #[test]
    fn umlaut_and_sharp_s_spellings_are_suggested_first() {
        let engine = engine();
        let none = OwnWords::default();
        for (word, expected) in [
            ("Fruehstueck", "Frühstück"),
            ("Gruesse", "Grüße"),
            ("Tur", "Tür"),
        ] {
            let suggestions = engine.suggest(word, "de-DE", &none).unwrap();
            assert_eq!(
                suggestions.first().map(String::as_str),
                Some(expected),
                "{word}: {suggestions:?}"
            );
            assert!(suggestions.len() <= MAX_SUGGESTIONS);
        }
    }

    #[test]
    fn close_words_are_found_without_the_suggestion_engine() {
        let store = engine();
        let dictionary = store.store.get("de-DE").unwrap();
        let alphabet = store.store.alphabet("de-DE").unwrap();
        let none = OwnWords::default();
        // Swapped letters or one deleted: two words, no unique correction.
        assert_eq!(
            close_words(&dictionary, &none, "Fesnter", &alphabet, true),
            ["Fenster", "Fester"]
        );
        assert_eq!(
            close_words(&dictionary, &none, "Zimemr", &alphabet, true),
            ["Zimmer"]
        );
        let mutter = close_words(&dictionary, &none, "Muetter", &alphabet, true);
        assert!(
            mutter.contains(&"Mütter".to_owned()) && mutter.contains(&"Mutter".to_owned()),
            "{mutter:?}"
        );
        let own = OwnWords::new(["Tauri".to_owned()]);
        assert_eq!(
            close_words(&dictionary, &own, "Tuari", &alphabet, true),
            ["Tauri"]
        );
    }

    #[test]
    fn a_missing_dictionary_is_reported() {
        let engine = engine();
        assert!(matches!(
            engine.check("bonjour", "fr-FR", &OwnWords::default()),
            Err(EngineError::NoDictionary(_))
        ));
    }
}
