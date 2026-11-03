//! Moving a library (section 9): "Der Umzug einer Bibliothek läuft in drei
//! Schritten: kopieren, prüfen, umschalten. Der alte Ordner bleibt, bis der
//! Nutzer ihn entfernt."
//!
//! 1. **Kopieren.** Writes are blocked for the whole move. Every file of the
//!    library is copied (temporary name, `fsync`, rename) and hashed on the
//!    way; the database is copied with `VACUUM INTO` (section 21 forbids
//!    copying the `.db` file). Not copied: `library.lock`, `tmp/`, temporary
//!    files of interrupted saves and the `-wal`/`-shm`/`-journal` files.
//! 2. **Prüfen.** Every copy is read again and compared by SHA-256; the
//!    database copy must pass `integrity_check` and hold as many documents as
//!    the original. Only then is `library.json` written into the new folder,
//!    which makes it a library.
//! 3. **Umschalten.** The old library is closed and the new one opened. If
//!    that fails, the old one is opened again. The old `library.json` gets a
//!    `movedTo` note, so opening the old copy by mistake needs a confirmation.
//!
//! Cancelling, a failure, or a new library that cannot be opened removes
//! exactly what the move created in the target folder, nothing else; the old
//! library stays in use.

use std::collections::BTreeSet;
use std::fs;
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use serde::Serialize;
use sha2::{Digest, Sha256};
use ts_rs::TS;

use super::layout::{DATABASE_FILE, LOCK_FILE, MANIFEST_FILE, SYSTEM_JUNK_FILES, TMP_DIR};
use super::manifest::{LibraryManifest, MovedTo};
use super::{Library, LibraryError, OpenOptions};
use crate::database::{DatabaseError, diagnostics};
use crate::filesystem::atomic::{self, is_temp_file_name};
use crate::security::paths::{self, PathError, display_path};

/// Size of the copy buffer.
const CHUNK: usize = 1024 * 1024;

/// Step of a running move.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum MovePhase {
    Copying,
    Verifying,
    Switching,
}

/// Progress of a running move.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct MoveProgress {
    pub phase: MovePhase,
    #[ts(type = "number")]
    pub done_bytes: u64,
    #[ts(type = "number")]
    pub total_bytes: u64,
    #[ts(type = "number")]
    pub done_files: u64,
    #[ts(type = "number")]
    pub total_files: u64,
}

/// Result of a completed move.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MoveReport {
    /// Files copied besides the database.
    pub files: u64,
    pub bytes: u64,
    /// Whether the old `library.json` received the `movedTo` note.
    pub old_copy_marked: bool,
}

#[derive(Debug, thiserror::Error)]
pub enum MoveError {
    #[error("the move was cancelled")]
    Cancelled,
    #[error("the target lies inside the library")]
    TargetInsideLibrary,
    #[error("the library lies inside the target")]
    LibraryInsideTarget,
    #[error("the target folder is not empty")]
    TargetNotEmpty,
    #[error("the path is not valid Unicode")]
    NonUnicodePath,
    #[error("unsafe path in the library: {0}")]
    UnsafePath(PathError),
    #[error("the copy differs from the original: {0}")]
    VerificationFailed(String),
    #[error("the new library could not be opened: {0}")]
    SwitchFailed(LibraryError),
    #[error(transparent)]
    Database(#[from] DatabaseError),
    #[error("file system error: {0}")]
    Io(#[from] io::Error),
}

impl From<PathError> for MoveError {
    fn from(error: PathError) -> Self {
        match error {
            PathError::Io { source, .. } | PathError::Root { source } => MoveError::Io(source),
            other => MoveError::UnsafePath(other),
        }
    }
}

impl From<sqlx::Error> for MoveError {
    fn from(error: sqlx::Error) -> Self {
        MoveError::Database(DatabaseError::Sqlx(error))
    }
}

/// A file to copy, relative to the library root (with `/`).
struct Entry {
    relative: String,
    size: u64,
}

/// Outcome of [`move_library`] when it fails: the error and the library that
/// is open afterwards (normally the old one; `None` only if neither library
/// could be opened). Boxed: the library handle is large.
pub type MoveFailure = Box<(Option<Library>, MoveError)>;

/// Moves `library` to `target` (missing or empty folder).
pub async fn move_library(
    library: Library,
    target: &Path,
    cancel: Arc<AtomicBool>,
    progress: impl Fn(MoveProgress) + Send + Sync + 'static,
) -> Result<(Library, MoveReport), MoveFailure> {
    let progress = Arc::new(progress);
    let source_root = library.path().to_path_buf();
    let target = match check_target(&source_root, target) {
        Ok(target) => target,
        Err(error) => return Err(Box::new((Some(library), error))),
    };

    // Writes wait until the move is over (copy and check see one state).
    let write_lock = library.write_lock.clone();
    let guard = write_lock.lock().await;

    let copied = copy_and_verify(
        &library,
        &source_root,
        &target,
        cancel.clone(),
        progress.clone(),
    )
    .await;
    let (files, bytes, created) = match copied {
        Ok(result) => result,
        Err((error, created)) => {
            remove_created(&target, &created);
            drop(guard);
            return Err(Box::new((Some(library), error)));
        }
    };

    // Switch.
    progress(MoveProgress {
        phase: MovePhase::Switching,
        done_bytes: bytes,
        total_bytes: bytes,
        done_files: files,
        total_files: files,
    });
    let mut old_manifest = library.manifest().clone();
    let generators = library.generators().clone();
    drop(guard);
    library.close().await;

    match Library::open(&target, OpenOptions::default(), generators.clone()).await {
        Ok((new_library, _)) => {
            old_manifest.moved_to = Some(MovedTo {
                path: display_path(&target),
                at: generators.now().to_iso(),
            });
            let old_copy_marked = match paths::SafeRoot::new(&source_root)
                .map_err(|e| e.to_string())
                .and_then(|root| old_manifest.write(&root).map_err(|e| e.to_string()))
            {
                Ok(()) => true,
                Err(error) => {
                    log::warn!("moved library: old library.json not marked: {error}");
                    false
                }
            };
            log::info!(
                "library {} moved ({files} files, {bytes} bytes)",
                new_library.manifest().id
            );
            Ok((
                new_library,
                MoveReport {
                    files,
                    bytes,
                    old_copy_marked,
                },
            ))
        }
        Err(error) => {
            log::error!("moved library cannot be opened: {error}");
            // The old library stays in use; the unused copy goes.
            remove_created(&target, &created);
            let reopened = Library::open(&source_root, OpenOptions::default(), generators)
                .await
                .map(|(library, _)| library)
                .map_err(|reopen| log::error!("old library cannot be reopened: {reopen}"))
                .ok();
            Err(Box::new((reopened, MoveError::SwitchFailed(error))))
        }
    }
}

/// Canonical target; must be missing or empty and unrelated to the library.
fn check_target(source_root: &Path, target: &Path) -> Result<PathBuf, MoveError> {
    target.to_str().ok_or(MoveError::NonUnicodePath)?;
    let target = paths::canonical_prospective(target).ok_or_else(|| {
        MoveError::Io(io::Error::new(
            io::ErrorKind::InvalidInput,
            "target folder is not usable",
        ))
    })?;
    if target.starts_with(source_root) {
        return Err(MoveError::TargetInsideLibrary);
    }
    if source_root.starts_with(&target) {
        return Err(MoveError::LibraryInsideTarget);
    }
    match fs::symlink_metadata(&target) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(target),
        Err(error) => Err(error.into()),
        Ok(metadata) if !metadata.is_dir() => Err(MoveError::TargetNotEmpty),
        Ok(_) => {
            for entry in fs::read_dir(&target)? {
                let name = entry?.file_name();
                if !SYSTEM_JUNK_FILES.contains(&name.to_string_lossy().as_ref()) {
                    return Err(MoveError::TargetNotEmpty);
                }
            }
            Ok(target)
        }
    }
}

/// Lists the files to copy. Links are refused.
fn collect(root: &Path) -> Result<Vec<Entry>, MoveError> {
    let mut entries = Vec::new();
    let mut stack = vec![(root.to_path_buf(), String::new())];
    while let Some((dir, prefix)) = stack.pop() {
        for item in fs::read_dir(&dir)? {
            let item = item?;
            let name = item.file_name();
            let name = name.to_str().ok_or(MoveError::NonUnicodePath)?.to_owned();
            let relative = if prefix.is_empty() {
                name.clone()
            } else {
                format!("{prefix}/{name}")
            };
            let file_type = item.file_type()?;
            if file_type.is_symlink() {
                return Err(MoveError::UnsafePath(PathError::SymbolicLink { relative }));
            }
            let skipped = relative == LOCK_FILE
                || relative == MANIFEST_FILE
                || relative == TMP_DIR
                || relative == "database"
                || is_temp_file_name(&name);
            if skipped {
                continue;
            }
            if file_type.is_dir() {
                stack.push((item.path(), relative));
            } else if file_type.is_file() {
                paths::split_relative(&relative)?;
                entries.push(Entry {
                    size: item.metadata()?.len(),
                    relative,
                });
            }
        }
    }
    entries.sort_by(|a, b| a.relative.cmp(&b.relative));
    Ok(entries)
}

type Created = BTreeSet<PathBuf>;

/// Copies and checks everything; returns files, bytes and the created paths
/// (for clean-up). On failure returns the created paths as well.
async fn copy_and_verify(
    library: &Library,
    source_root: &Path,
    target: &Path,
    cancel: Arc<AtomicBool>,
    progress: Arc<impl Fn(MoveProgress) + Send + Sync + 'static>,
) -> Result<(u64, u64, Created), (MoveError, Created)> {
    let mut created = Created::new();
    if !target.exists() {
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent).map_err(|e| (e.into(), created.clone()))?;
        }
        paths::create_private_dir(target).map_err(|e| (e.into(), created.clone()))?;
        created.insert(target.to_path_buf());
    }

    let source = source_root.to_path_buf();
    let destination = target.to_path_buf();
    let cancel_copy = cancel.clone();
    let progress_copy = progress.clone();
    let copied = tokio::task::spawn_blocking(move || {
        let mut created = Created::new();
        let result = copy_files(
            &source,
            &destination,
            &cancel_copy,
            &*progress_copy,
            &mut created,
        );
        (result, created)
    })
    .await
    .map_err(|e| (MoveError::Io(io::Error::other(e)), created.clone()))?;
    created.extend(copied.1);
    let (entries, hashes, bytes) = copied.0.map_err(|e| (e, created.clone()))?;

    // The database, as one consistent copy.
    let database_dir = target.join("database");
    if let Err(error) = fs::create_dir_all(&database_dir) {
        return Err((error.into(), created));
    }
    created.insert(database_dir.clone());
    let database_copy = target.join(DATABASE_FILE);
    if let Err(error) = library.database().vacuum_into(&database_copy).await {
        return Err((error.into(), created));
    }
    if cancel.load(Ordering::SeqCst) {
        return Err((MoveError::Cancelled, created));
    }

    // Check.
    let destination = target.to_path_buf();
    let cancel_check = cancel.clone();
    let progress_check = progress.clone();
    let total_files = entries.len() as u64;
    tokio::task::spawn_blocking(move || {
        verify_files(
            &destination,
            &entries,
            &hashes,
            bytes,
            &cancel_check,
            &*progress_check,
        )
    })
    .await
    .map_err(|e| (MoveError::Io(io::Error::other(e)), created.clone()))?
    .map_err(|e| (e, created.clone()))?;

    match diagnostics::integrity_check_file(&database_copy).await {
        Ok(true) => {}
        Ok(false) => {
            return Err((
                MoveError::VerificationFailed("integrity_check of the database copy".to_owned()),
                created,
            ));
        }
        Err(error) => return Err((error.into(), created)),
    }
    if let Err(error) = compare_databases(library, &database_copy).await {
        return Err((error, created));
    }
    if cancel.load(Ordering::SeqCst) {
        return Err((MoveError::Cancelled, created));
    }

    // Complete: library.json makes the folder a library.
    let mut manifest: LibraryManifest = library.manifest().clone();
    manifest.moved_to = None;
    let root = match paths::SafeRoot::new(target) {
        Ok(root) => root,
        Err(error) => return Err((error.into(), created)),
    };
    if let Err(error) = manifest.write(&root) {
        return Err((MoveError::Io(io::Error::other(error.to_string())), created));
    }
    created.insert(target.join(MANIFEST_FILE));
    Ok((total_files, bytes, created))
}

type Copied = (Vec<Entry>, Vec<String>, u64);

fn copy_files(
    source: &Path,
    target: &Path,
    cancel: &AtomicBool,
    progress: &(dyn Fn(MoveProgress) + Send + Sync),
    created: &mut Created,
) -> Result<Copied, MoveError> {
    let entries = collect(source)?;
    let total_bytes: u64 = entries.iter().map(|e| e.size).sum();
    let total_files = entries.len() as u64;
    let mut done_bytes = 0;
    let mut hashes = Vec::with_capacity(entries.len());
    let mut dirs = BTreeSet::new();
    progress(MoveProgress {
        phase: MovePhase::Copying,
        done_bytes,
        total_bytes,
        done_files: 0,
        total_files,
    });
    for (index, entry) in entries.iter().enumerate() {
        if cancel.load(Ordering::SeqCst) {
            return Err(MoveError::Cancelled);
        }
        let from = source.join(native(&entry.relative));
        let to = target.join(native(&entry.relative));
        let parent = to
            .parent()
            .expect("relative path has a parent")
            .to_path_buf();
        create_dirs(target, &parent, created)?;
        dirs.insert(parent);
        let hash = copy_file(&from, &to, cancel)?;
        created.insert(to);
        hashes.push(hash);
        done_bytes += entry.size;
        progress(MoveProgress {
            phase: MovePhase::Copying,
            done_bytes,
            total_bytes,
            done_files: index as u64 + 1,
            total_files,
        });
    }
    for dir in &dirs {
        atomic::sync_dir(dir)?;
    }
    Ok((entries, hashes, done_bytes))
}

/// `a/b/c` with the platform separator.
fn native(relative: &str) -> PathBuf {
    relative.split('/').collect()
}

/// Creates the directories between `root` and `dir`, recording new ones.
fn create_dirs(root: &Path, dir: &Path, created: &mut Created) -> io::Result<()> {
    let mut missing = Vec::new();
    let mut current = dir;
    while current != root && !current.exists() {
        missing.push(current.to_path_buf());
        current = match current.parent() {
            Some(parent) => parent,
            None => break,
        };
    }
    for path in missing.into_iter().rev() {
        paths::create_private_dir(&path)?;
        created.insert(path);
    }
    Ok(())
}

/// Copies one file through a temporary name and returns its SHA-256.
fn copy_file(from: &Path, to: &Path, cancel: &AtomicBool) -> Result<String, MoveError> {
    let name = to
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or(MoveError::NonUnicodePath)?;
    let temp = to.with_file_name(format!(
        ".{name}.{}{}",
        uuid::Uuid::new_v4().simple(),
        atomic::TEMP_SUFFIX
    ));
    let result = (|| {
        let mut input = atomic::open_read_no_follow(from)?;
        let mut output = atomic::create_new_private(&temp)?;
        let mut hasher = Sha256::new();
        let mut buffer = vec![0u8; CHUNK];
        loop {
            if cancel.load(Ordering::SeqCst) {
                return Err(MoveError::Cancelled);
            }
            let read = input.read(&mut buffer)?;
            if read == 0 {
                break;
            }
            hasher.update(&buffer[..read]);
            output.write_all(&buffer[..read])?;
        }
        output.sync_all()?;
        drop(output);
        fs::rename(&temp, to)?;
        Ok(hex(&hasher.finalize()))
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temp);
    }
    result
}

fn hash_file(path: &Path) -> io::Result<String> {
    let mut input = atomic::open_read_no_follow(path)?;
    let mut hasher = Sha256::new();
    let mut buffer = vec![0u8; CHUNK];
    loop {
        let read = input.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(hex(&hasher.finalize()))
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn verify_files(
    target: &Path,
    entries: &[Entry],
    hashes: &[String],
    total_bytes: u64,
    cancel: &AtomicBool,
    progress: &(dyn Fn(MoveProgress) + Send + Sync),
) -> Result<(), MoveError> {
    let total_files = entries.len() as u64;
    let mut done_bytes = 0;
    for (index, (entry, expected)) in entries.iter().zip(hashes).enumerate() {
        if cancel.load(Ordering::SeqCst) {
            return Err(MoveError::Cancelled);
        }
        let actual = hash_file(&target.join(native(&entry.relative)))?;
        if &actual != expected {
            return Err(MoveError::VerificationFailed(entry.relative.clone()));
        }
        done_bytes += entry.size;
        progress(MoveProgress {
            phase: MovePhase::Verifying,
            done_bytes,
            total_bytes,
            done_files: index as u64 + 1,
            total_files,
        });
    }
    Ok(())
}

/// The copy holds the same schema version and the same documents.
async fn compare_databases(library: &Library, copy: &Path) -> Result<(), MoveError> {
    use sqlx::sqlite::SqliteConnectOptions;
    use sqlx::{ConnectOptions, Connection};

    let original_version = library.database().schema_version().await?;
    let original_documents: i64 = sqlx::query_scalar("SELECT count(*) FROM documents")
        .fetch_one(library.database().reader())
        .await?;
    let mut connection = SqliteConnectOptions::new()
        .filename(copy)
        .read_only(true)
        .connect()
        .await?;
    let copy_version: i64 = sqlx::query_scalar(
        "SELECT coalesce(max(version), 0) FROM _sqlx_migrations WHERE success = 1",
    )
    .fetch_one(&mut connection)
    .await?;
    let copy_documents: i64 = sqlx::query_scalar("SELECT count(*) FROM documents")
        .fetch_one(&mut connection)
        .await?;
    connection.close().await?;
    if copy_version != original_version || copy_documents != original_documents {
        return Err(MoveError::VerificationFailed(format!(
            "database copy: schema {copy_version}/{original_version}, documents {copy_documents}/{original_documents}"
        )));
    }
    Ok(())
}

/// Removes what the move created, deepest paths first. Pre-existing files in
/// the target (only system files are allowed there) stay.
fn remove_created(target: &Path, created: &Created) {
    let database = target.join(DATABASE_FILE);
    let mut paths: Vec<&PathBuf> = created.iter().collect();
    paths.sort_by_key(|path| std::cmp::Reverse(path.components().count()));
    for path in paths {
        let result = if path.is_dir() {
            // The database folder also holds the VACUUM INTO copy.
            if path == &target.join("database") {
                for suffix in ["", "-journal", "-wal", "-shm"] {
                    let _ = fs::remove_file(format!("{}{suffix}", database.display()));
                }
            }
            fs::remove_dir(path)
        } else {
            fs::remove_file(path)
        };
        if let Err(error) = result
            && error.kind() != io::ErrorKind::NotFound
        {
            log::warn!("move clean-up: cannot remove {}: {error}", path.display());
        }
    }
}
