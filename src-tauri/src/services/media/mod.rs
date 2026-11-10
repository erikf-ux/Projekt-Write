//! Media of one open library (sections 9 and 17; ADR 0019): images, PDFs
//! and Office attachments, with import, thumbnails, trash and the files the
//! media handler serves.
//!
//! Files are named `<id>.<ext>` in `media/images/`, `media/pdf/` and
//! `media/attachments/`; the path is derived from the row, never stored.
//! Trash and archive go through the structure service (the structural undo
//! covers them); removing for good happens here, and only for media no
//! document uses any more.

pub mod detect;
pub mod import;
pub mod thumbnails;

use std::collections::BTreeSet;
use std::fs;
use std::io;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, PoisonError};

use serde::{Deserialize, Serialize};
use ts_rs::TS;
use uuid::Uuid;

use crate::clock::{Generators, Timestamp};
use crate::database::Database;
use crate::database::media::{self as rows, MediaRow};
use crate::filesystem::atomic::{is_temp_file_name, read_limited, write_atomic};
use crate::security::paths::{PathError, SafeRoot};
use crate::services::library::layout::TMP_DIR;
use crate::services::settings::library::{LibrarySettings, MediaMaxMegapixels};
use crate::services::structure::StructureError;

pub use detect::{MediaKind, MediaType, Refusal};
pub use import::{
    Decision, Duplicate, ImportOutcome, ImportPreview, ImportSource, ImportedItem, PreviewItem,
    SkipReason, SkippedItem, SubnetPlacement,
};
use thumbnails::{ThumbnailError, ThumbnailFormat};

/// Largest file from the file dialog or a folder (ADR 0019, decision 4).
pub const MAX_FILE_BYTES: u64 = 1024 * 1024 * 1024;
/// Largest medium dropped or pasted into the window (raw IPC body).
pub const MAX_IPC_BYTES: usize = 100 * 1024 * 1024;
pub const MAX_TITLE_CHARS: usize = 300;
pub const MAX_DESCRIPTION_CHARS: usize = 2000;
/// Read-only copies opened with the standard program (ADR 0019, decision 14).
pub const OPEN_DIR: &str = "tmp/open";
/// Thumbnails of images and PDFs (section 9).
pub const IMAGE_THUMBNAILS_DIR: &str = "thumbnails/images";
pub const PDF_THUMBNAILS_DIR: &str = "thumbnails/pdf";
/// Highest page number of a PDF reference and page count of a PDF.
pub const MAX_PAGES: u32 = 100_000;

#[derive(Debug, thiserror::Error)]
pub enum MediaError {
    #[error("medium not found")]
    NotFound,
    #[error("refused: {}", .0.as_str())]
    Refused(Refusal),
    #[error("above the limit of {max} {limit}")]
    TooLarge { limit: &'static str, max: u64 },
    #[error("still used by {documents} documents")]
    InUse { documents: i64 },
    #[error("not in the trash")]
    NotInTrash,
    #[error("no prepared import with this id")]
    ImportNotFound,
    #[error("import cancelled")]
    Cancelled,
    #[error("invalid title: {0}")]
    InvalidTitle(&'static str),
    #[error("invalid description: {0}")]
    InvalidDescription(&'static str),
    #[error("invalid file name: {0}")]
    InvalidName(&'static str),
    #[error("invalid value: {0}")]
    InvalidValue(&'static str),
    #[error("thumbnail: {0}")]
    Thumbnail(#[from] ThumbnailError),
    #[error("path: {0}")]
    Path(#[from] PathError),
    #[error("file system: {0}")]
    Io(#[from] io::Error),
    #[error("database: {0}")]
    Database(#[from] sqlx::Error),
    #[error("structure: {0}")]
    Structure(#[from] StructureError),
    #[error("settings: {0}")]
    Settings(String),
}

/// A medium as the interface sees it (no path).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct MediaSummary {
    pub id: String,
    pub kind: MediaKind,
    pub mime_type: String,
    pub title: String,
    pub description: String,
    pub original_name: String,
    /// The folder the file came from (metadata, section 9).
    pub import_folder: Option<String>,
    #[ts(type = "number")]
    pub size_bytes: u64,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub page_count: Option<u32>,
    pub created_at: String,
    pub updated_at: String,
    pub favorited_at: Option<String>,
    pub last_opened_at: Option<String>,
    pub archived_at: Option<String>,
    pub deleted_at: Option<String>,
}

impl From<&MediaRow> for MediaSummary {
    fn from(row: &MediaRow) -> Self {
        let positive = |value: Option<i64>| value.and_then(|v| u32::try_from(v).ok());
        Self {
            id: row.id.clone(),
            kind: MediaKind::parse(&row.kind).unwrap_or(MediaKind::Attachment),
            mime_type: row.mime_type.clone(),
            title: row.title.clone(),
            description: row.description.clone(),
            original_name: row.original_name.clone(),
            import_folder: row.import_folder.clone(),
            size_bytes: u64::try_from(row.size_bytes).unwrap_or(0),
            width: positive(row.width),
            height: positive(row.height),
            page_count: positive(row.page_count),
            created_at: row.created_at.clone(),
            updated_at: row.updated_at.clone(),
            favorited_at: row.favorited_at.clone(),
            last_opened_at: row.last_opened_at.clone(),
            archived_at: row.archived_at.clone(),
            deleted_at: row.deleted_at.clone(),
        }
    }
}

/// A document that uses a medium, with the page it refers to (0: the whole
/// medium).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct MediaUse {
    pub document_id: String,
    pub page: u32,
}

/// The file the media handler serves.
#[derive(Debug, Clone)]
pub struct ServedFile {
    pub path: PathBuf,
    pub mime_type: &'static str,
}

/// What [`purge_trash`](MediaLibrary::empty_trash) did.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct PurgeReport {
    pub removed: u32,
    /// Media that stay because documents still use them.
    pub kept_in_use: u32,
}

/// The relative path of a medium's file.
pub fn relative_path(kind: MediaKind, id: &str, extension: &str) -> String {
    format!("{}/{id}.{extension}", kind.directory())
}

/// Trims a title and checks length and characters.
pub fn validate_title(title: &str) -> Result<String, MediaError> {
    let title = title.trim();
    if title.is_empty() {
        return Err(MediaError::InvalidTitle("empty"));
    }
    if title.chars().count() > MAX_TITLE_CHARS {
        return Err(MediaError::InvalidTitle("too long"));
    }
    if title.chars().any(char::is_control) {
        return Err(MediaError::InvalidTitle("control characters"));
    }
    Ok(title.to_owned())
}

/// Line breaks and tabs are allowed in a description, other control
/// characters not.
pub fn validate_description(description: &str) -> Result<String, MediaError> {
    let description = description.trim();
    if description.chars().count() > MAX_DESCRIPTION_CHARS {
        return Err(MediaError::InvalidDescription("too long"));
    }
    if description
        .chars()
        .any(|c| c.is_control() && c != '\n' && c != '\t')
    {
        return Err(MediaError::InvalidDescription("control characters"));
    }
    Ok(description.to_owned())
}

/// The title a new medium gets: its file name without the extension.
pub fn title_from_name(name: &str) -> String {
    let stem = match name.rsplit_once('.') {
        Some((stem, _)) if !stem.trim().is_empty() => stem,
        _ => name,
    };
    let cleaned: String = stem
        .chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .take(MAX_TITLE_CHARS)
        .collect();
    let cleaned = cleaned.trim();
    if cleaned.is_empty() {
        "Medium".to_owned()
    } else {
        cleaned.to_owned()
    }
}

fn id_text(id: Uuid) -> String {
    id.hyphenated().to_string()
}

/// The media of one open library.
#[derive(Debug, Clone)]
pub struct MediaLibrary {
    root: SafeRoot,
    database: Database,
    generators: Generators,
    write_lock: Arc<tokio::sync::Mutex<()>>,
    imports: Arc<Mutex<import::Batches>>,
}

impl MediaLibrary {
    pub(crate) fn new(
        root: SafeRoot,
        database: Database,
        generators: Generators,
        write_lock: Arc<tokio::sync::Mutex<()>>,
        imports: Arc<Mutex<import::Batches>>,
    ) -> Self {
        Self {
            root,
            database,
            generators,
            write_lock,
            imports,
        }
    }

    fn batches(&self) -> std::sync::MutexGuard<'_, import::Batches> {
        self.imports.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn settings(&self) -> LibrarySettings {
        LibrarySettings::new(self.database.clone(), self.generators.clone())
    }

    async fn max_megapixels(&self) -> u32 {
        self.settings()
            .get::<MediaMaxMegapixels>()
            .await
            .unwrap_or_else(|error| {
                log::warn!("media limit not read, using the default: {error}");
                100
            })
    }

    async fn row(&self, id: Uuid) -> Result<MediaRow, MediaError> {
        rows::get(self.database.reader(), &id_text(id))
            .await?
            .ok_or(MediaError::NotFound)
    }

    /// Media outside the trash, newest first.
    pub async fn list(&self) -> Result<Vec<MediaSummary>, MediaError> {
        Ok(rows::list_active(self.database.reader())
            .await?
            .iter()
            .map(MediaSummary::from)
            .collect())
    }

    /// Media in the trash, last deleted first.
    pub async fn list_trash(&self) -> Result<Vec<MediaSummary>, MediaError> {
        Ok(rows::list_trash(self.database.reader())
            .await?
            .iter()
            .map(MediaSummary::from)
            .collect())
    }

    pub async fn get(&self, id: Uuid) -> Result<MediaSummary, MediaError> {
        Ok(MediaSummary::from(&self.row(id).await?))
    }

    /// Changes title and description; a title only changes the row.
    pub async fn update_details(
        &self,
        id: Uuid,
        title: Option<&str>,
        description: Option<&str>,
    ) -> Result<MediaSummary, MediaError> {
        let title = title.map(validate_title).transpose()?;
        let description = description.map(validate_description).transpose()?;
        let _guard = self.write_lock.lock().await;
        let row = self.row(id).await?;
        let title = title.unwrap_or(row.title.clone());
        let description = description.unwrap_or(row.description.clone());
        if title != row.title || description != row.description {
            rows::update_details(
                self.database.writer(),
                &row.id,
                &title,
                &description,
                &self.generators.now().to_iso(),
            )
            .await?;
        }
        drop(_guard);
        self.get(id).await
    }

    pub async fn set_favorite(&self, id: Uuid, favorite: bool) -> Result<MediaSummary, MediaError> {
        let now = self.generators.now().to_iso();
        let _guard = self.write_lock.lock().await;
        let row = self.row(id).await?;
        if favorite != row.favorited_at.is_some() {
            rows::set_favorited(
                self.database.writer(),
                &row.id,
                favorite.then_some(now.as_str()),
            )
            .await?;
        }
        drop(_guard);
        self.get(id).await
    }

    /// Records that a medium was opened ("Zuletzt verwendet").
    pub async fn mark_opened(&self, id: Uuid) -> Result<MediaSummary, MediaError> {
        let now = self.generators.now().to_iso();
        let _guard = self.write_lock.lock().await;
        let row = self.row(id).await?;
        rows::set_last_opened(self.database.writer(), &row.id, &now).await?;
        drop(_guard);
        self.get(id).await
    }

    /// The PDF view reports the page count once it has the file.
    pub async fn set_page_count(&self, id: Uuid, pages: u32) -> Result<(), MediaError> {
        if pages == 0 || pages > MAX_PAGES {
            return Err(MediaError::InvalidValue("page count"));
        }
        let _guard = self.write_lock.lock().await;
        let row = self.row(id).await?;
        if row.kind != MediaKind::Pdf.as_str() {
            return Err(MediaError::InvalidValue("not a PDF"));
        }
        if row.page_count != Some(i64::from(pages)) {
            rows::set_page_count(self.database.writer(), &row.id, i64::from(pages)).await?;
        }
        Ok(())
    }

    /// Documents that use a medium.
    pub async fn uses(&self, id: Uuid) -> Result<Vec<MediaUse>, MediaError> {
        let row = self.row(id).await?;
        Ok(rows::users_of(self.database.reader(), &row.id)
            .await?
            .into_iter()
            .map(|(document_id, page)| MediaUse {
                document_id,
                page: u32::try_from(page).unwrap_or(0),
            })
            .collect())
    }

    // --- Files for the media handler --------------------------------------------------

    /// The file of an image or PDF. Attachments are not served: Project
    /// Write reads nothing from them (section 17).
    pub async fn served_file(&self, id: Uuid) -> Result<ServedFile, MediaError> {
        let row = self.row(id).await?;
        let media_type = MediaType::from_mime_type(&row.mime_type).ok_or(MediaError::NotFound)?;
        if media_type.kind() == MediaKind::Attachment {
            return Err(MediaError::NotFound);
        }
        let path =
            self.root
                .resolve_file(&relative_path(media_type.kind(), &row.id, &row.extension))?;
        Ok(ServedFile {
            path,
            mime_type: media_type.mime_type(),
        })
    }

    /// The thumbnail of a medium: for raster images made on demand if it
    /// is missing; `None` for SVG (the image itself serves), for a PDF whose
    /// thumbnail the interface has not drawn yet, and for attachments.
    pub async fn thumbnail(&self, id: Uuid) -> Result<Option<ServedFile>, MediaError> {
        let row = self.row(id).await?;
        let media_type = MediaType::from_mime_type(&row.mime_type).ok_or(MediaError::NotFound)?;
        if media_type == MediaType::Pdf {
            let relative = format!("{PDF_THUMBNAILS_DIR}/{}.png", row.id);
            return Ok(self.existing_file(&relative)?.map(|path| ServedFile {
                path,
                mime_type: "image/png",
            }));
        }
        if !media_type.is_raster() {
            return Ok(None);
        }
        for format in [ThumbnailFormat::Jpeg, ThumbnailFormat::Png] {
            let relative = format!("{IMAGE_THUMBNAILS_DIR}/{}.{}", row.id, format.extension());
            if let Some(path) = self.existing_file(&relative)? {
                return Ok(Some(ServedFile {
                    path,
                    mime_type: format.mime_type(),
                }));
            }
        }
        let source =
            self.root
                .resolve_file(&relative_path(MediaKind::Image, &row.id, &row.extension))?;
        let max_megapixels = self.max_megapixels().await;
        let rendered =
            tokio::task::spawn_blocking(move || thumbnails::render(&source, max_megapixels))
                .await
                .map_err(|error| io::Error::other(error.to_string()))?;
        let (bytes, format) = match rendered {
            Ok(rendered) => rendered,
            Err(error) => {
                log::warn!("no thumbnail for medium {}: {error}", row.id);
                return Ok(None);
            }
        };
        self.root.create_dir_all(IMAGE_THUMBNAILS_DIR)?;
        let relative = format!("{IMAGE_THUMBNAILS_DIR}/{}.{}", row.id, format.extension());
        let path = self.root.resolve(&relative)?;
        write_atomic(&path, &bytes)?;
        Ok(Some(ServedFile {
            path,
            mime_type: format.mime_type(),
        }))
    }

    fn existing_file(&self, relative: &str) -> Result<Option<PathBuf>, MediaError> {
        let path = self.root.resolve(relative)?;
        match fs::symlink_metadata(&path) {
            Ok(metadata) if metadata.is_file() => Ok(Some(path)),
            Ok(_) => Ok(None),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(error.into()),
        }
    }

    /// Stores the thumbnail of a PDF that the PDF view drew (ADR 0019,
    /// decision 7).
    pub async fn store_pdf_thumbnail(&self, id: Uuid, png: &[u8]) -> Result<(), MediaError> {
        let row = self.row(id).await?;
        if row.kind != MediaKind::Pdf.as_str() {
            return Err(MediaError::InvalidValue("not a PDF"));
        }
        thumbnails::check_pdf_thumbnail(png)?;
        self.root.create_dir_all(PDF_THUMBNAILS_DIR)?;
        let path = self
            .root
            .resolve(&format!("{PDF_THUMBNAILS_DIR}/{}.png", row.id))?;
        write_atomic(&path, png)?;
        Ok(())
    }

    /// Makes the missing thumbnails of new images, one after the other
    /// (section 9: "Vorschaubild im Hintergrund erzeugen").
    pub(crate) fn spawn_thumbnails(&self, ids: Vec<Uuid>) {
        if ids.is_empty() {
            return;
        }
        let media = self.clone();
        tokio::spawn(async move {
            for id in ids {
                if let Err(error) = media.thumbnail(id).await {
                    log::warn!("thumbnail of medium {id} not made: {error}");
                }
            }
        });
    }

    // --- Removing for good ---------------------------------------------------------------

    /// Removes a medium from the trash for good: row, file and thumbnail.
    /// A medium a document still uses stays (section 9: "Dateien werden nur
    /// gelöscht, wenn sie nicht mehr referenziert werden").
    pub async fn purge(&self, id: Uuid) -> Result<(), MediaError> {
        let _guard = self.write_lock.lock().await;
        self.purge_locked(id).await
    }

    async fn purge_locked(&self, id: Uuid) -> Result<(), MediaError> {
        let row = self.row(id).await?;
        if row.deleted_at.is_none() {
            return Err(MediaError::NotInTrash);
        }
        let users = rows::user_count(self.database.reader(), &row.id).await?;
        if users > 0 {
            return Err(MediaError::InUse { documents: users });
        }
        let mut transaction = self.database.writer().begin().await?;
        let removed = rows::delete_trashed_unused(&mut transaction, &row.id).await?;
        transaction.commit().await?;
        if removed == 0 {
            // A document started to use it in the meantime.
            let documents = rows::user_count(self.database.reader(), &row.id).await?;
            return Err(MediaError::InUse { documents });
        }
        let kind = MediaKind::parse(&row.kind).unwrap_or(MediaKind::Attachment);
        self.remove_files(kind, &row.id, &row.extension);
        Ok(())
    }

    /// Removes the file and thumbnails of a medium whose row is gone.
    /// Failures are logged: the row is gone, the files are reported as
    /// orphans at the next opening.
    fn remove_files(&self, kind: MediaKind, id: &str, extension: &str) {
        let mut relative = vec![relative_path(kind, id, extension)];
        relative.push(format!("{IMAGE_THUMBNAILS_DIR}/{id}.jpg"));
        relative.push(format!("{IMAGE_THUMBNAILS_DIR}/{id}.png"));
        relative.push(format!("{PDF_THUMBNAILS_DIR}/{id}.png"));
        for relative in relative {
            match self.existing_file(&relative) {
                Ok(Some(path)) => {
                    if let Err(error) = fs::remove_file(&path) {
                        log::warn!("{relative} not removed: {error}");
                    }
                }
                Ok(None) => {}
                Err(error) => log::warn!("{relative} not removed: {error}"),
            }
        }
    }

    /// Removes media that have been in the trash longer than the retention
    /// period, except those documents still use.
    pub async fn purge_expired(&self, retention_days: u32) -> Result<PurgeReport, MediaError> {
        let cutoff = Timestamp::from_unix_millis(
            self.generators.now().unix_millis() - i64::from(retention_days) * 86_400_000,
        )
        .unwrap_or(Timestamp::from_unix_millis(0).unwrap_or(self.generators.now()));
        let ids = rows::trashed_before(self.database.reader(), &cutoff.to_iso()).await?;
        self.purge_all(ids).await
    }

    /// "Papierkorb leeren" for media.
    pub async fn empty_trash(&self) -> Result<PurgeReport, MediaError> {
        let ids: Vec<String> = rows::list_trash(self.database.reader())
            .await?
            .into_iter()
            .map(|row| row.id)
            .collect();
        self.purge_all(ids).await
    }

    async fn purge_all(&self, ids: Vec<String>) -> Result<PurgeReport, MediaError> {
        let _guard = self.write_lock.lock().await;
        let mut report = PurgeReport::default();
        for id in ids {
            let Ok(id) = Uuid::try_parse(&id) else {
                continue;
            };
            match self.purge_locked(id).await {
                Ok(()) => report.removed += 1,
                Err(MediaError::InUse { .. }) => report.kept_in_use += 1,
                Err(MediaError::NotFound | MediaError::NotInTrash) => {}
                Err(error) => return Err(error),
            }
        }
        Ok(report)
    }

    // --- Opening with the standard program ---------------------------------------------

    /// Copies a medium read-only to `tmp/open/<id>/<original name>` for the
    /// standard program; changes there never reach the library (ADR 0019,
    /// decision 14). Returns the path of the copy.
    pub async fn open_copy(&self, id: Uuid) -> Result<PathBuf, MediaError> {
        let row = self.row(id).await?;
        let kind = MediaKind::parse(&row.kind).ok_or(MediaError::NotFound)?;
        let source = self
            .root
            .resolve_file(&relative_path(kind, &row.id, &row.extension))?;
        let name = copy_name(&row);
        let dir = format!("{OPEN_DIR}/{}", row.id);
        self.root.create_dir_all(&dir)?;
        let target = self.root.resolve(&format!("{dir}/{name}"))?;
        tokio::task::spawn_blocking(move || -> io::Result<PathBuf> {
            if let Ok(metadata) = fs::symlink_metadata(&target) {
                if metadata.is_file() {
                    let mut permissions = metadata.permissions();
                    #[allow(clippy::permissions_set_readonly_false)]
                    permissions.set_readonly(false);
                    fs::set_permissions(&target, permissions)?;
                    fs::remove_file(&target)?;
                } else {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidInput,
                        "not a regular file",
                    ));
                }
            }
            fs::copy(&source, &target)?;
            let mut permissions = fs::metadata(&target)?.permissions();
            permissions.set_readonly(true);
            fs::set_permissions(&target, permissions)?;
            Ok(target)
        })
        .await
        .map_err(|error| io::Error::other(error.to_string()))?
        .map_err(MediaError::from)
    }
}

/// File name of the copy for the standard program: the original name if it
/// is a safe single component with the right extension, else title plus
/// extension.
fn copy_name(row: &MediaRow) -> String {
    let fits = |name: &str| {
        crate::security::paths::validate_component(name).is_ok()
            && detect::extension_of(name).as_deref() == Some(row.extension.as_str())
            && !is_temp_file_name(name)
    };
    if fits(&row.original_name) {
        return row.original_name.clone();
    }
    let stem: String = row
        .title
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == ' ' || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .take(100)
        .collect();
    let candidate = format!("{}.{}", stem.trim(), row.extension);
    if fits(&candidate) {
        candidate
    } else {
        format!("{}.{}", row.id, row.extension)
    }
}

// --- Maintenance when a library opens ---------------------------------------------------

/// An import journal left in `tmp/` (ADR 0019, decision 5).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportJournal {
    pub batch: String,
    pub files: Vec<JournalFile>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JournalFile {
    pub id: String,
    pub relative: String,
}

/// Name of the journal of a batch.
pub fn journal_relative(batch: Uuid) -> String {
    format!("{TMP_DIR}/import-{}.json", id_text(batch))
}

/// Reads the import journals in `tmp/` before it is emptied.
pub fn read_import_journals(root: &SafeRoot) -> Vec<ImportJournal> {
    let Ok(dir) = root.resolve(TMP_DIR) else {
        return Vec::new();
    };
    let Ok(entries) = fs::read_dir(&dir) else {
        return Vec::new();
    };
    let mut journals = Vec::new();
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        if !(name.starts_with("import-") && name.ends_with(".json")) {
            continue;
        }
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        if !file_type.is_file() {
            continue;
        }
        match read_limited(&entry.path(), 16 * 1024 * 1024)
            .ok()
            .and_then(|data| serde_json::from_slice::<ImportJournal>(&data).ok())
        {
            Some(journal) => journals.push(journal),
            None => log::warn!("import journal {name} not readable"),
        }
    }
    journals
}

/// Removes the files of imports that did not finish: files a journal names
/// whose row does not exist (the transaction did not commit). Returns how
/// many files were removed.
pub async fn recover_imports(
    root: &SafeRoot,
    database: &Database,
    journals: &[ImportJournal],
) -> Result<usize, MediaError> {
    let mut removed = 0;
    for journal in journals {
        let ids: Vec<String> = journal.files.iter().map(|file| file.id.clone()).collect();
        let present: BTreeSet<String> = rows::ids_of(database.reader(), &ids)
            .await?
            .into_iter()
            .collect();
        for file in &journal.files {
            if present.contains(&file.id) || !is_media_file(&file.relative, &file.id) {
                continue;
            }
            let path = root.resolve(&file.relative)?;
            match fs::symlink_metadata(&path) {
                Ok(metadata) if metadata.is_file() => {
                    fs::remove_file(&path)?;
                    removed += 1;
                }
                _ => {}
            }
        }
        if removed > 0 {
            log::info!(
                "unfinished import {}: {removed} files removed",
                journal.batch
            );
        }
    }
    Ok(removed)
}

/// Only paths below the media folders named after the id are touched.
fn is_media_file(relative: &str, id: &str) -> bool {
    [MediaKind::Image, MediaKind::Pdf, MediaKind::Attachment]
        .iter()
        .any(|kind| {
            relative
                .strip_prefix(kind.directory())
                .and_then(|rest| rest.strip_prefix('/'))
                .and_then(|name| name.strip_prefix(id))
                .and_then(|rest| rest.strip_prefix('.'))
                .is_some_and(|extension| detect::EXTENSIONS.contains(&extension))
        })
}

/// What [`scan_media`] found.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MediaScan {
    /// Files in the media folders without a row. Reported, never deleted.
    pub orphaned_files: Vec<String>,
    /// Rows whose file is missing.
    pub missing_files: Vec<String>,
}

/// Compares the media folders with the `media` table (section 9: "Dateien
/// ohne Datenbankeintrag werden gemeldet, nicht still gelöscht").
pub async fn scan_media(root: &SafeRoot, database: &Database) -> Result<MediaScan, MediaError> {
    let keys = rows::all_file_keys(database.reader()).await?;
    let expected: BTreeSet<String> = keys
        .iter()
        .filter_map(|(id, kind, extension)| {
            MediaKind::parse(kind).map(|kind| relative_path(kind, id, extension))
        })
        .collect();
    let mut found = BTreeSet::new();
    for kind in [MediaKind::Image, MediaKind::Pdf, MediaKind::Attachment] {
        let dir = root.resolve(kind.directory())?;
        let Ok(entries) = fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            found.insert(format!("{}/{name}", kind.directory()));
        }
    }
    Ok(MediaScan {
        orphaned_files: found.difference(&expected).cloned().collect(),
        missing_files: expected.difference(&found).cloned().collect(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn titles_come_from_file_names() {
        assert_eq!(title_from_name("Urlaub 2026.jpg"), "Urlaub 2026");
        assert_eq!(title_from_name(".png"), ".png");
        assert_eq!(title_from_name("ohne"), "ohne");
        assert_eq!(title_from_name("a\u{7}b.pdf"), "a b");
        assert!(validate_title("  ").is_err());
        assert!(validate_description("Zeile\nZeile").is_ok());
    }

    #[test]
    fn journals_only_touch_files_of_their_ids_in_the_media_folders() {
        let id = "6c1d2e3f-4a5b-4c6d-8e7f-9a0b1c2d3e4f";
        assert!(is_media_file(&format!("media/images/{id}.png"), id));
        assert!(is_media_file(&format!("media/pdf/{id}.pdf"), id));
        assert!(!is_media_file(&format!("documents/text/{id}.txt"), id));
        assert!(!is_media_file(&format!("media/images/{id}.exe"), id));
        assert!(!is_media_file("media/images/other.png", id));
        assert!(!is_media_file(&format!("media/images/../{id}.png"), id));
    }
}
