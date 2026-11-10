//! Media (Phase 8, ADR 0019): list, details, trash, thumbnails of PDFs,
//! opening with the standard program, and the two-step import.
//!
//! The frontend never names a path: files and folders to import come from
//! the system dialog that Rust shows, or as bytes of a file dropped onto
//! the window (raw IPC body). Paths stay in Rust.

use std::collections::BTreeMap;
use std::sync::Arc;

use base64::Engine as _;
use serde::{Deserialize, Serialize};
use tauri::State;
use tauri::ipc::{Channel, InvokeBody};
use ts_rs::TS;
use uuid::Uuid;

use crate::commands::input::{EmptyRequest, Input, Validate};
use crate::commands::structure::structure_error;
use crate::error::{CommandError, ErrorCode};
use crate::services::dialog::{FileFilter, FilePickRequest, PickRequest};
use crate::services::locale::Locale;
use crate::services::media::{
    self, Decision, ImportOutcome, ImportPreview, ImportSource, MediaError, MediaSummary, MediaUse,
    PurgeReport, SubnetPlacement, import::walk_folder,
};
use crate::services::structure::ElementKind;
use crate::state::AppState;

pub(crate) fn media_error(error: MediaError) -> CommandError {
    match error {
        MediaError::NotFound => CommandError::new(ErrorCode::MediaNotFound),
        MediaError::Refused(refusal) => {
            CommandError::new(ErrorCode::MediaTypeRefused).with_detail("reason", refusal.as_str())
        }
        MediaError::TooLarge { limit, max } => CommandError::new(ErrorCode::MediaTooLarge)
            .with_detail("limit", limit)
            .with_detail("max", max.to_string()),
        MediaError::InUse { documents } => {
            CommandError::new(ErrorCode::MediaInUse).with_detail("documents", documents.to_string())
        }
        MediaError::NotInTrash => CommandError::invalid_input("id", "not in the trash"),
        MediaError::ImportNotFound => CommandError::new(ErrorCode::ImportNotFound),
        MediaError::Cancelled => {
            CommandError::new(ErrorCode::ImportNotFound).with_detail("cancelled", "true")
        }
        MediaError::InvalidTitle(reason) => CommandError::invalid_input("title", reason),
        MediaError::InvalidDescription(reason) => {
            CommandError::invalid_input("description", reason)
        }
        MediaError::InvalidName(reason) => CommandError::invalid_input("fileName", reason),
        MediaError::InvalidValue(reason) => CommandError::invalid_input("value", reason),
        MediaError::Thumbnail(error) => CommandError::invalid_input("png", error.to_string()),
        MediaError::Path(error) => {
            log::error!("media path error: {error}");
            CommandError::new(ErrorCode::FileSystem)
        }
        MediaError::Io(error) => {
            log::error!("media file error: {error}");
            CommandError::new(ErrorCode::FileSystem)
        }
        MediaError::Database(error) => {
            log::error!("media database error: {error}");
            CommandError::new(ErrorCode::Database)
        }
        MediaError::Structure(error) => structure_error(error),
        MediaError::Settings(error) => {
            log::error!("media settings error: {error}");
            CommandError::new(ErrorCode::Internal)
        }
    }
}

fn parse_uuid(field: &str, id: &str) -> Result<Uuid, CommandError> {
    Uuid::try_parse(id)
        .ok()
        .filter(|uuid| uuid.hyphenated().to_string() == id)
        .ok_or_else(|| CommandError::invalid_input(field, "not an id"))
}

/// A request naming one medium.
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct MediaIdRequest {
    pub id: String,
}

impl Validate for MediaIdRequest {
    fn validate(&self) -> Result<(), CommandError> {
        parse_uuid("id", &self.id).map(|_| ())
    }
}

/// Media outside the trash, newest first.
#[tauri::command]
pub async fn media_list(
    state: State<'_, AppState>,
    request: Input<EmptyRequest>,
) -> Result<Vec<MediaSummary>, CommandError> {
    let _ = request.into_inner();
    state
        .session
        .with_media(|media, _| async move { media.list().await.map_err(media_error) })
        .await
}

/// Media in the trash.
#[tauri::command]
pub async fn media_trash_list(
    state: State<'_, AppState>,
    request: Input<EmptyRequest>,
) -> Result<Vec<MediaSummary>, CommandError> {
    let _ = request.into_inner();
    state
        .session
        .with_media(|media, _| async move { media.list_trash().await.map_err(media_error) })
        .await
}

#[tauri::command]
pub async fn media_get(
    state: State<'_, AppState>,
    request: Input<MediaIdRequest>,
) -> Result<MediaSummary, CommandError> {
    let id = parse_uuid("id", &request.into_inner().id)?;
    state
        .session
        .with_media(|media, _| async move { media.get(id).await.map_err(media_error) })
        .await
}

/// Request of `media_update`: the fields given change.
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct UpdateMediaRequest {
    pub id: String,
    #[serde(default)]
    #[ts(optional)]
    pub title: Option<String>,
    #[serde(default)]
    #[ts(optional)]
    pub description: Option<String>,
}

impl Validate for UpdateMediaRequest {
    fn validate(&self) -> Result<(), CommandError> {
        parse_uuid("id", &self.id)?;
        if let Some(title) = &self.title {
            media::validate_title(title).map_err(media_error)?;
        }
        if let Some(description) = &self.description {
            media::validate_description(description).map_err(media_error)?;
        }
        Ok(())
    }
}

/// Changes title and description.
#[tauri::command]
pub async fn media_update(
    state: State<'_, AppState>,
    request: Input<UpdateMediaRequest>,
) -> Result<MediaSummary, CommandError> {
    let request = request.into_inner();
    let id = parse_uuid("id", &request.id)?;
    state
        .session
        .with_media(|media, _| async move {
            media
                .update_details(id, request.title.as_deref(), request.description.as_deref())
                .await
                .map_err(media_error)
        })
        .await
}

/// Request of `media_set_favorite` and `media_archive`.
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct MediaFlagRequest {
    pub id: String,
    pub on: bool,
}

impl Validate for MediaFlagRequest {
    fn validate(&self) -> Result<(), CommandError> {
        parse_uuid("id", &self.id).map(|_| ())
    }
}

#[tauri::command]
pub async fn media_set_favorite(
    state: State<'_, AppState>,
    request: Input<MediaFlagRequest>,
) -> Result<MediaSummary, CommandError> {
    let request = request.into_inner();
    let id = parse_uuid("id", &request.id)?;
    state
        .session
        .with_media(|media, _| async move {
            media
                .set_favorite(id, request.on)
                .await
                .map_err(media_error)
        })
        .await
}

/// Records that a medium was opened in a tab ("Zuletzt verwendet").
#[tauri::command]
pub async fn media_mark_opened(
    state: State<'_, AppState>,
    request: Input<MediaIdRequest>,
) -> Result<MediaSummary, CommandError> {
    let id = parse_uuid("id", &request.into_inner().id)?;
    state
        .session
        .with_media(|media, _| async move { media.mark_opened(id).await.map_err(media_error) })
        .await
}

/// Moves a medium to the trash, a step of the structural undo.
#[tauri::command]
pub async fn media_delete(
    state: State<'_, AppState>,
    request: Input<MediaIdRequest>,
) -> Result<MediaSummary, CommandError> {
    let id = parse_uuid("id", &request.into_inner().id)?;
    state
        .session
        .with_media(|media, structure| async move {
            structure
                .set_element_deleted(ElementKind::Media, id, true)
                .await
                .map_err(structure_error)?;
            media.get(id).await.map_err(media_error)
        })
        .await
}

/// Takes a medium out of the trash.
#[tauri::command]
pub async fn media_restore(
    state: State<'_, AppState>,
    request: Input<MediaIdRequest>,
) -> Result<MediaSummary, CommandError> {
    let id = parse_uuid("id", &request.into_inner().id)?;
    state
        .session
        .with_media(|media, structure| async move {
            structure
                .set_element_deleted(ElementKind::Media, id, false)
                .await
                .map_err(structure_error)?;
            media.get(id).await.map_err(media_error)
        })
        .await
}

/// Archives a medium (`on`) or takes it out of the archive.
#[tauri::command]
pub async fn media_archive(
    state: State<'_, AppState>,
    request: Input<MediaFlagRequest>,
) -> Result<MediaSummary, CommandError> {
    let request = request.into_inner();
    let id = parse_uuid("id", &request.id)?;
    state
        .session
        .with_media(|media, structure| async move {
            structure
                .set_element_archived(ElementKind::Media, id, request.on)
                .await
                .map_err(structure_error)?;
            media.get(id).await.map_err(media_error)
        })
        .await
}

/// Removes a medium from the trash for good; refused while a document
/// uses it (`MEDIA_IN_USE`).
#[tauri::command]
pub async fn media_purge(
    state: State<'_, AppState>,
    request: Input<MediaIdRequest>,
) -> Result<(), CommandError> {
    let id = parse_uuid("id", &request.into_inner().id)?;
    state
        .session
        .with_media(|media, _| async move { media.purge(id).await.map_err(media_error) })
        .await
}

/// "Papierkorb leeren" for media; media in use stay.
#[tauri::command]
pub async fn media_empty_trash(
    state: State<'_, AppState>,
    request: Input<EmptyRequest>,
) -> Result<PurgeReport, CommandError> {
    let _ = request.into_inner();
    state
        .session
        .with_media(|media, _| async move { media.empty_trash().await.map_err(media_error) })
        .await
}

/// Documents that use a medium ("Verwendet in").
#[tauri::command]
pub async fn media_uses(
    state: State<'_, AppState>,
    request: Input<MediaIdRequest>,
) -> Result<Vec<MediaUse>, CommandError> {
    let id = parse_uuid("id", &request.into_inner().id)?;
    state
        .session
        .with_media(|media, _| async move { media.uses(id).await.map_err(media_error) })
        .await
}

/// Request of `media_set_page_count`.
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct PageCountRequest {
    pub id: String,
    pub pages: u32,
}

impl Validate for PageCountRequest {
    fn validate(&self) -> Result<(), CommandError> {
        parse_uuid("id", &self.id)?;
        if self.pages == 0 || self.pages > media::MAX_PAGES {
            return Err(CommandError::invalid_input("pages", "out of range"));
        }
        Ok(())
    }
}

/// The PDF view reports the page count of a PDF.
#[tauri::command]
pub async fn media_set_page_count(
    state: State<'_, AppState>,
    request: Input<PageCountRequest>,
) -> Result<(), CommandError> {
    let request = request.into_inner();
    let id = parse_uuid("id", &request.id)?;
    state
        .session
        .with_media(|media, _| async move {
            media
                .set_page_count(id, request.pages)
                .await
                .map_err(media_error)
        })
        .await
}

/// Request of `media_store_thumbnail`: page 1 of a PDF as PNG.
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct StoreThumbnailRequest {
    pub id: String,
    pub png_base64: String,
}

impl Validate for StoreThumbnailRequest {
    fn validate(&self) -> Result<(), CommandError> {
        parse_uuid("id", &self.id)?;
        if self.png_base64.len() > media::thumbnails::MAX_PDF_THUMBNAIL_BYTES * 4 / 3 + 4 {
            return Err(CommandError::invalid_input("pngBase64", "too large"));
        }
        Ok(())
    }
}

#[tauri::command]
pub async fn media_store_thumbnail(
    state: State<'_, AppState>,
    request: Input<StoreThumbnailRequest>,
) -> Result<(), CommandError> {
    let request = request.into_inner();
    let id = parse_uuid("id", &request.id)?;
    let png = base64::engine::general_purpose::STANDARD
        .decode(request.png_base64.as_bytes())
        .map_err(|_| CommandError::invalid_input("pngBase64", "not Base64"))?;
    state
        .session
        .with_media(|media, _| async move {
            media
                .store_pdf_thumbnail(id, &png)
                .await
                .map_err(media_error)
        })
        .await
}

/// Opens a read-only copy of a medium with the standard program of the
/// system (ADR 0019, decision 14).
#[tauri::command]
pub async fn media_open_external(
    state: State<'_, AppState>,
    request: Input<MediaIdRequest>,
) -> Result<(), CommandError> {
    let id = parse_uuid("id", &request.into_inner().id)?;
    let copy = state
        .session
        .with_media(|media, _| async move { media.open_copy(id).await.map_err(media_error) })
        .await?;
    let target = copy.to_string_lossy().into_owned();
    state.links.open(&target).map_err(|error| {
        log::warn!("medium {id} not opened with the standard program: {error}");
        CommandError::new(ErrorCode::MediaOpenFailed)
    })
}

// --- Import ------------------------------------------------------------------------------

/// Progress of preparing an import.
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ImportProgress {
    pub batch_id: String,
    pub done: u32,
    pub total: u32,
}

/// Request of `media_import_files` and `media_import_folder`: the interface
/// chooses the batch id, so it can cancel while the files are copied.
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct ImportStartRequest {
    pub batch_id: String,
    /// Title of the system dialog.
    pub title: String,
}

impl Validate for ImportStartRequest {
    fn validate(&self) -> Result<(), CommandError> {
        parse_uuid("batchId", &self.batch_id)?;
        let length = self.title.chars().count();
        if length == 0 || length > 200 || self.title.chars().any(char::is_control) {
            return Err(CommandError::invalid_input("title", "1 to 200 characters"));
        }
        Ok(())
    }
}

fn media_filter(locale: Locale) -> FileFilter {
    FileFilter {
        name: match locale {
            Locale::De => "Bilder, PDF und Office-Dateien",
            Locale::En => "Images, PDF and Office files",
        }
        .to_owned(),
        extensions: media::detect::EXTENSIONS
            .iter()
            .map(|extension| (*extension).to_owned())
            .collect(),
    }
}

fn progress_sender(
    batch: Uuid,
    channel: Channel<ImportProgress>,
) -> Arc<dyn Fn(usize, usize) + Send + Sync> {
    let batch_id = batch.hyphenated().to_string();
    Arc::new(move |done, total| {
        let _ = channel.send(ImportProgress {
            batch_id: batch_id.clone(),
            done: u32::try_from(done).unwrap_or(u32::MAX),
            total: u32::try_from(total).unwrap_or(u32::MAX),
        });
    })
}

/// "Medien importieren …": the open dialog for files, then preparing them.
/// `None` if the dialog was cancelled.
#[tauri::command]
pub async fn media_import_files(
    state: State<'_, AppState>,
    request: Input<ImportStartRequest>,
    on_progress: Channel<ImportProgress>,
) -> Result<Option<ImportPreview>, CommandError> {
    let request = request.into_inner();
    let batch = parse_uuid("batchId", &request.batch_id)?;
    let media = state.session.media().await?;
    let pick = FilePickRequest {
        title: request.title,
        start_in: None,
        file_name: None,
        filters: vec![media_filter(state.ui.locale())],
    };
    let Ok(Some(paths)) = state.media_files.pick_files(pick).await else {
        return Ok(None);
    };
    let sources: Vec<ImportSource> = paths
        .iter()
        .map(|path| ImportSource {
            name: path
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_default(),
            path: path.clone(),
            folders: Vec::new(),
        })
        .collect();
    let folder = paths
        .first()
        .and_then(|path| path.parent())
        .map(|parent| parent.display().to_string());
    media
        .prepare(
            batch,
            sources,
            Vec::new(),
            None,
            folder,
            progress_sender(batch, on_progress),
        )
        .await
        .map(Some)
        .map_err(media_error)
}

/// "Ordner importieren …": the folder dialog, then preparing everything
/// below it. `None` if the dialog was cancelled.
#[tauri::command]
pub async fn media_import_folder(
    state: State<'_, AppState>,
    request: Input<ImportStartRequest>,
    on_progress: Channel<ImportProgress>,
) -> Result<Option<ImportPreview>, CommandError> {
    let request = request.into_inner();
    let batch = parse_uuid("batchId", &request.batch_id)?;
    let media = state.session.media().await?;
    let answer = state.media_folders.pick_folder(PickRequest {
        title: request.title,
        start_in: None,
    });
    let Ok(Some(folder)) = answer.await else {
        return Ok(None);
    };
    let walked = {
        let folder = folder.clone();
        tokio::task::spawn_blocking(move || walk_folder(&folder))
            .await
            .map_err(|_| CommandError::new(ErrorCode::Internal))?
    };
    let (sources, skipped) = walked.map_err(media_error)?;
    let name = folder
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .filter(|name| !name.is_empty());
    media
        .prepare(
            batch,
            sources,
            skipped,
            name,
            Some(folder.display().to_string()),
            progress_sender(batch, on_progress),
        )
        .await
        .map(Some)
        .map_err(media_error)
}

/// Header of `media_import_bytes` with the file name (URI-encoded).
pub const FILE_NAME_HEADER: &str = "x-pw-file-name";

fn decode_uri_component(text: &str) -> Option<String> {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' {
            let hex = std::str::from_utf8(bytes.get(index + 1..index + 3)?).ok()?;
            out.push(u8::from_str_radix(hex, 16).ok()?);
            index += 3;
        } else {
            out.push(bytes[index]);
            index += 1;
        }
    }
    String::from_utf8(out).ok()
}

/// A file dropped onto the window or pasted (section 12: "Eingefügte Bilder
/// werden in die Bibliothek importiert"): its bytes as the raw request
/// body, its name in the header `x-pw-file-name`. Prepares a batch of one.
#[tauri::command]
pub async fn media_import_bytes(
    state: State<'_, AppState>,
    request: tauri::ipc::Request<'_>,
) -> Result<ImportPreview, CommandError> {
    let InvokeBody::Raw(data) = request.body() else {
        return Err(CommandError::invalid_input("body", "raw bytes expected"));
    };
    let name = request
        .headers()
        .get(FILE_NAME_HEADER)
        .and_then(|value| value.to_str().ok())
        .and_then(decode_uri_component)
        .ok_or_else(|| CommandError::invalid_input("fileName", "missing"))?;
    if data.len() > media::MAX_IPC_BYTES {
        return Err(media_error(MediaError::TooLarge {
            limit: "bytes",
            max: media::MAX_IPC_BYTES as u64,
        }));
    }
    let media = state.session.media().await?;
    media.prepare_bytes(&name, data).await.map_err(media_error)
}

/// One decision of the import dialog.
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct ImportDecision {
    pub index: u32,
    pub decision: Decision,
}

/// Request of `media_import_commit`.
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct ImportCommitRequest {
    pub batch_id: String,
    #[serde(default)]
    #[ts(optional)]
    pub decisions: Option<Vec<ImportDecision>>,
    /// What happens to duplicates without their own decision.
    pub duplicates: Decision,
    /// Subnet below which (folder structure) or into which media go.
    #[serde(default)]
    #[ts(optional)]
    pub subnet_id: Option<String>,
    /// Build subnets from the folder structure (section 9).
    #[serde(default)]
    #[ts(optional)]
    pub from_folders: Option<bool>,
}

impl Validate for ImportCommitRequest {
    fn validate(&self) -> Result<(), CommandError> {
        parse_uuid("batchId", &self.batch_id)?;
        if let Some(subnet) = &self.subnet_id {
            parse_uuid("subnetId", subnet)?;
        }
        if self
            .decisions
            .as_ref()
            .is_some_and(|decisions| decisions.len() > media::import::MAX_BATCH_FILES)
        {
            return Err(CommandError::invalid_input("decisions", "too many"));
        }
        Ok(())
    }
}

/// Imports a prepared batch.
#[tauri::command]
pub async fn media_import_commit(
    state: State<'_, AppState>,
    request: Input<ImportCommitRequest>,
) -> Result<ImportOutcome, CommandError> {
    let request = request.into_inner();
    let batch = parse_uuid("batchId", &request.batch_id)?;
    let parent = request
        .subnet_id
        .as_deref()
        .map(|id| parse_uuid("subnetId", id))
        .transpose()?;
    let decisions: BTreeMap<u32, Decision> = request
        .decisions
        .unwrap_or_default()
        .into_iter()
        .map(|entry| (entry.index, entry.decision))
        .collect();
    let placement = SubnetPlacement {
        parent,
        from_folders: request.from_folders.unwrap_or(false),
    };
    state
        .session
        .with_media(|media, _| async move {
            let outcome = media
                .commit_import(batch, &decisions, request.duplicates, &placement)
                .await
                .map_err(media_error)?;
            log::info!(
                "import {batch}: {} media ({} new)",
                outcome.items.len(),
                outcome.items.iter().filter(|item| item.created).count()
            );
            Ok(outcome)
        })
        .await
}

/// Request of `media_import_cancel`.
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct ImportCancelRequest {
    pub batch_id: String,
}

impl Validate for ImportCancelRequest {
    fn validate(&self) -> Result<(), CommandError> {
        parse_uuid("batchId", &self.batch_id).map(|_| ())
    }
}

/// Stops preparing or drops a prepared batch with its copies.
#[tauri::command]
pub async fn media_import_cancel(
    state: State<'_, AppState>,
    request: Input<ImportCancelRequest>,
) -> Result<(), CommandError> {
    let batch = parse_uuid("batchId", &request.into_inner().batch_id)?;
    let media = state.session.media().await?;
    media.cancel_import(batch);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_names_arrive_uri_encoded() {
        assert_eq!(
            decode_uri_component("Gr%C3%BC%C3%9Fe%20aus%20K%C3%B6ln.png").as_deref(),
            Some("Grüße aus Köln.png")
        );
        assert_eq!(
            decode_uri_component("plain.pdf").as_deref(),
            Some("plain.pdf")
        );
        assert_eq!(decode_uri_component("bad%2"), None);
        assert_eq!(decode_uri_component("%FF"), None);
    }
}
