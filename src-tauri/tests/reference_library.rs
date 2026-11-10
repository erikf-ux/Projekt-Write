//! Acceptance test of Phase 2 (section 25): "Die Referenzbibliothek entsteht
//! reproduzierbar." A small library (the full size runs in CI through
//! `node scripts/reference-library.mjs --check`). Phases 7 and 8 add the
//! structure and the media.

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
    // Phase 7: subnets and tags in proportion, most documents assigned.
    let (subnets, tags) = generate::structure_size(DOCUMENTS);
    assert_eq!((summary.subnets, summary.tags), (subnets, tags));
    assert!(summary.unassigned > 0 && summary.unassigned < DOCUMENTS / 3);
    assert!(summary.memberships >= DOCUMENTS - summary.unassigned);
    assert_eq!(
        generate::structure_size(generate::FULL_DOCUMENTS),
        (500, 1_000)
    );
    // Phase 8: media of all three kinds, imported, used and placed.
    assert_eq!(generate::media_size(generate::FULL_DOCUMENTS), 5_000);
    assert_eq!(summary.media, generate::media_size(DOCUMENTS));
    assert_eq!(
        summary.images + summary.pdfs + summary.attachments,
        summary.media
    );
    assert!(summary.images > 0 && summary.pdfs > 0 && summary.attachments > 0);
    assert!(summary.media_uses > 0);
    assert!(summary.media_memberships >= summary.media - summary.media_unassigned);

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
    let scan = project_write_lib::services::media::scan_media(library.root(), library.database())
        .await
        .unwrap();
    assert!(scan.orphaned_files.is_empty(), "{:?}", scan.orphaned_files);
    assert!(scan.missing_files.is_empty(), "{:?}", scan.missing_files);
    let media = library.media().list().await.unwrap();
    assert_eq!(media.len(), summary.media);
    assert!(
        media
            .iter()
            .filter(|medium| medium.kind == project_write_lib::services::media::MediaKind::Pdf)
            .all(|pdf| pdf
                .page_count
                .is_some_and(|pages| (1..=40).contains(&pages)))
    );
    // Pictures in all five formats (ADR 0019, decision 16).
    let formats: Vec<String> = sqlx::query_scalar(
        "SELECT DISTINCT extension FROM media WHERE kind = 'image' ORDER BY extension",
    )
    .fetch_all(library.database().reader())
    .await
    .unwrap();
    assert_eq!(formats, ["gif", "jpg", "png", "svg", "webp"]);
    let uses: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM document_media")
        .fetch_one(library.database().reader())
        .await
        .unwrap();
    assert_eq!(uses, summary.media_uses as i64);
    let documents = library.documents();
    assert_eq!(documents.count().await.unwrap(), DOCUMENTS as u64);
    let structure = library.structure();
    let rows = structure.subnets().await.unwrap();
    assert_eq!(rows.len(), subnets);
    // At most four levels, no cycles.
    for row in &rows {
        let mut depth = 1;
        let mut parent = row.parent_id.clone();
        while let Some(id) = parent {
            depth += 1;
            parent = rows.iter().find(|r| r.id == id).unwrap().parent_id.clone();
        }
        assert!(depth <= 4, "{} is on level {depth}", row.name);
    }

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
