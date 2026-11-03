//! Acceptance test of Phase 2 (section 25): "Die Referenzbibliothek entsteht
//! reproduzierbar." A small library (the full size runs in CI through
//! `node scripts/reference-library.mjs --check`).

mod common;

#[path = "../examples/reference_library/generate.rs"]
#[allow(dead_code)]
mod generate;

use project_write_lib::clock::Generators;
use project_write_lib::database::diagnostics;
use project_write_lib::services::library::{Library, OpenOptions};

const DOCUMENTS: usize = 150;

#[tokio::test]
async fn the_same_seed_builds_the_same_library() {
    let dir = common::temp_dir("pw-reference-");
    let first = dir.path().join("a");
    let second = dir.path().join("b");
    let other_seed = dir.path().join("c");

    let summary = generate::generate(&first, 7, DOCUMENTS, |_| {})
        .await
        .unwrap();
    generate::generate(&second, 7, DOCUMENTS, |_| {})
        .await
        .unwrap();
    generate::generate(&other_seed, 8, DOCUMENTS, |_| {})
        .await
        .unwrap();

    let a = generate::fingerprint(&first).await.unwrap();
    assert_eq!(a, generate::fingerprint(&second).await.unwrap());
    assert_ne!(a, generate::fingerprint(&other_seed).await.unwrap());
    assert_eq!(summary.documents, DOCUMENTS);
    assert_eq!(summary.markdown + summary.text, DOCUMENTS);

    // It is an ordinary, intact library.
    let (library, report) = Library::open(&first, OpenOptions::default(), Generators::system())
        .await
        .unwrap();
    assert!(report.documents.orphaned_files.is_empty());
    assert!(report.documents.missing_files.is_empty());
    assert!(
        diagnostics::integrity_check(library.database().reader())
            .await
            .unwrap()
    );
    let documents = library.documents();
    assert_eq!(documents.count().await.unwrap(), DOCUMENTS as u64);

    // One document has the 50,000 words of the performance targets.
    let longest = documents
        .list()
        .await
        .unwrap()
        .into_iter()
        .max_by_key(|meta| meta.size_bytes)
        .unwrap();
    let (_, content) = documents.read(longest.id).await.unwrap();
    let words = String::from_utf8(content)
        .unwrap()
        .split_whitespace()
        .count();
    assert!(words >= generate::LONG_DOCUMENT_WORDS, "{words} words");
    library.close().await;
}

#[test]
fn ids_and_times_come_from_the_seed() {
    let a = generate::generators(1);
    let b = generate::generators(1);
    let c = generate::generators(2);
    let ids_a: Vec<_> = (0..5).map(|_| a.new_id()).collect();
    let ids_b: Vec<_> = (0..5).map(|_| b.new_id()).collect();
    assert_eq!(ids_a, ids_b);
    assert_ne!(ids_a[0], c.new_id());
    assert!(ids_a.iter().all(|id| id.get_version_num() == 4));
    let times: Vec<_> = (0..5).map(|_| a.now()).collect();
    assert!(times.windows(2).all(|w| w[0] < w[1]));
    assert_eq!(times, (0..5).map(|_| b.now()).collect::<Vec<_>>());
}
