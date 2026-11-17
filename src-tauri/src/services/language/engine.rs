//! The interface every checking engine sits behind (section 7, section 14):
//! the spelling check with Hunspell dictionaries, the simple rules and, as
//! an add-on, LanguageTool.

use std::collections::HashSet;

use serde::Serialize;
use ts_rs::TS;

/// What a finding is; the editor draws spelling and context red and wavy,
/// grammar and punctuation blue and dashed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum FindingKind {
    Spelling,
    Grammar,
    Punctuation,
    Context,
}

/// A finding in byte offsets of the checked text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RawFinding {
    pub start: usize,
    pub end: usize,
    pub kind: FindingKind,
    /// `spelling`, `doubleWord`, `doubleSpace`, `sentenceStart` or the rule
    /// of LanguageTool.
    pub rule: String,
    /// A message from LanguageTool, in the language of the text; the
    /// interface words its own rules itself.
    pub message: Option<String>,
    /// Replacements known right away (the rules, LanguageTool). Spelling
    /// suggestions are asked for separately: they take up to 350 ms.
    pub replacements: Vec<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum EngineError {
    #[error("no dictionary for {0}")]
    NoDictionary(String),
    #[error("the dictionary for {0} cannot be read: {1}")]
    BadDictionary(String, String),
    #[error("the add-on does not answer: {0}")]
    Unavailable(String),
}

/// The user's own words that apply to a document: the union of the
/// dictionaries of the app, the document, its subnets and their parents
/// (section 14). A lower-case word also counts at the start of a sentence,
/// any word also in capitals, as in Hunspell.
#[derive(Debug, Clone, Default)]
pub struct OwnWords {
    words: HashSet<String>,
}

impl OwnWords {
    pub fn new<I: IntoIterator<Item = String>>(words: I) -> Self {
        Self {
            words: words
                .into_iter()
                .map(|word| normalize(&word))
                .filter(|word| !word.is_empty())
                .collect(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.words.is_empty()
    }

    pub fn len(&self) -> usize {
        self.words.len()
    }

    pub fn iter(&self) -> impl Iterator<Item = &String> {
        self.words.iter()
    }

    pub fn contains(&self, word: &str) -> bool {
        let word = normalize(word);
        if self.words.contains(&word) {
            return true;
        }
        let mut chars = word.chars();
        let Some(first) = chars.next() else {
            return false;
        };
        if first.is_uppercase() {
            let lowered: String = first.to_lowercase().chain(chars.clone()).collect();
            if self.words.contains(&lowered) {
                return true;
            }
        }
        if word.chars().any(char::is_lowercase) {
            return false;
        }
        // All capitals: any entry with the same letters.
        let lower = word.to_lowercase();
        self.words.iter().any(|entry| entry.to_lowercase() == lower)
    }
}

/// Apostrophes as in the dictionaries.
pub fn normalize(word: &str) -> String {
    word.trim().replace('’', "'")
}

/// One checking engine.
pub trait LanguageEngine: Send + Sync {
    /// For logs and the settings.
    fn name(&self) -> &'static str;
    /// Findings in `text`, written in `language` (a tag like `de-DE`).
    fn check(
        &self,
        text: &str,
        language: &str,
        own: &OwnWords,
    ) -> Result<Vec<RawFinding>, EngineError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn own_words_count_capitalised_and_in_capitals() {
        let own = OwnWords::new(["flohmarktstand".to_owned(), "Tauri".to_owned()]);
        assert!(own.contains("flohmarktstand"));
        assert!(own.contains("Flohmarktstand"));
        assert!(own.contains("FLOHMARKTSTAND"));
        assert!(own.contains("Tauri"));
        assert!(own.contains("TAURI"));
        assert!(!own.contains("tauri"));
        assert!(!own.contains("Flohmarkt"));
        let quoted = OwnWords::new(["geht’s".to_owned()]);
        assert!(quoted.contains("geht's"));
    }
}
