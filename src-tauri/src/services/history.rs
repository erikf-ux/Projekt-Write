//! Document history (stage 4, section 13; ADR 0016, decisions 4 to 7).
//!
//! A version is the content of the document's file at one moment,
//! compressed with deflate. Versions come from saving (at most one every
//! 10 minutes of editing), closing a tab, converting, restoring, and from
//! "Version speichern". A version equal to the newest one is not stored.
//! The retention rules thin out old versions; named versions and the newest
//! version always stay.

use std::collections::HashSet;
use std::io::{Read, Write};

use flate2::Compression;
use flate2::read::DeflateDecoder;
use flate2::write::DeflateEncoder;
use serde::{Deserialize, Serialize};
use ts_rs::TS;
use uuid::Uuid;

use crate::clock::{Generators, Timestamp};
use crate::database::Database;
use crate::database::history::{self as rows, VersionRow};
use crate::services::documents::{DocumentFormat, sha256_hex};
use crate::services::settings::library::{
    HistoryDailyDays, HistoryKeepAllHours, HistoryWeeklyDays, LibrarySettings,
};

/// Automatic versions while editing: at most one per this interval.
pub const AUTO_INTERVAL_MILLIS: i64 = 10 * 60 * 1000;
/// Longest name of a named version.
pub const MAX_NAME_CHARS: usize = 200;
const COMPRESSION: &str = "deflate";
const HOUR: i64 = 60 * 60 * 1000;
const DAY: i64 = 24 * HOUR;

/// Why a version exists.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum VersionKind {
    /// Every 10 minutes of editing.
    Auto,
    /// The tab was closed.
    Close,
    /// Before the document was converted (old content, old format).
    BeforeConversion,
    /// Before a version or a draft was restored.
    BeforeRestore,
    /// "Version speichern".
    Named,
}

impl VersionKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            VersionKind::Auto => "auto",
            VersionKind::Close => "close",
            VersionKind::BeforeConversion => "beforeConversion",
            VersionKind::BeforeRestore => "beforeRestore",
            VersionKind::Named => "named",
        }
    }

    fn parse(value: &str) -> Option<Self> {
        [
            VersionKind::Auto,
            VersionKind::Close,
            VersionKind::BeforeConversion,
            VersionKind::BeforeRestore,
            VersionKind::Named,
        ]
        .into_iter()
        .find(|kind| kind.as_str() == value)
    }
}

/// A version as the interface lists it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct VersionSummary {
    pub id: String,
    pub document_id: String,
    pub kind: VersionKind,
    pub name: Option<String>,
    pub format: DocumentFormat,
    /// Size of the content, uncompressed.
    #[ts(type = "number")]
    pub size_bytes: u64,
    pub created_at: String,
}

#[derive(Debug, thiserror::Error)]
pub enum HistoryError {
    #[error("version not found")]
    NotFound,
    #[error("stored version is damaged: {0}")]
    Damaged(&'static str),
    #[error("invalid name: {0}")]
    InvalidName(&'static str),
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),
}

/// The retention rules (section 13): all versions of the last
/// `keep_all_hours`, then the newest per day until `daily_days`, then the
/// newest per week until `weekly_days`; older ones go.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct RetentionPolicy {
    pub keep_all_hours: u32,
    pub daily_days: u32,
    pub weekly_days: u32,
}

impl Default for RetentionPolicy {
    fn default() -> Self {
        Self {
            keep_all_hours: 24,
            daily_days: 30,
            weekly_days: 365,
        }
    }
}

/// What the retention rules look at.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RetainedVersion {
    pub id: String,
    pub created_at: Timestamp,
    pub named: bool,
}

/// The versions the rules remove. `versions` in any order. Named versions
/// and the newest version always stay; per day and per week the newest
/// unnamed version represents its period.
pub fn versions_to_remove(
    versions: &[RetainedVersion],
    now: Timestamp,
    policy: RetentionPolicy,
) -> Vec<String> {
    let mut sorted: Vec<&RetainedVersion> = versions.iter().collect();
    sorted.sort_by(|a, b| {
        b.created_at
            .cmp(&a.created_at)
            .then_with(|| b.id.cmp(&a.id))
    });
    let keep_all = i64::from(policy.keep_all_hours) * HOUR;
    let daily = i64::from(policy.daily_days) * DAY;
    let weekly = i64::from(policy.weekly_days) * DAY;
    let mut days = HashSet::new();
    let mut weeks = HashSet::new();
    let mut remove = Vec::new();
    for (index, version) in sorted.iter().enumerate() {
        if index == 0 || version.named {
            continue;
        }
        let millis = version.created_at.unix_millis();
        let age = now.unix_millis() - millis;
        let day = millis.div_euclid(DAY);
        // Weeks from Monday: 1970-01-01 was a Thursday.
        let week = (day + 3).div_euclid(7);
        let keep = if age <= keep_all {
            true
        } else if age <= daily {
            days.insert(day)
        } else if age <= weekly {
            weeks.insert(week)
        } else {
            false
        };
        if !keep {
            remove.push(version.id.clone());
        }
    }
    remove
}

/// The history of the documents of one library.
#[derive(Debug, Clone)]
pub struct History {
    database: Database,
    generators: Generators,
}

impl History {
    pub fn new(database: Database, generators: Generators) -> Self {
        Self {
            database,
            generators,
        }
    }

    /// Stores a version unless it equals the newest one; a named version
    /// then names that newest one. Applies the retention rules to the
    /// document afterwards. Returns the version that holds the content.
    pub async fn record(
        &self,
        document_id: Uuid,
        kind: VersionKind,
        name: Option<&str>,
        format: DocumentFormat,
        content: &[u8],
    ) -> Result<VersionSummary, HistoryError> {
        let name = match (kind, name) {
            (VersionKind::Named, Some(name)) => Some(validate_name(name)?),
            (VersionKind::Named, None) => return Err(HistoryError::InvalidName("missing")),
            _ => None,
        };
        let document = document_id.hyphenated().to_string();
        let sha = sha256_hex(content);
        if let Some(latest) = rows::latest(self.database.reader(), &document).await?
            && latest.content_sha256 == sha
            && latest.format == format.as_str()
        {
            if let Some(name) = &name {
                rows::set_name(self.database.writer(), &latest.id, name).await?;
                return self.summary(&latest.id).await;
            }
            return summary_of(latest);
        }
        let compressed = compress(content);
        let row = VersionRow {
            id: self.generators.new_id().hyphenated().to_string(),
            document_id: document,
            kind: kind.as_str().to_owned(),
            name,
            format: format.as_str().to_owned(),
            size_bytes: i64::try_from(content.len()).unwrap_or(i64::MAX),
            content_sha256: sha,
            created_at: self.generators.now().to_iso(),
        };
        rows::insert(self.database.writer(), &row, &compressed, COMPRESSION).await?;
        self.apply_retention(document_id).await?;
        summary_of(row)
    }

    /// An automatic version, if the newest version is older than
    /// [`AUTO_INTERVAL_MILLIS`] (or there is none).
    pub async fn record_auto_if_due(
        &self,
        document_id: Uuid,
        format: DocumentFormat,
        content: &[u8],
    ) -> Result<Option<VersionSummary>, HistoryError> {
        let latest = rows::latest(
            self.database.reader(),
            &document_id.hyphenated().to_string(),
        )
        .await?;
        let due = match latest {
            None => true,
            Some(latest) => Timestamp::parse_iso(&latest.created_at).is_none_or(|created| {
                self.generators.now().unix_millis() - created.unix_millis() >= AUTO_INTERVAL_MILLIS
            }),
        };
        if !due {
            return Ok(None);
        }
        self.record(document_id, VersionKind::Auto, None, format, content)
            .await
            .map(Some)
    }

    /// The versions of a document, newest first.
    pub async fn list(&self, document_id: Uuid) -> Result<Vec<VersionSummary>, HistoryError> {
        rows::list(
            self.database.reader(),
            &document_id.hyphenated().to_string(),
        )
        .await?
        .into_iter()
        .map(summary_of)
        .collect()
    }

    /// A version and its content, checked against its checksum.
    pub async fn content(
        &self,
        version_id: Uuid,
    ) -> Result<(VersionSummary, Vec<u8>), HistoryError> {
        let (row, stored, compression) =
            rows::get(self.database.reader(), &version_id.hyphenated().to_string())
                .await?
                .ok_or(HistoryError::NotFound)?;
        if compression != COMPRESSION {
            return Err(HistoryError::Damaged("unknown compression"));
        }
        let content = decompress(&stored)?;
        if sha256_hex(&content) != row.content_sha256 {
            return Err(HistoryError::Damaged("checksum"));
        }
        Ok((summary_of(row)?, content))
    }

    /// Applies the retention rules to the versions of one document.
    pub async fn apply_retention(&self, document_id: Uuid) -> Result<usize, HistoryError> {
        let policy = self.policy().await;
        let versions: Vec<RetainedVersion> = rows::list(
            self.database.reader(),
            &document_id.hyphenated().to_string(),
        )
        .await?
        .into_iter()
        .filter_map(|row| {
            Some(RetainedVersion {
                created_at: Timestamp::parse_iso(&row.created_at)?,
                named: row.kind == VersionKind::Named.as_str(),
                id: row.id,
            })
        })
        .collect();
        let remove = versions_to_remove(&versions, self.generators.now(), policy);
        if remove.is_empty() {
            return Ok(0);
        }
        Ok(usize::try_from(rows::delete(self.database.writer(), &remove).await?).unwrap_or(0))
    }

    /// Applies the retention rules to every document (after opening).
    pub async fn apply_retention_all(&self) -> Result<usize, HistoryError> {
        let mut removed = 0;
        for document in rows::documents_with_versions(self.database.reader()).await? {
            if let Ok(id) = Uuid::try_parse(&document) {
                removed += self.apply_retention(id).await?;
            }
        }
        Ok(removed)
    }

    /// The rules from the library settings.
    pub async fn policy(&self) -> RetentionPolicy {
        let settings = LibrarySettings::new(self.database.clone(), self.generators.clone());
        let defaults = RetentionPolicy::default();
        RetentionPolicy {
            keep_all_hours: settings
                .get::<HistoryKeepAllHours>()
                .await
                .unwrap_or(defaults.keep_all_hours),
            daily_days: settings
                .get::<HistoryDailyDays>()
                .await
                .unwrap_or(defaults.daily_days),
            weekly_days: settings
                .get::<HistoryWeeklyDays>()
                .await
                .unwrap_or(defaults.weekly_days),
        }
    }

    async fn summary(&self, id: &str) -> Result<VersionSummary, HistoryError> {
        let (row, _, _) = rows::get(self.database.reader(), id)
            .await?
            .ok_or(HistoryError::NotFound)?;
        summary_of(row)
    }
}

/// Trims a version name and checks length and characters.
pub fn validate_name(name: &str) -> Result<String, HistoryError> {
    let name = name.trim();
    if name.is_empty() {
        return Err(HistoryError::InvalidName("empty"));
    }
    if name.chars().count() > MAX_NAME_CHARS {
        return Err(HistoryError::InvalidName("too long"));
    }
    if name.chars().any(char::is_control) {
        return Err(HistoryError::InvalidName("control character"));
    }
    Ok(name.to_owned())
}

fn summary_of(row: VersionRow) -> Result<VersionSummary, HistoryError> {
    Ok(VersionSummary {
        kind: VersionKind::parse(&row.kind).ok_or(HistoryError::Damaged("kind"))?,
        format: DocumentFormat::parse(&row.format).ok_or(HistoryError::Damaged("format"))?,
        size_bytes: u64::try_from(row.size_bytes).unwrap_or(0),
        id: row.id,
        document_id: row.document_id,
        name: row.name,
        created_at: row.created_at,
    })
}

fn compress(content: &[u8]) -> Vec<u8> {
    let mut encoder = DeflateEncoder::new(Vec::new(), Compression::default());
    // Writing into a Vec cannot fail.
    let _ = encoder.write_all(content);
    encoder.finish().unwrap_or_default()
}

fn decompress(stored: &[u8]) -> Result<Vec<u8>, HistoryError> {
    let mut content = Vec::new();
    DeflateDecoder::new(stored)
        .read_to_end(&mut content)
        .map_err(|_| HistoryError::Damaged("deflate"))?;
    Ok(content)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(text: &str) -> Timestamp {
        Timestamp::parse_iso(text).unwrap()
    }

    fn version(id: &str, created: &str, named: bool) -> RetainedVersion {
        RetainedVersion {
            id: id.to_owned(),
            created_at: at(created),
            named,
        }
    }

    #[test]
    fn retention_keeps_a_day_all_then_one_per_day_then_one_per_week() {
        let now = at("2026-11-05T12:00:00.000Z");
        let versions = [
            // Last 24 hours: all stay.
            version("h1", "2026-11-05T11:50:00.000Z", false),
            version("h2", "2026-11-05T09:00:00.000Z", false),
            version("h3", "2026-11-04T12:30:00.000Z", false),
            // Older than a day: the newest per day.
            version("d1", "2026-11-03T18:00:00.000Z", false),
            version("d2", "2026-11-03T08:00:00.000Z", false),
            version("d3", "2026-10-20T10:00:00.000Z", false),
            version("d4", "2026-10-20T09:00:00.000Z", false),
            // Older than 30 days: the newest per week (weeks from Monday).
            version("w1", "2026-09-24T10:00:00.000Z", false), // Thursday
            version("w2", "2026-09-22T10:00:00.000Z", false), // Tuesday, same week
            version("w3", "2026-09-20T10:00:00.000Z", false), // Sunday, week before
            // A named version stays at any age.
            version("n1", "2026-09-23T10:00:00.000Z", true),
            // Older than a year: goes.
            version("old", "2025-10-01T10:00:00.000Z", false),
            version("old-named", "2024-01-01T10:00:00.000Z", true),
        ];
        let mut removed = versions_to_remove(&versions, now, RetentionPolicy::default());
        removed.sort();
        assert_eq!(removed, ["d2", "d4", "old", "w2"]);
    }

    #[test]
    fn the_newest_version_always_stays_and_rules_are_adjustable() {
        let now = at("2026-11-05T12:00:00.000Z");
        let versions = [
            version("a", "2020-01-01T00:00:00.000Z", false),
            version("b", "2019-01-01T00:00:00.000Z", false),
        ];
        assert_eq!(
            versions_to_remove(&versions, now, RetentionPolicy::default()),
            ["b"]
        );
        let strict = RetentionPolicy {
            keep_all_hours: 1,
            daily_days: 1,
            weekly_days: 1,
        };
        let recent = [
            version("x", "2026-11-05T11:30:00.000Z", false),
            version("y", "2026-11-05T10:00:00.000Z", false),
            version("z", "2026-11-05T09:00:00.000Z", false),
        ];
        // y and z are older than an hour, same day: only the newer of them stays.
        assert_eq!(versions_to_remove(&recent, now, strict), ["z"]);
    }

    #[test]
    fn content_survives_compression() {
        let text = "Größe ".repeat(5000);
        let compressed = compress(text.as_bytes());
        assert!(compressed.len() < text.len() / 10);
        assert_eq!(decompress(&compressed).unwrap(), text.as_bytes());
        assert!(matches!(
            decompress(b"\xFF\xFF\xFF"),
            Err(HistoryError::Damaged(_))
        ));
    }

    #[test]
    fn names_are_checked() {
        assert_eq!(validate_name("  Erste Fassung ").unwrap(), "Erste Fassung");
        assert!(validate_name("   ").is_err());
        assert!(validate_name("a\nb").is_err());
        assert!(validate_name(&"x".repeat(201)).is_err());
    }
}
