//! Export, import and reset of the settings (ADR 0027, decision 4). Files
//! come from the system dialogs Rust shows; the frontend names no path and
//! sends no values: an import is applied by the token of its preview.

use std::path::PathBuf;

use serde::Deserialize;
use tauri::{Runtime, State, WebviewWindow};
use ts_rs::TS;

use crate::clock::Timestamp;
use crate::commands::backup::DialogTitleRequest;
use crate::commands::input::{Input, Validate};
use crate::commands::ui::apply_changed_settings;
use crate::error::{CommandError, ErrorCode};
use crate::filesystem::atomic::write_atomic;
use crate::services::dialog::{FileDialog, FileFilter, FilePickRequest};
use crate::services::settings::transfer::{
    self, Applied, EXTENSION, SettingsApplied, SettingsArea, SettingsImportPreview, Target,
};
use crate::state::AppState;

fn settings_filter() -> FileFilter {
    FileFilter {
        name: "Project Write Einstellungen".to_owned(),
        extensions: vec![EXTENSION.to_owned()],
    }
}

/// Exports the settings into a `.pwsettings` file the user picks; `false`
/// when the dialog was cancelled.
#[tauri::command]
pub async fn settings_export(
    state: State<'_, AppState>,
    request: Input<DialogTitleRequest>,
) -> Result<bool, CommandError> {
    let request = request.into_inner();
    let now = Timestamp::now();
    let answer = state.settings_files.pick_file(
        FileDialog::Save,
        FilePickRequest {
            title: request.title,
            start_in: state.session.dirs().documents.clone(),
            file_name: Some(format!(
                "projectwrite-{}.{EXTENSION}",
                now.to_iso().get(..10).unwrap_or("settings")
            )),
            filters: vec![settings_filter()],
        },
    );
    let Ok(Some(mut file)) = answer.await else {
        return Ok(false);
    };
    if file.extension().is_none() {
        file.set_extension(EXTENSION);
    }
    let ui = &state.ui;
    let config = state.session.dirs().config.clone();
    let config = &config;
    let document = state
        .session
        .with_library_database(move |library| async move {
            Target {
                ui,
                config,
                library: library
                    .as_ref()
                    .map(|(database, id)| (database, id.as_str())),
                now,
            }
            .document()
            .await
        })
        .await?;
    write_atomic(&file, &transfer::to_bytes(&document)).map_err(|error| {
        log::warn!("settings file not written: {error}");
        CommandError::new(ErrorCode::FileSystem)
    })?;
    log::info!("settings exported");
    Ok(true)
}

/// Reads a `.pwsettings` file the user picks and checks it; returns what
/// applying it would change, or none when the dialog was cancelled.
#[tauri::command]
pub async fn settings_import_preview(
    state: State<'_, AppState>,
    request: Input<DialogTitleRequest>,
) -> Result<Option<SettingsImportPreview>, CommandError> {
    let request = request.into_inner();
    let answer = state.settings_files.pick_file(
        FileDialog::Open,
        FilePickRequest {
            title: request.title,
            start_in: state.session.dirs().documents.clone(),
            file_name: None,
            filters: vec![settings_filter()],
        },
    );
    let Ok(Some(file)) = answer.await else {
        return Ok(None);
    };
    let data = transfer::read_file(&file)?;
    let ui = &state.ui;
    let transfer = &state.settings_transfer;
    let config = state.session.dirs().config.clone();
    let config = &config;
    state
        .session
        .with_library_database(move |library| async move {
            let target = Target {
                ui,
                config,
                library: library
                    .as_ref()
                    .map(|(database, id)| (database, id.as_str())),
                now: Timestamp::now(),
            };
            transfer.preview(&target, &data).await.map(Some)
        })
        .await
}

/// Request of `settings_import_apply`.
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct SettingsApplyRequest {
    /// The token of the preview.
    pub token: String,
}

impl Validate for SettingsApplyRequest {
    fn validate(&self) -> Result<(), CommandError> {
        let valid = self.token.len() == 32
            && self
                .token
                .bytes()
                .all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'));
        if valid {
            Ok(())
        } else {
            Err(CommandError::invalid_input("token", "not a token"))
        }
    }
}

/// Applies a previewed import: every value or none, after a backup of the
/// current state.
#[tauri::command]
pub async fn settings_import_apply<R: Runtime>(
    window: WebviewWindow<R>,
    state: State<'_, AppState>,
    request: Input<SettingsApplyRequest>,
) -> Result<SettingsApplied, CommandError> {
    let token = request.into_inner().token;
    let ui = &state.ui;
    let transfer = &state.settings_transfer;
    let config = state.session.dirs().config.clone();
    let config = &config;
    let applied = state
        .session
        .with_library_database(move |library| async move {
            let target = Target {
                ui,
                config,
                library: library
                    .as_ref()
                    .map(|(database, id)| (database, id.as_str())),
                now: Timestamp::now(),
            };
            transfer.apply(&target, &token).await
        })
        .await?;
    Ok(finish(&window, &state, applied))
}

/// Request of `settings_reset`.
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct SettingsResetRequest {
    pub area: SettingsArea,
}

impl Validate for SettingsResetRequest {
    fn validate(&self) -> Result<(), CommandError> {
        Ok(())
    }
}

/// Resets the interface or the library settings to their defaults, after a
/// backup of the current state.
#[tauri::command]
pub async fn settings_reset<R: Runtime>(
    window: WebviewWindow<R>,
    state: State<'_, AppState>,
    request: Input<SettingsResetRequest>,
) -> Result<SettingsApplied, CommandError> {
    let area = request.into_inner().area;
    let ui = &state.ui;
    let transfer = &state.settings_transfer;
    let config: PathBuf = state.session.dirs().config.clone();
    let config = &config;
    let applied = state
        .session
        .with_library_database(move |library| async move {
            let target = Target {
                ui,
                config,
                library: library
                    .as_ref()
                    .map(|(database, id)| (database, id.as_str())),
                now: Timestamp::now(),
            };
            transfer.reset(&target, area).await
        })
        .await?;
    Ok(finish(&window, &state, applied))
}

/// Zoom and menu language follow at once; the frontend applies the rest.
fn finish<R: Runtime>(
    window: &WebviewWindow<R>,
    state: &AppState,
    applied: Applied,
) -> SettingsApplied {
    apply_changed_settings(
        window,
        state,
        applied.previous.as_ref(),
        applied.previous_locale,
        &applied.ui,
    );
    SettingsApplied {
        ui: applied.ui,
        library_changed: applied.library_changed,
    }
}
