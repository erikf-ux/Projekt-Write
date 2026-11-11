//! Full backups and restoring from them, stage 5 (Phase 10, section 21,
//! ADR 0021).
//!
//! Acceptance criteria of Phase 10 in Rust: a backup while documents are
//! being saved is consistent; a hard kill while backing up leaves no file
//! that looks finished; after restoring every checksum matches; a changed
//! or damaged backup is refused; a backup holds no earlier backups.

mod common;

use std::collections::BTreeMap;
use std::fs;
use std::io::{BufRead, BufReader, Read, Write};
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::time::{Duration, Instant};

use common::SplitMix64;
use project_write_lib::clock::Generators;
use project_write_lib::database::diagnostics;
use project_write_lib::services::backup::{self, BackupError, BackupManifest, BackupProgress};
use project_write_lib::services::documents::{DocumentFormat, sha256_hex};
use project_write_lib::services::library::{Library, LibraryError, OpenOptions};
use project_write_lib::services::media::{Decision, SubnetPlacement};
use project_write_lib::services::pwrite;
use uuid::Uuid;

/// One entry of a ZIP: name and bytes.
type Entry = (String, Vec<u8>);

const SVG: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" width="20" height="20"><rect width="20" height="20" fill="#3a7"/></svg>"##;

fn no_progress() -> Arc<dyn Fn(BackupProgress) + Send + Sync> {
    Arc::new(|_| {})
}

fn not_cancelled() -> Arc<AtomicBool> {
    Arc::new(AtomicBool::new(false))
}

async fn create(path: &Path) -> Library {
    Library::create(path, Generators::system()).await.unwrap().0
}

async fn open(path: &Path) -> Library {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        match Library::open(path, OpenOptions::default(), Generators::system()).await {
            Ok((library, _)) => return library,
            Err(LibraryError::Locked) if Instant::now() < deadline => {
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
            Err(error) => panic!("open failed: {error}"),
        }
    }
}

/// A library with documents in all three formats, media and structure.
async fn filled(path: &Path) -> Library {
    let library = create(path).await;
    let documents = library.documents();
    documents
        .create("Notiz", DocumentFormat::Txt, b"Einfacher Text\n")
        .await
        .unwrap();
    documents
        .create(
            "Kapitel",
            DocumentFormat::Md,
            "# Kapitel\n\nMit **Müller** und Straße.\n".as_bytes(),
        )
        .await
        .unwrap();
    let file = pwrite::new_file(
        &Uuid::nil().to_string(),
        "Rich",
        "2026-11-11T10:00:00.000Z",
        "de",
        serde_json::json!({"type": "doc", "content": [{"type": "paragraph", "content": [{"type": "text", "text": "Hallo"}]}]}),
    );
    documents
        .create("Rich", DocumentFormat::Pwrite, &pwrite::to_bytes(&file))
        .await
        .unwrap();
    let media = library.media();
    for name in ["skizze.svg", "plan.svg"] {
        let content = format!("{SVG}<!-- {name} -->");
        let preview = media.prepare_bytes(name, content.as_bytes()).await.unwrap();
        media
            .commit_import(
                Uuid::try_parse(&preview.batch_id).unwrap(),
                &BTreeMap::new(),
                Decision::Import,
                &SubnetPlacement::default(),
            )
            .await
            .unwrap();
    }
    library
        .structure()
        .create_subnet("Projekte", None, None, "")
        .await
        .unwrap();
    library
}

/// Every file under `documents/` and `media/` with its checksum.
fn element_files(root: &Path) -> BTreeMap<String, String> {
    let mut files = BTreeMap::new();
    for top in ["documents", "media"] {
        let dir = root.join(top);
        if !dir.exists() {
            continue;
        }
        for entry in fs::read_dir(&dir).unwrap() {
            let sub = entry.unwrap().path();
            for file in fs::read_dir(&sub).unwrap() {
                let file = file.unwrap().path();
                let relative = file
                    .strip_prefix(root)
                    .unwrap()
                    .to_string_lossy()
                    .replace('\\', "/");
                files.insert(relative, sha256_hex(&fs::read(&file).unwrap()));
            }
        }
    }
    files
}

fn zip_names(path: &Path) -> Vec<String> {
    let mut archive = zip::ZipArchive::new(fs::File::open(path).unwrap()).unwrap();
    (0..archive.len())
        .map(|index| {
            archive
                .by_index(index)
                .unwrap()
                .name()
                .unwrap()
                .into_owned()
        })
        .collect()
}

/// Every entry of a backup with its bytes.
fn zip_entries(path: &Path) -> Vec<(String, Vec<u8>)> {
    let mut archive = zip::ZipArchive::new(fs::File::open(path).unwrap()).unwrap();
    (0..archive.len())
        .map(|index| {
            let mut entry = archive.by_index(index).unwrap();
            let mut data = Vec::new();
            entry.read_to_end(&mut data).unwrap();
            (entry.name().unwrap().into_owned(), data)
        })
        .collect()
}

fn write_zip(path: &Path, entries: &[(String, Vec<u8>)]) {
    let mut writer = zip::ZipWriter::new(fs::File::create(path).unwrap());
    for (name, data) in entries {
        writer
            .start_file(name.as_str(), zip::write::SimpleFileOptions::default())
            .unwrap();
        writer.write_all(data).unwrap();
    }
    writer.finish().unwrap();
}

fn manifest_of(entries: &[(String, Vec<u8>)]) -> BackupManifest {
    serde_json::from_slice(
        &entries
            .iter()
            .find(|(name, _)| name == "manifest.json")
            .unwrap()
            .1,
    )
    .unwrap()
}

fn with_manifest(entries: &mut [Entry], manifest: &BackupManifest) {
    let json = serde_json::to_vec_pretty(manifest).unwrap();
    entries
        .iter_mut()
        .find(|(name, _)| name == "manifest.json")
        .unwrap()
        .1 = json;
}

#[tokio::test]
async fn a_backup_restores_with_every_checksum() {
    let dir = common::temp_dir("pw-backup-");
    let path = dir.path().join("lib");
    let library = filled(&path).await;
    let target = dir.path().join("sicherungen");
    fs::create_dir_all(&target).unwrap();

    let outcome = library
        .backups()
        .create(&target, not_cancelled(), |_| {})
        .await
        .unwrap();
    assert!(
        outcome
            .path
            .extension()
            .is_some_and(|ext| ext == "pwbackup")
    );
    assert_eq!(outcome.summary.documents, 3);
    assert_eq!(outcome.summary.media, 2);
    // Nothing of the work stays behind.
    assert!(fs::read_dir(path.join("tmp")).unwrap().next().is_none());
    assert_eq!(fs::read_dir(&target).unwrap().count(), 1);
    let before = element_files(&path);
    let rows_before: i64 = sqlx::query_scalar("SELECT count(*) FROM documents")
        .fetch_one(library.database().reader())
        .await
        .unwrap();
    library.close().await;

    let scratch = dir.path().join("scratch");
    fs::create_dir_all(&scratch).unwrap();
    let summary = backup::check_summary(&outcome.path, &scratch, no_progress())
        .await
        .unwrap();
    assert_eq!(summary, outcome.summary);

    let restored = dir.path().join("wiederhergestellt");
    backup::restore(&outcome.path, &restored, no_progress())
        .await
        .unwrap();
    // Every file is there with its checksum, and nothing else.
    assert_eq!(element_files(&restored), before);
    assert!(!restored.join("backups").exists());
    assert!(
        !restored.join("tmp").exists()
            || fs::read_dir(restored.join("tmp")).unwrap().next().is_none()
    );
    assert!(
        diagnostics::integrity_check_file(&restored.join("database/projectwrite.db"))
            .await
            .unwrap()
    );
    // It opens as a library, with the same rows and a working search.
    let library = open(&restored).await;
    let rows: i64 = sqlx::query_scalar("SELECT count(*) FROM documents")
        .fetch_one(library.database().reader())
        .await
        .unwrap();
    assert_eq!(rows, rows_before);
    let documents = library.documents().list().await.unwrap();
    for meta in &documents {
        let content = fs::read(restored.join(meta.format.relative_path(meta.id))).unwrap();
        assert_eq!(sha256_hex(&content), meta.content_sha256, "{}", meta.title);
    }
    let hits = library
        .search()
        .search(
            "Mueller",
            &project_write_lib::services::search::SearchOptions {
                limit: 10,
                ..Default::default()
            },
        )
        .await
        .unwrap();
    assert_eq!(hits.len(), 1);
    library.close().await;
}

#[tokio::test]
async fn a_backup_holds_no_earlier_backups_and_nothing_outside_the_list() {
    let dir = common::temp_dir("pw-backup-");
    let path = dir.path().join("lib");
    let library = filled(&path).await;
    let backups_dir = path.join("backups");
    fs::create_dir_all(&backups_dir).unwrap();
    // Things that must not get in.
    fs::create_dir_all(path.join("recovery")).unwrap();
    fs::write(path.join("recovery/entwurf.json"), b"{}").unwrap();
    fs::create_dir_all(path.join("exports")).unwrap();
    fs::write(path.join("exports/alt.md"), b"alt").unwrap();
    fs::write(path.join("documents/markdown/fremd.md"), b"ohne Zeile").unwrap();

    let first = library
        .backups()
        .create(&backups_dir, not_cancelled(), |_| {})
        .await
        .unwrap();
    tokio::time::sleep(Duration::from_millis(5)).await;
    let second = library
        .backups()
        .create(&backups_dir, not_cancelled(), |_| {})
        .await
        .unwrap();
    assert_ne!(first.path, second.path);
    let names = zip_names(&second.path);
    assert!(names.iter().any(|name| name == "database/projectwrite.db"));
    assert!(names.iter().any(|name| name == "library.json"));
    for name in &names {
        assert!(
            name == "manifest.json"
                || name == "library.json"
                || name == "database/projectwrite.db"
                || name.starts_with("documents/")
                || name.starts_with("media/"),
            "{name}"
        );
        assert!(!name.contains("pwbackup"), "{name}");
        assert!(!name.ends_with("fremd.md"), "{name}");
    }
    assert_eq!(names.len(), 1 + 2 + 3 + 2);

    // Inside the library only backups/ is a target.
    let refused = library
        .backups()
        .create(&path.join("documents"), not_cancelled(), |_| {})
        .await;
    assert!(
        matches!(refused, Err(BackupError::TargetInsideLibrary)),
        "{refused:?}"
    );

    // Keeping the newest one of this library.
    let library_id = library.backups().library_id();
    assert_eq!(backup::prune(&backups_dir, library_id, 1).unwrap(), 1);
    assert_eq!(
        backup::own_backups(&backups_dir, library_id).unwrap(),
        std::slice::from_ref(&second.path)
    );
    library.close().await;
}

#[tokio::test]
async fn a_changed_or_damaged_backup_is_refused() {
    let dir = common::temp_dir("pw-backup-");
    let path = dir.path().join("lib");
    let library = filled(&path).await;
    let target = dir.path().join("ziel");
    fs::create_dir_all(&target).unwrap();
    let good = library
        .backups()
        .create(&target, not_cancelled(), |_| {})
        .await
        .unwrap()
        .path;
    library.close().await;
    let scratch = dir.path().join("scratch");
    fs::create_dir_all(&scratch).unwrap();
    let entries = zip_entries(&good);
    let document = entries
        .iter()
        .position(|(name, _)| name.starts_with("documents/markdown/"))
        .unwrap();

    let mut cases: Vec<(&str, Vec<Entry>)> = Vec::new();
    // A document changed, manifest as it was.
    let mut changed = entries.clone();
    changed[document].1.extend_from_slice(b"angehaengt");
    cases.push(("changed document", changed));
    // The manifest names another checksum.
    let mut changed = entries.clone();
    let mut manifest = manifest_of(&changed);
    manifest.files[1].sha256 = "0".repeat(64);
    with_manifest(&mut changed, &manifest);
    cases.push(("changed checksum", changed));
    // A file the manifest does not name.
    let mut changed = entries.clone();
    changed.push((
        format!("documents/text/{}.txt", Uuid::new_v4()),
        b"x".to_vec(),
    ));
    cases.push(("extra file", changed));
    // A file of the manifest is missing.
    let mut changed = entries.clone();
    changed.remove(document);
    cases.push(("missing file", changed));
    // Names that leave the folder or are not on the list.
    for name in [
        "../boese.txt",
        "/etc/passwd",
        "documents/markdown/../../x.md",
        "backups/alt.pwbackup",
        "C:/x",
    ] {
        let mut changed = entries.clone();
        changed.push((name.to_owned(), b"x".to_vec()));
        cases.push(("bad name", changed));
    }
    // A newer format.
    let mut changed = entries.clone();
    let mut manifest = manifest_of(&changed);
    manifest.format_version = 99;
    with_manifest(&mut changed, &manifest);
    cases.push(("newer format", changed));
    // A damaged database whose checksum the manifest agrees with.
    let mut changed = entries.clone();
    let database = changed
        .iter()
        .position(|(name, _)| name == "database/projectwrite.db")
        .unwrap();
    let bytes = &mut changed[database].1;
    let middle = bytes.len() / 2;
    let end = middle + 4096.min(bytes.len() - middle);
    for byte in &mut bytes[middle..end] {
        *byte = 0x5a;
    }
    let damaged = changed[database].1.clone();
    let mut manifest = manifest_of(&changed);
    let file = manifest
        .files
        .iter_mut()
        .find(|file| file.path == "database/projectwrite.db")
        .unwrap();
    file.sha256 = sha256_hex(&damaged);
    with_manifest(&mut changed, &manifest);
    cases.push(("damaged database", changed));
    // A larger file than the manifest says (a ZIP bomb would look so).
    let mut changed = entries.clone();
    changed[document].1 = vec![b'a'; 10 * 1024 * 1024];
    cases.push(("larger than listed", changed));

    for (index, (what, entries)) in cases.into_iter().enumerate() {
        let file = dir.path().join(format!("case-{index}.pwbackup"));
        write_zip(&file, &entries);
        let checked = backup::check(&file, &scratch, None, no_progress()).await;
        assert!(checked.is_err(), "{what} was accepted");
        let restored = dir.path().join(format!("restore-{index}"));
        let result = backup::restore(&file, &restored, no_progress()).await;
        assert!(result.is_err(), "{what} was restored");
        assert!(!restored.exists(), "{what} left the target behind");
    }
    // A truncated archive.
    let data = fs::read(&good).unwrap();
    let truncated = dir.path().join("truncated.pwbackup");
    fs::write(&truncated, &data[..data.len() / 2]).unwrap();
    assert!(
        backup::check(&truncated, &scratch, None, no_progress())
            .await
            .is_err()
    );
    // Restoring leaves nothing but the target; scratch stays empty.
    assert!(fs::read_dir(&scratch).unwrap().next().is_none());
    let leftovers: Vec<_> = fs::read_dir(dir.path())
        .unwrap()
        .filter_map(Result::ok)
        .filter(|entry| entry.file_name().to_string_lossy().contains(".restoring-"))
        .collect();
    assert!(leftovers.is_empty(), "{leftovers:?}");
    // The good one still passes, and a target with content is refused.
    assert!(
        backup::check(&good, &scratch, None, no_progress())
            .await
            .is_ok()
    );
    let full = dir.path().join("voll");
    fs::create_dir_all(&full).unwrap();
    fs::write(full.join("x"), b"x").unwrap();
    assert!(matches!(
        backup::restore(&good, &full, no_progress()).await,
        Err(BackupError::TargetNotEmpty)
    ));
}

#[tokio::test]
async fn a_backup_while_documents_are_saved_is_consistent() {
    let dir = common::temp_dir("pw-backup-");
    let path = dir.path().join("lib");
    let library = filled(&path).await;
    let documents = library.documents();
    let mut ids = Vec::new();
    for index in 0..20 {
        let meta = documents
            .create(&format!("Laufend {index}"), DocumentFormat::Md, b"Start\n")
            .await
            .unwrap();
        ids.push(meta.id);
    }
    let target = dir.path().join("ziel");
    fs::create_dir_all(&target).unwrap();

    // Saving goes on while the backups run.
    let stop = Arc::new(AtomicBool::new(false));
    let writer = {
        let documents = library.documents();
        let stop = stop.clone();
        let ids = ids.clone();
        tokio::spawn(async move {
            let mut round = 0u64;
            while !stop.load(std::sync::atomic::Ordering::Relaxed) {
                for id in &ids {
                    let text = format!("Fassung {round} von {id}\n").repeat(50);
                    documents.save(*id, text.as_bytes()).await.unwrap();
                }
                round += 1;
            }
            round
        })
    };
    let mut paths = Vec::new();
    for _ in 0..5 {
        let outcome = library
            .backups()
            .create(&target, not_cancelled(), |_| {})
            .await
            .unwrap();
        paths.push(outcome.path);
        tokio::time::sleep(Duration::from_millis(3)).await;
    }
    stop.store(true, std::sync::atomic::Ordering::Relaxed);
    let rounds = writer.await.unwrap();
    assert!(rounds > 0);
    library.close().await;

    // In every backup each row of the database describes the file next to it.
    for (index, backup_path) in paths.iter().enumerate() {
        let restored = dir.path().join(format!("r{index}"));
        backup::restore(backup_path, &restored, no_progress())
            .await
            .unwrap();
        let library = open(&restored).await;
        for meta in library.documents().list().await.unwrap() {
            let content = fs::read(restored.join(meta.format.relative_path(meta.id))).unwrap();
            assert_eq!(
                sha256_hex(&content),
                meta.content_sha256,
                "backup {index}: {}",
                meta.title
            );
            assert_eq!(content.len() as u64, meta.size_bytes);
        }
        library.close().await;
    }
}

#[tokio::test]
async fn a_cancelled_backup_leaves_nothing() {
    let dir = common::temp_dir("pw-backup-");
    let path = dir.path().join("lib");
    let library = filled(&path).await;
    let target = dir.path().join("ziel");
    fs::create_dir_all(&target).unwrap();
    let cancel = Arc::new(AtomicBool::new(true));
    let result = library.backups().create(&target, cancel, |_| {}).await;
    assert!(matches!(result, Err(BackupError::Cancelled)), "{result:?}");
    assert!(fs::read_dir(&target).unwrap().next().is_none());
    assert!(fs::read_dir(path.join("tmp")).unwrap().next().is_none());
    library.close().await;
}

const LIBRARY_ENV: &str = "PW_TEST_BACKUP_LIBRARY";
const TARGET_ENV: &str = "PW_TEST_BACKUP_TARGET";
const KILLS: usize = 25;

/// Helper process: writes backups again and again until killed.
#[test]
#[ignore = "helper process of hard_kills_while_backing_up_leave_no_finished_looking_file"]
fn backup_process() {
    let (Some(path), Some(target)) = (std::env::var_os(LIBRARY_ENV), std::env::var_os(TARGET_ENV))
    else {
        return;
    };
    let runtime = tokio::runtime::Runtime::new().unwrap();
    runtime.block_on(async {
        let library = open(Path::new(&path)).await;
        println!("\nPW-BACKUP-READY");
        let mut round = 0u64;
        loop {
            println!("PW-BACKUP-START {round}");
            library
                .backups()
                .create(Path::new(&target), not_cancelled(), |_| {})
                .await
                .unwrap();
            println!("PW-BACKUP-DONE {round}");
            round += 1;
        }
    });
}

fn spawn_backup_process(path: &Path, target: &Path) -> std::process::Child {
    let mut child = Command::new(std::env::current_exe().unwrap())
        .args([
            "backup_process",
            "--exact",
            "--ignored",
            "--nocapture",
            "--test-threads=1",
        ])
        .env(LIBRARY_ENV, path)
        .env(TARGET_ENV, target)
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn helper process");
    let stdout = child.stdout.take().unwrap();
    let (ready_tx, ready_rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        for line in BufReader::new(stdout).lines() {
            let Ok(line) = line else { break };
            if line.contains("PW-BACKUP-READY") {
                let _ = ready_tx.send(());
            }
        }
    });
    ready_rx
        .recv_timeout(Duration::from_secs(60))
        .expect("helper process became ready");
    child
}

#[test]
fn hard_kills_while_backing_up_leave_no_finished_looking_file() {
    let dir = common::temp_dir("pw-backup-kill-");
    let path = dir.path().join("lib");
    let target = dir.path().join("ziel");
    fs::create_dir_all(&target).unwrap();
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let library_id = runtime.block_on(async {
        let library = filled(&path).await;
        // Enough content that a backup takes a while.
        let mut rng = SplitMix64(0x5EED_2026_1111);
        for index in 0..60 {
            let words: String = (0..4000)
                .map(|_| format!("w{} ", rng.between(0..100_000)))
                .collect();
            library
                .documents()
                .create(
                    &format!("Text {index}"),
                    DocumentFormat::Md,
                    words.as_bytes(),
                )
                .await
                .unwrap();
        }
        let id = library.backups().library_id();
        library.close().await;
        id
    });
    let scratch = dir.path().join("scratch");
    fs::create_dir_all(&scratch).unwrap();
    let mut rng = SplitMix64(0x5EED_2026_1112);
    let mut partial_seen = 0;
    for kill in 0..KILLS {
        let mut child = spawn_backup_process(&path, &target);
        std::thread::sleep(Duration::from_millis(rng.between(20..400)));
        child.kill().unwrap();
        child.wait().unwrap();
        // Every file that looks finished is a whole, checked backup.
        runtime.block_on(async {
            for finished in backup::own_backups(&target, library_id).unwrap() {
                backup::check(&finished, &scratch, None, no_progress())
                    .await
                    .unwrap_or_else(|error| panic!("kill {kill}: {} {error}", finished.display()));
            }
        });
        for entry in fs::read_dir(&target).unwrap() {
            let name = entry.unwrap().file_name().to_string_lossy().into_owned();
            assert!(
                name.ends_with(".pwbackup") || name.ends_with(".pwbackup.part"),
                "kill {kill}: {name}"
            );
            if name.ends_with(".part") {
                partial_seen += 1;
            }
        }
        // The next start clears what was left.
        assert!(backup::remove_partial(&target, library_id).is_ok());
        backup::prune(&target, library_id, 3).unwrap();
    }
    println!("{KILLS} kills, {partial_seen} times a partial file was left");
    // The library itself is whole and opens.
    runtime.block_on(async {
        let library = open(&path).await;
        assert!(
            diagnostics::integrity_check(library.database().reader())
                .await
                .unwrap()
        );
        library.close().await;
    });
}
