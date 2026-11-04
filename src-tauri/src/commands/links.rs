//! Opening web links (Phase 4, section 12).

use std::sync::Arc;

use serde::Deserialize;
use tauri::State;
use ts_rs::TS;

use crate::commands::input::{Input, Validate};
use crate::error::{CommandError, ErrorCode};
use crate::services::links;
use crate::state::AppState;

/// Request of `link_open`.
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct OpenLinkRequest {
    /// An http, https or mailto address.
    pub url: String,
}

impl Validate for OpenLinkRequest {
    fn validate(&self) -> Result<(), CommandError> {
        links::check(&self.url)
    }
}

/// Opens a web link in the system browser (`Ctrl+Klick` in the editor).
#[tauri::command]
pub async fn link_open(
    state: State<'_, AppState>,
    request: Input<OpenLinkRequest>,
) -> Result<(), CommandError> {
    let request = request.into_inner();
    let opener = Arc::clone(&state.links);
    tauri::async_runtime::spawn_blocking(move || links::open(opener.as_ref(), &request.url))
        .await
        .map_err(|_| CommandError::new(ErrorCode::Internal))?
}
