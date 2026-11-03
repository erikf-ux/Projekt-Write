//! `library.lock`: a library is open in at most one app instance (section 9).
//!
//! The lock is an exclusive operating-system file lock (`flock` on Unix,
//! `LockFileEx` on Windows, via `std::fs::File::try_lock`). The operating
//! system releases it when the process ends, also after a crash or a kill, so
//! a stale lock file never blocks the library. The file content (process id,
//! app version, time) only helps diagnosis.

use std::fs::{File, OpenOptions};
use std::io::{self, Write};
use std::path::Path;

use crate::clock::Timestamp;

#[derive(Debug, thiserror::Error)]
pub enum LockError {
    #[error("the library is open in another instance of Project Write")]
    Locked,
    #[error("file locks are not supported at this location: {0}")]
    Unsupported(io::Error),
    #[error("cannot open library.lock: {0}")]
    Io(#[from] io::Error),
}

/// Holds the lock until dropped.
#[derive(Debug)]
pub struct LibraryLock {
    _file: File,
}

impl LibraryLock {
    /// Takes the lock on `path` (created if missing) without waiting.
    pub fn acquire(path: &Path, now: Timestamp) -> Result<Self, LockError> {
        let mut options = OpenOptions::new();
        options.read(true).write(true).create(true).truncate(false);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.custom_flags(libc::O_NOFOLLOW).mode(0o600);
        }
        let mut file = options.open(path)?;
        match file.try_lock() {
            Ok(()) => {}
            Err(std::fs::TryLockError::WouldBlock) => return Err(LockError::Locked),
            Err(std::fs::TryLockError::Error(error))
                if error.kind() == io::ErrorKind::Unsupported =>
            {
                return Err(LockError::Unsupported(error));
            }
            Err(std::fs::TryLockError::Error(error)) => return Err(LockError::Io(error)),
        }
        // Diagnostic content; the lock itself is what matters.
        let info = format!(
            "{{\"pid\": {}, \"appVersion\": \"{}\", \"lockedAt\": \"{}\"}}\n",
            std::process::id(),
            env!("CARGO_PKG_VERSION"),
            now.to_iso()
        );
        file.set_len(0)?;
        file.write_all(info.as_bytes())?;
        file.flush()?;
        Ok(Self { _file: file })
    }
}
