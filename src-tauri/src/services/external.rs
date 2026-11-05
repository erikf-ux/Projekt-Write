//! External files (section 9, "Externe Dateien"; ADR 0015, decisions 3–10).
//!
//! A file outside the library is chosen in a system dialog, passed at start
//! or taken from "Zuletzt verwendet". Rust keeps its path under a random id;
//! the frontend sees the id, the file name and the folder as display text.
//!
//! * Reading detects the encoding and the line endings
//!   ([`super::encoding`]); saving writes both back unchanged.
//! * A text equal to the loaded one is never written.
//! * Size, modification time and SHA-256 of the file are kept after each
//!   read and write. [`ExternalFiles::check`] compares them; a save refuses
//!   to overwrite a file changed from outside unless the user said so.
//! * Saving replaces the file atomically in its own folder and keeps its
//!   permissions.

use std::collections::HashMap;
use std::fs::{self, OpenOptions};
use std::io;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::SystemTime;

use serde::Serialize;
use serde_json::Value;
use sha2::{Digest, Sha256};
use ts_rs::TS;

use crate::clock::Generators;
use crate::filesystem::atomic;
use crate::security::paths::display_path;
use crate::services::documents::{DocumentFormat, MAX_DOCUMENT_BYTES};
use crate::services::encoding::{self, EncodingError, LineEnding, TextEncoding};
use crate::services::import;
use crate::services::pwrite::{self, PwriteFile};
use crate::services::settings::app::{AppSettingsStore, MAX_RECENT_FILES, RecentFile};

/// From this size the frontend asks before opening (section 9).
pub const LARGE_FILE_BYTES: u64 = 10 * 1024 * 1024;
/// Larger files are not opened; the limit only protects memory.
pub const MAX_FILE_BYTES: u64 = MAX_DOCUMENT_BYTES;
/// Most files open at the same time.
const MAX_OPEN_FILES: usize = 512;

#[derive(Debug, thiserror::Error)]
pub enum ExternalError {
    #[error("no open file with this id")]
    Unknown,
    #[error("the file no longer exists")]
    Missing,
    #[error("file type not supported")]
    UnsupportedType,
    #[error("file larger than {MAX_FILE_BYTES} bytes")]
    TooLarge,
    #[error("not a text file")]
    NotText,
    #[error("not a valid .pwrite file")]
    InvalidPwrite,
    /// A `.pwrite` file of a newer format version.
    #[error("the file belongs to a newer version and is not written")]
    NewerFormat,
    #[error("the file was changed outside the app")]
    Changed,
    #[error("the file cannot be written")]
    ReadOnly,
    #[error("character U+{0:04X} cannot be written in this encoding")]
    Unencodable(u32),
    #[error("too many open files")]
    TooManyOpen,
    #[error("file system: {0}")]
    Io(#[source] io::Error),
}

impl From<io::Error> for ExternalError {
    fn from(error: io::Error) -> Self {
        match error.kind() {
            io::ErrorKind::NotFound => ExternalError::Missing,
            io::ErrorKind::PermissionDenied | io::ErrorKind::ReadOnlyFilesystem => {
                ExternalError::ReadOnly
            }
            io::ErrorKind::FileTooLarge => ExternalError::TooLarge,
            _ => ExternalError::Io(error),
        }
    }
}

/// An external file as the frontend knows it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ExternalFileInfo {
    /// Id of the file in this session (32 hex digits).
    pub id: String,
    /// File name with extension.
    pub name: String,
    /// The folder, for display only.
    pub folder: String,
    pub format: DocumentFormat,
    #[ts(type = "number")]
    pub size: u64,
    /// At least [`LARGE_FILE_BYTES`]: ask before reading.
    pub large: bool,
}

/// The content of an external file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ExternalText {
    /// Text with LF line endings; for `.pwrite` the editor content as JSON.
    pub text: String,
    pub encoding: TextEncoding,
    pub line_ending: LineEnding,
    /// The file mixes line endings; saving makes them uniform.
    pub mixed_line_endings: bool,
    /// The file cannot be written (permissions or a newer `.pwrite`).
    pub read_only: bool,
    /// A `.pwrite` file of a newer format version.
    pub newer_format: bool,
}

/// How the file on disk relates to what was loaded or saved last.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum FileStatus {
    Unchanged,
    /// Changed outside the app.
    Changed,
    /// Deleted, moved or renamed outside the app.
    Missing,
}

/// Result of `external_check`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct FileCheck {
    pub status: FileStatus,
    pub read_only: bool,
}

/// Result of a save.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum SaveOutcome {
    Saved,
    /// Nothing differed from the file; nothing was written.
    Unchanged,
}

/// An entry of "Zuletzt verwendet" for external files.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct RecentFileEntry {
    /// Stable id of the entry (derived from the path, 32 hex digits).
    pub id: String,
    pub name: String,
    pub folder: String,
    pub format: DocumentFormat,
    pub opened_at: String,
    /// The file is not there (any more).
    pub missing: bool,
}

/// What a save writes.
#[derive(Debug, Clone)]
pub struct SaveRequest {
    pub text: String,
    pub encoding: TextEncoding,
    pub line_ending: LineEnding,
    /// Write even if the file changed outside the app.
    pub overwrite: bool,
}

/// Size, modification time and content hash of a file.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Stamp {
    len: u64,
    modified: Option<SystemTime>,
    hash: [u8; 32],
}

/// What was read or written last.
#[derive(Debug, Clone)]
struct Loaded {
    stamp: Stamp,
    encoding: TextEncoding,
    line_ending: LineEnding,
    /// SHA-256 of the text the frontend holds. `None` after the user kept
    /// their version against a change from outside: the next save writes.
    text: Option<[u8; 32]>,
    /// `.pwrite`: the envelope (unknown fields included).
    envelope: Option<Box<PwriteFile>>,
    newer_format: bool,
}

#[derive(Debug, Clone)]
struct OpenFile {
    path: PathBuf,
    format: DocumentFormat,
    loaded: Option<Loaded>,
}

/// The external files of this session.
pub struct ExternalFiles {
    open: Mutex<HashMap<String, OpenFile>>,
    settings: Arc<Mutex<AppSettingsStore>>,
    generators: Generators,
}

impl ExternalFiles {
    pub fn new(settings: Arc<Mutex<AppSettingsStore>>, generators: Generators) -> Self {
        Self {
            open: Mutex::new(HashMap::new()),
            settings,
            generators,
        }
    }

    /// Takes a file into the session (dialog, start argument, recent list)
    /// and remembers it in "Zuletzt verwendet". The same file keeps its id.
    pub fn register(&self, path: &Path) -> Result<ExternalFileInfo, ExternalError> {
        // The real file, so saving replaces it and not a link to it.
        let path = fs::canonicalize(path)?;
        let format = format_of_path(&path)?;
        let metadata = fs::metadata(&path)?;
        if !metadata.is_file() {
            return Err(ExternalError::UnsupportedType);
        }
        if metadata.len() > MAX_FILE_BYTES {
            return Err(ExternalError::TooLarge);
        }
        let id = {
            let mut open = lock(&self.open);
            let existing = open
                .iter()
                .find(|(_, file)| file.path == path)
                .map(|(id, _)| id.clone());
            match existing {
                Some(id) => id,
                None => {
                    if open.len() >= MAX_OPEN_FILES {
                        return Err(ExternalError::TooManyOpen);
                    }
                    let id = uuid::Uuid::new_v4().simple().to_string();
                    open.insert(
                        id.clone(),
                        OpenFile {
                            path: path.clone(),
                            format,
                            loaded: None,
                        },
                    );
                    id
                }
            }
        };
        self.remember_recent(&path);
        Ok(info(id, &path, format, metadata.len()))
    }

    /// Whether a file is open in this session (its tab exists).
    pub fn is_open(&self, path: &Path) -> bool {
        let path = fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
        lock(&self.open).values().any(|file| file.path == path)
    }

    /// The file behind an id.
    pub fn path(&self, id: &str) -> Result<PathBuf, ExternalError> {
        lock(&self.open)
            .get(id)
            .map(|file| file.path.clone())
            .ok_or(ExternalError::Unknown)
    }

    /// Reads the file and makes its current state the base for checks and
    /// saves.
    pub fn read(&self, id: &str) -> Result<ExternalText, ExternalError> {
        let (path, format) = {
            let open = lock(&self.open);
            let file = open.get(id).ok_or(ExternalError::Unknown)?;
            (file.path.clone(), file.format)
        };
        let (data, stamp) = read_with_stamp(&path)?;
        let (text, loaded) = if format == DocumentFormat::Pwrite {
            let (text, envelope, newer_format) = match pwrite::parse(&data) {
                Ok(pwrite::Parsed::Current(file)) => (to_json(&file.content), Some(file), false),
                Ok(pwrite::Parsed::Newer { content, .. }) => (to_json(&content), None, true),
                Err(_) => return Err(ExternalError::InvalidPwrite),
            };
            let loaded = Loaded {
                stamp,
                encoding: TextEncoding::Utf8,
                line_ending: LineEnding::Lf,
                text: Some(sha256(text.as_bytes())),
                envelope,
                newer_format,
            };
            (text, loaded)
        } else {
            let decoded = encoding::decode(&data).map_err(|_| ExternalError::NotText)?;
            let loaded = Loaded {
                stamp,
                encoding: decoded.encoding,
                line_ending: decoded.line_ending,
                text: Some(sha256(decoded.text.as_bytes())),
                envelope: None,
                newer_format: false,
            };
            let mixed = decoded.mixed_line_endings;
            return self.store_loaded(id, loaded, |loaded| ExternalText {
                text: decoded.text,
                encoding: loaded.encoding,
                line_ending: loaded.line_ending,
                mixed_line_endings: mixed,
                read_only: !writable(&path),
                newer_format: false,
            });
        };
        let newer_format = loaded.newer_format;
        self.store_loaded(id, loaded, |loaded| ExternalText {
            text,
            encoding: loaded.encoding,
            line_ending: loaded.line_ending,
            mixed_line_endings: false,
            read_only: newer_format || !writable(&path),
            newer_format,
        })
    }

    fn store_loaded<T>(
        &self,
        id: &str,
        loaded: Loaded,
        result: impl FnOnce(&Loaded) -> T,
    ) -> Result<T, ExternalError> {
        let mut open = lock(&self.open);
        let file = open.get_mut(id).ok_or(ExternalError::Unknown)?;
        let value = result(&loaded);
        file.loaded = Some(loaded);
        Ok(value)
    }

    /// Compares the file with what was read or saved last. A file that was
    /// only touched (same content) counts as unchanged.
    pub fn check(&self, id: &str) -> Result<FileCheck, ExternalError> {
        let (path, stamp) = self.loaded_stamp(id)?;
        let status = match disk_status(&path, &stamp)? {
            DiskStatus::Same => FileStatus::Unchanged,
            DiskStatus::Touched(current) => {
                self.update_stamp(id, &stamp, current);
                FileStatus::Unchanged
            }
            DiskStatus::Changed(_) => FileStatus::Changed,
            DiskStatus::Missing => FileStatus::Missing,
        };
        let read_only = match status {
            FileStatus::Missing => false,
            _ => self.is_newer(id) || !writable(&path),
        };
        Ok(FileCheck { status, read_only })
    }

    /// "Meine Fassung behalten": the file as it is now becomes the base, and
    /// the next save writes the user's text over it.
    pub fn keep_mine(&self, id: &str) -> Result<(), ExternalError> {
        let (path, stamp) = self.loaded_stamp(id)?;
        let current = match disk_status(&path, &stamp)? {
            DiskStatus::Same => stamp.clone(),
            DiskStatus::Touched(current) | DiskStatus::Changed(current) => current,
            DiskStatus::Missing => return Err(ExternalError::Missing),
        };
        let mut open = lock(&self.open);
        let loaded = open
            .get_mut(id)
            .and_then(|file| file.loaded.as_mut())
            .ok_or(ExternalError::Unknown)?;
        loaded.stamp = current;
        loaded.text = None;
        Ok(())
    }

    /// Saves the text into the file, in the given encoding and line ending.
    pub fn save(&self, id: &str, request: &SaveRequest) -> Result<SaveOutcome, ExternalError> {
        let (path, format, loaded) = {
            let open = lock(&self.open);
            let file = open.get(id).ok_or(ExternalError::Unknown)?;
            let loaded = file.loaded.clone().ok_or(ExternalError::Unknown)?;
            (file.path.clone(), file.format, loaded)
        };
        if loaded.newer_format {
            return Err(ExternalError::NewerFormat);
        }
        let text = normalise(&request.text);
        let encoding = effective_encoding(format, request.encoding);
        let line_ending = effective_line_ending(format, request.line_ending);
        if loaded.text == Some(sha256(text.as_bytes()))
            && encoding == loaded.encoding
            && line_ending == loaded.line_ending
        {
            return Ok(SaveOutcome::Unchanged);
        }
        let (data, envelope) = self.encode(format, &text, encoding, line_ending, &loaded)?;
        match disk_status(&path, &loaded.stamp)? {
            DiskStatus::Missing => return Err(ExternalError::Missing),
            DiskStatus::Changed(_) if !request.overwrite => return Err(ExternalError::Changed),
            DiskStatus::Same | DiskStatus::Touched(_) | DiskStatus::Changed(_) => {}
        }
        if !writable(&path) {
            return Err(ExternalError::ReadOnly);
        }
        atomic::write_atomic_keeping_permissions(&path, &data)?;
        let stamp = stamp_after_write(&path, &data)?;
        let mut open = lock(&self.open);
        let file = open.get_mut(id).ok_or(ExternalError::Unknown)?;
        file.loaded = Some(Loaded {
            stamp,
            encoding,
            line_ending,
            text: Some(sha256(text.as_bytes())),
            envelope: envelope.or(loaded.envelope),
            newer_format: false,
        });
        Ok(SaveOutcome::Saved)
    }

    /// "Speichern unter": writes the text to `target` (chosen in the save
    /// dialog); the id then stands for the new file. A missing or foreign
    /// extension gets the one of the format. A `.pwrite` copy gets a new
    /// envelope id.
    pub fn save_as(
        &self,
        id: &str,
        target: &Path,
        request: &SaveRequest,
    ) -> Result<ExternalFileInfo, ExternalError> {
        let (format, loaded) = {
            let open = lock(&self.open);
            let file = open.get(id).ok_or(ExternalError::Unknown)?;
            (file.format, file.loaded.clone())
        };
        let loaded = loaded.ok_or(ExternalError::Unknown)?;
        if loaded.newer_format {
            return Err(ExternalError::NewerFormat);
        }
        let target = with_extension_of(target, format);
        let text = normalise(&request.text);
        let encoding = effective_encoding(format, request.encoding);
        let line_ending = effective_line_ending(format, request.line_ending);
        let mut copy = loaded.clone();
        if let Some(envelope) = copy.envelope.as_mut() {
            let now = self.generators.now().to_iso();
            envelope.id = self.generators.new_id().hyphenated().to_string();
            envelope.created_at.clone_from(&now);
            envelope.updated_at = now;
        }
        let (data, envelope) = self.encode(format, &text, encoding, line_ending, &copy)?;
        atomic::write_atomic_keeping_permissions(&target, &data)?;
        let path = fs::canonicalize(&target)?;
        let stamp = stamp_after_write(&path, &data)?;
        {
            let mut open = lock(&self.open);
            // The file may already be open under another id; that id goes.
            open.retain(|other, file| other == id || file.path != path);
            let file = open.get_mut(id).ok_or(ExternalError::Unknown)?;
            file.path.clone_from(&path);
            file.loaded = Some(Loaded {
                stamp,
                encoding,
                line_ending,
                text: Some(sha256(text.as_bytes())),
                envelope: envelope.or(copy.envelope),
                newer_format: false,
            });
        }
        self.remember_recent(&path);
        Ok(info(id.to_owned(), &path, format, data.len() as u64))
    }

    /// The content for "In Bibliothek aufnehmen": the text the user sees,
    /// as a file of the import (UTF-8 text, or a `.pwrite` with the
    /// envelope of the file). Returns the file name and the data.
    pub fn import_source(&self, id: &str, text: &str) -> Result<(String, Vec<u8>), ExternalError> {
        let open = lock(&self.open);
        let file = open.get(id).ok_or(ExternalError::Unknown)?;
        let name = file_name(&file.path);
        let text = normalise(text);
        let data = if file.format == DocumentFormat::Pwrite {
            let content = parse_content(&text)?;
            let loaded = file.loaded.as_ref().ok_or(ExternalError::Unknown)?;
            if loaded.newer_format {
                return Err(ExternalError::NewerFormat);
            }
            let mut envelope = loaded
                .envelope
                .clone()
                .ok_or(ExternalError::InvalidPwrite)?;
            envelope.content = content;
            pwrite::to_bytes(&envelope)
        } else {
            text.into_bytes()
        };
        Ok((name, data))
    }

    /// Ends the session of a file (its tab was closed).
    pub fn close(&self, id: &str) {
        lock(&self.open).remove(id);
    }

    /// "Zuletzt verwendet": the external files opened last, newest first.
    pub fn recent(&self) -> Vec<RecentFileEntry> {
        let entries = lock(&self.settings).get().recent_files.clone();
        entries
            .iter()
            .filter_map(|entry| {
                let path = PathBuf::from(&entry.path);
                let format = format_of_path(&path).ok()?;
                Some(RecentFileEntry {
                    id: recent_id(&entry.path),
                    name: file_name(&path),
                    folder: folder_of(&path),
                    format,
                    opened_at: entry.opened_at.clone(),
                    missing: !path.is_file(),
                })
            })
            .collect()
    }

    /// The path of a recent entry.
    pub fn recent_path(&self, recent: &str) -> Result<PathBuf, ExternalError> {
        lock(&self.settings)
            .get()
            .recent_files
            .iter()
            .find(|entry| recent_id(&entry.path) == recent)
            .map(|entry| PathBuf::from(&entry.path))
            .ok_or(ExternalError::Unknown)
    }

    /// Removes an entry from "Zuletzt verwendet".
    pub fn forget_recent(&self, recent: &str) -> Vec<RecentFileEntry> {
        let mut settings = lock(&self.settings);
        let known = settings
            .get()
            .recent_files
            .iter()
            .any(|entry| recent_id(&entry.path) == recent);
        if known
            && let Err(error) = settings.update(|settings| {
                settings
                    .recent_files
                    .retain(|entry| recent_id(&entry.path) != recent);
            })
        {
            log::warn!("recent files not saved: {error}");
        }
        drop(settings);
        self.recent()
    }

    fn remember_recent(&self, path: &Path) {
        // settings.json holds text; a path that is not Unicode stays out.
        let Some(text) = path.to_str() else {
            return;
        };
        let opened_at = self.generators.now().to_iso();
        let mut settings = lock(&self.settings);
        if settings.is_read_only() {
            return;
        }
        let result = settings.update(|settings| {
            settings.recent_files.retain(|entry| entry.path != text);
            settings.recent_files.insert(
                0,
                RecentFile {
                    path: text.to_owned(),
                    opened_at,
                },
            );
            settings.recent_files.truncate(MAX_RECENT_FILES);
        });
        if let Err(error) = result {
            log::warn!("recent files not saved: {error}");
        }
    }

    fn loaded_stamp(&self, id: &str) -> Result<(PathBuf, Stamp), ExternalError> {
        let open = lock(&self.open);
        let file = open.get(id).ok_or(ExternalError::Unknown)?;
        let loaded = file.loaded.as_ref().ok_or(ExternalError::Unknown)?;
        Ok((file.path.clone(), loaded.stamp.clone()))
    }

    fn update_stamp(&self, id: &str, old: &Stamp, current: Stamp) {
        let mut open = lock(&self.open);
        if let Some(loaded) = open.get_mut(id).and_then(|file| file.loaded.as_mut())
            && loaded.stamp == *old
        {
            loaded.stamp = current;
        }
    }

    fn is_newer(&self, id: &str) -> bool {
        lock(&self.open)
            .get(id)
            .and_then(|file| file.loaded.as_ref())
            .is_some_and(|loaded| loaded.newer_format)
    }

    /// The bytes to write and, for `.pwrite`, the new envelope.
    fn encode(
        &self,
        format: DocumentFormat,
        text: &str,
        encoding: TextEncoding,
        line_ending: LineEnding,
        loaded: &Loaded,
    ) -> Result<(Vec<u8>, Option<Box<PwriteFile>>), ExternalError> {
        if format != DocumentFormat::Pwrite {
            let data =
                encoding::encode(text, encoding, line_ending).map_err(|error| match error {
                    EncodingError::Unencodable(code) => ExternalError::Unencodable(code),
                    EncodingError::Binary => ExternalError::NotText,
                })?;
            return Ok((data, None));
        }
        let content = parse_content(text)?;
        let mut envelope = loaded
            .envelope
            .clone()
            .ok_or(ExternalError::InvalidPwrite)?;
        envelope.content = content;
        envelope.updated_at = self.generators.now().to_iso();
        Ok((pwrite::to_bytes(&envelope), Some(envelope)))
    }
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

fn info(id: String, path: &Path, format: DocumentFormat, size: u64) -> ExternalFileInfo {
    ExternalFileInfo {
        id,
        name: file_name(path),
        folder: folder_of(path),
        format,
        size,
        large: size >= LARGE_FILE_BYTES,
    }
}

fn file_name(path: &Path) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default()
}

fn folder_of(path: &Path) -> String {
    path.parent().map(display_path).unwrap_or_default()
}

/// Format by extension (the positive list of the import).
pub fn format_of_path(path: &Path) -> Result<DocumentFormat, ExternalError> {
    let name = path.file_name().ok_or(ExternalError::UnsupportedType)?;
    import::format_of(&name.to_string_lossy())
        .map(|(_, format)| format)
        .map_err(|_| ExternalError::UnsupportedType)
}

/// `target` with the extension of `format` unless it has one of it.
fn with_extension_of(target: &Path, format: DocumentFormat) -> PathBuf {
    if format_of_path(target).ok() == Some(format) {
        return target.to_path_buf();
    }
    let mut name = target.file_name().unwrap_or_default().to_os_string();
    name.push(".");
    name.push(format.as_str());
    target.with_file_name(name)
}

/// The id of a recent entry: the start of the SHA-256 of its path.
fn recent_id(path: &str) -> String {
    let hash = sha256(path.as_bytes());
    hash[..16]
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn sha256(data: &[u8]) -> [u8; 32] {
    Sha256::digest(data).into()
}

fn to_json(value: &Value) -> String {
    serde_json::to_string(value).unwrap_or_default()
}

/// Line ends as the editor has them: LF.
fn normalise(text: &str) -> String {
    if text.contains('\r') {
        text.replace("\r\n", "\n").replace('\r', "\n")
    } else {
        text.to_owned()
    }
}

/// `.pwrite` is always UTF-8 JSON on one line.
fn effective_encoding(format: DocumentFormat, encoding: TextEncoding) -> TextEncoding {
    if format == DocumentFormat::Pwrite {
        TextEncoding::Utf8
    } else {
        encoding
    }
}

fn effective_line_ending(format: DocumentFormat, line_ending: LineEnding) -> LineEnding {
    if format == DocumentFormat::Pwrite {
        LineEnding::Lf
    } else {
        line_ending
    }
}

fn parse_content(text: &str) -> Result<Value, ExternalError> {
    let content: Value = serde_json::from_str(text).map_err(|_| ExternalError::InvalidPwrite)?;
    pwrite::check_content(&content).map_err(|_| ExternalError::InvalidPwrite)?;
    Ok(content)
}

/// Whether the file may be written: opening it for writing (without
/// truncating) succeeds. Permission bits alone do not tell (owner, ACLs).
fn writable(path: &Path) -> bool {
    OpenOptions::new().write(true).open(path).is_ok()
}

fn read_with_stamp(path: &Path) -> Result<(Vec<u8>, Stamp), ExternalError> {
    let modified = fs::metadata(path)?.modified().ok();
    let data = atomic::read_limited(path, MAX_FILE_BYTES)?;
    let stamp = Stamp {
        len: data.len() as u64,
        modified,
        hash: sha256(&data),
    };
    Ok((data, stamp))
}

fn stamp_after_write(path: &Path, data: &[u8]) -> Result<Stamp, ExternalError> {
    let metadata = fs::metadata(path)?;
    Ok(Stamp {
        len: metadata.len(),
        modified: metadata.modified().ok(),
        hash: sha256(data),
    })
}

enum DiskStatus {
    /// Size and modification time as known.
    Same,
    /// Touched, content unchanged.
    Touched(Stamp),
    Changed(Stamp),
    Missing,
}

fn disk_status(path: &Path, known: &Stamp) -> Result<DiskStatus, ExternalError> {
    let metadata = match fs::metadata(path) {
        Ok(metadata) if metadata.is_file() => metadata,
        Ok(_) => return Ok(DiskStatus::Missing),
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(DiskStatus::Missing),
        Err(error) => return Err(error.into()),
    };
    if metadata.len() == known.len && metadata.modified().ok() == known.modified {
        return Ok(DiskStatus::Same);
    }
    let (_, current) = match read_with_stamp(path) {
        Ok(read) => read,
        Err(ExternalError::Missing) => return Ok(DiskStatus::Missing),
        Err(error) => return Err(error),
    };
    if current.hash == known.hash {
        Ok(DiskStatus::Touched(current))
    } else {
        Ok(DiskStatus::Changed(current))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::clock::Timestamp;

    struct Fixture {
        dir: tempfile::TempDir,
        files: ExternalFiles,
    }

    fn fixture() -> Fixture {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("target");
        fs::create_dir_all(&root).unwrap();
        let dir = tempfile::Builder::new()
            .prefix("pw-external-")
            .tempdir_in(root)
            .unwrap();
        let (settings, _) = AppSettingsStore::load(&dir.path().join("config"), Timestamp::now());
        let files = ExternalFiles::new(Arc::new(Mutex::new(settings)), Generators::system());
        Fixture { dir, files }
    }

    impl Fixture {
        fn write(&self, name: &str, data: &[u8]) -> PathBuf {
            let path = self.dir.path().join(name);
            fs::write(&path, data).unwrap();
            path
        }
    }

    fn request(text: &str, encoding: TextEncoding, line_ending: LineEnding) -> SaveRequest {
        SaveRequest {
            text: text.to_owned(),
            encoding,
            line_ending,
            overwrite: false,
        }
    }

    /// Makes sure a later write gets another modification time even on
    /// file systems with coarse timestamps.
    fn set_old_mtime(path: &Path) {
        let file = OpenOptions::new().write(true).open(path).unwrap();
        file.set_modified(SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(1_000_000))
            .unwrap();
    }

    #[test]
    fn unchanged_files_are_never_written() {
        let f = fixture();
        let original = b"\xEF\xBB\xBFErste Zeile\r\nZweite Zeile\r\n".to_vec();
        let path = f.write("notiz.txt", &original);
        set_old_mtime(&path);
        let before = fs::metadata(&path).unwrap().modified().unwrap();
        let info = f.files.register(&path).unwrap();
        assert_eq!(info.format, DocumentFormat::Txt);
        assert_eq!(info.name, "notiz.txt");
        assert!(!info.large);
        let text = f.files.read(&info.id).unwrap();
        assert_eq!(text.text, "Erste Zeile\nZweite Zeile\n");
        assert_eq!(text.encoding, TextEncoding::Utf8Bom);
        assert_eq!(text.line_ending, LineEnding::Crlf);
        assert!(!text.read_only);
        let outcome = f
            .files
            .save(
                &info.id,
                &request(&text.text, text.encoding, text.line_ending),
            )
            .unwrap();
        assert_eq!(outcome, SaveOutcome::Unchanged);
        assert_eq!(fs::read(&path).unwrap(), original);
        assert_eq!(fs::metadata(&path).unwrap().modified().unwrap(), before);

        // An edit keeps encoding and line endings.
        let edited = "Erste Zeile\nZweite Zeile\nDritte\n";
        let outcome = f
            .files
            .save(&info.id, &request(edited, text.encoding, text.line_ending))
            .unwrap();
        assert_eq!(outcome, SaveOutcome::Saved);
        assert_eq!(
            fs::read(&path).unwrap(),
            b"\xEF\xBB\xBFErste Zeile\r\nZweite Zeile\r\nDritte\r\n"
        );
        assert_eq!(
            f.files.check(&info.id).unwrap().status,
            FileStatus::Unchanged
        );
    }

    #[test]
    fn outside_changes_are_detected_and_not_overwritten() {
        let f = fixture();
        let path = f.write("kapitel.md", b"# Eins\n");
        set_old_mtime(&path);
        let info = f.files.register(&path).unwrap();
        f.files.read(&info.id).unwrap();
        assert_eq!(
            f.files.check(&info.id).unwrap().status,
            FileStatus::Unchanged
        );

        // Touched only: still unchanged.
        let file = OpenOptions::new().write(true).open(&path).unwrap();
        file.set_modified(SystemTime::now()).unwrap();
        drop(file);
        assert_eq!(
            f.files.check(&info.id).unwrap().status,
            FileStatus::Unchanged
        );

        fs::write(&path, b"# Eins von au\xC3\x9Fen\n").unwrap();
        assert_eq!(f.files.check(&info.id).unwrap().status, FileStatus::Changed);
        let mine = request("# Meine Fassung\n", TextEncoding::Utf8, LineEnding::Lf);
        assert!(matches!(
            f.files.save(&info.id, &mine),
            Err(ExternalError::Changed)
        ));
        assert_eq!(fs::read(&path).unwrap(), "# Eins von außen\n".as_bytes());

        // "Meine Fassung behalten": the next save writes.
        f.files.keep_mine(&info.id).unwrap();
        assert_eq!(
            f.files.check(&info.id).unwrap().status,
            FileStatus::Unchanged
        );
        let same = request("# Eins\n", TextEncoding::Utf8, LineEnding::Lf);
        assert_eq!(f.files.save(&info.id, &same).unwrap(), SaveOutcome::Saved);
        assert_eq!(fs::read(&path).unwrap(), b"# Eins\n");

        // Overwrite on request.
        fs::write(&path, b"wieder von au\xC3\x9Fen\n").unwrap();
        let forced = SaveRequest {
            overwrite: true,
            ..request("# Zwei\n", TextEncoding::Utf8, LineEnding::Lf)
        };
        assert_eq!(f.files.save(&info.id, &forced).unwrap(), SaveOutcome::Saved);
        assert_eq!(fs::read(&path).unwrap(), b"# Zwei\n");

        fs::remove_file(&path).unwrap();
        assert_eq!(f.files.check(&info.id).unwrap().status, FileStatus::Missing);
        assert!(matches!(
            f.files.save(&info.id, &mine),
            Err(ExternalError::Missing)
        ));
        assert!(!path.exists());
    }

    #[test]
    fn encodings_and_line_endings_can_be_changed_on_save() {
        let f = fixture();
        let path = f.write("alt.txt", b"Gr\xF6\xDFe\r\n");
        let info = f.files.register(&path).unwrap();
        let text = f.files.read(&info.id).unwrap();
        assert_eq!(text.encoding, TextEncoding::Windows1252);
        assert_eq!(text.text, "Größe\n");
        // Windows-1252 has no "→": nothing is written.
        let arrow = request("Größe →\n", TextEncoding::Windows1252, LineEnding::Crlf);
        assert!(matches!(
            f.files.save(&info.id, &arrow),
            Err(ExternalError::Unencodable(0x2192))
        ));
        assert_eq!(fs::read(&path).unwrap(), b"Gr\xF6\xDFe\r\n");
        // Same text, other encoding and line ending: written.
        let utf8 = request("Größe\n", TextEncoding::Utf8, LineEnding::Lf);
        assert_eq!(f.files.save(&info.id, &utf8).unwrap(), SaveOutcome::Saved);
        assert_eq!(fs::read(&path).unwrap(), "Größe\n".as_bytes());
    }

    #[test]
    fn binary_and_unknown_files_do_not_open() {
        let f = fixture();
        let binary = f.write("bild.txt", b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR");
        let info = f.files.register(&binary).unwrap();
        assert!(matches!(
            f.files.read(&info.id),
            Err(ExternalError::NotText)
        ));
        let other = f.write("tabelle.csv", b"a;b\n");
        assert!(matches!(
            f.files.register(&other),
            Err(ExternalError::UnsupportedType)
        ));
        assert!(matches!(
            f.files.register(&f.dir.path().join("fehlt.md")),
            Err(ExternalError::Missing)
        ));
        assert!(matches!(
            f.files.read("0".repeat(32).as_str()),
            Err(ExternalError::Unknown)
        ));
    }

    #[test]
    fn save_as_moves_the_id_to_the_new_file() {
        let f = fixture();
        let path = f.write("entwurf.md", b"Text\r\n");
        let info = f.files.register(&path).unwrap();
        f.files.read(&info.id).unwrap();
        let target = f.dir.path().join("kopie");
        let copy = f
            .files
            .save_as(
                &info.id,
                &target,
                &request("Meine Fassung\n", TextEncoding::Utf8, LineEnding::Crlf),
            )
            .unwrap();
        assert_eq!(copy.id, info.id);
        assert_eq!(copy.name, "kopie.md");
        assert_eq!(
            fs::read(f.dir.path().join("kopie.md")).unwrap(),
            b"Meine Fassung\r\n"
        );
        assert_eq!(fs::read(&path).unwrap(), b"Text\r\n");
        assert_eq!(
            f.files.check(&info.id).unwrap().status,
            FileStatus::Unchanged
        );
        // Recent: newest first.
        let recent = f.files.recent();
        assert_eq!(recent.len(), 2);
        assert_eq!(recent[0].name, "kopie.md");
        assert_eq!(recent[1].name, "entwurf.md");
    }

    #[test]
    fn pwrite_files_keep_unknown_fields_and_newer_ones_stay_untouched() {
        let f = fixture();
        let mut file = pwrite::new_file(
            "0b8f8f4e-6d0e-4f3e-9a3c-2a1d3c4b5e6f",
            "Roman",
            "2026-11-04T10:00:00Z",
            "de-DE",
            pwrite::empty_content(),
        );
        file.unknown
            .insert("future".to_owned(), serde_json::json!({ "keep": true }));
        let path = f.write("roman.pwrite", &pwrite::to_bytes(&file));
        let info = f.files.register(&path).unwrap();
        let text = f.files.read(&info.id).unwrap();
        assert!(!text.read_only);
        let content: Value = serde_json::from_str(&text.text).unwrap();
        assert_eq!(content, pwrite::empty_content());
        let edited = serde_json::json!({ "type": "doc", "content": [
            { "type": "paragraph", "content": [{ "type": "text", "text": "Hallo" }] }
        ] });
        let save = request(&edited.to_string(), TextEncoding::Utf8, LineEnding::Lf);
        assert_eq!(f.files.save(&info.id, &save).unwrap(), SaveOutcome::Saved);
        let stored: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        assert_eq!(stored["content"], edited);
        assert_eq!(stored["future"], serde_json::json!({ "keep": true }));
        assert_eq!(stored["id"], "0b8f8f4e-6d0e-4f3e-9a3c-2a1d3c4b5e6f");
        assert!(matches!(
            f.files.save(
                &info.id,
                &request("[1]", TextEncoding::Utf8, LineEnding::Lf)
            ),
            Err(ExternalError::InvalidPwrite)
        ));

        let mut newer = stored.clone();
        newer["formatVersion"] = serde_json::json!(2);
        let bytes = serde_json::to_vec(&newer).unwrap();
        let newer_path = f.write("neu.pwrite", &bytes);
        let info = f.files.register(&newer_path).unwrap();
        let text = f.files.read(&info.id).unwrap();
        assert!(text.read_only && text.newer_format);
        assert!(matches!(
            f.files.save(&info.id, &save),
            Err(ExternalError::NewerFormat)
        ));
        assert_eq!(fs::read(&newer_path).unwrap(), bytes);
    }

    #[test]
    fn recent_files_are_listed_and_forgotten() {
        let f = fixture();
        let first = f.write("a.txt", b"a");
        let second = f.write("b.md", b"b");
        f.files.register(&first).unwrap();
        f.files.register(&second).unwrap();
        f.files.register(&first).unwrap();
        let recent = f.files.recent();
        let names: Vec<_> = recent.iter().map(|entry| entry.name.as_str()).collect();
        assert_eq!(names, ["a.txt", "b.md"]);
        assert_eq!(recent[0].id.len(), 32);
        assert_eq!(
            f.files.recent_path(&recent[1].id).unwrap(),
            fs::canonicalize(&second).unwrap()
        );
        fs::remove_file(&second).unwrap();
        assert!(f.files.recent()[1].missing);
        let left = f.files.forget_recent(&recent[1].id);
        assert_eq!(left.len(), 1);
        assert_eq!(left[0].name, "a.txt");
    }

    #[cfg(unix)]
    #[test]
    fn read_only_files_are_reported_and_not_replaced() {
        use std::os::unix::fs::PermissionsExt;
        let f = fixture();
        let path = f.write("gesperrt.txt", b"alt\n");
        fs::set_permissions(&path, fs::Permissions::from_mode(0o444)).unwrap();
        let info = f.files.register(&path).unwrap();
        let text = f.files.read(&info.id).unwrap();
        // Root may write anyway; then the check has nothing to prove.
        if writable(&path) {
            return;
        }
        assert!(text.read_only);
        assert!(matches!(
            f.files.save(
                &info.id,
                &request("neu\n", TextEncoding::Utf8, LineEnding::Lf)
            ),
            Err(ExternalError::ReadOnly)
        ));
        assert_eq!(fs::read(&path).unwrap(), b"alt\n");
    }
}
