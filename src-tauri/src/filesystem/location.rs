//! Recognises sync folders and network drives (section 9, "Bibliothek"):
//! "Erkenne bekannte Sync-Ordner (OneDrive, Dropbox, iCloud Drive, Google
//! Drive) und Netzlaufwerke. Warne und empfiehl einen lokalen Ort. Wählt der
//! Nutzer ihn trotzdem, nutze dort den Rollback-Modus (`journal_mode=DELETE`)
//! statt WAL."
//!
//! Sync clients copy files one by one and do not understand SQLite's `-wal`
//! and `-shm` files; network file systems often lack the shared memory WAL
//! needs. Both get the rollback journal.
//!
//! Signals, strongest first:
//!
//! 1. Network file system: Windows drive type / UNC path, Linux mount table.
//! 2. Windows Cloud Files API: the folder is a registered sync root.
//! 3. Sync roots the providers announce (environment variables such as
//!    `OneDrive`, Dropbox's `info.json`).
//! 4. Folder names the providers use (`OneDrive - Firma`, `Dropbox`,
//!    `Google Drive`, `iCloudDrive`, `Library/CloudStorage/…`).
//!
//! Name-based detection can give a false warning for a folder that merely
//! carries such a name; the user can still choose the location.

use std::path::{Component, Path, PathBuf};

use serde::Serialize;
use ts_rs::TS;

use crate::database::JournalMode;
use crate::security::paths::canonical_prospective;

/// Where a library lies, as far as storage safety is concerned.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(tag = "kind", rename_all = "camelCase")]
#[ts(export)]
pub enum StorageLocation {
    /// A local disk. The database uses WAL.
    Local,
    /// A folder a cloud sync client watches. The database uses the rollback
    /// journal.
    SyncFolder {
        /// Display name of the provider, for example `OneDrive`.
        provider: String,
    },
    /// A network drive or share. The database uses the rollback journal.
    NetworkDrive,
}

impl StorageLocation {
    /// The journal mode section 9 prescribes for this location.
    pub fn journal_mode(&self) -> JournalMode {
        match self {
            StorageLocation::Local => JournalMode::Wal,
            StorageLocation::SyncFolder { .. } | StorageLocation::NetworkDrive => {
                JournalMode::Rollback
            }
        }
    }

    /// Whether the user must be warned before using this location.
    pub fn needs_warning(&self) -> bool {
        !matches!(self, StorageLocation::Local)
    }
}

/// Facts about the running system that the classification needs. Tests
/// replace it to simulate other systems.
pub trait LocationProbe {
    /// Whether `path` (canonical, possibly not existing yet) is on a network
    /// file system.
    fn is_network(&self, path: &Path) -> bool;
    /// Provider name if the operating system reports `path` (canonical,
    /// possibly not existing yet) to be inside a cloud sync root.
    fn sync_root_provider(&self, path: &Path) -> Option<String>;
    /// Sync roots announced by the providers, with their display names.
    fn known_sync_roots(&self) -> Vec<(PathBuf, String)>;
}

/// Classifies `path`, which need not exist yet (a library about to be
/// created). Uses the real system.
pub fn classify(path: &Path) -> StorageLocation {
    classify_with(path, &SystemProbe::detect())
}

/// Classifies `path` with the given probe.
pub fn classify_with(path: &Path, probe: &dyn LocationProbe) -> StorageLocation {
    let path = canonical_prospective(path).unwrap_or_else(|| path.to_path_buf());

    if probe.is_network(&path) {
        return StorageLocation::NetworkDrive;
    }
    if let Some(provider) = probe.sync_root_provider(&path) {
        return StorageLocation::SyncFolder {
            provider: provider_display_name(&provider),
        };
    }
    for (root, provider) in probe.known_sync_roots() {
        let root = canonical_prospective(&root).unwrap_or(root);
        if path.starts_with(&root) {
            return StorageLocation::SyncFolder { provider };
        }
    }
    if let Some(provider) = provider_from_folder_names(&path) {
        return StorageLocation::SyncFolder {
            provider: provider.to_owned(),
        };
    }
    StorageLocation::Local
}

fn nearest_existing(path: &Path) -> PathBuf {
    let mut current = path;
    loop {
        if current.exists() {
            return current.to_path_buf();
        }
        match current.parent() {
            Some(parent) => current = parent,
            None => return path.to_path_buf(),
        }
    }
}

/// Maps a provider name reported by the system to a short display name.
fn provider_display_name(reported: &str) -> String {
    let lower = reported.to_lowercase();
    let known = [
        ("onedrive", "OneDrive"),
        ("dropbox", "Dropbox"),
        ("google", "Google Drive"),
        ("icloud", "iCloud Drive"),
        ("nextcloud", "Nextcloud"),
        ("owncloud", "ownCloud"),
        ("pcloud", "pCloud"),
        ("box", "Box"),
    ];
    if let Some((_, name)) = known.iter().find(|(needle, _)| lower.contains(needle)) {
        return (*name).to_owned();
    }
    let cleaned: String = reported
        .chars()
        .filter(|c| !c.is_control())
        .take(64)
        .collect();
    let cleaned = cleaned.trim();
    if cleaned.is_empty() {
        "Cloud".to_owned()
    } else {
        cleaned.to_owned()
    }
}

/// Recognises the folder names sync providers create.
pub fn provider_from_folder_names(path: &Path) -> Option<&'static str> {
    let names: Vec<String> = path
        .components()
        .filter_map(|component| match component {
            Component::Normal(name) => Some(name.to_string_lossy().to_lowercase()),
            _ => None,
        })
        .collect();
    for (index, name) in names.iter().enumerate() {
        let name = name.as_str();
        // macOS File Provider: ~/Library/CloudStorage/<Provider>-<Account>
        if index > 0 && names[index - 1] == "cloudstorage" {
            return Some(cloud_storage_provider(name));
        }
        let provider = match name {
            "onedrive" => Some("OneDrive"),
            n if n.starts_with("onedrive - ") || n.starts_with("onedrive-") => Some("OneDrive"),
            "dropbox" => Some("Dropbox"),
            n if n.starts_with("dropbox (") => Some("Dropbox"),
            "google drive" | "googledrive" | "my drive" | "meine ablage" => Some("Google Drive"),
            n if n.starts_with("googledrive-") => Some("Google Drive"),
            "icloud drive" | "iclouddrive" | "mobile documents" | "com~apple~clouddocs" => {
                Some("iCloud Drive")
            }
            "nextcloud" => Some("Nextcloud"),
            "owncloud" => Some("ownCloud"),
            "pcloud drive" | "pclouddrive" => Some("pCloud"),
            _ => None,
        };
        if provider.is_some() {
            return provider;
        }
    }
    None
}

fn cloud_storage_provider(name: &str) -> &'static str {
    let prefix = name.split('-').next().unwrap_or(name);
    match prefix {
        "onedrive" => "OneDrive",
        "dropbox" => "Dropbox",
        "googledrive" => "Google Drive",
        "box" => "Box",
        "icloud" | "iclouddrive" => "iCloud Drive",
        _ => "Cloud",
    }
}

/// File system types (Linux mount table) that are network or cloud mounts.
pub fn is_network_file_system(fs_type: &str) -> bool {
    matches!(
        fs_type,
        "nfs"
            | "nfs4"
            | "cifs"
            | "smb3"
            | "smbfs"
            | "ncpfs"
            | "afs"
            | "ceph"
            | "glusterfs"
            | "9p"
            | "lustre"
            | "gpfs"
            | "davfs"
            | "sshfs"
            | "fuse.sshfs"
            | "fuse.rclone"
            | "fuse.s3fs"
            | "fuse.gcsfuse"
            | "fuse.glusterfs"
            | "fuse.ceph"
            | "fuse.davfs2"
            | "fuse.goofys"
            | "fuse.juicefs"
            | "fuse.onedriver"
            | "fuse.gvfsd-fuse"
    )
}

/// Whether `path` lies on a network mount according to a Linux
/// `/proc/self/mountinfo` text. The deepest mount point containing the path
/// wins; among equal mount points the later (stacked) one.
pub fn network_mount_in_mountinfo(path: &Path, mountinfo: &str) -> bool {
    let mut best: Option<(usize, String)> = None;
    for line in mountinfo.lines() {
        let Some((left, right)) = line.split_once(" - ") else {
            continue;
        };
        let fields: Vec<&str> = left.split(' ').collect();
        let Some(mount_point) = fields.get(4) else {
            continue;
        };
        let mount_point = unescape_mount_field(mount_point);
        let fs_type = right.split(' ').next().unwrap_or_default();
        if path.starts_with(&mount_point) {
            let depth = Path::new(&mount_point).components().count();
            if best.as_ref().is_none_or(|(d, _)| depth >= *d) {
                best = Some((depth, fs_type.to_owned()));
            }
        }
    }
    best.is_some_and(|(_, fs_type)| is_network_file_system(&fs_type))
}

/// Decodes the octal escapes (`\040` for a space) of mount table fields.
fn unescape_mount_field(field: &str) -> String {
    let bytes = field.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        let escaped = bytes[i] == b'\\'
            && i + 4 <= bytes.len()
            && bytes[i + 1..i + 4]
                .iter()
                .all(|b| (b'0'..=b'7').contains(b));
        let value = escaped.then(|| {
            bytes[i + 1..i + 4]
                .iter()
                .fold(0u16, |acc, b| acc * 8 + u16::from(b - b'0'))
        });
        if let Some(value) = value.and_then(|v| u8::try_from(v).ok()) {
            out.push(value);
            i += 4;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Sync roots announced in Dropbox's `info.json`
/// (`{"personal": {"path": …}, "business": {"path": …}}`).
pub fn dropbox_roots_from_info(json: &str) -> Vec<PathBuf> {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(json) else {
        return Vec::new();
    };
    let Some(accounts) = value.as_object() else {
        return Vec::new();
    };
    accounts
        .values()
        .filter_map(|account| account.get("path")?.as_str())
        .filter(|path| !path.is_empty())
        .map(PathBuf::from)
        .collect()
}

/// The probe for the running system.
pub struct SystemProbe {
    roots: Vec<(PathBuf, String)>,
    #[cfg(target_os = "linux")]
    mountinfo: Option<String>,
}

impl SystemProbe {
    /// Reads environment variables and provider configuration once.
    pub fn detect() -> Self {
        Self {
            roots: system_sync_roots(),
            #[cfg(target_os = "linux")]
            mountinfo: std::fs::read_to_string("/proc/self/mountinfo").ok(),
        }
    }
}

impl LocationProbe for SystemProbe {
    fn is_network(&self, path: &Path) -> bool {
        // The system calls need an existing path; the library folder itself
        // may not exist yet.
        let existing = nearest_existing(path);
        #[cfg(windows)]
        {
            super::windows::is_network_path(&existing)
        }
        #[cfg(target_os = "linux")]
        {
            self.mountinfo
                .as_deref()
                .is_some_and(|text| network_mount_in_mountinfo(&existing, text))
        }
        #[cfg(not(any(windows, target_os = "linux")))]
        {
            // macOS: not detected yet (macOS is paused, see docs/status.md).
            let _ = existing;
            false
        }
    }

    fn sync_root_provider(&self, path: &Path) -> Option<String> {
        #[cfg(windows)]
        {
            super::windows::cloud_sync_provider(&nearest_existing(path))
        }
        #[cfg(not(windows))]
        {
            let _ = path;
            None
        }
    }

    fn known_sync_roots(&self) -> Vec<(PathBuf, String)> {
        self.roots.clone()
    }
}

/// Reads at most 64 KiB of a small configuration file.
fn read_small(path: &Path) -> Option<String> {
    let data = super::atomic::read_limited(path, 64 * 1024).ok()?;
    String::from_utf8(data).ok()
}

fn system_sync_roots() -> Vec<(PathBuf, String)> {
    let mut roots = Vec::new();
    let env_path = |name: &str| {
        std::env::var_os(name)
            .filter(|v| !v.is_empty())
            .map(PathBuf::from)
    };

    #[cfg(windows)]
    {
        for name in ["OneDrive", "OneDriveConsumer", "OneDriveCommercial"] {
            if let Some(path) = env_path(name) {
                roots.push((path, "OneDrive".to_owned()));
            }
        }
        if let Some(profile) = env_path("USERPROFILE") {
            roots.push((profile.join("iCloudDrive"), "iCloud Drive".to_owned()));
        }
        for base in ["APPDATA", "LOCALAPPDATA"] {
            if let Some(dir) = env_path(base)
                && let Some(text) = read_small(&dir.join("Dropbox").join("info.json"))
            {
                for path in dropbox_roots_from_info(&text) {
                    roots.push((path, "Dropbox".to_owned()));
                }
            }
        }
    }
    #[cfg(not(windows))]
    {
        if let Some(home) = env_path("HOME") {
            if let Some(text) = read_small(&home.join(".dropbox").join("info.json")) {
                for path in dropbox_roots_from_info(&text) {
                    roots.push((path, "Dropbox".to_owned()));
                }
            }
            #[cfg(target_os = "macos")]
            roots.push((
                home.join("Library").join("Mobile Documents"),
                "iCloud Drive".to_owned(),
            ));
        }
    }
    roots
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A probe for a simulated system.
    #[derive(Default)]
    struct FakeProbe {
        network: Vec<PathBuf>,
        sync_roots: Vec<(PathBuf, String)>,
        known: Vec<(PathBuf, String)>,
    }

    impl LocationProbe for FakeProbe {
        fn is_network(&self, path: &Path) -> bool {
            self.network.iter().any(|root| path.starts_with(root))
        }
        fn sync_root_provider(&self, path: &Path) -> Option<String> {
            self.sync_roots
                .iter()
                .find(|(root, _)| path.starts_with(root))
                .map(|(_, name)| name.clone())
        }
        fn known_sync_roots(&self) -> Vec<(PathBuf, String)> {
            self.known.clone()
        }
    }

    fn sync(provider: &str) -> StorageLocation {
        StorageLocation::SyncFolder {
            provider: provider.to_owned(),
        }
    }

    #[test]
    fn folder_names_of_the_providers_are_recognised() {
        for (path, provider) in [
            ("/home/a/OneDrive/Dokumente/ProjectWrite", "OneDrive"),
            (
                "C:/Users/a/OneDrive - Contoso GmbH/ProjectWrite",
                "OneDrive",
            ),
            (
                "/Users/a/Library/CloudStorage/OneDrive-Personal/x",
                "OneDrive",
            ),
            ("/home/a/Dropbox/x", "Dropbox"),
            ("C:/Users/a/Dropbox (Business)/x", "Dropbox"),
            ("/Users/a/Library/CloudStorage/Dropbox/x", "Dropbox"),
            ("G:/My Drive/ProjectWrite", "Google Drive"),
            ("G:/Meine Ablage/ProjectWrite", "Google Drive"),
            ("/home/a/Google Drive/x", "Google Drive"),
            (
                "/Users/a/Library/CloudStorage/GoogleDrive-a@b.de/x",
                "Google Drive",
            ),
            ("C:/Users/a/iCloudDrive/x", "iCloud Drive"),
            (
                "/Users/a/Library/Mobile Documents/com~apple~CloudDocs/x",
                "iCloud Drive",
            ),
            ("/home/a/Nextcloud/x", "Nextcloud"),
            ("/Users/a/Library/CloudStorage/Box-Box/x", "Box"),
        ] {
            assert_eq!(
                provider_from_folder_names(Path::new(path)),
                Some(provider),
                "{path}"
            );
        }
        for path in [
            "/home/a/Dokumente/ProjectWrite",
            "/home/a/OneDriveBackup/x",
            "/home/a/my-dropbox-notes/x",
            "C:/Users/a/Documents/ProjectWrite",
        ] {
            assert_eq!(provider_from_folder_names(Path::new(path)), None, "{path}");
        }
    }

    /// The canonical form `classify_with` hands to the probe (on Windows
    /// `/net` becomes `D:\\net`, for example).
    fn canonical(path: &str) -> PathBuf {
        canonical_prospective(Path::new(path)).unwrap()
    }

    #[test]
    fn network_beats_sync_root_beats_known_roots_beats_names() {
        let probe = FakeProbe {
            network: vec![canonical("/net")],
            sync_roots: vec![(canonical("/cloud"), "Dropbox, Inc.".to_owned())],
            known: vec![(PathBuf::from("/announced"), "OneDrive".to_owned())],
        };
        assert_eq!(
            classify_with(Path::new("/net/OneDrive/lib"), &probe),
            StorageLocation::NetworkDrive
        );
        assert_eq!(
            classify_with(Path::new("/cloud/OneDrive/lib"), &probe),
            sync("Dropbox")
        );
        assert_eq!(
            classify_with(Path::new("/announced/Dropbox/lib"), &probe),
            sync("OneDrive")
        );
        assert_eq!(
            classify_with(Path::new("/plain/Dropbox/lib"), &probe),
            sync("Dropbox")
        );
        assert_eq!(
            classify_with(Path::new("/plain/lib"), &probe),
            StorageLocation::Local
        );
    }

    #[test]
    fn journal_mode_follows_the_location() {
        assert_eq!(StorageLocation::Local.journal_mode(), JournalMode::Wal);
        assert_eq!(sync("OneDrive").journal_mode(), JournalMode::Rollback);
        assert_eq!(
            StorageLocation::NetworkDrive.journal_mode(),
            JournalMode::Rollback
        );
        assert!(!StorageLocation::Local.needs_warning());
        assert!(StorageLocation::NetworkDrive.needs_warning());
    }

    #[test]
    fn reported_provider_names_are_shortened() {
        assert_eq!(provider_display_name("OneDrive - Personal"), "OneDrive");
        assert_eq!(
            provider_display_name("Google Drive for desktop"),
            "Google Drive"
        );
        assert_eq!(provider_display_name("iCloud"), "iCloud Drive");
        assert_eq!(provider_display_name("Acme Sync\u{7}"), "Acme Sync");
        assert_eq!(provider_display_name("  "), "Cloud");
        assert_eq!(provider_display_name(&"x".repeat(100)).len(), 64);
    }

    #[test]
    fn mountinfo_network_detection() {
        let mountinfo = "\
22 1 8:1 / / rw,relatime shared:1 - ext4 /dev/sda1 rw
30 22 0:40 / /mnt/share rw,relatime shared:5 - cifs //server/share rw
31 22 0:41 / /mnt/nfs rw,relatime shared:6 - nfs4 server:/export rw
32 30 8:2 / /mnt/share/local rw,relatime shared:7 - ext4 /dev/sdb1 rw
33 22 0:42 / /home/a/My\\040Cloud rw,relatime shared:8 - fuse.rclone remote: rw
34 22 0:43 / /run/user/1000/gvfs rw,nosuid shared:9 - fuse.gvfsd-fuse gvfsd-fuse rw
";
        for (path, expected) in [
            ("/home/a/Dokumente/ProjectWrite", false),
            ("/mnt/share/ProjectWrite", true),
            ("/mnt/share/local/ProjectWrite", false),
            ("/mnt/nfs/x", true),
            ("/mnt/sharex/y", false),
            ("/home/a/My Cloud/lib", true),
            ("/run/user/1000/gvfs/smb-share:server=x,share=y/lib", true),
        ] {
            assert_eq!(
                network_mount_in_mountinfo(Path::new(path), mountinfo),
                expected,
                "{path}"
            );
        }
        assert!(!network_mount_in_mountinfo(
            Path::new("/x"),
            "garbage\n- -\n"
        ));
    }

    #[test]
    fn dropbox_info_json_is_parsed_defensively() {
        let roots = dropbox_roots_from_info(
            r#"{"personal": {"path": "/home/a/Dropbox", "host": 1}, "business": {"path": "/home/a/Dropbox (Firma)"}}"#,
        );
        assert_eq!(roots.len(), 2);
        assert!(roots.contains(&PathBuf::from("/home/a/Dropbox (Firma)")));
        assert!(dropbox_roots_from_info("not json").is_empty());
        assert!(dropbox_roots_from_info(r#"["a"]"#).is_empty());
        assert!(dropbox_roots_from_info(r#"{"personal": {"path": 5}}"#).is_empty());
    }
}
