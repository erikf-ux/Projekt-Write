//! Checks for files imported into the library (section 9, "Import in die
//! Bibliothek"; section 20: "Prüfe importierte Dateitypen über den Inhalt,
//! nicht nur über die Endung. Es gilt eine Positivliste.").
//!
//! Text documents dropped onto the window or taken into the library:
//! `.txt`, `.md` and `.markdown` in UTF-8, UTF-16 or Windows-1252 (Phase 5,
//! `encoding.rs`), and `.pwrite`. Library documents are stored in UTF-8
//! with LF line ends; the original file is never touched, only its content
//! arrives here. Media follow with the media library (Phase 8).

use crate::services::documents::{DocumentFormat, validate_title};
use crate::services::encoding::{self, EncodingError};
use crate::services::pwrite;

/// Largest file accepted for import (10 MiB; section 9 warns from 10 MB).
pub const MAX_IMPORT_BYTES: usize = 10 * 1024 * 1024;
/// Longest original file name kept (as in migration 0003).
pub const MAX_FILE_NAME_CHARS: usize = 255;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ImportError {
    #[error("file type not on the positive list")]
    UnsupportedType,
    #[error("content is not text of its type ({0})")]
    NotText(&'static str),
    #[error("file larger than {MAX_IMPORT_BYTES} bytes")]
    TooLarge,
    #[error("invalid file name ({0})")]
    InvalidName(&'static str),
}

/// A file that passed the checks, ready to be stored.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedImport {
    pub title: String,
    pub format: DocumentFormat,
    /// Text: UTF-8 without byte order mark, LF line ends. `.pwrite`: the
    /// file as it was (the library gives it a new envelope).
    pub content: Vec<u8>,
}

/// Checks a file name as the webview reports it: a name, never a path.
pub fn validate_file_name(name: &str) -> Result<(), ImportError> {
    let length = name.chars().count();
    if length == 0 || length > MAX_FILE_NAME_CHARS {
        return Err(ImportError::InvalidName("length"));
    }
    if name
        .chars()
        .any(|c| c.is_control() || c == '/' || c == '\\')
    {
        return Err(ImportError::InvalidName("character"));
    }
    if name.trim().is_empty() || name == "." || name == ".." {
        return Err(ImportError::InvalidName("empty"));
    }
    Ok(())
}

/// Checks type and content of a file to import.
pub fn check(file_name: &str, data: &[u8]) -> Result<CheckedImport, ImportError> {
    validate_file_name(file_name)?;
    let (stem, format) = format_of(file_name)?;
    if data.len() > MAX_IMPORT_BYTES {
        return Err(ImportError::TooLarge);
    }
    let content = if format == DocumentFormat::Pwrite {
        match pwrite::parse(data) {
            Ok(pwrite::Parsed::Current(_)) => data.to_vec(),
            Ok(pwrite::Parsed::Newer { .. }) => return Err(ImportError::NotText("newer .pwrite")),
            Err(_) => return Err(ImportError::NotText("not .pwrite")),
        }
    } else {
        text_content(data)?.into_bytes()
    };
    Ok(CheckedImport {
        title: title_of(stem, file_name),
        format,
        content,
    })
}

/// The positive list: the extension decides the format, the content must
/// then be text of that kind.
fn format_of(file_name: &str) -> Result<(&str, DocumentFormat), ImportError> {
    let (stem, extension) = file_name
        .rsplit_once('.')
        .ok_or(ImportError::UnsupportedType)?;
    let format = match extension.to_ascii_lowercase().as_str() {
        "txt" => DocumentFormat::Txt,
        "md" | "markdown" => DocumentFormat::Md,
        "pwrite" => DocumentFormat::Pwrite,
        _ => return Err(ImportError::UnsupportedType),
    };
    Ok((stem, format))
}

/// Text in one of the known encodings without NUL or other control
/// characters (except tab, line feed, carriage return and form feed); LF
/// line ends.
fn text_content(data: &[u8]) -> Result<String, ImportError> {
    match encoding::decode(data) {
        Ok(decoded) => Ok(decoded.text),
        Err(EncodingError::Binary | EncodingError::Unencodable(_)) => {
            Err(ImportError::NotText("binary"))
        }
    }
}

/// The title: the file name without extension; the whole name if the stem
/// is blank (`.md`). File names are shorter than the longest title.
fn title_of(stem: &str, file_name: &str) -> String {
    let base = if stem.trim().is_empty() {
        file_name
    } else {
        stem
    };
    validate_title(base).unwrap_or_else(|_| file_name.trim().to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::documents::MAX_TITLE_CHARS;

    #[test]
    fn text_files_on_the_positive_list_are_accepted() {
        let checked = check(
            "Projektplan.md",
            "# Plan\r\nÄrger über Größe\r\n".as_bytes(),
        )
        .unwrap();
        assert_eq!(checked.title, "Projektplan");
        assert_eq!(checked.format, DocumentFormat::Md);
        assert_eq!(checked.content, "# Plan\nÄrger über Größe\n".as_bytes());

        let checked = check("notiz.TXT", b"\xEF\xBB\xBFmit BOM\rund CR").unwrap();
        assert_eq!(checked.format, DocumentFormat::Txt);
        assert_eq!(checked.content, b"mit BOM\nund CR");

        let checked = check("Kapitel 1.markdown", b"").unwrap();
        assert_eq!(checked.title, "Kapitel 1");
        assert!(checked.content.is_empty());

        assert_eq!(check(".md", b"x").unwrap().title, ".md");
        assert_eq!(
            check("tabs.txt", b"a\tb\x0cc").unwrap().content,
            b"a\tb\x0cc"
        );
    }

    #[test]
    fn other_types_are_rejected_by_extension_and_content() {
        for name in ["bild.png", "README", "skript.md.exe", "notiz.txt.zip"] {
            assert_eq!(
                check(name, b"text"),
                Err(ImportError::UnsupportedType),
                "{name}"
            );
        }
        // A binary file renamed to .txt.
        assert_eq!(
            check("getarnt.txt", b"\x89PNG\r\n\x1a\n\0\0\0"),
            Err(ImportError::NotText("binary"))
        );
        assert_eq!(
            check("nul.txt", b"a\0b"),
            Err(ImportError::NotText("binary"))
        );
        assert_eq!(
            check("bell.md", b"a\x07b"),
            Err(ImportError::NotText("binary"))
        );
        assert_eq!(
            check("kein.pwrite", b"{\"format\": \"other\"}"),
            Err(ImportError::NotText("not .pwrite"))
        );
        assert_eq!(
            check("gross.txt", &vec![b'a'; MAX_IMPORT_BYTES + 1]),
            Err(ImportError::TooLarge)
        );
    }

    #[test]
    fn utf_16_and_windows_1252_become_utf_8() {
        assert_eq!(
            check("utf16.txt", b"\xFF\xFEa\0\r\0\n\0").unwrap().content,
            b"a\n"
        );
        assert_eq!(
            check("latin1.txt", b"Gr\xF6\xDFe \x80").unwrap().content,
            "Größe €".as_bytes()
        );
    }

    #[test]
    fn pwrite_files_are_checked_as_pwrite() {
        let file = pwrite::new_file(
            "0b8f8f4e-6d0e-4f3e-9a3c-2a1d3c4b5e6f",
            "Roman",
            "2026-11-04T10:00:00Z",
            "de-DE",
            pwrite::empty_content(),
        );
        let data = pwrite::to_bytes(&file);
        let checked = check("Roman.pwrite", &data).unwrap();
        assert_eq!(checked.format, DocumentFormat::Pwrite);
        assert_eq!(checked.content, data);
        let newer = String::from_utf8(data)
            .unwrap()
            .replace("\"formatVersion\":1", "\"formatVersion\":2");
        assert_eq!(
            check("Neu.pwrite", newer.as_bytes()),
            Err(ImportError::NotText("newer .pwrite"))
        );
    }

    #[test]
    fn names_must_be_plain_file_names() {
        for name in [
            "../geheim.txt",
            "ordner/a.txt",
            "C:\\a.txt",
            "",
            " ",
            "a\n.txt",
            "..",
        ] {
            assert!(
                matches!(check(name, b"x"), Err(ImportError::InvalidName(_))),
                "{name:?}"
            );
        }
        assert!(validate_file_name(&"a".repeat(255)).is_ok());
        assert!(validate_file_name(&"a".repeat(256)).is_err());
    }

    #[test]
    fn the_longest_file_name_still_fits_into_a_title() {
        // Titles may be longer than file names, so the stem is never cut.
        const { assert!(MAX_FILE_NAME_CHARS < MAX_TITLE_CHARS) };
        let stem = "Ä".repeat(MAX_FILE_NAME_CHARS - 4);
        let checked = check(&format!("{stem}.txt"), b"x").unwrap();
        assert_eq!(checked.title, stem);
    }
}
