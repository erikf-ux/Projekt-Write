//! Housekeeping when a library is opened (section 9, "Import in die
//! Bibliothek"): "Beim Start wird `tmp/` geleert. Dateien ohne
//! Datenbankeintrag werden gemeldet, nicht still gelöscht."

use std::collections::HashSet;
use std::fs;
use std::io;

use uuid::Uuid;

use super::layout::TMP_DIR;
use crate::database::Database;
use crate::database::documents as rows;
use crate::filesystem::atomic::is_temp_file_name;
use crate::security::paths::{PathError, SafeRoot};
use crate::services::documents::DocumentFormat;

/// What [`scan_documents`] found.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DocumentScan {
    /// Temporary files of interrupted saves that were removed. Their target
    /// files are intact (atomic replacement).
    pub removed_temp_files: usize,
    /// Files in the document folders without a database row, relative to the
    /// library. Reported, never deleted.
    pub orphaned_files: Vec<String>,
    /// Database rows whose file is missing.
    pub missing_files: Vec<Uuid>,
}

/// Empties `tmp/`. Links inside are removed as links, never followed.
/// Returns the number of removed entries.
pub fn clear_tmp(root: &SafeRoot) -> Result<usize, PathError> {
    let dir = root.resolve(TMP_DIR)?;
    let entries = match fs::read_dir(&dir) {
        Ok(entries) => entries,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(0),
        Err(source) => {
            return Err(PathError::Io {
                relative: TMP_DIR.to_owned(),
                source,
            });
        }
    };
    let io_error = |source| PathError::Io {
        relative: TMP_DIR.to_owned(),
        source,
    };
    let mut removed = 0;
    for entry in entries {
        let entry = entry.map_err(io_error)?;
        let file_type = entry.file_type().map_err(io_error)?;
        let path = entry.path();
        if file_type.is_dir() {
            // remove_dir_all does not follow links inside the tree.
            fs::remove_dir_all(&path).map_err(io_error)?;
        } else {
            remove_file_or_link(&path).map_err(io_error)?;
        }
        removed += 1;
    }
    Ok(removed)
}

/// Removes a file or a link (on Windows, a directory link is removed with
/// `remove_dir`).
fn remove_file_or_link(path: &std::path::Path) -> io::Result<()> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if cfg!(windows) => fs::remove_dir(path).map_err(|_| error),
        Err(error) => Err(error),
    }
}

/// Compares the document folders with the `documents` table. Removes
/// temporary files of interrupted saves, reports everything else.
pub async fn scan_documents(
    root: &SafeRoot,
    database: &Database,
) -> Result<DocumentScan, ScanError> {
    let mut scan = DocumentScan::default();
    let mut on_disk: HashSet<(Uuid, DocumentFormat)> = HashSet::new();

    for format in DocumentFormat::ALL {
        let folder = format.directory();
        let dir = root.resolve(folder)?;
        let entries = match fs::read_dir(&dir) {
            Ok(entries) => entries,
            Err(error) if error.kind() == io::ErrorKind::NotFound => continue,
            Err(error) => return Err(ScanError::Io(error)),
        };
        for entry in entries {
            let entry = entry?;
            let file_type = entry.file_type()?;
            let name = entry.file_name();
            let shown = format!("{folder}/{}", name.to_string_lossy());
            let Some(name) = name.to_str() else {
                scan.orphaned_files.push(shown);
                continue;
            };
            if file_type.is_file() && is_temp_file_name(name) {
                fs::remove_file(entry.path())?;
                scan.removed_temp_files += 1;
            } else if let Some(id) = format
                .id_from_file_name(name)
                .filter(|_| file_type.is_file())
            {
                on_disk.insert((id, format));
            } else {
                scan.orphaned_files.push(shown);
            }
        }
    }

    let mut in_database = HashSet::new();
    for (id, format) in rows::all_file_keys(database.reader()).await? {
        let (Ok(id), Some(format)) = (Uuid::try_parse(&id), DocumentFormat::parse(&format)) else {
            continue;
        };
        if !on_disk.contains(&(id, format)) {
            scan.missing_files.push(id);
        }
        in_database.insert((id, format));
    }
    for (id, format) in &on_disk {
        if !in_database.contains(&(*id, *format)) {
            scan.orphaned_files.push(format.relative_path(*id));
        }
    }
    scan.orphaned_files.sort();
    scan.missing_files.sort();
    Ok(scan)
}

#[derive(Debug, thiserror::Error)]
pub enum ScanError {
    #[error(transparent)]
    Path(#[from] PathError),
    #[error("I/O error: {0}")]
    Io(#[from] io::Error),
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),
}
