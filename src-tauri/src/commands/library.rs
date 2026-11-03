//! Library commands (Phase 2): status, choosing a folder, create, open,
//! move. The frontend never sends a path; folders are chosen in Rust and
//! referred to by token (section 8).

use std::sync::Mutex;
use std::time::{Duration, Instant};

use serde::Deserialize;
use tauri::State;
use tauri::ipc::Channel;
use ts_rs::TS;

use crate::commands::input::{EmptyRequest, Input, Validate};
use crate::error::CommandError;
use crate::services::library::relocate::{MovePhase, MoveProgress};
use crate::services::session::{LibraryStatus, LocationChoice, LocationPurpose};
use crate::state::AppState;

/// Longest dialog title accepted from the frontend.
const MAX_TITLE_CHARS: usize = 120;
/// Progress messages are sent at most this often (plus every phase change).
const PROGRESS_INTERVAL: Duration = Duration::from_millis(100);

fn validate_token(token: &str) -> Result<(), CommandError> {
    let valid = token.len() == 32
        && token
            .bytes()
            .all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'));
    if valid {
        Ok(())
    } else {
        Err(CommandError::invalid_input(
            "location",
            "not a location token",
        ))
    }
}

/// Request of `library_choose_location`.
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct ChooseLocationRequest {
    pub purpose: LocationPurpose,
    /// Title of the dialog, in the language of the interface.
    pub title: String,
}

impl Validate for ChooseLocationRequest {
    fn validate(&self) -> Result<(), CommandError> {
        let length = self.title.chars().count();
        if length == 0 || length > MAX_TITLE_CHARS {
            return Err(CommandError::invalid_input("title", "length"));
        }
        if self.title.chars().any(char::is_control) {
            return Err(CommandError::invalid_input("title", "control character"));
        }
        Ok(())
    }
}

/// Request of `library_create`.
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct CreateLibraryRequest {
    /// Token from `library_default_location` or `library_choose_location`.
    pub location: String,
    /// The user confirmed a sync folder or network drive.
    pub accept_warning: bool,
}

impl Validate for CreateLibraryRequest {
    fn validate(&self) -> Result<(), CommandError> {
        validate_token(&self.location)
    }
}

/// Request of `library_open`.
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct OpenLibraryRequest {
    pub location: String,
    /// Open a library although it was moved elsewhere.
    pub accept_moved: bool,
}

impl Validate for OpenLibraryRequest {
    fn validate(&self) -> Result<(), CommandError> {
        validate_token(&self.location)
    }
}

/// Request of `library_move`.
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct MoveLibraryRequest {
    pub location: String,
    pub accept_warning: bool,
}

impl Validate for MoveLibraryRequest {
    fn validate(&self) -> Result<(), CommandError> {
        validate_token(&self.location)
    }
}

/// State of the library of this instance.
#[tauri::command]
pub async fn library_status(
    state: State<'_, AppState>,
    request: Input<EmptyRequest>,
) -> Result<LibraryStatus, CommandError> {
    let _ = request.into_inner();
    Ok(state.session.status().await)
}

/// `Dokumente/ProjectWrite` as a location for a new library (`null` if the
/// system has no documents folder).
#[tauri::command]
pub async fn library_default_location(
    state: State<'_, AppState>,
    request: Input<EmptyRequest>,
) -> Result<Option<LocationChoice>, CommandError> {
    let _ = request.into_inner();
    Ok(state.session.default_location())
}

/// Shows the folder dialog (`null` if the user cancelled).
#[tauri::command]
pub async fn library_choose_location(
    state: State<'_, AppState>,
    request: Input<ChooseLocationRequest>,
) -> Result<Option<LocationChoice>, CommandError> {
    let request = request.into_inner();
    Ok(state
        .session
        .choose_location(state.picker.as_ref(), request.purpose, request.title)
        .await)
}

/// Creates a library and opens it.
#[tauri::command]
pub async fn library_create(
    state: State<'_, AppState>,
    request: Input<CreateLibraryRequest>,
) -> Result<LibraryStatus, CommandError> {
    let request = request.into_inner();
    state
        .session
        .create(&request.location, request.accept_warning)
        .await
}

/// Opens an existing library.
#[tauri::command]
pub async fn library_open(
    state: State<'_, AppState>,
    request: Input<OpenLibraryRequest>,
) -> Result<LibraryStatus, CommandError> {
    let request = request.into_inner();
    state
        .session
        .open(&request.location, request.accept_moved)
        .await
}

/// Opens the library from the app settings again (after an error).
#[tauri::command]
pub async fn library_reopen(
    state: State<'_, AppState>,
    request: Input<EmptyRequest>,
) -> Result<LibraryStatus, CommandError> {
    let _ = request.into_inner();
    state.session.reopen().await
}

/// Moves the open library; progress arrives through `on_progress`.
#[tauri::command]
pub async fn library_move(
    state: State<'_, AppState>,
    request: Input<MoveLibraryRequest>,
    on_progress: Channel<MoveProgress>,
) -> Result<LibraryStatus, CommandError> {
    let request = request.into_inner();
    let last: Mutex<Option<(Instant, MovePhase)>> = Mutex::new(None);
    let report = move |progress: MoveProgress| {
        let Ok(mut last) = last.lock() else {
            return;
        };
        let now = Instant::now();
        let due = match *last {
            None => true,
            Some((at, phase)) => {
                phase != progress.phase
                    || now.duration_since(at) >= PROGRESS_INTERVAL
                    || progress.done_files == progress.total_files
            }
        };
        if due {
            *last = Some((now, progress.phase));
            let _ = on_progress.send(progress);
        }
    };
    state
        .session
        .move_to(&request.location, request.accept_warning, report)
        .await
}

/// Stops a running move; `true` if one was running.
#[tauri::command]
pub async fn library_cancel_move(
    state: State<'_, AppState>,
    request: Input<EmptyRequest>,
) -> Result<bool, CommandError> {
    let _ = request.into_inner();
    Ok(state.session.cancel_move())
}

/// Accepts the sync folder or network drive the library lies in.
#[tauri::command]
pub async fn library_accept_location(
    state: State<'_, AppState>,
    request: Input<EmptyRequest>,
) -> Result<LibraryStatus, CommandError> {
    let _ = request.into_inner();
    state.session.accept_location().await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::ErrorCode;

    #[test]
    fn tokens_are_32_lower_case_hex_digits() {
        assert!(validate_token("0123456789abcdef0123456789abcdef").is_ok());
        for token in [
            "",
            "0123456789ABCDEF0123456789ABCDEF",
            "0123456789abcdef0123456789abcde",
            "0123456789abcdef0123456789abcdef0",
            "../../etc/passwd/xxxxxxxxxxxxxxxx",
            "C:\\Users\\a\\Documents\\ProjectWrit",
        ] {
            let error = validate_token(token).unwrap_err();
            assert_eq!(error.code, ErrorCode::InvalidInput, "{token}");
        }
    }

    #[test]
    fn dialog_titles_are_checked() {
        let request = |title: &str| ChooseLocationRequest {
            purpose: LocationPurpose::Create,
            title: title.to_owned(),
        };
        assert!(request("Ort für die Bibliothek wählen").validate().is_ok());
        assert!(request("").validate().is_err());
        assert!(request(&"x".repeat(121)).validate().is_err());
        assert!(request("a\u{0}b").validate().is_err());
    }
}
