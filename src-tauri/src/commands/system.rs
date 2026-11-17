//! System commands: the self-check and the storage locations of the app
//! (ADR 0027, decision 5).

use serde::{Deserialize, Serialize};
use tauri::State;
use ts_rs::TS;

use crate::commands::input::{EmptyRequest, Input, Validate};
use crate::error::{CommandError, ErrorCode};
use crate::security::paths::display_path;
use crate::services::system::{self, SystemCheckRequest, SystemCheckResponse};
use crate::state::AppState;

/// Runs the requested system checks (start screen).
#[tauri::command]
pub async fn system_check(
    state: State<'_, AppState>,
    request: Input<SystemCheckRequest>,
) -> Result<SystemCheckResponse, CommandError> {
    let request = request.into_inner();
    let database = state.check_database().await;
    system::run_checks(&database, &request.checks)
        .await
        .map_err(|error| {
            log::error!("system_check failed: {error}");
            CommandError::new(ErrorCode::Database)
        })
}

/// A folder of the app the user can open in the file manager. The frontend
/// names the folder, never a path.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum StorageFolder {
    /// The configuration folder (`settings.json`, imported themes and
    /// dictionaries, settings backups).
    Config,
    /// The log files.
    Logs,
    /// The open library.
    Library,
}

/// The storage locations, for display.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct StorageLocations {
    pub config: String,
    pub logs: String,
    /// None while no library is open.
    pub library: Option<String>,
}

/// Where the app keeps its configuration, its logs and the open library.
#[tauri::command]
pub async fn system_locations(
    state: State<'_, AppState>,
    request: Input<EmptyRequest>,
) -> Result<StorageLocations, CommandError> {
    let _ = request.into_inner();
    let dirs = state.session.dirs();
    Ok(StorageLocations {
        config: display_path(&dirs.config),
        logs: display_path(&dirs.logs),
        library: state
            .session
            .library_path()
            .await
            .map(|path| display_path(&path)),
    })
}

/// Request of `system_open_location`.
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct OpenLocationRequest {
    pub location: StorageFolder,
}

impl Validate for OpenLocationRequest {
    fn validate(&self) -> Result<(), CommandError> {
        Ok(())
    }
}

/// Shows one of the storage locations in the file manager of the system.
#[tauri::command]
pub async fn system_open_location(
    state: State<'_, AppState>,
    request: Input<OpenLocationRequest>,
) -> Result<(), CommandError> {
    let dirs = state.session.dirs();
    let folder = match request.into_inner().location {
        StorageFolder::Config => dirs.config.clone(),
        StorageFolder::Logs => dirs.logs.clone(),
        StorageFolder::Library => state
            .session
            .library_path()
            .await
            .ok_or_else(|| CommandError::new(ErrorCode::LibraryNotOpen))?,
    };
    // The configuration folder appears with the first saved setting.
    if let Err(error) = std::fs::create_dir_all(&folder) {
        log::warn!("folder not created: {error}");
        return Err(CommandError::new(ErrorCode::FolderOpenFailed));
    }
    let target = folder
        .to_str()
        .ok_or_else(|| CommandError::new(ErrorCode::PathNotUnicode))?;
    state.links.open(target).map_err(|error| {
        log::warn!("folder not opened: {error}");
        CommandError::new(ErrorCode::FolderOpenFailed)
    })
}
