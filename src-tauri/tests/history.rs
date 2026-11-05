//! Document history (stage 4, ADR 0016): versions while editing, on close,
//! before converting and restoring, named versions; restoring saves the
//! current state first; retention with simulated time.

mod common;

use std::sync::Arc;

use project_write_lib::clock::{Generators, ManualClock, RandomIds, Timestamp};
use project_write_lib::services::documents::DocumentFormat;
use project_write_lib::services::history::VersionKind;
use project_write_lib::services::library::Library;
use project_write_lib::services::settings::library::HistoryKeepAllHours;

const MINUTE: i64 = 60 * 1000;
const DAY: i64 = 24 * 60 * MINUTE;

struct Fixture {
    _dir: tempfile::TempDir,
    clock: Arc<ManualClock>,
    library: Library,
}

async fn fixture() -> Fixture {
    let dir = common::temp_dir("pw-history-");
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

fn kinds(versions: &[project_write_lib::services::history::VersionSummary]) -> Vec<VersionKind> {
    versions.iter().map(|version| version.kind).collect()
}

#[tokio::test]
async fn saving_keeps_one_version_per_ten_minutes_of_editing() {
    let f = fixture().await;
    let docs = f.library.documents();
    let history = docs.history();
    let meta = docs.create("Text", DocumentFormat::Md, b"").await.unwrap();
    docs.save(meta.id, b"Eins\n").await.unwrap();
    assert_eq!(
        kinds(&history.list(meta.id).await.unwrap()),
        [VersionKind::Auto]
    );
    // Further saves within 10 minutes: no new version.
    for text in ["Eins zwei\n", "Eins zwei drei\n", "Vier\n"] {
        f.clock.advance(3 * MINUTE);
        docs.save(meta.id, text.as_bytes()).await.unwrap();
    }
    assert_eq!(history.list(meta.id).await.unwrap().len(), 1);
    f.clock.advance(MINUTE);
    docs.save(meta.id, b"Fuenf\n").await.unwrap();
    let versions = history.list(meta.id).await.unwrap();
    assert_eq!(versions.len(), 2);
    let (newest, content) = history
        .content(uuid::Uuid::parse_str(&versions[0].id).unwrap())
        .await
        .unwrap();
    assert_eq!(content, b"Fuenf\n");
    assert_eq!(newest.created_at, "2026-11-05T09:10:00.000Z");
    // Unchanged content is not saved again, so no version either.
    f.clock.advance(20 * MINUTE);
    docs.save(meta.id, b"Fuenf\n").await.unwrap();
    assert_eq!(history.list(meta.id).await.unwrap().len(), 2);
    f.library.close().await;
}

#[tokio::test]
async fn named_close_and_conversion_versions() {
    let f = fixture().await;
    let docs = f.library.documents();
    let history = docs.history();
    let meta = docs
        .create("Kapitel", DocumentFormat::Md, b"# Kapitel\n")
        .await
        .unwrap();
    // Same content as the newest version: closing adds nothing new.
    docs.save(meta.id, b"# Kapitel\n\nText\n").await.unwrap();
    let close = docs
        .record_version(meta.id, VersionKind::Close, None)
        .await
        .unwrap();
    assert_eq!(close.kind, VersionKind::Auto);
    assert_eq!(history.list(meta.id).await.unwrap().len(), 1);
    // A name goes to that version instead of a copy.
    let named = docs
        .record_version(meta.id, VersionKind::Named, Some(" Erste Fassung "))
        .await
        .unwrap();
    assert_eq!(named.kind, VersionKind::Named);
    assert_eq!(named.name.as_deref(), Some("Erste Fassung"));
    assert_eq!(history.list(meta.id).await.unwrap().len(), 1);

    f.clock.advance(MINUTE);
    docs.save(meta.id, b"# Kapitel\n\nMehr Text\n")
        .await
        .unwrap();
    let closed = docs
        .record_version(meta.id, VersionKind::Close, None)
        .await
        .unwrap();
    assert_eq!(closed.kind, VersionKind::Close);

    // Converting keeps the old content in the old format first.
    docs.convert(
        meta.id,
        DocumentFormat::Txt,
        b"Kapitel\n\nMehr Text",
        "de-DE",
    )
    .await
    .unwrap();
    let versions = history.list(meta.id).await.unwrap();
    // The Markdown content equals the close version: no copy is stored.
    assert_eq!(kinds(&versions), [VersionKind::Close, VersionKind::Named]);
    assert_eq!(versions[0].format, DocumentFormat::Md);
    f.clock.advance(MINUTE);
    docs.save(meta.id, b"Nur noch Text").await.unwrap();
    docs.convert(meta.id, DocumentFormat::Md, b"Nur noch Text\n", "de-DE")
        .await
        .unwrap();
    let versions = history.list(meta.id).await.unwrap();
    assert_eq!(versions[0].kind, VersionKind::BeforeConversion);
    assert_eq!(versions[0].format, DocumentFormat::Txt);
    f.library.close().await;
}

#[tokio::test]
async fn restoring_a_version_saves_the_current_state_first() {
    let f = fixture().await;
    let docs = f.library.documents();
    let history = docs.history();
    let meta = docs.create("Brief", DocumentFormat::Md, b"").await.unwrap();
    docs.save(meta.id, b"Alte Fassung\n").await.unwrap();
    let old = history.list(meta.id).await.unwrap()[0].clone();
    f.clock.advance(11 * MINUTE);
    docs.save(meta.id, b"Neue Fassung\n").await.unwrap();
    f.clock.advance(MINUTE);
    docs.save(meta.id, b"Neueste, ungesicherte Fassung\n")
        .await
        .unwrap();

    let restored = docs
        .restore_version(uuid::Uuid::parse_str(&old.id).unwrap(), "de-DE")
        .await
        .unwrap();
    let (_, content) = docs.read(meta.id).await.unwrap();
    assert_eq!(content, b"Alte Fassung\n");
    assert_eq!(restored.format, DocumentFormat::Md);
    let versions = history.list(meta.id).await.unwrap();
    assert_eq!(versions[0].kind, VersionKind::BeforeRestore);
    let (_, saved) = history
        .content(uuid::Uuid::parse_str(&versions[0].id).unwrap())
        .await
        .unwrap();
    assert_eq!(saved, b"Neueste, ungesicherte Fassung\n");
    f.library.close().await;
}

#[tokio::test]
async fn a_version_from_before_a_conversion_comes_back_in_its_format() {
    let f = fixture().await;
    let docs = f.library.documents();
    let history = docs.history();
    let meta = docs
        .create_pwrite(
            "Reich",
            "de-DE",
            serde_json::json!({ "type": "doc", "content": [
                { "type": "paragraph", "content": [{ "type": "text", "text": "Fett", "marks": [{ "type": "bold" }] }] }
            ] }),
        )
        .await
        .unwrap();
    docs.convert(meta.id, DocumentFormat::Txt, b"Fett", "de-DE")
        .await
        .unwrap();
    let before = history.list(meta.id).await.unwrap()[0].clone();
    assert_eq!(before.kind, VersionKind::BeforeConversion);
    assert_eq!(before.format, DocumentFormat::Pwrite);
    let restored = docs
        .restore_version(uuid::Uuid::parse_str(&before.id).unwrap(), "de-DE")
        .await
        .unwrap();
    assert_eq!(restored.format, DocumentFormat::Pwrite);
    let (_, content) = docs.read(meta.id).await.unwrap();
    let file: serde_json::Value = serde_json::from_slice(&content).unwrap();
    assert_eq!(file["id"], meta.id.hyphenated().to_string());
    assert_eq!(
        file["content"]["content"][0]["content"][0]["marks"][0]["type"],
        "bold"
    );
    f.library.close().await;
}

#[tokio::test]
async fn retention_thins_out_versions_over_simulated_time() {
    let f = fixture().await;
    let docs = f.library.documents();
    let history = docs.history();
    let meta = docs
        .create("Tagebuch", DocumentFormat::Txt, b"")
        .await
        .unwrap();
    // 40 days, four sessions a day, 11 minutes apart.
    for day in 0..40 {
        for session in 0..4 {
            docs.save(meta.id, format!("Tag {day}, {session}").as_bytes())
                .await
                .unwrap();
            f.clock.advance(11 * MINUTE);
        }
        f.clock.advance(DAY - 44 * MINUTE);
    }
    docs.record_version(meta.id, VersionKind::Named, Some("Merken"))
        .await
        .unwrap();
    let versions = history.list(meta.id).await.unwrap();
    // Last 24 hours: all of the last day; up to 30 days one per day; older
    // ones one per week.
    let per_day = versions.len();
    assert!(per_day < 4 * 40, "{per_day} versions kept");
    assert!(versions.iter().any(|v| v.kind == VersionKind::Named));
    // Stricter rules apply on the next opening of the library.
    f.library
        .settings()
        .set::<HistoryKeepAllHours>(&1)
        .await
        .unwrap();
    f.library.housekeeping().await;
    let after = history.list(meta.id).await.unwrap();
    assert!(after.len() < per_day);
    assert!(after.iter().any(|v| v.name.as_deref() == Some("Merken")));
    // A version per calendar day at most within the daily window.
    let mut days: Vec<&str> = after.iter().map(|v| &v.created_at[..10]).collect();
    let total = days.len();
    days.dedup();
    assert!(total - days.len() <= 1, "{after:?}");
    f.library.close().await;
}

#[tokio::test]
async fn removing_a_document_for_good_removes_its_versions() {
    let f = fixture().await;
    let docs = f.library.documents();
    let meta = docs.create("Weg", DocumentFormat::Txt, b"").await.unwrap();
    docs.save(meta.id, b"Inhalt").await.unwrap();
    assert_eq!(docs.history().list(meta.id).await.unwrap().len(), 1);
    docs.move_to_trash(meta.id).await.unwrap();
    // In the trash the versions stay.
    assert_eq!(docs.history().list(meta.id).await.unwrap().len(), 1);
    docs.purge(meta.id).await.unwrap();
    assert!(docs.history().list(meta.id).await.unwrap().is_empty());
    f.library.close().await;
}
