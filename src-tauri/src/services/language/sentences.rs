//! Sentence segmentation (section 14, "Satzende").
//!
//! A sentence ends at `.`, `?`, `!` or an ellipsis, followed by closing
//! quotes or brackets and then white space or the end of the paragraph.
//! `Intl.Segmenter` alone splits after `z.`, `Dr.`, `1.` and `ca.` and in
//! URLs, so a full stop is checked against an exception list per language:
//!
//! - A mark that is not followed by white space never ends a sentence
//!   (`3.14`, URLs, `z.B.`, `?!`).
//! - A single letter before the full stop is an abbreviation or an initial
//!   (`z. B.`, `d. h.`, `u. a.`, `J. R. R.`).
//! - Words from the list of the language (`Dr.`, `ca.`, `usw.`).
//! - In German a number of one or two digits is an ordinal (`1. Mai`,
//!   `der 3. Platz`); a longer number ends a sentence unless a month, a
//!   lower-case word or a digit follows (`am 2019. …` is rare,
//!   `im Jahr 2019. Danach` is not).
//! - An ellipsis ends a sentence only before an upper-case letter
//!   (`Er wartete … und ging.` is one sentence).
//! - After a closing quote a lower-case word continues the sentence
//!   (`„Kommst du?“, fragte sie.`, `„Wohin?“ fragte sie.`).
//!
//! The segmentation leans towards too few boundaries: a missed boundary
//! only delays the check of a sentence; a wrong one would flag the next
//! word for missing capitalisation.

use std::ops::Range;

const CLOSERS: &[char] = &['"', '\'', '“', '”', '‘', '’', '»', '«', ')', ']', '}'];
const OPENERS: &[char] = &['"', '\'', '„', '“', '‚', '‘', '»', '«', '(', '[', '{'];

struct Rules {
    abbreviations: &'static [&'static str],
    months: &'static [&'static str],
    ordinal_dot: bool,
}

const GERMAN: Rules = Rules {
    abbreviations: &[
        "Abb", "Abs", "Abt", "allg", "Anm", "Aufl", "Bd", "Bde", "betr", "bspw", "bzgl", "bzw",
        "ca", "chr", "Dipl", "Dr", "dt", "ebd", "eigtl", "engl", "etc", "evtl", "Fa", "ff", "Fr",
        "geb", "gegr", "gem", "gest", "ggf", "ggfs", "Hr", "Hrn", "Hrsg", "hrsg", "Ing", "inkl",
        "Jh", "Jhd", "jmd", "Kap", "kath", "lat", "lt", "max", "med", "Mio", "min", "Mrd", "mtl",
        "Nr", "od", "Pkt", "Prof", "Rel", "rd", "Red", "röm", "sog", "Std", "St", "Str", "Tab",
        "Tel", "Tsd", "usf", "usw", "verh", "vgl", "Vors", "zzgl", "zz", "zzt", "Jan", "Feb",
        "Febr", "Aug", "Sept", "Okt", "Nov", "Dez",
    ],
    months: &[
        "Januar",
        "Jänner",
        "Februar",
        "März",
        "April",
        "Mai",
        "Juni",
        "Juli",
        "August",
        "September",
        "Oktober",
        "November",
        "Dezember",
        "Jan",
        "Feb",
        "Mär",
        "Apr",
        "Jun",
        "Jul",
        "Aug",
        "Sep",
        "Sept",
        "Okt",
        "Nov",
        "Dez",
    ],
    ordinal_dot: true,
};

const ENGLISH: Rules = Rules {
    abbreviations: &[
        "Mr", "Mrs", "Ms", "Dr", "Prof", "Sr", "Jr", "St", "Mt", "vs", "etc", "approx", "Inc",
        "Ltd", "Co", "Corp", "No", "Nos", "Fig", "Figs", "Vol", "Vols", "Ch", "pp", "cf", "Jan",
        "Feb", "Mar", "Apr", "Jun", "Jul", "Aug", "Sep", "Sept", "Oct", "Nov", "Dec", "Gen", "Gov",
        "Sen", "Rep", "Rev", "Lt", "Col", "Capt", "Sgt", "dept", "est", "misc",
    ],
    months: &[],
    ordinal_dot: false,
};

const OTHER: Rules = Rules {
    abbreviations: &[],
    months: &[],
    ordinal_dot: false,
};

fn rules(language: &str) -> &'static Rules {
    let base = language.split(['-', '_']).next().unwrap_or_default();
    match base.to_ascii_lowercase().as_str() {
        "de" => &GERMAN,
        "en" => &ENGLISH,
        _ => &OTHER,
    }
}

fn is_terminator(c: char) -> bool {
    matches!(c, '.' | '?' | '!' | '…')
}

/// Splits a paragraph into sentences. Returns byte ranges into `text`
/// without leading and trailing white space; closing quotes and brackets
/// after the final mark belong to the sentence.
pub fn sentences(text: &str, language: &str) -> Vec<Range<usize>> {
    let rules = rules(language);
    let chars: Vec<(usize, char)> = text.char_indices().collect();
    let mut result = Vec::new();
    let mut start: Option<usize> = None;
    let mut i = 0;
    while i < chars.len() {
        let (offset, c) = chars[i];
        if start.is_none() {
            if c.is_whitespace() {
                i += 1;
                continue;
            }
            start = Some(offset);
        }
        if !is_terminator(c) {
            i += 1;
            continue;
        }
        let run_start = i;
        while i < chars.len() && is_terminator(chars[i].1) {
            i += 1;
        }
        let run: String = chars[run_start..i].iter().map(|&(_, c)| c).collect();
        let closers_start = i;
        while i < chars.len() && CLOSERS.contains(&chars[i].1) {
            i += 1;
        }
        let had_closers = i > closers_start;
        let end = chars.get(i).map_or(text.len(), |&(o, _)| o);
        let next = match chars.get(i) {
            None => None,
            Some(&(_, c)) if !c.is_whitespace() => continue,
            Some(_) => next_word(&chars[i..]),
        };
        let Some(next) = next else {
            // White space up to the end: the last sentence closes below.
            continue;
        };
        if ends_sentence(text, &chars, run_start, &run, had_closers, next, rules) {
            if let Some(s) = start.take() {
                result.push(s..end);
            }
        }
    }
    if let Some(s) = start {
        let end = s + text[s..].trim_end().len();
        if end > s {
            result.push(s..end);
        }
    }
    result
}

/// The word after the white space, with opening quotes skipped.
fn next_word(rest: &[(usize, char)]) -> Option<NextWord> {
    let mut chars = rest.iter().map(|&(_, c)| c).skip_while(|c| c.is_whitespace());
    let mut opened = false;
    let mut first = chars.next()?;
    while OPENERS.contains(&first) {
        opened = true;
        first = chars.next()?;
    }
    let word: String = std::iter::once(first)
        .chain(chars.take_while(|c| c.is_alphanumeric()))
        .collect();
    Some(NextWord { first, word, opened })
}

struct NextWord {
    first: char,
    word: String,
    opened: bool,
}

fn ends_sentence(
    text: &str,
    chars: &[(usize, char)],
    run_start: usize,
    run: &str,
    had_closers: bool,
    next: NextWord,
    rules: &Rules,
) -> bool {
    if had_closers && next.first.is_lowercase() && !next.opened {
        return false;
    }
    let ellipsis = run.contains('…') || run.starts_with("..");
    if ellipsis {
        return next.first.is_uppercase();
    }
    if run.contains('?') || run.contains('!') {
        return true;
    }
    // A single full stop: look at the word before it.
    let dot = chars[run_start].0;
    let token_start = text[..dot]
        .char_indices()
        .rev()
        .find(|&(_, c)| c.is_whitespace() || OPENERS.contains(&c))
        .map_or(0, |(o, c)| o + c.len_utf8());
    let token = &text[token_start..dot];
    let last = token.rsplit('.').next().unwrap_or_default();
    if last.chars().count() == 1 && last.chars().all(char::is_alphabetic) {
        return false;
    }
    if is_abbreviation(last, rules) {
        return false;
    }
    if !last.is_empty() && last.chars().all(|c| c.is_ascii_digit()) {
        return number_ends_sentence(last, &next, rules);
    }
    true
}

fn is_abbreviation(word: &str, rules: &Rules) -> bool {
    rules.abbreviations.iter().any(|&a| {
        a == word || {
            // `Vgl.` or `Ca.` at the start of a sentence.
            let mut chars = a.chars();
            chars.next().is_some_and(char::is_lowercase)
                && word.chars().next().is_some_and(char::is_uppercase)
                && word.to_lowercase() == a
        }
    })
}

fn number_ends_sentence(digits: &str, next: &NextWord, rules: &Rules) -> bool {
    if !rules.ordinal_dot {
        return true;
    }
    if rules.months.contains(&next.word.as_str())
        || next.first.is_lowercase()
        || next.first.is_ascii_digit()
    {
        return false;
    }
    digits.len() > 2
}

#[cfg(test)]
mod tests {
    use super::*;

    fn split<'a>(text: &'a str, language: &str) -> Vec<&'a str> {
        sentences(text, language)
            .into_iter()
            .map(|r| &text[r])
            .collect()
    }

    /// Test data (section 14): one case a line, the sentences separated by
    /// ` | `; the text is the sentences joined by a space.
    #[test]
    fn the_sentence_end_test_data_are_green() {
        let data = include_str!("../../../tests/data/language/satzende.txt");
        let mut cases = 0;
        let mut failures = Vec::new();
        for line in data.lines() {
            if line.trim().is_empty() || line.starts_with('#') {
                continue;
            }
            let (language, rest) = line.split_once('\t').expect("language and a tab");
            let expected: Vec<&str> = rest.split(" | ").collect();
            let text = expected.join(" ");
            let actual = split(&text, language);
            if actual != expected {
                failures.push(format!("{language}: {text:?}\n  got {actual:?}"));
            }
            cases += 1;
        }
        assert!(cases >= 40, "only {cases} cases");
        assert!(failures.is_empty(), "{}", failures.join("\n"));
    }

    #[test]
    fn white_space_around_and_between_sentences_is_left_out() {
        let text = "  Eins.   Zwei!\n\tDrei  ";
        assert_eq!(split(text, "de-DE"), ["Eins.", "Zwei!", "Drei"]);
        assert!(sentences("   ", "de").is_empty());
        assert!(sentences("", "de").is_empty());
    }

    #[test]
    fn ranges_are_byte_offsets_with_multi_byte_text() {
        let text = "Größe ändern. Übel!";
        let ranges = sentences(text, "de");
        assert_eq!(ranges, [0..16, 17..text.len()]);
    }

    #[test]
    fn a_no_break_space_after_an_abbreviation_is_white_space() {
        let text = "Das gilt z.\u{a0}B. hier. Dann nicht.";
        assert_eq!(split(text, "de"), ["Das gilt z.\u{a0}B. hier.", "Dann nicht."]);
    }

    #[test]
    fn unknown_languages_keep_the_general_rules() {
        assert_eq!(
            split("Un deux. Trois 3.14 quatre? Cinq", "fr"),
            ["Un deux.", "Trois 3.14 quatre?", "Cinq"]
        );
    }
}
