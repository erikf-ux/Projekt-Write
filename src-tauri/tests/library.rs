//! Library lifecycle (Phase 2, sections 9 and 18): create, open, lock,
//! rollback journal in sync folders, housekeeping, backup before migrating.
//!
//! Acceptance criteria covered here:
//! * "Eine zweite Instanz kann dieselbe Bibliothek nicht öffnen." – same
//!   process here, a second process in tests/library_lock.rs.
//! * "Sync-Ordner … Wird trotzdem dort gespeichert, läuft die Datenbank im
//!   Rollback-Modus."
//! * "Pfadangriffe wie `..` und symbolische Links werden abgelehnt." – links
//!   inside the library structure.

mod common;

use std::fs;
use std::path::{Path, PathBuf};

use project_write_lib::clock::Generators;
use project_write_lib::database::{Database, DatabaseError, JournalMode, diagnostics};
use project_write_lib::filesystem::location::StorageLocation;
use project_write_lib::security::paths::SafeRoot;
use project_write_lib::services::documents::DocumentFormat;
use project_write_lib::services::library::manifest::{LibraryManifest, MovedTo};
use project_write_lib::services::library::{Library, LibraryError, OpenOptions};
use uuid::Uuid;

async fn create(path: &Path) -> Library {
    Library::create(path, Generators::system())
        .await
        .expect("create library")
        .0
}

async fn open(path: &Path) -> Result<Library, LibraryError> {
    Library::open(path, OpenOptions::default(), Generators::system())
        .await
        .map(|(library, _)| library)
}

#[tokio::test]
async fn create_builds_the_structure_and_reopens() {
    let dir = common::temp_dir("pw-lib-");
    let path = dir.path().join("ProjectWrite");
    let library = create(&path).await;

    for file in ["library.json", "library.lock", "database/projectwrite.db"] {
        assert!(path.join(file).is_file(), "{file}");
    }
    for folder in ["documents/text", "documents/markdown", "documents/richtext"] {
        assert!(path.join(folder).is_dir(), "{folder}");
    }
    let manifest = LibraryManifest::read(&SafeRoot::new(&path).unwrap()).unwrap();
    assert_eq!(manifest.id, library.manifest().id);
    assert_eq!(manifest.format_version, 1);
    assert_eq!(*library.location(), StorageLocation::Local);
    assert_eq!(
        diagnostics::journal_mode(library.database().writer())
            .await
            .unwrap(),
        "wal"
    );
    assert_eq!(
        library.database().schema_version().await.unwrap(),
        Database::supported_version()
    );
    assert!(
        diagnostics::foreign_keys_enabled(library.database().writer())
            .await
            .unwrap()
    );
    library.close().await;

    let (library, report) = Library::open(&path, OpenOptions::default(), Generators::system())
        .await
        .unwrap();
    assert_eq!(report.schema_version_before, Database::supported_version());
    assert_eq!(report.schema_version, Database::supported_version());
    assert_eq!(report.migration_backup, None);
    assert!(report.documents.orphaned_files.is_empty());
    assert!(report.documents.missing_files.is_empty());
    assert_eq!(library.manifest().id, manifest.id);
    library.close().await;
}

#[tokio::test]
async fn create_needs_an_empty_folder() {
    let dir = common::temp_dir("pw-lib-");

    let junk_only = dir.path().join("junk");
    fs::create_dir(&junk_only).unwrap();
    fs::write(junk_only.join(".DS_Store"), b"x").unwrap();
    fs::write(junk_only.join("desktop.ini"), b"x").unwrap();
    create(&junk_only).await.close().await;

    let foreign = dir.path().join("foreign");
    fs::create_dir(&foreign).unwrap();
    fs::write(foreign.join("Steuererklärung.pdf"), b"x").unwrap();
    assert!(matches!(
        Library::create(&foreign, Generators::system()).await,
        Err(LibraryError::FolderNotEmpty)
    ));
    // Nothing was added to the foreign folder.
    assert_eq!(fs::read_dir(&foreign).unwrap().count(), 1);

    let foreign_documents = dir.path().join("documents-of-user");
    fs::create_dir_all(foreign_documents.join("documents")).unwrap();
    fs::write(foreign_documents.join("documents").join("brief.txt"), b"x").unwrap();
    assert!(matches!(
        Library::create(&foreign_documents, Generators::system()).await,
        Err(LibraryError::FolderNotEmpty)
    ));

    assert!(matches!(
        Library::create(&junk_only, Generators::system()).await,
        Err(LibraryError::AlreadyALibrary)
    ));
}

#[tokio::test]
async fn create_resumes_an_interrupted_creation() {
    let dir = common::temp_dir("pw-lib-");
    let path = dir.path().join("lib");
    // What an interrupted creation leaves: lock, folders, a database, no
    // library.json.
    fs::create_dir_all(path.join("documents").join("text")).unwrap();
    fs::create_dir_all(path.join("database")).unwrap();
    fs::write(path.join("library.lock"), b"").unwrap();
    Database::open_file(
        &path.join("database").join("projectwrite.db"),
        JournalMode::Wal,
    )
    .await
    .unwrap()
    .close()
    .await;
    create(&path).await.close().await;
    assert!(path.join("library.json").is_file());
}

#[tokio::test]
async fn folders_without_a_library_are_rejected() {
    let dir = common::temp_dir("pw-lib-");
    assert!(matches!(
        open(&dir.path().join("missing")).await,
        Err(LibraryError::NotALibrary)
    ));
    assert!(matches!(
        open(dir.path()).await,
        Err(LibraryError::NotALibrary)
    ));
    // Nothing was written into the folder.
    assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 0);
}

#[tokio::test]
async fn a_second_open_in_the_same_process_is_refused() {
    let dir = common::temp_dir("pw-lib-");
    let path = dir.path().join("lib");
    let first = create(&path).await;
    assert!(matches!(open(&path).await, Err(LibraryError::Locked)));
    first.close().await;
    open(&path).await.unwrap().close().await;
}

#[tokio::test]
async fn newer_library_format_is_rejected_without_changes() {
    let dir = common::temp_dir("pw-lib-");
    let path = dir.path().join("lib");
    create(&path).await.close().await;
    let manifest_path = path.join("library.json");
    let mut json: serde_json::Value =
        serde_json::from_slice(&fs::read(&manifest_path).unwrap()).unwrap();
    json["formatVersion"] = 2.into();
    fs::write(&manifest_path, json.to_string()).unwrap();
    let before = snapshot(&path);

    assert!(matches!(
        open(&path).await,
        Err(LibraryError::FormatTooNew {
            found: 2,
            supported: 1
        })
    ));
    assert_eq!(snapshot(&path), before);
}

#[tokio::test]
async fn newer_schema_is_rejected_without_backup_or_changes() {
    let dir = common::temp_dir("pw-lib-");
    let path = dir.path().join("lib");
    let library = create(&path).await;
    sqlx::query(
        "INSERT INTO _sqlx_migrations (version, description, installed_on, success, checksum, execution_time) \
         VALUES (999, 'future', '2030-01-01 00:00:00', 1, x'00', 0)",
    )
    .execute(library.database().writer())
    .await
    .unwrap();
    library.close().await;

    let error = open(&path).await.unwrap_err();
    assert!(
        matches!(
            error,
            LibraryError::Database(DatabaseError::SchemaTooNew { found: 999, .. })
        ),
        "{error:?}"
    );
    assert!(!path.join("backups").exists());
}

#[tokio::test]
async fn moved_library_needs_confirmation() {
    let dir = common::temp_dir("pw-lib-");
    let path = dir.path().join("lib");
    let library = create(&path).await;
    let mut manifest = library.manifest().clone();
    library.close().await;
    manifest.moved_to = Some(MovedTo {
        path: "D:\\Neu\\ProjectWrite".to_owned(),
        at: "2026-11-03T10:00:00.000Z".to_owned(),
    });
    manifest.write(&SafeRoot::new(&path).unwrap()).unwrap();

    match open(&path).await {
        Err(LibraryError::Moved(moved)) => assert_eq!(moved.path, "D:\\Neu\\ProjectWrite"),
        other => panic!("expected Moved, got {other:?}"),
    }
    let (library, _) = Library::open(
        &path,
        OpenOptions { accept_moved: true },
        Generators::system(),
    )
    .await
    .unwrap();
    library.close().await;
}

#[tokio::test]
async fn links_inside_the_library_structure_are_rejected() {
    let dir = common::temp_dir("pw-lib-");
    let outside = dir.path().join("outside");
    fs::create_dir(&outside).unwrap();

    for guarded in ["documents/text", "tmp", "database"] {
        let path = dir
            .path()
            .join(format!("lib-{}", guarded.replace('/', "-")));
        create(&path).await.close().await;
        let target = path.join(guarded);
        if target.exists() {
            if guarded == "database" {
                fs::rename(&target, outside.join("moved-database")).unwrap();
            } else {
                fs::remove_dir_all(&target).unwrap();
            }
        }
        common::symlink_dir(&outside, &target)
            .expect("create directory symlink (Windows: needs admin rights or developer mode)");
        match open(&path).await {
            Err(LibraryError::UnsafePath(error)) => {
                assert_eq!(error.relative(), Some(guarded), "{error}");
            }
            other => panic!("{guarded}: expected UnsafePath, got {other:?}"),
        }
    }
    // Nothing was written through the links.
    let names: Vec<_> = fs::read_dir(&outside)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    assert_eq!(names, ["moved-database"]);
}

#[tokio::test]
async fn opening_clears_tmp_without_following_links() {
    let dir = common::temp_dir("pw-lib-");
    let path = dir.path().join("lib");
    create(&path).await.close().await;
    let outside = dir.path().join("outside");
    fs::create_dir_all(&outside).unwrap();
    fs::write(outside.join("keep.txt"), b"keep").unwrap();

    let tmp = path.join("tmp");
    fs::create_dir_all(tmp.join("import").join("nested")).unwrap();
    fs::write(tmp.join("import").join("nested").join("a.bin"), b"x").unwrap();
    fs::write(tmp.join("b.bin"), b"x").unwrap();
    common::symlink_dir(&outside, &tmp.join("link"))
        .expect("create directory symlink (Windows: needs admin rights or developer mode)");

    let (library, report) = Library::open(&path, OpenOptions::default(), Generators::system())
        .await
        .unwrap();
    assert_eq!(report.cleared_tmp_entries, 3);
    assert_eq!(fs::read_dir(&tmp).unwrap().count(), 0);
    assert_eq!(fs::read(outside.join("keep.txt")).unwrap(), b"keep");
    library.close().await;
}

#[tokio::test]
async fn housekeeping_reports_orphans_and_never_deletes_them() {
    let dir = common::temp_dir("pw-lib-");
    let path = dir.path().join("lib");
    let library = create(&path).await;
    let documents = library.documents();
    let kept = documents
        .create("Bleibt", DocumentFormat::Txt, b"Inhalt")
        .await
        .unwrap();
    let lost = documents
        .create("Datei fehlt", DocumentFormat::Md, b"# Inhalt")
        .await
        .unwrap();
    library.close().await;

    let orphan_id = Uuid::new_v4();
    let orphan = format!("documents/text/{orphan_id}.txt");
    fs::write(path.join(&orphan), b"orphan").unwrap();
    fs::write(path.join("documents/text/notizen.txt"), b"foreign").unwrap();
    fs::write(
        path.join("documents/markdown/.x.md.0123456789abcdef.pwtmp"),
        b"half written",
    )
    .unwrap();
    fs::remove_file(path.join(DocumentFormat::Md.relative_path(lost.id))).unwrap();

    let (library, report) = Library::open(&path, OpenOptions::default(), Generators::system())
        .await
        .unwrap();
    assert_eq!(report.documents.removed_temp_files, 1);
    let mut expected = vec!["documents/text/notizen.txt".to_owned(), orphan.clone()];
    expected.sort();
    assert_eq!(report.documents.orphaned_files, expected);
    assert_eq!(report.documents.missing_files, vec![lost.id]);
    // Reported files are still there; the intact document is untouched.
    assert!(path.join(&orphan).is_file());
    assert!(path.join("documents/text/notizen.txt").is_file());
    let (_, content) = library.documents().read(kept.id).await.unwrap();
    assert_eq!(content, b"Inhalt");
    library.close().await;
}

#[tokio::test]
async fn sync_folder_uses_the_rollback_journal() {
    let dir = common::temp_dir("pw-lib-");
    let path = dir.path().join("Dropbox").join("ProjectWrite");
    let library = create(&path).await;
    assert_eq!(
        *library.location(),
        StorageLocation::SyncFolder {
            provider: "Dropbox".to_owned()
        }
    );
    assert_eq!(library.journal_mode(), JournalMode::Rollback);
    assert_eq!(
        diagnostics::journal_mode(library.database().writer())
            .await
            .unwrap(),
        "delete"
    );
    library.close().await;
    assert!(!path.join("database/projectwrite.db-wal").exists());

    let library = open(&path).await.unwrap();
    assert_eq!(
        diagnostics::journal_mode(library.database().writer())
            .await
            .unwrap(),
        "delete"
    );
    library.close().await;
}

#[tokio::test]
async fn a_library_moved_into_a_sync_folder_switches_from_wal_to_rollback() {
    let dir = common::temp_dir("pw-lib-");
    let local = dir.path().join("lokal");
    let library = create(&local).await;
    library
        .documents()
        .create("Notiz", DocumentFormat::Txt, b"Text")
        .await
        .unwrap();
    library.close().await;
    // The user moves the folder by hand.
    let synced = dir.path().join("OneDrive - Firma").join("lokal");
    fs::create_dir_all(synced.parent().unwrap()).unwrap();
    fs::rename(&local, &synced).unwrap();

    let library = open(&synced).await.unwrap();
    assert_eq!(library.journal_mode(), JournalMode::Rollback);
    assert_eq!(
        diagnostics::journal_mode(library.database().writer())
            .await
            .unwrap(),
        "delete"
    );
    assert_eq!(library.documents().count().await.unwrap(), 1);
    library.close().await;
}

#[tokio::test]
async fn damaged_database_is_reported_and_left_alone() {
    let dir = common::temp_dir("pw-lib-");
    let path = dir.path().join("lib");
    create(&path).await.close().await;
    let database = path.join("database/projectwrite.db");
    for side in ["-wal", "-shm"] {
        let _ = fs::remove_file(format!("{}{side}", database.display()));
    }
    fs::write(&database, b"this is not a database at all, just some bytes").unwrap();
    let before = fs::read(&database).unwrap();
    assert!(matches!(
        open(&path).await,
        Err(LibraryError::DatabaseDamaged)
    ));
    assert_eq!(fs::read(&database).unwrap(), before);

    common::remove_file_eventually(&database);
    assert!(matches!(
        open(&path).await,
        Err(LibraryError::DatabaseMissing)
    ));
    assert!(!database.exists());
}

#[tokio::test]
async fn pending_migrations_are_preceded_by_a_checked_backup() {
    let dir = common::temp_dir("pw-lib-");
    let path = dir.path().join("lib");
    // A library written by a version that knew only schema 1.
    create_library_at_schema(&path, 1).await;
    let database =
        Database::open_existing(&path.join("database/projectwrite.db"), JournalMode::Wal)
            .await
            .unwrap();
    sqlx::query(
        "INSERT INTO application_settings (key, value, updated_at) VALUES ('test.key', '\"Wert\"', '2026-11-01T10:00:00.000Z')",
    )
    .execute(database.writer())
    .await
    .unwrap();
    database.close().await;

    let (library, report) = Library::open(&path, OpenOptions::default(), Generators::system())
        .await
        .unwrap();
    assert_eq!(report.schema_version_before, 1);
    assert_eq!(report.schema_version, Database::supported_version());
    let backup = report.migration_backup.expect("backup before migrating");
    assert!(
        backup.starts_with("backups/pre-migration/projectwrite-"),
        "{backup}"
    );
    assert!(backup.ends_with(".pwbackup"), "{backup}");
    library.close().await;

    // Since Phase 10 a full backup (ADR 0021): restored, its database is
    // a complete, intact schema-1 database with the data.
    let restored = dir.path().join("restored");
    project_write_lib::services::backup::restore(
        &path.join(&backup),
        &restored,
        std::sync::Arc::new(|_| {}),
    )
    .await
    .unwrap();
    let copy = restored.join("database/projectwrite.db");
    assert!(diagnostics::integrity_check_file(&copy).await.unwrap());
    let copy_db = Database::open_existing(&copy, JournalMode::Rollback)
        .await
        .unwrap();
    assert_eq!(copy_db.schema_version().await.unwrap(), 1);
    let value: String =
        sqlx::query_scalar("SELECT value FROM application_settings WHERE key = 'test.key'")
            .fetch_one(copy_db.reader())
            .await
            .unwrap();
    assert_eq!(value, "\"Wert\"");
    copy_db.close().await;
}

#[tokio::test]
async fn only_the_newest_pre_migration_backups_are_kept() {
    let dir = common::temp_dir("pw-lib-");
    let path = dir.path().join("lib");
    create_library_at_schema(&path, 1).await;
    let backups = path.join("backups/pre-migration");
    fs::create_dir_all(&backups).unwrap();
    for day in 1..=6 {
        fs::write(
            backups.join(format!(
                "projectwrite-schema-1-202610{day:02}T000000000Z.db"
            )),
            b"old",
        )
        .unwrap();
    }
    fs::write(backups.join("my-own-copy.db"), b"user file").unwrap();
    let (library, report) = Library::open(&path, OpenOptions::default(), Generators::system())
        .await
        .unwrap();
    library.close().await;
    let mut names: Vec<String> = fs::read_dir(&backups)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    // Copies of earlier versions: of 6 the 5 newest stay; the full backup
    // of this migration and foreign files stay too.
    assert_eq!(names.len(), 7, "{names:?}");
    assert!(names.contains(&"my-own-copy.db".to_owned()));
    assert!(!names.iter().any(|n| n.contains("20261001")));
    assert!(names.iter().any(|n| n.contains("20261002")));
    let newest = Path::new(&report.migration_backup.unwrap())
        .file_name()
        .unwrap()
        .to_string_lossy()
        .into_owned();
    assert!(names.contains(&newest));
}

/// Builds a library folder whose database stops at `version`.
async fn create_library_at_schema(path: &Path, version: i64) {
    create(path).await.close().await;
    let database_path = path.join("database/projectwrite.db");
    for suffix in ["", "-wal", "-shm"] {
        let _ = fs::remove_file(format!("{}{suffix}", database_path.display()));
    }
    let database = Database::open_file(&database_path, JournalMode::Wal)
        .await
        .unwrap();
    database.migrate_to(version).await.unwrap();
    assert_eq!(database.schema_version().await.unwrap(), version);
    database.close().await;
}

/// File names and contents below `path`, for "nothing changed" checks.
fn snapshot(path: &Path) -> Vec<(PathBuf, Vec<u8>)> {
    let mut entries = Vec::new();
    let mut stack = vec![path.to_path_buf()];
    while let Some(dir) = stack.pop() {
        for entry in fs::read_dir(&dir).unwrap() {
            let entry = entry.unwrap();
            let entry_path = entry.path();
            if entry.file_type().unwrap().is_dir() {
                stack.push(entry_path);
            } else {
                let content = fs::read(&entry_path).unwrap_or_default();
                entries.push((entry_path, content));
            }
        }
    }
    entries.sort();
    entries
}
