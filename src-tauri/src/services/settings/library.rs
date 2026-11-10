//! Library-wide settings (section 9): "Bibliotheksweite Einstellungen liegen
//! in der Tabelle `application_settings`."
//!
//! Each setting is a type implementing [`LibrarySetting`]: key, value type,
//! default and validation in one place. A setting without a row has its
//! default. Rows of keys this build does not know (written by a newer
//! version) are never touched.

use serde::Serialize;
use serde::de::DeserializeOwned;

use crate::clock::Generators;
use crate::database::Database;
use crate::database::settings as rows;
use crate::filesystem::location::StorageLocation;

/// A typed library setting.
pub trait LibrarySetting {
    /// Row key, `area.name`.
    const KEY: &'static str;
    type Value: Serialize + DeserializeOwned;
    fn default_value() -> Self::Value;
    /// Semantic checks beyond the type. The reason is a short English text.
    fn validate(_value: &Self::Value) -> Result<(), &'static str> {
        Ok(())
    }
}

/// The location the user accepted although section 9 warns against it
/// (sync folder or network drive). While the library stays at a location of
/// this kind, the warning is not repeated as a prompt.
pub struct AcceptedLocation;

impl LibrarySetting for AcceptedLocation {
    const KEY: &'static str = "storage.acceptedLocation";
    type Value = Option<StorageLocation>;

    fn default_value() -> Self::Value {
        None
    }

    fn validate(value: &Self::Value) -> Result<(), &'static str> {
        match value {
            Some(StorageLocation::Local) => Err("a local location needs no acceptance"),
            _ => Ok(()),
        }
    }
}

/// Days a document stays in the trash before it is removed for good
/// (section 4, "Papierkorb": 30 days, adjustable).
pub struct TrashRetentionDays;

impl LibrarySetting for TrashRetentionDays {
    const KEY: &'static str = "trash.retentionDays";
    type Value = u32;

    fn default_value() -> Self::Value {
        30
    }

    fn validate(value: &Self::Value) -> Result<(), &'static str> {
        if (1..=3650).contains(value) {
            Ok(())
        } else {
            Err("between 1 and 3650 days")
        }
    }
}

/// Largest image accepted, in megapixels (section 17: "Grenzwerte schützen
/// vor Bildbomben, Startwert 100 Megapixel"; ADR 0019, decision 4).
pub struct MediaMaxMegapixels;

impl LibrarySetting for MediaMaxMegapixels {
    const KEY: &'static str = "media.maxMegapixels";
    type Value = u32;

    fn default_value() -> Self::Value {
        100
    }

    fn validate(value: &Self::Value) -> Result<(), &'static str> {
        if (1..=1000).contains(value) {
            Ok(())
        } else {
            Err("between 1 and 1000 megapixels")
        }
    }
}

/// History: every version of the last hours stays (section 13: 24 hours).
pub struct HistoryKeepAllHours;

impl LibrarySetting for HistoryKeepAllHours {
    const KEY: &'static str = "history.keepAllHours";
    type Value = u32;

    fn default_value() -> Self::Value {
        24
    }

    fn validate(value: &Self::Value) -> Result<(), &'static str> {
        if (1..=8760).contains(value) {
            Ok(())
        } else {
            Err("between 1 and 8760 hours")
        }
    }
}

/// History: up to this age one version per day stays (section 13: 30 days).
pub struct HistoryDailyDays;

impl LibrarySetting for HistoryDailyDays {
    const KEY: &'static str = "history.dailyDays";
    type Value = u32;

    fn default_value() -> Self::Value {
        30
    }

    fn validate(value: &Self::Value) -> Result<(), &'static str> {
        if (1..=3650).contains(value) {
            Ok(())
        } else {
            Err("between 1 and 3650 days")
        }
    }
}

/// History: up to this age one version per week stays (section 13: a year).
pub struct HistoryWeeklyDays;

impl LibrarySetting for HistoryWeeklyDays {
    const KEY: &'static str = "history.weeklyDays";
    type Value = u32;

    fn default_value() -> Self::Value {
        365
    }

    fn validate(value: &Self::Value) -> Result<(), &'static str> {
        if (1..=36500).contains(value) {
            Ok(())
        } else {
            Err("between 1 and 36500 days")
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum SettingsError {
    #[error("invalid value for {key}: {reason}")]
    Invalid { key: &'static str, reason: String },
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),
}

/// Access to the settings of one library.
#[derive(Debug, Clone)]
pub struct LibrarySettings {
    database: Database,
    generators: Generators,
}

impl LibrarySettings {
    pub fn new(database: Database, generators: Generators) -> Self {
        Self {
            database,
            generators,
        }
    }

    /// The stored value, or the default. A stored value that no longer parses
    /// or validates yields the default and stays in the table unchanged.
    pub async fn get<S: LibrarySetting>(&self) -> Result<S::Value, SettingsError> {
        let Some(json) = rows::get(self.database.reader(), S::KEY).await? else {
            return Ok(S::default_value());
        };
        match serde_json::from_str::<S::Value>(&json) {
            Ok(value) if S::validate(&value).is_ok() => Ok(value),
            Ok(_) | Err(_) => {
                log::warn!(
                    "library setting {} has an unusable value; using the default",
                    S::KEY
                );
                Ok(S::default_value())
            }
        }
    }

    pub async fn set<S: LibrarySetting>(&self, value: &S::Value) -> Result<(), SettingsError> {
        S::validate(value).map_err(|reason| SettingsError::Invalid {
            key: S::KEY,
            reason: reason.to_owned(),
        })?;
        let json = serde_json::to_string(value).map_err(|error| SettingsError::Invalid {
            key: S::KEY,
            reason: error.to_string(),
        })?;
        let now = self.generators.now().to_iso();
        rows::set(self.database.writer(), S::KEY, &json, &now).await?;
        Ok(())
    }

    /// Back to the default (the row is removed).
    pub async fn reset<S: LibrarySetting>(&self) -> Result<(), SettingsError> {
        rows::remove(self.database.writer(), S::KEY).await?;
        Ok(())
    }
}
