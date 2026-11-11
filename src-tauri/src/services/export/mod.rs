//! Exports (section 21; ADR 0021, decisions 2 to 4): one document as TXT or
//! Markdown, and the whole library as a folder of Markdown and media that
//! stays usable without the app.
//!
//! The frontend converts `.pwrite` content (it owns the editor schema);
//! Rust writes every file, copies media and rewrites the links. Files are
//! written atomically; the library export goes into a hidden folder next to
//! the target and is renamed only at the end.

pub mod names;

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, PoisonError};

use serde::Serialize;
use ts_rs::TS;
use uuid::Uuid;

use crate::clock::Generators;
use crate::database::Database;
use crate::database::structure::ElementKind;
use crate::filesystem::atomic::{read_limited, sync_dir, write_atomic};
use crate::security::paths::{PathError, SafeRoot};
use crate::services::documents::{DocumentFormat, MAX_DOCUMENT_BYTES, media_in};
use crate::services::media::{self, MediaKind};

use names::{LinkTargets, UniqueNames, relative_path, rewrite_links, safe_name};

/// Folder of the media in an export.
pub const MEDIA_FOLDER: &str = "Medien";
/// Folder of archived documents in a library export.
pub const ARCHIVE_FOLDER: &str = "Archiv";
/// Most library exports prepared at a time.
const MAX_OPEN_EXPORTS: usize = 2;

#[derive(Debug, thiserror::Error)]
pub enum ExportError {
    #[error("document not found")]
    NotFound,
    #[error("no library export with this id")]
    UnknownExport,
    #[error("the target lies inside the library")]
    TargetInsideLibrary,
    #[error("text not valid: {0}")]
    Invalid(&'static str),
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),
    #[error("path error: {0}")]
    Path(#[from] PathError),
    #[error("file error: {0}")]
    Io(#[from] io::Error),
}

/// What to write a document as.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextKind {
    Txt,
    Markdown,
}

impl TextKind {
    pub fn extension(self) -> &'static str {
        match self {
            TextKind::Txt => "txt",
            TextKind::Markdown => "md",
        }
    }
}

/// A document of a library export that the frontend converts (`.pwrite`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ExportItem {
    pub id: String,
    pub title: String,
}

/// A library export in progress.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ExportPlan {
    pub export_id: String,
    /// `.pwrite` documents to convert and send, one by one.
    pub to_convert: Vec<ExportItem>,
    /// Documents Rust wrote itself (Markdown and text).
    pub written: u32,
}

/// Result of an export.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ExportOutcome {
    /// Where it went, for display.
    pub location: String,
    pub documents: u32,
    pub media: u32,
}

#[derive(Debug)]
pub(crate) struct LibraryExport {
    staging: PathBuf,
    target: PathBuf,
    /// Document id → path in the export (`/`).
    documents: BTreeMap<Uuid, String>,
    /// Ids of `.pwrite` documents still to come.
    pending: BTreeSet<Uuid>,
    /// Medium id → path in the export, for those copied so far.
    media: BTreeMap<Uuid, String>,
    media_names: UniqueNames,
    written: u32,
}

/// Library exports in progress, for one opening of a library.
pub(crate) type ExportRegistry = Arc<Mutex<BTreeMap<Uuid, LibraryExport>>>;

/// Exports of one open library.
#[derive(Clone)]
pub struct Exports {
    root: SafeRoot,
    database: Database,
    generators: Generators,
    open: ExportRegistry,
}

fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

impl Exports {
    pub(crate) fn new(
        root: SafeRoot,
        database: Database,
        generators: Generators,
        open: ExportRegistry,
    ) -> Self {
        Self {
            root,
            database,
            generators,
            open,
        }
    }

    pub(crate) fn new_registry() -> ExportRegistry {
        Arc::new(Mutex::new(BTreeMap::new()))
    }

    /// Refuses a folder inside the library, except its `exports/`.
    fn check_outside(&self, folder: &Path) -> Result<(), ExportError> {
        let folder = crate::security::paths::canonical_dir(folder)?;
        let root = self.root.path();
        if folder.starts_with(root) && !folder.starts_with(root.join("exports")) {
            return Err(ExportError::TargetInsideLibrary);
        }
        Ok(())
    }

    /// The suggested file name of a document: its title.
    pub async fn suggested_name(&self, id: Uuid, kind: TextKind) -> Result<String, ExportError> {
        let row =
            crate::database::documents::get(self.database.reader(), &id.hyphenated().to_string())
                .await?
                .ok_or(ExportError::NotFound)?;
        Ok(format!(
            "{}.{}",
            safe_name(&row.title, "Dokument"),
            kind.extension()
        ))
    }

    /// Writes one document to `file`. Markdown gets the media it uses in
    /// `<name>-Medien/` next to it; links to other documents become their
    /// text (ADR 0021, decision 3).
    pub async fn export_document(
        &self,
        id: Uuid,
        text: &str,
        kind: TextKind,
        file: &Path,
    ) -> Result<ExportOutcome, ExportError> {
        if text.len() as u64 > MAX_DOCUMENT_BYTES {
            return Err(ExportError::Invalid("too large"));
        }
        crate::database::documents::get(self.database.reader(), &id.hyphenated().to_string())
            .await?
            .ok_or(ExportError::NotFound)?;
        let folder = file.parent().ok_or(ExportError::Invalid("no folder"))?;
        self.check_outside(folder)?;
        let mut media_count = 0;
        let body = match kind {
            TextKind::Txt => text.to_owned(),
            TextKind::Markdown => {
                let stem = file
                    .file_stem()
                    .and_then(|stem| stem.to_str())
                    .unwrap_or("Dokument")
                    .to_owned();
                let media_dir_name = format!("{stem}-{MEDIA_FOLDER}");
                let mut targets = LinkTargets::default();
                let mut names = UniqueNames::default();
                for (medium, _) in media_in(DocumentFormat::Md, text.as_bytes()) {
                    let Ok(medium) = Uuid::try_parse(&medium) else {
                        continue;
                    };
                    if targets.media.contains_key(&medium) {
                        continue;
                    }
                    if let Some(name) = self
                        .copy_medium(medium, &folder.join(&media_dir_name), &mut names)
                        .await?
                    {
                        targets
                            .media
                            .insert(medium, format!("{media_dir_name}/{name}"));
                        media_count += 1;
                    }
                }
                rewrite_links(text, &targets)
            }
        };
        let path = file.to_owned();
        tokio::task::spawn_blocking(move || write_atomic(&path, body.as_bytes()))
            .await
            .map_err(|error| ExportError::Io(io::Error::other(error)))??;
        Ok(ExportOutcome {
            location: crate::security::paths::display_path(file),
            documents: 1,
            media: media_count,
        })
    }

    /// Copies a medium (not in the trash) into `dir` under its original name;
    /// returns the name, or `None` for a medium that is gone.
    async fn copy_medium(
        &self,
        id: Uuid,
        dir: &Path,
        names: &mut UniqueNames,
    ) -> Result<Option<String>, ExportError> {
        let Some(row) =
            crate::database::media::get(self.database.reader(), &id.hyphenated().to_string())
                .await?
        else {
            return Ok(None);
        };
        if row.deleted_at.is_some() {
            return Ok(None);
        }
        let Some(kind) = MediaKind::parse(&row.kind) else {
            return Ok(None);
        };
        let source = self
            .root
            .resolve(&media::relative_path(kind, &row.id, &row.extension))?;
        let original = Path::new(&row.original_name);
        let stem = original
            .file_stem()
            .and_then(|stem| stem.to_str())
            .unwrap_or(&row.title);
        let name = names.take(&safe_name(stem, "Medium"), &row.extension);
        let target = dir.join(&name);
        let dir = dir.to_owned();
        tokio::task::spawn_blocking(move || -> io::Result<()> {
            fs::create_dir_all(&dir)?;
            let data = read_limited(&source, media::MAX_FILE_BYTES)?;
            write_atomic(&target, &data)
        })
        .await
        .map_err(|error| ExportError::Io(io::Error::other(error)))??;
        Ok(Some(name))
    }

    /// Starts a library export into a new folder inside `parent`: writes the
    /// Markdown and text documents at once and returns the `.pwrite` ones
    /// for the frontend to convert (ADR 0021, decision 4).
    pub async fn begin_library_export(&self, parent: &Path) -> Result<ExportPlan, ExportError> {
        self.check_outside(parent)?;
        if lock(&self.open).len() >= MAX_OPEN_EXPORTS {
            return Err(ExportError::Invalid("too many exports at once"));
        }
        let export_id = self.generators.new_id();
        let date = self.generators.now().to_iso();
        let name = format!("Project Write Export {}", &date[..10]);
        let mut top_names = UniqueNames::default();
        for entry in fs::read_dir(parent)?.filter_map(Result::ok) {
            top_names.take(&entry.file_name().to_string_lossy(), "");
        }
        let target = parent.join(top_names.take(&name, ""));
        let staging = parent.join(format!(".{}.partial-{}", name, export_id.simple()));
        fs::create_dir_all(&staging)?;

        let pool = self.database.reader();
        let documents = crate::database::documents::list_active(pool).await?;
        let subnets = crate::database::structure::list_subnets(pool).await?;
        let memberships = crate::database::structure::all_memberships(pool).await?;
        let by_id: BTreeMap<&str, &crate::database::structure::SubnetRow> =
            subnets.iter().map(|row| (row.id.as_str(), row)).collect();
        let primary: BTreeMap<&str, &str> = memberships
            .iter()
            .filter(|row| row.is_primary && row.element.kind == ElementKind::Document)
            .map(|row| (row.element.id.as_str(), row.subnet_id.as_str()))
            .collect();
        // Folder of each subnet: the names of its path, unique among siblings.
        let mut folder_of: BTreeMap<String, String> = BTreeMap::new();
        let mut siblings: BTreeMap<String, UniqueNames> = BTreeMap::new();
        fn folder(
            id: &str,
            by_id: &BTreeMap<&str, &crate::database::structure::SubnetRow>,
            folder_of: &mut BTreeMap<String, String>,
            siblings: &mut BTreeMap<String, UniqueNames>,
            depth: usize,
        ) -> String {
            if let Some(known) = folder_of.get(id) {
                return known.clone();
            }
            let Some(row) = by_id.get(id) else {
                return String::new();
            };
            let parent = match (&row.parent_id, depth < 64) {
                (Some(parent), true) => folder(parent, by_id, folder_of, siblings, depth + 1),
                _ => String::new(),
            };
            let name = siblings
                .entry(parent.clone())
                .or_default()
                .take(&safe_name(&row.name, "Subnetz"), "");
            let path = if parent.is_empty() {
                name
            } else {
                format!("{parent}/{name}")
            };
            folder_of.insert(id.to_owned(), path.clone());
            path
        }
        let mut files_in: BTreeMap<String, UniqueNames> = BTreeMap::new();
        let mut planned: BTreeMap<Uuid, String> = BTreeMap::new();
        let mut formats: BTreeMap<Uuid, (DocumentFormat, String)> = BTreeMap::new();
        for row in &documents {
            let Ok(id) = Uuid::try_parse(&row.id) else {
                continue;
            };
            let Some(format) = DocumentFormat::parse(&row.format) else {
                continue;
            };
            let mut dir = primary
                .get(row.id.as_str())
                .map(|subnet| folder(subnet, &by_id, &mut folder_of, &mut siblings, 0))
                .unwrap_or_default();
            if row.archived_at.is_some() {
                dir = if dir.is_empty() {
                    ARCHIVE_FOLDER.to_owned()
                } else {
                    format!("{ARCHIVE_FOLDER}/{dir}")
                };
            }
            let extension = if format == DocumentFormat::Txt {
                "txt"
            } else {
                "md"
            };
            let file = files_in
                .entry(dir.clone())
                .or_default()
                .take(&safe_name(&row.title, "Dokument"), extension);
            let path = if dir.is_empty() {
                file
            } else {
                format!("{dir}/{file}")
            };
            planned.insert(id, path);
            formats.insert(id, (format, row.title.clone()));
        }
        let mut export = LibraryExport {
            staging,
            target,
            documents: planned,
            pending: BTreeSet::new(),
            media: BTreeMap::new(),
            media_names: UniqueNames::default(),
            written: 0,
        };
        let mut to_convert = Vec::new();
        for (id, (format, title)) in &formats {
            match format {
                DocumentFormat::Pwrite => {
                    export.pending.insert(*id);
                    to_convert.push(ExportItem {
                        id: id.hyphenated().to_string(),
                        title: title.clone(),
                    });
                }
                DocumentFormat::Md | DocumentFormat::Txt => {
                    let path = self.root.resolve(&format.relative_path(*id))?;
                    let data = read_limited(&path, MAX_DOCUMENT_BYTES)?;
                    let text = String::from_utf8_lossy(&data).into_owned();
                    self.write_into(&mut export, *id, &text, *format == DocumentFormat::Md)
                        .await?;
                }
            }
        }
        let written = export.written;
        lock(&self.open).insert(export_id, export);
        Ok(ExportPlan {
            export_id: export_id.hyphenated().to_string(),
            to_convert,
            written,
        })
    }

    /// Writes one document into a library export; Markdown gets its links
    /// rewritten and its media copied.
    async fn write_into(
        &self,
        export: &mut LibraryExport,
        id: Uuid,
        text: &str,
        markdown: bool,
    ) -> Result<(), ExportError> {
        let path = export
            .documents
            .get(&id)
            .cloned()
            .ok_or(ExportError::NotFound)?;
        let dir = path
            .rsplit_once('/')
            .map(|(dir, _)| dir.to_owned())
            .unwrap_or_default();
        let body = if markdown {
            let mut targets = LinkTargets::default();
            for (other, other_path) in &export.documents {
                targets
                    .documents
                    .insert(*other, relative_path(&dir, other_path));
            }
            for (medium, _) in media_in(DocumentFormat::Md, text.as_bytes()) {
                let Ok(medium) = Uuid::try_parse(&medium) else {
                    continue;
                };
                if !export.media.contains_key(&medium) {
                    let media_dir = export.staging.join(MEDIA_FOLDER);
                    if let Some(name) = self
                        .copy_medium(medium, &media_dir, &mut export.media_names)
                        .await?
                    {
                        export
                            .media
                            .insert(medium, format!("{MEDIA_FOLDER}/{name}"));
                    }
                }
                if let Some(media_path) = export.media.get(&medium) {
                    targets
                        .media
                        .insert(medium, relative_path(&dir, media_path));
                }
            }
            rewrite_links(text, &targets)
        } else {
            text.to_owned()
        };
        let file = export.staging.join(&path);
        tokio::task::spawn_blocking(move || -> io::Result<()> {
            if let Some(parent) = file.parent() {
                fs::create_dir_all(parent)?;
            }
            write_atomic(&file, body.as_bytes())
        })
        .await
        .map_err(|error| ExportError::Io(io::Error::other(error)))??;
        export.written += 1;
        Ok(())
    }

    /// The converted Markdown of a `.pwrite` document of a library export.
    pub async fn add_converted(
        &self,
        export_id: Uuid,
        id: Uuid,
        markdown: &str,
    ) -> Result<(), ExportError> {
        if markdown.len() as u64 > MAX_DOCUMENT_BYTES {
            return Err(ExportError::Invalid("too large"));
        }
        let mut export = lock(&self.open)
            .remove(&export_id)
            .ok_or(ExportError::UnknownExport)?;
        if !export.pending.remove(&id) {
            lock(&self.open).insert(export_id, export);
            return Err(ExportError::NotFound);
        }
        let result = self.write_into(&mut export, id, markdown, true).await;
        lock(&self.open).insert(export_id, export);
        result
    }

    /// Writes `index.md` and renames the export to its name. Documents the
    /// frontend did not send are listed as missing.
    pub async fn finish_library_export(
        &self,
        export_id: Uuid,
    ) -> Result<ExportOutcome, ExportError> {
        let export = lock(&self.open)
            .remove(&export_id)
            .ok_or(ExportError::UnknownExport)?;
        let mut index = String::from("# Project Write Export\n\n");
        let mut current_dir: Option<String> = None;
        let mut entries: Vec<(&Uuid, &String)> = export.documents.iter().collect();
        entries.sort_by(|a, b| a.1.cmp(b.1));
        for (id, path) in entries {
            if export.pending.contains(id) {
                continue;
            }
            let (dir, file) = path.rsplit_once('/').unwrap_or(("", path));
            if current_dir.as_deref() != Some(dir) {
                index.push_str(&format!(
                    "\n## {}\n\n",
                    if dir.is_empty() { "Ohne Subnetz" } else { dir }
                ));
                current_dir = Some(dir.to_owned());
            }
            let title = file.rsplit_once('.').map_or(file, |(stem, _)| stem);
            index.push_str(&format!("- [{title}]({})\n", names::encode_path(path)));
        }
        let staging = export.staging.clone();
        let target = export.target.clone();
        let documents = export.written;
        let media_count = u32::try_from(export.media.len()).unwrap_or(u32::MAX);
        tokio::task::spawn_blocking(move || -> io::Result<()> {
            write_atomic(&staging.join("index.md"), index.as_bytes())?;
            fs::rename(&staging, &target)?;
            if let Some(parent) = target.parent() {
                sync_dir(parent)?;
            }
            Ok(())
        })
        .await
        .map_err(|error| ExportError::Io(io::Error::other(error)))??;
        Ok(ExportOutcome {
            location: crate::security::paths::display_path(&export.target),
            documents,
            media: media_count,
        })
    }

    /// Stops a library export and removes what it wrote.
    pub fn cancel_library_export(&self, export_id: Uuid) -> bool {
        match lock(&self.open).remove(&export_id) {
            Some(export) => {
                let _ = fs::remove_dir_all(&export.staging);
                true
            }
            None => false,
        }
    }
}
