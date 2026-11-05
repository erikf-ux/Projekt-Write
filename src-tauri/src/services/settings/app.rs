//! App-wide settings (section 9, "Einstellungen"): "Appweite Einstellungen
//! wie Bibliothekspfad, Fensterzustand, Sprache und Tastenkürzel liegen im
//! Konfigurationsordner des Systems, weil sie schon vor dem Öffnen der
//! Bibliothek gebraucht werden."
//!
//! One JSON file, `settings.json`, in the configuration folder of the system
//! (`%APPDATA%\app.projectwrite.desktop` on Windows,
//! `~/.config/app.projectwrite.desktop` on Linux).
//!
//! * Written atomically.
//! * Unknown fields are kept (written by a newer version).
//! * A newer `formatVersion` makes the file read-only for this build.
//! * A damaged file is kept under a new name (`settings.json.damaged-…`) and
//!   the defaults are used; nothing is lost silently.
//!
//! Phase 2 stores the library path; Phase 3 adds the interface settings and
//! the window state ([`super::ui`]); Phase 5 the recently used external
//! files (`recentFiles`, ADR 0015). Interface values are checked one by one
//! when loading: an invalid value gets its default instead of marking the
//! whole file as damaged.

use std::io;
use std::path::{Path, PathBuf};

use serde_json::{Map, Value};

use crate::clock::Timestamp;
use crate::filesystem::atomic;
use crate::services::settings::ui::{UiSettings, WindowState};

pub const APP_SETTINGS_FILE: &str = "settings.json";
pub const APP_SETTINGS_FORMAT_VERSION: u32 = 1;
const MAX_SETTINGS_BYTES: u64 = 256 * 1024;

#[derive(Debug, Clone, PartialEq)]
pub struct AppSettings {
    pub format_version: u32,
    /// Folder of the library that is opened at start.
    pub library_path: Option<String>,
    /// Language, design, zoom, side bar, line width, shortcuts.
    pub ui: UiSettings,
    /// Size and position of the main window.
    pub window: Option<WindowState>,
    /// External files opened last, newest first (at most
    /// [`MAX_RECENT_FILES`]).
    pub recent_files: Vec<RecentFile>,
    /// The tabs of the last session (stage 1, ADR 0016).
    pub session: Option<SavedSession>,
    /// Fields of newer versions, kept unchanged.
    pub other: Map<String, Value>,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            format_version: APP_SETTINGS_FORMAT_VERSION,
            library_path: None,
            ui: UiSettings::default(),
            window: None,
            recent_files: Vec::new(),
            session: None,
            other: Map::new(),
        }
    }
}

/// Most tabs a saved session holds.
pub const MAX_SESSION_TABS: usize = 100;

/// What a tab of the last session showed.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum SavedTabTarget {
    Library { id: String },
    External { path: String },
}

/// One tab of the last session.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SavedTab {
    #[serde(flatten)]
    pub target: SavedTabTarget,
    /// Selection in the editor (ProseMirror positions).
    pub anchor: u32,
    pub head: u32,
    /// `rich`, `source` or `plain`.
    pub view: String,
}

/// The tabs of the last session in one library (stage 1).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SavedSession {
    pub library_id: String,
    pub tabs: Vec<SavedTab>,
    /// Index of the active tab.
    pub active: Option<u32>,
}

/// Most entries kept in `recentFiles`.
pub const MAX_RECENT_FILES: usize = 20;

/// An external file in "Zuletzt verwendet". The path stays in Rust.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecentFile {
    pub path: String,
    /// ISO 8601 (UTC).
    pub opened_at: String,
}

impl RecentFile {
    fn from_value(value: &Value) -> Option<Self> {
        let path = value.get("path")?.as_str()?;
        let opened_at = value.get("openedAt")?.as_str()?;
        (!path.is_empty() && Timestamp::parse_iso(opened_at).is_some()).then(|| Self {
            path: path.to_owned(),
            opened_at: opened_at.to_owned(),
        })
    }

    fn to_value(&self) -> Value {
        serde_json::json!({ "path": self.path, "openedAt": self.opened_at })
    }
}

const KEY_FORMAT_VERSION: &str = "formatVersion";
const KEY_LIBRARY_PATH: &str = "libraryPath";
const KEY_RECENT_FILES: &str = "recentFiles";
const KEY_SESSION: &str = "session";

impl AppSettings {
    /// Reads the settings from the top-level object of the file.
    /// `formatVersion` and `libraryPath` must be valid; interface values that
    /// are not get their defaults.
    fn from_object(mut object: Map<String, Value>, format_version: u32) -> Result<Self, String> {
        object.remove(KEY_FORMAT_VERSION);
        let library_path = match object.remove(KEY_LIBRARY_PATH) {
            None | Some(Value::Null) => None,
            Some(Value::String(path)) => Some(path),
            Some(_) => return Err("libraryPath is not a string".to_owned()),
        };
        let ui = UiSettings::take_from(&mut object);
        let window = WindowState::take_from(&mut object);
        // Invalid entries are dropped one by one, like interface values.
        let recent_files = match object.remove(KEY_RECENT_FILES) {
            Some(Value::Array(entries)) => entries
                .iter()
                .filter_map(RecentFile::from_value)
                .take(MAX_RECENT_FILES)
                .collect(),
            _ => Vec::new(),
        };
        // A session that does not parse is dropped, like an invalid
        // interface value.
        let session = object
            .remove(KEY_SESSION)
            .and_then(|value| serde_json::from_value::<SavedSession>(value).ok())
            .map(|mut session| {
                session.tabs.truncate(MAX_SESSION_TABS);
                session
            });
        Ok(Self {
            format_version,
            library_path,
            ui,
            window,
            recent_files,
            session,
            other: object,
        })
    }

    /// The JSON written to `settings.json`.
    fn to_json(&self) -> Value {
        let mut object = self.other.clone();
        object.insert(
            KEY_FORMAT_VERSION.to_owned(),
            Value::from(self.format_version),
        );
        match &self.library_path {
            Some(path) => {
                object.insert(KEY_LIBRARY_PATH.to_owned(), Value::from(path.as_str()));
            }
            None => {
                object.remove(KEY_LIBRARY_PATH);
            }
        }
        self.ui.put_into(&mut object);
        WindowState::put_into(self.window.as_ref(), &mut object);
        match &self.session {
            Some(session) => {
                if let Ok(value) = serde_json::to_value(session) {
                    object.insert(KEY_SESSION.to_owned(), value);
                }
            }
            None => {
                object.remove(KEY_SESSION);
            }
        }
        if self.recent_files.is_empty() {
            object.remove(KEY_RECENT_FILES);
        } else {
            object.insert(
                KEY_RECENT_FILES.to_owned(),
                Value::Array(self.recent_files.iter().map(RecentFile::to_value).collect()),
            );
        }
        Value::Object(object)
    }
}

/// What happened while loading.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LoadOutcome {
    /// No settings file yet (first start).
    Missing,
    Loaded,
    /// The file was damaged; it was kept as `kept_as` and defaults are used.
    Damaged {
        kept_as: String,
    },
    /// The file comes from a newer version; it is used read-only.
    Newer {
        version: u64,
    },
}

#[derive(Debug, thiserror::Error)]
pub enum AppSettingsError {
    #[error("the settings file belongs to a newer version and is not changed")]
    ReadOnly,
    #[error("cannot write the settings: {0}")]
    Io(#[from] io::Error),
}

/// The loaded settings and where they live.
#[derive(Debug)]
pub struct AppSettingsStore {
    dir: PathBuf,
    current: AppSettings,
    read_only: bool,
}

impl AppSettingsStore {
    /// Loads `settings.json` from `dir` (created on the first write).
    pub fn load(dir: &Path, now: Timestamp) -> (Self, LoadOutcome) {
        let path = dir.join(APP_SETTINGS_FILE);
        let data = match atomic::read_limited(&path, MAX_SETTINGS_BYTES) {
            Ok(data) => data,
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                return (Self::fresh(dir, false), LoadOutcome::Missing);
            }
            Err(error) => {
                log::warn!("app settings unreadable: {error}");
                return Self::keep_damaged(dir, &path, now);
            }
        };
        match parse(&data) {
            Ok(Parsed::Current(settings)) => (
                Self {
                    dir: dir.to_path_buf(),
                    current: settings,
                    read_only: false,
                },
                LoadOutcome::Loaded,
            ),
            Ok(Parsed::Newer { version, settings }) => {
                log::warn!("app settings from a newer version ({version}); used read-only");
                let mut store = Self::fresh(dir, true);
                store.current = settings;
                (store, LoadOutcome::Newer { version })
            }
            Err(reason) => {
                log::warn!("app settings damaged: {reason}");
                Self::keep_damaged(dir, &path, now)
            }
        }
    }

    fn fresh(dir: &Path, read_only: bool) -> Self {
        Self {
            dir: dir.to_path_buf(),
            current: AppSettings::default(),
            read_only,
        }
    }

    /// Renames a damaged file so the next write cannot overwrite it.
    fn keep_damaged(dir: &Path, path: &Path, now: Timestamp) -> (Self, LoadOutcome) {
        let kept_as = format!("{APP_SETTINGS_FILE}.damaged-{}", now.to_compact());
        match std::fs::rename(path, dir.join(&kept_as)) {
            Ok(()) => (Self::fresh(dir, false), LoadOutcome::Damaged { kept_as }),
            Err(error) => {
                // Better not to write at all than to overwrite the file.
                log::error!("cannot keep damaged app settings: {error}");
                (Self::fresh(dir, true), LoadOutcome::Damaged { kept_as })
            }
        }
    }

    pub fn get(&self) -> &AppSettings {
        &self.current
    }

    pub fn is_read_only(&self) -> bool {
        self.read_only
    }

    /// Changes the settings and writes them atomically.
    pub fn update(
        &mut self,
        change: impl FnOnce(&mut AppSettings),
    ) -> Result<(), AppSettingsError> {
        if self.read_only {
            return Err(AppSettingsError::ReadOnly);
        }
        let mut next = self.current.clone();
        change(&mut next);
        std::fs::create_dir_all(&self.dir)?;
        let mut json = serde_json::to_vec_pretty(&next.to_json()).map_err(io::Error::other)?;
        json.push(b'\n');
        atomic::write_atomic(&self.dir.join(APP_SETTINGS_FILE), &json)?;
        self.current = next;
        Ok(())
    }
}

enum Parsed {
    Current(AppSettings),
    /// A file of a newer version: what this build understands of it.
    Newer {
        version: u64,
        settings: AppSettings,
    },
}

fn parse(data: &[u8]) -> Result<Parsed, String> {
    let value: Value = serde_json::from_slice(data).map_err(|e| format!("not JSON: {e}"))?;
    let Value::Object(object) = value else {
        return Err("not a JSON object".to_owned());
    };
    let version = object
        .get(KEY_FORMAT_VERSION)
        .and_then(Value::as_u64)
        .ok_or("formatVersion missing")?;
    if version > u64::from(APP_SETTINGS_FORMAT_VERSION) {
        // Read leniently; the store is read-only and never writes it back.
        let mut settings = AppSettings::from_object(object.clone(), APP_SETTINGS_FORMAT_VERSION)
            .unwrap_or_default();
        if settings.library_path.is_none() {
            settings.library_path = object
                .get(KEY_LIBRARY_PATH)
                .and_then(Value::as_str)
                .map(str::to_owned);
        }
        return Ok(Parsed::Newer { version, settings });
    }
    if version == 0 {
        return Err("formatVersion 0".to_owned());
    }
    let version = u32::try_from(version).map_err(|e| e.to_string())?;
    AppSettings::from_object(object, version).map(Parsed::Current)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dir() -> tempfile::TempDir {
        let base = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("target")
            .join("tmp");
        std::fs::create_dir_all(&base).unwrap();
        tempfile::tempdir_in(base).unwrap()
    }

    fn now() -> Timestamp {
        Timestamp::parse_iso("2026-11-03T12:00:00Z").unwrap()
    }

    #[test]
    fn first_start_has_defaults_and_writes_on_change() {
        let dir = dir();
        let config = dir.path().join("config");
        let (mut store, outcome) = AppSettingsStore::load(&config, now());
        assert_eq!(outcome, LoadOutcome::Missing);
        assert_eq!(store.get(), &AppSettings::default());
        assert!(!config.exists(), "loading must not write");

        store
            .update(|s| s.library_path = Some("/home/a/Dokumente/ProjectWrite".to_owned()))
            .unwrap();
        let (again, outcome) = AppSettingsStore::load(&config, now());
        assert_eq!(outcome, LoadOutcome::Loaded);
        assert_eq!(
            again.get().library_path.as_deref(),
            Some("/home/a/Dokumente/ProjectWrite")
        );
    }

    #[test]
    fn unknown_fields_survive_an_update() {
        let dir = dir();
        std::fs::write(
            dir.path().join(APP_SETTINGS_FILE),
            r#"{"formatVersion": 1, "libraryPath": "/a", "windowState": {"width": 900}}"#,
        )
        .unwrap();
        let (mut store, _) = AppSettingsStore::load(dir.path(), now());
        store
            .update(|s| s.library_path = Some("/b".to_owned()))
            .unwrap();
        let written: Value =
            serde_json::from_slice(&std::fs::read(dir.path().join(APP_SETTINGS_FILE)).unwrap())
                .unwrap();
        assert_eq!(written["libraryPath"], "/b");
        assert_eq!(written["windowState"]["width"], 900);
    }

    #[test]
    fn an_invalid_interface_value_does_not_damage_the_file() {
        let dir = dir();
        let path = dir.path().join(APP_SETTINGS_FILE);
        std::fs::write(
            &path,
            r#"{"formatVersion": 1, "libraryPath": "/a", "zoom": 9, "language": "en"}"#,
        )
        .unwrap();
        let (mut store, outcome) = AppSettingsStore::load(dir.path(), now());
        assert_eq!(outcome, LoadOutcome::Loaded);
        assert_eq!(store.get().library_path.as_deref(), Some("/a"));
        assert_eq!(store.get().ui.zoom, 1.0);
        assert_eq!(
            store.get().ui.language,
            crate::services::settings::ui::LanguageSetting::En
        );

        store.update(|s| s.ui.zoom = 1.5).unwrap();
        let written: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        assert_eq!(written["zoom"], 1.5);
        assert_eq!(written["language"], "en");
        assert_eq!(written["libraryPath"], "/a");
        assert_eq!(written["formatVersion"], 1);
    }

    #[test]
    fn recent_files_drop_invalid_entries() {
        let dir = dir();
        let path = dir.path().join(APP_SETTINGS_FILE);
        std::fs::write(
            &path,
            r#"{"formatVersion": 1, "recentFiles": [
                {"path": "/a/Notiz.md", "openedAt": "2026-11-04T10:00:00Z"},
                {"path": "", "openedAt": "2026-11-04T10:00:00Z"},
                {"path": "/b.txt", "openedAt": "gestern"},
                "/c.txt"
            ]}"#,
        )
        .unwrap();
        let (mut store, outcome) = AppSettingsStore::load(dir.path(), now());
        assert_eq!(outcome, LoadOutcome::Loaded);
        assert_eq!(
            store.get().recent_files,
            [RecentFile {
                path: "/a/Notiz.md".to_owned(),
                opened_at: "2026-11-04T10:00:00Z".to_owned()
            }]
        );
        store.update(|s| s.recent_files.clear()).unwrap();
        let written: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        assert!(written.get("recentFiles").is_none());
    }

    #[test]
    fn damaged_files_are_kept_not_overwritten() {
        let dir = dir();
        let path = dir.path().join(APP_SETTINGS_FILE);
        for content in [
            "{not json",
            "[1, 2]",
            r#"{"libraryPath": "/a"}"#,
            r#"{"formatVersion": 1, "libraryPath": 5}"#,
        ] {
            std::fs::write(&path, content).unwrap();
            let (mut store, outcome) = AppSettingsStore::load(dir.path(), now());
            let LoadOutcome::Damaged { kept_as } = outcome else {
                panic!("{content}: expected Damaged, got {outcome:?}");
            };
            assert_eq!(
                std::fs::read_to_string(dir.path().join(&kept_as)).unwrap(),
                content
            );
            assert_eq!(store.get(), &AppSettings::default());
            store
                .update(|s| s.library_path = Some("/neu".to_owned()))
                .unwrap();
            std::fs::remove_file(dir.path().join(kept_as)).unwrap();
        }
    }

    #[test]
    fn newer_files_are_read_but_never_written() {
        let dir = dir();
        let path = dir.path().join(APP_SETTINGS_FILE);
        let content = r#"{"formatVersion": 7, "libraryPath": "/zukunft", "x": 1}"#;
        std::fs::write(&path, content).unwrap();
        let (mut store, outcome) = AppSettingsStore::load(dir.path(), now());
        assert_eq!(outcome, LoadOutcome::Newer { version: 7 });
        assert_eq!(store.get().library_path.as_deref(), Some("/zukunft"));
        assert!(store.is_read_only());
        assert!(matches!(
            store.update(|s| s.library_path = None),
            Err(AppSettingsError::ReadOnly)
        ));
        assert_eq!(std::fs::read_to_string(&path).unwrap(), content);
    }
}
