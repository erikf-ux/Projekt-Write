//! Helpers shared by the integration tests.
//!
//! All test data lives in temporary folders inside `target/` – never outside
//! the project folder (section 2).

#![allow(dead_code)]

use std::io;
use std::path::Path;

/// A fresh temporary folder inside `target/tmp`, removed when dropped.
pub fn temp_dir(prefix: &str) -> tempfile::TempDir {
    tempfile::Builder::new()
        .prefix(prefix)
        .tempdir_in(env!("CARGO_TARGET_TMPDIR"))
        .expect("create temp dir inside target/")
}

/// Creates a symbolic link to a directory.
pub fn symlink_dir(target: &Path, link: &Path) -> io::Result<()> {
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(target, link)
    }
    #[cfg(windows)]
    {
        std::os::windows::fs::symlink_dir(target, link)
    }
}

/// Creates a symbolic link to a file.
pub fn symlink_file(target: &Path, link: &Path) -> io::Result<()> {
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(target, link)
    }
    #[cfg(windows)]
    {
        std::os::windows::fs::symlink_file(target, link)
    }
}

/// Creates a Windows junction (works without administrator rights).
#[cfg(windows)]
pub fn junction(target: &Path, link: &Path) -> io::Result<()> {
    let status = std::process::Command::new("cmd")
        .arg("/C")
        .arg("mklink")
        .arg("/J")
        .arg(link)
        .arg(target)
        .stdout(std::process::Stdio::null())
        .status()?;
    if status.success() {
        Ok(())
    } else {
        Err(io::Error::other(format!("mklink /J failed: {status}")))
    }
}

/// Small deterministic pseudo-random generator (SplitMix64) for tests.
pub struct SplitMix64(pub u64);

impl SplitMix64 {
    pub fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// Uniform value in `range` (end exclusive).
    pub fn between(&mut self, range: std::ops::Range<u64>) -> u64 {
        range.start + self.next() % (range.end - range.start)
    }
}
