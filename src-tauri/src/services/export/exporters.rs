//! The registry of exporters (ADR 0027, decision 1).
//!
//! Each exporter says what it is (id, name in the interface), where it
//! writes (one file with an extension, or a new folder), which document
//! formats it takes and how it writes. The export commands look the
//! exporter up by id and use only what the entry says.
//!
//! Built in: TXT, Markdown (with its media), the package `.pwritepack` and
//! the library as a folder of Markdown files.

use std::future::Future;
use std::path::Path;
use std::pin::Pin;

use uuid::Uuid;

use super::pack::{self, PackError};
use super::{ExportError, ExportOutcome, ExportPlan, Exports};
use crate::services::documents::DocumentFormat;
use crate::services::registry::RegistryEntry;

/// A future the exporters return; boxed so that [`Exporter`] can be used as
/// a trait object.
pub type ExportFuture<'a, T> = Pin<Box<dyn Future<Output = Result<T, ExporterError>> + Send + 'a>>;

/// Ids of the built-in exporters.
pub const TXT: &str = "txt";
pub const MARKDOWN: &str = "markdown";
pub const PACK: &str = "pack";
pub const LIBRARY: &str = "library";

#[derive(Debug, thiserror::Error)]
pub enum ExporterError {
    #[error(transparent)]
    Export(#[from] ExportError),
    #[error(transparent)]
    Pack(#[from] PackError),
    /// The exporter does not write this kind of target (a programming
    /// error in the caller).
    #[error("exporter {0} does not write this target")]
    WrongTarget(&'static str),
}

/// Where an exporter writes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportTarget {
    /// One file from a save dialog with this filter.
    File {
        extension: &'static str,
        filter_name: &'static str,
    },
    /// A new folder inside one chosen in a folder dialog.
    Folder,
}

/// One exporter.
pub trait Exporter: RegistryEntry + Send + Sync {
    /// Key of its name in the interface (`src/i18n`).
    fn label_key(&self) -> &'static str;

    fn target(&self) -> ExportTarget;

    /// The formats of the content it takes. The frontend converts other
    /// formats first (it owns the editor schema, ADR 0021).
    fn accepts(&self) -> &'static [DocumentFormat];

    /// The file name the save dialog suggests for a document whose title
    /// gives `stem`.
    fn suggested_name(&self, stem: &str) -> String {
        match self.target() {
            ExportTarget::File { extension, .. } => format!("{stem}.{extension}"),
            ExportTarget::Folder => stem.to_owned(),
        }
    }

    /// Writes the document `id` with `content` (in a format of
    /// [`Self::accepts`]) to `file`. Only for a [`ExportTarget::File`].
    fn export_file<'a>(
        &'a self,
        exports: &'a Exports,
        id: Uuid,
        content: &'a str,
        file: &'a Path,
    ) -> ExportFuture<'a, ExportOutcome> {
        let _ = (exports, id, content, file);
        Box::pin(async move { Err(ExporterError::WrongTarget(self.id())) })
    }

    /// Starts an export into a new folder inside `parent`. Only for a
    /// [`ExportTarget::Folder`].
    fn begin_folder<'a>(
        &'a self,
        exports: &'a Exports,
        parent: &'a Path,
    ) -> ExportFuture<'a, ExportPlan> {
        let _ = (exports, parent);
        Box::pin(async move { Err(ExporterError::WrongTarget(self.id())) })
    }
}

/// One document as text: TXT as it is, Markdown with its media in
/// `<name>-Medien/` and links to other documents as their text.
struct DocumentExporter {
    id: &'static str,
    label_key: &'static str,
    accepts: &'static [DocumentFormat],
    filter_name: &'static str,
}

impl RegistryEntry for DocumentExporter {
    fn id(&self) -> &'static str {
        self.id
    }
}

impl Exporter for DocumentExporter {
    fn label_key(&self) -> &'static str {
        self.label_key
    }

    fn target(&self) -> ExportTarget {
        ExportTarget::File {
            extension: self.accepts[0].extension(),
            filter_name: self.filter_name,
        }
    }

    fn accepts(&self) -> &'static [DocumentFormat] {
        self.accepts
    }

    fn export_file<'a>(
        &'a self,
        exports: &'a Exports,
        id: Uuid,
        content: &'a str,
        file: &'a Path,
    ) -> ExportFuture<'a, ExportOutcome> {
        Box::pin(async move {
            Ok(exports
                .export_document(id, content, self.accepts[0], file)
                .await?)
        })
    }
}

/// The package `.pwritepack`: a `.pwrite` file with its media.
struct PackExporter;

impl RegistryEntry for PackExporter {
    fn id(&self) -> &'static str {
        PACK
    }
}

impl Exporter for PackExporter {
    fn label_key(&self) -> &'static str {
        "command.export.pack"
    }

    fn target(&self) -> ExportTarget {
        ExportTarget::File {
            extension: pack::EXTENSION,
            filter_name: "Project Write Paket",
        }
    }

    fn accepts(&self) -> &'static [DocumentFormat] {
        &[DocumentFormat::Pwrite]
    }

    /// As since Phase 10: the name of the Markdown export with `.md`
    /// replaced, so a title ending in `.md` loses it.
    fn suggested_name(&self, stem: &str) -> String {
        let markdown = format!("{stem}.{}", DocumentFormat::Md.extension());
        format!("{}.{}", markdown.trim_end_matches(".md"), pack::EXTENSION)
    }

    fn export_file<'a>(
        &'a self,
        exports: &'a Exports,
        _id: Uuid,
        content: &'a str,
        file: &'a Path,
    ) -> ExportFuture<'a, ExportOutcome> {
        Box::pin(async move {
            let media = exports.export_pack(content.as_bytes(), file).await?;
            Ok(ExportOutcome {
                location: crate::security::paths::display_path(file),
                documents: 1,
                media,
            })
        })
    }
}

/// The whole library as a folder of Markdown and text files with the media
/// and an `index.md` (ADR 0021, decision 4). Rust writes the text and
/// Markdown documents itself; the frontend sends rich text converted to
/// Markdown, one by one.
struct LibraryExporter;

impl RegistryEntry for LibraryExporter {
    fn id(&self) -> &'static str {
        LIBRARY
    }
}

impl Exporter for LibraryExporter {
    fn label_key(&self) -> &'static str {
        "command.export.library"
    }

    fn target(&self) -> ExportTarget {
        ExportTarget::Folder
    }

    fn accepts(&self) -> &'static [DocumentFormat] {
        &[DocumentFormat::Txt, DocumentFormat::Md]
    }

    fn begin_folder<'a>(
        &'a self,
        exports: &'a Exports,
        parent: &'a Path,
    ) -> ExportFuture<'a, ExportPlan> {
        Box::pin(async move { Ok(exports.begin_library_export(parent, self.accepts()).await?) })
    }
}

/// The built-in exporters, in the order of the export menu.
pub fn builtin() -> Vec<Box<dyn Exporter>> {
    vec![
        Box::new(DocumentExporter {
            id: TXT,
            label_key: "command.export.txt",
            accepts: &[DocumentFormat::Txt],
            filter_name: "Text",
        }),
        Box::new(DocumentExporter {
            id: MARKDOWN,
            label_key: "command.export.markdown",
            accepts: &[DocumentFormat::Md],
            filter_name: "Markdown",
        }),
        Box::new(PackExporter),
        Box::new(LibraryExporter),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::registry::Registry;

    fn exporters() -> Registry<Box<dyn Exporter>> {
        Registry::new("exporters", builtin())
    }

    #[test]
    fn file_exporters_name_their_extension_and_filter() {
        let exporters = exporters();
        let target = |id: &str| exporters.get(id).map(|exporter| exporter.target());
        assert_eq!(
            target(TXT),
            Some(ExportTarget::File {
                extension: "txt",
                filter_name: "Text"
            })
        );
        assert_eq!(
            target(MARKDOWN),
            Some(ExportTarget::File {
                extension: "md",
                filter_name: "Markdown"
            })
        );
        assert_eq!(
            target(PACK),
            Some(ExportTarget::File {
                extension: "pwritepack",
                filter_name: "Project Write Paket"
            })
        );
        assert_eq!(target(LIBRARY), Some(ExportTarget::Folder));
    }

    #[test]
    fn suggested_names_keep_the_behaviour_of_phase_10() {
        let exporters = exporters();
        let name = |id: &str, stem: &str| exporters.get(id).unwrap().suggested_name(stem);
        assert_eq!(name(TXT, "Brief"), "Brief.txt");
        assert_eq!(name(MARKDOWN, "Brief"), "Brief.md");
        assert_eq!(name(PACK, "Brief"), "Brief.pwritepack");
        assert_eq!(name(PACK, "Notizen.md"), "Notizen.pwritepack");
    }

    #[test]
    fn exporters_take_the_formats_they_write() {
        let exporters = exporters();
        let accepts = |id: &str| exporters.get(id).unwrap().accepts().to_vec();
        assert_eq!(accepts(TXT), [DocumentFormat::Txt]);
        assert_eq!(accepts(MARKDOWN), [DocumentFormat::Md]);
        assert_eq!(accepts(PACK), [DocumentFormat::Pwrite]);
        assert_eq!(accepts(LIBRARY), [DocumentFormat::Txt, DocumentFormat::Md]);
        for exporter in &exporters {
            assert!(exporter.label_key().starts_with("command.export."));
        }
    }
}
