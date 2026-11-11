//! Turns what the user typed into FTS5 queries (section 19, ADR 0020).
//!
//! * Rust builds every query itself: each word is quoted (`"` doubled), so
//!   `C++` or a lone `"` never reach FTS5 as syntax.
//! * Each word is widened by its spellings: `ß`/`ss`, `ä`/`ae`, `ö`/`oe`,
//!   `ü`/`ue`. `Strasse` becomes `"strasse"* OR "straße"*`.
//! * Words query (`search_words`): every word must match, as a word or the
//!   beginning of one. A word that ends in a sign (`C++`) must match as a
//!   whole word: its beginning alone (`c`) would match nearly everything.
//! * A word with a sign in it (`C++`, `E-Mail`) loses the sign in the words
//!   index (`C++` becomes `c`). With three or more characters it must also
//!   occur as typed in title or body (`exact`, looked up in the trigram
//!   index); otherwise `C++` would find every text with a lone `C`.
//! * Parts query (`search_parts`, trigram): every word of three or more
//!   characters must occur inside a word; shorter words must still match
//!   in the words index (FTS5's trigram cannot look for them).

use crate::database::fts::quote_term;

/// More words than this are ignored.
pub const MAX_TERMS: usize = 16;
/// Characters of one word; longer words are cut.
pub const MAX_TERM_CHARS: usize = 100;
/// Spellings of one word at most (the word itself included).
pub const MAX_SPELLINGS: usize = 8;
/// The trigram tokenizer needs three characters.
const MIN_PART_CHARS: usize = 3;

const SPELLINGS: [(&str, &str); 4] = [("ß", "ss"), ("ä", "ae"), ("ö", "oe"), ("ü", "ue")];

/// The FTS5 expressions of one search. `None` means the index has nothing
/// to look for.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Expressions {
    /// For `search_words`.
    pub words: Option<String>,
    /// For `search_parts`.
    pub parts: Option<String>,
    /// Words below three characters, for `search_words`, that parts hits
    /// must match as well.
    pub short_words: Option<String>,
    /// Words with a sign, for `search_parts`, that words hits must contain
    /// as typed.
    pub exact: Option<String>,
}

impl Expressions {
    pub fn is_empty(&self) -> bool {
        self.words.is_none()
    }
}

/// The words of a search: split at white space, in lower case, without
/// repeats, and only words with a letter or digit.
pub fn terms(input: &str) -> Vec<String> {
    let mut terms: Vec<String> = Vec::new();
    for raw in input.split_whitespace() {
        let term: String = raw.to_lowercase().chars().take(MAX_TERM_CHARS).collect();
        if term.chars().any(char::is_alphanumeric) && !terms.contains(&term) {
            terms.push(term);
        }
        if terms.len() == MAX_TERMS {
            break;
        }
    }
    terms
}

/// The spellings of a word (in lower case), the word itself first.
pub fn spellings(term: &str) -> Vec<String> {
    let mut found = vec![term.to_owned()];
    for (special, plain) in SPELLINGS {
        let mut added = Vec::new();
        for spelling in &found {
            if spelling.contains(special) {
                added.push(spelling.replace(special, plain));
            }
            if spelling.contains(plain) {
                added.push(spelling.replace(plain, special));
            }
        }
        for spelling in added {
            if found.len() == MAX_SPELLINGS {
                break;
            }
            if !found.contains(&spelling) {
                found.push(spelling);
            }
        }
    }
    found
}

fn word_group(term: &str) -> String {
    // A word that ends in a sign matches as a whole word only.
    let prefix = term.chars().last().is_some_and(char::is_alphanumeric);
    let alternatives: Vec<String> = spellings(term)
        .iter()
        .map(|spelling| {
            let quoted = quote_term(spelling);
            if prefix { format!("{quoted}*") } else { quoted }
        })
        .collect();
    format!("({})", alternatives.join(" OR "))
}

fn part_group(term: &str) -> String {
    let alternatives: Vec<String> = spellings(term)
        .iter()
        .filter(|spelling| spelling.chars().count() >= MIN_PART_CHARS)
        .map(|spelling| quote_term(spelling))
        .collect();
    format!("({})", alternatives.join(" OR "))
}

fn join(groups: Vec<String>) -> Option<String> {
    (!groups.is_empty()).then(|| groups.join(" AND "))
}

/// The expressions for what the user typed.
pub fn expressions(input: &str) -> Expressions {
    let terms = terms(input);
    let (long, short): (Vec<&String>, Vec<&String>) = terms
        .iter()
        .partition(|term| term.chars().count() >= MIN_PART_CHARS);
    let parts = join(long.iter().map(|term| part_group(term)).collect());
    let exact = join(
        long.iter()
            .filter(|term| !term.chars().all(char::is_alphanumeric))
            .map(|term| part_group(term))
            .collect(),
    );
    Expressions {
        exact,
        words: join(terms.iter().map(|term| word_group(term)).collect()),
        short_words: parts
            .as_ref()
            .and(join(short.iter().map(|term| word_group(term)).collect())),
        parts,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spellings_cover_sharp_s_and_umlauts_both_ways() {
        assert_eq!(spellings("strasse"), ["strasse", "straße"]);
        assert_eq!(spellings("straße"), ["straße", "strasse"]);
        assert_eq!(spellings("mueller"), ["mueller", "müller"]);
        assert_eq!(spellings("müller"), ["müller", "mueller"]);
        assert_eq!(spellings("haus"), ["haus"]);
        let both = spellings("muellerstrasse");
        assert!(both.contains(&"müllerstraße".to_owned()), "{both:?}");
        assert!(both.len() <= MAX_SPELLINGS);
    }

    #[test]
    fn many_umlauts_stay_below_the_limit() {
        assert_eq!(spellings("äöüßaeoeuess").len(), MAX_SPELLINGS);
    }

    #[test]
    fn terms_are_lower_case_without_repeats_and_signs_only() {
        assert_eq!(
            terms("  Haus  HAUS \" ++ C++ Straße "),
            ["haus", "c++", "straße"]
        );
        assert!(terms("\"").is_empty());
        assert_eq!(terms(&"a ".repeat(40)).len(), 1);
        let many: String = (0..40).map(|i| format!("w{i} ")).collect();
        assert_eq!(terms(&many).len(), MAX_TERMS);
        assert_eq!(terms(&"x".repeat(500))[0].chars().count(), MAX_TERM_CHARS);
    }

    #[test]
    fn words_are_quoted_and_widened() {
        let built = expressions("Strasse C++");
        assert_eq!(
            built.words.as_deref(),
            Some(r#"("strasse"* OR "straße"*) AND ("c++")"#)
        );
        assert_eq!(
            built.parts.as_deref(),
            Some(r#"("strasse" OR "straße") AND ("c++")"#)
        );
        assert_eq!(built.short_words, None);
        assert_eq!(built.exact.as_deref(), Some(r#"("c++")"#));
        assert_eq!(expressions("haus c#").exact, None);
    }

    #[test]
    fn a_quote_is_doubled() {
        let built = expressions("a\"b");
        assert_eq!(built.words.as_deref(), Some(r#"("a""b"*)"#));
        assert_eq!(built.parts.as_deref(), Some(r#"("a""b")"#));
    }

    #[test]
    fn short_words_only_search_words() {
        let built = expressions("ab");
        assert_eq!(built.words.as_deref(), Some(r#"("ab"*)"#));
        assert_eq!(built.parts, None);
        assert_eq!(built.short_words, None);

        let mixed = expressions("ab haus");
        assert_eq!(mixed.parts.as_deref(), Some(r#"("haus")"#));
        assert_eq!(mixed.short_words.as_deref(), Some(r#"("ab"*)"#));
    }

    #[test]
    fn nothing_to_look_for() {
        assert!(expressions("").is_empty());
        assert!(expressions("\"").is_empty());
        assert!(expressions(" + - * ").is_empty());
    }
}
