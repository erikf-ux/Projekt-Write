//! The registries in Rust, for the settings section "Erweitert" (Phase 13,
//! ADR 0027, decision 5). Read only: there is no command that adds an
//! entry or loads code.

use tauri::State;

use crate::commands::input::{EmptyRequest, Input};
use crate::error::CommandError;
use crate::services::extensions::ExtensionInfo;
use crate::state::AppState;

/// Every entry of the document formats, exporters and checking engines, in
/// registration order.
#[tauri::command]
pub async fn extensions_list(
    state: State<'_, AppState>,
    _request: Input<EmptyRequest>,
) -> Result<Vec<ExtensionInfo>, CommandError> {
    Ok(state.extensions.list())
}
