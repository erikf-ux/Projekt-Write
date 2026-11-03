//! `library.json`: library id and format version (section 9).
//!
//! ```json
//! {
//!   "format": "projectwrite-library",
//!   "formatVersion": 1,
//!   "id": "…",
//!   "createdAt": "2026-11-03T10:00:00.000Z",
//!   "createdWith": "0.1.0"
//! }
//! ```
//!
//! * Unknown fields are kept when the file is rewritten (written by a newer
//!   version of the app).
//! * A newer `formatVersion` is rejected before anything is changed.
//! * `movedTo` marks a library that was moved; opening it again needs a
//!   confirmation (the copy at the new location is the one in use).

use std::io;

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use uuid::Uuid;

use super::layout::MANIFEST_FILE;
use crate::clock::Timestamp;
use crate::filesystem::atomic;
use crate::security::paths::{PathError, SafeRoot};

/// Value of the `format` field.
pub const LIBRARY_FORMAT: &str = "projectwrite-library";
/// Highest `formatVersion` this build reads and writes.
pub const LIBRARY_FORMAT_VERSION: u32 = 1;
/// `library.json` is tiny; anything larger is not ours.
const MAX_MANIFEST_BYTES: u64 = 64 * 1024;

/// Where a library was moved to (written into the old copy after a move).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MovedTo {
    /// The new location, as display text.
    pub path: String,
    pub at: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryManifest {
    pub format: String,
    pub format_version: u32,
    pub id: Uuid,
    pub created_at: String,
    pub created_with: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub moved_to: Option<MovedTo>,
    /// Fields written by newer versions, kept unchanged.
    #[serde(flatten)]
    pub other: Map<String, Value>,
}

#[derive(Debug, thiserror::Error)]
pub enum ManifestError {
    #[error("library.json is missing")]
    Missing,
    #[error("library.json is invalid: {0}")]
    Invalid(String),
    #[error("library format {found} is newer than the supported version {supported}")]
    TooNew { found: u64, supported: u32 },
    #[error(transparent)]
    Path(#[from] PathError),
    #[error("cannot read or write library.json: {0}")]
    Io(#[from] io::Error),
}

impl LibraryManifest {
    /// The manifest of a new library.
    pub fn new(id: Uuid, created_at: Timestamp) -> Self {
        Self {
            format: LIBRARY_FORMAT.to_owned(),
            format_version: LIBRARY_FORMAT_VERSION,
            id,
            created_at: created_at.to_iso(),
            created_with: env!("CARGO_PKG_VERSION").to_owned(),
            moved_to: None,
            other: Map::new(),
        }
    }

    /// Reads and validates `library.json` of `root`.
    pub fn read(root: &SafeRoot) -> Result<Self, ManifestError> {
        let path = root.resolve(MANIFEST_FILE)?;
        let data = match atomic::read_limited(&path, MAX_MANIFEST_BYTES) {
            Ok(data) => data,
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                return Err(ManifestError::Missing);
            }
            Err(error) if error.kind() == io::ErrorKind::FileTooLarge => {
                return Err(ManifestError::Invalid("file too large".to_owned()));
            }
            Err(error) => return Err(ManifestError::Io(error)),
        };
        Self::parse(&data)
    }

    /// Parses the bytes of a `library.json`.
    pub fn parse(data: &[u8]) -> Result<Self, ManifestError> {
        let value: Value = serde_json::from_slice(data)
            .map_err(|error| ManifestError::Invalid(format!("not JSON: {error}")))?;
        let object = value
            .as_object()
            .ok_or_else(|| ManifestError::Invalid("not a JSON object".to_owned()))?;
        if object.get("format").and_then(Value::as_str) != Some(LIBRARY_FORMAT) {
            return Err(ManifestError::Invalid("unknown format".to_owned()));
        }
        // The version is checked before the other fields: a newer version may
        // have changed them.
        let version = object
            .get("formatVersion")
            .and_then(Value::as_u64)
            .ok_or_else(|| ManifestError::Invalid("formatVersion missing".to_owned()))?;
        if version > u64::from(LIBRARY_FORMAT_VERSION) {
            return Err(ManifestError::TooNew {
                found: version,
                supported: LIBRARY_FORMAT_VERSION,
            });
        }
        if version == 0 {
            return Err(ManifestError::Invalid("formatVersion 0".to_owned()));
        }
        let manifest: LibraryManifest = serde_json::from_value(value)
            .map_err(|error| ManifestError::Invalid(error.to_string()))?;
        if Timestamp::parse_iso(&manifest.created_at).is_none() {
            return Err(ManifestError::Invalid(
                "createdAt is not a UTC time".to_owned(),
            ));
        }
        Ok(manifest)
    }

    /// Writes `library.json` atomically.
    pub fn write(&self, root: &SafeRoot) -> Result<(), ManifestError> {
        let path = root.resolve(MANIFEST_FILE)?;
        let mut json = serde_json::to_vec_pretty(self)
            .map_err(|error| ManifestError::Invalid(error.to_string()))?;
        json.push(b'\n');
        atomic::write_atomic(&path, &json)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> LibraryManifest {
        LibraryManifest::new(
            Uuid::parse_str("1f8a9a4e-6c39-4d6b-9b7e-6c0f5d2a1b3c").unwrap(),
            Timestamp::parse_iso("2026-11-03T10:00:00Z").unwrap(),
        )
    }

    #[test]
    fn round_trip_keeps_unknown_fields() {
        let mut json = serde_json::to_value(sample()).unwrap();
        json["futureField"] = serde_json::json!({ "nested": [1, 2] });
        let parsed = LibraryManifest::parse(json.to_string().as_bytes()).unwrap();
        assert_eq!(parsed.other["futureField"]["nested"][1], 2);
        let written = serde_json::to_value(&parsed).unwrap();
        assert_eq!(written, json);
    }

    #[test]
    fn newer_versions_are_rejected_even_if_fields_changed() {
        let json = r#"{"format": "projectwrite-library", "formatVersion": 2, "id": 5}"#;
        assert!(matches!(
            LibraryManifest::parse(json.as_bytes()),
            Err(ManifestError::TooNew {
                found: 2,
                supported: 1
            })
        ));
    }

    #[test]
    fn invalid_manifests_are_rejected() {
        for json in [
            "",
            "[]",
            r#"{"format": "other", "formatVersion": 1}"#,
            r#"{"format": "projectwrite-library"}"#,
            r#"{"format": "projectwrite-library", "formatVersion": 0}"#,
            r#"{"format": "projectwrite-library", "formatVersion": "1"}"#,
            r#"{"format": "projectwrite-library", "formatVersion": 1, "id": "not-a-uuid", "createdAt": "2026-11-03T10:00:00Z", "createdWith": "0.1.0"}"#,
            r#"{"format": "projectwrite-library", "formatVersion": 1, "id": "1f8a9a4e-6c39-4d6b-9b7e-6c0f5d2a1b3c", "createdAt": "yesterday", "createdWith": "0.1.0"}"#,
        ] {
            assert!(
                matches!(
                    LibraryManifest::parse(json.as_bytes()),
                    Err(ManifestError::Invalid(_))
                ),
                "{json}"
            );
        }
    }
}
