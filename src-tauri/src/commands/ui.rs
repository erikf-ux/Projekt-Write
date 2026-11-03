//! Interface commands (Phase 3): settings of the interface.

use serde::Deserialize;
use tauri::{Runtime, State, WebviewWindow};
use ts_rs::TS;

use crate::commands::input::{EmptyRequest, Input, Validate};
use crate::error::CommandError;
use crate::services::settings::ui::UiSettings;
use crate::services::ui::{UiState, window};
use crate::state::AppState;

/// Request of `ui_update_settings`.
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct UpdateUiSettingsRequest {
    /// All interface settings; they replace the current ones.
    pub settings: UiSettings,
}

impl Validate for UpdateUiSettingsRequest {
    fn validate(&self) -> Result<(), CommandError> {
        self.settings.validate()
    }
}

/// Settings, language and platform for setting up the interface.
#[tauri::command]
pub async fn ui_state(
    state: State<'_, AppState>,
    request: Input<EmptyRequest>,
) -> Result<UiState, CommandError> {
    let _ = request.into_inner();
    Ok(state.ui.state())
}

/// Saves the interface settings and applies what Rust controls (zoom).
#[tauri::command]
pub async fn ui_update_settings<R: Runtime>(
    window: WebviewWindow<R>,
    state: State<'_, AppState>,
    request: Input<UpdateUiSettingsRequest>,
) -> Result<UiState, CommandError> {
    let request = request.into_inner();
    let (ui, previous) = state.ui.update(request.settings)?;
    if (previous.zoom - ui.settings.zoom).abs() > f64::EPSILON {
        window::apply_zoom(&window, ui.settings.zoom);
    }
    Ok(ui)
}
