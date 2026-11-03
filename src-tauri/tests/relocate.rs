//! Moving a library (section 9): copy, check, switch; the old folder stays.

mod common;

use std::fs;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use project_write_lib::clock::Generators;
use project_write_lib::database::{JournalMode, diagnostics};
use project_write_lib::security::paths;
use project_write_lib::services::documents::DocumentFormat;
use project_write_lib::services::library::relocate::{
    MoveError, MovePhase, MoveProgress, move_library,
};
use project_write_lib::services::library::{Library, LibraryError, OpenOptions};
use uuid::Uuid;

async fn library_with_documents(path: &Path) -> (Library, Vec<(Uuid, Vec<u8>)>) {
    let (library, _) = Library::create(path, Generators::system()).await.unwrap();
    let docs = library.documents();
    let mut stored = Vec::new();
    for (i, format) in [
        DocumentFormat::Txt,
        DocumentFormat::Md,
        DocumentFormat::Pwrite,
    ]
    .into_iter()
    .enumerate()
    {
        let content = format!("Dokument {i}: {}", "Inhalt ".repeat(200 * (i + 1))).into_bytes();
        let meta = docs
            .create(&format!("Titel {i}"), format, &content)
            .await
            .unwrap();
        stored.push((meta.id, content));
    }
    (library, stored)
}

fn names(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> = fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names
}

#[tokio::test]
async fn copy_check_switch_and_keep_the_old_folder() {
    let dir = common::temp_dir("pw-move-");
    let old = dir.path().join("alt");
    let (library, stored) = library_with_documents(&old).await;
    let id = library.manifest().id;
    // Content that must travel, and content that must not.
    fs::create_dir_all(old.join("backups")).unwrap();
    fs::write(old.join("backups").join("früher.pwbackup"), b"backup").unwrap();
    fs::create_dir_all(old.join("tmp")).unwrap();
    fs::write(old.join("tmp").join("zwischen.bin"), b"tmp").unwrap();

    let events = Arc::new(Mutex::new(Vec::<MoveProgress>::new()));
    let recorder = events.clone();
    let new = dir.path().join("neu").join("ProjectWrite");
    let (library, report) =
        move_library(library, &new, Arc::new(AtomicBool::new(false)), move |p| {
            recorder.lock().unwrap().push(p);
        })
        .await
        .map_err(|(_, e)| e)
        .unwrap();

    assert_eq!(
        library.path(),
        paths::canonical_dir(&new).unwrap().as_path()
    );
    assert_eq!(library.manifest().id, id);
    assert_eq!(library.manifest().moved_to, None);
    assert_eq!(report.files, 4, "3 documents and 1 backup");
    assert!(report.old_copy_marked);
    for (doc, content) in &stored {
        assert_eq!(&library.documents().read(*doc).await.unwrap().1, content);
    }
    assert!(new.join("backups").join("früher.pwbackup").is_file());
    assert!(!new.join("tmp").join("zwischen.bin").exists());
    assert!(
        diagnostics::integrity_check(library.database().reader())
            .await
            .unwrap()
    );

    // Progress: copying, then checking, then switching; byte counts grow.
    let events = events.lock().unwrap().clone();
    let phases: Vec<MovePhase> = events.iter().map(|e| e.phase).collect();
    assert_eq!(phases.first(), Some(&MovePhase::Copying));
    assert_eq!(phases.last(), Some(&MovePhase::Switching));
    assert!(phases.contains(&MovePhase::Verifying));
    let copying: Vec<u64> = events
        .iter()
        .filter(|e| e.phase == MovePhase::Copying)
        .map(|e| e.done_bytes)
        .collect();
    assert!(copying.windows(2).all(|w| w[0] <= w[1]));

    // The old folder is still complete, marked as moved.
    for (doc, content) in &stored {
        let file = old.join(stored_format(&library, *doc).await.relative_path(*doc));
        assert_eq!(&fs::read(file).unwrap(), content);
    }
    library.close().await;
    match Library::open(&old, OpenOptions::default(), Generators::system()).await {
        Err(LibraryError::Moved(moved)) => {
            assert!(moved.path.ends_with("ProjectWrite"), "{}", moved.path);
        }
        other => panic!("expected Moved, got {other:?}"),
    }
}

async fn stored_format(library: &Library, id: Uuid) -> DocumentFormat {
    library.documents().get(id).await.unwrap().unwrap().format
}

#[tokio::test]
async fn invalid_targets_are_refused_and_the_library_stays_open() {
    let dir = common::temp_dir("pw-move-");
    let old = dir.path().join("lib");
    let (mut library, stored) = library_with_documents(&old).await;

    let occupied = dir.path().join("belegt");
    fs::create_dir(&occupied).unwrap();
    fs::write(occupied.join("eigene Datei.txt"), b"x").unwrap();

    for (target, expected) in [
        (old.join("documents").join("sub"), "inside"),
        (dir.path().to_path_buf(), "contains"),
        (occupied.clone(), "not empty"),
    ] {
        let (returned, error) =
            move_library(library, &target, Arc::new(AtomicBool::new(false)), |_| {})
                .await
                .map(|_| ())
                .unwrap_err();
        match (&error, expected) {
            (MoveError::TargetInsideLibrary, "inside")
            | (MoveError::LibraryInsideTarget, "contains")
            | (MoveError::TargetNotEmpty, "not empty") => {}
            _ => panic!("{target:?}: unexpected {error:?}"),
        }
        library = returned.expect("the old library is still open");
    }
    assert_eq!(names(&occupied), ["eigene Datei.txt"]);
    assert!(!old.join("documents").join("sub").exists());
    // Still usable.
    let (doc, content) = &stored[0];
    assert_eq!(&library.documents().read(*doc).await.unwrap().1, content);
    library.documents().save(*doc, b"neu").await.unwrap();
    library.close().await;
}

#[tokio::test]
async fn cancelling_removes_only_what_the_move_created() {
    let dir = common::temp_dir("pw-move-");
    let old = dir.path().join("lib");
    let (library, stored) = library_with_documents(&old).await;

    // Target exists and holds a system file; cancel after the first file.
    let target = dir.path().join("ziel");
    fs::create_dir(&target).unwrap();
    fs::write(target.join(".DS_Store"), b"system").unwrap();
    let cancel = Arc::new(AtomicBool::new(false));
    let trigger = cancel.clone();
    let (returned, error) = move_library(library, &target, cancel, move |progress| {
        if progress.done_files >= 1 {
            trigger.store(true, Ordering::SeqCst);
        }
    })
    .await
    .map(|_| ())
    .unwrap_err();
    assert!(matches!(error, MoveError::Cancelled), "{error:?}");
    assert_eq!(names(&target), [".DS_Store"]);
    let library = returned.unwrap();
    assert_eq!(
        &library.documents().read(stored[1].0).await.unwrap().1,
        &stored[1].1
    );

    // Cancelled before anything was copied: the created folder disappears.
    let cancel = Arc::new(AtomicBool::new(true));
    let fresh = dir.path().join("neu");
    let (returned, error) = move_library(library, &fresh, cancel, |_| {})
        .await
        .map(|_| ())
        .unwrap_err();
    assert!(matches!(error, MoveError::Cancelled));
    assert!(!fresh.exists());
    let library = returned.unwrap();
    assert!(
        Library::open(&old, OpenOptions::default(), Generators::system())
            .await
            .is_err_and(|e| matches!(e, LibraryError::Locked))
    );
    library.close().await;
}

#[tokio::test]
async fn moving_into_a_sync_folder_switches_to_the_rollback_journal() {
    let dir = common::temp_dir("pw-move-");
    let (library, stored) = library_with_documents(&dir.path().join("lokal")).await;
    assert_eq!(library.journal_mode(), JournalMode::Wal);
    let target = dir.path().join("Dropbox").join("ProjectWrite");
    let (library, _) = move_library(library, &target, Arc::new(AtomicBool::new(false)), |_| {})
        .await
        .map_err(|(_, e)| e)
        .unwrap();
    assert_eq!(library.journal_mode(), JournalMode::Rollback);
    assert_eq!(
        diagnostics::journal_mode(library.database().writer())
            .await
            .unwrap(),
        "delete"
    );
    assert_eq!(
        &library.documents().read(stored[2].0).await.unwrap().1,
        &stored[2].1
    );
    library.close().await;
}

#[tokio::test]
async fn a_link_inside_the_library_stops_the_move_before_copying() {
    let dir = common::temp_dir("pw-move-");
    let old = dir.path().join("lib");
    let (library, _) = library_with_documents(&old).await;
    let outside = dir.path().join("outside");
    fs::create_dir(&outside).unwrap();
    fs::write(outside.join("geheim.txt"), b"secret").unwrap();
    common::symlink_dir(&outside, &old.join("media"))
        .expect("create directory symlink (Windows: needs admin rights or developer mode)");

    let target = dir.path().join("ziel");
    let (returned, error) =
        move_library(library, &target, Arc::new(AtomicBool::new(false)), |_| {})
            .await
            .map(|_| ())
            .unwrap_err();
    assert!(matches!(error, MoveError::UnsafePath(_)), "{error:?}");
    assert!(!target.exists());
    returned.unwrap().close().await;
}
