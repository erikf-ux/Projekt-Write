//! Uniqueness rule of the autocorrect (section 14, "Eindeutigkeitsregel").
//!
//! A misspelt word is corrected automatically only if all seven conditions
//! hold:
//!
//! 1. The autocorrect is on for this document.
//! 2. The word is in no active dictionary and correct in no active
//!    language of the section.
//! 3. There is exactly one one-word suggestion at edit distance 1. A swap
//!    of two neighbouring letters counts as one step; suggestions with a
//!    space or a hyphen do not count.
//! 4. The word has at least 4 characters.
//! 5. It contains no digits, no capital inside the word and none of `@`,
//!    `/`, `_` or `.`, and it is not all capitals.
//! 6. It is not in code, a link, a quotation or a section marked with
//!    another language.
//! 7. The user has not rejected this correction before.
//!
//! The rule only decides; the caller runs it after the word is finished,
//! asynchronously, and drops the result when the word has changed since.

/// Where the word stands (condition 6).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Span {
    Text,
    Code,
    Link,
    Quote,
    OtherLanguage,
}

/// The dictionaries and languages that apply to the section of the word:
/// in the app the engines behind `LanguageEngine` and the user's
/// dictionaries.
pub trait Lexicon {
    /// Condition 2: whether the word is correct in any of them.
    fn is_correct(&self, word: &str) -> bool;
    /// The suggestions for the word, best first.
    fn suggestions(&self, word: &str) -> Vec<String>;
}

/// The word and what the editor knows about it.
#[derive(Debug, Clone, Copy)]
pub struct Candidate<'a> {
    pub word: &'a str,
    /// Condition 1: the autocorrect is on globally and for the document.
    pub enabled: bool,
    pub span: Span,
}

/// Why a word is not corrected automatically. `condition()` gives the
/// number from section 14. The cheap conditions are checked first, so with
/// several failing the one reported is not always the lowest number.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Blocked {
    /// 1
    Disabled,
    /// 2
    Correct,
    /// 3: no or more than one one-word suggestion at edit distance 1.
    NotUnique,
    /// 4
    TooShort,
    /// 5
    WordShape,
    /// 6
    MarkedSpan,
    /// 7
    Rejected,
}

impl Blocked {
    pub fn condition(self) -> u8 {
        match self {
            Blocked::Disabled => 1,
            Blocked::Correct => 2,
            Blocked::NotUnique => 3,
            Blocked::TooShort => 4,
            Blocked::WordShape => 5,
            Blocked::MarkedSpan => 6,
            Blocked::Rejected => 7,
        }
    }
}

pub const MIN_LENGTH: usize = 4;

/// Returns the correction, or the first condition that blocks it. The
/// cheap conditions are checked before the dictionaries are asked;
/// `rejected(word, correction)` answers condition 7.
pub fn correction(
    candidate: Candidate<'_>,
    lexicon: &dyn Lexicon,
    rejected: &dyn Fn(&str, &str) -> bool,
) -> Result<String, Blocked> {
    let word = candidate.word;
    if !candidate.enabled {
        return Err(Blocked::Disabled);
    }
    if word.chars().count() < MIN_LENGTH {
        return Err(Blocked::TooShort);
    }
    if !plain_word(word) {
        return Err(Blocked::WordShape);
    }
    if candidate.span != Span::Text {
        return Err(Blocked::MarkedSpan);
    }
    if lexicon.is_correct(word) {
        return Err(Blocked::Correct);
    }
    let correction = unique_suggestion(word, &lexicon.suggestions(word))
        .ok_or(Blocked::NotUnique)?
        .to_owned();
    if rejected(word, &correction) {
        return Err(Blocked::Rejected);
    }
    Ok(correction)
}

/// Condition 5.
pub fn plain_word(word: &str) -> bool {
    if word
        .chars()
        .any(|c| c.is_numeric() || matches!(c, '@' | '/' | '_' | '.'))
    {
        return false;
    }
    let letters: Vec<char> = word.chars().filter(|c| c.is_alphabetic()).collect();
    if !letters.is_empty() && letters.iter().all(|c| c.is_uppercase()) {
        return false;
    }
    // A capital after the first letter: `iPhone`, `PostgreSQL`, `McDonald`.
    !word.chars().skip(1).any(char::is_uppercase)
}

/// Condition 3: the only one-word suggestion at edit distance 1.
pub fn unique_suggestion<'a>(word: &str, suggestions: &'a [String]) -> Option<&'a str> {
    let mut found: Option<&str> = None;
    for suggestion in suggestions {
        if suggestion.contains(char::is_whitespace) || suggestion.contains('-') {
            continue;
        }
        if edit_distance(word, suggestion) != 1 {
            continue;
        }
        match found {
            None => found = Some(suggestion),
            Some(previous) if previous == suggestion => {}
            Some(_) => return None,
        }
    }
    found
}

/// Edit distance over characters with swaps of neighbours as one step
/// (optimal string alignment).
pub fn edit_distance(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    let mut rows = vec![vec![0usize; b.len() + 1]; a.len() + 1];
    for (i, row) in rows.iter_mut().enumerate() {
        row[0] = i;
    }
    for j in 0..=b.len() {
        rows[0][j] = j;
    }
    for i in 1..=a.len() {
        for j in 1..=b.len() {
            let cost = usize::from(a[i - 1] != b[j - 1]);
            let mut best = (rows[i - 1][j] + 1)
                .min(rows[i][j - 1] + 1)
                .min(rows[i - 1][j - 1] + cost);
            if i > 1 && j > 1 && a[i - 1] == b[j - 2] && a[i - 2] == b[j - 1] {
                best = best.min(rows[i - 2][j - 2] + 1);
            }
            rows[i][j] = best;
        }
    }
    rows[a.len()][b.len()]
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A small dictionary standing in for the engines.
    struct Words {
        correct: &'static [&'static str],
        suggestions: &'static [(&'static str, &'static [&'static str])],
    }

    impl Lexicon for Words {
        fn is_correct(&self, word: &str) -> bool {
            self.correct.contains(&word)
        }
        fn suggestions(&self, word: &str) -> Vec<String> {
            self.suggestions
                .iter()
                .find(|(w, _)| *w == word)
                .map(|(_, s)| s.iter().map(|s| (*s).to_owned()).collect())
                .unwrap_or_default()
        }
    }

    const WORDS: Words = Words {
        correct: &["Haus", "Mutter", "Fenster"],
        suggestions: &[
            ("Fesnter", &["Fenster", "Fenster-", "Fest er"]),
            ("Hasu", &["Haus", "Hase u"]),
            ("Mitter", &["Mutter", "Mittler", "Ritter"]),
            ("Worterbuch", &["Wörterbuch", "Worterbtuch"]),
            ("Strasse", &["Straße", "Trasse"]),
            ("Ifone", &["Ifon", "Ifon e"]),
            ("Fenstr", &["Fenster"]),
        ],
    };

    fn never(_: &str, _: &str) -> bool {
        false
    }

    fn text(word: &str) -> Candidate<'_> {
        Candidate {
            word,
            enabled: true,
            span: Span::Text,
        }
    }

    fn check(candidate: Candidate<'_>) -> Result<String, Blocked> {
        correction(candidate, &WORDS, &never)
    }

    #[test]
    fn a_unique_typo_is_corrected() {
        assert_eq!(check(text("Fesnter")).as_deref(), Ok("Fenster"));
        assert_eq!(check(text("Fenstr")).as_deref(), Ok("Fenster"));
        assert_eq!(check(text("Hasu")).as_deref(), Ok("Haus"));
    }

    #[test]
    fn condition_1_the_autocorrect_is_off_for_the_document() {
        let off = Candidate {
            enabled: false,
            ..text("Fesnter")
        };
        assert_eq!(check(off), Err(Blocked::Disabled));
        assert_eq!(Blocked::Disabled.condition(), 1);
    }

    #[test]
    fn condition_2_a_correct_word_is_never_changed() {
        for word in WORDS.correct {
            assert_eq!(check(text(word)), Err(Blocked::Correct));
        }
        assert_eq!(Blocked::Correct.condition(), 2);
    }

    #[test]
    fn condition_3_needs_exactly_one_one_word_suggestion_at_distance_1() {
        // Two candidates at distance 1: Mutter and Ritter.
        assert_eq!(check(text("Mitter")), Err(Blocked::NotUnique));
        // ö for o is one step, but the second suggestion is one too.
        assert_eq!(check(text("Worterbuch")), Err(Blocked::NotUnique));
        // ß for ss is two steps.
        assert_eq!(check(text("Strasse")), Err(Blocked::NotUnique));
        // No suggestion at all.
        assert_eq!(check(text("Qwertz")), Err(Blocked::NotUnique));
        assert_eq!(Blocked::NotUnique.condition(), 3);
    }

    #[test]
    fn condition_3_ignores_suggestions_with_a_space_or_a_hyphen() {
        let suggestions = ["Fest er".to_owned(), "Fenster-".to_owned()];
        assert_eq!(unique_suggestion("Fester", &suggestions), None);
        assert_eq!(
            unique_suggestion("Fesnter", &["Fenster".to_owned(), "Fe nster".to_owned()]),
            Some("Fenster")
        );
    }

    #[test]
    fn condition_4_short_words_are_left_alone() {
        assert_eq!(check(text("Hsu")), Err(Blocked::TooShort));
        assert_eq!(check(text("äöü")), Err(Blocked::TooShort));
        assert_eq!(Blocked::TooShort.condition(), 4);
    }

    #[test]
    fn condition_5_digits_inner_capitals_symbols_and_capitals_only() {
        for word in [
            "Fenst3r", "iFenster", "FensTer", "Fen@ster", "Fen/ster", "Fen_ster", "Fen.ster",
            "FENSTR", "ABCD",
        ] {
            assert_eq!(check(text(word)), Err(Blocked::WordShape), "{word}");
        }
        assert!(plain_word("Straße"));
        assert!(plain_word("Fesnter"));
        assert_eq!(Blocked::WordShape.condition(), 5);
    }

    #[test]
    fn condition_6_code_links_quotations_and_other_languages() {
        for span in [Span::Code, Span::Link, Span::Quote, Span::OtherLanguage] {
            let candidate = Candidate {
                span,
                ..text("Fesnter")
            };
            assert_eq!(check(candidate), Err(Blocked::MarkedSpan), "{span:?}");
        }
        assert_eq!(Blocked::MarkedSpan.condition(), 6);
    }

    #[test]
    fn condition_7_a_rejected_correction_is_not_made_again() {
        let rejected = |word: &str, correction: &str| word == "Fesnter" && correction == "Fenster";
        assert_eq!(
            correction(text("Fesnter"), &WORDS, &rejected),
            Err(Blocked::Rejected)
        );
        assert_eq!(
            correction(text("Hasu"), &WORDS, &rejected).as_deref(),
            Ok("Haus")
        );
        assert_eq!(Blocked::Rejected.condition(), 7);
    }

    #[test]
    fn the_edit_distance_counts_a_swap_as_one_step() {
        assert_eq!(edit_distance("Fesnter", "Fenster"), 1);
        assert_eq!(edit_distance("Fenstr", "Fenster"), 1);
        assert_eq!(edit_distance("Fensster", "Fenster"), 1);
        assert_eq!(edit_distance("Fanster", "Fenster"), 1);
        assert_eq!(edit_distance("Strasse", "Straße"), 2);
        assert_eq!(edit_distance("Tür", "Tur"), 1);
        assert_eq!(edit_distance("", "abc"), 3);
        assert_eq!(edit_distance("gleich", "gleich"), 0);
    }
}
