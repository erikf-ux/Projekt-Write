//! Backups and restoring from them, stage 5 (Phase 10, section 21, ADR
//! 0021). Targets and files come from the system dialogs Rust shows; the
//! frontend names no path.

use std::sync::Mutex;
use std::time::{Duration, Instant};

use serde::Deserialize;
use tauri::State;
use tauri::ipc::Channel;
use ts_rs::TS;

use crate::commands::input::{EmptyRequest, Input, Validate};
use crate::error::CommandError;
use crate::services::backup::{self, BackupProgress, BackupStep, BackupSummary};
use crate::services::dialog::{FileDialog, FileFilter, FilePickRequest, PickRequest};
use crate::services::session::{BackupOverview, LibraryStatus, backup_error};
use crate::state::AppState;

/// Progress reaches the interface at most this often (plus every change of
/// step).
const PROGRESS_INTERVAL: Duration = Duration::from_millis(100);
const MAX_TITLE_CHARS: usize = 200;

fn throttled(
    on_progress: Channel<BackupProgress>,
) -> impl Fn(BackupProgress) + Send + Sync + 'static {
    let last: Mutex<Option<(Instant, BackupStep)>> = Mutex::new(None);
    move |progress: BackupProgress| {
        let Ok(mut last) = last.lock() else {
            return;
        };
        let now = Instant::now();
        let due = match *last {
            None => true,
            Some((at, step)) => {
                step != progress.step || now.duration_since(at) >= PROGRESS_INTERVAL
            }
        };
        if due {
            *last = Some((now, progress.step));
            let _ = on_progress.send(progress);
        }
    }
}

/// A request with the title of the dialog it shows.
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct DialogTitleRequest {
    pub title: String,
}

impl Validate for DialogTitleRequest {
    fn validate(&self) -> Result<(), CommandError> {
        let count = self.title.chars().count();
        if count == 0 || count > MAX_TITLE_CHARS || self.title.chars().any(char::is_control) {
            return Err(CommandError::invalid_input("title", "1 to 200 characters"));
        }
        Ok(())
    }
}

/// Request of `backup_set_options`.
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct BackupOptionsRequest {
    #[serde(default)]
    #[ts(optional)]
    pub automatic: Option<bool>,
    #[serde(default)]
    #[ts(optional)]
    pub keep: Option<u32>,
}

impl Validate for BackupOptionsRequest {
    fn validate(&self) -> Result<(), CommandError> {
        if self.keep.is_some_and(|keep| !(1..=100).contains(&keep)) {
            return Err(CommandError::invalid_input("keep", "1 to 100"));
        }
        Ok(())
    }
}

/// Request of `backup_restore`: the titles of its two dialogs.
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct RestoreRequest {
    pub file_title: String,
    pub folder_title: String,
}

impl Validate for RestoreRequest {
    fn validate(&self) -> Result<(), CommandError> {
        for (field, title) in [
            ("fileTitle", &self.file_title),
            ("folderTitle", &self.folder_title),
        ] {
            let count = title.chars().count();
            if count == 0 || count > MAX_TITLE_CHARS || title.chars().any(char::is_control) {
                return Err(CommandError::invalid_input(field, "1 to 200 characters"));
            }
        }
        Ok(())
    }
}

fn backup_filter() -> FileFilter {
    FileFilter {
        name: "Project Write Backup".to_owned(),
        extensions: vec![backup::EXTENSION.to_owned()],
    }
}

/// Target, automatic backup, how many are kept, the last backup.
#[tauri::command]
pub async fn backup_overview(
    state: State<'_, AppState>,
    request: Input<EmptyRequest>,
) -> Result<BackupOverview, CommandError> {
    let _ = request.into_inner();
    state.session.backup_overview().await
}

/// Chooses the backup target in the folder dialog; `None` if cancelled.
#[tauri::command]
pub async fn backup_choose_target(
    state: State<'_, AppState>,
    request: Input<DialogTitleRequest>,
) -> Result<Option<BackupOverview>, CommandError> {
    let request = request.into_inner();
    let answer = state.picker.pick_folder(PickRequest {
        title: request.title,
        start_in: None,
    });
    let Ok(Some(path)) = answer.await else {
        return Ok(None);
    };
    state.session.set_backup_target(Some(path)).await.map(Some)
}

/// Back to `backups/` of the library.
#[tauri::command]
pub async fn backup_reset_target(
    state: State<'_, AppState>,
    request: Input<EmptyRequest>,
) -> Result<BackupOverview, CommandError> {
    let _ = request.into_inner();
    state.session.set_backup_target(None).await
}

#[tauri::command]
pub async fn backup_set_options(
    state: State<'_, AppState>,
    request: Input<BackupOptionsRequest>,
) -> Result<BackupOverview, CommandError> {
    let request = request.into_inner();
    state
        .session
        .set_backup_options(request.automatic, request.keep)
        .await
}

/// "Backup erstellen".
#[tauri::command]
pub async fn backup_create(
    state: State<'_, AppState>,
    request: Input<EmptyRequest>,
    on_progress: Channel<BackupProgress>,
) -> Result<BackupSummary, CommandError> {
    let _ = request.into_inner();
    state.session.create_backup(throttled(on_progress)).await
}

/// Stops the running backup; `false` if none runs.
#[tauri::command]
pub async fn backup_cancel(
    state: State<'_, AppState>,
    request: Input<EmptyRequest>,
) -> Result<bool, CommandError> {
    let _ = request.into_inner();
    Ok(state.session.cancel_backup())
}

/// "Backup prüfen": a backup from the file dialog, checked in full,
/// changing nothing; `None` if cancelled.
#[tauri::command]
pub async fn backup_check(
    state: State<'_, AppState>,
    request: Input<DialogTitleRequest>,
    on_progress: Channel<BackupProgress>,
) -> Result<Option<BackupSummary>, CommandError> {
    let request = request.into_inner();
    let answer = state.files.pick_file(
        FileDialog::Open,
        FilePickRequest {
            title: request.title,
            start_in: None,
            file_name: None,
            filters: vec![backup_filter()],
        },
    );
    let Ok(Some(path)) = answer.await else {
        return Ok(None);
    };
    let scratch = state.session.scratch_dir().await?;
    let progress = throttled(on_progress);
    let result = backup::check_summary(&path, &scratch, std::sync::Arc::new(progress)).await;
    let _ = std::fs::remove_dir_all(&scratch);
    result.map(Some).map_err(backup_error)
}

/// Stage 5: a backup from the file dialog into a new folder inside the
/// folder chosen next; then that library opens. `None` if cancelled.
#[tauri::command]
pub async fn backup_restore(
    state: State<'_, AppState>,
    request: Input<RestoreRequest>,
    on_progress: Channel<BackupProgress>,
) -> Result<Option<LibraryStatus>, CommandError> {
    let request = request.into_inner();
    let file = state.files.pick_file(
        FileDialog::Open,
        FilePickRequest {
            title: request.file_title,
            start_in: None,
            file_name: None,
            filters: vec![backup_filter()],
        },
    );
    let Ok(Some(path)) = file.await else {
        return Ok(None);
    };
    let folder = state.picker.pick_folder(PickRequest {
        title: request.folder_title,
        start_in: None,
    });
    let Ok(Some(parent)) = folder.await else {
        return Ok(None);
    };
    let target = state.session.restore_target(&parent)?;
    let progress = throttled(on_progress);
    backup::restore(&path, &target, std::sync::Arc::new(progress))
        .await
        .map_err(backup_error)?;
    state.session.open_restored(&target).await.map(Some)
}
