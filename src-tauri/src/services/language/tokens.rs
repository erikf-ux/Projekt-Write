//! The words of a text for the spelling check (section 14).
//!
//! - A word is a run of letters; a hyphen or an apostrophe between two
//!   letters belongs to it (`E-Mail-Adresse`, `geht's`).
//! - Not checked: web addresses and e-mail addresses (a chunk between
//!   spaces with `://`, `www.` or `@`), words next to a digit, `_`, `/` or
//!   `\` (`MP3`, `file_name`), single letters, and words next to masked
//!   text. The editor masks code, inline code and links with U+FFFC, one
//!   per UTF-16 unit, so the offsets stay the same.

/// What the editor puts in place of text that is not checked.
pub const MASK: char = '\u{FFFC}';

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Word<'a> {
    /// Byte range in the text.
    pub start: usize,
    pub end: usize,
    pub text: &'a str,
    /// A full stop follows directly (`usw.`): the word may be an
    /// abbreviation.
    pub dot: bool,
}

fn joins(c: char) -> bool {
    matches!(c, '-' | '\'' | '’' | '\u{2011}')
}

fn blocks(c: char) -> bool {
    c.is_numeric() || matches!(c, '_' | '/' | '\\' | MASK)
}

/// A chunk between spaces that is an address as a whole.
fn is_address(chunk: &str) -> bool {
    let trimmed = chunk.trim_start_matches(|c: char| !c.is_alphanumeric());
    chunk.contains("://") || chunk.contains('@') || trimmed.to_lowercase().starts_with("www.")
}

/// The words of `text` that the spelling check looks at.
pub fn words(text: &str) -> Vec<Word<'_>> {
    let mut result = Vec::new();
    let mut chunk_start = None;
    for (offset, c) in text
        .char_indices()
        .chain(std::iter::once((text.len(), ' ')))
    {
        match (chunk_start, c.is_whitespace()) {
            (None, false) => chunk_start = Some(offset),
            (Some(start), true) => {
                let chunk = &text[start..offset];
                if !is_address(chunk) {
                    chunk_words(text, start, chunk, &mut result);
                }
                chunk_start = None;
            }
            _ => {}
        }
    }
    result
}

fn chunk_words<'a>(text: &'a str, base: usize, chunk: &str, out: &mut Vec<Word<'a>>) {
    let chars: Vec<(usize, char)> = chunk.char_indices().collect();
    let mut i = 0;
    while i < chars.len() {
        if !chars[i].1.is_alphabetic() {
            i += 1;
            continue;
        }
        let first = i;
        let mut last = i;
        i += 1;
        while i < chars.len() {
            let c = chars[i].1;
            if c.is_alphabetic() || is_mark(c) {
                last = i;
                i += 1;
            } else if joins(c) && chars.get(i + 1).is_some_and(|&(_, n)| n.is_alphabetic()) {
                i += 1;
            } else {
                break;
            }
        }
        let start = chars[first].0;
        let end = chars[last].0 + chars[last].1.len_utf8();
        let before = first.checked_sub(1).map(|p| chars[p].1);
        let after = chars.get(last + 1).map(|&(_, c)| c);
        let blocked = before.is_some_and(blocks) || after.is_some_and(blocks);
        let word = &chunk[start..end];
        if !blocked && word.chars().count() > 1 {
            out.push(Word {
                start: base + start,
                end: base + end,
                text: &text[base + start..base + end],
                dot: after == Some('.'),
            });
        }
    }
}

/// Unicode combining marks count as part of a word (`e` + U+0301).
fn is_mark(c: char) -> bool {
    matches!(c as u32, 0x0300..=0x036F | 0x1AB0..=0x1AFF | 0x1DC0..=0x1DFF | 0x20D0..=0x20FF)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn texts(text: &str) -> Vec<&str> {
        words(text).into_iter().map(|w| w.text).collect()
    }

    #[test]
    fn words_with_hyphens_and_apostrophes_stay_together() {
        assert_eq!(
            texts("Die E-Mail-Adresse, geht's? Ein- und Ausgang."),
            ["Die", "E-Mail-Adresse", "geht's", "Ein", "und", "Ausgang"]
        );
        assert_eq!(texts("don’t stop"), ["don’t", "stop"]);
    }

    #[test]
    fn addresses_digits_and_single_letters_are_left_out() {
        assert_eq!(
            texts("Siehe https://example.org/Seite und www.Beispiel.de oder info@example.org."),
            ["Siehe", "und", "oder"]
        );
        assert_eq!(
            texts("MP3 und 3D, file_name, a/b, Kapitel 3"),
            ["und", "Kapitel"]
        );
        assert_eq!(texts("z. B. x y"), Vec::<&str>::new());
    }

    #[test]
    fn masked_text_is_not_checked_and_keeps_its_place() {
        let text = "Nimm \u{FFFC}\u{FFFC}\u{FFFC} oder Fooo\u{FFFC}\u{FFFC}.";
        let found = words(text);
        assert_eq!(
            found.iter().map(|w| w.text).collect::<Vec<_>>(),
            ["Nimm", "oder"]
        );
        assert_eq!(&text[found[1].start..found[1].end], "oder");
    }

    #[test]
    fn byte_ranges_and_the_following_full_stop() {
        let text = "Größe usw. Ende";
        let found = words(text);
        assert_eq!((found[0].start, found[0].end), (0, 7));
        assert!(!found[0].dot);
        assert_eq!(found[1].text, "usw");
        assert!(found[1].dot);
    }
}
