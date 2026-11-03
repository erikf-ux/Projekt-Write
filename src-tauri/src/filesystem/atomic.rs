//! Atomic file replacement (section 9, "Speichern"):
//! "Temporärdatei im selben Ordner, Daten auf den Datenträger schreiben
//! (`fsync`), dann umbenennen."
//!
//! After a crash at any point the target holds either the complete old or the
//! complete new content, never a mixture. A crash can leave a temporary file
//! behind; its name matches [`is_temp_file_name`], so opening the library can
//! recognise and remove it.

use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};

/// Suffix of temporary files written by [`write_atomic`].
pub const TEMP_SUFFIX: &str = ".pwtmp";

/// Whether `name` is a temporary file left by an interrupted [`write_atomic`].
pub fn is_temp_file_name(name: &str) -> bool {
    name.starts_with('.') && name.ends_with(TEMP_SUFFIX) && name.len() > 1 + TEMP_SUFFIX.len()
}

/// Writes `data` to `target` atomically and durably.
///
/// 1. Create `.<name>.<random>.pwtmp` next to the target (`create_new`, so an
///    existing file or link is never reused).
/// 2. Write all data and flush it to the device (`sync_all`).
/// 3. Replace the target by renaming (Windows: `MoveFileExW` with
///    `MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH`).
/// 4. Unix: flush the directory so the rename itself is durable.
///
/// On failure the temporary file is removed; the target is unchanged.
pub fn write_atomic(target: &Path, data: &[u8]) -> io::Result<()> {
    let parent = target
        .parent()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "target has no parent"))?;
    let name = target
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "target has no file name"))?;
    let temp = parent.join(temp_name(name));

    let result = write_and_replace(&temp, target, parent, data);
    if result.is_err() {
        // The temp file may or may not exist at this point.
        let _ = fs::remove_file(&temp);
    }
    result
}

fn temp_name(name: &str) -> String {
    format!(".{name}.{}{TEMP_SUFFIX}", uuid::Uuid::new_v4().simple())
}

fn write_and_replace(temp: &Path, target: &Path, parent: &Path, data: &[u8]) -> io::Result<()> {
    let mut file = create_new_private(temp)?;
    file.write_all(data)?;
    file.sync_all()?;
    drop(file);
    replace(temp, target)?;
    sync_dir(parent)
}

/// Creates a new file for writing. Fails if anything (file, link) already
/// exists at `path`. On Unix the file is readable only by the current user.
pub fn create_new_private(path: &Path) -> io::Result<File> {
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    options.open(path)
}

/// Opens an existing regular file for reading without following a link in
/// the last component (Unix: `O_NOFOLLOW`; Windows: checked beforehand by
/// [`crate::security::paths::SafeRoot`]).
pub fn open_read_no_follow(path: &Path) -> io::Result<File> {
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NOFOLLOW);
    }
    let file = options.open(path)?;
    if !file.metadata()?.is_file() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "not a regular file",
        ));
    }
    Ok(file)
}

/// Reads a whole file, refusing files larger than `max_bytes`.
pub fn read_limited(path: &Path, max_bytes: u64) -> io::Result<Vec<u8>> {
    let file = open_read_no_follow(path)?;
    let len = file.metadata()?.len();
    if len > max_bytes {
        return Err(io::Error::new(
            io::ErrorKind::FileTooLarge,
            format!("file is larger than {max_bytes} bytes"),
        ));
    }
    let mut data = Vec::with_capacity(usize::try_from(len).unwrap_or(0));
    // `take` guards against a file that grows while it is read.
    file.take(max_bytes + 1).read_to_end(&mut data)?;
    if data.len() as u64 > max_bytes {
        return Err(io::Error::new(
            io::ErrorKind::FileTooLarge,
            format!("file is larger than {max_bytes} bytes"),
        ));
    }
    Ok(data)
}

#[cfg(windows)]
fn replace(from: &Path, to: &Path) -> io::Result<()> {
    // Virus scanners, indexers and sync clients briefly open new files without
    // FILE_SHARE_DELETE, which makes the rename fail. Retry for about a second.
    const DELAYS_MS: [u64; 8] = [5, 10, 20, 40, 80, 160, 320, 400];
    let mut attempt = 0;
    loop {
        match super::windows::replace_file_write_through(from, to) {
            Ok(()) => return Ok(()),
            Err(error) if attempt < DELAYS_MS.len() && is_transient_windows_error(&error) => {
                std::thread::sleep(std::time::Duration::from_millis(DELAYS_MS[attempt]));
                attempt += 1;
            }
            Err(error) => return Err(error),
        }
    }
}

#[cfg(windows)]
fn is_transient_windows_error(error: &io::Error) -> bool {
    // ERROR_ACCESS_DENIED, ERROR_SHARING_VIOLATION, ERROR_LOCK_VIOLATION
    matches!(error.raw_os_error(), Some(5 | 32 | 33))
}

#[cfg(not(windows))]
fn replace(from: &Path, to: &Path) -> io::Result<()> {
    fs::rename(from, to)
}

/// Makes a completed rename durable (Unix). Windows renames with
/// `MOVEFILE_WRITE_THROUGH` instead.
#[cfg(unix)]
pub fn sync_dir(dir: &Path) -> io::Result<()> {
    match File::open(dir).and_then(|d| d.sync_all()) {
        Ok(()) => Ok(()),
        // Some file systems (for example a few FUSE mounts) cannot sync
        // directories; the rename is still atomic there.
        Err(error)
            if matches!(
                error.kind(),
                io::ErrorKind::Unsupported | io::ErrorKind::InvalidInput
            ) =>
        {
            Ok(())
        }
        Err(error) => Err(error),
    }
}

#[cfg(not(unix))]
pub fn sync_dir(_dir: &Path) -> io::Result<()> {
    Ok(())
}

/// Lists the temporary files [`write_atomic`] may have left in `dir`.
pub fn leftover_temp_files(dir: &Path) -> io::Result<Vec<PathBuf>> {
    let mut found = Vec::new();
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let is_file = entry.file_type()?.is_file();
        if let Some(name) = entry.file_name().to_str()
            && is_file
            && is_temp_file_name(name)
        {
            found.push(entry.path());
        }
    }
    found.sort();
    Ok(found)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir() -> tempfile::TempDir {
        let base = Path::new(env!("CARGO_MANIFEST_DIR")).join("target/tmp");
        fs::create_dir_all(&base).unwrap();
        tempfile::tempdir_in(base).unwrap()
    }

    #[test]
    fn temp_names_are_recognised() {
        let name = temp_name("abc.md");
        assert!(name.starts_with(".abc.md."));
        assert!(is_temp_file_name(&name));
        assert!(!is_temp_file_name("abc.md"));
        assert!(!is_temp_file_name(".pwtmp"));
        assert!(!is_temp_file_name("x.pwtmp"));
    }

    #[test]
    fn creates_and_replaces_without_leftovers() {
        let dir = temp_dir();
        let target = dir.path().join("doc.txt");
        write_atomic(&target, b"first").unwrap();
        assert_eq!(fs::read(&target).unwrap(), b"first");
        write_atomic(&target, b"second, longer content").unwrap();
        assert_eq!(fs::read(&target).unwrap(), b"second, longer content");
        write_atomic(&target, b"").unwrap();
        assert_eq!(fs::read(&target).unwrap(), b"");
        assert!(leftover_temp_files(dir.path()).unwrap().is_empty());
        let names: Vec<_> = fs::read_dir(dir.path())
            .unwrap()
            .map(|e| e.unwrap().file_name())
            .collect();
        assert_eq!(names, ["doc.txt"]);
    }

    #[test]
    fn failure_leaves_target_and_directory_unchanged() {
        let dir = temp_dir();
        // The target is a directory: the rename must fail, nothing may remain.
        let target = dir.path().join("occupied");
        fs::create_dir(&target).unwrap();
        fs::write(target.join("inside"), b"keep").unwrap();
        assert!(write_atomic(&target, b"data").is_err());
        assert_eq!(fs::read(target.join("inside")).unwrap(), b"keep");
        assert!(leftover_temp_files(dir.path()).unwrap().is_empty());
    }

    #[test]
    fn missing_parent_is_an_error() {
        let dir = temp_dir();
        let target = dir.path().join("missing").join("doc.txt");
        assert!(write_atomic(&target, b"data").is_err());
        assert!(!dir.path().join("missing").exists());
    }

    #[test]
    fn read_limited_enforces_the_limit() {
        let dir = temp_dir();
        let path = dir.path().join("f");
        fs::write(&path, vec![7u8; 100]).unwrap();
        assert_eq!(read_limited(&path, 100).unwrap().len(), 100);
        let error = read_limited(&path, 99).unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::FileTooLarge);
    }

    #[cfg(unix)]
    #[test]
    fn new_files_are_private_on_unix() {
        use std::os::unix::fs::PermissionsExt;
        let dir = temp_dir();
        let target = dir.path().join("private.txt");
        write_atomic(&target, b"x").unwrap();
        let mode = fs::metadata(&target).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600);
    }

    #[cfg(unix)]
    #[test]
    fn reading_does_not_follow_a_final_symlink() {
        let dir = temp_dir();
        let real = dir.path().join("real");
        fs::write(&real, b"secret").unwrap();
        let link = dir.path().join("link");
        std::os::unix::fs::symlink(&real, &link).unwrap();
        assert!(open_read_no_follow(&link).is_err());
        assert!(open_read_no_follow(&real).is_ok());
    }
}
