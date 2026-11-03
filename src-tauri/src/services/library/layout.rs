//! Fixed names inside a library folder (section 9, "Speicherstruktur").
//!
//! All paths are relative to the library root and resolved through
//! [`crate::security::paths::SafeRoot`]. Folders other than [`INITIAL_DIRS`]
//! are created by the phase that first writes into them.

/// Library id and format version.
pub const MANIFEST_FILE: &str = "library.json";
/// Held with an exclusive operating-system lock while the library is open.
pub const LOCK_FILE: &str = "library.lock";
/// The SQLite database.
pub const DATABASE_FILE: &str = "database/projectwrite.db";
/// Files SQLite creates next to the database. They must not be links either.
pub const DATABASE_SIDE_FILES: &[&str] = &[
    "database/projectwrite.db-wal",
    "database/projectwrite.db-shm",
    "database/projectwrite.db-journal",
];
/// Intermediate files; emptied every time the library is opened.
pub const TMP_DIR: &str = "tmp";
/// Backups, if no other destination is chosen.
pub const BACKUPS_DIR: &str = "backups";
/// Database copies made before a schema migration (section 18).
pub const PRE_MIGRATION_BACKUPS_DIR: &str = "backups/pre-migration";
/// How many pre-migration copies are kept.
pub const PRE_MIGRATION_BACKUPS_KEPT: usize = 5;

/// Folders every new library gets.
pub const INITIAL_DIRS: &[&str] = &[
    "database",
    "documents/text",
    "documents/markdown",
    "documents/richtext",
];

/// Top-level entries of the structure in section 9. Used to tell a library
/// skeleton left by an interrupted creation from foreign content.
pub const KNOWN_TOP_LEVEL: &[&str] = &[
    MANIFEST_FILE,
    LOCK_FILE,
    "database",
    "documents",
    "media",
    "thumbnails",
    "recovery",
    TMP_DIR,
    BACKUPS_DIR,
    "exports",
];

/// Files operating systems put into folders on their own. A folder that
/// contains only these counts as empty.
pub const SYSTEM_JUNK_FILES: &[&str] = &[".DS_Store", "Thumbs.db", "desktop.ini", ".directory"];

/// Every fixed path that must never be a link. Checked on every open.
pub const GUARDED_PATHS: &[&str] = &[
    MANIFEST_FILE,
    LOCK_FILE,
    "database",
    DATABASE_FILE,
    "database/projectwrite.db-wal",
    "database/projectwrite.db-shm",
    "database/projectwrite.db-journal",
    "documents",
    "documents/text",
    "documents/markdown",
    "documents/richtext",
    "media",
    "thumbnails",
    "recovery",
    TMP_DIR,
    BACKUPS_DIR,
    PRE_MIGRATION_BACKUPS_DIR,
    "exports",
];
