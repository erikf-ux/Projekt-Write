//! System commands.

use tauri::State;

use crate::commands::input::Input;
use crate::error::{CommandError, ErrorCode};
use crate::services::system::{self, SystemCheckRequest, SystemCheckResponse};
use crate::state::AppState;

/// Runs the requested system checks (start screen).
#[tauri::command]
pub async fn system_check(
    state: State<'_, AppState>,
    request: Input<SystemCheckRequest>,
) -> Result<SystemCheckResponse, CommandError> {
    let request = request.into_inner();
    system::run_checks(&state.database, &request.checks)
        .await
        .map_err(|error| {
            log::error!("system_check failed: {error}");
            CommandError::new(ErrorCode::Database)
        })
}
