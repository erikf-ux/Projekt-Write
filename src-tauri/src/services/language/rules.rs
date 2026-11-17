//! The simple rules that work without the add-on (section 14): repeated
//! words, double spaces and a capital letter at the start of a sentence.
//!
//! - **Repeated word:** the same word twice, only white space between.
//!   Not flagged where the repetition is good grammar: German relative
//!   clauses (`die die Kinder`, `der der Frau`, `das das Kind`), `sie sie`;
//!   English `had had`, `that that`.
//! - **Double space:** two or more spaces between words.
//! - **Sentence start:** the first word of a sentence (sentence
//!   segmentation with exceptions, `sentences.rs`) starts with a small
//!   letter. Not flagged: words with a capital inside (`iPhone`, `eBay`),
//!   sentences that start with masked text, a digit or a symbol.

use super::dictionaries::base_language;
use super::engine::{FindingKind, RawFinding};
use super::sentences::sentences;
use super::tokens::MASK;

const GERMAN_REPEATABLE: &[&str] = &[
    "der", "die", "das", "den", "dem", "des", "sie", "ihr", "wir",
];
const ENGLISH_REPEATABLE: &[&str] = &["had", "that", "is"];

fn repeatable(language: &str) -> &'static [&'static str] {
    match base_language(language) {
        "de" => GERMAN_REPEATABLE,
        "en" => ENGLISH_REPEATABLE,
        _ => &[],
    }
}

/// Words with their byte ranges: runs of letters and digits.
fn plain_words(text: &str) -> Vec<(usize, usize)> {
    let mut words = Vec::new();
    let mut start = None;
    for (offset, c) in text
        .char_indices()
        .chain(std::iter::once((text.len(), ' ')))
    {
        let inside = c.is_alphanumeric() || c == '\'' || c == '’';
        match (start, inside) {
            (None, true) => start = Some(offset),
            (Some(s), false) => {
                words.push((s, offset));
                start = None;
            }
            _ => {}
        }
    }
    words
}

fn repeated_words(text: &str, language: &str, out: &mut Vec<RawFinding>) {
    let allowed = repeatable(language);
    let words = plain_words(text);
    for pair in words.windows(2) {
        let (a, b) = (pair[0], pair[1]);
        let gap = &text[a.1..b.0];
        if gap.is_empty() || !gap.chars().all(|c| c == ' ' || c == '\u{a0}') {
            continue;
        }
        let first = &text[a.0..a.1];
        let second = &text[b.0..b.1];
        // `die die Kinder` is grammar; `Das Das` at a sentence start is not.
        let grammatical = first == second && allowed.contains(&second);
        if first.to_lowercase() != second.to_lowercase()
            || first.chars().any(|c| c.is_numeric())
            || grammatical
        {
            continue;
        }
        out.push(RawFinding {
            start: a.1,
            end: b.1,
            kind: FindingKind::Grammar,
            rule: "doubleWord".to_owned(),
            message: None,
            replacements: vec![String::new()],
        });
    }
}

fn double_spaces(text: &str, out: &mut Vec<RawFinding>) {
    let bytes = text.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] != b' ' {
            i += 1;
            continue;
        }
        let start = i;
        while i < bytes.len() && bytes[i] == b' ' {
            i += 1;
        }
        let between_words = start > 0 && i < bytes.len() && !text[..start].ends_with(['\n', '\t']);
        if i - start >= 2 && between_words {
            out.push(RawFinding {
                start,
                end: i,
                kind: FindingKind::Punctuation,
                rule: "doubleSpace".to_owned(),
                message: None,
                replacements: vec![" ".to_owned()],
            });
        }
    }
}

fn sentence_starts(text: &str, language: &str, out: &mut Vec<RawFinding>) {
    for range in sentences(text, language) {
        let sentence = &text[range.clone()];
        let Some((offset, first)) = sentence.char_indices().find(|&(_, c)| {
            !matches!(
                c,
                '"' | '\'' | '„' | '“' | '‚' | '‘' | '»' | '«' | '(' | '['
            )
        }) else {
            continue;
        };
        if first == MASK || !first.is_lowercase() {
            continue;
        }
        let word_end = sentence[offset..]
            .char_indices()
            .find(|&(_, c)| !c.is_alphabetic())
            .map_or(sentence.len(), |(o, _)| offset + o);
        let word = &sentence[offset..word_end];
        let rest = &sentence[word_end..];
        // `iPhone`, `eBay`; words running into a web address or code.
        if word.chars().skip(1).any(char::is_uppercase)
            || rest.starts_with([MASK, '.', '@', '/', ':', '_'])
            || rest.chars().next().is_some_and(char::is_numeric)
        {
            continue;
        }
        let start = range.start + offset;
        let end = range.start + word_end;
        let mut chars = word.chars();
        let capital: String = chars
            .next()
            .map(|c| c.to_uppercase().chain(chars).collect())
            .unwrap_or_default();
        out.push(RawFinding {
            start,
            end,
            kind: FindingKind::Grammar,
            rule: "sentenceStart".to_owned(),
            message: None,
            replacements: vec![capital],
        });
    }
}

/// The findings of the simple rules, sorted by position.
pub fn check(text: &str, language: &str) -> Vec<RawFinding> {
    let mut out = Vec::new();
    repeated_words(text, language, &mut out);
    double_spaces(text, &mut out);
    sentence_starts(text, language, &mut out);
    out.sort_by_key(|f| (f.start, f.end));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn found(text: &str, language: &str) -> Vec<(String, String, Vec<String>)> {
        check(text, language)
            .into_iter()
            .map(|f| (f.rule, text[f.start..f.end].to_owned(), f.replacements))
            .collect()
    }

    #[test]
    fn a_repeated_word_is_flagged_and_its_repetition_offered_for_removal() {
        assert_eq!(
            found("Das ist ist gut.", "de-DE"),
            [(
                "doubleWord".to_owned(),
                " ist".to_owned(),
                vec![String::new()]
            )]
        );
        assert_eq!(found("Er sagte: Das Das war gut.", "de-DE").len(), 1);
    }

    #[test]
    fn grammatical_repetitions_stay() {
        for text in [
            "Die Frauen, die die Kinder betreuen, kommen.",
            "Der Mann, der der Frau hilft, geht.",
            "Ob sie sie kennt?",
            "Er sagte, ist, ist.",
            "Seite 12 12 Mal.",
        ] {
            assert!(found(text, "de-DE").is_empty(), "{text}");
        }
        assert!(found("She had had enough.", "en-GB").is_empty());
    }

    #[test]
    fn double_spaces_between_words_only() {
        assert_eq!(
            found("Ein  Satz mit   Lücken.", "de-DE"),
            [
                (
                    "doubleSpace".to_owned(),
                    "  ".to_owned(),
                    vec![" ".to_owned()]
                ),
                (
                    "doubleSpace".to_owned(),
                    "   ".to_owned(),
                    vec![" ".to_owned()]
                ),
            ]
        );
        assert!(found("  Eingerückt.", "de-DE").is_empty());
        assert!(found("Ende.  ", "de-DE").is_empty());
    }

    #[test]
    fn a_sentence_starts_with_a_capital() {
        assert_eq!(
            found("Das ist gut. dann ging er. „wohin?“, fragte sie.", "de-DE"),
            [
                (
                    "sentenceStart".to_owned(),
                    "dann".to_owned(),
                    vec!["Dann".to_owned()]
                ),
                (
                    "sentenceStart".to_owned(),
                    "wohin".to_owned(),
                    vec!["Wohin".to_owned()]
                ),
            ]
        );
        assert_eq!(found("über alles.", "de-DE")[0].2, ["Über"]);
    }

    #[test]
    fn sentence_starts_that_stay_small() {
        for text in [
            "iPhone und eBay sind Namen.",
            "Das gilt z. B. hier. Und dort.",
            "\u{FFFC}\u{FFFC}\u{FFFC} ist ein Befehl.",
            "3 Äpfel. 4 Birnen.",
            "www.example.org ist eine Seite.",
            "Am 1. Mai ist frei.",
        ] {
            assert!(
                found(text, "de-DE").is_empty(),
                "{text}: {:?}",
                found(text, "de-DE")
            );
        }
    }
}
