//! The `.pwrite` format, version 1 (section 10, `docs/pwrite-format.md`).
//!
//! Rust owns the envelope: it checks `format` and `formatVersion`, keeps
//! fields it does not know (also in `media` entries) and writes title and
//! time stamps. `content` is the editor document; the interface checks it
//! against the editor schema. A file of a newer format version opens read
//! only and is never written.

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::services::locale::Locale;

/// The value of `format`.
pub const FORMAT: &str = "pwrite";
/// The format version this build reads and writes.
pub const FORMAT_VERSION: u64 = 1;

/// One medium the document refers to (Phase 8).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MediaEntry {
    pub id: String,
    pub sha256: String,
    pub original_name: String,
    pub mime_type: String,
    #[serde(flatten)]
    pub unknown: Map<String, Value>,
}

/// A `.pwrite` file of version 1.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PwriteFile {
    pub format: String,
    pub format_version: u64,
    pub id: String,
    pub title: String,
    pub created_at: String,
    pub updated_at: String,
    #[serde(default)]
    pub language: String,
    #[serde(default)]
    pub tags: Vec<String>,
    pub content: Value,
    #[serde(default)]
    pub media: Vec<MediaEntry>,
    /// Fields of the envelope this build does not know; written back as read.
    #[serde(flatten)]
    pub unknown: Map<String, Value>,
}

/// A read `.pwrite` file.
#[derive(Debug, Clone, PartialEq)]
pub enum Parsed {
    /// Version 1: can be edited and saved.
    Current(Box<PwriteFile>),
    /// A newer version: only its content is shown, nothing is written.
    Newer { version: u64, content: Value },
}

impl Parsed {
    pub fn content(&self) -> &Value {
        match self {
            Parsed::Current(file) => &file.content,
            Parsed::Newer { content, .. } => content,
        }
    }

    pub fn read_only(&self) -> bool {
        matches!(self, Parsed::Newer { .. })
    }
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum PwriteError {
    #[error("not a .pwrite file: {0}")]
    Invalid(&'static str),
}

/// Reads a `.pwrite` file.
pub fn parse(data: &[u8]) -> Result<Parsed, PwriteError> {
    let value: Value =
        serde_json::from_slice(data).map_err(|_| PwriteError::Invalid("not JSON"))?;
    let object = value
        .as_object()
        .ok_or(PwriteError::Invalid("not an object"))?;
    if object.get("format").and_then(Value::as_str) != Some(FORMAT) {
        return Err(PwriteError::Invalid("format"));
    }
    let version = object
        .get("formatVersion")
        .and_then(Value::as_u64)
        .filter(|version| *version >= 1)
        .ok_or(PwriteError::Invalid("formatVersion"))?;
    if version > FORMAT_VERSION {
        let content = object
            .get("content")
            .cloned()
            .ok_or(PwriteError::Invalid("content"))?;
        check_content(&content)?;
        return Ok(Parsed::Newer { version, content });
    }
    let file: PwriteFile =
        serde_json::from_value(value).map_err(|_| PwriteError::Invalid("envelope"))?;
    check_content(&file.content)?;
    Ok(Parsed::Current(Box::new(file)))
}

/// `content` is an editor document: an object of type `doc` with a list
/// of blocks. The schema of the blocks is checked by the editor.
pub fn check_content(content: &Value) -> Result<(), PwriteError> {
    let object = content.as_object().ok_or(PwriteError::Invalid("content"))?;
    if object.get("type").and_then(Value::as_str) != Some("doc") {
        return Err(PwriteError::Invalid("content type"));
    }
    match object.get("content") {
        None => Ok(()),
        Some(Value::Array(_)) => Ok(()),
        Some(_) => Err(PwriteError::Invalid("content blocks")),
    }
}

/// An empty document with one empty paragraph.
pub fn empty_content() -> Value {
    serde_json::json!({ "type": "doc", "content": [{ "type": "paragraph" }] })
}

/// A new version-1 file.
pub fn new_file(id: &str, title: &str, now: &str, language: &str, content: Value) -> PwriteFile {
    PwriteFile {
        format: FORMAT.to_owned(),
        format_version: FORMAT_VERSION,
        id: id.to_owned(),
        title: title.to_owned(),
        created_at: now.to_owned(),
        updated_at: now.to_owned(),
        language: language.to_owned(),
        tags: Vec::new(),
        content,
        media: Vec::new(),
        unknown: Map::new(),
    }
}

/// The bytes of a file: compact UTF-8 JSON with a final line feed.
pub fn to_bytes(file: &PwriteFile) -> Vec<u8> {
    let mut data = serde_json::to_vec(file).unwrap_or_default();
    data.push(b'\n');
    data
}

/// The language tag a new document gets from the interface language.
pub fn language_tag(locale: Locale) -> &'static str {
    match locale {
        Locale::De => "de-DE",
        Locale::En => "en-US",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Value {
        serde_json::json!({
            "format": "pwrite",
            "formatVersion": 1,
            "id": "0b8f8f4e-6d0e-4f3e-9a3c-2a1d3c4b5e6f",
            "title": "Kapitel 1",
            "createdAt": "2026-11-04T10:00:00Z",
            "updatedAt": "2026-11-04T10:05:00Z",
            "language": "de-DE",
            "tags": ["Beispiel"],
            "content": { "type": "doc", "content": [{ "type": "paragraph", "content": [{ "type": "text", "text": "Hallo" }] }] },
            "media": [{ "id": "m1", "sha256": "00", "originalName": "bild.png", "mimeType": "image/png", "caption": "neu" }],
            "future": { "keep": [1, 2] }
        })
    }

    #[test]
    fn version_1_round_trips_with_unknown_fields() {
        let data = serde_json::to_vec(&sample()).unwrap();
        let Parsed::Current(file) = parse(&data).unwrap() else {
            panic!("version 1 is current");
        };
        assert_eq!(
            file.unknown["future"],
            serde_json::json!({ "keep": [1, 2] })
        );
        assert_eq!(file.media[0].unknown["caption"], "neu");
        let again: Value = serde_json::from_slice(&to_bytes(&file)).unwrap();
        assert_eq!(again, sample());
    }

    #[test]
    fn newer_versions_open_read_only() {
        let mut value = sample();
        value["formatVersion"] = serde_json::json!(2);
        value["renamedField"] = serde_json::json!(true);
        let parsed = parse(&serde_json::to_vec(&value).unwrap()).unwrap();
        assert!(parsed.read_only());
        assert_eq!(parsed.content()["type"], "doc");
    }

    #[test]
    fn other_files_are_refused() {
        for data in [
            &b"not json"[..],
            br#"[1, 2]"#,
            br#"{"format": "other", "formatVersion": 1}"#,
            br#"{"format": "pwrite", "formatVersion": 0}"#,
            br#"{"format": "pwrite", "formatVersion": "1"}"#,
            br#"{"format": "pwrite", "formatVersion": 1, "content": {"type": "doc"}}"#,
            br#"{"format": "pwrite", "formatVersion": 3, "content": {"type": "paragraph"}}"#,
        ] {
            assert!(parse(data).is_err(), "{}", String::from_utf8_lossy(data));
        }
        let mut value = sample();
        value["content"] = serde_json::json!({ "type": "doc", "content": "text" });
        assert!(parse(&serde_json::to_vec(&value).unwrap()).is_err());
    }

    #[test]
    fn new_files_have_the_documented_envelope() {
        let file = new_file(
            "id",
            "Titel",
            "2026-11-04T10:00:00Z",
            language_tag(Locale::De),
            empty_content(),
        );
        let value: Value = serde_json::from_slice(&to_bytes(&file)).unwrap();
        assert_eq!(value["format"], "pwrite");
        assert_eq!(value["formatVersion"], 1);
        assert_eq!(value["language"], "de-DE");
        assert_eq!(value["tags"], serde_json::json!([]));
        assert_eq!(value["media"], serde_json::json!([]));
        assert!(matches!(parse(&to_bytes(&file)), Ok(Parsed::Current(_))));
    }
}
