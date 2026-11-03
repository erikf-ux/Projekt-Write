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
//! Phase 2 stores the library path. Window state, language and shortcuts
//! follow with the interface (Phase 3).

use std::io;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::clock::Timestamp;
use crate::filesystem::atomic;

pub const APP_SETTINGS_FILE: &str = "settings.json";
pub const APP_SETTINGS_FORMAT_VERSION: u32 = 1;
const MAX_SETTINGS_BYTES: u64 = 256 * 1024;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppSettings {
    pub format_version: u32,
    /// Folder of the library that is opened at start.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub library_path: Option<String>,
    /// Fields of newer versions, kept unchanged.
    #[serde(flatten)]
    pub other: Map<String, Value>,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            format_version: APP_SETTINGS_FORMAT_VERSION,
            library_path: None,
            other: Map::new(),
        }
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
            Ok(Parsed::Newer {
                version,
                library_path,
            }) => {
                log::warn!("app settings from a newer version ({version}); used read-only");
                let mut store = Self::fresh(dir, true);
                store.current.library_path = library_path;
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
        let mut json = serde_json::to_vec_pretty(&next).map_err(io::Error::other)?;
        json.push(b'\n');
        atomic::write_atomic(&self.dir.join(APP_SETTINGS_FILE), &json)?;
        self.current = next;
        Ok(())
    }
}

enum Parsed {
    Current(AppSettings),
    Newer {
        version: u64,
        library_path: Option<String>,
    },
}

fn parse(data: &[u8]) -> Result<Parsed, String> {
    let value: Value = serde_json::from_slice(data).map_err(|e| format!("not JSON: {e}"))?;
    let object = value.as_object().ok_or("not a JSON object")?;
    let version = object
        .get("formatVersion")
        .and_then(Value::as_u64)
        .ok_or("formatVersion missing")?;
    if version > u64::from(APP_SETTINGS_FORMAT_VERSION) {
        let library_path = object
            .get("libraryPath")
            .and_then(Value::as_str)
            .map(str::to_owned);
        return Ok(Parsed::Newer {
            version,
            library_path,
        });
    }
    if version == 0 {
        return Err("formatVersion 0".to_owned());
    }
    serde_json::from_value(value)
        .map(Parsed::Current)
        .map_err(|e| e.to_string())
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
