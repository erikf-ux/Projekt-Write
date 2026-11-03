//! Acceptance test of Phase 2 (section 25): "Eine zweite Instanz kann
//! dieselbe Bibliothek nicht öffnen." – with a real second process.
//!
//! Kept in its own test binary: on Unix a process that is being started
//! shares the open files of its parent until it executes its program, which
//! would let it briefly hold the lock of an unrelated test running in
//! parallel.

mod common;

use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use project_write_lib::clock::Generators;
use project_write_lib::services::library::{Library, LibraryError, OpenOptions};

const CHILD_ENV: &str = "PW_TEST_LOCK_HOLDER_LIBRARY";

async fn open(path: &Path) -> Result<Library, LibraryError> {
    Library::open(path, OpenOptions::default(), Generators::system())
        .await
        .map(|(library, _)| library)
}

/// Helper process: opens the library given in [`CHILD_ENV`], prints
/// `PW-LOCK-HELD`, then waits for a line on stdin and closes it.
#[test]
#[ignore = "helper process of a_second_process_cannot_open_the_library"]
fn lock_holder_process() {
    let Some(path) = std::env::var_os(CHILD_ENV) else {
        return;
    };
    let runtime = tokio::runtime::Runtime::new().unwrap();
    runtime.block_on(async {
        let library = open(Path::new(&path)).await.expect("child opens library");
        println!("\nPW-LOCK-HELD");
        std::io::stdout().flush().unwrap();
        let mut line = String::new();
        std::io::stdin().read_line(&mut line).unwrap();
        library.close().await;
    });
}

fn spawn_lock_holder(path: &Path) -> Child {
    let mut child = Command::new(std::env::current_exe().unwrap())
        .args([
            "lock_holder_process",
            "--exact",
            "--ignored",
            "--nocapture",
            "--test-threads=1",
        ])
        .env(CHILD_ENV, path)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .expect("spawn helper process");
    let stdout = child.stdout.take().unwrap();
    let (locked_tx, locked_rx) = std::sync::mpsc::channel();
    // Keep reading until the helper ends, so its later output never hits a
    // closed pipe.
    std::thread::spawn(move || {
        for line in BufReader::new(stdout).lines() {
            let Ok(line) = line else { break };
            if line.contains("PW-LOCK-HELD") {
                let _ = locked_tx.send(());
            }
        }
    });
    locked_rx
        .recv_timeout(Duration::from_secs(60))
        .expect("helper process locked the library");
    child
}

/// The OS may need a moment to release the lock of a killed process.
async fn open_eventually(path: &Path) -> Library {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        match open(path).await {
            Ok(library) => return library,
            Err(LibraryError::Locked) if Instant::now() < deadline => {
                tokio::time::sleep(Duration::from_millis(50)).await;
            }
            Err(error) => panic!("open failed: {error}"),
        }
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn a_second_process_cannot_open_the_library() {
    let dir = common::temp_dir("pw-lib-");
    let path = dir.path().join("lib");
    Library::create(&path, Generators::system())
        .await
        .unwrap()
        .0
        .close()
        .await;

    // Orderly end of the other process.
    let mut child = spawn_lock_holder(&path);
    assert!(matches!(open(&path).await, Err(LibraryError::Locked)));
    child.stdin.as_mut().unwrap().write_all(b"\n").unwrap();
    assert!(child.wait().unwrap().success());
    open_eventually(&path).await.close().await;

    // Hard kill: the operating system releases the lock, no stale lock.
    let mut child = spawn_lock_holder(&path);
    assert!(matches!(open(&path).await, Err(LibraryError::Locked)));
    child.kill().unwrap();
    child.wait().unwrap();
    open_eventually(&path).await.close().await;
}
