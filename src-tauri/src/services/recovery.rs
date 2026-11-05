//! Recovery drafts (stage 2, "Absturz"; section 9; ADR 0016, decisions 8
//! and 9).
//!
//! While a tab holds changes that are not saved, the frontend sends its
//! text here at least once per second; Rust writes it atomically to
//! `recovery/<key>.json` in the library. A hard crash therefore loses at
//! most about a second of typing. Saving or discarding the tab removes the
//! draft. After the next start the drafts that still differ from the saved
//! state are offered for restoring.
//!
//! Drafts of external files keep the path of the file inside the draft (it
//! never reaches the frontend). Like the documents, drafts are not
//! encrypted (section 20).

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::filesystem::atomic;
use crate::services::documents::{DocumentFormat, MAX_DOCUMENT_BYTES};
use crate::services::encoding::{LineEnding, TextEncoding};

/// Folder of the drafts inside the library.
pub const RECOVERY_DIR: &str = "recovery";
const DRAFT_FORMAT_VERSION: u32 = 1;
/// Largest draft file read back (text plus a little JSON).
const MAX_DRAFT_BYTES: u64 = MAX_DOCUMENT_BYTES * 2 + 64 * 1024;

/// What a draft belongs to.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum DraftOwner {
    Library { document_id: String },
    External { path: String },
}

/// One draft file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Draft {
    pub format_version: u32,
    pub owner: DraftOwner,
    pub format: DocumentFormat,
    /// External files: how the text is written back.
    pub encoding: Option<TextEncoding>,
    pub line_ending: Option<LineEnding>,
    /// SHA-256 of the saved state the changes started from.
    pub base_sha256: Option<String>,
    pub updated_at: String,
    /// Text with LF line ends; `.pwrite`: the editor content as JSON.
    pub text: String,
}

impl Draft {
    pub fn new(
        owner: DraftOwner,
        format: DocumentFormat,
        updated_at: String,
        text: String,
    ) -> Self {
        Self {
            format_version: DRAFT_FORMAT_VERSION,
            owner,
            format,
            encoding: None,
            line_ending: None,
            base_sha256: None,
            updated_at,
            text,
        }
    }

    /// The file name key of the draft.
    pub fn key(&self) -> String {
        match &self.owner {
            DraftOwner::Library { document_id } => document_id.clone(),
            DraftOwner::External { path } => external_key(Path::new(path)),
        }
    }
}

/// The key of a library document's draft.
pub fn library_key(id: Uuid) -> String {
    id.hyphenated().to_string()
}

/// The key of an external file's draft: `ext-` and the start of the SHA-256
/// of its path.
pub fn external_key(path: &Path) -> String {
    let hash = Sha256::digest(path.to_string_lossy().as_bytes());
    let hex: String = hash[..16]
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    format!("ext-{hex}")
}

/// Whether `key` is a key this module writes (no path parts).
pub fn valid_key(key: &str) -> bool {
    if let Some(hex) = key.strip_prefix("ext-") {
        return hex.len() == 32 && hex.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'));
    }
    Uuid::try_parse(key).is_ok_and(|id| id.hyphenated().to_string() == key)
}

#[derive(Debug, thiserror::Error)]
pub enum RecoveryError {
    #[error("no draft with this key")]
    NotFound,
    #[error("draft is too large")]
    TooLarge,
    #[error("draft is not readable: {0}")]
    Invalid(String),
    #[error("file error: {0}")]
    Io(#[from] io::Error),
}

/// The drafts of one library.
#[derive(Debug, Clone)]
pub struct Drafts {
    dir: PathBuf,
}

impl Drafts {
    /// `dir` is the library's `recovery/` folder (created when needed).
    pub fn new(dir: PathBuf) -> Self {
        Self { dir }
    }

    fn path(&self, key: &str) -> Result<PathBuf, RecoveryError> {
        if !valid_key(key) {
            return Err(RecoveryError::NotFound);
        }
        Ok(self.dir.join(format!("{key}.json")))
    }

    /// Writes a draft atomically, replacing the previous one.
    pub fn write(&self, draft: &Draft) -> Result<(), RecoveryError> {
        if draft.text.len() as u64 > MAX_DOCUMENT_BYTES {
            return Err(RecoveryError::TooLarge);
        }
        fs::create_dir_all(&self.dir)?;
        let data =
            serde_json::to_vec(draft).map_err(|error| RecoveryError::Invalid(error.to_string()))?;
        atomic::write_atomic(&self.path(&draft.key())?, &data)?;
        Ok(())
    }

    pub fn read(&self, key: &str) -> Result<Draft, RecoveryError> {
        let path = self.path(key)?;
        let data = match atomic::read_limited(&path, MAX_DRAFT_BYTES) {
            Ok(data) => data,
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                return Err(RecoveryError::NotFound);
            }
            Err(error) => return Err(error.into()),
        };
        let draft: Draft = serde_json::from_slice(&data)
            .map_err(|error| RecoveryError::Invalid(error.to_string()))?;
        if draft.key() != key {
            return Err(RecoveryError::Invalid("key does not match".to_owned()));
        }
        Ok(draft)
    }

    /// Removes a draft; a missing one is fine.
    pub fn remove(&self, key: &str) -> Result<(), RecoveryError> {
        match fs::remove_file(self.path(key)?) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(error.into()),
        }
    }

    /// All readable drafts with their keys, oldest change first. Files that
    /// are not drafts stay where they are; unreadable drafts are logged.
    pub fn all(&self) -> Vec<Draft> {
        let Ok(entries) = fs::read_dir(&self.dir) else {
            return Vec::new();
        };
        let mut drafts: Vec<Draft> = entries
            .filter_map(Result::ok)
            .filter_map(|entry| {
                let name = entry.file_name().to_str()?.to_owned();
                let key = name.strip_suffix(".json")?;
                if !valid_key(key) {
                    return None;
                }
                match self.read(key) {
                    Ok(draft) => Some(draft),
                    Err(error) => {
                        log::warn!("recovery draft {key} not readable: {error}");
                        None
                    }
                }
            })
            .collect();
        drafts.sort_by(|a, b| a.updated_at.cmp(&b.updated_at));
        drafts
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn drafts() -> (tempfile::TempDir, Drafts) {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("target");
        fs::create_dir_all(&root).unwrap();
        let dir = tempfile::Builder::new()
            .prefix("pw-recovery-")
            .tempdir_in(root)
            .unwrap();
        let drafts = Drafts::new(dir.path().join(RECOVERY_DIR));
        (dir, drafts)
    }

    #[test]
    fn drafts_are_written_read_listed_and_removed() {
        let (_dir, drafts) = drafts();
        assert!(drafts.all().is_empty());
        let id = Uuid::new_v4();
        let library = Draft::new(
            DraftOwner::Library {
                document_id: library_key(id),
            },
            DocumentFormat::Md,
            "2026-11-05T10:00:01.000Z".to_owned(),
            "Ungespeichert\n".to_owned(),
        );
        let mut external = Draft::new(
            DraftOwner::External {
                path: "/home/a/Brief.txt".to_owned(),
            },
            DocumentFormat::Txt,
            "2026-11-05T10:00:00.000Z".to_owned(),
            "Größe\n".to_owned(),
        );
        external.encoding = Some(TextEncoding::Windows1252);
        drafts.write(&library).unwrap();
        drafts.write(&external).unwrap();
        assert_eq!(drafts.read(&library.key()).unwrap(), library);
        assert_eq!(drafts.all(), [external.clone(), library.clone()]);
        assert!(external.key().starts_with("ext-"));
        drafts.remove(&library.key()).unwrap();
        drafts.remove(&library.key()).unwrap();
        assert_eq!(drafts.all(), [external]);
    }

    #[test]
    fn keys_cannot_name_other_files() {
        let (_dir, drafts) = drafts();
        for key in [
            "../library",
            "ext-zz",
            "ext-",
            "a/b",
            "1F8A9A4E-6C39-4D6B-9B7E-6C0F5D2A1B3C",
        ] {
            assert!(!valid_key(key), "{key}");
            assert!(matches!(drafts.read(key), Err(RecoveryError::NotFound)));
        }
        assert!(valid_key(&external_key(Path::new("/x"))));
    }
}
