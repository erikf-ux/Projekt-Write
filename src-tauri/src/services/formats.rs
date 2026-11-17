//! The registry of document formats (section 10; ADR 0027, decision 1).
//!
//! [`DocumentFormat`] is the typed id of a format; everything the code
//! decides by format lives in its [`FormatEntry`]: file extension, folder in
//! the library, kind (text or rich text), the extensions accepted when a
//! file comes from outside, the name in file dialogs and how links, media
//! and searchable text are read from a content. Code looks a format up by
//! id ([`DocumentFormat::spec`]) or by file extension
//! ([`DocumentFormats::by_extension`]) instead of matching on it.

use crate::services::documents::{self, DocumentFormat};
use crate::services::locale::Locale;
use crate::services::registry::{Registry, RegistryEntry};
use crate::services::search::text as search_text;

/// What a content of a format is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FormatKind {
    /// UTF-8 text without markup.
    PlainText,
    /// UTF-8 Markdown; internal links and media are written in the text.
    Markdown,
    /// Rich text: the editor content as JSON in a `.pwrite` envelope
    /// (section 10). Its text form is made by the frontend, which owns the
    /// editor schema.
    RichText,
}

/// A name in the two interface languages.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LocalizedName {
    pub de: &'static str,
    pub en: &'static str,
}

impl LocalizedName {
    pub const fn get(self, locale: Locale) -> &'static str {
        match locale {
            Locale::De => self.de,
            Locale::En => self.en,
        }
    }
}

/// One document format.
#[derive(Debug, Clone)]
pub struct FormatEntry {
    pub format: DocumentFormat,
    /// File extension of library files and new external files; also the
    /// value of the `format` column.
    pub extension: &'static str,
    /// Extensions a file from outside may have (import, opening, "save
    /// as"), lower case; the first is [`Self::extension`].
    pub accepted_extensions: &'static [&'static str],
    /// Folder of the files, relative to the library root.
    pub directory: &'static str,
    pub kind: FormatKind,
    /// Name of the filter in file dialogs.
    pub filter_name: LocalizedName,
    /// The documents a content links to (`pwrite://<id>`), sorted, unique.
    pub links: fn(&[u8]) -> Vec<String>,
    /// The media a content uses (`pwmedia://<id>`, page), sorted, unique.
    pub media: fn(&[u8]) -> Vec<(String, u32)>,
    /// The searchable text of a content, before control characters are
    /// cleaned.
    pub text: fn(&[u8]) -> String,
}

impl RegistryEntry for FormatEntry {
    fn id(&self) -> &'static str {
        self.format.as_str()
    }
}

impl FormatEntry {
    pub fn is_rich_text(&self) -> bool {
        self.kind == FormatKind::RichText
    }

    pub fn is_markdown(&self) -> bool {
        self.kind == FormatKind::Markdown
    }

    /// Whether a file with `extension` (any case) is of this format.
    pub fn accepts_extension(&self, extension: &str) -> bool {
        self.accepted_extensions
            .iter()
            .any(|accepted| accepted.eq_ignore_ascii_case(extension))
    }
}

/// The registry of document formats.
#[derive(Debug)]
pub struct DocumentFormats(Registry<FormatEntry>);

impl DocumentFormats {
    pub fn new(entries: impl IntoIterator<Item = FormatEntry>) -> Self {
        Self(Registry::new("documentFormats", entries))
    }

    pub fn registry(&self) -> &Registry<FormatEntry> {
        &self.0
    }

    /// The entry of a format.
    ///
    /// # Panics
    ///
    /// If the format is not registered: every [`DocumentFormat`] is built in
    /// (checked by a test).
    pub fn spec(&self, format: DocumentFormat) -> &FormatEntry {
        self.0
            .get(format.as_str())
            .unwrap_or_else(|| panic!("document format {} is not registered", format.as_str()))
    }

    /// The format with the id (the value of the `format` column).
    pub fn by_id(&self, id: &str) -> Option<DocumentFormat> {
        self.0.get(id).map(|entry| entry.format)
    }

    /// The format of a file extension from the accepted ones (any case).
    pub fn by_extension(&self, extension: &str) -> Option<DocumentFormat> {
        self.0
            .find(|entry| entry.accepts_extension(extension))
            .map(|entry| entry.format)
    }

    /// The formats in registration order.
    pub fn formats(&self) -> impl Iterator<Item = DocumentFormat> + '_ {
        self.0.iter().map(|entry| entry.format)
    }

    /// Every accepted extension of every format, in registration order.
    pub fn accepted_extensions(&self) -> impl Iterator<Item = &'static str> + '_ {
        self.0
            .iter()
            .flat_map(|entry| entry.accepted_extensions.iter().copied())
    }

    pub fn iter(&self) -> std::slice::Iter<'_, FormatEntry> {
        self.0.iter()
    }
}

/// The built-in document formats (section 10).
pub fn builtin() -> Vec<FormatEntry> {
    vec![
        FormatEntry {
            format: DocumentFormat::Txt,
            extension: "txt",
            accepted_extensions: &["txt"],
            directory: "documents/text",
            kind: FormatKind::PlainText,
            filter_name: LocalizedName {
                de: "Textdatei (.txt)",
                en: "Text file (.txt)",
            },
            links: |_| Vec::new(),
            media: |_| Vec::new(),
            text: search_text::plain_text,
        },
        FormatEntry {
            format: DocumentFormat::Md,
            extension: "md",
            accepted_extensions: &["md", "markdown"],
            directory: "documents/markdown",
            kind: FormatKind::Markdown,
            filter_name: LocalizedName {
                de: "Markdown (.md)",
                en: "Markdown (.md)",
            },
            links: documents::markdown_links,
            media: documents::markdown_media,
            text: search_text::markdown_text,
        },
        FormatEntry {
            format: DocumentFormat::Pwrite,
            extension: "pwrite",
            accepted_extensions: &["pwrite"],
            directory: "documents/richtext",
            kind: FormatKind::RichText,
            filter_name: LocalizedName {
                de: "Rich Text (.pwrite)",
                en: "Rich text (.pwrite)",
            },
            links: documents::pwrite_links,
            media: documents::pwrite_media,
            text: search_text::pwrite_text,
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn formats() -> DocumentFormats {
        DocumentFormats::new(builtin())
    }

    #[test]
    fn every_typed_format_is_registered_once_under_its_column_value() {
        let formats = formats();
        assert_eq!(
            formats.formats().collect::<Vec<_>>(),
            [
                DocumentFormat::Txt,
                DocumentFormat::Md,
                DocumentFormat::Pwrite
            ]
        );
        for entry in formats.iter() {
            assert_eq!(entry.extension, entry.format.as_str());
            assert_eq!(entry.accepted_extensions[0], entry.extension);
            assert_eq!(formats.by_id(entry.format.as_str()), Some(entry.format));
            assert!(entry.directory.starts_with("documents/"));
        }
    }

    #[test]
    fn extensions_are_found_in_any_case_and_only_from_the_positive_list() {
        let formats = formats();
        assert_eq!(formats.by_extension("txt"), Some(DocumentFormat::Txt));
        assert_eq!(formats.by_extension("MD"), Some(DocumentFormat::Md));
        assert_eq!(formats.by_extension("Markdown"), Some(DocumentFormat::Md));
        assert_eq!(formats.by_extension("pwrite"), Some(DocumentFormat::Pwrite));
        for other in ["", "rtf", "docx", "pwritepack", "text", "mdx"] {
            assert_eq!(formats.by_extension(other), None, "{other}");
        }
        assert_eq!(
            formats.accepted_extensions().collect::<Vec<_>>(),
            ["txt", "md", "markdown", "pwrite"]
        );
    }

    #[test]
    fn only_pwrite_is_rich_text_and_only_md_is_markdown() {
        let formats = formats();
        let rich: Vec<_> = formats
            .iter()
            .filter(|entry| entry.is_rich_text())
            .map(|entry| entry.format)
            .collect();
        assert_eq!(rich, [DocumentFormat::Pwrite]);
        let markdown: Vec<_> = formats
            .iter()
            .filter(|entry| entry.is_markdown())
            .map(|entry| entry.format)
            .collect();
        assert_eq!(markdown, [DocumentFormat::Md]);
    }

    #[test]
    fn dialog_names_follow_the_locale() {
        let formats = formats();
        let txt = formats.spec(DocumentFormat::Txt);
        assert_eq!(txt.filter_name.get(Locale::De), "Textdatei (.txt)");
        assert_eq!(txt.filter_name.get(Locale::En), "Text file (.txt)");
    }
}
