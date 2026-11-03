//! Interface commands (Phase 3): settings of the interface, the event
//! channel, the menu state, focus mode and closing the window.

use std::collections::BTreeMap;

use serde::Deserialize;
use tauri::ipc::Channel;
use tauri::{AppHandle, Manager, Runtime, State, WebviewWindow};
use ts_rs::TS;

use crate::commands::input::{EmptyRequest, Input, Validate};
use crate::error::CommandError;
use crate::services::session::lock;
use crate::services::settings::ui::{UiSettings, is_theme_id};
use crate::services::themes::MAX_THEME_BYTES;
use crate::services::ui::menu::{MAX_SHORTCUT_LABEL_CHARS, MenuCommand, MenuItemState, MenuSlot};
use crate::services::ui::{UiEvent, UiState, window};
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
    let previous_locale = state.ui.locale();
    let (ui, previous) = state.ui.update(request.settings)?;
    if (previous.zoom - ui.settings.zoom).abs() > f64::EPSILON {
        window::apply_zoom(&window, ui.settings.zoom);
    }
    if previous_locale != ui.locale {
        refresh_menu(window.app_handle(), &state);
    }
    Ok(ui)
}

/// Registers the channel for menu clicks and close requests.
#[tauri::command]
pub async fn ui_subscribe(
    state: State<'_, AppState>,
    request: Input<EmptyRequest>,
    on_event: Channel<UiEvent>,
) -> Result<(), CommandError> {
    let _ = request.into_inner();
    state.ui.subscribe(on_event);
    Ok(())
}

/// Request of `ui_menu_state`.
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct MenuStateRequest {
    /// State of the menu items; items not listed are disabled.
    pub items: BTreeMap<MenuCommand, MenuItemState>,
}

impl Validate for MenuStateRequest {
    fn validate(&self) -> Result<(), CommandError> {
        for item in self.items.values() {
            if let Some(shortcut) = &item.shortcut {
                let length = shortcut.chars().count();
                if length == 0 || length > MAX_SHORTCUT_LABEL_CHARS {
                    return Err(CommandError::invalid_input("shortcut", "length"));
                }
                if shortcut.chars().any(char::is_control) {
                    return Err(CommandError::invalid_input("shortcut", "control character"));
                }
            }
        }
        Ok(())
    }
}

/// Enables the menu items of available commands and shows their shortcuts.
#[tauri::command]
pub async fn ui_menu_state<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, AppState>,
    request: Input<MenuStateRequest>,
) -> Result<(), CommandError> {
    state.ui.set_menu_state(request.into_inner().items);
    refresh_menu(&app, &state);
    Ok(())
}

/// Request of `ui_set_focus_mode`.
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct FocusModeRequest {
    pub enabled: bool,
}

impl Validate for FocusModeRequest {
    fn validate(&self) -> Result<(), CommandError> {
        Ok(())
    }
}

/// Focus mode (section 11): full screen without the menu bar; the frontend
/// hides everything else except the writing surface.
#[tauri::command]
pub async fn ui_set_focus_mode<R: Runtime>(
    window: WebviewWindow<R>,
    request: Input<FocusModeRequest>,
) -> Result<(), CommandError> {
    let enabled = request.into_inner().enabled;
    let menu = if enabled {
        window.hide_menu()
    } else {
        window.show_menu()
    };
    if let Err(error) = menu {
        log::warn!("menu bar not switched: {error}");
    }
    if let Err(error) = window.set_fullscreen(enabled) {
        log::warn!("full screen not switched: {error}");
    }
    Ok(())
}

/// Request of `ui_close_response`.
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct CloseResponseRequest {
    /// `true`: everything is saved, close the window. `false`: stay open.
    pub close: bool,
}

impl Validate for CloseResponseRequest {
    fn validate(&self) -> Result<(), CommandError> {
        Ok(())
    }
}

/// The frontend's answer to a close request (or its own "Quit").
#[tauri::command]
pub async fn ui_close_response<R: Runtime>(
    window: WebviewWindow<R>,
    state: State<'_, AppState>,
    request: Input<CloseResponseRequest>,
) -> Result<(), CommandError> {
    let close = request.into_inner().close;
    state.ui.close_response(close);
    if close && let Err(error) = window.close() {
        log::error!("window not closed: {error}");
    }
    Ok(())
}

/// Applies language and item states to the menu, if the window has one.
fn refresh_menu<R: Runtime>(app: &AppHandle<R>, state: &AppState) {
    if let Some(slot) = app.try_state::<MenuSlot<R>>()
        && let Some(handles) = lock(&slot.0).as_ref()
    {
        handles.apply(state.ui.locale(), &state.ui.menu_state());
    }
}

/// Request of `theme_import`.
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct ImportThemeRequest {
    /// Text of the theme file the user picked.
    pub content: String,
}

impl Validate for ImportThemeRequest {
    fn validate(&self) -> Result<(), CommandError> {
        if self.content.len() > MAX_THEME_BYTES {
            return Err(CommandError::invalid_input("content", "too large"));
        }
        Ok(())
    }
}

/// Imports a theme file and makes it the active theme.
#[tauri::command]
pub async fn theme_import(
    state: State<'_, AppState>,
    request: Input<ImportThemeRequest>,
) -> Result<UiState, CommandError> {
    state.ui.import_theme(&request.into_inner().content)
}

/// Request of `theme_remove`.
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct RemoveThemeRequest {
    pub id: String,
}

impl Validate for RemoveThemeRequest {
    fn validate(&self) -> Result<(), CommandError> {
        if is_theme_id(&self.id) {
            Ok(())
        } else {
            Err(CommandError::invalid_input("id", "not a theme id"))
        }
    }
}

/// Removes an imported theme (the copy in the configuration folder).
#[tauri::command]
pub async fn theme_remove(
    state: State<'_, AppState>,
    request: Input<RemoveThemeRequest>,
) -> Result<UiState, CommandError> {
    state.ui.remove_theme(&request.into_inner().id)
}
