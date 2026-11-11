//! Full library backups (`.pwbackup`) and restoring from them, stage 5
//! "Backup" (section 21; ADR 0021, decisions 7 to 9).
//!
//! Creating, in the four steps of section 21:
//!
//! 1. Under the write lock of the library (saves wait, drafts in
//!    `recovery/` go on): the database with `VACUUM INTO` into
//!    `tmp/backup-<id>/`, every document and medium the database names as a
//!    hard link next to it (files are only ever replaced, so a link is a
//!    fixed state), a copy where links are not possible.
//! 2. `PRAGMA integrity_check` of the copy.
//! 3. SHA-256 of every file, `manifest.json`.
//! 4. ZIP as `<name>.pwbackup.part` in the target, synced, read back and
//!    checked in full, then renamed to `<name>.pwbackup`.
//!
//! What goes in is a positive list taken from the database: `library.json`,
//! the database and the files of the rows in `documents` and `media`. Nothing
//! else of the folder (backups, `tmp/`, `recovery/`, thumbnails, exports, the
//! lock, `-wal`/`-shm`) can get in.

pub mod archive;

use std::fs;
use std::io::{self, BufWriter, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use serde::{Deserialize, Serialize};
use ts_rs::TS;
use uuid::Uuid;

use crate::clock::Generators;
use crate::database::{Database, diagnostics};
use crate::filesystem::atomic::{create_new_private, open_read_no_follow, sync_dir};
use crate::security::paths::{PathError, SafeRoot, create_private_dir};
use crate::services::documents::DocumentFormat;
use crate::services::library::layout::{DATABASE_FILE, MANIFEST_FILE, TMP_DIR};
use crate::services::media::{self, MediaKind, MediaType};

use archive::{Archive, ArchiveError, Discard};

pub const FORMAT: &str = "pwbackup";
pub const FORMAT_VERSION: u32 = 1;
pub const EXTENSION: &str = "pwbackup";
/// Suffix of a backup that is still being written.
pub const PART_SUFFIX: &str = ".part";
const MANIFEST_ENTRY: &str = "manifest.json";

/// Limits while reading a backup (counted, not taken from the archive).
const MAX_MANIFEST_BYTES: u64 = 64 * 1024 * 1024;
const MAX_LIBRARY_JSON_BYTES: u64 = 1024 * 1024;
const MAX_DATABASE_BYTES: u64 = 64 * 1024 * 1024 * 1024;

#[derive(Debug, thiserror::Error)]
pub enum BackupError {
    /// The archive or its manifest is damaged, changed or incomplete.
    #[error("backup refused: {0}")]
    Refused(String),
    /// Written by a newer version of the app.
    #[error("backup is newer: {0}")]
    Newer(String),
    #[error("the target is not empty")]
    TargetNotEmpty,
    #[error("the target lies inside the library")]
    TargetInsideLibrary,
    #[error("cancelled")]
    Cancelled,
    #[error("database error: {0}")]
    Database(String),
    #[error("path error: {0}")]
    Path(#[from] PathError),
    #[error("file error: {0}")]
    Io(#[from] io::Error),
}

impl From<ArchiveError> for BackupError {
    fn from(error: ArchiveError) -> Self {
        match error {
            ArchiveError::Io(error) => BackupError::Io(error),
            other => BackupError::Refused(other.to_string()),
        }
    }
}

fn refused(reason: impl Into<String>) -> BackupError {
    BackupError::Refused(reason.into())
}

/// One file of a backup.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BackupFile {
    pub path: String,
    pub size: u64,
    pub sha256: String,
}

/// `manifest.json` of a backup.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupManifest {
    pub format: String,
    pub format_version: u32,
    pub app_version: String,
    pub schema_version: i64,
    pub library_id: Uuid,
    pub library_format_version: u32,
    pub created_at: String,
    pub files: Vec<BackupFile>,
}

/// What a checked backup holds.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct BackupSummary {
    pub created_at: String,
    pub app_version: String,
    pub schema_version: i64,
    pub library_id: String,
    pub documents: u32,
    pub media: u32,
    #[ts(type = "number")]
    pub bytes: u64,
}

impl BackupManifest {
    fn summary(&self) -> BackupSummary {
        let count = |prefix: &str| {
            u32::try_from(
                self.files
                    .iter()
                    .filter(|file| file.path.starts_with(prefix))
                    .count(),
            )
            .unwrap_or(u32::MAX)
        };
        BackupSummary {
            created_at: self.created_at.clone(),
            app_version: self.app_version.clone(),
            schema_version: self.schema_version,
            library_id: self.library_id.hyphenated().to_string(),
            documents: count("documents/"),
            media: count("media/"),
            bytes: self.files.iter().map(|file| file.size).sum(),
        }
    }
}

/// Progress of creating, checking or restoring: files done of all.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct BackupProgress {
    pub step: BackupStep,
    pub done: u32,
    pub total: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum BackupStep {
    /// Database copy and links, under the write lock.
    Snapshot,
    /// Checksums and the archive.
    Writing,
    /// Reading the archive back.
    Checking,
    /// Unpacking (restore).
    Restoring,
}

/// Where a file of the library lives, by its entry name; `None` for a name
/// outside the positive list.
fn allowed_entry(name: &str) -> bool {
    name == MANIFEST_ENTRY || name == MANIFEST_FILE || name == DATABASE_FILE || element_entry(name)
}

fn element_entry(name: &str) -> bool {
    let id_and_extension = |rest: &str| -> Option<(Uuid, String)> {
        let (stem, extension) = rest.rsplit_once('.')?;
        let id = Uuid::try_parse(stem).ok()?;
        (id.hyphenated().to_string() == stem).then(|| (id, extension.to_owned()))
    };
    for format in DocumentFormat::ALL {
        if let Some(rest) = name.strip_prefix(&format!("{}/", format.directory()))
            && let Some((_, extension)) = id_and_extension(rest)
        {
            return extension == format.extension();
        }
    }
    for kind in [MediaKind::Image, MediaKind::Pdf, MediaKind::Attachment] {
        if let Some(rest) = name.strip_prefix(&format!("{}/", kind.directory()))
            && let Some((_, extension)) = id_and_extension(rest)
        {
            return MediaType::ALL.iter().any(|media_type| {
                media_type.kind() == kind && media_type.accepts_extension(&extension)
            });
        }
    }
    false
}

fn limit_of(name: &str) -> u64 {
    if name == MANIFEST_FILE {
        MAX_LIBRARY_JSON_BYTES
    } else if name == DATABASE_FILE {
        MAX_DATABASE_BYTES
    } else if name.starts_with("documents/") {
        crate::services::documents::MAX_DOCUMENT_BYTES
    } else {
        media::MAX_FILE_BYTES
    }
}

/// File name of a backup: `projectwrite-<first 8 of the library id>-<time>`.
pub fn backup_name(library_id: Uuid, generators: &Generators) -> String {
    let id = library_id.simple().to_string();
    format!(
        "projectwrite-{}-{}.{EXTENSION}",
        &id[..8],
        generators.now().to_compact()
    )
}

fn is_own_backup(name: &str, library_id: Uuid) -> bool {
    let id = library_id.simple().to_string();
    name.starts_with(&format!("projectwrite-{}-", &id[..8]))
        && name.ends_with(&format!(".{EXTENSION}"))
}

/// Backups of this library in `dir`, oldest first (the name ends with a
/// fixed-width UTC time).
pub fn own_backups(dir: &Path, library_id: Uuid) -> io::Result<Vec<PathBuf>> {
    let mut found: Vec<PathBuf> = match fs::read_dir(dir) {
        Ok(entries) => entries
            .filter_map(Result::ok)
            .filter(|entry| entry.file_type().is_ok_and(|kind| kind.is_file()))
            .map(|entry| entry.path())
            .filter(|path| {
                path.file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| is_own_backup(name, library_id))
            })
            .collect(),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Vec::new(),
        Err(error) => return Err(error),
    };
    found.sort();
    Ok(found)
}

/// Removes all but the newest `keep` backups of this library in `dir`;
/// returns how many went.
pub fn prune(dir: &Path, library_id: Uuid, keep: usize) -> io::Result<usize> {
    let backups = own_backups(dir, library_id)?;
    let excess = backups.len().saturating_sub(keep);
    for path in backups.iter().take(excess) {
        fs::remove_file(path)?;
    }
    Ok(excess)
}

/// Removes `.part` files of this library that an interrupted backup left in
/// `dir`.
pub fn remove_partial(dir: &Path, library_id: Uuid) -> io::Result<usize> {
    let id = library_id.simple().to_string();
    let prefix = format!("projectwrite-{}-", &id[..8]);
    let suffix = format!(".{EXTENSION}{PART_SUFFIX}");
    let mut removed = 0;
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.filter_map(Result::ok) {
            let name = entry.file_name();
            let Some(name) = name.to_str() else { continue };
            if name.starts_with(&prefix)
                && name.ends_with(&suffix)
                && entry.file_type().is_ok_and(|kind| kind.is_file())
            {
                fs::remove_file(entry.path())?;
                removed += 1;
            }
        }
    }
    Ok(removed)
}

/// The backups of one open library.
#[derive(Clone)]
pub struct Backups {
    root: SafeRoot,
    database: Database,
    generators: Generators,
    write_lock: Arc<tokio::sync::Mutex<()>>,
    library_id: Uuid,
    library_format_version: u32,
}

/// Result of creating a backup.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BackupOutcome {
    pub path: PathBuf,
    pub summary: BackupSummary,
}

struct Staging {
    dir: PathBuf,
}

impl Drop for Staging {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.dir);
    }
}

impl Backups {
    pub(crate) fn new(
        root: SafeRoot,
        database: Database,
        generators: Generators,
        write_lock: Arc<tokio::sync::Mutex<()>>,
        library_id: Uuid,
        library_format_version: u32,
    ) -> Self {
        Self {
            root,
            database,
            generators,
            write_lock,
            library_id,
            library_format_version,
        }
    }

    pub fn library_id(&self) -> Uuid {
        self.library_id
    }

    /// Refuses a target inside the library, except its `backups/` folder.
    pub fn check_target(&self, target: &Path) -> Result<PathBuf, BackupError> {
        let target = crate::security::paths::canonical_dir(target)?;
        let root = self.root.path();
        if target.starts_with(root)
            && !target.starts_with(root.join(crate::services::library::layout::BACKUPS_DIR))
        {
            return Err(BackupError::TargetInsideLibrary);
        }
        Ok(target)
    }

    /// Creates a backup in `target` (a folder). `cancel` stops it between
    /// files; nothing of it stays behind then.
    pub async fn create(
        &self,
        target: &Path,
        cancel: Arc<AtomicBool>,
        progress: impl Fn(BackupProgress) + Send + Sync + 'static,
    ) -> Result<BackupOutcome, BackupError> {
        let target = self.check_target(target)?;
        let progress = Arc::new(progress);
        self.root.create_dir_all(TMP_DIR)?;
        let staging = Staging {
            dir: self.root.resolve(&format!(
                "{TMP_DIR}/backup-{}",
                self.generators.new_id().hyphenated()
            ))?,
        };
        create_private_dir(&staging.dir)?;

        // Step 1: a fixed state under the write lock.
        let files = {
            let _guard = self.write_lock.lock().await;
            let files = self.library_files().await?;
            progress(BackupProgress {
                step: BackupStep::Snapshot,
                done: 0,
                total: u32::try_from(files.len()).unwrap_or(u32::MAX),
            });
            let database_copy = staging.dir.join("projectwrite.db");
            self.database
                .vacuum_into(&database_copy)
                .await
                .map_err(|error| BackupError::Database(error.to_string()))?;
            let root = self.root.clone();
            let dir = staging.dir.clone();
            let list = files.clone();
            tokio::task::spawn_blocking(move || link_all(&root, &dir, &list))
                .await
                .map_err(|error| BackupError::Io(io::Error::other(error)))??;
            files
        };
        if cancel.load(Ordering::Relaxed) {
            return Err(BackupError::Cancelled);
        }

        // Step 2: the copy of the database is whole.
        let database_copy = staging.dir.join("projectwrite.db");
        match diagnostics::integrity_check_file(&database_copy).await {
            Ok(true) => {}
            Ok(false) => {
                return Err(BackupError::Database(
                    "integrity check of the copy failed".into(),
                ));
            }
            Err(error) => return Err(BackupError::Database(error.to_string())),
        }
        let schema_version = self
            .database
            .schema_version()
            .await
            .map_err(|error| BackupError::Database(error.to_string()))?;

        // Steps 3 and 4: checksums, manifest, archive, check, rename.
        let name = backup_name(self.library_id, &self.generators);
        let part = target.join(format!("{name}{PART_SUFFIX}"));
        let finished = target.join(&name);
        let manifest_base = BackupManifest {
            format: FORMAT.to_owned(),
            format_version: FORMAT_VERSION,
            app_version: env!("CARGO_PKG_VERSION").to_owned(),
            schema_version,
            library_id: self.library_id,
            library_format_version: self.library_format_version,
            created_at: self.generators.now().to_iso(),
            files: Vec::new(),
        };
        let dir = staging.dir.clone();
        let writer_cancel = cancel.clone();
        let writer_progress = progress.clone();
        let part_path = part.clone();
        let write_result = tokio::task::spawn_blocking(move || {
            write_archive(
                &dir,
                &files,
                manifest_base,
                &part_path,
                &writer_cancel,
                &*writer_progress,
            )
        })
        .await
        .map_err(|error| BackupError::Io(io::Error::other(error)));
        let manifest = match write_result {
            Ok(Ok(manifest)) => manifest,
            Ok(Err(error)) | Err(error) => {
                let _ = fs::remove_file(&part);
                return Err(error);
            }
        };
        let check_dir = staging.dir.join("check");
        create_private_dir(&check_dir)?;
        let checked = check(&part, &check_dir, Some(cancel.clone()), progress.clone()).await;
        match checked {
            Ok(read_back) if read_back == manifest => {}
            Ok(_) => {
                let _ = fs::remove_file(&part);
                return Err(refused(
                    "the archive read back differs from what was written",
                ));
            }
            Err(error) => {
                let _ = fs::remove_file(&part);
                return Err(error);
            }
        }
        fs::rename(&part, &finished)?;
        sync_dir(&target)?;
        log::info!(
            "backup {} written ({} files)",
            finished.display(),
            manifest.files.len()
        );
        Ok(BackupOutcome {
            path: finished,
            summary: manifest.summary(),
        })
    }

    /// The files of the library that a backup holds, from the database.
    async fn library_files(&self) -> Result<Vec<String>, BackupError> {
        let pool = self.database.writer();
        let database = |error: sqlx::Error| BackupError::Database(error.to_string());
        let mut files = vec![MANIFEST_FILE.to_owned()];
        for (id, format) in crate::database::documents::all_file_keys(pool)
            .await
            .map_err(database)?
        {
            let format =
                DocumentFormat::parse(&format).ok_or_else(|| refused("unknown document format"))?;
            let id = Uuid::try_parse(&id).map_err(|_| refused("document id"))?;
            files.push(format.relative_path(id));
        }
        for (id, kind, extension) in crate::database::media::all_file_keys(pool)
            .await
            .map_err(database)?
        {
            let kind = MediaKind::parse(&kind).ok_or_else(|| refused("unknown media kind"))?;
            files.push(media::relative_path(kind, &id, &extension));
        }
        files.sort();
        Ok(files)
    }
}

/// Links (or copies) every file into `dir` under its relative path.
fn link_all(root: &SafeRoot, dir: &Path, files: &[String]) -> Result<(), BackupError> {
    for relative in files {
        let source = root.resolve(relative)?;
        let target = dir.join("files").join(relative);
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent)?;
        }
        if source.symlink_metadata()?.file_type().is_symlink() {
            return Err(refused(format!("{relative} is a link")));
        }
        if fs::hard_link(&source, &target).is_err() {
            fs::copy(&source, &target)?;
        }
    }
    Ok(())
}

fn sha_and_size(path: &Path) -> io::Result<(u64, String)> {
    let mut file = open_read_no_follow(path)?;
    let mut hasher = sha2::Sha256::new();
    let size = io::copy(&mut file, &mut HashWriter(&mut hasher))?;
    use sha2::Digest as _;
    Ok((size, archive::hex(&hasher.finalize())))
}

struct HashWriter<'a>(&'a mut sha2::Sha256);

impl Write for HashWriter<'_> {
    fn write(&mut self, data: &[u8]) -> io::Result<usize> {
        use sha2::Digest as _;
        self.0.update(data);
        Ok(data.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

/// Writes the archive to `part`: manifest first, then the database and the
/// files. Media are stored, everything else is deflated.
fn write_archive(
    dir: &Path,
    files: &[String],
    mut manifest: BackupManifest,
    part: &Path,
    cancel: &AtomicBool,
    progress: &(dyn Fn(BackupProgress) + Send + Sync),
) -> Result<BackupManifest, BackupError> {
    let source = |entry: &str| {
        if entry == DATABASE_FILE {
            dir.join("projectwrite.db")
        } else {
            dir.join("files").join(entry)
        }
    };
    let mut entries: Vec<String> = Vec::with_capacity(files.len() + 1);
    entries.push(DATABASE_FILE.to_owned());
    entries.extend(files.iter().cloned());
    let total = u32::try_from(entries.len()).unwrap_or(u32::MAX);
    for (index, entry) in entries.iter().enumerate() {
        if cancel.load(Ordering::Relaxed) {
            return Err(BackupError::Cancelled);
        }
        let (size, sha256) = sha_and_size(&source(entry))?;
        manifest.files.push(BackupFile {
            path: entry.clone(),
            size,
            sha256,
        });
        if index % 64 == 0 {
            progress(BackupProgress {
                step: BackupStep::Writing,
                done: u32::try_from(index).unwrap_or(u32::MAX),
                total: total * 2,
            });
        }
    }
    let file = create_new_private(part)?;
    let mut zip = zip::ZipWriter::new(BufWriter::new(file));
    let deflated = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated)
        .unix_permissions(0o600);
    let stored = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Stored)
        .unix_permissions(0o600);
    let manifest_json = serde_json::to_vec_pretty(&manifest).map_err(io::Error::other)?;
    zip.start_file(MANIFEST_ENTRY, deflated)
        .map_err(|error| BackupError::Io(io::Error::other(error)))?;
    zip.write_all(&manifest_json)?;
    for (index, file) in manifest.files.iter().enumerate() {
        if cancel.load(Ordering::Relaxed) {
            return Err(BackupError::Cancelled);
        }
        let options = if file.path.starts_with("media/") {
            stored
        } else {
            deflated
        };
        let options = options.large_file(file.size >= u64::from(u32::MAX));
        zip.start_file(file.path.as_str(), options)
            .map_err(|error| BackupError::Io(io::Error::other(error)))?;
        let mut reader = open_read_no_follow(&source(&file.path))?;
        io::copy(&mut reader, &mut zip)?;
        if index % 64 == 0 {
            progress(BackupProgress {
                step: BackupStep::Writing,
                done: total + u32::try_from(index).unwrap_or(u32::MAX),
                total: total * 2,
            });
        }
    }
    let writer = zip
        .finish()
        .map_err(|error| BackupError::Io(io::Error::other(error)))?;
    let file = writer.into_inner().map_err(|error| error.into_error())?;
    file.sync_all()?;
    Ok(manifest)
}

/// Opens a backup, checks its entries against the positive list and reads
/// its manifest.
fn open_backup(path: &Path) -> Result<(Archive, BackupManifest), BackupError> {
    let file = open_read_no_follow(path)?;
    let mut archive = Archive::open(file, allowed_entry)?;
    if !archive.names().iter().any(|name| name == MANIFEST_ENTRY) {
        return Err(refused("no manifest"));
    }
    let raw = archive.read(MANIFEST_ENTRY, MAX_MANIFEST_BYTES)?;
    let manifest: BackupManifest =
        serde_json::from_slice(&raw).map_err(|_| refused("manifest is not valid"))?;
    if manifest.format != FORMAT {
        return Err(refused("not a backup of Project Write"));
    }
    if manifest.format_version > FORMAT_VERSION {
        return Err(BackupError::Newer(format!(
            "format version {}",
            manifest.format_version
        )));
    }
    if manifest.schema_version > Database::supported_version() {
        return Err(BackupError::Newer(format!(
            "schema version {}",
            manifest.schema_version
        )));
    }
    // Every entry is in the manifest, every file of the manifest is in the
    // archive, each once, and the library and the database are there.
    let listed: std::collections::BTreeSet<&str> = manifest
        .files
        .iter()
        .map(|file| file.path.as_str())
        .collect();
    if listed.len() != manifest.files.len() {
        return Err(refused("manifest names a file twice"));
    }
    for file in &manifest.files {
        if !allowed_entry(&file.path) || file.path == MANIFEST_ENTRY {
            return Err(refused(format!("manifest names {}", file.path)));
        }
        if file.size > limit_of(&file.path) {
            return Err(refused(format!("{} is too large", file.path)));
        }
        if file.sha256.len() != 64
            || !file
                .sha256
                .bytes()
                .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
        {
            return Err(refused(format!("checksum of {}", file.path)));
        }
    }
    for name in archive.names() {
        if name != MANIFEST_ENTRY && !listed.contains(name.as_str()) {
            return Err(refused(format!("{name} is not in the manifest")));
        }
    }
    for required in [MANIFEST_FILE, DATABASE_FILE] {
        if !listed.contains(required) {
            return Err(refused(format!("{required} is missing")));
        }
    }
    if archive.names().len() != manifest.files.len() + 1 {
        return Err(refused("files of the manifest are missing"));
    }
    Ok((archive, manifest))
}

/// "Backup prüfen": reads every file, compares sizes and checksums and checks
/// the database copy in `scratch` (removed afterwards). Changes nothing.
pub async fn check(
    path: &Path,
    scratch: &Path,
    cancel: Option<Arc<AtomicBool>>,
    progress: Arc<dyn Fn(BackupProgress) + Send + Sync>,
) -> Result<BackupManifest, BackupError> {
    let path = path.to_owned();
    let database_copy = scratch.join(format!("check-{}.db", Uuid::new_v4().simple()));
    let copy = database_copy.clone();
    let result = tokio::task::spawn_blocking(move || -> Result<BackupManifest, BackupError> {
        let (mut archive, manifest) = open_backup(&path)?;
        let total = u32::try_from(manifest.files.len()).unwrap_or(u32::MAX);
        for (index, file) in manifest.files.iter().enumerate() {
            if cancel
                .as_ref()
                .is_some_and(|flag| flag.load(Ordering::Relaxed))
            {
                return Err(BackupError::Cancelled);
            }
            let (size, sha256) = if file.path == DATABASE_FILE {
                let mut out = BufWriter::new(create_new_private(&copy)?);
                let read = archive.copy(&file.path, file.size, &mut out)?;
                out.flush()?;
                read
            } else {
                archive.copy(&file.path, file.size, &mut Discard)?
            };
            if size != file.size || sha256 != file.sha256 {
                return Err(refused(format!("{} differs from the manifest", file.path)));
            }
            if index % 64 == 0 {
                progress(BackupProgress {
                    step: BackupStep::Checking,
                    done: u32::try_from(index).unwrap_or(u32::MAX),
                    total,
                });
            }
        }
        Ok(manifest)
    })
    .await
    .map_err(|error| BackupError::Io(io::Error::other(error)))
    .and_then(|result| result);
    let result = match result {
        Ok(manifest) => match diagnostics::integrity_check_file(&database_copy).await {
            Ok(true) => Ok(manifest),
            Ok(false) => Err(refused("the database in the backup is damaged")),
            Err(_) => Err(refused("the database in the backup is damaged")),
        },
        Err(error) => Err(error),
    };
    let _ = fs::remove_file(&database_copy);
    result
}

/// Summary of a checked backup.
pub async fn check_summary(
    path: &Path,
    scratch: &Path,
    progress: Arc<dyn Fn(BackupProgress) + Send + Sync>,
) -> Result<BackupSummary, BackupError> {
    check(path, scratch, None, progress)
        .await
        .map(|manifest| manifest.summary())
}

/// Stage 5: restores the backup at `path` into the new folder `target`
/// (missing or empty). Unpacks next to it under a temporary name, checks
/// every file and the database, then renames. Returns the summary.
pub async fn restore(
    path: &Path,
    target: &Path,
    progress: Arc<dyn Fn(BackupProgress) + Send + Sync>,
) -> Result<BackupSummary, BackupError> {
    let parent = target
        .parent()
        .ok_or(BackupError::TargetNotEmpty)?
        .to_owned();
    let name = target
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or(BackupError::TargetNotEmpty)?
        .to_owned();
    match fs::read_dir(target) {
        Ok(mut entries) => {
            if entries.next().is_some() {
                return Err(BackupError::TargetNotEmpty);
            }
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => return Err(error.into()),
    }
    let staging = Staging {
        dir: parent.join(format!(".{name}.restoring-{}", Uuid::new_v4().simple())),
    };
    create_private_dir(&staging.dir)?;
    let path = path.to_owned();
    let dir = staging.dir.clone();
    let manifest = tokio::task::spawn_blocking(move || -> Result<BackupManifest, BackupError> {
        let (mut archive, manifest) = open_backup(&path)?;
        let total = u32::try_from(manifest.files.len()).unwrap_or(u32::MAX);
        for (index, file) in manifest.files.iter().enumerate() {
            let out_path = dir.join(&file.path);
            if let Some(parent) = out_path.parent() {
                fs::create_dir_all(parent)?;
            }
            let mut out = BufWriter::new(create_new_private(&out_path)?);
            let (size, sha256) = archive.copy(&file.path, file.size, &mut out)?;
            let out = out.into_inner().map_err(|error| error.into_error())?;
            out.sync_all()?;
            if size != file.size || sha256 != file.sha256 {
                return Err(refused(format!("{} differs from the manifest", file.path)));
            }
            if index % 64 == 0 {
                progress(BackupProgress {
                    step: BackupStep::Restoring,
                    done: u32::try_from(index).unwrap_or(u32::MAX),
                    total,
                });
            }
        }
        // Read everything back once more from the disk.
        for file in &manifest.files {
            let (size, sha256) = sha_and_size(&dir.join(&file.path))?;
            if size != file.size || sha256 != file.sha256 {
                return Err(refused(format!("{} differs after writing", file.path)));
            }
        }
        Ok(manifest)
    })
    .await
    .map_err(|error| BackupError::Io(io::Error::other(error)))??;
    match diagnostics::integrity_check_file(&staging.dir.join(DATABASE_FILE)).await {
        Ok(true) => {}
        _ => return Err(refused("the database in the backup is damaged")),
    }
    if target.exists() {
        fs::remove_dir(target)?;
    }
    fs::rename(&staging.dir, target)?;
    sync_dir(&parent)?;
    log::info!("backup restored into {} (stage 5)", target.display());
    Ok(manifest.summary())
}

#[cfg(test)]
mod tests {
    use super::*;

    const ID: &str = "0b0e5a3e-0c5f-4a43-9f39-3a3b9a9d2c11";

    #[test]
    fn only_the_files_of_a_library_are_allowed() {
        for good in [
            "manifest.json".to_owned(),
            "library.json".to_owned(),
            "database/projectwrite.db".to_owned(),
            format!("documents/markdown/{ID}.md"),
            format!("documents/text/{ID}.txt"),
            format!("documents/richtext/{ID}.pwrite"),
            format!("media/images/{ID}.png"),
            format!("media/pdf/{ID}.pdf"),
            format!("media/attachments/{ID}.docx"),
        ] {
            assert!(allowed_entry(&good), "{good}");
        }
        for bad in [
            format!("documents/markdown/{ID}.txt"),
            format!("media/images/{ID}.pdf"),
            "backups/projectwrite-0b0e5a3e-20261111T100000000Z.pwbackup".to_owned(),
            "database/projectwrite.db-wal".to_owned(),
            "recovery/x.json".to_owned(),
            "thumbnails/images/x.png".to_owned(),
            "tmp/x".to_owned(),
            "library.lock".to_owned(),
            format!("documents/markdown/{}.md", ID.to_uppercase()),
            "documents/markdown/kein-id.md".to_owned(),
        ] {
            assert!(!allowed_entry(&bad), "{bad}");
        }
    }

    #[test]
    fn backup_names_carry_the_library_and_sort_by_time() {
        let id = Uuid::try_parse(ID).unwrap();
        assert!(is_own_backup(
            "projectwrite-0b0e5a3e-20261111T100000000Z.pwbackup",
            id
        ));
        assert!(!is_own_backup(
            "projectwrite-11111111-20261111T100000000Z.pwbackup",
            id
        ));
        assert!(!is_own_backup(
            "projectwrite-0b0e5a3e-20261111T100000000Z.pwbackup.part",
            id
        ));
    }
}
