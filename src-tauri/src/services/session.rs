//! The library of this app instance: which one is open, how it was chosen,
//! and what the frontend may know about it (sections 4, 8 and 9).
//!
//! * Exactly one library is open at a time (or none, or a move is running).
//! * Folders are chosen in Rust (native dialog or the default location);
//!   the frontend receives a token and a display text, never sends a path.
//! * Sync folders and network drives need an explicit confirmation before a
//!   library is created there or moved there; the confirmation is stored in
//!   the library (`storage.acceptedLocation`).
//! * The path of the open library is remembered in the app settings, so the
//!   next start opens it again.

use std::collections::VecDeque;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::clock::Generators;
use crate::database::{Database, DatabaseError, JournalMode};
use crate::error::{CommandError, ErrorCode};
use crate::filesystem::location::{self, StorageLocation};
use crate::security::paths::{self, display_path};
use crate::services::backup::{self, BackupProgress, BackupSummary};
use crate::services::dialog::{FolderPicker, PickRequest};
use crate::services::documents::DocumentRepository;
use crate::services::history::RetentionPolicy;
use crate::services::library::layout::{MANIFEST_FILE, SYSTEM_JUNK_FILES};
use crate::services::library::relocate::{self, MoveError, MoveProgress};
use crate::services::library::{Library, LibraryError, OpenOptions, OpenReport};
use crate::services::media::MediaLibrary;
use crate::services::search::SearchIndex;
use crate::services::settings::app::{AppSettingsStore, LoadOutcome, SavedSession};
use crate::services::settings::library::{
    AcceptedLocation, HistoryDailyDays, HistoryKeepAllHours, HistoryWeeklyDays, TrashRetentionDays,
};
use crate::services::structure::Structure;

/// Name of the library folder inside the documents folder (section 9:
/// "Standard ist `Dokumente/ProjectWrite`").
pub const DEFAULT_LIBRARY_FOLDER: &str = "ProjectWrite";
/// Location tokens kept at the same time.
const MAX_TOKENS: usize = 16;

/// Folders of the app outside any library.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppDirs {
    /// App settings (`settings.json`).
    pub config: PathBuf,
    /// Log files.
    pub logs: PathBuf,
    /// The user's documents folder; parent of the default library location.
    pub documents: Option<PathBuf>,
}

/// What a chosen folder is meant for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum LocationPurpose {
    /// A new library.
    Create,
    /// An existing library.
    Open,
    /// The new place of the open library.
    Move,
}

/// What the chosen folder contains.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum FolderContents {
    /// Does not exist yet; it will be created.
    Missing,
    /// Exists and is empty.
    Empty,
    /// Contains a library.
    Library,
    /// Contains other files.
    Other,
}

/// A folder chosen in Rust, as the frontend sees it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct LocationChoice {
    /// Refers to the folder in later commands.
    pub token: String,
    pub purpose: LocationPurpose,
    /// The folder as text (display only).
    pub display_path: String,
    pub contents: FolderContents,
    /// Sync folder or network drive need a confirmation for create and move.
    pub location: StorageLocation,
}

/// The journal mode in use, for display.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum JournalModeName {
    Wal,
    Rollback,
}

impl From<JournalMode> for JournalModeName {
    fn from(mode: JournalMode) -> Self {
        match mode {
            JournalMode::Wal => JournalModeName::Wal,
            JournalMode::Rollback => JournalModeName::Rollback,
        }
    }
}

/// Findings of the last open, worth showing to the user.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct OpenNotes {
    /// Schema version before the migration, if one ran.
    #[ts(type = "number | null")]
    pub migrated_from: Option<i64>,
    /// Database copy made before the migration (relative to the library).
    pub migration_backup: Option<String>,
    /// Files without database entry (reported, not deleted).
    pub orphaned_files: u32,
    /// Documents whose file is missing.
    pub missing_files: u32,
}

impl OpenNotes {
    fn from_report(report: &OpenReport) -> Self {
        let count = |n: usize| u32::try_from(n).unwrap_or(u32::MAX);
        Self {
            migrated_from: report
                .migration_backup
                .as_ref()
                .map(|_| report.schema_version_before),
            migration_backup: report.migration_backup.clone(),
            orphaned_files: count(report.documents.orphaned_files.len()),
            missing_files: count(report.documents.missing_files.len()),
        }
    }
}

/// Problems with the app settings file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum SettingsNotice {
    /// The file was damaged; it was kept under another name.
    Damaged,
    /// The file comes from a newer version and is not changed.
    NewerVersion,
    /// The library path could not be saved; the next start will not open it.
    NotSaved,
}

/// The open library, as the frontend sees it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct LibraryInfo {
    pub display_path: String,
    /// Name of the library folder.
    pub name: String,
    pub location: StorageLocation,
    /// The user confirmed this sync folder or network drive (always true
    /// for local disks).
    pub location_accepted: bool,
    pub journal_mode: JournalModeName,
    #[ts(type = "number")]
    pub schema_version: i64,
    #[ts(type = "number")]
    pub document_count: u64,
    pub notes: OpenNotes,
}

/// State of the library of this instance.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(tag = "state", rename_all = "camelCase")]
#[ts(export)]
pub enum LibraryStatus {
    /// No library is open.
    #[serde(rename_all = "camelCase")]
    Closed {
        /// The library from the app settings, if any (display text).
        configured_path: Option<String>,
        /// Why it is not open.
        error: Option<CommandError>,
        settings_notice: Option<SettingsNotice>,
    },
    /// A library is open.
    #[serde(rename_all = "camelCase")]
    Open {
        library: LibraryInfo,
        settings_notice: Option<SettingsNotice>,
    },
    /// The library is being moved.
    Moving,
}

enum Slot {
    Closed {
        error: Option<CommandError>,
    },
    Open {
        library: Box<Library>,
        notes: OpenNotes,
    },
    Moving,
}

struct Chosen {
    token: String,
    path: PathBuf,
    purpose: LocationPurpose,
}

/// The library session of the app.
pub struct LibrarySession {
    dirs: AppDirs,
    generators: Generators,
    /// Shared with the interface settings ([`crate::services::ui`]).
    settings: Arc<Mutex<AppSettingsStore>>,
    settings_notice: Mutex<Option<SettingsNotice>>,
    slot: tokio::sync::Mutex<Slot>,
    tokens: Mutex<VecDeque<Chosen>>,
    move_cancel: Mutex<Option<Arc<AtomicBool>>>,
    /// A backup in progress (ADR 0021).
    backup: Arc<Mutex<backup::control::Activity>>,
    /// The daily automatic backup at start (off in end-to-end runs that ask).
    automatic_backups: AtomicBool,
}

impl LibrarySession {
    pub fn new(
        dirs: AppDirs,
        generators: Generators,
        settings: Arc<Mutex<AppSettingsStore>>,
        outcome: &LoadOutcome,
    ) -> Self {
        let notice = match outcome {
            LoadOutcome::Damaged { .. } => Some(SettingsNotice::Damaged),
            LoadOutcome::Newer { .. } => Some(SettingsNotice::NewerVersion),
            LoadOutcome::Missing | LoadOutcome::Loaded => None,
        };
        Self {
            dirs,
            generators,
            settings,
            settings_notice: Mutex::new(notice),
            slot: tokio::sync::Mutex::new(Slot::Closed { error: None }),
            tokens: Mutex::new(VecDeque::new()),
            move_cancel: Mutex::new(None),
            backup: Arc::new(Mutex::new(backup::control::Activity::default())),
            automatic_backups: AtomicBool::new(automatic_backups_wanted()),
        }
    }

    pub fn dirs(&self) -> &AppDirs {
        &self.dirs
    }

    fn configured_path(&self) -> Option<String> {
        lock(&self.settings).get().library_path.clone()
    }

    fn notice(&self) -> Option<SettingsNotice> {
        *lock(&self.settings_notice)
    }

    /// Opens the library from the app settings (at start).
    pub async fn open_configured(&self) {
        let Some(path) = self.configured_path() else {
            return;
        };
        let mut slot = self.slot.lock().await;
        let result = Library::open(
            Path::new(&path),
            OpenOptions::default(),
            self.generators.clone(),
        )
        .await;
        *slot = match result {
            Ok((library, report)) => {
                library.housekeeping().await;
                self.start_automatic_backup(&library);
                Slot::Open {
                    notes: OpenNotes::from_report(&report),
                    library: Box::new(library),
                }
            }
            Err(error) => {
                log::warn!("configured library not opened: {error}");
                Slot::Closed {
                    error: Some(library_error(error)),
                }
            }
        };
    }

    /// Current state for the frontend.
    pub async fn status(&self) -> LibraryStatus {
        let slot = self.slot.lock().await;
        self.describe(&slot).await
    }

    async fn describe(&self, slot: &Slot) -> LibraryStatus {
        let settings_notice = self.notice();
        match slot {
            Slot::Closed { error } => LibraryStatus::Closed {
                configured_path: self.configured_path(),
                error: error.clone(),
                settings_notice,
            },
            Slot::Moving => LibraryStatus::Moving,
            Slot::Open { library, notes } => {
                let document_count = library.documents().count().await.unwrap_or_else(|error| {
                    log::warn!("document count failed: {error}");
                    0
                });
                let schema_version = library.database().schema_version().await.unwrap_or(0);
                let location = library.location().clone();
                let accepted = library
                    .settings()
                    .get::<AcceptedLocation>()
                    .await
                    .ok()
                    .flatten();
                let location_accepted =
                    !location.needs_warning() || accepted.as_ref() == Some(&location);
                LibraryStatus::Open {
                    library: LibraryInfo {
                        display_path: display_path(library.path()),
                        name: library
                            .path()
                            .file_name()
                            .map(|n| n.to_string_lossy().into_owned())
                            .unwrap_or_default(),
                        journal_mode: library.journal_mode().into(),
                        location,
                        location_accepted,
                        schema_version,
                        document_count,
                        notes: notes.clone(),
                    },
                    settings_notice,
                }
            }
        }
    }

    /// Runs `operation` with the document repository of the open library.
    /// The library stays locked meanwhile, so a move never starts in the
    /// middle of a save (it waits; later calls get `LIBRARY_BUSY`).
    pub async fn with_documents<T, F, Fut>(&self, operation: F) -> Result<T, CommandError>
    where
        F: FnOnce(DocumentRepository) -> Fut,
        Fut: std::future::Future<Output = Result<T, CommandError>>,
    {
        let slot = self.slot.lock().await;
        let repository = match &*slot {
            Slot::Open { library, .. } => library.documents(),
            Slot::Moving => return Err(CommandError::new(ErrorCode::LibraryBusy)),
            Slot::Closed { .. } => return Err(CommandError::new(ErrorCode::LibraryNotOpen)),
        };
        let result = operation(repository).await;
        drop(slot);
        result
    }

    /// Runs `operation` with the structure (subnets, tags, structural undo)
    /// and the document repository of the open library, locked like
    /// [`Self::with_documents`].
    pub async fn with_structure<T, F, Fut>(&self, operation: F) -> Result<T, CommandError>
    where
        F: FnOnce(Structure, DocumentRepository) -> Fut,
        Fut: std::future::Future<Output = Result<T, CommandError>>,
    {
        let slot = self.slot.lock().await;
        let (structure, documents) = match &*slot {
            Slot::Open { library, .. } => (library.structure(), library.documents()),
            Slot::Moving => return Err(CommandError::new(ErrorCode::LibraryBusy)),
            Slot::Closed { .. } => return Err(CommandError::new(ErrorCode::LibraryNotOpen)),
        };
        let result = operation(structure, documents).await;
        drop(slot);
        result
    }

    /// Runs `operation` with the media of the open library.
    pub async fn with_media<T, F, Fut>(&self, operation: F) -> Result<T, CommandError>
    where
        F: FnOnce(MediaLibrary, Structure) -> Fut,
        Fut: std::future::Future<Output = Result<T, CommandError>>,
    {
        let slot = self.slot.lock().await;
        let (media, structure) = match &*slot {
            Slot::Open { library, .. } => (library.media(), library.structure()),
            Slot::Moving => return Err(CommandError::new(ErrorCode::LibraryBusy)),
            Slot::Closed { .. } => return Err(CommandError::new(ErrorCode::LibraryNotOpen)),
        };
        let result = operation(media, structure).await;
        drop(slot);
        result
    }

    /// Runs `operation` with the Mesh and the structure of the open library.
    pub async fn with_mesh<T, F, Fut>(&self, operation: F) -> Result<T, CommandError>
    where
        F: FnOnce(crate::services::mesh::Mesh, Structure) -> Fut,
        Fut: std::future::Future<Output = Result<T, CommandError>>,
    {
        let slot = self.slot.lock().await;
        let (mesh, structure) = match &*slot {
            Slot::Open { library, .. } => (library.mesh(), library.structure()),
            Slot::Moving => return Err(CommandError::new(ErrorCode::LibraryBusy)),
            Slot::Closed { .. } => return Err(CommandError::new(ErrorCode::LibraryNotOpen)),
        };
        let result = operation(mesh, structure).await;
        drop(slot);
        result
    }

    /// Runs `operation` with the search index of the open library.
    pub async fn with_search<T, F, Fut>(&self, operation: F) -> Result<T, CommandError>
    where
        F: FnOnce(SearchIndex) -> Fut,
        Fut: std::future::Future<Output = Result<T, CommandError>>,
    {
        let slot = self.slot.lock().await;
        let search = match &*slot {
            Slot::Open { library, .. } => library.search(),
            Slot::Moving => return Err(CommandError::new(ErrorCode::LibraryBusy)),
            Slot::Closed { .. } => return Err(CommandError::new(ErrorCode::LibraryNotOpen)),
        };
        let result = operation(search).await;
        drop(slot);
        result
    }

    /// The Mesh of the open library, without holding the session: the
    /// grouping analysis runs in the background.
    pub async fn mesh(&self) -> Result<crate::services::mesh::Mesh, CommandError> {
        match &*self.slot.lock().await {
            Slot::Open { library, .. } => Ok(library.mesh()),
            Slot::Moving => Err(CommandError::new(ErrorCode::LibraryBusy)),
            Slot::Closed { .. } => Err(CommandError::new(ErrorCode::LibraryNotOpen)),
        }
    }

    /// The exports of the open library, without holding the session: a
    /// library export runs over many calls.
    pub async fn exports(&self) -> Result<crate::services::export::Exports, CommandError> {
        match &*self.slot.lock().await {
            Slot::Open { library, .. } => Ok(library.exports()),
            Slot::Moving => Err(CommandError::new(ErrorCode::LibraryBusy)),
            Slot::Closed { .. } => Err(CommandError::new(ErrorCode::LibraryNotOpen)),
        }
    }

    /// The media of the open library, without holding the session: for long
    /// work (copying a folder for import) and for the media handler, which
    /// must not wait for other commands. If the library closes meanwhile,
    /// its database refuses further work.
    pub async fn media(&self) -> Result<MediaLibrary, CommandError> {
        match &*self.slot.lock().await {
            Slot::Open { library, .. } => Ok(library.media()),
            Slot::Moving => Err(CommandError::new(ErrorCode::LibraryBusy)),
            Slot::Closed { .. } => Err(CommandError::new(ErrorCode::LibraryNotOpen)),
        }
    }

    /// The id of the open library.
    pub async fn library_id(&self) -> Option<String> {
        match &*self.slot.lock().await {
            Slot::Open { library, .. } => Some(library.manifest().id.hyphenated().to_string()),
            Slot::Closed { .. } | Slot::Moving => None,
        }
    }

    /// Stores the tabs of this session (stage 1).
    pub fn save_tabs(&self, session: Option<SavedSession>) {
        let result = lock(&self.settings).update(|settings| settings.session = session);
        if let Err(error) = result {
            log::warn!("session not saved: {error}");
        }
    }

    /// The tabs of the last session.
    pub fn saved_tabs(&self) -> Option<SavedSession> {
        lock(&self.settings).get().session.clone()
    }

    /// The current time (ISO 8601), from the clock of the session.
    pub fn now(&self) -> String {
        self.generators.now().to_iso()
    }

    /// Days documents stay in the trash, if a library is open.
    pub async fn trash_retention_days(&self) -> Option<u32> {
        let slot = self.slot.lock().await;
        match &*slot {
            Slot::Open { library, .. } => library.settings().get::<TrashRetentionDays>().await.ok(),
            Slot::Closed { .. } | Slot::Moving => None,
        }
    }

    /// The database of the open library (for the system check).
    pub async fn database(&self) -> Option<Database> {
        match &*self.slot.lock().await {
            Slot::Open { library, .. } => Some(library.database().clone()),
            Slot::Closed { .. } | Slot::Moving => None,
        }
    }

    /// `Dokumente/ProjectWrite`, if the system has a documents folder.
    pub fn default_location(&self) -> Option<LocationChoice> {
        let documents = self.dirs.documents.as_ref()?;
        Some(self.register(
            documents.join(DEFAULT_LIBRARY_FOLDER),
            LocationPurpose::Create,
        ))
    }

    /// Shows the folder dialog. `None` if the user cancelled.
    pub async fn choose_location(
        &self,
        picker: &dyn FolderPicker,
        purpose: LocationPurpose,
        title: String,
    ) -> Option<LocationChoice> {
        let start_in = self.dirs.documents.clone();
        let picked = picker
            .pick_folder(PickRequest { title, start_in })
            .await
            .ok()
            .flatten()?;
        let target = match purpose {
            LocationPurpose::Open => picked,
            // A folder with other content gets the library as a subfolder,
            // so the user's files and the library never mix.
            LocationPurpose::Create | LocationPurpose::Move => {
                if inspect(&picked) == FolderContents::Other {
                    picked.join(DEFAULT_LIBRARY_FOLDER)
                } else {
                    picked
                }
            }
        };
        Some(self.register(target, purpose))
    }

    fn register(&self, path: PathBuf, purpose: LocationPurpose) -> LocationChoice {
        let path = paths::canonical_prospective(&path).unwrap_or(path);
        let token = uuid::Uuid::new_v4().simple().to_string();
        let choice = LocationChoice {
            token: token.clone(),
            purpose,
            display_path: display_path(&path),
            contents: inspect(&path),
            location: location::classify(&path),
        };
        let mut tokens = lock(&self.tokens);
        tokens.push_back(Chosen {
            token,
            path,
            purpose,
        });
        while tokens.len() > MAX_TOKENS {
            tokens.pop_front();
        }
        choice
    }

    /// The folder behind a token. Creating and moving need a token chosen for
    /// that purpose (they write into the folder); opening accepts any token,
    /// so a folder that turns out to hold a library can be opened directly.
    fn resolve_token(
        &self,
        token: &str,
        purpose: LocationPurpose,
    ) -> Result<PathBuf, CommandError> {
        lock(&self.tokens)
            .iter()
            .find(|chosen| {
                chosen.token == token
                    && (chosen.purpose == purpose || purpose == LocationPurpose::Open)
            })
            .map(|chosen| chosen.path.clone())
            .ok_or_else(|| CommandError::new(ErrorCode::LocationExpired))
    }

    /// Creates a library at a chosen location and makes it the open one.
    pub async fn create(
        &self,
        token: &str,
        accept_warning: bool,
    ) -> Result<LibraryStatus, CommandError> {
        let path = self.resolve_token(token, LocationPurpose::Create)?;
        let location = location::classify(&path);
        if location.needs_warning() && !accept_warning {
            return Err(CommandError::new(ErrorCode::LocationNotAccepted));
        }
        let mut slot = self.slot.lock().await;
        if matches!(*slot, Slot::Moving) {
            return Err(CommandError::new(ErrorCode::LibraryBusy));
        }
        let (library, report) = Library::create(&path, self.generators.clone())
            .await
            .map_err(library_error)?;
        if library.location().needs_warning() {
            let accepted = Some(library.location().clone());
            if let Err(error) = library.settings().set::<AcceptedLocation>(&accepted).await {
                log::warn!("accepted location not stored: {error}");
            }
        }
        self.switch_to(&mut slot, library, &report).await;
        Ok(self.describe(&slot).await)
    }

    /// Opens an existing library and makes it the open one.
    pub async fn open(
        &self,
        token: &str,
        accept_moved: bool,
    ) -> Result<LibraryStatus, CommandError> {
        let path = self.resolve_token(token, LocationPurpose::Open)?;
        let mut slot = self.slot.lock().await;
        if matches!(*slot, Slot::Moving) {
            return Err(CommandError::new(ErrorCode::LibraryBusy));
        }
        if let Slot::Open { library, .. } = &*slot
            && paths::canonical_dir(&path).ok().as_deref() == Some(library.path())
        {
            // Already open here.
            return Ok(self.describe(&slot).await);
        }
        let (library, report) =
            Library::open(&path, OpenOptions { accept_moved }, self.generators.clone())
                .await
                .map_err(library_error)?;
        self.switch_to(&mut slot, library, &report).await;
        Ok(self.describe(&slot).await)
    }

    /// Tries the library from the app settings again (after an error).
    pub async fn reopen(&self) -> Result<LibraryStatus, CommandError> {
        {
            let slot = self.slot.lock().await;
            if !matches!(*slot, Slot::Closed { .. }) {
                return Ok(self.describe(&slot).await);
            }
        }
        self.open_configured().await;
        let slot = self.slot.lock().await;
        match &*slot {
            Slot::Closed { error: Some(error) } => Err(error.clone()),
            _ => Ok(self.describe(&slot).await),
        }
    }

    /// Replaces the open library by `library` and remembers its path.
    async fn switch_to(&self, slot: &mut Slot, library: Library, report: &OpenReport) {
        library.housekeeping().await;
        let previous = std::mem::replace(
            slot,
            Slot::Open {
                notes: OpenNotes::from_report(report),
                library: Box::new(library),
            },
        );
        if let Slot::Open { library: old, .. } = previous {
            old.close().await;
        }
        if let Slot::Open { library, .. } = &*slot {
            self.remember(library.path());
        }
    }

    fn remember(&self, path: &Path) {
        let Some(text) = path.to_str() else {
            return;
        };
        let result =
            lock(&self.settings).update(|settings| settings.library_path = Some(text.to_owned()));
        if let Err(error) = result {
            log::warn!("library path not saved: {error}");
            *lock(&self.settings_notice) = Some(SettingsNotice::NotSaved);
        }
    }

    /// Moves the open library to a chosen location (section 9).
    pub async fn move_to(
        &self,
        token: &str,
        accept_warning: bool,
        progress: impl Fn(MoveProgress) + Send + Sync + 'static,
    ) -> Result<LibraryStatus, CommandError> {
        let target = self.resolve_token(token, LocationPurpose::Move)?;
        let location = location::classify(&target);
        if location.needs_warning() && !accept_warning {
            return Err(CommandError::new(ErrorCode::LocationNotAccepted));
        }
        let (library, notes) = {
            let mut slot = self.slot.lock().await;
            match std::mem::replace(&mut *slot, Slot::Moving) {
                Slot::Open { library, notes } => (library, notes),
                other => {
                    let busy = matches!(other, Slot::Moving);
                    *slot = other;
                    return Err(CommandError::new(if busy {
                        ErrorCode::LibraryBusy
                    } else {
                        ErrorCode::LibraryNotOpen
                    }));
                }
            }
        };
        let cancel = Arc::new(AtomicBool::new(false));
        *lock(&self.move_cancel) = Some(cancel.clone());

        let result = relocate::move_library(*library, &target, cancel, progress).await;
        *lock(&self.move_cancel) = None;

        let mut slot = self.slot.lock().await;
        match result {
            Ok((library, _report)) => {
                if library.location().needs_warning() {
                    let accepted = Some(library.location().clone());
                    if let Err(error) = library.settings().set::<AcceptedLocation>(&accepted).await
                    {
                        log::warn!("accepted location not stored: {error}");
                    }
                }
                self.remember(library.path());
                *slot = Slot::Open {
                    library: Box::new(library),
                    notes,
                };
                Ok(self.describe(&slot).await)
            }
            Err(failure) => {
                let (library, error) = *failure;
                *slot = match library {
                    Some(library) => Slot::Open {
                        library: Box::new(library),
                        notes,
                    },
                    None => Slot::Closed {
                        error: Some(move_error(&error)),
                    },
                };
                Err(move_error(&error))
            }
        }
    }

    /// Asks a running move to stop. Returns whether one was running.
    pub fn cancel_move(&self) -> bool {
        match &*lock(&self.move_cancel) {
            Some(flag) => {
                flag.store(true, Ordering::SeqCst);
                true
            }
            None => false,
        }
    }

    /// Records that the user accepts the current sync folder or network
    /// drive; the warning is then no longer shown.
    pub async fn accept_location(&self) -> Result<LibraryStatus, CommandError> {
        let slot = self.slot.lock().await;
        let Slot::Open { library, .. } = &*slot else {
            return Err(CommandError::new(ErrorCode::LibraryNotOpen));
        };
        if library.location().needs_warning() {
            library
                .settings()
                .set::<AcceptedLocation>(&Some(library.location().clone()))
                .await
                .map_err(|error| {
                    log::error!("accepting the location failed: {error}");
                    CommandError::new(ErrorCode::Database)
                })?;
        }
        Ok(self.describe(&slot).await)
    }

    /// The library settings the interface shows (ADR 0016).
    pub async fn preferences(&self) -> Result<LibraryPreferences, CommandError> {
        let slot = self.slot.lock().await;
        let Slot::Open { library, .. } = &*slot else {
            return Err(CommandError::new(ErrorCode::LibraryNotOpen));
        };
        read_preferences(library).await
    }

    /// Changes the library settings the interface shows.
    pub async fn set_preferences(
        &self,
        preferences: &LibraryPreferences,
    ) -> Result<LibraryPreferences, CommandError> {
        let slot = self.slot.lock().await;
        let Slot::Open { library, .. } = &*slot else {
            return Err(CommandError::new(ErrorCode::LibraryNotOpen));
        };
        let settings = library.settings();
        settings
            .set::<TrashRetentionDays>(&preferences.trash_retention_days)
            .await
            .map_err(preferences_error)?;
        settings
            .set::<HistoryKeepAllHours>(&preferences.history.keep_all_hours)
            .await
            .map_err(preferences_error)?;
        settings
            .set::<HistoryDailyDays>(&preferences.history.daily_days)
            .await
            .map_err(preferences_error)?;
        settings
            .set::<HistoryWeeklyDays>(&preferences.history.weekly_days)
            .await
            .map_err(preferences_error)?;
        read_preferences(library).await
    }

    /// Closes the open library (at exit).
    pub async fn close(&self) {
        let mut slot = self.slot.lock().await;
        if let Slot::Open { library, .. } =
            std::mem::replace(&mut *slot, Slot::Closed { error: None })
        {
            library.close().await;
        }
    }
}

/// Locks `mutex`; a panic in another thread does not make the data unusable.
/// Library settings shown in Einstellungen › Bibliothek (ADR 0016).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, ts_rs::TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct LibraryPreferences {
    /// Days a document stays in the trash (1–3650).
    pub trash_retention_days: u32,
    /// Retention rules of the history.
    pub history: RetentionPolicy,
}

async fn read_preferences(library: &Library) -> Result<LibraryPreferences, CommandError> {
    let trash_retention_days = library
        .settings()
        .get::<TrashRetentionDays>()
        .await
        .map_err(preferences_error)?;
    let history = library.documents().history().policy().await;
    Ok(LibraryPreferences {
        trash_retention_days,
        history,
    })
}

fn preferences_error(error: crate::services::settings::library::SettingsError) -> CommandError {
    match error {
        crate::services::settings::library::SettingsError::Invalid { key, reason } => {
            CommandError::invalid_input(key, reason)
        }
        crate::services::settings::library::SettingsError::Database(error) => {
            log::error!("library settings: {error}");
            CommandError::new(ErrorCode::Database)
        }
    }
}

/// Backups as the settings show them (ADR 0021, decision 7).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct BackupOverview {
    /// The target as text, for display.
    pub target: String,
    /// `backups/` of the library.
    pub default_target: bool,
    /// The target lies on the drive of the library (section 21: warn).
    pub same_drive: bool,
    pub automatic: bool,
    pub keep: u32,
    pub running: bool,
    /// When the newest backup of this library in the target was written.
    pub last_backup_at: Option<String>,
}

/// Marks a backup as running for as long as it lives.
struct Running {
    activity: Arc<Mutex<backup::control::Activity>>,
}

impl Running {
    fn start(
        activity: &Arc<Mutex<backup::control::Activity>>,
        cancel: Arc<AtomicBool>,
    ) -> Result<Self, CommandError> {
        let mut current = lock(activity);
        if current.running {
            return Err(CommandError::new(ErrorCode::BackupRunning));
        }
        current.running = true;
        current.cancel = Some(cancel);
        Ok(Self {
            activity: activity.clone(),
        })
    }
}

impl Drop for Running {
    fn drop(&mut self) {
        let mut current = lock(&self.activity);
        current.running = false;
        current.cancel = None;
    }
}

pub(crate) fn backup_error(error: backup::BackupError) -> CommandError {
    match error {
        backup::BackupError::Refused(reason) => {
            log::warn!("backup refused: {reason}");
            CommandError::new(ErrorCode::BackupRefused).with_detail("reason", reason)
        }
        backup::BackupError::Newer(what) => {
            CommandError::new(ErrorCode::BackupNewer).with_detail("reason", what)
        }
        backup::BackupError::TargetNotEmpty => CommandError::new(ErrorCode::FolderNotEmpty),
        backup::BackupError::TargetInsideLibrary => {
            CommandError::new(ErrorCode::BackupTargetInsideLibrary)
        }
        backup::BackupError::Cancelled => CommandError::new(ErrorCode::BackupCancelled),
        backup::BackupError::Database(error) => {
            log::error!("backup database error: {error}");
            CommandError::new(ErrorCode::LibraryBackupFailed)
        }
        backup::BackupError::Path(error) => {
            log::error!("backup path error: {error}");
            CommandError::new(ErrorCode::FileSystem)
        }
        backup::BackupError::Io(error) => {
            log::error!("backup file error: {error}");
            CommandError::new(ErrorCode::FileSystem)
        }
    }
}

impl LibrarySession {
    /// End-to-end runs measure the app; a backup in the middle would disturb
    /// them.
    pub fn disable_automatic_backups(&self) {
        self.automatic_backups.store(false, Ordering::Relaxed);
    }

    /// The backup target of `library`: the chosen folder if it still exists,
    /// else its `backups/`.
    fn backup_target_of(&self, library: &Library) -> Result<(PathBuf, bool), CommandError> {
        let id = library.manifest().id.hyphenated().to_string();
        let chosen = lock(&self.settings).get().backup_targets.get(&id).cloned();
        if let Some(path) = chosen.map(PathBuf::from).filter(|path| path.is_dir()) {
            return Ok((path, false));
        }
        library
            .root()
            .create_dir_all(crate::services::library::layout::BACKUPS_DIR)
            .map_err(|error| {
                log::error!("backups folder not created: {error}");
                CommandError::new(ErrorCode::FileSystem)
            })?;
        let path = library
            .root()
            .resolve(crate::services::library::layout::BACKUPS_DIR)
            .map_err(|_| CommandError::new(ErrorCode::FileSystem))?;
        Ok((path, true))
    }

    /// What the settings show about backups.
    pub async fn backup_overview(&self) -> Result<BackupOverview, CommandError> {
        let slot = self.slot.lock().await;
        let library = match &*slot {
            Slot::Open { library, .. } => library,
            Slot::Moving => return Err(CommandError::new(ErrorCode::LibraryBusy)),
            Slot::Closed { .. } => return Err(CommandError::new(ErrorCode::LibraryNotOpen)),
        };
        let (target, default_target) = self.backup_target_of(library)?;
        let settings = library.settings();
        let automatic = settings
            .get::<crate::services::settings::library::BackupAutomatic>()
            .await
            .unwrap_or(true);
        let keep = settings
            .get::<crate::services::settings::library::BackupKeep>()
            .await
            .unwrap_or(7);
        let last_backup_at = backup::control::newest_backup(&target, library.manifest().id)
            .and_then(|time| {
                let millis = time.duration_since(std::time::UNIX_EPOCH).ok()?.as_millis();
                crate::clock::Timestamp::from_unix_millis(i64::try_from(millis).ok()?)
            })
            .map(crate::clock::Timestamp::to_iso);
        Ok(BackupOverview {
            target: display_path(&target),
            default_target,
            same_drive: !default_target && backup::control::same_drive(&target, library.path()),
            automatic,
            keep,
            running: lock(&self.backup).running,
            last_backup_at,
        })
    }

    /// Sets the backup target (`None`: back to `backups/` of the library).
    pub async fn set_backup_target(
        &self,
        target: Option<PathBuf>,
    ) -> Result<BackupOverview, CommandError> {
        {
            let slot = self.slot.lock().await;
            let library = match &*slot {
                Slot::Open { library, .. } => library,
                Slot::Moving => return Err(CommandError::new(ErrorCode::LibraryBusy)),
                Slot::Closed { .. } => return Err(CommandError::new(ErrorCode::LibraryNotOpen)),
            };
            let id = library.manifest().id.hyphenated().to_string();
            let target = match target {
                Some(path) => Some(
                    library
                        .backups()
                        .check_target(&path)
                        .map_err(backup_error)?,
                ),
                None => None,
            };
            let text = match &target {
                Some(path) => Some(
                    path.to_str()
                        .ok_or_else(|| CommandError::new(ErrorCode::PathNotUnicode))?
                        .to_owned(),
                ),
                None => None,
            };
            lock(&self.settings)
                .update(|settings| match text {
                    Some(text) => {
                        settings.backup_targets.insert(id, text);
                    }
                    None => {
                        settings.backup_targets.remove(&id);
                    }
                })
                .map_err(|error| {
                    log::warn!("backup target not saved: {error}");
                    CommandError::new(ErrorCode::SettingsNotSaved)
                })?;
        }
        self.backup_overview().await
    }

    /// Changes the automatic backup and how many are kept.
    pub async fn set_backup_options(
        &self,
        automatic: Option<bool>,
        keep: Option<u32>,
    ) -> Result<BackupOverview, CommandError> {
        {
            let slot = self.slot.lock().await;
            let library = match &*slot {
                Slot::Open { library, .. } => library,
                Slot::Moving => return Err(CommandError::new(ErrorCode::LibraryBusy)),
                Slot::Closed { .. } => return Err(CommandError::new(ErrorCode::LibraryNotOpen)),
            };
            let settings = library.settings();
            let failed = |error: crate::services::settings::library::SettingsError| {
                log::warn!("backup setting not saved: {error}");
                CommandError::new(ErrorCode::SettingsNotSaved)
            };
            if let Some(automatic) = automatic {
                settings
                    .set::<crate::services::settings::library::BackupAutomatic>(&automatic)
                    .await
                    .map_err(failed)?;
            }
            if let Some(keep) = keep {
                settings
                    .set::<crate::services::settings::library::BackupKeep>(&keep)
                    .await
                    .map_err(failed)?;
            }
        }
        self.backup_overview().await
    }

    /// The backups, target and settings of the open library, without
    /// holding the session for the length of a backup.
    async fn backup_parts(
        &self,
    ) -> Result<
        (
            backup::Backups,
            crate::services::settings::library::LibrarySettings,
            PathBuf,
        ),
        CommandError,
    > {
        let slot = self.slot.lock().await;
        let library = match &*slot {
            Slot::Open { library, .. } => library,
            Slot::Moving => return Err(CommandError::new(ErrorCode::LibraryBusy)),
            Slot::Closed { .. } => return Err(CommandError::new(ErrorCode::LibraryNotOpen)),
        };
        let (target, _) = self.backup_target_of(library)?;
        Ok((library.backups(), library.settings(), target))
    }

    /// "Backup erstellen": writes a backup into the target and keeps the
    /// newest ones.
    pub async fn create_backup(
        &self,
        progress: impl Fn(BackupProgress) + Send + Sync + 'static,
    ) -> Result<BackupSummary, CommandError> {
        let (backups, settings, target) = self.backup_parts().await?;
        let cancel = Arc::new(AtomicBool::new(false));
        let _running = Running::start(&self.backup, cancel.clone())?;
        run_backup(&backups, &settings, &target, cancel, progress).await
    }

    /// A new, empty folder for checking a backup: in `tmp/` of the open
    /// library, else next to the app settings. The caller removes it.
    pub async fn scratch_dir(&self) -> Result<PathBuf, CommandError> {
        let base = {
            let slot = self.slot.lock().await;
            match &*slot {
                Slot::Open { library, .. } => {
                    library
                        .root()
                        .create_dir_all(crate::services::library::layout::TMP_DIR)
                        .map_err(|_| CommandError::new(ErrorCode::FileSystem))?;
                    library
                        .root()
                        .resolve(crate::services::library::layout::TMP_DIR)
                        .map_err(|_| CommandError::new(ErrorCode::FileSystem))?
                }
                _ => self.dirs.config.join("tmp"),
            }
        };
        let dir = base.join(format!("check-{}", self.generators.new_id().simple()));
        fs::create_dir_all(&dir).map_err(|error| {
            log::error!("folder for checking not created: {error}");
            CommandError::new(ErrorCode::FileSystem)
        })?;
        Ok(dir)
    }

    /// The new folder a backup is restored into, inside `parent` (stage 5:
    /// never over an existing library).
    pub fn restore_target(&self, parent: &Path) -> Result<PathBuf, CommandError> {
        if !parent.is_dir() {
            return Err(CommandError::new(ErrorCode::FileSystem));
        }
        let target = parent.join(format!(
            "ProjectWrite-Wiederherstellung-{}",
            self.generators.now().to_compact()
        ));
        if target.exists() {
            return Err(CommandError::new(ErrorCode::FolderNotEmpty));
        }
        Ok(target)
    }

    /// Opens a restored library and makes it the open one; the old one stays
    /// where it is (stage 5).
    pub async fn open_restored(&self, path: &Path) -> Result<LibraryStatus, CommandError> {
        let mut slot = self.slot.lock().await;
        if matches!(*slot, Slot::Moving) {
            return Err(CommandError::new(ErrorCode::LibraryBusy));
        }
        let (library, report) = Library::open(
            path,
            OpenOptions { accept_moved: true },
            self.generators.clone(),
        )
        .await
        .map_err(library_error)?;
        self.switch_to(&mut slot, library, &report).await;
        Ok(self.describe(&slot).await)
    }

    /// Stops a running backup between two files.
    pub fn cancel_backup(&self) -> bool {
        match &lock(&self.backup).cancel {
            Some(cancel) => {
                cancel.store(true, Ordering::Relaxed);
                true
            }
            None => false,
        }
    }

    /// Section 21: once a day at start, in the background.
    fn start_automatic_backup(&self, library: &Library) {
        if !self.automatic_backups.load(Ordering::Relaxed) {
            return;
        }
        let Ok((target, _)) = self.backup_target_of(library) else {
            return;
        };
        let backups = library.backups();
        let settings = library.settings();
        let activity = self.backup.clone();
        let id = library.manifest().id;
        // What an interrupted backup left there.
        if let Err(error) = backup::remove_partial(&target, id) {
            log::warn!("partial backups not removed: {error}");
        }
        tokio::spawn(async move {
            tokio::time::sleep(backup::control::AUTOMATIC_DELAY).await;
            let automatic = settings
                .get::<crate::services::settings::library::BackupAutomatic>()
                .await
                .unwrap_or(true);
            if !automatic || !backup::control::due(&target, id, std::time::SystemTime::now()) {
                return;
            }
            let cancel = Arc::new(AtomicBool::new(false));
            let Ok(_running) = Running::start(&activity, cancel.clone()) else {
                return;
            };
            match run_backup(&backups, &settings, &target, cancel, |_| {}).await {
                Ok(summary) => log::info!(
                    "automatic backup: {} documents, {} media",
                    summary.documents,
                    summary.media
                ),
                Err(error) => log::warn!("automatic backup failed: {:?}", error.code),
            }
        });
    }
}

/// End-to-end builds started with `PW_E2E_NO_AUTO_BACKUP` leave out the
/// automatic backup: it would run in the middle of the measurements.
fn automatic_backups_wanted() -> bool {
    !(cfg!(feature = "e2e") && std::env::var_os("PW_E2E_NO_AUTO_BACKUP").is_some())
}

async fn run_backup(
    backups: &backup::Backups,
    settings: &crate::services::settings::library::LibrarySettings,
    target: &Path,
    cancel: Arc<AtomicBool>,
    progress: impl Fn(BackupProgress) + Send + Sync + 'static,
) -> Result<BackupSummary, CommandError> {
    let outcome = backups
        .create(target, cancel, progress)
        .await
        .map_err(backup_error)?;
    let keep = settings
        .get::<crate::services::settings::library::BackupKeep>()
        .await
        .unwrap_or(7);
    if let Err(error) = backup::prune(target, backups.library_id(), keep as usize) {
        log::warn!("old backups not removed: {error}");
    }
    Ok(outcome.summary)
}

pub(crate) fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// What a folder contains.
fn inspect(path: &Path) -> FolderContents {
    match fs::metadata(path) {
        Err(_) => FolderContents::Missing,
        Ok(metadata) if !metadata.is_dir() => FolderContents::Other,
        Ok(_) => {
            if path.join(MANIFEST_FILE).exists() {
                return FolderContents::Library;
            }
            let Ok(entries) = fs::read_dir(path) else {
                return FolderContents::Other;
            };
            let only_junk = entries.filter_map(Result::ok).all(|entry| {
                SYSTEM_JUNK_FILES.contains(&entry.file_name().to_string_lossy().as_ref())
            });
            if only_junk {
                FolderContents::Empty
            } else {
                FolderContents::Other
            }
        }
    }
}

/// Maps a library error to the uniform error format. Details name only
/// library-relative paths or display texts.
pub fn library_error(error: LibraryError) -> CommandError {
    use crate::security::paths::PathError;
    match error {
        LibraryError::NotALibrary => CommandError::new(ErrorCode::LibraryNotFound),
        LibraryError::InvalidManifest(reason) => {
            log::warn!("library.json invalid: {reason}");
            CommandError::new(ErrorCode::LibraryInvalid)
        }
        LibraryError::FormatTooNew { found, supported } => {
            CommandError::new(ErrorCode::LibraryTooNew)
                .with_detail("found", found.to_string())
                .with_detail("supported", supported.to_string())
        }
        LibraryError::Locked => CommandError::new(ErrorCode::LibraryLocked),
        LibraryError::LockUnsupported => CommandError::new(ErrorCode::LibraryLockUnsupported),
        LibraryError::Moved(moved) => {
            CommandError::new(ErrorCode::LibraryMoved).with_detail("path", moved.path)
        }
        LibraryError::UnsafePath(error) => {
            let path = match &error {
                PathError::InvalidComponent { component, .. } => component.clone(),
                other => other.relative().unwrap_or_default().to_owned(),
            };
            CommandError::new(ErrorCode::LibraryUnsafePath).with_detail("path", path)
        }
        LibraryError::DatabaseMissing => CommandError::new(ErrorCode::LibraryDatabaseMissing),
        LibraryError::DatabaseDamaged => CommandError::new(ErrorCode::LibraryDatabaseDamaged),
        LibraryError::BackupFailed(reason) => {
            log::error!("backup before migration failed: {reason}");
            CommandError::new(ErrorCode::LibraryBackupFailed)
        }
        LibraryError::FolderNotEmpty => CommandError::new(ErrorCode::FolderNotEmpty),
        LibraryError::AlreadyALibrary => CommandError::new(ErrorCode::FolderIsLibrary),
        LibraryError::NonUnicodePath => CommandError::new(ErrorCode::PathNotUnicode),
        LibraryError::Database(DatabaseError::SchemaTooNew { found, supported }) => {
            CommandError::new(ErrorCode::SchemaTooNew)
                .with_detail("found", found.to_string())
                .with_detail("supported", supported.to_string())
        }
        LibraryError::Database(error) => {
            log::error!("library database error: {error}");
            CommandError::new(ErrorCode::Database)
        }
        LibraryError::Io(error) => {
            log::error!("library file system error: {error}");
            CommandError::new(ErrorCode::FileSystem)
        }
    }
}

fn move_error(error: &MoveError) -> CommandError {
    match error {
        MoveError::Cancelled => CommandError::new(ErrorCode::MoveCancelled),
        MoveError::TargetInsideLibrary | MoveError::LibraryInsideTarget => {
            CommandError::new(ErrorCode::MoveTargetInvalid)
        }
        MoveError::TargetNotEmpty => CommandError::new(ErrorCode::FolderNotEmpty),
        MoveError::NonUnicodePath => CommandError::new(ErrorCode::PathNotUnicode),
        MoveError::UnsafePath(error) => CommandError::new(ErrorCode::LibraryUnsafePath)
            .with_detail("path", error.relative().unwrap_or_default()),
        MoveError::VerificationFailed(what) => {
            log::error!("move verification failed: {what}");
            CommandError::new(ErrorCode::MoveVerificationFailed)
        }
        MoveError::SwitchFailed(error) => {
            log::error!("moved library cannot be opened: {error}");
            CommandError::new(ErrorCode::MoveSwitchFailed)
        }
        MoveError::Database(error) => {
            log::error!("move database error: {error}");
            CommandError::new(ErrorCode::Database)
        }
        MoveError::Io(error) => {
            log::error!("move file system error: {error}");
            CommandError::new(ErrorCode::FileSystem)
        }
    }
}
