//! What the session needs around backups (ADR 0021, decision 7): whether
//! one is running, whether the automatic one is due, and whether a target
//! shares the drive with the library.

use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::time::{Duration, SystemTime};

use uuid::Uuid;

/// The automatic backup runs when the newest one is older than this.
pub const AUTOMATIC_INTERVAL: Duration = Duration::from_secs(24 * 60 * 60);
/// Pause after opening before the automatic backup starts: the interface and
/// the database check come first.
pub const AUTOMATIC_DELAY: Duration = Duration::from_secs(30);

/// A backup in progress (at most one at a time).
#[derive(Debug, Default)]
pub struct Activity {
    pub running: bool,
    pub cancel: Option<Arc<AtomicBool>>,
}

/// When the newest backup of this library in `dir` was written.
pub fn newest_backup(dir: &Path, library_id: Uuid) -> Option<SystemTime> {
    super::own_backups(dir, library_id)
        .ok()?
        .iter()
        .filter_map(|path| path.metadata().ok()?.modified().ok())
        .max()
}

/// Whether the automatic backup is due at `now`.
pub fn due(dir: &Path, library_id: Uuid, now: SystemTime) -> bool {
    match newest_backup(dir, library_id) {
        None => true,
        Some(newest) => now
            .duration_since(newest)
            .is_ok_and(|age| age >= AUTOMATIC_INTERVAL),
    }
}

/// Whether two existing paths lie on the same drive (section 21: a failure of
/// the drive would hit both). Unknown counts as the same, so the app warns.
pub fn same_drive(a: &Path, b: &Path) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        match (a.metadata(), b.metadata()) {
            (Ok(first), Ok(second)) => first.dev() == second.dev(),
            _ => true,
        }
    }
    #[cfg(windows)]
    {
        use std::path::Component;
        let prefix = |path: &Path| {
            std::fs::canonicalize(path)
                .ok()
                .and_then(|path| match path.components().next() {
                    Some(Component::Prefix(prefix)) => {
                        Some(prefix.as_os_str().to_string_lossy().to_lowercase())
                    }
                    _ => None,
                })
        };
        match (prefix(a), prefix(b)) {
            (Some(first), Some(second)) => first == second,
            _ => true,
        }
    }
    #[cfg(not(any(unix, windows)))]
    {
        let _ = (a, b);
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_folder_shares_the_drive_with_itself() {
        let dir = std::env::current_dir().unwrap();
        assert!(same_drive(&dir, &dir));
    }

    #[test]
    fn no_backup_means_due() {
        let dir = std::env::current_dir()
            .unwrap()
            .join("no-such-folder-for-backups");
        assert!(due(&dir, Uuid::nil(), SystemTime::now()));
    }
}
