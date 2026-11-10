//! Importing media (section 9, "Import in die Bibliothek"; ADR 0019,
//! decisions 5 and 6), in two steps:
//!
//! 1. **Prepare.** Every file is opened for reading only, without following a
//!    link, and copied to `tmp/import-<batch>/`; the copy is hashed while it
//!    is written and its type checked by content. The original is never
//!    opened for writing. Duplicates (same SHA-256 as a medium outside the
//!    trash, or as an earlier file of the batch) are marked for the dialog.
//! 2. **Commit.** A journal in `tmp/` names the files about to appear in
//!    `media/`. Within one transaction the copies are renamed into place and
//!    the rows (and memberships) written; then the transaction commits and
//!    the journal goes. On an error the renamed files go back; after a crash
//!    the next opening deletes the files of the journal that have no row.
//!
//! Cancelling removes the batch: an aborted import leaves neither files nor
//! rows.

use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use ts_rs::TS;
use uuid::Uuid;

use super::detect::{self, DetectError, MediaKind, MediaType, Refusal};
use super::thumbnails;
use super::{
    ImportJournal, JournalFile, MAX_FILE_BYTES, MediaError, MediaLibrary, id_text,
    journal_relative, relative_path, title_from_name,
};
use crate::database::media::{self as rows, MediaRow};
use crate::database::structure::Element;
use crate::filesystem::atomic::{create_new_private, open_read_no_follow, sync_dir, write_atomic};
use crate::services::library::layout::TMP_DIR;
use crate::services::structure;

/// Most files one batch takes (a folder with more is refused).
pub const MAX_BATCH_FILES: usize = 10_000;
/// Deepest folder level a folder import walks into.
pub const MAX_FOLDER_DEPTH: usize = 32;
const COPY_CHUNK: usize = 1024 * 1024;

/// Document files a folder may contain; listed as skipped (ADR 0019,
/// decision 6).
const DOCUMENT_EXTENSIONS: &[&str] = &["txt", "md", "markdown", "pwrite"];

/// A file to import: where it is, the name it had and the folders below the
/// chosen import folder.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportSource {
    pub path: PathBuf,
    pub name: String,
    pub folders: Vec<String>,
}

/// Why a file is not part of the import.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum SkipReason {
    /// Not on the positive list.
    Type,
    /// The extension does not belong to the content.
    Extension,
    /// Office file with macros.
    Macro,
    /// The content is not valid for its type.
    Content,
    /// Larger than the file limit.
    TooLarge,
    /// An image above the megapixel limit.
    Megapixels,
    /// A text document (imported by dropping, not with media).
    Document,
    /// A symbolic link.
    Link,
    /// Could not be read.
    Unreadable,
}

impl From<Refusal> for SkipReason {
    fn from(refusal: Refusal) -> Self {
        match refusal {
            Refusal::Type => SkipReason::Type,
            Refusal::Extension => SkipReason::Extension,
            Refusal::Macro => SkipReason::Macro,
            Refusal::Content => SkipReason::Content,
        }
    }
}

/// A file that is not imported, and why.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct SkippedItem {
    pub name: String,
    /// Folders below the import folder, joined with `/`.
    pub folder: String,
    pub reason: SkipReason,
}

/// What a file of the batch duplicates.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase", tag = "kind")]
#[ts(export)]
pub enum Duplicate {
    /// A medium of the library with the same content.
    Existing { id: String, title: String },
    /// An earlier file of the same batch.
    InBatch { index: u32 },
}

/// A file ready to be imported.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct PreviewItem {
    pub index: u32,
    pub name: String,
    pub folder: String,
    pub kind: MediaKind,
    #[ts(type = "number")]
    pub size_bytes: u64,
    pub duplicate: Option<Duplicate>,
}

/// Result of preparing: what the dialog shows.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ImportPreview {
    pub batch_id: String,
    pub items: Vec<PreviewItem>,
    pub skipped: Vec<SkippedItem>,
    /// Name of the chosen folder (folder import).
    pub folder_name: Option<String>,
}

/// What to do with a file of the batch.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum Decision {
    Import,
    /// "Vorhandenes Element verwenden" (duplicates only).
    UseExisting,
    Skip,
}

/// Where imported media are placed in the structure.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SubnetPlacement {
    /// Subnet below which (folder structure) or into which media go;
    /// `None`: the top level (folder structure) or no subnet.
    pub parent: Option<Uuid>,
    /// Build subnets from the folder structure (section 9).
    pub from_folders: bool,
}

/// A file of the batch after the import.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ImportedItem {
    pub index: u32,
    /// The new medium, or the existing one used instead.
    pub id: String,
    pub created: bool,
}

/// Result of committing.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ImportOutcome {
    pub items: Vec<ImportedItem>,
    /// The subnet of the import folder, when subnets were made from folders.
    pub subnet_id: Option<String>,
}

/// A staged file.
#[derive(Debug, Clone)]
struct Staged {
    name: String,
    folders: Vec<String>,
    /// Copy in `tmp/import-<batch>/`.
    copy: PathBuf,
    size: u64,
    sha256: String,
    media_type: MediaType,
    extension: String,
    width: Option<u32>,
    height: Option<u32>,
    duplicate: Option<Duplicate>,
}

#[derive(Debug)]
struct Batch {
    items: Vec<Staged>,
    folder_name: Option<String>,
    import_folder: Option<String>,
}

/// Prepared batches and running preparations of one open library.
#[derive(Debug, Default)]
pub struct Batches {
    prepared: HashMap<Uuid, Batch>,
    running: HashMap<Uuid, Arc<AtomicBool>>,
}

fn batch_dir(batch: Uuid) -> String {
    format!("{TMP_DIR}/import-{}", id_text(batch))
}

/// Walks a folder for import: files below it with their folders, and what
/// is skipped on the way (links, hidden entries are left out; documents and
/// files off the positive list are listed).
pub fn walk_folder(folder: &Path) -> Result<(Vec<ImportSource>, Vec<SkippedItem>), MediaError> {
    let mut sources = Vec::new();
    let mut skipped = Vec::new();
    let mut stack: Vec<(PathBuf, Vec<String>)> = vec![(folder.to_path_buf(), Vec::new())];
    while let Some((dir, folders)) = stack.pop() {
        let mut entries: Vec<_> = fs::read_dir(&dir)?.flatten().collect();
        entries.sort_by_key(fs::DirEntry::file_name);
        let mut below = Vec::new();
        for entry in entries {
            let name = entry.file_name().to_string_lossy().into_owned();
            if name.starts_with('.') {
                continue;
            }
            let Ok(metadata) = fs::symlink_metadata(entry.path()) else {
                continue;
            };
            let joined = folders.join("/");
            if metadata.file_type().is_symlink() {
                skipped.push(SkippedItem {
                    name,
                    folder: joined,
                    reason: SkipReason::Link,
                });
            } else if metadata.is_dir() {
                if folders.len() < MAX_FOLDER_DEPTH {
                    let mut next = folders.clone();
                    next.push(name);
                    below.push((entry.path(), next));
                }
            } else if metadata.is_file() {
                let extension = detect::extension_of(&name).unwrap_or_default();
                let reason = if DOCUMENT_EXTENSIONS.contains(&extension.as_str()) {
                    Some(SkipReason::Document)
                } else if detect::MACRO_EXTENSIONS.contains(&extension.as_str()) {
                    Some(SkipReason::Macro)
                } else if !detect::EXTENSIONS.contains(&extension.as_str()) {
                    Some(SkipReason::Type)
                } else if metadata.len() > MAX_FILE_BYTES {
                    Some(SkipReason::TooLarge)
                } else {
                    None
                };
                match reason {
                    Some(reason) => skipped.push(SkippedItem {
                        name,
                        folder: joined,
                        reason,
                    }),
                    None => sources.push(ImportSource {
                        path: entry.path(),
                        name,
                        folders: folders.clone(),
                    }),
                }
                if sources.len() > MAX_BATCH_FILES {
                    return Err(MediaError::TooLarge {
                        limit: "files",
                        max: MAX_BATCH_FILES as u64,
                    });
                }
            }
        }
        // Depth first, in name order.
        below.reverse();
        stack.extend(below);
    }
    Ok((sources, skipped))
}

enum StageError {
    Skip(SkipReason),
    Cancelled,
    Io(io::Error),
}

/// Copies `source` to `target` while hashing it. The original is opened
/// read-only and never followed through a link.
fn stage_copy(
    source: &Path,
    target: &Path,
    max_bytes: u64,
    cancel: &AtomicBool,
) -> Result<(u64, String), StageError> {
    match fs::symlink_metadata(source) {
        Ok(metadata) if metadata.file_type().is_symlink() => {
            return Err(StageError::Skip(SkipReason::Link));
        }
        Ok(metadata) if metadata.len() > max_bytes => {
            return Err(StageError::Skip(SkipReason::TooLarge));
        }
        Ok(_) => {}
        Err(_) => return Err(StageError::Skip(SkipReason::Unreadable)),
    }
    let mut input =
        open_read_no_follow(source).map_err(|_| StageError::Skip(SkipReason::Unreadable))?;
    let mut output = create_new_private(target).map_err(StageError::Io)?;
    let mut hasher = Sha256::new();
    let mut buffer = vec![0u8; COPY_CHUNK];
    let mut size: u64 = 0;
    loop {
        if cancel.load(Ordering::Relaxed) {
            return Err(StageError::Cancelled);
        }
        let read = match input.read(&mut buffer) {
            Ok(0) => break,
            Ok(read) => read,
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            Err(_) => return Err(StageError::Skip(SkipReason::Unreadable)),
        };
        size += read as u64;
        if size > max_bytes {
            return Err(StageError::Skip(SkipReason::TooLarge));
        }
        hasher.update(&buffer[..read]);
        output.write_all(&buffer[..read]).map_err(StageError::Io)?;
    }
    output.sync_all().map_err(StageError::Io)?;
    let digest = hasher.finalize();
    let mut hex = String::with_capacity(64);
    for byte in digest {
        use std::fmt::Write as _;
        let _ = write!(hex, "{byte:02x}");
    }
    Ok((size, hex))
}

/// Checks a staged copy: type by content, pixels of raster images.
fn check_copy(
    copy: &Path,
    name: &str,
    max_megapixels: u32,
) -> Result<(MediaType, Option<(u32, u32)>), SkipReason> {
    let media_type = match detect::detect(copy, name) {
        Ok(media_type) => media_type,
        Err(DetectError::Refused(refusal)) => return Err(refusal.into()),
        Err(DetectError::SvgTooLarge) => return Err(SkipReason::TooLarge),
        Err(DetectError::Io(_)) => return Err(SkipReason::Unreadable),
    };
    if !media_type.is_raster() {
        return Ok((media_type, None));
    }
    let (width, height) = thumbnails::dimensions(copy).map_err(|_| SkipReason::Content)?;
    if width == 0 || height == 0 {
        return Err(SkipReason::Content);
    }
    if !thumbnails::within_megapixels(width, height, max_megapixels) {
        return Err(SkipReason::Megapixels);
    }
    Ok((media_type, Some((width, height))))
}

impl MediaLibrary {
    /// Prepares a batch from files or a folder. `batch` comes from the
    /// interface, so it can cancel while the files are copied; `progress`
    /// hears (done, total) after each file.
    pub async fn prepare(
        &self,
        batch: Uuid,
        sources: Vec<ImportSource>,
        mut skipped: Vec<SkippedItem>,
        folder_name: Option<String>,
        import_folder: Option<String>,
        progress: Arc<dyn Fn(usize, usize) + Send + Sync>,
    ) -> Result<ImportPreview, MediaError> {
        if sources.len() > MAX_BATCH_FILES {
            return Err(MediaError::TooLarge {
                limit: "files",
                max: MAX_BATCH_FILES as u64,
            });
        }
        let cancel = Arc::new(AtomicBool::new(false));
        {
            let mut batches = self.batches();
            if batches.prepared.contains_key(&batch) || batches.running.contains_key(&batch) {
                return Err(MediaError::InvalidValue("batch id in use"));
            }
            batches.running.insert(batch, Arc::clone(&cancel));
        }
        let result = self
            .prepare_running(
                batch,
                sources,
                &mut skipped,
                folder_name,
                import_folder,
                progress,
                &cancel,
            )
            .await;
        self.batches().running.remove(&batch);
        match result {
            Ok(preview) => Ok(ImportPreview { skipped, ..preview }),
            Err(error) => {
                self.batches().prepared.remove(&batch);
                self.remove_batch_files(batch);
                Err(error)
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    async fn prepare_running(
        &self,
        batch: Uuid,
        sources: Vec<ImportSource>,
        skipped: &mut Vec<SkippedItem>,
        folder_name: Option<String>,
        import_folder: Option<String>,
        progress: Arc<dyn Fn(usize, usize) + Send + Sync>,
        cancel: &Arc<AtomicBool>,
    ) -> Result<ImportPreview, MediaError> {
        let dir = self.root.create_dir_all(&batch_dir(batch))?;
        let max_megapixels = self.max_megapixels().await;
        let total = sources.len();
        let stage_cancel = Arc::clone(cancel);
        let staged = tokio::task::spawn_blocking(move || {
            let mut staged = Vec::new();
            let mut skipped = Vec::new();
            for (index, source) in sources.into_iter().enumerate() {
                let copy = dir.join(index.to_string());
                let folder = source.folders.join("/");
                let skip = |reason| SkippedItem {
                    name: source.name.clone(),
                    folder: folder.clone(),
                    reason,
                };
                match stage_copy(&source.path, &copy, MAX_FILE_BYTES, &stage_cancel) {
                    Ok((size, sha256)) => match check_copy(&copy, &source.name, max_megapixels) {
                        Ok((media_type, dimensions)) => {
                            let extension = detect::extension_of(&source.name).unwrap_or_default();
                            staged.push(Staged {
                                name: source.name.clone(),
                                folders: source.folders.clone(),
                                copy,
                                size,
                                sha256,
                                media_type,
                                extension,
                                width: dimensions.map(|(width, _)| width),
                                height: dimensions.map(|(_, height)| height),
                                duplicate: None,
                            });
                        }
                        Err(reason) => {
                            let _ = fs::remove_file(&copy);
                            skipped.push(skip(reason));
                        }
                    },
                    Err(StageError::Skip(reason)) => {
                        let _ = fs::remove_file(&copy);
                        skipped.push(skip(reason));
                    }
                    Err(StageError::Cancelled) => return Err(MediaError::Cancelled),
                    Err(StageError::Io(error)) => return Err(MediaError::Io(error)),
                }
                progress(index + 1, total);
            }
            Ok((staged, skipped))
        })
        .await
        .map_err(|error| io::Error::other(error.to_string()))??;
        let (mut items, more_skipped) = staged;
        skipped.extend(more_skipped);
        if cancel.load(Ordering::Relaxed) {
            return Err(MediaError::Cancelled);
        }

        // Duplicates: in the library, then within the batch.
        let mut connection = self.database.reader().acquire().await?;
        let mut first_with: HashMap<String, usize> = HashMap::new();
        for (index, item) in items.iter_mut().enumerate() {
            if let Some(existing) =
                rows::find_active_by_sha256(&mut connection, &item.sha256).await?
            {
                item.duplicate = Some(Duplicate::Existing {
                    id: existing.id,
                    title: existing.title,
                });
            } else if let Some(first) = first_with.get(&item.sha256) {
                item.duplicate = Some(Duplicate::InBatch {
                    index: u32::try_from(*first).unwrap_or(u32::MAX),
                });
            } else {
                first_with.insert(item.sha256.clone(), index);
            }
        }
        drop(connection);

        let preview = ImportPreview {
            batch_id: id_text(batch),
            items: items
                .iter()
                .enumerate()
                .map(|(index, item)| PreviewItem {
                    index: u32::try_from(index).unwrap_or(u32::MAX),
                    name: item.name.clone(),
                    folder: item.folders.join("/"),
                    kind: item.media_type.kind(),
                    size_bytes: item.size,
                    duplicate: item.duplicate.clone(),
                })
                .collect(),
            skipped: Vec::new(),
            folder_name: folder_name.clone(),
        };
        self.batches().prepared.insert(
            batch,
            Batch {
                items,
                folder_name,
                import_folder,
            },
        );
        Ok(preview)
    }

    /// Prepares one file dropped or pasted into the window (its bytes).
    pub async fn prepare_bytes(
        &self,
        name: &str,
        data: &[u8],
    ) -> Result<ImportPreview, MediaError> {
        crate::services::import::validate_file_name(name)
            .map_err(|_| MediaError::InvalidName("file name"))?;
        if data.len() > super::MAX_IPC_BYTES {
            return Err(MediaError::TooLarge {
                limit: "bytes",
                max: super::MAX_IPC_BYTES as u64,
            });
        }
        let batch = self.generators.new_id();
        self.root.create_dir_all(&batch_dir(batch))?;
        let source = self.root.resolve(&format!("{}/source", batch_dir(batch)))?;
        let mut file = create_new_private(&source)?;
        file.write_all(data)?;
        file.sync_all()?;
        drop(file);
        let preview = self
            .prepare(
                batch,
                vec![ImportSource {
                    path: source.clone(),
                    name: name.to_owned(),
                    folders: Vec::new(),
                }],
                Vec::new(),
                None,
                None,
                Arc::new(|_, _| {}),
            )
            .await;
        let _ = fs::remove_file(&source);
        preview
    }

    /// Stops a running preparation or drops a prepared batch with its
    /// copies. Unknown ids are ignored.
    pub fn cancel_import(&self, batch: Uuid) {
        let removed = {
            let mut batches = self.batches();
            if let Some(cancel) = batches.running.get(&batch) {
                cancel.store(true, Ordering::Relaxed);
            }
            batches.prepared.remove(&batch).is_some()
        };
        if removed {
            self.remove_batch_files(batch);
        }
    }

    fn remove_batch_files(&self, batch: Uuid) {
        if let Ok(dir) = self.root.resolve(&batch_dir(batch))
            && dir.exists()
            && let Err(error) = fs::remove_dir_all(&dir)
        {
            log::warn!("copies of import {batch} not removed: {error}");
        }
    }

    /// Imports a prepared batch. `decisions` overrides the default per file:
    /// files import, duplicates follow `duplicates`.
    pub async fn commit_import(
        &self,
        batch: Uuid,
        decisions: &BTreeMap<u32, Decision>,
        duplicates: Decision,
        placement: &SubnetPlacement,
    ) -> Result<ImportOutcome, MediaError> {
        let prepared = self
            .batches()
            .prepared
            .remove(&batch)
            .ok_or(MediaError::ImportNotFound)?;
        let result = self
            .commit_prepared(batch, &prepared, decisions, duplicates, placement)
            .await;
        self.remove_batch_files(batch);
        result
    }

    async fn commit_prepared(
        &self,
        batch: Uuid,
        prepared: &Batch,
        decisions: &BTreeMap<u32, Decision>,
        duplicates: Decision,
        placement: &SubnetPlacement,
    ) -> Result<ImportOutcome, MediaError> {
        let now = self.generators.now().to_iso();
        // What happens to each file, and the id it ends up with.
        let mut plan: Vec<(usize, Option<String>, bool)> = Vec::new();
        let mut ids: Vec<Option<String>> = vec![None; prepared.items.len()];
        for (index, item) in prepared.items.iter().enumerate() {
            let key = u32::try_from(index).unwrap_or(u32::MAX);
            let default = if item.duplicate.is_some() {
                duplicates
            } else {
                Decision::Import
            };
            let decision = decisions.get(&key).copied().unwrap_or(default);
            match (decision, &item.duplicate) {
                (Decision::Skip, _) => {}
                (Decision::UseExisting, Some(Duplicate::Existing { id, .. })) => {
                    ids[index] = Some(id.clone());
                    plan.push((index, Some(id.clone()), false));
                }
                (Decision::UseExisting, Some(Duplicate::InBatch { index: first })) => {
                    let first = *first as usize;
                    if let Some(Some(id)) = ids.get(first) {
                        let id = id.clone();
                        ids[index] = Some(id.clone());
                        plan.push((index, Some(id), false));
                    }
                }
                (Decision::Import | Decision::UseExisting, _) => {
                    let id = id_text(self.generators.new_id());
                    ids[index] = Some(id.clone());
                    plan.push((index, Some(id), true));
                }
            }
        }

        // Targets and journal before anything appears in media/.
        let mut moves: Vec<(PathBuf, PathBuf)> = Vec::new();
        let mut journal = ImportJournal {
            batch: id_text(batch),
            files: Vec::new(),
        };
        for (index, id, created) in &plan {
            if !created {
                continue;
            }
            let item = &prepared.items[*index];
            let id = id.as_deref().unwrap_or_default();
            let kind = item.media_type.kind();
            self.root.create_dir_all(kind.directory())?;
            let relative = relative_path(kind, id, &item.extension);
            let target = self.root.resolve(&relative)?;
            if fs::symlink_metadata(&target).is_ok() {
                return Err(MediaError::Io(io::Error::new(
                    io::ErrorKind::AlreadyExists,
                    format!("{relative} exists"),
                )));
            }
            journal.files.push(JournalFile {
                id: id.to_owned(),
                relative,
            });
            moves.push((item.copy.clone(), target));
        }
        let journal_path = self.root.resolve(&journal_relative(batch))?;
        if !journal.files.is_empty() {
            let data = serde_json::to_vec(&journal).map_err(io::Error::other)?;
            write_atomic(&journal_path, &data)?;
        }

        let _guard = self.write_lock.lock().await;
        let mut moved: Vec<(PathBuf, PathBuf)> = Vec::new();
        let result = async {
            let mut transaction = self.database.writer().begin().await?;
            // Files first: a failure in the rows below moves them back.
            for (from, to) in &moves {
                fs::rename(from, to)?;
                moved.push((from.clone(), to.clone()));
            }
            for (index, id, created) in &plan {
                if !created {
                    continue;
                }
                let item = &prepared.items[*index];
                let folder = match (&prepared.import_folder, item.folders.is_empty()) {
                    (Some(base), true) => Some(base.clone()),
                    (Some(base), false) => Some(format!("{base}/{}", item.folders.join("/"))),
                    (None, _) => None,
                };
                rows::insert(
                    &mut transaction,
                    &MediaRow {
                        id: id.clone().unwrap_or_default(),
                        kind: item.media_type.kind().as_str().to_owned(),
                        mime_type: item.media_type.mime_type().to_owned(),
                        extension: item.extension.clone(),
                        title: title_from_name(&item.name),
                        description: String::new(),
                        original_name: item.name.chars().take(255).collect(),
                        import_folder: folder.map(|folder| folder.chars().take(4096).collect()),
                        size_bytes: i64::try_from(item.size).unwrap_or(i64::MAX),
                        sha256: item.sha256.clone(),
                        width: item.width.map(i64::from),
                        height: item.height.map(i64::from),
                        page_count: None,
                        created_at: now.clone(),
                        updated_at: now.clone(),
                        favorited_at: None,
                        last_opened_at: None,
                        archived_at: None,
                        deleted_at: None,
                    },
                )
                .await?;
            }
            // Subnets: from the folder structure, or one chosen subnet.
            let mut subnet_of_folder: HashMap<Vec<String>, Option<String>> = HashMap::new();
            let mut top = None;
            if placement.from_folders || placement.parent.is_some() {
                let parent = placement.parent.map(id_text);
                for (index, id, _) in &plan {
                    let item = &prepared.items[*index];
                    let path: Vec<String> = if placement.from_folders {
                        prepared
                            .folder_name
                            .iter()
                            .cloned()
                            .chain(item.folders.iter().cloned())
                            .collect()
                    } else {
                        Vec::new()
                    };
                    let subnet = match subnet_of_folder.get(&path) {
                        Some(subnet) => subnet.clone(),
                        None => {
                            let subnet = structure::ensure_subnet_path(
                                &mut transaction,
                                &self.generators,
                                parent.clone(),
                                &path,
                            )
                            .await?;
                            subnet_of_folder.insert(path.clone(), subnet.clone());
                            subnet
                        }
                    };
                    if placement.from_folders && top.is_none() {
                        let first: Vec<String> = path.iter().take(1).cloned().collect();
                        top = match subnet_of_folder.get(&first) {
                            Some(subnet) => subnet.clone(),
                            None => {
                                structure::ensure_subnet_path(
                                    &mut transaction,
                                    &self.generators,
                                    parent.clone(),
                                    &first,
                                )
                                .await?
                            }
                        };
                    }
                    if let (Some(subnet), Some(id)) = (subnet, id) {
                        structure::add_membership(
                            &mut transaction,
                            &Element::media(id.clone()),
                            &subnet,
                            &now,
                        )
                        .await?;
                    }
                }
            }
            for kind in [MediaKind::Image, MediaKind::Pdf, MediaKind::Attachment] {
                if let Ok(dir) = self.root.resolve(kind.directory())
                    && dir.is_dir()
                {
                    sync_dir(&dir)?;
                }
            }
            transaction.commit().await?;
            Ok::<_, MediaError>(top)
        }
        .await;
        let top = match result {
            Ok(top) => top,
            Err(error) => {
                for (from, to) in moved.iter().rev() {
                    if let Err(back) = fs::rename(to, from) {
                        log::error!("import rollback: {} not moved back: {back}", to.display());
                        let _ = fs::remove_file(to);
                    }
                }
                let _ = fs::remove_file(&journal_path);
                return Err(error);
            }
        };
        drop(_guard);
        if let Err(error) = fs::remove_file(&journal_path)
            && error.kind() != io::ErrorKind::NotFound
        {
            log::warn!("import journal not removed: {error}");
        }
        let created: Vec<Uuid> = plan
            .iter()
            .filter(|(index, _, created)| *created && prepared.items[*index].media_type.is_raster())
            .filter_map(|(_, id, _)| id.as_deref().and_then(|id| Uuid::try_parse(id).ok()))
            .collect();
        self.spawn_thumbnails(created);
        Ok(ImportOutcome {
            items: plan
                .into_iter()
                .filter_map(|(index, id, created)| {
                    id.map(|id| ImportedItem {
                        index: u32::try_from(index).unwrap_or(u32::MAX),
                        id,
                        created,
                    })
                })
                .collect(),
            subnet_id: top,
        })
    }
}
