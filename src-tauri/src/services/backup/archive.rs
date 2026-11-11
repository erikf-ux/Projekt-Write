//! Reading ZIP files that come from outside (section 20; ADR 0021, decision
//! 9): backups (`.pwbackup`) and packages (`.pwritepack`).
//!
//! * Entry names are checked byte by byte: relative, lower-case ASCII
//!   letters, digits, `.`, `-`, `_` and `/` only; no `..`, no empty parts,
//!   no backslash, no drive letters, no repeats. What a name may be is up to
//!   the caller (a positive list).
//! * No directories, symbolic links or encrypted entries.
//! * The size of an entry is counted while reading, never taken from the
//!   archive; reading stops at the limit the caller gives.

use std::collections::BTreeSet;
use std::fs::File;
use std::io::{self, Read, Write};

use sha2::{Digest, Sha256};

/// More entries than this are refused.
pub const MAX_ENTRIES: usize = 200_000;
/// Longest entry name.
pub const MAX_NAME_BYTES: usize = 300;

#[derive(Debug, thiserror::Error)]
pub enum ArchiveError {
    #[error("not a ZIP file")]
    NotZip,
    #[error("too many entries")]
    TooManyEntries,
    #[error("entry name not allowed: {0}")]
    Name(String),
    #[error("entry repeated: {0}")]
    Repeated(String),
    #[error("entry is no plain file: {0}")]
    NotAFile(String),
    #[error("entry is encrypted: {0}")]
    Encrypted(String),
    #[error("entry larger than allowed: {0}")]
    TooLarge(String),
    #[error("entry damaged: {0}")]
    Damaged(String),
    #[error("file error: {0}")]
    Io(#[from] io::Error),
}

/// Whether `name` is a safe relative entry name (the shape only).
pub fn safe_name(name: &[u8]) -> bool {
    if name.is_empty() || name.len() > MAX_NAME_BYTES || name[0] == b'/' {
        return false;
    }
    if !name.iter().all(|b| {
        b.is_ascii_lowercase() || b.is_ascii_digit() || matches!(b, b'.' | b'-' | b'_' | b'/')
    }) {
        return false;
    }
    name.split(|b| *b == b'/')
        .all(|part| !part.is_empty() && part != b"." && part != b".." && !part.starts_with(b"."))
}

/// An archive opened for checking.
pub struct Archive {
    zip: zip::ZipArchive<File>,
    names: Vec<String>,
}

impl Archive {
    /// Opens `file` and checks the names of all entries: their shape and
    /// `allowed`, no repeats.
    pub fn open(file: File, allowed: impl Fn(&str) -> bool) -> Result<Self, ArchiveError> {
        let mut zip = zip::ZipArchive::new(file).map_err(|_| ArchiveError::NotZip)?;
        if zip.len() > MAX_ENTRIES {
            return Err(ArchiveError::TooManyEntries);
        }
        let mut names = Vec::with_capacity(zip.len());
        let mut seen = BTreeSet::new();
        for index in 0..zip.len() {
            let entry = zip
                .by_index_raw(index)
                .map_err(|_| ArchiveError::Damaged(index.to_string()))?;
            let raw = entry.name_raw();
            let shown = String::from_utf8_lossy(raw).into_owned();
            if !safe_name(raw) {
                return Err(ArchiveError::Name(shown));
            }
            if entry.is_dir() || entry.is_symlink() || !entry.is_file() {
                return Err(ArchiveError::NotAFile(shown));
            }
            if entry.encrypted() {
                return Err(ArchiveError::Encrypted(shown));
            }
            if !allowed(&shown) {
                return Err(ArchiveError::Name(shown));
            }
            if !seen.insert(shown.clone()) {
                return Err(ArchiveError::Repeated(shown));
            }
            names.push(shown);
        }
        Ok(Self { zip, names })
    }

    /// The entry names, in archive order.
    pub fn names(&self) -> &[String] {
        &self.names
    }

    /// Reads one entry into memory, at most `limit` bytes.
    pub fn read(&mut self, name: &str, limit: u64) -> Result<Vec<u8>, ArchiveError> {
        let mut data = Vec::new();
        self.copy(name, limit, &mut data)?;
        Ok(data)
    }

    /// Streams one entry into `out`, at most `limit` bytes; returns size and
    /// SHA-256 (lower-case hex) of what was read.
    pub fn copy(
        &mut self,
        name: &str,
        limit: u64,
        out: &mut impl Write,
    ) -> Result<(u64, String), ArchiveError> {
        let index = self
            .names
            .iter()
            .position(|entry| entry == name)
            .ok_or_else(|| ArchiveError::Damaged(name.to_owned()))?;
        let mut entry = self
            .zip
            .by_index(index)
            .map_err(|_| ArchiveError::Damaged(name.to_owned()))?;
        let mut hasher = Sha256::new();
        let mut total: u64 = 0;
        let mut buffer = vec![0u8; 64 * 1024];
        loop {
            let read = match entry.read(&mut buffer) {
                Ok(0) => break,
                Ok(read) => read,
                Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
                // Includes a wrong CRC at the end of the entry.
                Err(_) => return Err(ArchiveError::Damaged(name.to_owned())),
            };
            total += read as u64;
            if total > limit {
                return Err(ArchiveError::TooLarge(name.to_owned()));
            }
            hasher.update(&buffer[..read]);
            out.write_all(&buffer[..read])?;
        }
        Ok((total, hex(&hasher.finalize())))
    }
}

/// Lower-case hex of a digest.
pub fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write as _;
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        let _ = write!(out, "{byte:02x}");
    }
    out
}

/// A writer that only counts and hashes (checking without unpacking).
pub struct Discard;

impl Write for Discard {
    fn write(&mut self, data: &[u8]) -> io::Result<usize> {
        Ok(data.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_are_relative_and_plain() {
        for good in [
            "manifest.json",
            "library.json",
            "documents/markdown/0b0e5a3e-0c5f-4a43-9f39-3a3b9a9d2c11.md",
            "media/images/a_b-c.png",
        ] {
            assert!(safe_name(good.as_bytes()), "{good}");
        }
        for bad in [
            "",
            "/etc/passwd",
            "../x",
            "a/../b",
            "a//b",
            "a/./b",
            "C:/x",
            "c:x",
            "a\\b",
            "Manifest.json",
            "a/.hidden",
            "a\u{0}b",
            "ä.md",
            "a/b/",
        ] {
            assert!(!safe_name(bad.as_bytes()), "{bad:?}");
        }
        assert!(!safe_name("x".repeat(MAX_NAME_BYTES + 1).as_bytes()));
    }
}
