//! The transport package `.pwritepack` (section 10; ADR 0021, decision 5):
//! a ZIP with `manifest.json`, `document.pwrite` and the media it uses in
//! `media/<id>.<ext>`.
//!
//! Importing checks the archive like a backup (names, links, sizes counted
//! while reading, checksums), puts every medium through the type check of
//! the media import and only then commits: media with a known checksum are
//! used, others imported, the document gets a new id and the ids of its
//! media. A refusal leaves nothing.

use std::collections::BTreeMap;
use std::fs;
use std::io::{self, BufWriter, Write};
use std::path::Path;

use serde::{Deserialize, Serialize};
use ts_rs::TS;
use uuid::Uuid;

use crate::filesystem::atomic::{create_new_private, open_read_no_follow, sync_dir};
use crate::services::backup::archive::{Archive, ArchiveError};
use crate::services::documents::{DocumentFormat, MAX_DOCUMENT_BYTES, MEDIA_PREFIX, media_in};
use crate::services::media::{self, Decision, MediaKind, MediaLibrary, SubnetPlacement};
use crate::services::pwrite;

pub const FORMAT: &str = "pwritepack";
pub const FORMAT_VERSION: u32 = 1;
pub const EXTENSION: &str = "pwritepack";
const MANIFEST_ENTRY: &str = "manifest.json";
const DOCUMENT_ENTRY: &str = "document.pwrite";
const MAX_MANIFEST_BYTES: u64 = 1024 * 1024;

#[derive(Debug, thiserror::Error)]
pub enum PackError {
    #[error("package refused: {0}")]
    Refused(String),
    #[error("package is newer: {0}")]
    Newer(String),
    #[error("not a .pwrite document")]
    NotPwrite,
    #[error("medium refused: {0}")]
    MediumRefused(String),
    #[error("document error: {0}")]
    Document(String),
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),
    #[error("file error: {0}")]
    Io(#[from] io::Error),
}

impl From<ArchiveError> for PackError {
    fn from(error: ArchiveError) -> Self {
        match error {
            ArchiveError::Io(error) => PackError::Io(error),
            other => PackError::Refused(other.to_string()),
        }
    }
}

/// One medium of a package.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PackMedium {
    pub id: Uuid,
    pub path: String,
    pub size: u64,
    pub sha256: String,
    pub original_name: String,
    pub mime_type: String,
}

/// `manifest.json` of a package.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PackManifest {
    pub format: String,
    pub format_version: u32,
    pub app_version: String,
    pub title: String,
    pub document_sha256: String,
    pub media: Vec<PackMedium>,
}

/// Result of importing a package.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct PackImport {
    pub document_id: String,
    pub title: String,
    /// Media new to the library.
    pub imported: u32,
    /// Media the library already had (same checksum).
    pub reused: u32,
}

fn refused(reason: impl Into<String>) -> PackError {
    PackError::Refused(reason.into())
}

/// `media/<uuid>.<ext>` with an extension of the positive list.
fn media_entry(name: &str) -> bool {
    let Some(rest) = name.strip_prefix("media/") else {
        return false;
    };
    let Some((stem, extension)) = rest.rsplit_once('.') else {
        return false;
    };
    Uuid::try_parse(stem).is_ok_and(|id| id.hyphenated().to_string() == stem)
        && media::MediaType::ALL
            .iter()
            .any(|media_type| media_type.accepts_extension(extension))
}

fn allowed_entry(name: &str) -> bool {
    name == MANIFEST_ENTRY || name == DOCUMENT_ENTRY || media_entry(name)
}

/// Writes a package of the `.pwrite` file `document` (the bytes the
/// frontend produced) with the media it uses to `file`, under a temporary
/// name renamed at the end.
pub async fn export_pack(
    root: &crate::security::paths::SafeRoot,
    database: &crate::database::Database,
    document: &[u8],
    file: &Path,
) -> Result<u32, PackError> {
    if document.len() as u64 > MAX_DOCUMENT_BYTES {
        return Err(refused("document too large"));
    }
    let parsed = match pwrite::parse(document) {
        Ok(pwrite::Parsed::Current(file)) => file,
        _ => return Err(PackError::NotPwrite),
    };
    let mut entries: Vec<(PackMedium, std::path::PathBuf)> = Vec::new();
    for (id, _) in media_in(DocumentFormat::Pwrite, document) {
        let Ok(id) = Uuid::try_parse(&id) else {
            continue;
        };
        if entries.iter().any(|(medium, _)| medium.id == id) {
            continue;
        }
        let Some(row) =
            crate::database::media::get(database.reader(), &id.hyphenated().to_string()).await?
        else {
            continue;
        };
        let Some(kind) = MediaKind::parse(&row.kind) else {
            continue;
        };
        let source = root
            .resolve(&media::relative_path(kind, &row.id, &row.extension))
            .map_err(|error| PackError::Io(io::Error::other(error)))?;
        entries.push((
            PackMedium {
                id,
                path: format!("media/{}.{}", row.id, row.extension),
                size: u64::try_from(row.size_bytes).unwrap_or(0),
                sha256: row.sha256.clone(),
                original_name: row.original_name.clone(),
                mime_type: row.mime_type.clone(),
            },
            source,
        ));
    }
    let manifest = PackManifest {
        format: FORMAT.to_owned(),
        format_version: FORMAT_VERSION,
        app_version: env!("CARGO_PKG_VERSION").to_owned(),
        title: parsed.title.clone(),
        document_sha256: crate::services::documents::sha256_hex(document),
        media: entries.iter().map(|(medium, _)| medium.clone()).collect(),
    };
    let count = u32::try_from(entries.len()).unwrap_or(u32::MAX);
    let part = file.with_extension(format!("{EXTENSION}.part"));
    let target = file.to_owned();
    let document = document.to_vec();
    tokio::task::spawn_blocking(move || -> Result<(), PackError> {
        let result = (|| -> Result<(), PackError> {
            let out = create_new_private(&part)?;
            let mut zip = zip::ZipWriter::new(BufWriter::new(out));
            let deflated = zip::write::SimpleFileOptions::default()
                .compression_method(zip::CompressionMethod::Deflated);
            let stored = zip::write::SimpleFileOptions::default()
                .compression_method(zip::CompressionMethod::Stored);
            let io_error = |error: zip::result::ZipError| PackError::Io(io::Error::other(error));
            zip.start_file(MANIFEST_ENTRY, deflated).map_err(io_error)?;
            zip.write_all(&serde_json::to_vec_pretty(&manifest).map_err(io::Error::other)?)?;
            zip.start_file(DOCUMENT_ENTRY, deflated).map_err(io_error)?;
            zip.write_all(&document)?;
            for (medium, source) in &entries {
                zip.start_file(
                    medium.path.as_str(),
                    stored.large_file(medium.size >= u64::from(u32::MAX)),
                )
                .map_err(io_error)?;
                io::copy(&mut open_read_no_follow(source)?, &mut zip)?;
            }
            let writer = zip.finish().map_err(io_error)?;
            writer
                .into_inner()
                .map_err(|error| error.into_error())?
                .sync_all()?;
            fs::rename(&part, &target)?;
            if let Some(parent) = target.parent() {
                sync_dir(parent)?;
            }
            Ok(())
        })();
        if result.is_err() {
            let _ = fs::remove_file(&part);
        }
        result
    })
    .await
    .map_err(|error| PackError::Io(io::Error::other(error)))??;
    Ok(count)
}

/// Reads and checks a package: manifest, document and every medium with
/// size and checksum. Returns the manifest, the document and the media
/// bytes by their id in the package.
/// A checked package: manifest, document, media bytes by package id.
type ReadPack = (PackManifest, Vec<u8>, BTreeMap<Uuid, Vec<u8>>);

fn read_pack(file: &Path) -> Result<ReadPack, PackError> {
    let mut archive = Archive::open(open_read_no_follow(file)?, allowed_entry)?;
    for required in [MANIFEST_ENTRY, DOCUMENT_ENTRY] {
        if !archive.names().iter().any(|name| name == required) {
            return Err(refused(format!("{required} is missing")));
        }
    }
    let manifest: PackManifest =
        serde_json::from_slice(&archive.read(MANIFEST_ENTRY, MAX_MANIFEST_BYTES)?)
            .map_err(|_| refused("manifest is not valid"))?;
    if manifest.format != FORMAT {
        return Err(refused("not a package of Project Write"));
    }
    if manifest.format_version > FORMAT_VERSION {
        return Err(PackError::Newer(format!(
            "format version {}",
            manifest.format_version
        )));
    }
    let document = archive.read(DOCUMENT_ENTRY, MAX_DOCUMENT_BYTES)?;
    if crate::services::documents::sha256_hex(&document) != manifest.document_sha256 {
        return Err(refused("document differs from the manifest"));
    }
    let listed: Vec<&str> = manifest
        .media
        .iter()
        .map(|medium| medium.path.as_str())
        .collect();
    for name in archive.names() {
        if name != MANIFEST_ENTRY && name != DOCUMENT_ENTRY && !listed.contains(&name.as_str()) {
            return Err(refused(format!("{name} is not in the manifest")));
        }
    }
    let mut media_bytes = BTreeMap::new();
    for medium in &manifest.media {
        if !media_entry(&medium.path) || !medium.path.starts_with(&format!("media/{}.", medium.id))
        {
            return Err(refused(format!("manifest names {}", medium.path)));
        }
        if medium.size > media::MAX_IPC_BYTES as u64 {
            return Err(refused(format!("{} is too large", medium.path)));
        }
        if !archive.names().iter().any(|name| name == &medium.path) {
            return Err(refused(format!("{} is missing", medium.path)));
        }
        let mut data = Vec::new();
        let (size, sha) = archive.copy(&medium.path, medium.size, &mut data)?;
        if size != medium.size || sha != medium.sha256 {
            return Err(refused(format!(
                "{} differs from the manifest",
                medium.path
            )));
        }
        if media_bytes.insert(medium.id, data).is_some() {
            return Err(refused("a medium is named twice"));
        }
    }
    Ok((manifest, document, media_bytes))
}

/// Imports a package into the library (see the module notes).
pub async fn import_pack(
    file: &Path,
    media_library: &MediaLibrary,
    documents: &crate::services::documents::DocumentRepository,
) -> Result<PackImport, PackError> {
    let path = file.to_owned();
    let (manifest, document, media_bytes) = tokio::task::spawn_blocking(move || read_pack(&path))
        .await
        .map_err(|error| PackError::Io(io::Error::other(error)))??;
    let parsed = match pwrite::parse(&document) {
        Ok(pwrite::Parsed::Current(file)) => file,
        Ok(pwrite::Parsed::Newer { version, .. }) => {
            return Err(PackError::Newer(format!("pwrite {version}")));
        }
        Err(_) => return Err(PackError::NotPwrite),
    };
    // Every medium through the type check first; nothing is committed yet.
    let mut prepared: Vec<(Uuid, Uuid)> = Vec::new();
    let cancel_all = |prepared: &[(Uuid, Uuid)]| {
        for (_, batch) in prepared {
            media_library.cancel_import(*batch);
        }
    };
    for medium in &manifest.media {
        let data = &media_bytes[&medium.id];
        let name = if medium.original_name.trim().is_empty() {
            medium.path.trim_start_matches("media/").to_owned()
        } else {
            medium.original_name.clone()
        };
        let preview = match media_library.prepare_bytes(&name, data).await {
            Ok(preview) => preview,
            Err(error) => {
                cancel_all(&prepared);
                return Err(PackError::MediumRefused(error.to_string()));
            }
        };
        let batch = Uuid::try_parse(&preview.batch_id).map_err(|_| refused("batch"))?;
        if preview.items.is_empty() {
            prepared.push((medium.id, batch));
            cancel_all(&prepared);
            let reason = preview
                .skipped
                .first()
                .map(|item| format!("{name}: {:?}", item.reason))
                .unwrap_or(name);
            return Err(PackError::MediumRefused(reason));
        }
        prepared.push((medium.id, batch));
    }
    // Commit: a known checksum uses the medium of the library.
    let mut new_ids: BTreeMap<Uuid, String> = BTreeMap::new();
    let mut imported = 0;
    let mut reused = 0;
    for (old, batch) in &prepared {
        let outcome = media_library
            .commit_import(
                *batch,
                &BTreeMap::new(),
                Decision::UseExisting,
                &SubnetPlacement::default(),
            )
            .await
            .map_err(|error| PackError::MediumRefused(error.to_string()))?;
        let item = outcome
            .items
            .first()
            .ok_or_else(|| refused("medium not imported"))?;
        if item.created {
            imported += 1;
        } else {
            reused += 1;
        }
        new_ids.insert(*old, item.id.clone());
    }
    // The document with the ids of the library.
    let mut content = serde_json::to_string(&parsed.content).map_err(io::Error::other)?;
    for (old, new) in &new_ids {
        content = content.replace(
            &format!("{MEDIA_PREFIX}{}", old.hyphenated()),
            &format!("{MEDIA_PREFIX}{new}"),
        );
    }
    let content: serde_json::Value = serde_json::from_str(&content).map_err(io::Error::other)?;
    let meta = documents
        .create_pwrite(&parsed.title, &parsed.language, content)
        .await
        .map_err(|error| PackError::Document(error.to_string()))?;
    Ok(PackImport {
        document_id: meta.id.hyphenated().to_string(),
        title: meta.title,
        imported,
        reused,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_entries_of_a_package_are_allowed() {
        assert!(allowed_entry("manifest.json"));
        assert!(allowed_entry("document.pwrite"));
        assert!(allowed_entry(
            "media/0b0e5a3e-0c5f-4a43-9f39-3a3b9a9d2c11.png"
        ));
        for bad in [
            "media/0b0e5a3e-0c5f-4a43-9f39-3a3b9a9d2c11.exe",
            "media/x.png",
            "document.md",
            "database/projectwrite.db",
            "media/sub/0b0e5a3e-0c5f-4a43-9f39-3a3b9a9d2c11.png",
        ] {
            assert!(!allowed_entry(bad), "{bad}");
        }
    }
}
