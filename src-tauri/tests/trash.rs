//! Papierkorb and Archiv (stage 3, ADR 0016): a deleted document comes back
//! with its links, versions and favourite; the trash keeps documents for the
//! retention period, tested with simulated time.

mod common;

use std::fs;
use std::sync::Arc;

use project_write_lib::clock::{Generators, ManualClock, RandomIds, Timestamp};
use project_write_lib::services::documents::{DocumentError, DocumentFormat};
use project_write_lib::services::library::Library;
use project_write_lib::services::settings::library::TrashRetentionDays;

const DAY: i64 = 24 * 60 * 60 * 1000;

struct Fixture {
    _dir: tempfile::TempDir,
    clock: Arc<ManualClock>,
    library: Library,
}

async fn fixture() -> Fixture {
    let dir = common::temp_dir("pw-trash-");
    let clock = Arc::new(ManualClock::new(
        Timestamp::parse_iso("2026-11-05T09:00:00.000Z").unwrap(),
    ));
    let generators = Generators::new(clock.clone(), Arc::new(RandomIds));
    let (library, _) = Library::create(&dir.path().join("lib"), generators)
        .await
        .unwrap();
    Fixture {
        _dir: dir,
        clock,
        library,
    }
}

#[tokio::test]
async fn a_deleted_document_returns_with_its_links_and_favourite() {
    let f = fixture().await;
    let docs = f.library.documents();
    let target = docs
        .create("Ziel", DocumentFormat::Md, b"Ziel\n")
        .await
        .unwrap();
    let text = format!("Siehe [Ziel](pwrite://{}).\n", target.id);
    let source = docs
        .create("Quelle", DocumentFormat::Md, text.as_bytes())
        .await
        .unwrap();
    docs.set_favorite(source.id, true).await.unwrap();
    assert_eq!(docs.backlinks(target.id).await.unwrap().len(), 1);

    let trashed = docs.move_to_trash(source.id).await.unwrap();
    assert!(trashed.deleted_at.is_some());
    let active: Vec<_> = docs.list().await.unwrap().iter().map(|d| d.id).collect();
    assert!(!active.contains(&source.id));
    assert_eq!(docs.list_trash().await.unwrap().len(), 1);
    // While in the trash it links nowhere visible, but nothing is lost.
    assert!(docs.backlinks(target.id).await.unwrap().is_empty());
    assert_eq!(docs.links_of(source.id).await.unwrap(), [target.id]);

    let restored = docs.restore_from_trash(source.id).await.unwrap();
    assert_eq!(restored.deleted_at, None);
    assert!(restored.favorited_at.is_some());
    assert_eq!(restored.content_sha256, source.content_sha256);
    let back = docs.backlinks(target.id).await.unwrap();
    assert_eq!(back.len(), 1);
    assert_eq!(back[0].id, source.id);
    assert!(docs.list_trash().await.unwrap().is_empty());
    assert!(matches!(
        docs.restore_from_trash(source.id).await,
        Err(DocumentError::NotFound)
    ));
    f.library.close().await;
}

#[tokio::test]
async fn archiving_hides_nothing_in_the_data_and_is_undone() {
    let f = fixture().await;
    let docs = f.library.documents();
    let meta = docs
        .create("Alt", DocumentFormat::Txt, b"alt")
        .await
        .unwrap();
    let archived = docs.set_archived(meta.id, true).await.unwrap();
    assert_eq!(
        archived.archived_at.as_deref(),
        Some("2026-11-05T09:00:00.000Z")
    );
    assert_eq!(archived.updated_at, meta.updated_at);
    // An archived document can go to the trash and comes back archived.
    docs.move_to_trash(meta.id).await.unwrap();
    assert!(matches!(
        docs.set_archived(meta.id, false).await,
        Err(DocumentError::NotFound)
    ));
    let restored = docs.restore_from_trash(meta.id).await.unwrap();
    assert!(restored.archived_at.is_some());
    let unarchived = docs.set_archived(meta.id, false).await.unwrap();
    assert_eq!(unarchived.archived_at, None);
    f.library.close().await;
}

#[tokio::test]
async fn the_trash_keeps_documents_for_the_retention_period() {
    let f = fixture().await;
    let docs = f.library.documents();
    let first = docs
        .create("Erst", DocumentFormat::Md, b"a\n")
        .await
        .unwrap();
    let second = docs
        .create("Zweit", DocumentFormat::Md, b"b\n")
        .await
        .unwrap();
    let kept = docs
        .create("Bleibt", DocumentFormat::Txt, b"c")
        .await
        .unwrap();
    let first_file = f.library.path().join(first.format.relative_path(first.id));

    docs.move_to_trash(first.id).await.unwrap();
    f.clock.advance(29 * DAY);
    docs.move_to_trash(second.id).await.unwrap();
    assert_eq!(docs.purge_expired(30).await.unwrap(), 0);

    // 30 days and a moment after the first deletion: only that one goes.
    f.clock.advance(DAY + 1);
    assert_eq!(docs.purge_expired(30).await.unwrap(), 1);
    assert!(docs.get(first.id).await.unwrap().is_none());
    assert!(!first_file.exists());
    assert!(docs.get(second.id).await.unwrap().is_some());

    // The setting decides; the work after opening applies it.
    f.library
        .settings()
        .set::<TrashRetentionDays>(&1)
        .await
        .unwrap();
    f.clock.advance(DAY);
    f.library.housekeeping().await;
    assert!(docs.get(second.id).await.unwrap().is_none());
    // Documents outside the trash are never removed.
    assert!(docs.get(kept.id).await.unwrap().is_some());
    assert!(matches!(
        docs.purge(kept.id).await,
        Err(DocumentError::NotInTrash)
    ));
    assert!(
        f.library
            .path()
            .join(kept.format.relative_path(kept.id))
            .exists()
    );
    f.library.close().await;
}

#[tokio::test]
async fn removing_for_good_takes_the_links_with_it() {
    let f = fixture().await;
    let docs = f.library.documents();
    let target = docs
        .create("Ziel", DocumentFormat::Md, b"Ziel\n")
        .await
        .unwrap();
    let text = format!("[Ziel](pwrite://{})\n", target.id);
    let source = docs
        .create("Quelle", DocumentFormat::Md, text.as_bytes())
        .await
        .unwrap();
    let other = docs
        .create("Andere", DocumentFormat::Txt, b"x")
        .await
        .unwrap();
    docs.move_to_trash(source.id).await.unwrap();
    docs.move_to_trash(other.id).await.unwrap();
    assert_eq!(docs.empty_trash().await.unwrap(), 2);
    assert!(docs.list_trash().await.unwrap().is_empty());
    assert!(docs.links_of(source.id).await.unwrap().is_empty());
    assert!(docs.backlinks(target.id).await.unwrap().is_empty());
    for meta in [&source, &other] {
        assert!(
            !f.library
                .path()
                .join(meta.format.relative_path(meta.id))
                .exists()
        );
    }
    // The target is untouched.
    assert_eq!(
        fs::read(
            f.library
                .path()
                .join(target.format.relative_path(target.id))
        )
        .unwrap(),
        b"Ziel\n"
    );
    f.library.close().await;
}
