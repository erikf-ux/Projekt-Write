//! Exports (Phase 10, section 21, ADR 0021): one document as TXT or
//! Markdown, the whole library as a folder. The frontend converts `.pwrite`
//! content; Rust shows the dialogs and writes the files.

use serde::Deserialize;
use tauri::State;
use ts_rs::TS;
use uuid::Uuid;

use crate::commands::input::{Input, Validate};
use crate::error::{CommandError, ErrorCode};
use crate::services::dialog::{FileDialog, FileFilter, FilePickRequest, PickRequest};
use crate::services::documents::MAX_DOCUMENT_BYTES;
use crate::services::export::{ExportError, ExportOutcome, ExportPlan, TextKind};
use crate::state::AppState;

const MAX_TITLE_CHARS: usize = 200;

pub(crate) fn export_error(error: ExportError) -> CommandError {
    match error {
        ExportError::NotFound => CommandError::new(ErrorCode::DocumentNotFound),
        ExportError::UnknownExport => CommandError::invalid_input("exportId", "no such export"),
        ExportError::TargetInsideLibrary => CommandError::new(ErrorCode::BackupTargetInsideLibrary),
        ExportError::Invalid(reason) => CommandError::invalid_input("text", reason),
        ExportError::Database(error) => {
            log::error!("export database error: {error}");
            CommandError::new(ErrorCode::Database)
        }
        ExportError::Path(error) => {
            log::error!("export path error: {error}");
            CommandError::new(ErrorCode::FileSystem)
        }
        ExportError::Io(error) => {
            log::error!("export file error: {error}");
            CommandError::new(ErrorCode::FileSystem)
        }
    }
}

fn parse_uuid(field: &str, id: &str) -> Result<Uuid, CommandError> {
    Uuid::try_parse(id)
        .ok()
        .filter(|uuid| uuid.hyphenated().to_string() == id)
        .ok_or_else(|| CommandError::invalid_input(field, "not an id"))
}

fn check_title(field: &str, title: &str) -> Result<(), CommandError> {
    let count = title.chars().count();
    if count == 0 || count > MAX_TITLE_CHARS || title.chars().any(char::is_control) {
        return Err(CommandError::invalid_input(field, "1 to 200 characters"));
    }
    Ok(())
}

fn check_text(text: &str) -> Result<(), CommandError> {
    if text.len() as u64 > MAX_DOCUMENT_BYTES {
        return Err(CommandError::invalid_input("text", "too large"));
    }
    Ok(())
}

/// Format of a document export.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum ExportFormat {
    Txt,
    Markdown,
}

/// Request of `export_document`: the content as the frontend converted it.
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct ExportDocumentRequest {
    pub id: String,
    pub format: ExportFormat,
    pub text: String,
    /// Title of the save dialog.
    pub title: String,
}

impl Validate for ExportDocumentRequest {
    fn validate(&self) -> Result<(), CommandError> {
        parse_uuid("id", &self.id)?;
        check_title("title", &self.title)?;
        check_text(&self.text)
    }
}

/// Writes one document to a file from the save dialog; `None` if cancelled.
#[tauri::command]
pub async fn export_document(
    state: State<'_, AppState>,
    request: Input<ExportDocumentRequest>,
) -> Result<Option<ExportOutcome>, CommandError> {
    let request = request.into_inner();
    let id = parse_uuid("id", &request.id)?;
    let kind = match request.format {
        ExportFormat::Txt => TextKind::Txt,
        ExportFormat::Markdown => TextKind::Markdown,
    };
    let exports = state.session.exports().await?;
    let suggested = exports
        .suggested_name(id, kind)
        .await
        .map_err(export_error)?;
    let filter = FileFilter {
        name: match kind {
            TextKind::Txt => "Text".to_owned(),
            TextKind::Markdown => "Markdown".to_owned(),
        },
        extensions: vec![kind.extension().to_owned()],
    };
    let answer = state.export_files.pick_file(
        FileDialog::Save,
        FilePickRequest {
            title: request.title,
            start_in: None,
            file_name: Some(suggested),
            filters: vec![filter],
        },
    );
    let Ok(Some(file)) = answer.await else {
        return Ok(None);
    };
    exports
        .export_document(id, &request.text, kind, &file)
        .await
        .map(Some)
        .map_err(export_error)
}

/// A request with the title of the dialog it shows.
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct ExportLibraryRequest {
    pub title: String,
}

impl Validate for ExportLibraryRequest {
    fn validate(&self) -> Result<(), CommandError> {
        check_title("title", &self.title)
    }
}

/// Starts a library export into a new folder inside the one chosen;
/// `None` if cancelled.
#[tauri::command]
pub async fn export_library_begin(
    state: State<'_, AppState>,
    request: Input<ExportLibraryRequest>,
) -> Result<Option<ExportPlan>, CommandError> {
    let request = request.into_inner();
    let exports = state.session.exports().await?;
    let answer = state.export_folders.pick_folder(PickRequest {
        title: request.title,
        start_in: None,
    });
    let Ok(Some(parent)) = answer.await else {
        return Ok(None);
    };
    exports
        .begin_library_export(&parent)
        .await
        .map(Some)
        .map_err(export_error)
}

/// Request of `export_library_add`.
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct ExportLibraryAddRequest {
    pub export_id: String,
    pub id: String,
    pub markdown: String,
}

impl Validate for ExportLibraryAddRequest {
    fn validate(&self) -> Result<(), CommandError> {
        parse_uuid("exportId", &self.export_id)?;
        parse_uuid("id", &self.id)?;
        check_text(&self.markdown)
    }
}

/// The converted Markdown of one `.pwrite` document of a library export.
#[tauri::command]
pub async fn export_library_add(
    state: State<'_, AppState>,
    request: Input<ExportLibraryAddRequest>,
) -> Result<(), CommandError> {
    let request = request.into_inner();
    let export_id = parse_uuid("exportId", &request.export_id)?;
    let id = parse_uuid("id", &request.id)?;
    state
        .session
        .exports()
        .await?
        .add_converted(export_id, id, &request.markdown)
        .await
        .map_err(export_error)
}

/// Names one library export.
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct ExportIdRequest {
    pub export_id: String,
}

impl Validate for ExportIdRequest {
    fn validate(&self) -> Result<(), CommandError> {
        parse_uuid("exportId", &self.export_id).map(|_| ())
    }
}

/// Writes the index and gives the export its name.
#[tauri::command]
pub async fn export_library_finish(
    state: State<'_, AppState>,
    request: Input<ExportIdRequest>,
) -> Result<ExportOutcome, CommandError> {
    let export_id = parse_uuid("exportId", &request.into_inner().export_id)?;
    state
        .session
        .exports()
        .await?
        .finish_library_export(export_id)
        .await
        .map_err(export_error)
}

/// Stops a library export and removes what it wrote.
#[tauri::command]
pub async fn export_library_cancel(
    state: State<'_, AppState>,
    request: Input<ExportIdRequest>,
) -> Result<bool, CommandError> {
    let export_id = parse_uuid("exportId", &request.into_inner().export_id)?;
    Ok(state
        .session
        .exports()
        .await?
        .cancel_library_export(export_id))
}

pub(crate) fn pack_error(error: crate::services::export::pack::PackError) -> CommandError {
    use crate::services::export::pack::PackError;
    match error {
        PackError::Refused(reason) => {
            log::warn!("package refused: {reason}");
            CommandError::new(ErrorCode::BackupRefused).with_detail("reason", reason)
        }
        PackError::Newer(what) => {
            CommandError::new(ErrorCode::BackupNewer).with_detail("reason", what)
        }
        PackError::NotPwrite => CommandError::invalid_input("document", "not a .pwrite document"),
        PackError::MediumRefused(reason) => {
            CommandError::new(ErrorCode::MediaTypeRefused).with_detail("reason", reason)
        }
        PackError::Document(error) => {
            log::error!("package document not created: {error}");
            CommandError::new(ErrorCode::FileSystem)
        }
        PackError::Database(error) => {
            log::error!("package database error: {error}");
            CommandError::new(ErrorCode::Database)
        }
        PackError::Io(error) => {
            log::error!("package file error: {error}");
            CommandError::new(ErrorCode::FileSystem)
        }
    }
}

/// Request of `export_pack`: the `.pwrite` file the frontend produced.
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct ExportPackRequest {
    pub id: String,
    pub document: String,
    pub title: String,
}

impl Validate for ExportPackRequest {
    fn validate(&self) -> Result<(), CommandError> {
        parse_uuid("id", &self.id)?;
        check_title("title", &self.title)?;
        check_text(&self.document)
    }
}

/// "Als Paket exportieren": a `.pwritepack` to the file of the save
/// dialog; returns the number of media, `None` if cancelled.
#[tauri::command]
pub async fn export_pack(
    state: State<'_, AppState>,
    request: Input<ExportPackRequest>,
) -> Result<Option<u32>, CommandError> {
    let request = request.into_inner();
    let id = parse_uuid("id", &request.id)?;
    let exports = state.session.exports().await?;
    let suggested = exports
        .suggested_name(id, TextKind::Markdown)
        .await
        .map_err(export_error)?;
    let stem = suggested.trim_end_matches(".md").to_owned();
    let answer = state.export_files.pick_file(
        FileDialog::Save,
        FilePickRequest {
            title: request.title,
            start_in: None,
            file_name: Some(format!(
                "{stem}.{}",
                crate::services::export::pack::EXTENSION
            )),
            filters: vec![FileFilter {
                name: "Project Write Paket".to_owned(),
                extensions: vec![crate::services::export::pack::EXTENSION.to_owned()],
            }],
        },
    );
    let Ok(Some(file)) = answer.await else {
        return Ok(None);
    };
    exports
        .export_pack(request.document.as_bytes(), &file)
        .await
        .map(Some)
        .map_err(pack_error)
}

/// "Paket importieren": a `.pwritepack` from the file dialog; `None` if
/// cancelled.
#[tauri::command]
pub async fn import_pack(
    state: State<'_, AppState>,
    request: Input<ExportLibraryRequest>,
) -> Result<Option<crate::services::export::pack::PackImport>, CommandError> {
    let request = request.into_inner();
    let media = state.session.media().await?;
    let documents = state
        .session
        .with_documents(|documents| async move { Ok(documents) })
        .await?;
    let answer = state.export_files.pick_file(
        FileDialog::Open,
        FilePickRequest {
            title: request.title,
            start_in: None,
            file_name: None,
            filters: vec![FileFilter {
                name: "Project Write Paket".to_owned(),
                extensions: vec![crate::services::export::pack::EXTENSION.to_owned()],
            }],
        },
    );
    let Ok(Some(file)) = answer.await else {
        return Ok(None);
    };
    crate::services::export::pack::import_pack(&file, &media, &documents)
        .await
        .map(Some)
        .map_err(pack_error)
}
