//! Language check (section 14, phase 12).
//!
//! The engines sit behind [`engine::LanguageEngine`]: the spelling check
//! with Hunspell dictionaries (`spellbook`, ADR 0023), the simple rules and,
//! as an add-on, LanguageTool. The check runs paragraph by paragraph on the
//! blocking pool, never on the input path; the editor keeps a cache and
//! maps the findings back to its positions (UTF-16 offsets).

pub mod autocorrect;
pub mod detect;
pub mod dictionaries;
pub mod engine;
pub mod languagetool;
pub mod own;
pub mod rules;
pub mod sentences;
pub mod spelling;
pub mod tokens;

use std::sync::Arc;

use serde::Serialize;
use ts_rs::TS;

use self::autocorrect::{Blocked, Candidate, Lexicon, Span};
use self::dictionaries::{DictionaryStore, LanguageDictionary};
use self::engine::{EngineError, FindingKind, LanguageEngine, OwnWords, RawFinding};
use self::spelling::{MAX_SUGGESTIONS, SpellingEngine, is_correct, order_suggestions};

/// A finding as the editor gets it: UTF-16 offsets into the checked text.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Finding {
    pub start: u32,
    pub end: u32,
    pub kind: FindingKind,
    pub rule: String,
    pub message: Option<String>,
    pub replacements: Vec<String>,
}

/// The result for one text: the dictionary used (none when the language has
/// no dictionary) and the findings.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct CheckedText {
    pub language: String,
    pub dictionary: Option<String>,
    pub findings: Vec<Finding>,
}

/// Which parts of the check run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CheckParts {
    pub spelling: bool,
    pub rules: bool,
}

/// UTF-16 offsets of byte offsets in `text` (any order).
fn to_utf16(text: &str, offsets: &[usize]) -> Vec<u32> {
    let mut sorted: Vec<(usize, usize)> = offsets
        .iter()
        .copied()
        .enumerate()
        .map(|(i, o)| (o, i))
        .collect();
    sorted.sort_unstable();
    let mut result = vec![0u32; offsets.len()];
    let mut units = 0u32;
    let mut chars = text.char_indices().peekable();
    for (offset, index) in sorted {
        while let Some(&(at, c)) = chars.peek() {
            if at >= offset {
                break;
            }
            units += u32::try_from(c.len_utf16()).unwrap_or(2);
            chars.next();
        }
        result[index] = units;
    }
    result
}

fn convert(text: &str, raw: Vec<RawFinding>) -> Vec<Finding> {
    let offsets: Vec<usize> = raw.iter().flat_map(|f| [f.start, f.end]).collect();
    let units = to_utf16(text, &offsets);
    raw.into_iter()
        .enumerate()
        .map(|(i, f)| Finding {
            start: units[2 * i],
            end: units[2 * i + 1],
            kind: f.kind,
            rule: f.rule,
            message: f.message,
            replacements: f.replacements,
        })
        .collect()
}

pub struct LanguageService {
    store: Arc<DictionaryStore>,
    spelling: SpellingEngine,
}

/// The dictionaries and own words of one check, for the autocorrect rule.
struct Words<'a> {
    service: &'a LanguageService,
    languages: &'a [String],
    own: &'a OwnWords,
}

impl Lexicon for Words<'_> {
    fn is_correct(&self, word: &str) -> bool {
        self.languages.iter().any(|language| {
            self.service
                .spelling
                .check_word(word, language, self.own)
                .unwrap_or(true)
        })
    }

    /// The candidates one step away, found deterministically (not the
    /// suggestion list of the engine, which can differ between runs).
    fn suggestions(&self, word: &str) -> Vec<String> {
        let Some(language) = self.languages.first() else {
            return Vec::new();
        };
        let Some(tag) = self.service.store.resolve(language) else {
            return Vec::new();
        };
        let (Ok(dictionary), Ok(alphabet)) = (
            self.service.store.get(&tag),
            self.service.store.alphabet(&tag),
        ) else {
            return Vec::new();
        };
        let german = dictionaries::base_language(&tag) == "de";
        spelling::close_words(&dictionary, self.own, word, &alphabet, german)
    }
}

impl LanguageService {
    pub fn new(store: Arc<DictionaryStore>) -> Self {
        Self {
            spelling: SpellingEngine::new(Arc::clone(&store)),
            store,
        }
    }

    pub fn store(&self) -> &Arc<DictionaryStore> {
        &self.store
    }

    pub fn languages(&self) -> Vec<LanguageDictionary> {
        self.store.languages()
    }

    /// Checks one text in one language. A language without a dictionary
    /// only gets the rules. A word that is correct in one of `also` (the
    /// other languages of the document's profile) is not flagged.
    pub fn check(
        &self,
        text: &str,
        language: &str,
        also: &[String],
        own: &OwnWords,
        parts: CheckParts,
        addon: Option<&dyn LanguageEngine>,
    ) -> CheckedText {
        let dictionary = self.store.resolve(language);
        let mut raw = Vec::new();
        if parts.spelling && dictionary.is_some() {
            match self.spelling.check(text, language, own) {
                Ok(found) => raw.extend(found.into_iter().filter(|finding| {
                    let word = &text[finding.start..finding.end];
                    !also
                        .iter()
                        .filter(|other| other.as_str() != language)
                        .any(|other| self.is_correct(word, other, own).unwrap_or(false))
                })),
                Err(error) => log::warn!("spelling check failed: {error}"),
            }
        }
        if parts.rules {
            raw.extend(rules::check(text, language));
        }
        // The add-on (LanguageTool): grammar, punctuation, context. A place
        // a simple rule already found is not underlined twice.
        if let Some(addon) = addon {
            match addon.check(text, language, own) {
                Ok(found) => {
                    let known: Vec<(usize, usize)> = raw.iter().map(|f| (f.start, f.end)).collect();
                    raw.extend(found.into_iter().filter(|f| {
                        !known
                            .iter()
                            .any(|&(start, end)| f.start < end && start < f.end)
                    }));
                }
                Err(error) => log::warn!("{} failed: {error}", addon.name()),
            }
        }
        raw.sort_by_key(|f| (f.start, f.end));
        CheckedText {
            language: language.to_owned(),
            dictionary,
            findings: convert(text, raw),
        }
    }

    /// All suggestions in the order of section 14, not cut to five.
    fn all_suggestions(
        &self,
        word: &str,
        language: &str,
        own: &OwnWords,
    ) -> Result<Vec<String>, EngineError> {
        let tag = self
            .store
            .resolve(language)
            .ok_or_else(|| EngineError::NoDictionary(language.to_owned()))?;
        let dictionary = self.store.get(&tag)?;
        let word = engine::normalize(word);
        let mut found = Vec::new();
        dictionary.suggest(&word, &mut found);
        Ok(order_suggestions(&word, &tag, &dictionary, own, found))
    }

    /// At most five suggestions for a word (section 14).
    pub fn suggest(
        &self,
        word: &str,
        language: &str,
        own: &OwnWords,
    ) -> Result<Vec<String>, EngineError> {
        let mut all = self.all_suggestions(word, language, own)?;
        all.truncate(MAX_SUGGESTIONS);
        Ok(all)
    }

    /// Whether a word is correct in a language.
    pub fn is_correct(
        &self,
        word: &str,
        language: &str,
        own: &OwnWords,
    ) -> Result<bool, EngineError> {
        let tag = self
            .store
            .resolve(language)
            .ok_or_else(|| EngineError::NoDictionary(language.to_owned()))?;
        let dictionary = self.store.get(&tag)?;
        Ok(is_correct(&dictionary, own, word, false))
    }

    /// The uniqueness rule of the autocorrect for a finished word.
    /// `languages`: the active languages of the section, the first one
    /// suggests; `rejected(word, correction)` answers condition 7.
    pub fn autocorrect(
        &self,
        candidate: Candidate<'_>,
        languages: &[String],
        own: &OwnWords,
        rejected: &dyn Fn(&str, &str) -> bool,
    ) -> Result<String, Blocked> {
        if languages
            .iter()
            .all(|language| self.store.resolve(language).is_none())
        {
            // No dictionary: nothing is known to be wrong.
            return Err(Blocked::Correct);
        }
        let words = Words {
            service: self,
            languages,
            own,
        };
        autocorrect::correction(candidate, &words, rejected)
    }
}

/// Where a word stands, as the editor tells it (condition 6).
pub fn span_of(code: bool, link: bool, quote: bool, other_language: bool) -> Span {
    if code {
        Span::Code
    } else if link {
        Span::Link
    } else if quote {
        Span::Quote
    } else if other_language {
        Span::OtherLanguage
    } else {
        Span::Text
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    pub(crate) fn service() -> LanguageService {
        let bundled = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("dictionaries");
        LanguageService::new(Arc::new(DictionaryStore::new(
            Some(bundled),
            PathBuf::from("/nonexistent"),
        )))
    }

    const ALL: CheckParts = CheckParts {
        spelling: true,
        rules: true,
    };

    #[test]
    fn findings_come_in_utf_16_offsets() {
        let text = "😀 Größe Fesnter  gut.";
        let checked = service().check(text, "de-DE", &[], &OwnWords::default(), ALL, None);
        let units: Vec<u16> = text.encode_utf16().collect();
        let shown: Vec<String> = checked
            .findings
            .iter()
            .map(|f| String::from_utf16_lossy(&units[f.start as usize..f.end as usize]))
            .collect();
        assert_eq!(shown, ["Fesnter", "  "]);
        assert_eq!(checked.dictionary.as_deref(), Some("de-DE"));
    }

    #[test]
    fn a_language_without_dictionary_gets_the_rules_only() {
        let checked = service().check(
            "bonjour  le monde",
            "fr-FR",
            &[],
            &OwnWords::default(),
            ALL,
            None,
        );
        assert_eq!(checked.dictionary, None);
        assert_eq!(checked.findings.len(), 2, "{:?}", checked.findings);
    }

    #[test]
    fn parts_can_be_switched_off() {
        let only_rules = CheckParts {
            spelling: false,
            rules: true,
        };
        let checked = service().check(
            "Das Fesnter  ist offen.",
            "de-DE",
            &[],
            &OwnWords::default(),
            only_rules,
            None,
        );
        assert_eq!(
            checked
                .findings
                .iter()
                .map(|f| f.rule.as_str())
                .collect::<Vec<_>>(),
            ["doubleSpace"]
        );
    }

    #[test]
    fn a_word_of_another_profile_language_is_not_flagged() {
        let text = "Das Spreadsheet war kurz.";
        let none = OwnWords::default();
        let alone = service().check(text, "de-DE", &[], &none, ALL, None);
        assert_eq!(alone.findings.len(), 1);
        let both = service().check(text, "de-DE", &["en-GB".to_owned()], &none, ALL, None);
        assert!(both.findings.is_empty());
    }

    /// Stands in for LanguageTool: one grammar finding and one on a double
    /// space the rules already have.
    struct Addon;

    impl LanguageEngine for Addon {
        fn name(&self) -> &'static str {
            "test"
        }
        fn check(
            &self,
            _text: &str,
            _language: &str,
            _own: &OwnWords,
        ) -> Result<Vec<RawFinding>, EngineError> {
            Ok(vec![
                RawFinding {
                    start: 4,
                    end: 9,
                    kind: FindingKind::Grammar,
                    rule: "lt:AGREEMENT".to_owned(),
                    message: Some("Kongruenz".to_owned()),
                    replacements: vec!["geht".to_owned()],
                },
                RawFinding {
                    start: 9,
                    end: 11,
                    kind: FindingKind::Punctuation,
                    rule: "lt:WHITESPACE".to_owned(),
                    message: None,
                    replacements: vec![" ".to_owned()],
                },
            ])
        }
    }

    #[test]
    fn the_add_on_adds_findings_but_none_twice() {
        let checked = service().check(
            "Ich gehen  heim.",
            "de-DE",
            &[],
            &OwnWords::default(),
            ALL,
            Some(&Addon),
        );
        let rules: Vec<&str> = checked.findings.iter().map(|f| f.rule.as_str()).collect();
        assert_eq!(rules, ["lt:AGREEMENT", "doubleSpace"]);
        assert_eq!(checked.findings[0].message.as_deref(), Some("Kongruenz"));
    }

    #[test]
    fn utf_16_offsets_of_unordered_byte_offsets() {
        assert_eq!(to_utf16("a😀b", &[5, 0, 1, 6]), [3, 0, 1, 4]);
    }
}
