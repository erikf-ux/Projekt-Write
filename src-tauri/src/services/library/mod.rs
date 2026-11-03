//! The library: one folder with database, documents and media (sections 4
//! and 9). In version 1.0 exactly one library is open at a time.
//!
//! Opening a library, in this order:
//!
//! 1. Canonicalise the folder and read `library.json` (format and version).
//! 2. Refuse links at every fixed path of the structure.
//! 3. Take the exclusive lock on `library.lock`.
//! 4. Empty `tmp/`.
//! 5. Classify the location: WAL locally, rollback journal in sync folders
//!    and on network drives.
//! 6. Open the database, `PRAGMA quick_check`.
//! 7. Refuse a newer schema; before pending migrations copy the database to
//!    `backups/pre-migration/` and check the copy; then migrate.
//! 8. Compare the document folders with the database (report only).

pub mod layout;
pub mod lock;
pub mod maintenance;
pub mod manifest;

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::clock::Generators;
use crate::database::{Database, DatabaseError, JournalMode, diagnostics};
use crate::filesystem::location::{self, StorageLocation};
use crate::security::paths::{self, PathError, SafeRoot};
use crate::services::documents::DocumentRepository;

use layout::{
    DATABASE_FILE, GUARDED_PATHS, INITIAL_DIRS, KNOWN_TOP_LEVEL, LOCK_FILE, MANIFEST_FILE,
    PRE_MIGRATION_BACKUPS_DIR, PRE_MIGRATION_BACKUPS_KEPT, SYSTEM_JUNK_FILES,
};
use lock::{LibraryLock, LockError};
use maintenance::{DocumentScan, ScanError};
use manifest::{LibraryManifest, ManifestError, MovedTo};

#[derive(Debug, thiserror::Error)]
pub enum LibraryError {
    #[error("the folder does not contain a library")]
    NotALibrary,
    #[error("library.json is invalid: {0}")]
    InvalidManifest(String),
    #[error("library format {found} is newer than the supported version {supported}")]
    FormatTooNew { found: u64, supported: u32 },
    #[error("the library is open in another instance of Project Write")]
    Locked,
    #[error("file locks are not supported at this location")]
    LockUnsupported,
    #[error("the library was moved to {}", .0.path)]
    Moved(MovedTo),
    #[error("unsafe path in the library: {0}")]
    UnsafePath(PathError),
    #[error("the library database is missing")]
    DatabaseMissing,
    #[error("the library database is damaged")]
    DatabaseDamaged,
    #[error("the backup before the migration failed: {0}")]
    BackupFailed(String),
    #[error("the folder is not empty")]
    FolderNotEmpty,
    #[error("the folder already contains a library")]
    AlreadyALibrary,
    #[error("the path is not valid Unicode")]
    NonUnicodePath,
    #[error(transparent)]
    Database(DatabaseError),
    #[error("file system error: {0}")]
    Io(#[from] io::Error),
}

impl From<PathError> for LibraryError {
    fn from(error: PathError) -> Self {
        match error {
            PathError::Io { source, .. } | PathError::Root { source } => LibraryError::Io(source),
            other => LibraryError::UnsafePath(other),
        }
    }
}

impl From<DatabaseError> for LibraryError {
    fn from(error: DatabaseError) -> Self {
        LibraryError::Database(error)
    }
}

impl From<sqlx::Error> for LibraryError {
    fn from(error: sqlx::Error) -> Self {
        LibraryError::Database(DatabaseError::Sqlx(error))
    }
}

impl From<LockError> for LibraryError {
    fn from(error: LockError) -> Self {
        match error {
            LockError::Locked => LibraryError::Locked,
            LockError::Unsupported(_) => LibraryError::LockUnsupported,
            LockError::Io(error) => LibraryError::Io(error),
        }
    }
}

impl From<ManifestError> for LibraryError {
    fn from(error: ManifestError) -> Self {
        match error {
            ManifestError::Missing => LibraryError::NotALibrary,
            ManifestError::Invalid(reason) => LibraryError::InvalidManifest(reason),
            ManifestError::TooNew { found, supported } => {
                LibraryError::FormatTooNew { found, supported }
            }
            ManifestError::Path(error) => error.into(),
            ManifestError::Io(error) => LibraryError::Io(error),
        }
    }
}

impl From<ScanError> for LibraryError {
    fn from(error: ScanError) -> Self {
        match error {
            ScanError::Path(error) => error.into(),
            ScanError::Io(error) => LibraryError::Io(error),
            ScanError::Database(error) => error.into(),
        }
    }
}

/// Options for [`Library::open`].
#[derive(Debug, Clone, Copy, Default)]
pub struct OpenOptions {
    /// Open a library although `library.json` says it was moved elsewhere.
    pub accept_moved: bool,
}

/// What happened while a library was opened or created.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpenReport {
    /// Schema version found (0 for a new database).
    pub schema_version_before: i64,
    /// Schema version after migrating.
    pub schema_version: i64,
    /// Database copy made before migrating, relative to the library.
    pub migration_backup: Option<String>,
    /// Entries removed from `tmp/`.
    pub cleared_tmp_entries: usize,
    /// Result of comparing the document folders with the database.
    pub documents: DocumentScan,
}

/// An open, locked library.
#[derive(Debug)]
pub struct Library {
    root: SafeRoot,
    manifest: LibraryManifest,
    location: StorageLocation,
    database: Database,
    generators: Generators,
    write_lock: Arc<tokio::sync::Mutex<()>>,
    // Declared last: released after everything else.
    _lock: LibraryLock,
}

impl Library {
    /// Creates a library in `path`, which must be missing or empty.
    pub async fn create(
        path: &Path,
        generators: Generators,
    ) -> Result<(Self, OpenReport), LibraryError> {
        require_unicode(path)?;
        prepare_empty_folder(path)?;
        let root = SafeRoot::new(path)?;
        check_guarded_paths(&root)?;
        let lock = LibraryLock::acquire(&root.resolve(LOCK_FILE)?, generators.now())?;
        for dir in INITIAL_DIRS {
            root.create_dir_all(dir)?;
        }
        let location = location::classify(root.path());
        let database =
            Database::open_file(&root.resolve(DATABASE_FILE)?, location.journal_mode()).await?;
        let migrated = match database.migrate().await {
            Ok(report) => report,
            Err(error) => {
                database.close().await;
                return Err(error.into());
            }
        };
        // Written last: a folder is a library only once everything else exists.
        let manifest = LibraryManifest::new(generators.new_id(), generators.now());
        if let Err(error) = manifest.write(&root) {
            database.close().await;
            return Err(error.into());
        }
        log::info!("library {} created", manifest.id);
        let report = OpenReport {
            schema_version_before: 0,
            schema_version: migrated.version,
            migration_backup: None,
            cleared_tmp_entries: 0,
            documents: DocumentScan::default(),
        };
        Ok((
            Self::assemble(root, manifest, location, database, generators, lock),
            report,
        ))
    }

    /// Opens the library in `path`.
    pub async fn open(
        path: &Path,
        options: OpenOptions,
        generators: Generators,
    ) -> Result<(Self, OpenReport), LibraryError> {
        require_unicode(path)?;
        let root = SafeRoot::new(path).map_err(|error| match error {
            PathError::Root { source } if source.kind() == io::ErrorKind::NotFound => {
                LibraryError::NotALibrary
            }
            other => other.into(),
        })?;
        check_guarded_paths(&root)?;
        let manifest = LibraryManifest::read(&root)?;
        if let Some(moved) = &manifest.moved_to
            && !options.accept_moved
        {
            return Err(LibraryError::Moved(moved.clone()));
        }
        let lock = LibraryLock::acquire(&root.resolve(LOCK_FILE)?, generators.now())?;
        let cleared_tmp_entries = maintenance::clear_tmp(&root)?;

        let location = location::classify(root.path());
        let database_path = root.resolve(DATABASE_FILE)?;
        if !database_path.is_file() {
            return Err(LibraryError::DatabaseMissing);
        }
        let database = Database::open_existing(&database_path, location.journal_mode())
            .await
            .map_err(|error| {
                if is_not_a_database(&error) {
                    LibraryError::DatabaseDamaged
                } else {
                    error.into()
                }
            })?;
        match Self::prepare_database(&root, &database, &generators).await {
            Ok((schema_version_before, schema_version, migration_backup)) => {
                let documents = match maintenance::scan_documents(&root, &database).await {
                    Ok(scan) => scan,
                    Err(error) => {
                        database.close().await;
                        return Err(error.into());
                    }
                };
                if !documents.orphaned_files.is_empty() || !documents.missing_files.is_empty() {
                    log::warn!(
                        "library {}: {} files without database entry, {} documents without file",
                        manifest.id,
                        documents.orphaned_files.len(),
                        documents.missing_files.len()
                    );
                }
                log::info!(
                    "library {} opened ({:?}, schema {schema_version})",
                    manifest.id,
                    location.journal_mode()
                );
                let report = OpenReport {
                    schema_version_before,
                    schema_version,
                    migration_backup,
                    cleared_tmp_entries,
                    documents,
                };
                Ok((
                    Self::assemble(root, manifest, location, database, generators, lock),
                    report,
                ))
            }
            Err(error) => {
                database.close().await;
                Err(error)
            }
        }
    }

    /// Integrity check, schema check, backup and migration.
    async fn prepare_database(
        root: &SafeRoot,
        database: &Database,
        generators: &Generators,
    ) -> Result<(i64, i64, Option<String>), LibraryError> {
        match diagnostics::quick_check(database.writer()).await {
            Ok(true) => {}
            Ok(false) => return Err(LibraryError::DatabaseDamaged),
            Err(error) => {
                let error = DatabaseError::Sqlx(error);
                return Err(if is_not_a_database(&error) {
                    LibraryError::DatabaseDamaged
                } else {
                    error.into()
                });
            }
        }
        let before = database.schema_version().await?;
        let supported = Database::supported_version();
        if before > supported {
            return Err(DatabaseError::SchemaTooNew {
                found: before,
                supported,
            }
            .into());
        }
        let backup = if before > 0 && before < supported {
            Some(backup_before_migration(root, database, before, generators).await?)
        } else {
            None
        };
        let migrated = database.migrate().await?;
        for dir in INITIAL_DIRS {
            root.create_dir_all(dir)?;
        }
        Ok((before, migrated.version, backup))
    }

    fn assemble(
        root: SafeRoot,
        manifest: LibraryManifest,
        location: StorageLocation,
        database: Database,
        generators: Generators,
        lock: LibraryLock,
    ) -> Self {
        Self {
            root,
            manifest,
            location,
            database,
            generators,
            write_lock: Arc::new(tokio::sync::Mutex::new(())),
            _lock: lock,
        }
    }

    /// The canonical library folder.
    pub fn root(&self) -> &SafeRoot {
        &self.root
    }

    pub fn path(&self) -> &Path {
        self.root.path()
    }

    pub fn manifest(&self) -> &LibraryManifest {
        &self.manifest
    }

    pub fn location(&self) -> &StorageLocation {
        &self.location
    }

    pub fn journal_mode(&self) -> JournalMode {
        self.location.journal_mode()
    }

    pub fn database(&self) -> &Database {
        &self.database
    }

    pub fn generators(&self) -> &Generators {
        &self.generators
    }

    /// The document repository of this library.
    pub fn documents(&self) -> DocumentRepository {
        DocumentRepository::new(
            self.root.clone(),
            self.database.clone(),
            self.generators.clone(),
            self.write_lock.clone(),
        )
    }

    /// Closes the database, then releases the lock.
    pub async fn close(self) {
        let id = self.manifest.id;
        // Wait for running writes to finish.
        let _guard = self.write_lock.lock().await;
        self.database.close().await;
        log::info!("library {id} closed");
    }
}

/// SQLite reports `SQLITE_NOTADB` (26) or `SQLITE_CORRUPT` (11) for files
/// that are damaged beyond opening.
fn is_not_a_database(error: &DatabaseError) -> bool {
    match error {
        DatabaseError::Sqlx(sqlx::Error::Database(db)) => {
            matches!(db.code().as_deref(), Some("26" | "11"))
        }
        _ => false,
    }
}

fn require_unicode(path: &Path) -> Result<(), LibraryError> {
    path.to_str()
        .map(|_| ())
        .ok_or(LibraryError::NonUnicodePath)
}

/// Creates `path` if missing; otherwise requires an empty folder. A folder
/// that holds only system files or the skeleton of an interrupted creation
/// (no `library.json` yet) counts as empty.
fn prepare_empty_folder(path: &Path) -> Result<(), LibraryError> {
    match fs::symlink_metadata(path) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent)?;
            }
            paths::create_private_dir(path)?;
            Ok(())
        }
        Err(error) => Err(error.into()),
        Ok(metadata) if metadata.is_file() => Err(LibraryError::FolderNotEmpty),
        Ok(_) => {
            // A link to a folder is fine here: SafeRoot canonicalises it.
            for entry in fs::read_dir(path)? {
                let entry = entry?;
                let name = entry.file_name();
                let name = name.to_string_lossy();
                if name == MANIFEST_FILE {
                    return Err(LibraryError::AlreadyALibrary);
                }
                if SYSTEM_JUNK_FILES.contains(&name.as_ref()) {
                    continue;
                }
                if !KNOWN_TOP_LEVEL.contains(&name.as_ref()) || !is_skeleton(&entry.path())? {
                    return Err(LibraryError::FolderNotEmpty);
                }
            }
            Ok(())
        }
    }
}

/// Whether `path` holds nothing but empty folders and, in `database/`, the
/// database files of an interrupted creation.
fn is_skeleton(path: &Path) -> io::Result<bool> {
    let metadata = fs::symlink_metadata(path)?;
    if metadata.file_type().is_symlink() {
        return Ok(false);
    }
    if metadata.is_file() {
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or_default();
        let in_database_dir = path
            .parent()
            .and_then(Path::file_name)
            .is_some_and(|n| n == "database");
        return Ok(name == LOCK_FILE || (in_database_dir && name.starts_with("projectwrite.db")));
    }
    for entry in fs::read_dir(path)? {
        if !is_skeleton(&entry?.path())? {
            return Ok(false);
        }
    }
    Ok(true)
}

/// Rejects links at every fixed path of the library structure.
fn check_guarded_paths(root: &SafeRoot) -> Result<(), LibraryError> {
    for relative in GUARDED_PATHS {
        root.resolve(relative)?;
    }
    Ok(())
}

/// Copies the database before a migration (section 18: "mit Sicherung
/// vorab") and checks the copy. Keeps the newest copies only.
async fn backup_before_migration(
    root: &SafeRoot,
    database: &Database,
    version: i64,
    generators: &Generators,
) -> Result<String, LibraryError> {
    root.create_dir_all(PRE_MIGRATION_BACKUPS_DIR)?;
    let relative = format!(
        "{PRE_MIGRATION_BACKUPS_DIR}/projectwrite-schema-{version}-{}.db",
        generators.now().to_compact()
    );
    let target = root.resolve(&relative)?;
    let failed = |reason: String| {
        let _ = fs::remove_file(&target);
        LibraryError::BackupFailed(reason)
    };
    database
        .vacuum_into(&target)
        .await
        .map_err(|error| failed(error.to_string()))?;
    match diagnostics::integrity_check_file(&target).await {
        Ok(true) => {}
        Ok(false) => return Err(failed("integrity check of the copy failed".to_owned())),
        Err(error) => return Err(failed(error.to_string())),
    }
    prune_pre_migration_backups(root)?;
    log::info!("database copied to {relative} before migrating from schema {version}");
    Ok(relative)
}

fn prune_pre_migration_backups(root: &SafeRoot) -> Result<(), LibraryError> {
    let dir = root.resolve(PRE_MIGRATION_BACKUPS_DIR)?;
    let mut copies: Vec<PathBuf> = fs::read_dir(&dir)?
        .filter_map(Result::ok)
        .filter(|entry| entry.file_type().is_ok_and(|t| t.is_file()))
        .map(|entry| entry.path())
        .filter(|path| {
            path.file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.starts_with("projectwrite-schema-") && n.ends_with(".db"))
        })
        .collect();
    // Names end with a fixed-width UTC time stamp: newest last after sorting
    // by that stamp.
    copies.sort_by_key(|path| {
        path.file_name()
            .and_then(|n| n.to_str())
            .and_then(|n| n.rsplit('-').next())
            .unwrap_or_default()
            .to_owned()
    });
    let excess = copies.len().saturating_sub(PRE_MIGRATION_BACKUPS_KEPT);
    for path in copies.into_iter().take(excess) {
        fs::remove_file(path)?;
    }
    Ok(())
}
