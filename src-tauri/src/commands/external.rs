//! External file commands (Phase 5, ADR 0015): open through the system
//! dialog or "Zuletzt verwendet", read, check for changes from outside,
//! save, save as, take into the library. The frontend never sends a path;
//! files are referred to by the id Rust gave them (section 8).

use std::path::PathBuf;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tauri::State;
use ts_rs::TS;

use crate::commands::documents::{DocumentSummary, ImportResult, document_error, import_error};
use crate::commands::input::{EmptyRequest, Input, Validate};
use crate::error::{CommandError, ErrorCode};
use crate::services::dialog::{FileDialog, FileFilter, FilePickRequest};
use crate::services::documents::DocumentFormat;
use crate::services::encoding::{LineEnding, TextEncoding};
use crate::services::external::{
    ExternalError, ExternalFileInfo, ExternalFiles, ExternalText, FileCheck, MAX_FILE_BYTES,
    RecentFileEntry, SaveOutcome, SaveRequest,
};
use crate::services::import;
use crate::services::locale::Locale;
use crate::state::AppState;

/// Longest dialog title accepted from the frontend.
const MAX_TITLE_CHARS: usize = 120;

fn validate_id(id: &str) -> Result<(), CommandError> {
    let valid = id.len() == 32 && id.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'));
    if valid {
        Ok(())
    } else {
        Err(CommandError::invalid_input("id", "not a file id"))
    }
}

fn validate_dialog_title(title: &str) -> Result<(), CommandError> {
    let length = title.chars().count();
    if length == 0 || length > MAX_TITLE_CHARS {
        return Err(CommandError::invalid_input("title", "length"));
    }
    if title.chars().any(char::is_control) {
        return Err(CommandError::invalid_input("title", "control character"));
    }
    Ok(())
}

fn validate_text(text: &str) -> Result<(), CommandError> {
    if text.len() as u64 > MAX_FILE_BYTES {
        return Err(CommandError::new(ErrorCode::DocumentTooLarge));
    }
    Ok(())
}

/// Request of `external_open`.
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct OpenExternalRequest {
    /// Title of the dialog, in the language of the interface.
    pub title: String,
}

impl Validate for OpenExternalRequest {
    fn validate(&self) -> Result<(), CommandError> {
        validate_dialog_title(&self.title)
    }
}

/// Request of the commands that name one external file or recent entry.
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct ExternalFileRequest {
    pub id: String,
}

impl Validate for ExternalFileRequest {
    fn validate(&self) -> Result<(), CommandError> {
        validate_id(&self.id)
    }
}

/// Request of `external_save`.
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct SaveExternalRequest {
    pub id: String,
    /// Text with LF line endings; for `.pwrite` the editor content as JSON.
    pub text: String,
    pub encoding: TextEncoding,
    pub line_ending: LineEnding,
    /// Write even if the file was changed outside the app.
    pub overwrite: bool,
}

impl Validate for SaveExternalRequest {
    fn validate(&self) -> Result<(), CommandError> {
        validate_id(&self.id)?;
        validate_text(&self.text)
    }
}

/// Request of `external_save_as`.
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct SaveExternalAsRequest {
    pub id: String,
    /// Title of the dialog, in the language of the interface.
    pub title: String,
    pub text: String,
    pub encoding: TextEncoding,
    pub line_ending: LineEnding,
}

impl Validate for SaveExternalAsRequest {
    fn validate(&self) -> Result<(), CommandError> {
        validate_id(&self.id)?;
        validate_dialog_title(&self.title)?;
        validate_text(&self.text)
    }
}

/// Request of `external_import`.
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct ImportExternalRequest {
    pub id: String,
    /// The text the tab shows (it may differ from the file).
    pub text: String,
    /// Import even if a document with the same content exists.
    pub allow_duplicate: bool,
}

impl Validate for ImportExternalRequest {
    fn validate(&self) -> Result<(), CommandError> {
        validate_id(&self.id)?;
        validate_text(&self.text)
    }
}

/// Runs file work off the async runtime.
async fn blocking<T: Send + 'static>(
    external: &Arc<ExternalFiles>,
    work: impl FnOnce(&ExternalFiles) -> Result<T, ExternalError> + Send + 'static,
) -> Result<T, CommandError> {
    let external = Arc::clone(external);
    tauri::async_runtime::spawn_blocking(move || work(&external))
        .await
        .map_err(|error| {
            log::error!("external file task failed: {error}");
            CommandError::new(ErrorCode::Internal)
        })?
        .map_err(external_error)
}

/// Shows the open dialog; `None` if the user cancels.
#[tauri::command]
pub async fn external_open(
    state: State<'_, AppState>,
    request: Input<OpenExternalRequest>,
) -> Result<Option<ExternalFileInfo>, CommandError> {
    let request = request.into_inner();
    let locale = state.ui.locale();
    let pick = FilePickRequest {
        title: request.title,
        start_in: last_folder(&state.external),
        file_name: None,
        filters: vec![FileFilter {
            name: open_filter_name(locale).to_owned(),
            extensions: ["txt", "md", "markdown", "pwrite"]
                .map(str::to_owned)
                .to_vec(),
        }],
    };
    let answer = state.files.pick_file(FileDialog::Open, pick);
    let Ok(Some(path)) = answer.await else {
        return Ok(None);
    };
    let info = blocking(&state.external, move |external| external.register(&path)).await?;
    log::info!("external file {} opened", info.id);
    Ok(Some(info))
}

/// Reads an opened file.
#[tauri::command]
pub async fn external_read(
    state: State<'_, AppState>,
    request: Input<ExternalFileRequest>,
) -> Result<ExternalText, CommandError> {
    let id = request.into_inner().id;
    blocking(&state.external, move |external| external.read(&id)).await
}

/// Compares the file with what was read or saved last.
#[tauri::command]
pub async fn external_check(
    state: State<'_, AppState>,
    request: Input<ExternalFileRequest>,
) -> Result<FileCheck, CommandError> {
    let id = request.into_inner().id;
    blocking(&state.external, move |external| external.check(&id)).await
}

/// "Meine Fassung behalten" after a change from outside.
#[tauri::command]
pub async fn external_keep_mine(
    state: State<'_, AppState>,
    request: Input<ExternalFileRequest>,
) -> Result<(), CommandError> {
    let id = request.into_inner().id;
    blocking(&state.external, move |external| external.keep_mine(&id)).await
}

/// Saves into the file (never if nothing changed).
#[tauri::command]
pub async fn external_save(
    state: State<'_, AppState>,
    request: Input<SaveExternalRequest>,
) -> Result<SaveOutcome, CommandError> {
    let request = request.into_inner();
    let id = request.id;
    let save = SaveRequest {
        text: request.text,
        encoding: request.encoding,
        line_ending: request.line_ending,
        overwrite: request.overwrite,
    };
    blocking(&state.external, move |external| external.save(&id, &save)).await
}

/// "Speichern unter": shows the save dialog and writes the new file; the
/// id then stands for it. `None` if the user cancels.
#[tauri::command]
pub async fn external_save_as(
    state: State<'_, AppState>,
    request: Input<SaveExternalAsRequest>,
) -> Result<Option<ExternalFileInfo>, CommandError> {
    let request = request.into_inner();
    let id = request.id;
    let current = state.external.path(&id).map_err(external_error)?;
    let format = crate::services::external::format_of_path(&current).map_err(external_error)?;
    let locale = state.ui.locale();
    let pick = FilePickRequest {
        title: request.title,
        start_in: current.parent().map(PathBuf::from),
        file_name: current
            .file_name()
            .map(|name| name.to_string_lossy().into_owned()),
        filters: vec![FileFilter {
            name: format_filter_name(format, locale).to_owned(),
            extensions: extensions(format).iter().map(|e| (*e).to_owned()).collect(),
        }],
    };
    let answer = state.files.pick_file(FileDialog::Save, pick);
    let Ok(Some(target)) = answer.await else {
        return Ok(None);
    };
    let save = SaveRequest {
        text: request.text,
        encoding: request.encoding,
        line_ending: request.line_ending,
        overwrite: true,
    };
    let info = blocking(&state.external, move |external| {
        external.save_as(&id, &target, &save)
    })
    .await?;
    Ok(Some(info))
}

/// Ends the session of a file (its tab was closed). Unknown ids are fine.
#[tauri::command]
pub async fn external_close(
    state: State<'_, AppState>,
    request: Input<ExternalFileRequest>,
) -> Result<(), CommandError> {
    state.external.close(&request.into_inner().id);
    Ok(())
}

/// "In Bibliothek aufnehmen": a copy of the shown text becomes a library
/// document (UTF-8, LF); the file stays as it is. The id stays valid until
/// the tab closes it.
#[tauri::command]
pub async fn external_import(
    state: State<'_, AppState>,
    request: Input<ImportExternalRequest>,
) -> Result<ImportResult, CommandError> {
    let request = request.into_inner();
    let id = request.id.clone();
    let text = request.text;
    let (file_name, data) = blocking(&state.external, move |external| {
        external.import_source(&id, &text)
    })
    .await?;
    let checked = import::check(&file_name, &data).map_err(import_error)?;
    let allow_duplicate = request.allow_duplicate;
    state
        .session
        .with_documents(|documents| async move {
            if !allow_duplicate
                && let Some(existing) = documents
                    .find_duplicate(&checked.content)
                    .await
                    .map_err(document_error)?
            {
                return Ok(ImportResult::Duplicate {
                    existing: DocumentSummary::from(&existing),
                });
            }
            let meta = documents
                .import(&checked.title, checked.format, &checked.content, &file_name)
                .await
                .map_err(document_error)?;
            log::info!("external file taken into the library as {}", meta.id);
            Ok(ImportResult::Imported {
                document: DocumentSummary::from(&meta),
            })
        })
        .await
}

/// External files opened last, newest first.
#[tauri::command]
pub async fn external_recent(
    state: State<'_, AppState>,
    request: Input<EmptyRequest>,
) -> Result<Vec<RecentFileEntry>, CommandError> {
    let _ = request.into_inner();
    blocking(&state.external, |external| Ok(external.recent())).await
}

/// Opens an entry of "Zuletzt verwendet".
#[tauri::command]
pub async fn external_open_recent(
    state: State<'_, AppState>,
    request: Input<ExternalFileRequest>,
) -> Result<ExternalFileInfo, CommandError> {
    let recent = request.into_inner().id;
    blocking(&state.external, move |external| {
        let path = external.recent_path(&recent)?;
        external.register(&path)
    })
    .await
}

/// Removes an entry from "Zuletzt verwendet".
#[tauri::command]
pub async fn external_forget_recent(
    state: State<'_, AppState>,
    request: Input<ExternalFileRequest>,
) -> Result<Vec<RecentFileEntry>, CommandError> {
    let recent = request.into_inner().id;
    blocking(&state.external, move |external| {
        Ok(external.forget_recent(&recent))
    })
    .await
}

/// A file from outside, ready for a tab.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(tag = "kind", rename_all = "camelCase")]
#[ts(export)]
pub enum OpenTarget {
    /// The file is a document of the open library.
    Library {
        id: String,
    },
    External {
        file: ExternalFileInfo,
    },
    /// The file cannot be opened; `name` is its file name.
    Failed {
        name: String,
        error: CommandError,
    },
}

/// The files passed at start or by a second start, oldest first (ADR 0015,
/// decision 15). The interface calls this once it is ready and after each
/// `openRequested` event; the queue is empty afterwards.
#[tauri::command]
pub async fn open_requests_take(
    state: State<'_, AppState>,
    request: Input<EmptyRequest>,
) -> Result<Vec<OpenTarget>, CommandError> {
    let _ = request.into_inner();
    let mut targets = Vec::new();
    for path in state.opening.take() {
        let name = path
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default();
        let probe = path.clone();
        let library = state
            .session
            .with_documents(|documents| async move { Ok(documents.document_at(&probe).await) })
            .await
            .ok()
            .flatten();
        if let Some(meta) = library {
            targets.push(OpenTarget::Library {
                id: meta.id.hyphenated().to_string(),
            });
            continue;
        }
        match blocking(&state.external, move |external| external.register(&path)).await {
            Ok(file) => {
                log::info!("external file {} opened from outside", file.id);
                targets.push(OpenTarget::External { file });
            }
            Err(error) => targets.push(OpenTarget::Failed { name, error }),
        }
    }
    Ok(targets)
}

/// The folder of the newest recent file that still exists.
fn last_folder(external: &ExternalFiles) -> Option<PathBuf> {
    external
        .recent()
        .iter()
        .filter(|entry| !entry.missing)
        .find_map(|entry| external.recent_path(&entry.id).ok())
        .and_then(|path| path.parent().map(PathBuf::from))
}

fn extensions(format: DocumentFormat) -> &'static [&'static str] {
    match format {
        DocumentFormat::Txt => &["txt"],
        DocumentFormat::Md => &["md", "markdown"],
        DocumentFormat::Pwrite => &["pwrite"],
    }
}

fn open_filter_name(locale: Locale) -> &'static str {
    match locale {
        Locale::De => "Dokumente (.txt, .md, .pwrite)",
        Locale::En => "Documents (.txt, .md, .pwrite)",
    }
}

fn format_filter_name(format: DocumentFormat, locale: Locale) -> &'static str {
    match (format, locale) {
        (DocumentFormat::Txt, Locale::De) => "Textdatei (.txt)",
        (DocumentFormat::Txt, Locale::En) => "Text file (.txt)",
        (DocumentFormat::Md, _) => "Markdown (.md)",
        (DocumentFormat::Pwrite, Locale::De) => "Rich Text (.pwrite)",
        (DocumentFormat::Pwrite, Locale::En) => "Rich text (.pwrite)",
    }
}

pub(crate) fn external_error(error: ExternalError) -> CommandError {
    match error {
        ExternalError::Unknown => CommandError::new(ErrorCode::FileUnknown),
        ExternalError::Missing => CommandError::new(ErrorCode::FileMissing),
        ExternalError::UnsupportedType => CommandError::new(ErrorCode::ImportUnsupportedType),
        ExternalError::TooLarge => CommandError::new(ErrorCode::DocumentTooLarge),
        ExternalError::NotText => CommandError::new(ErrorCode::DocumentNotText),
        ExternalError::InvalidPwrite => CommandError::new(ErrorCode::PwriteInvalid),
        ExternalError::NewerFormat => CommandError::new(ErrorCode::DocumentReadOnly),
        ExternalError::Changed => CommandError::new(ErrorCode::FileChanged),
        ExternalError::ReadOnly => CommandError::new(ErrorCode::FileReadOnly),
        ExternalError::Unencodable(code) => {
            let character = char::from_u32(code).map(String::from).unwrap_or_default();
            CommandError::new(ErrorCode::FileUnencodable)
                .with_detail("character", character)
                .with_detail("codePoint", format!("U+{code:04X}"))
        }
        ExternalError::TooManyOpen => {
            CommandError::new(ErrorCode::FileSystem).with_detail("reason", "too many open files")
        }
        ExternalError::Io(error) => {
            log::warn!("external file: {error}");
            CommandError::new(ErrorCode::FileSystem)
        }
    }
}
