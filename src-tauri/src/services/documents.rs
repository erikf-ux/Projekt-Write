//! Document repository (Phase 2, "Dokument-Repository im Backend").
//!
//! A document is a row in `documents` plus a file in the library:
//! `documents/<folder>/<id>.<ext>` (section 9). File names are UUIDs; titles
//! live only in the database, so renaming never touches the file.
//!
//! Write order, chosen so that a crash never loses saved content:
//!
//! * **Create:** write the file atomically, then insert the row. A crash in
//!   between leaves a file without a row; opening the library reports it and
//!   never deletes it.
//! * **Save:** write the file atomically, then update size and checksum. A
//!   crash in between leaves the new content in the file and the old
//!   checksum in the row; the next read notices the difference and corrects
//!   the row.
//!
//! All writes of one library are serialised by a lock, so two saves of the
//! same document cannot interleave.

use std::io;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::clock::{Generators, Timestamp};
use crate::database::Database;
use crate::database::documents::{self as rows, DocumentRow};
use crate::filesystem::atomic;
use crate::security::paths::{PathError, SafeRoot};
use crate::services::history::{History, HistoryError, VersionKind, VersionSummary};
use crate::services::pwrite;

/// Largest document content accepted (64 MiB). A 50,000-word text is about
/// 400 KB; the limit only protects memory.
pub const MAX_DOCUMENT_BYTES: u64 = 64 * 1024 * 1024;
/// Longest title in characters.
pub const MAX_TITLE_CHARS: usize = 300;

/// Storage format of a document (section 10).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, ts_rs::TS)]
#[serde(rename_all = "lowercase")]
#[ts(export)]
pub enum DocumentFormat {
    /// Plain text, `documents/text/<id>.txt`.
    Txt,
    /// Markdown, `documents/markdown/<id>.md`.
    Md,
    /// Rich text, `documents/richtext/<id>.pwrite`.
    Pwrite,
}

impl DocumentFormat {
    pub const ALL: [DocumentFormat; 3] = [
        DocumentFormat::Txt,
        DocumentFormat::Md,
        DocumentFormat::Pwrite,
    ];

    /// Value of the `format` column.
    pub const fn as_str(self) -> &'static str {
        match self {
            DocumentFormat::Txt => "txt",
            DocumentFormat::Md => "md",
            DocumentFormat::Pwrite => "pwrite",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|format| format.as_str() == value)
    }

    /// Folder of the files, relative to the library root.
    pub const fn directory(self) -> &'static str {
        match self {
            DocumentFormat::Txt => "documents/text",
            DocumentFormat::Md => "documents/markdown",
            DocumentFormat::Pwrite => "documents/richtext",
        }
    }

    /// File extension (equal to the column value).
    pub const fn extension(self) -> &'static str {
        self.as_str()
    }

    /// `documents/<folder>/<id>.<ext>`.
    pub fn relative_path(self, id: Uuid) -> String {
        format!(
            "{}/{}.{}",
            self.directory(),
            id.hyphenated(),
            self.extension()
        )
    }

    /// The id encoded in a file name of this format's folder, if the name is
    /// exactly `<lower-case uuid>.<ext>`.
    pub fn id_from_file_name(self, name: &str) -> Option<Uuid> {
        let stem = name.strip_suffix(self.extension())?.strip_suffix('.')?;
        let id = Uuid::try_parse(stem).ok()?;
        (id.hyphenated().to_string() == stem).then_some(id)
    }
}

/// Metadata of a document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocumentMeta {
    pub id: Uuid,
    pub title: String,
    pub format: DocumentFormat,
    pub size_bytes: u64,
    /// SHA-256 of the file content, lower-case hex.
    pub content_sha256: String,
    pub created_at: String,
    pub updated_at: String,
    pub archived_at: Option<String>,
    pub deleted_at: Option<String>,
    /// File name at import, for imported documents.
    pub original_name: Option<String>,
    /// Favourite since (section 13).
    pub favorited_at: Option<String>,
    /// Last opened in a tab ("Zuletzt verwendet", section 13).
    pub last_opened_at: Option<String>,
}

/// Result of [`DocumentRepository::save`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SaveOutcome {
    /// The content changed and was written.
    Saved(DocumentMeta),
    /// The content equals the stored content; nothing was written.
    Unchanged(DocumentMeta),
}

impl SaveOutcome {
    pub fn meta(&self) -> &DocumentMeta {
        match self {
            SaveOutcome::Saved(meta) | SaveOutcome::Unchanged(meta) => meta,
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum DocumentError {
    #[error("document not found")]
    NotFound,
    #[error("document is in the trash")]
    InTrash,
    #[error("invalid title: {0}")]
    InvalidTitle(&'static str),
    #[error("document content is larger than {MAX_DOCUMENT_BYTES} bytes")]
    TooLarge,
    #[error("stored document row is invalid: {0}")]
    InvalidRow(String),
    #[error(transparent)]
    Path(#[from] PathError),
    #[error("file error: {0}")]
    Io(#[from] io::Error),
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),
    #[error("not a valid .pwrite file")]
    InvalidPwrite,
    #[error("document is read only")]
    ReadOnly,
    #[error("the document has this format already")]
    SameFormat,
    #[error("the document is not in the trash")]
    NotInTrash,
    #[error("history: {0}")]
    History(#[from] HistoryError),
    #[error("content is not UTF-8 text")]
    NotText,
}

/// Trims a title and checks length and characters.
pub fn validate_title(title: &str) -> Result<String, DocumentError> {
    let title = title.trim();
    if title.is_empty() {
        return Err(DocumentError::InvalidTitle("empty"));
    }
    if title.chars().count() > MAX_TITLE_CHARS {
        return Err(DocumentError::InvalidTitle("too long"));
    }
    if title.chars().any(char::is_control) {
        return Err(DocumentError::InvalidTitle("control character"));
    }
    Ok(title.to_owned())
}

/// SHA-256 of `data` as lower-case hex.
pub fn sha256_hex(data: &[u8]) -> String {
    let digest = Sha256::digest(data);
    let mut hex = String::with_capacity(64);
    for byte in digest {
        use std::fmt::Write;
        let _ = write!(hex, "{byte:02x}");
    }
    hex
}

/// Access to the documents of one open library.
#[derive(Debug, Clone)]
pub struct DocumentRepository {
    root: SafeRoot,
    database: Database,
    generators: Generators,
    write_lock: Arc<tokio::sync::Mutex<()>>,
}

impl DocumentRepository {
    pub(crate) fn new(
        root: SafeRoot,
        database: Database,
        generators: Generators,
        write_lock: Arc<tokio::sync::Mutex<()>>,
    ) -> Self {
        Self {
            root,
            database,
            generators,
            write_lock,
        }
    }

    /// Creates a document with `content`.
    pub async fn create(
        &self,
        title: &str,
        format: DocumentFormat,
        content: &[u8],
    ) -> Result<DocumentMeta, DocumentError> {
        self.insert_new(title, format, content, None).await
    }

    /// Creates a `.pwrite` document with `content` (an editor document); the
    /// envelope gets the new id, the title and the time (section 10).
    pub async fn create_pwrite(
        &self,
        title: &str,
        language: &str,
        content: serde_json::Value,
    ) -> Result<DocumentMeta, DocumentError> {
        let language = language.to_owned();
        self.insert_new_with(
            title,
            DocumentFormat::Pwrite,
            None,
            move |id, now, title| {
                Ok(pwrite::to_bytes(&pwrite::new_file(
                    id, title, now, &language, content,
                )))
            },
        )
        .await
    }

    /// Creates a document from an imported file (section 9, "Import in die
    /// Bibliothek"). The caller has checked the type; `original_name` is the
    /// file name at import. The original file is never touched: only its
    /// content arrives here.
    pub async fn import(
        &self,
        title: &str,
        format: DocumentFormat,
        content: &[u8],
        original_name: &str,
    ) -> Result<DocumentMeta, DocumentError> {
        if format != DocumentFormat::Pwrite {
            return self
                .insert_new(title, format, content, Some(original_name.to_owned()))
                .await;
        }
        let pwrite::Parsed::Current(mut file) =
            pwrite::parse(content).map_err(|_| DocumentError::InvalidPwrite)?
        else {
            return Err(DocumentError::ReadOnly);
        };
        // In the library the envelope names the library document.
        self.insert_new_with(
            title,
            DocumentFormat::Pwrite,
            Some(original_name.to_owned()),
            move |id, now, title| {
                file.id = id.to_owned();
                file.title = title.to_owned();
                file.created_at = now.to_owned();
                file.updated_at = now.to_owned();
                Ok(pwrite::to_bytes(&file))
            },
        )
        .await
    }

    /// A document not in the trash with exactly `content`, if any.
    pub async fn find_duplicate(
        &self,
        content: &[u8],
    ) -> Result<Option<DocumentMeta>, DocumentError> {
        rows::find_active_by_sha256(self.database.reader(), &sha256_hex(content))
            .await?
            .map(meta_from_row)
            .transpose()
    }

    /// The file first, then the row; without a row the file is removed again.
    async fn insert_new(
        &self,
        title: &str,
        format: DocumentFormat,
        content: &[u8],
        original_name: Option<String>,
    ) -> Result<DocumentMeta, DocumentError> {
        let content = content.to_vec();
        self.insert_new_with(title, format, original_name, move |_, _, _| Ok(content))
            .await
    }

    /// As [`Self::insert_new`], with the content built from the new id, the
    /// time and the checked title (`.pwrite` carries them in its envelope).
    async fn insert_new_with(
        &self,
        title: &str,
        format: DocumentFormat,
        original_name: Option<String>,
        build: impl FnOnce(&str, &str, &str) -> Result<Vec<u8>, DocumentError>,
    ) -> Result<DocumentMeta, DocumentError> {
        let title = validate_title(title)?;
        let _guard = self.write_lock.lock().await;
        let id = self.generators.new_id();
        let now = self.generators.now().to_iso();
        let content = build(&id.hyphenated().to_string(), &now, &title)?;
        let content = content.as_slice();
        check_size(content)?;
        let relative = format.relative_path(id);

        let path = self.prepare_path(format, &relative)?;
        let data = content.to_vec();
        let target = path.clone();
        blocking(move || atomic::write_atomic(&target, &data)).await?;

        let row = DocumentRow {
            id: id.hyphenated().to_string(),
            title,
            format: format.as_str().to_owned(),
            size_bytes: i64::try_from(content.len()).map_err(|_| DocumentError::TooLarge)?,
            content_sha256: sha256_hex(content),
            created_at: now.clone(),
            updated_at: now,
            archived_at: None,
            deleted_at: None,
            original_name,
            favorited_at: None,
            last_opened_at: None,
        };
        let links = links_in(format, content);
        if let Err(error) = rows::insert(self.database.writer(), &row, &links).await {
            // Without a row the new file would be an orphan.
            let _ = std::fs::remove_file(&path);
            return Err(error.into());
        }
        meta_from_row(row)
    }

    /// Metadata of a document, also if it is in the trash.
    pub async fn get(&self, id: Uuid) -> Result<Option<DocumentMeta>, DocumentError> {
        rows::get(self.database.reader(), &id.hyphenated().to_string())
            .await?
            .map(meta_from_row)
            .transpose()
    }

    /// Documents not in the trash, most recently changed first.
    pub async fn list(&self) -> Result<Vec<DocumentMeta>, DocumentError> {
        rows::list_active(self.database.reader())
            .await?
            .into_iter()
            .map(meta_from_row)
            .collect()
    }

    /// Number of documents not in the trash.
    pub async fn count(&self) -> Result<u64, DocumentError> {
        let count = rows::count_active(self.database.reader()).await?;
        Ok(u64::try_from(count).unwrap_or(0))
    }

    /// Reads the content. If the file differs from the recorded size or
    /// checksum (crash between file and row update, or a change from
    /// outside), the row is corrected and the file content wins.
    pub async fn read(&self, id: Uuid) -> Result<(DocumentMeta, Vec<u8>), DocumentError> {
        let meta = self.get(id).await?.ok_or(DocumentError::NotFound)?;
        let relative = meta.format.relative_path(meta.id);
        let root = self.root.clone();
        let content = blocking(move || -> Result<Vec<u8>, DocumentError> {
            let path = root.resolve_file(&relative)?;
            Ok(atomic::read_limited(&path, MAX_DOCUMENT_BYTES)?)
        })
        .await?;

        let sha = sha256_hex(&content);
        if sha == meta.content_sha256 && content.len() as u64 == meta.size_bytes {
            return Ok((meta, content));
        }
        log::warn!(
            "document {} differs from its recorded checksum; correcting the record",
            meta.id
        );
        let _guard = self.write_lock.lock().await;
        let size = i64::try_from(content.len()).map_err(|_| DocumentError::TooLarge)?;
        let now = self.generators.now().to_iso();
        rows::update_content(
            self.database.writer(),
            &meta.id.hyphenated().to_string(),
            size,
            &sha,
            &now,
            &links_in(meta.format, &content),
        )
        .await?;
        let meta = self.get(id).await?.ok_or(DocumentError::NotFound)?;
        Ok((meta, content))
    }

    /// Saves new content. Unchanged content is not written again.
    pub async fn save(&self, id: Uuid, content: &[u8]) -> Result<SaveOutcome, DocumentError> {
        check_size(content)?;
        let _guard = self.write_lock.lock().await;
        let meta = self.get(id).await?.ok_or(DocumentError::NotFound)?;
        if meta.deleted_at.is_some() {
            return Err(DocumentError::InTrash);
        }
        let sha = sha256_hex(content);
        if sha == meta.content_sha256 && content.len() as u64 == meta.size_bytes {
            return Ok(SaveOutcome::Unchanged(meta));
        }

        let relative = meta.format.relative_path(meta.id);
        let path = self.prepare_path(meta.format, &relative)?;
        let data = content.to_vec();
        blocking(move || atomic::write_atomic(&path, &data)).await?;

        let size = i64::try_from(content.len()).map_err(|_| DocumentError::TooLarge)?;
        let now = self.generators.now().to_iso();
        rows::update_content(
            self.database.writer(),
            &meta.id.hyphenated().to_string(),
            size,
            &sha,
            &now,
            &links_in(meta.format, content),
        )
        .await?;
        // Stage 4: one version per 10 minutes of editing. A failure here
        // must not fail the save; the content is safe in its file.
        if let Err(error) = self
            .history()
            .record_auto_if_due(meta.id, meta.format, content)
            .await
        {
            log::warn!("automatic version of {} not stored: {error}", meta.id);
        }
        let meta = self.get(id).await?.ok_or(DocumentError::NotFound)?;
        Ok(SaveOutcome::Saved(meta))
    }

    /// Changes the title. The file is not touched (section 13).
    pub async fn rename(&self, id: Uuid, title: &str) -> Result<DocumentMeta, DocumentError> {
        let title = validate_title(title)?;
        let _guard = self.write_lock.lock().await;
        let now = self.generators.now().to_iso();
        let changed = rows::update_title(
            self.database.writer(),
            &id.hyphenated().to_string(),
            &title,
            &now,
        )
        .await?;
        if changed == 0 {
            return Err(DocumentError::NotFound);
        }
        self.get(id).await?.ok_or(DocumentError::NotFound)
    }

    /// Copies a document (section 13): a new id and `title`, the same format
    /// and content. The history is not copied (Phase 6); tags and subnets
    /// follow with Phase 7.
    pub async fn copy(&self, id: Uuid, title: &str) -> Result<DocumentMeta, DocumentError> {
        let (meta, content) = self.read(id).await?;
        if meta.deleted_at.is_some() {
            return Err(DocumentError::InTrash);
        }
        if meta.format != DocumentFormat::Pwrite {
            return self.insert_new(title, meta.format, &content, None).await;
        }
        // The envelope of the copy names the copy.
        let pwrite::Parsed::Current(mut file) =
            pwrite::parse(&content).map_err(|_| DocumentError::InvalidPwrite)?
        else {
            return Err(DocumentError::ReadOnly);
        };
        self.insert_new_with(
            title,
            DocumentFormat::Pwrite,
            None,
            move |id, now, title| {
                file.id = id.to_owned();
                file.title = title.to_owned();
                file.created_at = now.to_owned();
                file.updated_at = now.to_owned();
                Ok(pwrite::to_bytes(&file))
            },
        )
        .await
    }

    /// Saves new content of a `.pwrite` document (an editor document). The
    /// envelope keeps its unknown fields and gets the title and the time;
    /// unchanged content and title are not written again.
    pub async fn save_pwrite(
        &self,
        id: Uuid,
        content: serde_json::Value,
    ) -> Result<SaveOutcome, DocumentError> {
        pwrite::check_content(&content).map_err(|_| DocumentError::InvalidPwrite)?;
        let (meta, stored) = self.read(id).await?;
        if meta.format != DocumentFormat::Pwrite {
            return Err(DocumentError::InvalidPwrite);
        }
        let pwrite::Parsed::Current(mut file) =
            pwrite::parse(&stored).map_err(|_| DocumentError::InvalidPwrite)?
        else {
            return Err(DocumentError::ReadOnly);
        };
        if file.content == content && file.title == meta.title {
            return Ok(SaveOutcome::Unchanged(meta));
        }
        file.content = content;
        file.title.clone_from(&meta.title);
        file.updated_at = self.generators.now().to_iso();
        self.save(id, &pwrite::to_bytes(&file)).await
    }

    /// Converts a document to another format (section 10; ADR 0015,
    /// decision 13). The frontend converts the content; `content` is the
    /// text of the new format (`.pwrite`: the editor content as JSON). The
    /// id stays. The new file is written first, then the row changes, the
    /// old file goes last: a crash in between leaves at most an orphaned
    /// file, which the check on opening reports, never a row without file.
    pub async fn convert(
        &self,
        id: Uuid,
        format: DocumentFormat,
        content: &[u8],
        language: &str,
    ) -> Result<DocumentMeta, DocumentError> {
        let (meta, stored) = self.read(id).await?;
        if meta.deleted_at.is_some() {
            return Err(DocumentError::InTrash);
        }
        if meta.format == format {
            return Err(DocumentError::SameFormat);
        }
        if meta.format == DocumentFormat::Pwrite
            && matches!(pwrite::parse(&stored), Ok(pwrite::Parsed::Newer { .. }))
        {
            return Err(DocumentError::ReadOnly);
        }
        // The old content in the old format first (section 13); without it
        // nothing is converted.
        self.history()
            .record(
                meta.id,
                VersionKind::BeforeConversion,
                None,
                meta.format,
                &stored,
            )
            .await?;
        let now = self.generators.now().to_iso();
        let data = if format == DocumentFormat::Pwrite {
            let value: serde_json::Value =
                serde_json::from_slice(content).map_err(|_| DocumentError::InvalidPwrite)?;
            pwrite::check_content(&value).map_err(|_| DocumentError::InvalidPwrite)?;
            let mut file = pwrite::new_file(
                &meta.id.hyphenated().to_string(),
                &meta.title,
                &meta.created_at,
                language,
                value,
            );
            file.updated_at.clone_from(&now);
            pwrite::to_bytes(&file)
        } else {
            std::str::from_utf8(content).map_err(|_| DocumentError::NotText)?;
            content.to_vec()
        };
        check_size(&data)?;

        let _guard = self.write_lock.lock().await;
        let new_path = self.prepare_path(format, &format.relative_path(meta.id))?;
        let old_path = self.prepare_path(meta.format, &meta.format.relative_path(meta.id))?;
        let size = i64::try_from(data.len()).map_err(|_| DocumentError::TooLarge)?;
        let sha = sha256_hex(&data);
        let links = links_in(format, &data);
        let target = new_path.clone();
        blocking(move || atomic::write_atomic(&target, &data)).await?;
        let updated = rows::update_format(
            self.database.writer(),
            &meta.id.hyphenated().to_string(),
            format.as_str(),
            size,
            &sha,
            &now,
            &links,
        )
        .await;
        match updated {
            Ok(1) => {}
            Ok(_) => {
                let _ = std::fs::remove_file(&new_path);
                return Err(DocumentError::NotFound);
            }
            Err(error) => {
                let _ = std::fs::remove_file(&new_path);
                return Err(error.into());
            }
        }
        if let Err(error) = std::fs::remove_file(&old_path)
            && error.kind() != std::io::ErrorKind::NotFound
        {
            log::warn!(
                "old file of converted document {} not removed: {error}",
                meta.id
            );
        }
        log::info!(
            "document {} converted from {} to {}",
            meta.id,
            meta.format.as_str(),
            format.as_str()
        );
        self.get(id).await?.ok_or(DocumentError::NotFound)
    }

    /// Documents that link to `id` ("Verbindungen", ADR 0015, decision 14).
    pub async fn backlinks(&self, id: Uuid) -> Result<Vec<DocumentMeta>, DocumentError> {
        rows::backlinks(self.database.reader(), &id.hyphenated().to_string())
            .await?
            .into_iter()
            .map(meta_from_row)
            .collect()
    }

    /// The documents `id` links to, as stored.
    pub async fn links_of(&self, id: Uuid) -> Result<Vec<Uuid>, DocumentError> {
        Ok(
            rows::links_of(self.database.reader(), &id.hyphenated().to_string())
                .await?
                .iter()
                .filter_map(|target| Uuid::try_parse(target).ok())
                .collect(),
        )
    }

    /// The history of this library's documents.
    pub fn history(&self) -> History {
        History::new(self.database.clone(), self.generators.clone())
    }

    /// Stores the current content as a version (tab closed, "Version
    /// speichern"). Unchanged content names or reuses the newest version.
    pub async fn record_version(
        &self,
        id: Uuid,
        kind: VersionKind,
        name: Option<&str>,
    ) -> Result<VersionSummary, DocumentError> {
        let (meta, content) = self.read(id).await?;
        if meta.deleted_at.is_some() {
            return Err(DocumentError::InTrash);
        }
        Ok(self
            .history()
            .record(meta.id, kind, name, meta.format, &content)
            .await?)
    }

    /// Restores a version (stage 4): the current content becomes a version
    /// `beforeRestore` first, then the version is written – in its own
    /// format, if it was taken before a conversion.
    pub async fn restore_version(
        &self,
        version_id: Uuid,
        language: &str,
    ) -> Result<DocumentMeta, DocumentError> {
        let (version, content) = self.history().content(version_id).await?;
        let id = Uuid::try_parse(&version.document_id).map_err(|_| DocumentError::NotFound)?;
        let (meta, current) = self.read(id).await?;
        if meta.deleted_at.is_some() {
            return Err(DocumentError::InTrash);
        }
        self.history()
            .record(
                meta.id,
                VersionKind::BeforeRestore,
                None,
                meta.format,
                &current,
            )
            .await?;
        let restored = self
            .write_restored(&meta, version.format, &content, language)
            .await?;
        log::info!("version {} of document {} restored", version.id, meta.id);
        Ok(restored)
    }

    /// Writes restored content (a version or a draft) in `format`.
    pub(crate) async fn write_restored(
        &self,
        meta: &DocumentMeta,
        format: DocumentFormat,
        content: &[u8],
        language: &str,
    ) -> Result<DocumentMeta, DocumentError> {
        // `.pwrite` versions hold the whole file; the document keeps its own
        // envelope and takes the content.
        let pwrite_content = || -> Result<serde_json::Value, DocumentError> {
            match pwrite::parse(content).map_err(|_| DocumentError::InvalidPwrite)? {
                pwrite::Parsed::Current(file) => Ok(file.content),
                pwrite::Parsed::Newer { .. } => Err(DocumentError::ReadOnly),
            }
        };
        if format == meta.format {
            if format == DocumentFormat::Pwrite {
                return Ok(self
                    .save_pwrite(meta.id, pwrite_content()?)
                    .await?
                    .meta()
                    .clone());
            }
            return Ok(self.save(meta.id, content).await?.meta().clone());
        }
        let converted = if format == DocumentFormat::Pwrite {
            serde_json::to_vec(&pwrite_content()?).map_err(|_| DocumentError::InvalidPwrite)?
        } else {
            content.to_vec()
        };
        self.convert(meta.id, format, &converted, language).await
    }

    /// The library document stored in `path`, if the file lies in this
    /// library's documents folder (ADR 0015, decision 15: such a file opens
    /// as the library document, not as an external file).
    pub async fn document_at(&self, path: &Path) -> Option<DocumentMeta> {
        let path = std::fs::canonicalize(path).ok()?;
        let name = path.file_name()?.to_str()?;
        for format in DocumentFormat::ALL {
            let Some(id) = format.id_from_file_name(name) else {
                continue;
            };
            let folder = std::fs::canonicalize(self.root.resolve(format.directory()).ok()?).ok()?;
            if path.parent() != Some(folder.as_path()) {
                continue;
            }
            return self
                .get(id)
                .await
                .ok()
                .flatten()
                .filter(|meta| meta.format == format && meta.deleted_at.is_none());
        }
        None
    }

    /// Makes a document a favourite or not. Changes neither content nor
    /// `updated_at`.
    pub async fn set_favorite(
        &self,
        id: Uuid,
        favorite: bool,
    ) -> Result<DocumentMeta, DocumentError> {
        let _guard = self.write_lock.lock().await;
        let meta = self.get(id).await?.ok_or(DocumentError::NotFound)?;
        if meta.favorited_at.is_some() != favorite {
            let now = self.generators.now().to_iso();
            rows::set_favorited(
                self.database.writer(),
                &id.hyphenated().to_string(),
                favorite.then_some(now.as_str()),
            )
            .await?;
        }
        self.get(id).await?.ok_or(DocumentError::NotFound)
    }

    /// Records that a document was opened ("Zuletzt verwendet").
    pub async fn mark_opened(&self, id: Uuid) -> Result<DocumentMeta, DocumentError> {
        let _guard = self.write_lock.lock().await;
        let now = self.generators.now().to_iso();
        let changed =
            rows::set_last_opened(self.database.writer(), &id.hyphenated().to_string(), &now)
                .await?;
        if changed == 0 {
            return Err(DocumentError::NotFound);
        }
        self.get(id).await?.ok_or(DocumentError::NotFound)
    }

    /// Takes a document out of the trash (stage 3, "Papierkorb"). Its links,
    /// versions and its favourite come back with it: they never left.
    pub async fn restore_from_trash(&self, id: Uuid) -> Result<DocumentMeta, DocumentError> {
        let _guard = self.write_lock.lock().await;
        let changed =
            rows::clear_deleted(self.database.writer(), &id.hyphenated().to_string()).await?;
        if changed == 0 {
            return Err(DocumentError::NotFound);
        }
        self.get(id).await?.ok_or(DocumentError::NotFound)
    }

    /// Archives a document or takes it out of the archive (section 4,
    /// "Archiv"). Changes neither content nor `updated_at`.
    pub async fn set_archived(
        &self,
        id: Uuid,
        archived: bool,
    ) -> Result<DocumentMeta, DocumentError> {
        let _guard = self.write_lock.lock().await;
        let now = self.generators.now().to_iso();
        let changed = rows::set_archived(
            self.database.writer(),
            &id.hyphenated().to_string(),
            archived.then_some(now.as_str()),
        )
        .await?;
        if changed == 0 {
            return Err(DocumentError::NotFound);
        }
        self.get(id).await?.ok_or(DocumentError::NotFound)
    }

    /// Documents in the trash, most recently deleted first.
    pub async fn list_trash(&self) -> Result<Vec<DocumentMeta>, DocumentError> {
        rows::list_trash(self.database.reader())
            .await?
            .into_iter()
            .map(meta_from_row)
            .collect()
    }

    /// Removes a document in the trash for good: first the row (with its
    /// links and versions), then its file. A document outside the trash is
    /// never removed.
    pub async fn purge(&self, id: Uuid) -> Result<(), DocumentError> {
        let _guard = self.write_lock.lock().await;
        let meta = self.get(id).await?.ok_or(DocumentError::NotFound)?;
        if meta.deleted_at.is_none() {
            return Err(DocumentError::NotInTrash);
        }
        let changed =
            rows::delete_trashed(self.database.writer(), &meta.id.hyphenated().to_string()).await?;
        if changed == 0 {
            return Err(DocumentError::NotFound);
        }
        // Without a row nothing refers to the file any more (section 9).
        let path = self.root.resolve(&meta.format.relative_path(meta.id))?;
        if let Err(error) = std::fs::remove_file(&path)
            && error.kind() != io::ErrorKind::NotFound
        {
            log::warn!("file of removed document {} not deleted: {error}", meta.id);
        }
        log::info!("document {} removed from the trash for good", meta.id);
        Ok(())
    }

    /// Removes every document that has been in the trash for longer than
    /// `retention_days`. Returns how many went.
    pub async fn purge_expired(&self, retention_days: u32) -> Result<usize, DocumentError> {
        let now = self.generators.now();
        let cutoff = Timestamp::from_unix_millis(
            now.unix_millis() - i64::from(retention_days) * 24 * 60 * 60 * 1000,
        )
        .unwrap_or(now)
        .to_iso();
        let expired = rows::trashed_before(self.database.reader(), &cutoff).await?;
        let mut removed = 0;
        for id in expired.iter().filter_map(|id| Uuid::try_parse(id).ok()) {
            match self.purge(id).await {
                Ok(()) => removed += 1,
                Err(DocumentError::NotFound | DocumentError::NotInTrash) => {}
                Err(error) => return Err(error),
            }
        }
        Ok(removed)
    }

    /// "Papierkorb leeren": removes every document in the trash.
    pub async fn empty_trash(&self) -> Result<usize, DocumentError> {
        let trashed = self.list_trash().await?;
        let mut removed = 0;
        for meta in trashed {
            match self.purge(meta.id).await {
                Ok(()) => removed += 1,
                Err(DocumentError::NotFound | DocumentError::NotInTrash) => {}
                Err(error) => return Err(error),
            }
        }
        Ok(removed)
    }

    /// Moves a document to the trash. Row and file stay (section 13).
    pub async fn move_to_trash(&self, id: Uuid) -> Result<DocumentMeta, DocumentError> {
        let _guard = self.write_lock.lock().await;
        let meta = self.get(id).await?.ok_or(DocumentError::NotFound)?;
        if meta.deleted_at.is_some() {
            return Err(DocumentError::InTrash);
        }
        let now = self.generators.now().to_iso();
        rows::set_deleted(self.database.writer(), &id.hyphenated().to_string(), &now).await?;
        self.get(id).await?.ok_or(DocumentError::NotFound)
    }

    /// Resolves the file path and creates the format folder if needed.
    fn prepare_path(
        &self,
        format: DocumentFormat,
        relative: &str,
    ) -> Result<PathBuf, DocumentError> {
        self.root.create_dir_all(format.directory())?;
        Ok(self.root.resolve(relative)?)
    }
}

/// Prefix of internal links (ADR 0015, decision 14).
pub const INTERNAL_LINK_PREFIX: &str = "pwrite://";

/// The documents a content links to (`pwrite://<id>`), sorted and without
/// repeats. Plain text has no links; Markdown is searched as text, `.pwrite`
/// through the link marks of its content.
pub fn links_in(format: DocumentFormat, content: &[u8]) -> Vec<String> {
    let mut targets = std::collections::BTreeSet::new();
    match format {
        DocumentFormat::Txt => {}
        DocumentFormat::Md => {
            let text = String::from_utf8_lossy(content);
            for (index, _) in text.match_indices(INTERNAL_LINK_PREFIX) {
                if let Some(id) = link_target(&text[index..]) {
                    targets.insert(id);
                }
            }
        }
        DocumentFormat::Pwrite => {
            if let Ok(pwrite::Parsed::Current(file)) = pwrite::parse(content) {
                collect_link_marks(&file.content, &mut targets);
            }
        }
    }
    targets.into_iter().collect()
}

/// The id of `pwrite://<id>` at the start of `text`.
fn link_target(text: &str) -> Option<String> {
    let candidate = text.strip_prefix(INTERNAL_LINK_PREFIX)?.get(..36)?;
    let id = Uuid::try_parse(candidate).ok()?;
    (id.hyphenated().to_string() == candidate).then(|| candidate.to_owned())
}

fn collect_link_marks(node: &serde_json::Value, targets: &mut std::collections::BTreeSet<String>) {
    if let Some(marks) = node.get("marks").and_then(serde_json::Value::as_array) {
        for mark in marks {
            if mark.get("type").and_then(serde_json::Value::as_str) == Some("link")
                && let Some(href) = mark
                    .get("attrs")
                    .and_then(|attrs| attrs.get("href"))
                    .and_then(serde_json::Value::as_str)
                && href.len() == INTERNAL_LINK_PREFIX.len() + 36
                && let Some(id) = link_target(href)
            {
                targets.insert(id);
            }
        }
    }
    if let Some(children) = node.get("content").and_then(serde_json::Value::as_array) {
        for child in children {
            collect_link_marks(child, targets);
        }
    }
}

fn check_size(content: &[u8]) -> Result<(), DocumentError> {
    if content.len() as u64 > MAX_DOCUMENT_BYTES {
        return Err(DocumentError::TooLarge);
    }
    Ok(())
}

/// Runs file system work off the async runtime threads.
async fn blocking<T, E>(
    work: impl FnOnce() -> Result<T, E> + Send + 'static,
) -> Result<T, DocumentError>
where
    T: Send + 'static,
    E: Into<DocumentError> + Send + 'static,
{
    tokio::task::spawn_blocking(work)
        .await
        .map_err(|error| DocumentError::Io(io::Error::other(error)))?
        .map_err(Into::into)
}

fn meta_from_row(row: DocumentRow) -> Result<DocumentMeta, DocumentError> {
    let invalid = |what: &str| DocumentError::InvalidRow(format!("{what} of {}", row.id));
    let id = Uuid::try_parse(&row.id).map_err(|_| invalid("id"))?;
    let format = DocumentFormat::parse(&row.format).ok_or_else(|| invalid("format"))?;
    let size_bytes = u64::try_from(row.size_bytes).map_err(|_| invalid("size"))?;
    Ok(DocumentMeta {
        id,
        title: row.title,
        format,
        size_bytes,
        content_sha256: row.content_sha256,
        created_at: row.created_at,
        updated_at: row.updated_at,
        archived_at: row.archived_at,
        deleted_at: row.deleted_at,
        original_name: row.original_name,
        favorited_at: row.favorited_at,
        last_opened_at: row.last_opened_at,
    })
}

#[cfg(test)]
mod link_tests {
    use super::*;

    const A: &str = "0b8f8f4e-6d0e-4f3e-9a3c-2a1d3c4b5e6f";
    const B: &str = "1f8a9a4e-6c39-4d6b-9b7e-6c0f5d2a1b3c";

    #[test]
    fn internal_links_are_found_in_markdown_and_pwrite() {
        let markdown = format!(
            "[A](pwrite://{A}) und <pwrite://{B}> und [A wieder](pwrite://{A}) \
             und [kaputt](pwrite://{}) und [groß](pwrite://{})",
            &A[..30],
            A.to_uppercase()
        );
        assert_eq!(links_in(DocumentFormat::Md, markdown.as_bytes()), [A, B]);
        assert!(links_in(DocumentFormat::Txt, markdown.as_bytes()).is_empty());

        let content = serde_json::json!({ "type": "doc", "content": [{ "type": "paragraph", "content": [
            { "type": "text", "text": "A", "marks": [{ "type": "link", "attrs": { "href": format!("pwrite://{A}") } }] },
            { "type": "text", "text": "Web", "marks": [{ "type": "link", "attrs": { "href": "https://example.org" } }] },
            { "type": "text", "text": format!("pwrite://{B}") }
        ] }] });
        let file = pwrite::new_file(B, "B", "2026-11-04T10:00:00Z", "de-DE", content);
        assert_eq!(
            links_in(DocumentFormat::Pwrite, &pwrite::to_bytes(&file)),
            [A]
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_names_encode_lower_case_uuids_only() {
        let id = Uuid::parse_str("1f8a9a4e-6c39-4d6b-9b7e-6c0f5d2a1b3c").unwrap();
        assert_eq!(
            DocumentFormat::Md.relative_path(id),
            "documents/markdown/1f8a9a4e-6c39-4d6b-9b7e-6c0f5d2a1b3c.md"
        );
        let name = "1f8a9a4e-6c39-4d6b-9b7e-6c0f5d2a1b3c.md";
        assert_eq!(DocumentFormat::Md.id_from_file_name(name), Some(id));
        for other in [
            "1F8A9A4E-6C39-4D6B-9B7E-6C0F5D2A1B3C.md",
            "1f8a9a4e6c394d6b9b7e6c0f5d2a1b3c.md",
            "1f8a9a4e-6c39-4d6b-9b7e-6c0f5d2a1b3c.txt",
            "1f8a9a4e-6c39-4d6b-9b7e-6c0f5d2a1b3c.md.bak",
            "{1f8a9a4e-6c39-4d6b-9b7e-6c0f5d2a1b3c}.md",
            "notes.md",
            ".md",
        ] {
            assert_eq!(DocumentFormat::Md.id_from_file_name(other), None, "{other}");
        }
    }

    #[test]
    fn titles_are_trimmed_and_checked() {
        assert_eq!(validate_title("  Projektplan  ").unwrap(), "Projektplan");
        assert_eq!(
            validate_title("Ä".repeat(300).as_str())
                .unwrap()
                .chars()
                .count(),
            300
        );
        for (title, reason) in [
            ("", "empty"),
            ("   ", "empty"),
            ("line\nbreak", "control character"),
            ("tab\tstop", "control character"),
        ] {
            assert!(
                matches!(validate_title(title), Err(DocumentError::InvalidTitle(r)) if r == reason),
                "{title:?}"
            );
        }
        assert!(validate_title(&"x".repeat(301)).is_err());
    }

    #[test]
    fn sha256_is_lower_case_hex() {
        assert_eq!(
            sha256_hex(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        assert_eq!(sha256_hex(b"").len(), 64);
    }
}
