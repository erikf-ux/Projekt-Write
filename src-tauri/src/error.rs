//! Uniform error format for every command (section 8):
//! `{ code, messageKey, details? }`.
//!
//! Internal errors are logged on the Rust side and mapped to a generic
//! [`CommandError`]. Stack traces, SQL messages and absolute paths never reach
//! the frontend.

use std::collections::BTreeMap;

use serde::Serialize;
use ts_rs::TS;

/// Maximum length of a single `details` value. Keeps responses small and
/// prevents large inputs from being echoed back.
const MAX_DETAIL_LEN: usize = 200;

/// Stable, machine-readable error codes shared with the frontend.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[ts(export)]
pub enum ErrorCode {
    /// The request was malformed or failed validation in Rust.
    InvalidInput,
    /// A database operation failed.
    Database,
    /// The database was written by a newer version of Project Write.
    SchemaTooNew,
    /// An unexpected internal error occurred.
    Internal,
    /// Tauri rejected the call before it reached a command (for example
    /// because the capability does not allow it). Produced by the frontend
    /// wrapper in `src/services/ipc.ts`, never by a command itself.
    IpcRejected,
    /// Reading or writing a file failed (disk full, no permission …).
    FileSystem,
    /// The command needs an open library.
    LibraryNotOpen,
    /// The folder does not contain a library.
    LibraryNotFound,
    /// `library.json` is damaged.
    LibraryInvalid,
    /// The library was written by a newer version of Project Write.
    LibraryTooNew,
    /// The library is open in another instance.
    LibraryLocked,
    /// The location does not support file locks.
    LibraryLockUnsupported,
    /// The library was moved; `details.path` names the new location.
    LibraryMoved,
    /// A link or an invalid name in the library; `details.path` names it.
    LibraryUnsafePath,
    /// `database/projectwrite.db` is missing.
    LibraryDatabaseMissing,
    /// The database fails the integrity check.
    LibraryDatabaseDamaged,
    /// The copy before a migration failed; nothing was migrated.
    LibraryBackupFailed,
    /// A library move is running.
    LibraryBusy,
    /// The chosen folder contains other files.
    FolderNotEmpty,
    /// The chosen folder already contains a library.
    FolderIsLibrary,
    /// The location needs a confirmation (sync folder or network drive).
    LocationNotAccepted,
    /// The chosen location is no longer known (choose again).
    LocationExpired,
    /// The path cannot be represented as text.
    PathNotUnicode,
    /// The move was cancelled; the library was not changed.
    MoveCancelled,
    /// The target is inside the library or contains it.
    MoveTargetInvalid,
    /// A copied file differed from the original; nothing was switched.
    MoveVerificationFailed,
    /// The moved library could not be opened; the old one stays in use.
    MoveSwitchFailed,
    /// `settings.json` comes from a newer version and is not changed.
    SettingsReadOnly,
    /// `settings.json` could not be written.
    SettingsNotSaved,
    /// No document with this id (or it is in the trash).
    DocumentNotFound,
    /// The content exceeds the size limit.
    DocumentTooLarge,
    /// The content is not UTF-8 text.
    DocumentNotText,
    /// The format cannot be edited yet (`.pwrite` before Phase 4).
    DocumentFormatUnsupported,
    /// The file type is not on the import positive list.
    ImportUnsupportedType,
    /// A theme file is not valid (`details.field`).
    ThemeInvalid,
    /// A theme misses a contrast rule (`details.pair`, `details.ratio`).
    ThemeContrast,
    /// Too many imported themes.
    ThemeLimit,
    /// No imported theme with this id.
    ThemeNotFound,
}

impl ErrorCode {
    /// All codes, used by tests to verify that every message key is translated.
    pub const ALL: [ErrorCode; 38] = [
        ErrorCode::InvalidInput,
        ErrorCode::Database,
        ErrorCode::SchemaTooNew,
        ErrorCode::Internal,
        ErrorCode::IpcRejected,
        ErrorCode::FileSystem,
        ErrorCode::LibraryNotOpen,
        ErrorCode::LibraryNotFound,
        ErrorCode::LibraryInvalid,
        ErrorCode::LibraryTooNew,
        ErrorCode::LibraryLocked,
        ErrorCode::LibraryLockUnsupported,
        ErrorCode::LibraryMoved,
        ErrorCode::LibraryUnsafePath,
        ErrorCode::LibraryDatabaseMissing,
        ErrorCode::LibraryDatabaseDamaged,
        ErrorCode::LibraryBackupFailed,
        ErrorCode::LibraryBusy,
        ErrorCode::FolderNotEmpty,
        ErrorCode::FolderIsLibrary,
        ErrorCode::LocationNotAccepted,
        ErrorCode::LocationExpired,
        ErrorCode::PathNotUnicode,
        ErrorCode::MoveCancelled,
        ErrorCode::MoveTargetInvalid,
        ErrorCode::MoveVerificationFailed,
        ErrorCode::MoveSwitchFailed,
        ErrorCode::SettingsReadOnly,
        ErrorCode::SettingsNotSaved,
        ErrorCode::DocumentNotFound,
        ErrorCode::DocumentTooLarge,
        ErrorCode::DocumentNotText,
        ErrorCode::DocumentFormatUnsupported,
        ErrorCode::ImportUnsupportedType,
        ErrorCode::ThemeInvalid,
        ErrorCode::ThemeContrast,
        ErrorCode::ThemeLimit,
        ErrorCode::ThemeNotFound,
    ];

    /// Translation key for the user-facing message (see `src/i18n/*.json`).
    pub const fn message_key(self) -> &'static str {
        match self {
            ErrorCode::InvalidInput => "error.invalidInput",
            ErrorCode::Database => "error.database",
            ErrorCode::SchemaTooNew => "error.schemaTooNew",
            ErrorCode::Internal => "error.internal",
            ErrorCode::IpcRejected => "error.ipcRejected",
            ErrorCode::FileSystem => "error.fileSystem",
            ErrorCode::LibraryNotOpen => "error.libraryNotOpen",
            ErrorCode::LibraryNotFound => "error.libraryNotFound",
            ErrorCode::LibraryInvalid => "error.libraryInvalid",
            ErrorCode::LibraryTooNew => "error.libraryTooNew",
            ErrorCode::LibraryLocked => "error.libraryLocked",
            ErrorCode::LibraryLockUnsupported => "error.libraryLockUnsupported",
            ErrorCode::LibraryMoved => "error.libraryMoved",
            ErrorCode::LibraryUnsafePath => "error.libraryUnsafePath",
            ErrorCode::LibraryDatabaseMissing => "error.libraryDatabaseMissing",
            ErrorCode::LibraryDatabaseDamaged => "error.libraryDatabaseDamaged",
            ErrorCode::LibraryBackupFailed => "error.libraryBackupFailed",
            ErrorCode::LibraryBusy => "error.libraryBusy",
            ErrorCode::FolderNotEmpty => "error.folderNotEmpty",
            ErrorCode::FolderIsLibrary => "error.folderIsLibrary",
            ErrorCode::LocationNotAccepted => "error.locationNotAccepted",
            ErrorCode::LocationExpired => "error.locationExpired",
            ErrorCode::PathNotUnicode => "error.pathNotUnicode",
            ErrorCode::MoveCancelled => "error.moveCancelled",
            ErrorCode::MoveTargetInvalid => "error.moveTargetInvalid",
            ErrorCode::MoveVerificationFailed => "error.moveVerificationFailed",
            ErrorCode::MoveSwitchFailed => "error.moveSwitchFailed",
            ErrorCode::SettingsReadOnly => "error.settingsReadOnly",
            ErrorCode::SettingsNotSaved => "error.settingsNotSaved",
            ErrorCode::DocumentNotFound => "error.documentNotFound",
            ErrorCode::DocumentTooLarge => "error.documentTooLarge",
            ErrorCode::DocumentNotText => "error.documentNotText",
            ErrorCode::DocumentFormatUnsupported => "error.documentFormatUnsupported",
            ErrorCode::ImportUnsupportedType => "error.importUnsupportedType",
            ErrorCode::ThemeInvalid => "error.themeInvalid",
            ErrorCode::ThemeContrast => "error.themeContrast",
            ErrorCode::ThemeLimit => "error.themeLimit",
            ErrorCode::ThemeNotFound => "error.themeNotFound",
        }
    }
}

/// The error value every command returns to the frontend.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS, thiserror::Error)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
#[error("{code:?} ({message_key})")]
pub struct CommandError {
    pub code: ErrorCode,
    pub message_key: String,
    /// Optional machine-readable context, for example the invalid field.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub details: Option<BTreeMap<String, String>>,
}

impl CommandError {
    pub fn new(code: ErrorCode) -> Self {
        Self {
            code,
            message_key: code.message_key().to_owned(),
            details: None,
        }
    }

    /// Validation failure for a named input field.
    pub fn invalid_input(field: &str, reason: impl Into<String>) -> Self {
        Self::new(ErrorCode::InvalidInput)
            .with_detail("field", field)
            .with_detail("reason", reason)
    }

    pub fn with_detail(mut self, key: &str, value: impl Into<String>) -> Self {
        let mut value: String = value.into();
        if value.len() > MAX_DETAIL_LEN {
            let mut cut = MAX_DETAIL_LEN;
            while !value.is_char_boundary(cut) {
                cut -= 1;
            }
            value.truncate(cut);
            value.push('…');
        }
        self.details
            .get_or_insert_with(BTreeMap::new)
            .insert(key.to_owned(), value);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn serializes_to_uniform_format_without_details() {
        let value = serde_json::to_value(CommandError::new(ErrorCode::Internal)).unwrap();
        assert_eq!(
            value,
            json!({ "code": "INTERNAL", "messageKey": "error.internal" })
        );
    }

    #[test]
    fn serializes_details_for_invalid_input() {
        let value = serde_json::to_value(CommandError::invalid_input("checks", "empty")).unwrap();
        assert_eq!(
            value,
            json!({
                "code": "INVALID_INPUT",
                "messageKey": "error.invalidInput",
                "details": { "field": "checks", "reason": "empty" }
            })
        );
    }

    #[test]
    fn truncates_long_details_on_char_boundary() {
        let long = "ä".repeat(MAX_DETAIL_LEN);
        let error = CommandError::invalid_input("field", long);
        let reason = &error.details.unwrap()["reason"];
        assert!(reason.len() <= MAX_DETAIL_LEN + '…'.len_utf8());
        assert!(reason.ends_with('…'));
    }

    #[test]
    fn every_code_has_a_distinct_message_key() {
        let mut keys: Vec<_> = ErrorCode::ALL.iter().map(|c| c.message_key()).collect();
        keys.sort_unstable();
        keys.dedup();
        assert_eq!(keys.len(), ErrorCode::ALL.len());
    }
}
