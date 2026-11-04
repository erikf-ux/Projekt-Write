//! Document commands (Phase 3): the Explorer lists library documents, the
//! writing surface reads and saves their text, dropped files are imported.
//!
//! Documents are referred to by id; paths never cross the boundary
//! (section 8). The text of `.txt` and `.md` documents is UTF-8.

use base64::Engine as _;
use serde::{Deserialize, Serialize};
use tauri::State;
use ts_rs::TS;
use uuid::Uuid;

use crate::commands::input::{EmptyRequest, Input, Validate};
use crate::error::{CommandError, ErrorCode};
use crate::services::documents::{
    DocumentError, DocumentFormat, DocumentMeta, MAX_DOCUMENT_BYTES, validate_title,
};
use crate::services::import::{self, ImportError, MAX_IMPORT_BYTES};
use crate::state::AppState;

/// What the interface shows of a document.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct DocumentSummary {
    pub id: String,
    pub title: String,
    pub format: DocumentFormat,
    #[ts(type = "number")]
    pub size_bytes: u64,
    pub created_at: String,
    pub updated_at: String,
    /// File name at import, for imported documents.
    pub original_name: Option<String>,
    /// Favourite since, if it is one.
    pub favorited_at: Option<String>,
    /// Last opened in a tab ("Zuletzt verwendet").
    pub last_opened_at: Option<String>,
}

impl From<&DocumentMeta> for DocumentSummary {
    fn from(meta: &DocumentMeta) -> Self {
        Self {
            id: meta.id.hyphenated().to_string(),
            title: meta.title.clone(),
            format: meta.format,
            size_bytes: meta.size_bytes,
            created_at: meta.created_at.clone(),
            updated_at: meta.updated_at.clone(),
            original_name: meta.original_name.clone(),
            favorited_at: meta.favorited_at.clone(),
            last_opened_at: meta.last_opened_at.clone(),
        }
    }
}

/// A document with its text.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct DocumentText {
    pub document: DocumentSummary,
    pub text: String,
}

/// Formats the writing surface of Phase 3 edits (`.pwrite` follows with the
/// editor, Phase 4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, TS)]
#[serde(rename_all = "lowercase")]
#[ts(export)]
pub enum TextFormat {
    Txt,
    Md,
}

impl From<TextFormat> for DocumentFormat {
    fn from(format: TextFormat) -> Self {
        match format {
            TextFormat::Txt => DocumentFormat::Txt,
            TextFormat::Md => DocumentFormat::Md,
        }
    }
}

fn parse_id(id: &str) -> Result<Uuid, CommandError> {
    Uuid::try_parse(id)
        .ok()
        .filter(|uuid| uuid.hyphenated().to_string() == id)
        .ok_or_else(|| CommandError::invalid_input("id", "not a document id"))
}

fn check_title(title: &str) -> Result<(), CommandError> {
    validate_title(title)
        .map(|_| ())
        .map_err(|error| match error {
            DocumentError::InvalidTitle(reason) => CommandError::invalid_input("title", reason),
            _ => CommandError::invalid_input("title", "invalid"),
        })
}

/// Request of `document_create`.
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct CreateDocumentRequest {
    pub title: String,
    pub format: TextFormat,
}

impl Validate for CreateDocumentRequest {
    fn validate(&self) -> Result<(), CommandError> {
        check_title(&self.title)
    }
}

/// Request of `document_read`.
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct DocumentRequest {
    pub id: String,
}

impl Validate for DocumentRequest {
    fn validate(&self) -> Result<(), CommandError> {
        parse_id(&self.id).map(|_| ())
    }
}

/// Request of `document_save`.
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct SaveDocumentRequest {
    pub id: String,
    /// The whole text; stored as UTF-8.
    pub text: String,
}

impl Validate for SaveDocumentRequest {
    fn validate(&self) -> Result<(), CommandError> {
        parse_id(&self.id)?;
        if self.text.len() as u64 > MAX_DOCUMENT_BYTES {
            return Err(CommandError::new(ErrorCode::DocumentTooLarge));
        }
        Ok(())
    }
}

/// Request of `document_rename`.
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct RenameDocumentRequest {
    pub id: String,
    pub title: String,
}

impl Validate for RenameDocumentRequest {
    fn validate(&self) -> Result<(), CommandError> {
        parse_id(&self.id)?;
        check_title(&self.title)
    }
}

/// Request of `document_copy`.
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct CopyDocumentRequest {
    pub id: String,
    /// Title of the copy (the interface proposes "… (Kopie)").
    pub title: String,
}

impl Validate for CopyDocumentRequest {
    fn validate(&self) -> Result<(), CommandError> {
        parse_id(&self.id)?;
        check_title(&self.title)
    }
}

/// Request of `document_set_favorite`.
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct SetFavoriteRequest {
    pub id: String,
    pub favorite: bool,
}

impl Validate for SetFavoriteRequest {
    fn validate(&self) -> Result<(), CommandError> {
        parse_id(&self.id).map(|_| ())
    }
}

/// Request of `document_import`.
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct ImportDocumentRequest {
    /// Name of the dropped file (no path; the webview does not reveal it).
    pub file_name: String,
    /// The file content, Base64-encoded.
    pub content_base64: String,
    /// Import even if a document with the same content exists.
    pub allow_duplicate: bool,
}

/// Longest Base64 text of an accepted file.
const MAX_IMPORT_BASE64_LEN: usize = MAX_IMPORT_BYTES.div_ceil(3) * 4;

impl Validate for ImportDocumentRequest {
    fn validate(&self) -> Result<(), CommandError> {
        import::validate_file_name(&self.file_name)
            .map_err(|error| CommandError::invalid_input("fileName", error.to_string()))?;
        if self.content_base64.len() > MAX_IMPORT_BASE64_LEN {
            return Err(CommandError::new(ErrorCode::DocumentTooLarge));
        }
        Ok(())
    }
}

/// Result of `document_import`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(tag = "kind", rename_all = "camelCase")]
#[ts(export)]
pub enum ImportResult {
    Imported {
        document: DocumentSummary,
    },
    /// A document with the same content exists; nothing was imported
    /// (section 9: ask "use the existing one or import anyway").
    Duplicate {
        existing: DocumentSummary,
    },
}

/// Documents of the open library (not in the trash), most recently changed
/// first.
#[tauri::command]
pub async fn documents_list(
    state: State<'_, AppState>,
    request: Input<EmptyRequest>,
) -> Result<Vec<DocumentSummary>, CommandError> {
    let _ = request.into_inner();
    state
        .session
        .with_documents(|documents| async move {
            let list = documents.list().await.map_err(document_error)?;
            Ok(list.iter().map(DocumentSummary::from).collect())
        })
        .await
}

/// Creates an empty document.
#[tauri::command]
pub async fn document_create(
    state: State<'_, AppState>,
    request: Input<CreateDocumentRequest>,
) -> Result<DocumentSummary, CommandError> {
    let request = request.into_inner();
    state
        .session
        .with_documents(|documents| async move {
            let meta = documents
                .create(&request.title, request.format.into(), b"")
                .await
                .map_err(document_error)?;
            Ok(DocumentSummary::from(&meta))
        })
        .await
}

/// Reads the text of a document.
#[tauri::command]
pub async fn document_read(
    state: State<'_, AppState>,
    request: Input<DocumentRequest>,
) -> Result<DocumentText, CommandError> {
    let id = parse_id(&request.into_inner().id)?;
    state
        .session
        .with_documents(|documents| async move {
            let (_, content) = documents.read(id).await.map_err(document_error)?;
            // Opening in a tab is what "Zuletzt verwendet" lists.
            let meta = documents.mark_opened(id).await.map_err(document_error)?;
            if meta.format == DocumentFormat::Pwrite {
                return Err(CommandError::new(ErrorCode::DocumentFormatUnsupported));
            }
            let text = String::from_utf8(content)
                .map_err(|_| CommandError::new(ErrorCode::DocumentNotText))?;
            Ok(DocumentText {
                document: DocumentSummary::from(&meta),
                text,
            })
        })
        .await
}

/// Saves the text of a document (unchanged text is not written again).
#[tauri::command]
pub async fn document_save(
    state: State<'_, AppState>,
    request: Input<SaveDocumentRequest>,
) -> Result<DocumentSummary, CommandError> {
    let request = request.into_inner();
    let id = parse_id(&request.id)?;
    state
        .session
        .with_documents(|documents| async move {
            let meta = documents.get(id).await.map_err(document_error)?;
            match meta {
                Some(meta) if meta.format == DocumentFormat::Pwrite => {
                    return Err(CommandError::new(ErrorCode::DocumentFormatUnsupported));
                }
                Some(meta) if meta.deleted_at.is_some() => {
                    return Err(CommandError::new(ErrorCode::DocumentNotFound));
                }
                Some(_) => {}
                None => return Err(CommandError::new(ErrorCode::DocumentNotFound)),
            }
            let outcome = documents
                .save(id, request.text.as_bytes())
                .await
                .map_err(document_error)?;
            Ok(DocumentSummary::from(outcome.meta()))
        })
        .await
}

/// Changes the title (only the title, section 13).
#[tauri::command]
pub async fn document_rename(
    state: State<'_, AppState>,
    request: Input<RenameDocumentRequest>,
) -> Result<DocumentSummary, CommandError> {
    let request = request.into_inner();
    let id = parse_id(&request.id)?;
    state
        .session
        .with_documents(|documents| async move {
            let meta = documents
                .rename(id, &request.title)
                .await
                .map_err(document_error)?;
            Ok(DocumentSummary::from(&meta))
        })
        .await
}

/// Copies a document: new id, same format and content (section 13).
#[tauri::command]
pub async fn document_copy(
    state: State<'_, AppState>,
    request: Input<CopyDocumentRequest>,
) -> Result<DocumentSummary, CommandError> {
    let request = request.into_inner();
    let id = parse_id(&request.id)?;
    state
        .session
        .with_documents(|documents| async move {
            let meta = documents
                .copy(id, &request.title)
                .await
                .map_err(document_error)?;
            log::info!("document {id} copied to {}", meta.id);
            Ok(DocumentSummary::from(&meta))
        })
        .await
}

/// Makes a document a favourite or not (section 13).
#[tauri::command]
pub async fn document_set_favorite(
    state: State<'_, AppState>,
    request: Input<SetFavoriteRequest>,
) -> Result<DocumentSummary, CommandError> {
    let request = request.into_inner();
    let id = parse_id(&request.id)?;
    state
        .session
        .with_documents(|documents| async move {
            let meta = documents
                .set_favorite(id, request.favorite)
                .await
                .map_err(document_error)?;
            Ok(DocumentSummary::from(&meta))
        })
        .await
}

/// Imports a dropped text file as a library document (section 9).
#[tauri::command]
pub async fn document_import(
    state: State<'_, AppState>,
    request: Input<ImportDocumentRequest>,
) -> Result<ImportResult, CommandError> {
    let request = request.into_inner();
    let data = base64::engine::general_purpose::STANDARD
        .decode(request.content_base64.as_bytes())
        .map_err(|_| CommandError::invalid_input("contentBase64", "not Base64"))?;
    let checked = import::check(&request.file_name, &data).map_err(import_error)?;
    state
        .session
        .with_documents(|documents| async move {
            if !request.allow_duplicate
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
                .import(
                    &checked.title,
                    checked.format,
                    &checked.content,
                    &request.file_name,
                )
                .await
                .map_err(document_error)?;
            log::info!("document {} imported", meta.id);
            Ok(ImportResult::Imported {
                document: DocumentSummary::from(&meta),
            })
        })
        .await
}

fn import_error(error: ImportError) -> CommandError {
    match error {
        ImportError::UnsupportedType => CommandError::new(ErrorCode::ImportUnsupportedType),
        ImportError::NotText(reason) => {
            CommandError::new(ErrorCode::DocumentNotText).with_detail("reason", reason)
        }
        ImportError::TooLarge => CommandError::new(ErrorCode::DocumentTooLarge),
        ImportError::InvalidName(reason) => CommandError::invalid_input("fileName", reason),
    }
}

fn document_error(error: DocumentError) -> CommandError {
    match error {
        DocumentError::NotFound | DocumentError::InTrash => {
            CommandError::new(ErrorCode::DocumentNotFound)
        }
        DocumentError::InvalidTitle(reason) => CommandError::invalid_input("title", reason),
        DocumentError::TooLarge => CommandError::new(ErrorCode::DocumentTooLarge),
        DocumentError::Path(error) => {
            log::error!("unsafe document path: {error}");
            CommandError::new(ErrorCode::LibraryUnsafePath).with_detail("path", "documents")
        }
        DocumentError::InvalidRow(reason) => {
            log::error!("invalid document row: {reason}");
            CommandError::new(ErrorCode::Database)
        }
        DocumentError::Io(error) => {
            log::error!("document file error: {error}");
            CommandError::new(ErrorCode::FileSystem)
        }
        DocumentError::Database(error) => {
            log::error!("document database error: {error}");
            CommandError::new(ErrorCode::Database)
        }
    }
}
