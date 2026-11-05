//! Recovery commands (stage 2, "Absturz"; ADR 0016, decisions 8 and 9):
//! write and discard the draft of a tab, list the drafts after a start,
//! restore or drop them.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use tauri::State;
use ts_rs::TS;
use uuid::Uuid;

use crate::commands::documents::{DocumentSummary, document_error};
use crate::commands::external::external_error;
use crate::commands::input::{EmptyRequest, Input, Validate};
use crate::error::{CommandError, ErrorCode};
use crate::services::documents::{DocumentFormat, DocumentRepository, MAX_DOCUMENT_BYTES};
use crate::services::encoding::{self, LineEnding, TextEncoding};
use crate::services::external::{ExternalFileInfo, ExternalFiles, format_of_path};
use crate::services::pwrite;
use crate::services::recovery::{
    Draft, DraftOwner, RecoveryError, external_key, library_key, valid_key,
};
use crate::state::AppState;

/// The tab a draft belongs to.
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub enum DraftTarget {
    /// A library document by its id.
    Library { id: String },
    /// An external file by its session id.
    External { id: String },
}

impl DraftTarget {
    fn validate(&self) -> Result<(), CommandError> {
        match self {
            DraftTarget::Library { id } => Uuid::try_parse(id)
                .ok()
                .filter(|uuid| uuid.hyphenated().to_string() == *id)
                .map(|_| ())
                .ok_or_else(|| CommandError::invalid_input("id", "not a document id")),
            DraftTarget::External { id } => {
                let valid =
                    id.len() == 32 && id.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'));
                if valid {
                    Ok(())
                } else {
                    Err(CommandError::invalid_input("id", "not a file id"))
                }
            }
        }
    }
}

/// Request of `recovery_write`.
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct WriteDraftRequest {
    pub target: DraftTarget,
    /// Text with LF line ends; `.pwrite`: the editor content as JSON.
    pub text: String,
    /// External files: how the text would be saved.
    #[ts(optional)]
    pub encoding: Option<TextEncoding>,
    #[ts(optional)]
    pub line_ending: Option<LineEnding>,
}

impl Validate for WriteDraftRequest {
    fn validate(&self) -> Result<(), CommandError> {
        self.target.validate()?;
        if self.text.len() as u64 > MAX_DOCUMENT_BYTES {
            return Err(CommandError::new(ErrorCode::DocumentTooLarge));
        }
        Ok(())
    }
}

/// Request of `recovery_discard`.
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct DiscardDraftRequest {
    pub target: DraftTarget,
}

impl Validate for DiscardDraftRequest {
    fn validate(&self) -> Result<(), CommandError> {
        self.target.validate()
    }
}

/// Request of `recovery_restore` and `recovery_drop`.
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct DraftRequest {
    /// Key from `recovery_list`.
    pub key: String,
}

impl Validate for DraftRequest {
    fn validate(&self) -> Result<(), CommandError> {
        if valid_key(&self.key) {
            Ok(())
        } else {
            Err(CommandError::invalid_input("key", "not a draft key"))
        }
    }
}

/// A draft offered after a start.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct DraftEntry {
    pub key: String,
    /// `library` or `external`.
    pub external: bool,
    /// Title of the document or name of the file.
    pub title: String,
    pub format: DocumentFormat,
    pub updated_at: String,
    /// The external file is gone; restoring creates a library document.
    pub missing: bool,
}

/// What a restored draft became.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(tag = "kind", rename_all = "camelCase")]
#[ts(export)]
pub enum RestoredDraft {
    /// The library document holds the draft now (the old state is a version).
    Library { document: DocumentSummary },
    /// The external file opens with the draft as unsaved change.
    External {
        file: ExternalFileInfo,
        text: String,
    },
    /// The external file was gone: the draft became a library document.
    Created { document: DocumentSummary },
}

fn recovery_error(error: RecoveryError) -> CommandError {
    match error {
        RecoveryError::NotFound => CommandError::invalid_input("key", "no such draft"),
        RecoveryError::TooLarge => CommandError::new(ErrorCode::DocumentTooLarge),
        RecoveryError::Invalid(reason) => {
            log::warn!("recovery draft not readable: {reason}");
            CommandError::new(ErrorCode::FileSystem)
        }
        RecoveryError::Io(error) => {
            log::error!("recovery draft: {error}");
            CommandError::new(ErrorCode::FileSystem)
        }
    }
}

fn parse_id(id: &str) -> Result<Uuid, CommandError> {
    Uuid::try_parse(id).map_err(|_| CommandError::invalid_input("id", "not a document id"))
}

fn external_path(external: &ExternalFiles, id: &str) -> Result<PathBuf, CommandError> {
    external.path(id).map_err(external_error)
}

/// Writes the draft of a tab (at most once a second while it has changes).
#[tauri::command]
pub async fn recovery_write(
    state: State<'_, AppState>,
    request: Input<WriteDraftRequest>,
) -> Result<(), CommandError> {
    let request = request.into_inner();
    let now = state.session.now();
    match request.target {
        DraftTarget::Library { id } => {
            let id = parse_id(&id)?;
            let text = request.text;
            state
                .session
                .with_documents(|documents| async move {
                    let meta = documents
                        .get(id)
                        .await
                        .map_err(document_error)?
                        .ok_or_else(|| CommandError::new(ErrorCode::DocumentNotFound))?;
                    let mut draft = Draft::new(
                        DraftOwner::Library {
                            document_id: library_key(id),
                        },
                        meta.format,
                        now,
                        text,
                    );
                    draft.base_sha256 = Some(meta.content_sha256);
                    write_blocking(documents, draft).await
                })
                .await
        }
        DraftTarget::External { id } => {
            let path = external_path(&state.external, &id)?;
            let format = format_of_path(&path).map_err(external_error)?;
            let mut draft = Draft::new(
                DraftOwner::External {
                    path: path.to_string_lossy().into_owned(),
                },
                format,
                now,
                request.text,
            );
            draft.encoding = request.encoding;
            draft.line_ending = request.line_ending;
            state
                .session
                .with_documents(|documents| async move { write_blocking(documents, draft).await })
                .await
        }
    }
}

async fn write_blocking(documents: DocumentRepository, draft: Draft) -> Result<(), CommandError> {
    let drafts = documents.drafts();
    tauri::async_runtime::spawn_blocking(move || drafts.write(&draft))
        .await
        .map_err(|_| CommandError::new(ErrorCode::Internal))?
        .map_err(recovery_error)
}

/// The tab was saved or its changes discarded: its draft goes.
#[tauri::command]
pub async fn recovery_discard(
    state: State<'_, AppState>,
    request: Input<DiscardDraftRequest>,
) -> Result<(), CommandError> {
    let key = match request.into_inner().target {
        DraftTarget::Library { id } => library_key(parse_id(&id)?),
        DraftTarget::External { id } => external_key(&external_path(&state.external, &id)?),
    };
    state
        .session
        .with_documents(|documents| async move {
            documents.drafts().remove(&key).map_err(recovery_error)
        })
        .await
}

/// The drafts that differ from the saved state (after a start). Drafts that
/// match it, of documents that are gone, or of files open in a tab are not
/// offered; the first two are removed.
#[tauri::command]
pub async fn recovery_list(
    state: State<'_, AppState>,
    request: Input<EmptyRequest>,
) -> Result<Vec<DraftEntry>, CommandError> {
    let _ = request.into_inner();
    let external = state.external.clone();
    state
        .session
        .with_documents(|documents| async move {
            let drafts = documents.drafts();
            let mut entries = Vec::new();
            for draft in drafts.all() {
                let key = draft.key();
                let entry = match &draft.owner {
                    DraftOwner::Library { document_id } => {
                        library_entry(&documents, &draft, document_id).await
                    }
                    DraftOwner::External { path } => {
                        if external.is_open(Path::new(path)) {
                            continue;
                        }
                        external_entry(&draft, Path::new(path))
                    }
                };
                match entry {
                    Some(entry) => entries.push(entry),
                    None => {
                        if let Err(error) = drafts.remove(&key) {
                            log::warn!("recovery draft {key} not removed: {error}");
                        }
                    }
                }
            }
            Ok(entries)
        })
        .await
}

async fn library_entry(
    documents: &DocumentRepository,
    draft: &Draft,
    document_id: &str,
) -> Option<DraftEntry> {
    let id = Uuid::try_parse(document_id).ok()?;
    let meta = documents.get(id).await.ok()??;
    if meta.deleted_at.is_some()
        || documents
            .matches_stored(id, &draft.text)
            .await
            .unwrap_or(true)
    {
        return None;
    }
    Some(DraftEntry {
        key: draft.key(),
        external: false,
        title: meta.title,
        format: meta.format,
        updated_at: draft.updated_at.clone(),
        missing: false,
    })
}

fn external_entry(draft: &Draft, path: &Path) -> Option<DraftEntry> {
    let name = path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();
    let missing = !path.is_file();
    if !missing && external_matches(draft, path) {
        return None;
    }
    Some(DraftEntry {
        key: draft.key(),
        external: true,
        title: name,
        format: draft.format,
        updated_at: draft.updated_at.clone(),
        missing,
    })
}

/// Whether the draft text is what the external file holds now.
fn external_matches(draft: &Draft, path: &Path) -> bool {
    let Ok(data) = std::fs::read(path) else {
        return false;
    };
    if draft.format == DocumentFormat::Pwrite {
        let Ok(text) = serde_json::from_str::<serde_json::Value>(&draft.text) else {
            return false;
        };
        return pwrite::parse(&data).is_ok_and(|parsed| *parsed.content() == text);
    }
    encoding::decode(&data).is_ok_and(|decoded| decoded.text == draft.text)
}

/// Restores a draft (stage 2): a library document saves it after keeping
/// the current state as a version; an external file opens with it as an
/// unsaved change; a vanished external file becomes a library document.
#[tauri::command]
pub async fn recovery_restore(
    state: State<'_, AppState>,
    request: Input<DraftRequest>,
) -> Result<RestoredDraft, CommandError> {
    let key = request.into_inner().key;
    let external = state.external.clone();
    let language = pwrite::language_tag(state.ui.locale());
    state
        .session
        .with_documents(|documents| async move {
            let drafts = documents.drafts();
            let draft = drafts.read(&key).map_err(recovery_error)?;
            match &draft.owner {
                DraftOwner::Library { document_id } => {
                    let id = parse_id(document_id)?;
                    let meta = documents
                        .apply_draft(id, &draft.text)
                        .await
                        .map_err(document_error)?;
                    drafts.remove(&key).map_err(recovery_error)?;
                    Ok(RestoredDraft::Library {
                        document: DocumentSummary::from(&meta),
                    })
                }
                DraftOwner::External { path } => {
                    let path = PathBuf::from(path);
                    if path.is_file() {
                        let file = external.register(&path).map_err(external_error)?;
                        // The draft stays until the tab is saved or discarded.
                        return Ok(RestoredDraft::External {
                            file,
                            text: draft.text.clone(),
                        });
                    }
                    let title = path
                        .file_stem()
                        .map(|stem| stem.to_string_lossy().into_owned())
                        .filter(|stem| !stem.trim().is_empty())
                        .unwrap_or_else(|| "Wiederhergestellt".to_owned());
                    let meta = if draft.format == DocumentFormat::Pwrite {
                        let content: serde_json::Value = serde_json::from_str(&draft.text)
                            .map_err(|_| CommandError::new(ErrorCode::PwriteInvalid))?;
                        documents.create_pwrite(&title, language, content).await
                    } else {
                        documents
                            .create(&title, draft.format, draft.text.as_bytes())
                            .await
                    }
                    .map_err(document_error)?;
                    drafts.remove(&key).map_err(recovery_error)?;
                    log::info!("draft of a vanished file restored as document {}", meta.id);
                    Ok(RestoredDraft::Created {
                        document: DocumentSummary::from(&meta),
                    })
                }
            }
        })
        .await
}

/// "Verwerfen" in the recovery question: the draft goes.
#[tauri::command]
pub async fn recovery_drop(
    state: State<'_, AppState>,
    request: Input<DraftRequest>,
) -> Result<(), CommandError> {
    let key = request.into_inner().key;
    state
        .session
        .with_documents(|documents| async move {
            documents.drafts().remove(&key).map_err(recovery_error)
        })
        .await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_of_both_kinds_are_accepted() {
        let id = Uuid::new_v4();
        assert!(valid_key(&library_key(id)));
        assert!(valid_key(&external_key(Path::new("/a"))));
    }
}
