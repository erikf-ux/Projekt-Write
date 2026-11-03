//! Acceptance test of Phase 2 (sections 6 and 25): "100 harte
//! Prozessabbrüche während des Speicherns: keine beschädigte Datei,
//! `integrity_check` meldet `ok`."
//!
//! A helper process (this test binary, started again) opens a library and
//! saves documents without pause: existing documents with new content of
//! 1 KB to 512 KB (prepared before the loop), and every tenth operation a
//! new document. The test kills
//! it hard (`SIGKILL` on Unix, `TerminateProcess` on Windows) at a random
//! moment, 100 times. After every kill:
//!
//! * every document file is one complete version (header with length and
//!   SHA-256 of the payload),
//! * the library opens, `PRAGMA integrity_check` reports `ok`,
//! * no document lost its file; files of interrupted creations are reported,
//!   not deleted, and are complete as well,
//! * every document reads back completely and its row matches the file.

mod common;

use std::fs;
use std::io::{BufRead, BufReader};
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex, mpsc};
use std::time::{Duration, Instant};

use common::SplitMix64;
use project_write_lib::clock::Generators;
use project_write_lib::database::diagnostics;
use project_write_lib::filesystem::atomic::is_temp_file_name;
use project_write_lib::services::documents::{DocumentFormat, sha256_hex};
use project_write_lib::services::library::{Library, LibraryError, OpenOptions};

const LIBRARY_ENV: &str = "PW_TEST_CRASH_LIBRARY";
const SEED_ENV: &str = "PW_TEST_CRASH_SEED";
const KILLS: usize = 100;
const INITIAL_DOCUMENTS: usize = 4;

/// `PWCRASH v=<version> n=<length> sha=<hex>\n` followed by the payload.
fn content(version: u64, length: usize, rng: &mut SplitMix64) -> Vec<u8> {
    const ALPHABET: &[u8; 32] = b"abcdefghijklmnopqrstuvwxyz \n.,;-";
    let mut payload = Vec::with_capacity(length + 8);
    while payload.len() < length {
        // Eight characters per random number.
        let bits = rng.next();
        payload.extend((0..8).map(|i| ALPHABET[((bits >> (i * 8)) & 31) as usize]));
    }
    payload.truncate(length);
    let mut data = format!(
        "PWCRASH v={version} n={length} sha={}\n",
        sha256_hex(&payload)
    )
    .into_bytes();
    data.extend_from_slice(&payload);
    data
}

/// Checks that `data` is one complete version; returns the version.
fn verify_content(data: &[u8]) -> Result<u64, String> {
    let newline = data
        .iter()
        .position(|&b| b == b'\n')
        .ok_or("no header line")?;
    let header = std::str::from_utf8(&data[..newline]).map_err(|_| "header not UTF-8")?;
    let payload = &data[newline + 1..];
    let mut version = None;
    let mut length = None;
    let mut sha = None;
    for (index, part) in header.split(' ').enumerate() {
        match (index, part.split_once('=')) {
            (0, None) if part == "PWCRASH" => {}
            (_, Some(("v", v))) => version = v.parse::<u64>().ok(),
            (_, Some(("n", n))) => length = n.parse::<usize>().ok(),
            (_, Some(("sha", s))) => sha = Some(s.to_owned()),
            _ => return Err(format!("bad header {header:?}")),
        }
    }
    let (Some(version), Some(length), Some(sha)) = (version, length, sha) else {
        return Err(format!("incomplete header {header:?}"));
    };
    if payload.len() != length {
        return Err(format!(
            "version {version}: payload has {} of {length} bytes",
            payload.len()
        ));
    }
    if sha256_hex(payload) != sha {
        return Err(format!("version {version}: checksum mismatch"));
    }
    Ok(version)
}

async fn open_eventually(
    path: &Path,
) -> (Library, project_write_lib::services::library::OpenReport) {
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        match Library::open(path, OpenOptions::default(), Generators::system()).await {
            Ok(opened) => return opened,
            // The OS releases the lock of a killed process shortly after.
            Err(LibraryError::Locked) if Instant::now() < deadline => {
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
            Err(error) => panic!("open failed: {error}"),
        }
    }
}

/// Helper process: saves documents until it is killed.
#[test]
#[ignore = "helper process of hard_kills_while_saving_never_damage_the_library"]
fn crash_writer_process() {
    let Some(path) = std::env::var_os(LIBRARY_ENV) else {
        return;
    };
    let seed: u64 = std::env::var(SEED_ENV).unwrap().parse().unwrap();
    let runtime = tokio::runtime::Runtime::new().unwrap();
    runtime.block_on(async {
        let (library, _) = open_eventually(Path::new(&path)).await;
        let docs = library.documents();
        let ids: Vec<_> = docs
            .list()
            .await
            .unwrap()
            .into_iter()
            .map(|m| m.id)
            .collect();
        let mut rng = SplitMix64(seed);
        // Prepared up front, so the loop does nothing but save and a kill
        // almost always lands inside a save.
        let pool: Vec<Vec<u8>> = (0..16u64)
            .map(|i| {
                let length = rng.between(1024..512 * 1024) as usize;
                content((seed + 1) * 1_000 + i, length, &mut rng)
            })
            .collect();
        println!("\nPW-CRASH-READY");
        for k in 0u64.. {
            let version = (seed + 1) * 1_000_000 + k;
            let data = &pool[(k % 16) as usize];
            if k % 10 == 9 {
                println!("PW-CRASH-SAVING create {k}");
                docs.create(&format!("Neu {version}"), DocumentFormat::Txt, data)
                    .await
                    .unwrap();
            } else {
                println!("PW-CRASH-SAVING save {k}");
                docs.save(ids[(k as usize) % ids.len()], data)
                    .await
                    .unwrap();
            }
            println!("PW-CRASH-SAVED {k}");
        }
    });
}

/// Starts the helper and waits until it saves. Returns the child and a
/// handle that yields the last line the helper printed.
fn spawn_writer(path: &Path, seed: u64) -> (std::process::Child, std::thread::JoinHandle<String>) {
    let mut child = Command::new(std::env::current_exe().unwrap())
        .args([
            "crash_writer_process",
            "--exact",
            "--ignored",
            "--nocapture",
            "--test-threads=1",
        ])
        .env(LIBRARY_ENV, path)
        .env(SEED_ENV, seed.to_string())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn helper process");
    let stdout = child.stdout.take().unwrap();
    let (ready_tx, ready_rx) = mpsc::channel();
    let last = Arc::new(Mutex::new(String::new()));
    let reader = {
        let last = last.clone();
        std::thread::spawn(move || {
            for line in BufReader::new(stdout).lines() {
                let Ok(line) = line else { break };
                if line.contains("PW-CRASH-READY") {
                    let _ = ready_tx.send(());
                }
                if line.starts_with("PW-CRASH-") {
                    *last.lock().unwrap() = line;
                }
            }
            last.lock().unwrap().clone()
        })
    };
    ready_rx
        .recv_timeout(Duration::from_secs(60))
        .expect("helper process became ready");
    (child, reader)
}

/// Every non-temporary file in the document folders is complete.
fn verify_files_on_disk(path: &Path) -> usize {
    let mut files = 0;
    for format in DocumentFormat::ALL {
        let dir = path.join(format.directory());
        let Ok(entries) = fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries {
            let entry = entry.unwrap();
            let name = entry.file_name().to_string_lossy().into_owned();
            if is_temp_file_name(&name) {
                continue;
            }
            let data = fs::read(entry.path()).unwrap();
            if let Err(error) = verify_content(&data) {
                panic!("{}: damaged file: {error}", entry.path().display());
            }
            files += 1;
        }
    }
    files
}

#[test]
fn hard_kills_while_saving_never_damage_the_library() {
    let dir = common::temp_dir("pw-crash-");
    let path = dir.path().join("lib");
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let mut rng = SplitMix64(0x5EED_2026_1103);

    runtime.block_on(async {
        let (library, _) = Library::create(&path, Generators::system()).await.unwrap();
        for i in 0..INITIAL_DOCUMENTS {
            let data = content(i as u64, 2048, &mut rng);
            library
                .documents()
                .create(&format!("Dokument {i}"), DocumentFormat::Txt, &data)
                .await
                .unwrap();
        }
        library.close().await;
    });

    let started = Instant::now();
    let mut killed_while_saving = 0;
    let mut orphans_seen = 0;
    let mut temp_files_removed = 0;
    for iteration in 0..KILLS {
        let (mut child, reader) = spawn_writer(&path, iteration as u64);
        std::thread::sleep(Duration::from_millis(rng.between(5..150)));
        child.kill().expect("kill helper process");
        child.wait().unwrap();
        let last = reader.join().unwrap();
        if last.starts_with("PW-CRASH-SAVING") {
            killed_while_saving += 1;
        }

        let files = verify_files_on_disk(&path);
        assert!(
            files >= INITIAL_DOCUMENTS,
            "iteration {iteration}: {files} files"
        );

        runtime.block_on(async {
            let (library, report) = open_eventually(&path).await;
            assert!(
                diagnostics::integrity_check(library.database().reader())
                    .await
                    .unwrap(),
                "iteration {iteration}: integrity_check failed"
            );
            assert!(
                report.documents.missing_files.is_empty(),
                "iteration {iteration}: documents without file: {:?}",
                report.documents.missing_files
            );
            temp_files_removed += report.documents.removed_temp_files;
            orphans_seen = report.documents.orphaned_files.len();
            for orphan in &report.documents.orphaned_files {
                // A creation interrupted after the file and before the row.
                let data = fs::read(path.join(orphan)).unwrap();
                verify_content(&data).unwrap_or_else(|e| panic!("orphan {orphan}: {e}"));
            }
            let docs = library.documents();
            let all = docs.list().await.unwrap();
            assert!(all.len() >= INITIAL_DOCUMENTS);
            for meta in all {
                let (corrected, data) = docs.read(meta.id).await.unwrap();
                verify_content(&data)
                    .unwrap_or_else(|e| panic!("iteration {iteration}, {}: {e}", meta.id));
                assert_eq!(corrected.content_sha256, sha256_hex(&data));
                assert_eq!(corrected.size_bytes, data.len() as u64);
            }
            library.close().await;
        });
        // Opening removed every temporary file of the interrupted save.
        for format in DocumentFormat::ALL {
            if let Ok(entries) = fs::read_dir(path.join(format.directory())) {
                for entry in entries {
                    let name = entry.unwrap().file_name().to_string_lossy().into_owned();
                    assert!(!is_temp_file_name(&name), "left over: {name}");
                }
            }
        }
    }

    eprintln!(
        "{KILLS} hard kills in {:.1} s: {killed_while_saving} while a save was running, \
         {temp_files_removed} temporary files removed on open, {orphans_seen} files of \
         interrupted creations reported",
        started.elapsed().as_secs_f64()
    );
    // The helper saves without pause, so nearly every kill lands in a save.
    assert!(
        killed_while_saving >= KILLS * 3 / 4,
        "only {killed_while_saving} of {KILLS} kills hit a running save"
    );
}
