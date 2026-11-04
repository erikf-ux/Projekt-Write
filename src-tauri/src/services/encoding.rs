//! Encodings and line endings of text files (section 9: "Erkenne Kodierung
//! (UTF-8, UTF-8 mit BOM, UTF-16, Windows-1252) und Zeilenenden (LF, CRLF)
//! und behalte beide beim Speichern bei.").
//!
//! The editor works with text in LF; [`decode`] normalises, [`encode`]
//! writes the encoding and line endings of the file back. Without changes in
//! the text and with uniform line endings, `encode(decode(bytes))` gives the
//! same bytes (tests below). No dependency: four encodings fit in a table.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// The character encoding of a text file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum TextEncoding {
    Utf8,
    /// UTF-8 with byte order mark.
    Utf8Bom,
    /// UTF-16 little endian with byte order mark.
    Utf16Le,
    /// UTF-16 big endian with byte order mark.
    Utf16Be,
    /// UTF-16 little endian without byte order mark.
    Utf16LeNoBom,
    /// UTF-16 big endian without byte order mark.
    Utf16BeNoBom,
    Windows1252,
}

/// The line endings of a text file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum LineEnding {
    Lf,
    Crlf,
}

/// A decoded text file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Decoded {
    /// The text with LF line endings, without byte order mark.
    pub text: String,
    pub encoding: TextEncoding,
    /// The line ending most lines use.
    pub line_ending: LineEnding,
    /// The file mixes line endings (CR LF, LF or a lone CR); saving makes
    /// them uniform.
    pub mixed_line_endings: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum EncodingError {
    /// NUL or control characters: not a text file (section 9: "Binärdateien
    /// werden nicht als Text geöffnet").
    #[error("binary content")]
    Binary,
    /// A character the encoding cannot represent.
    #[error("character U+{0:04X} cannot be written in this encoding")]
    Unencodable(u32),
}

/// Windows-1252 bytes 0x80–0x9F; `None` where the code page has no character.
const WINDOWS_1252_HIGH: [Option<char>; 32] = [
    Some('\u{20AC}'),
    None,
    Some('\u{201A}'),
    Some('\u{0192}'),
    Some('\u{201E}'),
    Some('\u{2026}'),
    Some('\u{2020}'),
    Some('\u{2021}'),
    Some('\u{02C6}'),
    Some('\u{2030}'),
    Some('\u{0160}'),
    Some('\u{2039}'),
    Some('\u{0152}'),
    None,
    Some('\u{017D}'),
    None,
    None,
    Some('\u{2018}'),
    Some('\u{2019}'),
    Some('\u{201C}'),
    Some('\u{201D}'),
    Some('\u{2022}'),
    Some('\u{2013}'),
    Some('\u{2014}'),
    Some('\u{02DC}'),
    Some('\u{2122}'),
    Some('\u{0161}'),
    Some('\u{203A}'),
    Some('\u{0153}'),
    None,
    Some('\u{017E}'),
    Some('\u{0178}'),
];

/// Decodes a text file: byte order mark, else UTF-8, else UTF-16 by its
/// zero bytes, else Windows-1252.
pub fn decode(data: &[u8]) -> Result<Decoded, EncodingError> {
    let (raw, encoding) = if let Some(rest) = data.strip_prefix(&[0xEF, 0xBB, 0xBF]) {
        (
            String::from_utf8(rest.to_vec()).map_err(|_| EncodingError::Binary)?,
            TextEncoding::Utf8Bom,
        )
    } else if let Some(rest) = data.strip_prefix(&[0xFF, 0xFE]) {
        (decode_utf16(rest, false)?, TextEncoding::Utf16Le)
    } else if let Some(rest) = data.strip_prefix(&[0xFE, 0xFF]) {
        (decode_utf16(rest, true)?, TextEncoding::Utf16Be)
    } else if let Some(big_endian) = data.contains(&0).then(|| utf16_without_bom(data)).flatten() {
        // Zero bytes are valid UTF-8, so UTF-16 is recognised first.
        let encoding = if big_endian {
            TextEncoding::Utf16BeNoBom
        } else {
            TextEncoding::Utf16LeNoBom
        };
        (decode_utf16(data, big_endian)?, encoding)
    } else if let Ok(text) = std::str::from_utf8(data) {
        (text.to_owned(), TextEncoding::Utf8)
    } else {
        (decode_windows_1252(data), TextEncoding::Windows1252)
    };
    if raw
        .chars()
        .any(|c| c.is_control() && !matches!(c, '\t' | '\n' | '\r' | '\u{000C}'))
    {
        return Err(EncodingError::Binary);
    }
    let (text, line_ending, mixed_line_endings) = normalise_line_endings(&raw);
    Ok(Decoded {
        text,
        encoding,
        line_ending,
        mixed_line_endings,
    })
}

/// Encodes text with LF line endings in `encoding` with `line_ending`.
pub fn encode(
    text: &str,
    encoding: TextEncoding,
    line_ending: LineEnding,
) -> Result<Vec<u8>, EncodingError> {
    let text = match line_ending {
        LineEnding::Lf => std::borrow::Cow::Borrowed(text),
        LineEnding::Crlf => std::borrow::Cow::Owned(text.replace('\n', "\r\n")),
    };
    Ok(match encoding {
        TextEncoding::Utf8 => text.as_bytes().to_vec(),
        TextEncoding::Utf8Bom => [&[0xEF, 0xBB, 0xBF][..], text.as_bytes()].concat(),
        TextEncoding::Utf16Le => [&[0xFF, 0xFE][..], &utf16_bytes(&text, false)].concat(),
        TextEncoding::Utf16Be => [&[0xFE, 0xFF][..], &utf16_bytes(&text, true)].concat(),
        TextEncoding::Utf16LeNoBom => utf16_bytes(&text, false),
        TextEncoding::Utf16BeNoBom => utf16_bytes(&text, true),
        TextEncoding::Windows1252 => encode_windows_1252(&text)?,
    })
}

/// The first character the encoding cannot represent, if any.
pub fn unencodable(text: &str, encoding: TextEncoding) -> Option<char> {
    if encoding != TextEncoding::Windows1252 {
        return None;
    }
    text.chars().find(|c| windows_1252_byte(*c).is_none())
}

fn decode_utf16(data: &[u8], big_endian: bool) -> Result<String, EncodingError> {
    if !data.len().is_multiple_of(2) {
        return Err(EncodingError::Binary);
    }
    let units: Vec<u16> = data
        .as_chunks::<2>()
        .0
        .iter()
        .map(|pair| {
            if big_endian {
                u16::from_be_bytes([pair[0], pair[1]])
            } else {
                u16::from_le_bytes([pair[0], pair[1]])
            }
        })
        .collect();
    String::from_utf16(&units).map_err(|_| EncodingError::Binary)
}

fn utf16_bytes(text: &str, big_endian: bool) -> Vec<u8> {
    text.encode_utf16()
        .flat_map(|unit| {
            if big_endian {
                unit.to_be_bytes()
            } else {
                unit.to_le_bytes()
            }
        })
        .collect()
}

/// UTF-16 without byte order mark: an even length and zero bytes in at
/// least 40 % of one byte position and almost none in the other (Latin
/// text has a zero high byte). `Some(true)` for big endian.
fn utf16_without_bom(data: &[u8]) -> Option<bool> {
    if data.len() < 4 || !data.len().is_multiple_of(2) {
        return None;
    }
    let pairs = data.len() / 2;
    let even = data.iter().step_by(2).filter(|b| **b == 0).count();
    let odd = data.iter().skip(1).step_by(2).filter(|b| **b == 0).count();
    let many = |count: usize| count * 10 >= pairs * 4;
    let few = |count: usize| count * 20 <= pairs;
    if many(odd) && few(even) {
        Some(false)
    } else if many(even) && few(odd) {
        Some(true)
    } else {
        None
    }
}

fn decode_windows_1252(data: &[u8]) -> String {
    data.iter()
        .map(|byte| match byte {
            0x80..=0x9F => WINDOWS_1252_HIGH[usize::from(byte - 0x80)]
                // Undefined bytes become C1 controls, so the file reads as binary.
                .unwrap_or(char::from(*byte)),
            _ => char::from(*byte),
        })
        .collect()
}

fn windows_1252_byte(c: char) -> Option<u8> {
    let code = u32::from(c);
    if code < 0x80 || (0xA0..=0xFF).contains(&code) {
        return u8::try_from(code).ok();
    }
    WINDOWS_1252_HIGH
        .iter()
        .position(|entry| *entry == Some(c))
        .and_then(|index| u8::try_from(0x80 + index).ok())
}

fn encode_windows_1252(text: &str) -> Result<Vec<u8>, EncodingError> {
    text.chars()
        .map(|c| windows_1252_byte(c).ok_or(EncodingError::Unencodable(u32::from(c))))
        .collect()
}

/// CR LF and lone CR become LF; returns the line ending most lines use and
/// whether the file mixed them.
fn normalise_line_endings(text: &str) -> (String, LineEnding, bool) {
    let mut out = String::with_capacity(text.len());
    let (mut crlf, mut lf, mut cr) = (0usize, 0usize, 0usize);
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '\r' if chars.peek() == Some(&'\n') => {
                chars.next();
                crlf += 1;
                out.push('\n');
            }
            '\r' => {
                cr += 1;
                out.push('\n');
            }
            '\n' => {
                lf += 1;
                out.push('\n');
            }
            other => out.push(other),
        }
    }
    let line_ending = if crlf > lf {
        LineEnding::Crlf
    } else {
        LineEnding::Lf
    };
    let kinds = [crlf, lf, cr].iter().filter(|count| **count > 0).count();
    (out, line_ending, kinds > 1)
}

#[cfg(test)]
mod tests {
    use super::*;

    const GERMAN: &str = "Größe, Maß und „Anführungszeichen“ – 3 €\nZweite Zeile\n";

    #[test]
    fn every_encoding_and_line_ending_comes_back_byte_for_byte() {
        for encoding in [
            TextEncoding::Utf8,
            TextEncoding::Utf8Bom,
            TextEncoding::Utf16Le,
            TextEncoding::Utf16Be,
            TextEncoding::Utf16LeNoBom,
            TextEncoding::Utf16BeNoBom,
            TextEncoding::Windows1252,
        ] {
            for line_ending in [LineEnding::Lf, LineEnding::Crlf] {
                let bytes = encode(GERMAN, encoding, line_ending).unwrap();
                let decoded = decode(&bytes).unwrap();
                assert_eq!(decoded.encoding, encoding, "{encoding:?} {line_ending:?}");
                assert_eq!(decoded.line_ending, line_ending);
                assert!(!decoded.mixed_line_endings);
                assert_eq!(decoded.text, GERMAN);
                assert_eq!(
                    encode(&decoded.text, decoded.encoding, decoded.line_ending).unwrap(),
                    bytes,
                    "{encoding:?} {line_ending:?}"
                );
            }
        }
    }

    #[test]
    fn windows_1252_is_read_where_utf_8_fails() {
        // "Größe – 5 €" in Windows-1252.
        let bytes = [
            0x47, 0x72, 0xF6, 0xDF, 0x65, 0x20, 0x96, 0x20, 0x35, 0x20, 0x80,
        ];
        let decoded = decode(&bytes).unwrap();
        assert_eq!(decoded.encoding, TextEncoding::Windows1252);
        assert_eq!(decoded.text, "Größe – 5 €");
    }

    #[test]
    fn mixed_line_endings_are_reported_and_made_uniform() {
        let decoded = decode(b"a\r\nb\r\nc\nd\re").unwrap();
        assert_eq!(decoded.text, "a\nb\nc\nd\ne");
        assert_eq!(decoded.line_ending, LineEnding::Crlf);
        assert!(decoded.mixed_line_endings);
    }

    #[test]
    fn binary_files_are_refused() {
        for data in [
            &b"PNG\x00\x01\x02"[..],
            &b"\x89PNG\r\n\x1a\n\x00\x00\x00"[..],
            &b"text\x07bell"[..],
            &[0xEF, 0xBB, 0xBF, 0xFF][..],
            // Undefined Windows-1252 bytes.
            &[0x81, 0x8D, 0x41][..],
        ] {
            assert_eq!(decode(data), Err(EncodingError::Binary), "{data:?}");
        }
        assert_eq!(decode(b"").unwrap().text, "");
    }

    #[test]
    fn characters_windows_1252_cannot_hold_are_named() {
        assert_eq!(
            unencodable("Größe 😀", TextEncoding::Windows1252),
            Some('😀')
        );
        assert_eq!(unencodable("Größe 😀", TextEncoding::Utf8), None);
        assert_eq!(
            encode("Ω", TextEncoding::Windows1252, LineEnding::Lf),
            Err(EncodingError::Unencodable(0x3A9))
        );
    }
}
