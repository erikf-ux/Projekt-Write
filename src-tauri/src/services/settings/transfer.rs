//! Export, import and reset of the settings (ADR 0027, decision 4; format in
//! `docs/settings-format.md`).
//!
//! A `.pwsettings` file is JSON in UTF-8:
//!
//! ```json
//! { "format": "projectwrite-settings", "formatVersion": 1, "exportedAt": "…",
//!   "app": { … }, "library": { … } }
//! ```
//!
//! * `app` holds the interface settings ([`UiSettings`]), without what is
//!   bound to this computer or this session (library path, window state,
//!   session, recent files, backup targets, the LanguageTool folder, whether
//!   the first start asked about the autocorrect).
//! * `library` holds the keys of `application_settings` of the open library
//!   listed in [`library_keys`] (trash, backup, history, Mesh, media); it is
//!   missing when no library is open.
//! * Importing checks every value with the validators of the normal setters
//!   and keeps the checked data in Rust: the frontend only sees the changes
//!   and hands back a token. Applying changes everything or nothing.
//! * Before every import and reset the current state is saved as a
//!   `.pwsettings` file in `settings-backups/` of the configuration folder;
//!   the five newest are kept.

use std::collections::BTreeMap;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use ts_rs::TS;

use crate::clock::Timestamp;
use crate::database::Database;
use crate::database::settings as rows;
use crate::error::{CommandError, ErrorCode};
use crate::filesystem::atomic;
use crate::services::locale::Locale;
use crate::services::session::lock;
use crate::services::settings::library::{
    AcceptedLocation, BackupAutomatic, BackupKeep, HistoryDailyDays, HistoryKeepAllHours,
    HistoryWeeklyDays, LibrarySetting, MediaMaxMegapixels, MeshSuggestionThreshold,
    TrashRetentionDays,
};
use crate::services::settings::ui::UiSettings;
use crate::services::ui::{UiService, UiState};

/// Marker of a settings file.
pub const FORMAT: &str = "projectwrite-settings";
/// Version of the format this build writes and reads.
pub const FORMAT_VERSION: u64 = 1;
/// File extension, without dot.
pub const EXTENSION: &str = "pwsettings";
/// Largest settings file accepted (an export is a few kilobytes).
pub const MAX_FILE_BYTES: u64 = 1024 * 1024;
/// Folder of the automatic backups, inside the configuration folder.
pub const BACKUP_FOLDER: &str = "settings-backups";
/// Backups kept in [`BACKUP_FOLDER`].
pub const BACKUPS_KEPT: usize = 5;
const BACKUP_PREFIX: &str = "settings-";

const KEY_FORMAT: &str = "format";
const KEY_FORMAT_VERSION: &str = "formatVersion";
const KEY_EXPORTED_AT: &str = "exportedAt";
const KEY_APP: &str = "app";
const KEY_LIBRARY: &str = "library";
const KEY_LANGUAGE_CHECK: &str = "languageCheck";
const KEY_AUTOCORRECT_ASKED: &str = "autocorrectAsked";

/// Keys of `settings.json` bound to this computer or this session. In a
/// settings file they are skipped (and named) instead of applied.
const MACHINE_APP_KEYS: [&str; 7] = [
    "libraryPath",
    "window",
    "session",
    "recentFiles",
    "backupTargets",
    "languageTool",
    "formatVersion",
];

/// Which part of the settings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum SettingsArea {
    /// The interface settings in `settings.json`.
    App,
    /// The settings of the open library (`application_settings`).
    Library,
}

/// One value the import changes.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct SettingsChange {
    pub area: SettingsArea,
    /// Key inside the area, such as `zoom` or `trash.retentionDays`.
    pub key: String,
    #[ts(type = "unknown")]
    pub before: Value,
    #[ts(type = "unknown")]
    pub after: Value,
}

/// Why a key of the file is not applied.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum SettingsSkipReason {
    /// This build does not know the key (a newer version wrote it).
    Unknown,
    /// Bound to a computer or a session; never taken from a file.
    Machine,
    /// A library setting while no library is open.
    NoLibrary,
}

/// A key of the file that is not applied.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct SettingsSkippedKey {
    /// Path in the file, such as `app.fontUrl` or `library.trash.retentionDays`.
    pub key: String,
    pub reason: SettingsSkipReason,
}

/// What an import would change; `token` applies it.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct SettingsImportPreview {
    pub token: String,
    /// `exportedAt` of the file, if it is a valid time.
    pub exported_at: Option<String>,
    pub changes: Vec<SettingsChange>,
    pub skipped: Vec<SettingsSkippedKey>,
}

/// Result of an import or a reset: the interface state to apply at once.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct SettingsApplied {
    pub ui: UiState,
    /// Settings of the library changed (views showing them reload).
    pub library_changed: bool,
}

/// What a file holds, checked.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ParsedFile {
    pub exported_at: Option<String>,
    /// Checked and normalised values of `app`, by key.
    pub app: Map<String, Value>,
    /// Checked and normalised values of `library`, by key.
    pub library: Option<BTreeMap<String, Value>>,
    pub skipped: Vec<SettingsSkippedKey>,
}

/// Why a file cannot be imported.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum TransferError {
    #[error("the file is larger than {MAX_FILE_BYTES} bytes")]
    TooLarge,
    #[error("the file is not JSON")]
    NotJson,
    #[error("not a settings file: {0}")]
    Format(&'static str),
    #[error("format version {0} is newer than this build")]
    Newer(u64),
    #[error("invalid value for {key}: {reason}")]
    Invalid { key: String, reason: String },
}

impl From<TransferError> for CommandError {
    fn from(error: TransferError) -> Self {
        match error {
            TransferError::TooLarge => {
                CommandError::new(ErrorCode::SettingsFileInvalid).with_detail("reason", "size")
            }
            TransferError::NotJson => {
                CommandError::new(ErrorCode::SettingsFileInvalid).with_detail("reason", "json")
            }
            TransferError::Format(reason) => {
                CommandError::new(ErrorCode::SettingsFileInvalid).with_detail("reason", reason)
            }
            TransferError::Newer(version) => CommandError::new(ErrorCode::SettingsFileNewer)
                .with_detail("version", version.to_string()),
            TransferError::Invalid { key, reason } => {
                CommandError::new(ErrorCode::SettingsValueInvalid)
                    .with_detail("key", key)
                    .with_detail("reason", reason)
            }
        }
    }
}

// --- The app part ------------------------------------------------------------------------

/// The `app` object of a file: the interface settings without what belongs
/// to this computer.
pub fn app_object(settings: &UiSettings) -> Map<String, Value> {
    let mut object = Map::new();
    settings.put_into(&mut object);
    if let Some(Value::Object(check)) = object.get_mut(KEY_LANGUAGE_CHECK) {
        check.remove(KEY_AUTOCORRECT_ASKED);
    }
    object
}

/// Keys of `app` this build knows.
fn app_keys() -> Vec<String> {
    app_object(&UiSettings::default()).keys().cloned().collect()
}

/// `base` with the values of `overlay`, checked like a change from the
/// interface ([`UiSettings::validate`]). `autocorrectAsked` stays as in
/// `base`.
fn merge_app(base: &UiSettings, overlay: &Map<String, Value>) -> Result<UiSettings, TransferError> {
    let mut object = Map::new();
    base.put_into(&mut object);
    let mut last_key = String::new();
    for (key, value) in overlay {
        let mut value = value.clone();
        if key == KEY_LANGUAGE_CHECK
            && let Value::Object(check) = &mut value
        {
            check.insert(
                KEY_AUTOCORRECT_ASKED.to_owned(),
                Value::Bool(base.language_check.autocorrect_asked),
            );
        }
        object.insert(key.clone(), value);
        last_key.clone_from(key);
    }
    let settings: UiSettings =
        serde_json::from_value(Value::Object(object)).map_err(|_| TransferError::Invalid {
            key: format!("{KEY_APP}.{last_key}"),
            reason: "type".to_owned(),
        })?;
    settings.validate().map_err(|error| {
        let details = error.details.unwrap_or_default();
        TransferError::Invalid {
            key: format!(
                "{KEY_APP}.{}",
                details.get("field").cloned().unwrap_or(last_key)
            ),
            reason: details.get("reason").cloned().unwrap_or_default(),
        }
    })?;
    Ok(settings)
}

/// Checks one value of `app` on its own and returns it normalised.
fn check_app_value(key: &str, value: &Value) -> Result<Value, TransferError> {
    let mut overlay = Map::new();
    overlay.insert(key.to_owned(), value.clone());
    let settings = merge_app(&UiSettings::default(), &overlay).map_err(|error| match error {
        TransferError::Invalid { reason, .. } => TransferError::Invalid {
            key: format!("{KEY_APP}.{key}"),
            reason,
        },
        other => other,
    })?;
    Ok(app_object(&settings)
        .remove(key)
        .expect("a known key is written"))
}

/// The defaults of the interface settings; whether the first start asked
/// about the autocorrect stays.
pub fn reset_app(current: &UiSettings) -> UiSettings {
    let mut settings = UiSettings::default();
    settings.language_check.autocorrect_asked = current.language_check.autocorrect_asked;
    settings
}

fn app_changes(before: &UiSettings, after: &UiSettings) -> Vec<SettingsChange> {
    let before = app_object(before);
    let after = app_object(after);
    after
        .into_iter()
        .filter_map(|(key, value)| {
            let old = before.get(&key).cloned().unwrap_or(Value::Null);
            (old != value).then_some(SettingsChange {
                area: SettingsArea::App,
                key,
                before: old,
                after: value,
            })
        })
        .collect()
}

// --- The library part --------------------------------------------------------------------

/// A library setting without its type: key, check and default.
pub struct LibraryKey {
    pub key: &'static str,
    /// Parses and validates a value; returns it normalised.
    check: fn(&Value) -> Result<Value, &'static str>,
    default: fn() -> Value,
}

fn check_library_value<S: LibrarySetting>(value: &Value) -> Result<Value, &'static str> {
    let parsed: S::Value = serde_json::from_value(value.clone()).map_err(|_| "type")?;
    S::validate(&parsed)?;
    serde_json::to_value(&parsed).map_err(|_| "type")
}

fn library_default<S: LibrarySetting>() -> Value {
    serde_json::to_value(S::default_value()).unwrap_or(Value::Null)
}

fn library_key<S: LibrarySetting>() -> LibraryKey {
    LibraryKey {
        key: S::KEY,
        check: check_library_value::<S>,
        default: library_default::<S>,
    }
}

/// The library settings a file carries: trash, backup, history, Mesh,
/// media. `storage.acceptedLocation` belongs to the place of the library
/// and is never exported.
pub fn library_keys() -> [LibraryKey; 8] {
    [
        library_key::<TrashRetentionDays>(),
        library_key::<BackupAutomatic>(),
        library_key::<BackupKeep>(),
        library_key::<HistoryKeepAllHours>(),
        library_key::<HistoryDailyDays>(),
        library_key::<HistoryWeeklyDays>(),
        library_key::<MeshSuggestionThreshold>(),
        library_key::<MediaMaxMegapixels>(),
    ]
}

/// Library keys bound to the place of the library.
const MACHINE_LIBRARY_KEYS: [&str; 1] = [AcceptedLocation::KEY];

/// The values in effect: the stored one, or the default if there is none
/// or it does not validate (like [`super::library::LibrarySettings::get`]).
pub async fn library_values(database: &Database) -> Result<BTreeMap<String, Value>, sqlx::Error> {
    let mut values = BTreeMap::new();
    for entry in library_keys() {
        let stored = rows::get(database.reader(), entry.key).await?;
        let value = stored
            .and_then(|json| serde_json::from_str::<Value>(&json).ok())
            .and_then(|value| (entry.check)(&value).ok())
            .unwrap_or_else(entry.default);
        values.insert(entry.key.to_owned(), value);
    }
    Ok(values)
}

/// The stored rows of the keys in [`library_keys`] (none: no row).
async fn stored_library_rows(
    database: &Database,
) -> Result<Vec<(&'static str, Option<String>)>, sqlx::Error> {
    let mut stored = Vec::new();
    for entry in library_keys() {
        stored.push((entry.key, rows::get(database.reader(), entry.key).await?));
    }
    Ok(stored)
}

/// Puts rows back as [`stored_library_rows`] read them.
async fn restore_library_rows(
    database: &Database,
    stored: &[(&'static str, Option<String>)],
    now: &str,
) -> Result<(), sqlx::Error> {
    let set: Vec<(&str, String)> = stored
        .iter()
        .filter_map(|(key, value)| value.clone().map(|value| (*key, value)))
        .collect();
    let remove: Vec<&str> = stored
        .iter()
        .filter(|(_, value)| value.is_none())
        .map(|(key, _)| *key)
        .collect();
    rows::replace(database.writer(), &set, &remove, now).await
}

/// Writes checked values in one transaction.
async fn apply_library(
    database: &Database,
    values: &BTreeMap<String, Value>,
    now: &str,
) -> Result<(), sqlx::Error> {
    let set: Vec<(&str, String)> = values
        .iter()
        .map(|(key, value)| (key.as_str(), value.to_string()))
        .collect();
    rows::replace(database.writer(), &set, &[], now).await
}

/// Back to the defaults: the rows of [`library_keys`] are removed, nothing
/// else of the library changes.
async fn reset_library(database: &Database, now: &str) -> Result<(), sqlx::Error> {
    let keys: Vec<&str> = library_keys().iter().map(|entry| entry.key).collect();
    rows::replace(database.writer(), &[], &keys, now).await
}

fn library_changes(
    before: &BTreeMap<String, Value>,
    after: &BTreeMap<String, Value>,
) -> Vec<SettingsChange> {
    after
        .iter()
        .filter_map(|(key, value)| {
            let old = before.get(key).cloned().unwrap_or(Value::Null);
            (&old != value).then(|| SettingsChange {
                area: SettingsArea::Library,
                key: key.clone(),
                before: old,
                after: value.clone(),
            })
        })
        .collect()
}

// --- The file ----------------------------------------------------------------------------

/// The document of a settings file.
pub fn document(
    settings: &UiSettings,
    library: Option<&BTreeMap<String, Value>>,
    now: Timestamp,
) -> Value {
    let mut object = Map::new();
    object.insert(KEY_FORMAT.to_owned(), Value::from(FORMAT));
    object.insert(KEY_FORMAT_VERSION.to_owned(), Value::from(FORMAT_VERSION));
    object.insert(KEY_EXPORTED_AT.to_owned(), Value::from(now.to_iso()));
    object.insert(KEY_APP.to_owned(), Value::Object(app_object(settings)));
    if let Some(library) = library {
        object.insert(
            KEY_LIBRARY.to_owned(),
            Value::Object(library.clone().into_iter().collect()),
        );
    }
    Value::Object(object)
}

/// The bytes of a settings file (pretty JSON with a final line break).
pub fn to_bytes(document: &Value) -> Vec<u8> {
    let mut bytes = serde_json::to_vec_pretty(document).expect("JSON value serializes");
    bytes.push(b'\n');
    bytes
}

/// Reads a settings file, refusing files above [`MAX_FILE_BYTES`].
pub fn read_file(path: &Path) -> Result<Vec<u8>, TransferError> {
    atomic::read_limited(path, MAX_FILE_BYTES).map_err(|error| {
        if error.kind() == io::ErrorKind::FileTooLarge {
            TransferError::TooLarge
        } else {
            log::warn!("settings file unreadable: {error}");
            TransferError::Format("unreadable")
        }
    })
}

/// Parses and checks a settings file. Unknown keys and keys bound to a
/// computer are skipped and listed; any invalid value refuses the whole
/// file.
pub fn parse(data: &[u8]) -> Result<ParsedFile, TransferError> {
    if data.len() as u64 > MAX_FILE_BYTES {
        return Err(TransferError::TooLarge);
    }
    let value: Value = serde_json::from_slice(data).map_err(|_| TransferError::NotJson)?;
    let Value::Object(object) = value else {
        return Err(TransferError::Format("not an object"));
    };
    if object.get(KEY_FORMAT).and_then(Value::as_str) != Some(FORMAT) {
        return Err(TransferError::Format("format"));
    }
    let version = object
        .get(KEY_FORMAT_VERSION)
        .and_then(Value::as_u64)
        .filter(|version| *version > 0)
        .ok_or(TransferError::Format("formatVersion"))?;
    if version > FORMAT_VERSION {
        return Err(TransferError::Newer(version));
    }

    let mut parsed = ParsedFile::default();
    let known_app = app_keys();
    let known_library = library_keys();
    for (key, value) in object {
        match key.as_str() {
            KEY_FORMAT | KEY_FORMAT_VERSION => {}
            KEY_EXPORTED_AT => {
                parsed.exported_at = value
                    .as_str()
                    .and_then(Timestamp::parse_iso)
                    .map(Timestamp::to_iso);
            }
            KEY_APP => {
                let Value::Object(app) = value else {
                    return Err(invalid(KEY_APP, "not an object"));
                };
                for (key, mut value) in app {
                    let path = format!("{KEY_APP}.{key}");
                    if known_app.contains(&key) {
                        if key == KEY_LANGUAGE_CHECK
                            && let Value::Object(check) = &mut value
                            && check.remove(KEY_AUTOCORRECT_ASKED).is_some()
                        {
                            parsed.skipped.push(SettingsSkippedKey {
                                key: format!("{path}.{KEY_AUTOCORRECT_ASKED}"),
                                reason: SettingsSkipReason::Machine,
                            });
                        }
                        let checked = check_app_value(&key, &value)?;
                        parsed.app.insert(key, checked);
                    } else {
                        let reason = if MACHINE_APP_KEYS.contains(&key.as_str()) {
                            SettingsSkipReason::Machine
                        } else {
                            SettingsSkipReason::Unknown
                        };
                        parsed
                            .skipped
                            .push(SettingsSkippedKey { key: path, reason });
                    }
                }
            }
            KEY_LIBRARY => {
                let library = match value {
                    Value::Null => continue,
                    Value::Object(library) => library,
                    _ => return Err(invalid(KEY_LIBRARY, "not an object")),
                };
                let mut values = BTreeMap::new();
                for (key, value) in library {
                    let path = format!("{KEY_LIBRARY}.{key}");
                    match known_library.iter().find(|entry| entry.key == key) {
                        Some(entry) => {
                            let checked =
                                (entry.check)(&value).map_err(|reason| invalid(&path, reason))?;
                            values.insert(key, checked);
                        }
                        None => {
                            let reason = if MACHINE_LIBRARY_KEYS.contains(&key.as_str()) {
                                SettingsSkipReason::Machine
                            } else {
                                SettingsSkipReason::Unknown
                            };
                            parsed
                                .skipped
                                .push(SettingsSkippedKey { key: path, reason });
                        }
                    }
                }
                parsed.library = Some(values);
            }
            _ => parsed.skipped.push(SettingsSkippedKey {
                key,
                reason: SettingsSkipReason::Unknown,
            }),
        }
    }
    Ok(parsed)
}

fn invalid(key: &str, reason: &str) -> TransferError {
    TransferError::Invalid {
        key: key.to_owned(),
        reason: reason.to_owned(),
    }
}

// --- Backups -----------------------------------------------------------------------------

/// `settings-YYYYMMDDTHHMMSSmmmZ.pwsettings`: the names of the backups this
/// module writes (and the only files it ever removes).
pub fn is_backup_name(name: &str) -> bool {
    let Some(stamp) = name
        .strip_prefix(BACKUP_PREFIX)
        .and_then(|rest| rest.strip_suffix(&format!(".{EXTENSION}")))
    else {
        return false;
    };
    let bytes = stamp.as_bytes();
    bytes.len() == 19
        && bytes[..8].iter().all(u8::is_ascii_digit)
        && bytes[8] == b'T'
        && bytes[9..18].iter().all(u8::is_ascii_digit)
        && bytes[18] == b'Z'
}

/// The backups in `folder`, oldest first.
pub fn backups(folder: &Path) -> io::Result<Vec<PathBuf>> {
    let mut found: Vec<PathBuf> = match std::fs::read_dir(folder) {
        Ok(entries) => entries
            .filter_map(Result::ok)
            .filter(|entry| entry.file_type().is_ok_and(|kind| kind.is_file()))
            .filter(|entry| entry.file_name().to_str().is_some_and(is_backup_name))
            .map(|entry| entry.path())
            .collect(),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Vec::new(),
        Err(error) => return Err(error),
    };
    // The compact time stamps sort like the times they stand for.
    found.sort();
    Ok(found)
}

/// Writes `document` as a new backup into `folder` and removes all but the
/// [`BACKUPS_KEPT`] newest backups.
pub fn write_backup(folder: &Path, document: &Value, now: Timestamp) -> io::Result<PathBuf> {
    std::fs::create_dir_all(folder)?;
    let mut time = now;
    let mut path = None;
    for _ in 0..1000 {
        let candidate = folder.join(format!("{BACKUP_PREFIX}{}.{EXTENSION}", time.to_compact()));
        if !candidate.exists() {
            path = Some(candidate);
            break;
        }
        time = Timestamp::from_unix_millis(time.unix_millis() + 1).unwrap_or(time);
    }
    let path = path.ok_or_else(|| io::Error::from(io::ErrorKind::AlreadyExists))?;
    atomic::write_atomic(&path, &to_bytes(document))?;
    let existing = backups(folder)?;
    if existing.len() > BACKUPS_KEPT {
        for old in &existing[..existing.len() - BACKUPS_KEPT] {
            if let Err(error) = std::fs::remove_file(old) {
                log::warn!("old settings backup not removed: {error}");
            }
        }
    }
    Ok(path)
}

// --- The service -------------------------------------------------------------------------

/// What an import, reset or export works on.
pub struct Target<'a> {
    pub ui: &'a UiService,
    /// The configuration folder (backups go to [`BACKUP_FOLDER`] in it).
    pub config: &'a Path,
    /// Database and id of the open library.
    pub library: Option<(&'a Database, &'a str)>,
    pub now: Timestamp,
}

impl Target<'_> {
    async fn library_values(&self) -> Result<Option<BTreeMap<String, Value>>, CommandError> {
        match self.library {
            Some((database, _)) => library_values(database)
                .await
                .map(Some)
                .map_err(database_error),
            None => Ok(None),
        }
    }

    /// The current state as a settings document.
    pub async fn document(&self) -> Result<Value, CommandError> {
        let library = self.library_values().await?;
        Ok(document(
            &self.ui.state().settings,
            library.as_ref(),
            self.now,
        ))
    }

    /// Saves the current state in [`BACKUP_FOLDER`].
    async fn backup(&self) -> Result<PathBuf, CommandError> {
        let document = self.document().await?;
        let path = write_backup(&self.config.join(BACKUP_FOLDER), &document, self.now).map_err(
            |error| {
                log::error!("settings backup not written: {error}");
                CommandError::new(ErrorCode::FileSystem)
            },
        )?;
        log::info!("settings backup written");
        Ok(path)
    }
}

/// An import waiting for "Übernehmen".
struct PendingImport {
    token: String,
    app: Map<String, Value>,
    library: Option<BTreeMap<String, Value>>,
    /// The library the preview was made for.
    library_id: Option<String>,
}

/// The outcome of an import or reset before the interface side effects
/// (zoom, menu language).
#[derive(Debug)]
pub struct Applied {
    pub ui: UiState,
    /// Interface settings before, if they were written.
    pub previous: Option<UiSettings>,
    pub previous_locale: Locale,
    pub library_changed: bool,
}

/// Imports waiting for confirmation (one at a time).
#[derive(Default)]
pub struct SettingsTransfer {
    pending: Mutex<Option<PendingImport>>,
}

impl SettingsTransfer {
    /// Checks a file and remembers it; returns what applying would change.
    pub async fn preview(
        &self,
        target: &Target<'_>,
        data: &[u8],
    ) -> Result<SettingsImportPreview, CommandError> {
        let parsed = parse(data)?;
        let current = target.ui.state();
        if !parsed.app.is_empty() && current.read_only {
            return Err(CommandError::new(ErrorCode::SettingsReadOnly));
        }
        let next = merge_app(&current.settings, &parsed.app)?;
        let mut changes = app_changes(&current.settings, &next);
        let mut skipped = parsed.skipped;
        let library = match (parsed.library, target.library) {
            (Some(values), Some(_)) => {
                let before = target.library_values().await?.unwrap_or_default();
                changes.extend(library_changes(&before, &values));
                Some(values)
            }
            (Some(values), None) => {
                skipped.extend(values.into_keys().map(|key| SettingsSkippedKey {
                    key: format!("{KEY_LIBRARY}.{key}"),
                    reason: SettingsSkipReason::NoLibrary,
                }));
                None
            }
            (None, _) => None,
        };
        changes.sort_by(|a, b| (a.area, &a.key).cmp(&(b.area, &b.key)));
        skipped.sort_by(|a, b| a.key.cmp(&b.key));
        let token = uuid::Uuid::new_v4().simple().to_string();
        *lock(&self.pending) = Some(PendingImport {
            token: token.clone(),
            app: parsed.app,
            library,
            library_id: target.library.map(|(_, id)| id.to_owned()),
        });
        Ok(SettingsImportPreview {
            token,
            exported_at: parsed.exported_at,
            changes,
            skipped,
        })
    }

    /// Applies a previewed import: all values or none.
    pub async fn apply(&self, target: &Target<'_>, token: &str) -> Result<Applied, CommandError> {
        let pending = {
            let mut pending = lock(&self.pending);
            match pending.take() {
                Some(import) if import.token == token => import,
                other => {
                    *pending = other;
                    return Err(CommandError::new(ErrorCode::SettingsImportExpired));
                }
            }
        };
        if pending.library.is_some() && pending.library_id.as_deref() != target.library.map(|l| l.1)
        {
            // Another library was opened since the preview.
            return Err(CommandError::new(ErrorCode::SettingsImportExpired));
        }
        let current = target.ui.state();
        let next = if pending.app.is_empty() {
            None
        } else {
            if current.read_only {
                return Err(CommandError::new(ErrorCode::SettingsReadOnly));
            }
            Some(merge_app(&current.settings, &pending.app)?)
        };
        target.backup().await?;

        let now = target.now.to_iso();
        // The library first, in one transaction; if the interface settings
        // cannot be written afterwards, its rows are put back.
        let mut written_library = None;
        if let (Some(values), Some((database, _))) = (&pending.library, target.library) {
            let stored = stored_library_rows(database)
                .await
                .map_err(database_error)?;
            apply_library(database, values, &now)
                .await
                .map_err(database_error)?;
            written_library = Some((database, stored));
        }
        let previous_locale = current.locale;
        let (ui, previous) = match next {
            Some(next) => match target.ui.update(next) {
                Ok((ui, previous)) => (ui, Some(previous)),
                Err(error) => {
                    if let Some((database, stored)) = &written_library
                        && let Err(undo) = restore_library_rows(database, stored, &now).await
                    {
                        log::error!("library settings not restored after a failed import: {undo}");
                    }
                    return Err(error);
                }
            },
            None => (current, None),
        };
        log::info!("settings imported");
        Ok(Applied {
            ui,
            previous,
            previous_locale,
            library_changed: written_library.is_some(),
        })
    }

    /// Resets one area to its defaults after a backup.
    pub async fn reset(
        &self,
        target: &Target<'_>,
        area: SettingsArea,
    ) -> Result<Applied, CommandError> {
        let current = target.ui.state();
        match area {
            SettingsArea::App => {
                if current.read_only {
                    return Err(CommandError::new(ErrorCode::SettingsReadOnly));
                }
                target.backup().await?;
                let previous_locale = current.locale;
                let (ui, previous) = target.ui.update(reset_app(&current.settings))?;
                log::info!("interface settings reset");
                Ok(Applied {
                    ui,
                    previous: Some(previous),
                    previous_locale,
                    library_changed: false,
                })
            }
            SettingsArea::Library => {
                let Some((database, _)) = target.library else {
                    return Err(CommandError::new(ErrorCode::LibraryNotOpen));
                };
                target.backup().await?;
                reset_library(database, &target.now.to_iso())
                    .await
                    .map_err(database_error)?;
                log::info!("library settings reset");
                Ok(Applied {
                    previous_locale: current.locale,
                    ui: current,
                    previous: None,
                    library_changed: true,
                })
            }
        }
    }
}

fn database_error(error: sqlx::Error) -> CommandError {
    log::error!("library settings: {error}");
    CommandError::new(ErrorCode::Database)
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use serde_json::json;

    use super::*;
    use crate::services::settings::app::AppSettingsStore;
    use crate::services::settings::ui::{LanguageSetting, SidebarSettings};

    struct Fixture {
        _dir: tempfile::TempDir,
        config: PathBuf,
        ui: UiService,
        database: Database,
    }

    impl Fixture {
        fn config(&self) -> PathBuf {
            self.config.clone()
        }

        fn target(&self, library: bool) -> Target<'_> {
            Target {
                ui: &self.ui,
                config: &self.config,
                library: library.then_some((&self.database, LIBRARY_ID)),
                now: now(),
            }
        }

        fn backups(&self) -> Vec<PathBuf> {
            backups(&self.config.join(BACKUP_FOLDER)).unwrap()
        }

        fn settings_file(&self) -> Value {
            serde_json::from_slice(&std::fs::read(self.config.join("settings.json")).unwrap())
                .unwrap()
        }
    }

    const LIBRARY_ID: &str = "0b5a4a8e-3f0e-4d39-9f7e-6c1f2a7a1d11";

    fn now() -> Timestamp {
        Timestamp::parse_iso("2026-11-17T12:00:00Z").unwrap()
    }

    async fn fixture() -> Fixture {
        let base = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("target")
            .join("tmp");
        std::fs::create_dir_all(&base).unwrap();
        let dir = tempfile::tempdir_in(base).unwrap();
        let config = dir.path().join("config");
        let (mut store, _) = AppSettingsStore::load(&config, now());
        store
            .update(|settings| {
                settings.library_path = Some("/home/a/Bibliothek".to_owned());
                settings.window = Some(crate::services::settings::ui::WindowState {
                    x: 10,
                    y: 20,
                    width: 900,
                    height: 700,
                    maximized: false,
                });
                settings
                    .other
                    .insert("languageTool".to_owned(), json!({ "folder": "/opt/lt" }));
            })
            .unwrap();
        let ui = UiService::new(Arc::new(Mutex::new(store)), Locale::De, &config);
        let database = Database::open_in_memory().await.unwrap();
        database.migrate().await.unwrap();
        Fixture {
            _dir: dir,
            config,
            ui,
            database,
        }
    }

    fn changed_ui() -> UiSettings {
        let mut settings = UiSettings {
            language: LanguageSetting::En,
            theme: "dark".to_owned(),
            zoom: 1.25,
            sidebar: SidebarSettings {
                visible: false,
                width: 300,
            },
            ..UiSettings::default()
        };
        settings
            .keybindings
            .insert("palette.show".to_owned(), vec!["Ctrl+K".to_owned()]);
        settings.language_check.autocorrect = true;
        settings
    }

    async fn set_library(database: &Database, key: &str, value: Value) {
        rows::set(
            database.writer(),
            key,
            &value.to_string(),
            "2026-11-17T11:00:00Z",
        )
        .await
        .unwrap();
    }

    async fn stored(database: &Database, key: &str) -> Option<Value> {
        rows::get(database.reader(), key)
            .await
            .unwrap()
            .map(|json| serde_json::from_str(&json).unwrap())
    }

    #[tokio::test]
    async fn export_and_import_round_trip() {
        let source = fixture().await;
        source.ui.update(changed_ui()).unwrap();
        set_library(&source.database, TrashRetentionDays::KEY, json!(12)).await;
        set_library(&source.database, MeshSuggestionThreshold::KEY, json!(0.5)).await;
        let document = source.target(true).document().await.unwrap();
        let bytes = to_bytes(&document);

        assert_eq!(document["format"], FORMAT);
        assert_eq!(document["formatVersion"], 1);
        assert_eq!(document["exportedAt"], "2026-11-17T12:00:00.000Z");
        let app = document["app"].as_object().unwrap();
        for machine in [
            "libraryPath",
            "window",
            "session",
            "recentFiles",
            "languageTool",
        ] {
            assert!(!app.contains_key(machine), "{machine} exported");
        }
        assert!(
            document["app"]["languageCheck"]
                .get("autocorrectAsked")
                .is_none()
        );
        assert_eq!(document["app"]["zoom"], 1.25);
        assert_eq!(document["library"]["trash.retentionDays"], 12);
        assert_eq!(
            document["library"]["backup.keep"], 7,
            "defaults are exported"
        );
        assert!(document["library"].get(AcceptedLocation::KEY).is_none());
        assert_eq!(document["library"].as_object().unwrap().len(), 8);

        let target = fixture().await;
        let transfer = SettingsTransfer::default();
        let preview = transfer
            .preview(&target.target(true), &bytes)
            .await
            .unwrap();
        assert_eq!(
            preview.exported_at.as_deref(),
            Some("2026-11-17T12:00:00.000Z")
        );
        assert!(preview.skipped.is_empty(), "{:?}", preview.skipped);
        let keys: Vec<(SettingsArea, &str)> = preview
            .changes
            .iter()
            .map(|change| (change.area, change.key.as_str()))
            .collect();
        assert_eq!(
            keys,
            [
                (SettingsArea::App, "keybindings"),
                (SettingsArea::App, "language"),
                (SettingsArea::App, "languageCheck"),
                (SettingsArea::App, "sidebar"),
                (SettingsArea::App, "theme"),
                (SettingsArea::App, "zoom"),
                (SettingsArea::Library, "mesh.suggestionThreshold"),
                (SettingsArea::Library, "trash.retentionDays"),
            ]
        );
        let zoom = &preview.changes[5];
        assert_eq!(
            (zoom.before.clone(), zoom.after.clone()),
            (json!(1.0), json!(1.25))
        );
        // Nothing changed before "Übernehmen".
        assert_eq!(target.ui.state().settings, UiSettings::default());
        assert!(target.backups().is_empty());

        let applied = transfer
            .apply(&target.target(true), &preview.token)
            .await
            .unwrap();
        assert_eq!(applied.ui.settings, changed_ui());
        assert_eq!(applied.previous, Some(UiSettings::default()));
        assert!(applied.library_changed);
        assert_eq!(
            stored(&target.database, TrashRetentionDays::KEY).await,
            Some(json!(12))
        );
        assert_eq!(
            library_values(&target.database).await.unwrap(),
            library_values(&source.database).await.unwrap()
        );
        // Paths of this computer stay.
        let file = target.settings_file();
        assert_eq!(file["libraryPath"], "/home/a/Bibliothek");
        assert_eq!(file["window"]["width"], 900);
        assert_eq!(file["languageTool"]["folder"], "/opt/lt");
        assert_eq!(target.backups().len(), 1);

        // A token works once.
        let error = transfer
            .apply(&target.target(true), &preview.token)
            .await
            .unwrap_err();
        assert_eq!(error.code, ErrorCode::SettingsImportExpired);
    }

    #[test]
    fn wrong_marker_and_newer_versions_are_refused() {
        let cases = [
            (b"not json".as_slice(), TransferError::NotJson),
            (b"[1]", TransferError::Format("not an object")),
            (
                br#"{"format": "something-else", "formatVersion": 1}"#,
                TransferError::Format("format"),
            ),
            (
                br#"{"formatVersion": 1, "app": {}}"#,
                TransferError::Format("format"),
            ),
            (
                br#"{"format": "projectwrite-settings", "formatVersion": 0}"#,
                TransferError::Format("formatVersion"),
            ),
            (
                br#"{"format": "projectwrite-settings", "formatVersion": "1"}"#,
                TransferError::Format("formatVersion"),
            ),
            (
                br#"{"format": "projectwrite-settings", "formatVersion": 2, "app": {}}"#,
                TransferError::Newer(2),
            ),
        ];
        for (data, expected) in cases {
            assert_eq!(
                parse(data).unwrap_err(),
                expected,
                "{}",
                String::from_utf8_lossy(data)
            );
        }
        let error = CommandError::from(TransferError::Newer(2));
        assert_eq!(error.code, ErrorCode::SettingsFileNewer);
        let error = CommandError::from(TransferError::Format("format"));
        assert_eq!(error.code, ErrorCode::SettingsFileInvalid);
    }

    #[test]
    fn values_are_checked_with_the_validators_of_the_setters() {
        for (app, key) in [
            (json!({ "zoom": 9 }), "app.zoom"),
            (json!({ "zoom": "groß" }), "app.zoom"),
            (json!({ "theme": "neon" }), "app.theme"),
            (
                json!({ "keybindings": { "palette.show": ["P"] } }),
                "app.keybindings",
            ),
            (
                json!({ "sidebar": { "visible": true, "width": 9000 } }),
                "app.sidebar",
            ),
            (json!({ "sidebar": { "visible": true } }), "app.sidebar"),
            (
                json!({ "languageCheck": { "enabled": true, "rules": true, "autocorrect": false,
                    "defaultLanguage": "Deutsch", "detect": true } }),
                "app.languageCheck",
            ),
        ] {
            let data = json!({ "format": FORMAT, "formatVersion": 1, "app": app });
            match parse(data.to_string().as_bytes()) {
                Err(TransferError::Invalid { key: found, .. }) => assert_eq!(found, key, "{app}"),
                other => panic!("{app}: {other:?}"),
            }
        }
        for (library, key) in [
            (
                json!({ "trash.retentionDays": 0 }),
                "library.trash.retentionDays",
            ),
            (json!({ "backup.keep": "viele" }), "library.backup.keep"),
            (
                json!({ "mesh.suggestionThreshold": 1.5 }),
                "library.mesh.suggestionThreshold",
            ),
        ] {
            let data = json!({ "format": FORMAT, "formatVersion": 1, "library": library });
            match parse(data.to_string().as_bytes()) {
                Err(TransferError::Invalid { key: found, .. }) => assert_eq!(found, key),
                other => panic!("{library}: {other:?}"),
            }
        }
    }

    #[tokio::test]
    async fn one_invalid_value_changes_nothing() {
        let fixture = fixture().await;
        let before_file = std::fs::read(fixture.config().join("settings.json")).unwrap();
        let data = json!({
            "format": FORMAT,
            "formatVersion": 1,
            "app": { "theme": "dark", "zoom": 1.5 },
            "library": { "trash.retentionDays": 10, "backup.keep": 1000 }
        });
        let transfer = SettingsTransfer::default();
        let error = transfer
            .preview(&fixture.target(true), data.to_string().as_bytes())
            .await
            .unwrap_err();
        assert_eq!(error.code, ErrorCode::SettingsValueInvalid);
        assert_eq!(error.details.unwrap()["key"], "library.backup.keep");
        assert_eq!(
            std::fs::read(fixture.config().join("settings.json")).unwrap(),
            before_file
        );
        assert_eq!(
            stored(&fixture.database, TrashRetentionDays::KEY).await,
            None
        );
        assert!(fixture.backups().is_empty());
        assert!(lock(&transfer.pending).is_none());
    }

    #[tokio::test]
    async fn unknown_and_machine_keys_are_skipped_and_named() {
        let fixture = fixture().await;
        let data = json!({
            "format": FORMAT,
            "formatVersion": 1,
            "exportedAt": "gestern",
            "comment": "von Hand",
            "app": {
                "zoom": 0.8,
                "fontUrl": "https://example.com/font.woff",
                "libraryPath": "/anderer/Rechner",
                "window": { "x": 0, "y": 0, "width": 900, "height": 700, "maximized": true },
                "languageCheck": { "enabled": false, "rules": true, "autocorrect": false,
                    "autocorrectAsked": true, "defaultLanguage": null, "detect": true }
            },
            "library": {
                "trash.retentionDays": 40,
                "storage.acceptedLocation": "sync",
                "future.setting": 1
            }
        });
        let transfer = SettingsTransfer::default();
        let preview = transfer
            .preview(&fixture.target(true), data.to_string().as_bytes())
            .await
            .unwrap();
        assert_eq!(preview.exported_at, None);
        let mut skipped: Vec<(String, SettingsSkipReason)> = preview
            .skipped
            .iter()
            .map(|entry| (entry.key.clone(), entry.reason))
            .collect();
        skipped.sort_by(|a, b| a.0.cmp(&b.0));
        assert_eq!(
            skipped,
            [
                ("app.fontUrl".to_owned(), SettingsSkipReason::Unknown),
                (
                    "app.languageCheck.autocorrectAsked".to_owned(),
                    SettingsSkipReason::Machine
                ),
                ("app.libraryPath".to_owned(), SettingsSkipReason::Machine),
                ("app.window".to_owned(), SettingsSkipReason::Machine),
                ("comment".to_owned(), SettingsSkipReason::Unknown),
                (
                    "library.future.setting".to_owned(),
                    SettingsSkipReason::Unknown
                ),
                (
                    "library.storage.acceptedLocation".to_owned(),
                    SettingsSkipReason::Machine
                ),
            ]
        );
        transfer
            .apply(&fixture.target(true), &preview.token)
            .await
            .unwrap();
        let settings = fixture.ui.state().settings;
        assert_eq!(settings.zoom, 0.8);
        assert!(!settings.language_check.enabled);
        assert!(
            !settings.language_check.autocorrect_asked,
            "the first-start question is not taken from a file"
        );
        assert_eq!(fixture.settings_file()["libraryPath"], "/home/a/Bibliothek");
        assert_eq!(stored(&fixture.database, AcceptedLocation::KEY).await, None);
        assert_eq!(stored(&fixture.database, "future.setting").await, None);
        assert_eq!(
            stored(&fixture.database, TrashRetentionDays::KEY).await,
            Some(json!(40))
        );
    }

    #[tokio::test]
    async fn library_values_without_an_open_library_are_skipped() {
        let fixture = fixture().await;
        let data = json!({
            "format": FORMAT,
            "formatVersion": 1,
            "app": { "zoom": 1.5 },
            "library": { "trash.retentionDays": 40 }
        });
        let transfer = SettingsTransfer::default();
        let preview = transfer
            .preview(&fixture.target(false), data.to_string().as_bytes())
            .await
            .unwrap();
        assert_eq!(
            preview.skipped,
            [SettingsSkippedKey {
                key: "library.trash.retentionDays".to_owned(),
                reason: SettingsSkipReason::NoLibrary
            }]
        );
        let applied = transfer
            .apply(&fixture.target(false), &preview.token)
            .await
            .unwrap();
        assert!(!applied.library_changed);
        assert_eq!(applied.ui.settings.zoom, 1.5);
    }

    #[tokio::test]
    async fn a_preview_for_another_library_expires() {
        let fixture = fixture().await;
        let data = json!({
            "format": FORMAT, "formatVersion": 1, "library": { "trash.retentionDays": 40 }
        });
        let transfer = SettingsTransfer::default();
        let preview = transfer
            .preview(&fixture.target(true), data.to_string().as_bytes())
            .await
            .unwrap();
        let error = transfer
            .apply(&fixture.target(false), &preview.token)
            .await
            .unwrap_err();
        assert_eq!(error.code, ErrorCode::SettingsImportExpired);
        assert_eq!(
            stored(&fixture.database, TrashRetentionDays::KEY).await,
            None
        );

        // A new preview replaces the waiting one.
        let first = transfer
            .preview(&fixture.target(true), data.to_string().as_bytes())
            .await
            .unwrap();
        let second = transfer
            .preview(&fixture.target(true), data.to_string().as_bytes())
            .await
            .unwrap();
        let error = transfer
            .apply(&fixture.target(true), &first.token)
            .await
            .unwrap_err();
        assert_eq!(error.code, ErrorCode::SettingsImportExpired);
        transfer
            .apply(&fixture.target(true), &second.token)
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn a_failed_app_write_puts_the_library_back() {
        let fixture = fixture().await;
        set_library(&fixture.database, BackupKeep::KEY, json!(3)).await;
        let data = json!({
            "format": FORMAT,
            "formatVersion": 1,
            "app": { "zoom": 1.5 },
            "library": { "trash.retentionDays": 40, "backup.keep": 9 }
        });
        let transfer = SettingsTransfer::default();
        let preview = transfer
            .preview(&fixture.target(true), data.to_string().as_bytes())
            .await
            .unwrap();
        // settings.json can no longer be written: a folder takes its
        // temporary file's place.
        let blocker = fixture.config().join("settings.json");
        std::fs::remove_file(&blocker).unwrap();
        std::fs::create_dir(&blocker).unwrap();
        std::fs::write(blocker.join("inside"), b"x").unwrap();

        let error = transfer
            .apply(&fixture.target(true), &preview.token)
            .await
            .unwrap_err();
        assert_eq!(error.code, ErrorCode::SettingsNotSaved);
        assert_eq!(
            stored(&fixture.database, TrashRetentionDays::KEY).await,
            None
        );
        assert_eq!(
            stored(&fixture.database, BackupKeep::KEY).await,
            Some(json!(3))
        );
        assert_eq!(fixture.ui.state().settings.zoom, 1.0);
    }

    #[tokio::test]
    async fn backups_are_written_and_only_the_five_newest_kept() {
        let fixture = fixture().await;
        let folder = fixture.config().join(BACKUP_FOLDER);
        std::fs::create_dir_all(&folder).unwrap();
        // Files of the user in the folder are never removed.
        std::fs::write(folder.join("meine.pwsettings"), b"{}").unwrap();
        std::fs::write(folder.join("settings-alt.pwsettings"), b"{}").unwrap();
        let transfer = SettingsTransfer::default();
        let mut written = Vec::new();
        for step in 0..7 {
            let mut target = fixture.target(true);
            target.now = Timestamp::from_unix_millis(now().unix_millis() + step * 1000).unwrap();
            let zoom = if step % 2 == 0 { 1.5 } else { 1.0 };
            let data = json!({ "format": FORMAT, "formatVersion": 1, "app": { "zoom": zoom } });
            let preview = transfer
                .preview(&target, data.to_string().as_bytes())
                .await
                .unwrap();
            transfer.apply(&target, &preview.token).await.unwrap();
            written.push(format!("settings-{}.pwsettings", target.now.to_compact()));
        }
        let kept: Vec<String> = fixture
            .backups()
            .iter()
            .map(|path| path.file_name().unwrap().to_string_lossy().into_owned())
            .collect();
        assert_eq!(kept, written[2..]);
        assert!(folder.join("meine.pwsettings").exists());
        assert!(folder.join("settings-alt.pwsettings").exists());
        // Each backup holds the state before its import (the sixth import
        // set 1.0) and parses again.
        let last = std::fs::read(fixture.backups().last().unwrap()).unwrap();
        let parsed = parse(&last).unwrap();
        assert_eq!(parsed.app["zoom"], json!(1.0));
        assert!(parsed.library.is_some());

        // Two backups in the same millisecond get distinct names (later than
        // the others, so pruning keeps both).
        let document = json!({ "format": FORMAT, "formatVersion": 1 });
        let later = Timestamp::from_unix_millis(now().unix_millis() + 3_600_000).unwrap();
        let first = write_backup(&folder, &document, later).unwrap();
        let second = write_backup(&folder, &document, later).unwrap();
        assert_ne!(first, second);
        assert!(first.exists() && second.exists());
    }

    #[test]
    fn backup_names_are_recognised_exactly() {
        assert!(is_backup_name("settings-20261117T120000000Z.pwsettings"));
        for name in [
            "settings-20261117T120000000Z.pwsettings.tmp",
            "settings-2026111T120000000Z.pwsettings",
            "settings-20261117X120000000Z.pwsettings",
            "einstellungen-20261117T120000000Z.pwsettings",
            "settings-alt.pwsettings",
            "settings.json",
        ] {
            assert!(!is_backup_name(name), "{name}");
        }
    }

    #[tokio::test]
    async fn reset_restores_defaults_and_leaves_the_rest() {
        let fixture = fixture().await;
        let mut settings = changed_ui();
        settings.language_check.autocorrect_asked = true;
        fixture.ui.update(settings).unwrap();
        set_library(&fixture.database, TrashRetentionDays::KEY, json!(12)).await;
        set_library(&fixture.database, HistoryDailyDays::KEY, json!(60)).await;
        set_library(&fixture.database, AcceptedLocation::KEY, json!("sync")).await;
        set_library(&fixture.database, "future.setting", json!(1)).await;
        let transfer = SettingsTransfer::default();

        let applied = transfer
            .reset(&fixture.target(true), SettingsArea::App)
            .await
            .unwrap();
        let mut expected = UiSettings::default();
        expected.language_check.autocorrect_asked = true;
        assert_eq!(applied.ui.settings, expected);
        assert!(!applied.library_changed);
        let file = fixture.settings_file();
        assert_eq!(file["libraryPath"], "/home/a/Bibliothek");
        assert_eq!(file["window"]["height"], 700);
        assert_eq!(file["languageTool"]["folder"], "/opt/lt");
        // The library is untouched by an app reset.
        assert_eq!(
            stored(&fixture.database, TrashRetentionDays::KEY).await,
            Some(json!(12))
        );
        assert_eq!(fixture.backups().len(), 1);
        let backup = parse(&std::fs::read(&fixture.backups()[0]).unwrap()).unwrap();
        assert_eq!(
            backup.app["theme"], "dark",
            "the backup holds the state before"
        );

        let applied = transfer
            .reset(&fixture.target(true), SettingsArea::Library)
            .await
            .unwrap();
        assert!(applied.library_changed);
        assert_eq!(
            stored(&fixture.database, TrashRetentionDays::KEY).await,
            None
        );
        assert_eq!(stored(&fixture.database, HistoryDailyDays::KEY).await, None);
        assert_eq!(
            library_values(&fixture.database).await.unwrap()[TrashRetentionDays::KEY],
            json!(30)
        );
        assert_eq!(
            stored(&fixture.database, AcceptedLocation::KEY).await,
            Some(json!("sync"))
        );
        assert_eq!(
            stored(&fixture.database, "future.setting").await,
            Some(json!(1))
        );
        assert_eq!(fixture.backups().len(), 2);

        let error = transfer
            .reset(&fixture.target(false), SettingsArea::Library)
            .await
            .unwrap_err();
        assert_eq!(error.code, ErrorCode::LibraryNotOpen);
    }

    #[test]
    fn files_above_the_size_limit_are_refused() {
        let base = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("target")
            .join("tmp");
        std::fs::create_dir_all(&base).unwrap();
        let dir = tempfile::tempdir_in(base).unwrap();
        let path = dir.path().join("gross.pwsettings");
        let mut data =
            format!(r#"{{"format": "{FORMAT}", "formatVersion": 1, "x": ""#).into_bytes();
        data.resize(usize::try_from(MAX_FILE_BYTES).unwrap() + 10, b'a');
        data.extend_from_slice(b"\"}");
        std::fs::write(&path, &data).unwrap();
        assert_eq!(read_file(&path).unwrap_err(), TransferError::TooLarge);
        assert_eq!(parse(&data).unwrap_err(), TransferError::TooLarge);
        let error = CommandError::from(TransferError::TooLarge);
        assert_eq!(error.details.unwrap()["reason"], "size");
    }
}
